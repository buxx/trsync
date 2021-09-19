use std::{
    fs::{self, File},
    io,
    path::{Path, PathBuf},
    sync::mpsc::Receiver,
};

use rusqlite::Connection;

use crate::{
    client::Client,
    database::DatabaseOperation,
    types::{ContentId, RelativeFilePath},
    util::FileInfos,
};

#[derive(Debug, PartialEq)]
pub enum OperationalMessage {
    // Local files messages
    NewLocalFile(RelativeFilePath),
    ModifiedLocalFile(RelativeFilePath),
    DeletedLocalFile(RelativeFilePath),
    // Remote files messages
    NewRemoteFile(ContentId),
    ModifiedRemoteFile(ContentId),
    DeletedRemoteFile(ContentId),
}

// TODO : Manage a flag set to true when program start to indicate to manage conflicts.
// When resolution done, set flag to false and proceed local and remote messages without
// taking care of conflicts
pub struct OperationalHandler {
    connection: Connection,
    client: Client,
    path: PathBuf,
    ignore_messages: Vec<OperationalMessage>,
}

impl OperationalHandler {
    pub fn new(connection: Connection, client: Client, path: PathBuf) -> Self {
        Self {
            connection,
            client,
            path: fs::canonicalize(&path).unwrap(),
            ignore_messages: vec![],
        }
    }

    fn ignore_message(&self, message: &OperationalMessage) -> bool {
        // TODO : For local files, ignore some patterns given by config : eg. ".*", "*~"
        match message {
            OperationalMessage::NewLocalFile(relative_path)
            | OperationalMessage::ModifiedLocalFile(relative_path)
            | OperationalMessage::DeletedLocalFile(relative_path) => {
                Path::new(relative_path)
                    .file_name()
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .starts_with(".")
                    | Path::new(relative_path)
                        .file_name()
                        .unwrap()
                        .to_str()
                        .unwrap()
                        .ends_with("~")
            }
            _ => false,
        }
    }

    pub fn listen(&mut self, receiver: Receiver<OperationalMessage>) {
        // TODO : Why loop is required ?!
        loop {
            for message in receiver.recv() {
                if self.ignore_messages.contains(&message) {
                    self.ignore_messages.retain(|x| *x != message);
                    println!("IGNORE MESSAGE (1) : {:?}", &message);
                    continue;
                };

                if self.ignore_message(&message) {
                    println!("IGNORE MESSAGE (2) : {:?}", &message);
                    continue;
                }

                println!("MESSAGE : {:?}", &message);

                match message {
                    // Local changes
                    OperationalMessage::NewLocalFile(relative_path) => {
                        self.new_local_file(relative_path)
                    }
                    OperationalMessage::ModifiedLocalFile(relative_path) => {
                        self.modified_local_file(relative_path)
                    }
                    OperationalMessage::DeletedLocalFile(relative_path) => {
                        self.deleted_local_file(relative_path)
                    }
                    // Remote changes
                    OperationalMessage::NewRemoteFile(content_id) => {
                        self.new_remote_file(content_id)
                    }
                    OperationalMessage::ModifiedRemoteFile(content_id) => {
                        self.modified_remote_file(content_id)
                    }
                    OperationalMessage::DeletedRemoteFile(content_id) => {
                        self.deleted_remote_file(content_id)
                    }
                }
            }
        }
    }

    fn new_local_file(&mut self, relative_path: String) {
        // Grab file infos
        let file_infos = FileInfos::from(&self.path, relative_path);
        let parent_id = file_infos.parent_id(&self.connection);

        // FIXME : for each parent folders, create it on remote if required

        // Create it on remote
        let content_id = self
            .client
            .create_content(
                file_infos.absolute_path,
                file_infos.file_name,
                file_infos.content_type,
                parent_id,
            )
            .unwrap();

        // Prepare to ignore remote create event
        self.ignore_messages
            .push(OperationalMessage::NewRemoteFile(content_id));

        // Update database
        DatabaseOperation::new(&self.connection).insert_new_file(
            file_infos.relative_path,
            file_infos.last_modified_timestamp,
            Some(content_id),
        );
    }

