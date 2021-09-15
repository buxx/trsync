use std::{
    fs,
    path::{Component, Path, PathBuf},
    sync::mpsc::Receiver,
    time::UNIX_EPOCH,
};

use rusqlite::Connection;

use crate::{
    client::Client,
    database::{delete_file, get_content_id_from_path, insert_new_file, update_file},
    types::{ContentId, LastModifiedTimestamp, RelativeFilePath},
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
                    println!("IGNORE MESSAGE : {:?}", &message);
                    continue;
                };

                if self.ignore_message(&message) {
                    println!("IGNORE MESSAGE : {:?}", &message);
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

        // Create it on remote
        let content_id = self.client.create_content(
            file_infos.absolute_path,
            file_infos.file_name,
            file_infos.content_type,
            parent_id,
        );

        // Prepare to ignore remote create event
        self.ignore_messages
            .push(OperationalMessage::NewRemoteFile(content_id));

        // Update database
        insert_new_file(
            &self.connection,
            file_infos.relative_path,
            file_infos.last_modified_timestamp,
            Some(content_id),
        );
    }

    fn modified_local_file(&mut self, relative_path: RelativeFilePath) {
        // Grab file infos
        let file_infos = FileInfos::from(&self.path, relative_path);
        let content_id =
            get_content_id_from_path(&self.connection, file_infos.relative_path.clone());

        // Update file on remote
        self.client.update_content(
            file_infos.absolute_path,
            file_infos.file_name,
            file_infos.content_type,
            content_id,
        );

        // Prepare to ignore remote create event
        self.ignore_messages
            .push(OperationalMessage::ModifiedRemoteFile(content_id));

        // Update database
        update_file(
            &self.connection,
            file_infos.relative_path,
            file_infos.last_modified_timestamp,
        );
    }

    fn deleted_local_file(&mut self, relative_path: String) {
        // Grab file infos
        let content_id = get_content_id_from_path(&self.connection, relative_path);

        // Delete on remote
        self.client.trash_content(content_id);

        // Prepare to ignore remote trashed event
        self.ignore_messages
            .push(OperationalMessage::DeletedRemoteFile(content_id));

        // Update database
        delete_file(&self.connection, content_id);
    }

    fn new_remote_file(&self, content_id: i32) {
        // TODO : Get content path, filename, then content and create local file on disk
        // TODO : Insert it in database (move code here?)
    }

    fn modified_remote_file(&self, content_id: i32) {
        // TODO : Get new file content, and update local disk file (path is in db)
        // TODO : Update it in database (move code here ?)
    }

    fn deleted_remote_file(&self, content_id: i32) {
        // TODO : Delete local disk file (path is in db)
        // TODO : Delete it from database (move code here ?)
    }
}