    fn modified_local_file(&mut self, relative_path: RelativeFilePath) {
        let database_operation = DatabaseOperation::new(&self.connection);

        // Grab file infos
        let file_infos = FileInfos::from(&self.path, relative_path);
        let content_id =
            database_operation.get_content_id_from_path(file_infos.relative_path.clone());

        // Prepare to ignore remote create event
        self.ignore_messages
            .push(OperationalMessage::ModifiedRemoteFile(content_id));

        // Update file on remote
        self.client.update_content(
            file_infos.absolute_path,
            file_infos.file_name,
            file_infos.content_type,
            content_id,
        );

        // Update database
        database_operation
            .update_file(file_infos.relative_path, file_infos.last_modified_timestamp);
    }

    fn deleted_local_file(&mut self, relative_path: String) {
        let database_operation = DatabaseOperation::new(&self.connection);

        // Grab file infos
        let content_id = database_operation.get_content_id_from_path(relative_path);

        // Delete on remote
        self.client.trash_content(content_id);

        // Prepare to ignore remote trashed event
        self.ignore_messages
            .push(OperationalMessage::DeletedRemoteFile(content_id));

        // Update database
        database_operation.delete_file(content_id);
    }

    fn new_remote_file(&mut self, content_id: i32) {
        // Grab file infos
        let remote_content = self.client.get_remote_content(content_id);
        let relative_path = self.client.build_relative_path(&remote_content);
        let absolute_path = self.path.join(&relative_path);

        // Prepare to ignore new local file
        self.ignore_messages
            .push(OperationalMessage::NewLocalFile(relative_path.clone()));

        // Check tree before create new file
        if let Some(parent_id) = remote_content.parent_id {
            // If parent content id is unknown, folder is not on disk
            if !DatabaseOperation::new(&self.connection).content_id_is_known(parent_id) {
                // Use recursive to create this parent and possible parents parent
                self.new_remote_file(parent_id);
            }
        }

        // Write file/folder on disk
        if remote_content.content_type == "folder" {
            fs::create_dir(absolute_path).unwrap();
        } else {
            let mut response = self
                .client
                .get_file_content_response(remote_content.content_id, remote_content.filename);
            let mut out = File::create(absolute_path).unwrap();
            io::copy(&mut response, &mut out).unwrap();
        }

        // Update database
        let file_infos = FileInfos::from(&self.path, relative_path);
        DatabaseOperation::new(&self.connection).insert_new_file(
            file_infos.relative_path,
            file_infos.last_modified_timestamp,
            Some(content_id),
        );
    }

    fn modified_remote_file(&mut self, content_id: i32) {
        let database_operation = DatabaseOperation::new(&self.connection);

        // Grab file infos
        let remote_content = self.client.get_remote_content(content_id);
        let relative_path = self.client.build_relative_path(&remote_content);
        let absolute_path = self.path.join(&relative_path);

        if remote_content.content_type == "folder" {
            return;
        }

        // Prepare to ignore modified local file
        self.ignore_messages
            .push(OperationalMessage::ModifiedLocalFile(relative_path.clone()));

        // Write file on disk
        let mut response = self
            .client
            .get_file_content_response(content_id, remote_content.filename);
        let mut out = File::open(absolute_path).unwrap();
        io::copy(&mut response, &mut out).unwrap();

        // Update database
        let file_infos = FileInfos::from(&self.path, relative_path);
        database_operation
            .update_file(file_infos.relative_path, file_infos.last_modified_timestamp);
    }

    fn deleted_remote_file(&mut self, content_id: i32) {
        let database_operation = DatabaseOperation::new(&self.connection);

        // Grab file infos (by chance, Tracim don't remove really file and we can access to these infos)
        let remote_content = self.client.get_remote_content(content_id);
        let relative_path = self.client.build_relative_path(&remote_content);
        let absolute_path = self.path.join(&relative_path);

        // Prepare to ignore deleted local file
        self.ignore_messages
            .push(OperationalMessage::DeletedLocalFile(relative_path.clone()));

        // Delete disk file
        fs::remove_file(absolute_path).unwrap();
        database_operation.delete_file(content_id);
    }
}
