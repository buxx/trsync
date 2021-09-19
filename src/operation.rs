use std::{
    fs::{self, File},
    io,
    path::{Path, PathBuf},
    sync::mpsc::Receiver,
};

use rusqlite::Connection;

use crate::{
    client::Client,
    database::{Database, DatabaseOperation},
    error::{ClientError, OperationError},
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
                        match self.new_local_file(relative_path) {
                            Err(err) => {
                                eprintln!("{:?}", err)
                            }
                            _ => {}
                        }
                    }
                    OperationalMessage::ModifiedLocalFile(relative_path) => {
                        match self.modified_local_file(relative_path) {
                            Err(err) => {
                                eprintln!("{:?}", err)
                            }
                            _ => {}
                        }
                    }
                    OperationalMessage::DeletedLocalFile(relative_path) => {
                        match self.deleted_local_file(relative_path) {
                            Err(err) => {
                                eprintln!("{:?}", err)
                            }
                            _ => {}
                        }
                    }
                    // Remote changes
                    OperationalMessage::NewRemoteFile(content_id) => {
                        match self.new_remote_file(content_id) {
                            Err(err) => {
                                eprintln!("{:?}", err)
                            }
                            _ => {}
                        }
                    }
                    OperationalMessage::ModifiedRemoteFile(content_id) => {
                        match self.modified_remote_file(content_id) {
                            Err(err) => {
                                eprintln!("{:?}", err)
                            }
                            _ => {}
                        }
                    }
                    OperationalMessage::DeletedRemoteFile(content_id) => {
                        match self.deleted_remote_file(content_id) {
                            Err(err) => {
                                eprintln!("{:?}", err)
                            }
                            _ => {}
                        }
                    }
                }
            }
        }
    }

    fn new_local_file(&mut self, relative_path: String) -> Result<(), OperationError> {
        // Grab file infos
        let file_infos = FileInfos::from(&self.path, relative_path);
        let parent_id = file_infos.parent_id(&self.connection);

        // FIXME : for each parent folders, create it on remote if required

        // Create it on remote
        let content_id = match self.client.create_content(
            file_infos.absolute_path,
            file_infos.content_type,
            parent_id,
        ) {
            Ok(content_id) => {
                // Prepare to ignore remote create event
                self.ignore_messages
                    .push(OperationalMessage::NewRemoteFile(content_id));
                content_id
            }
            Err(ClientError::AlreadyExistResponse(existing_content_id)) => existing_content_id,
            Err(err) => {
                return Err(OperationError::FailToCreateContentOnRemote(format!(
                    "Fail to create new local file on remote : {:?}",
                    err
                )))
            }
        };

        // Update database
        DatabaseOperation::new(&self.connection).insert_new_file(
            file_infos.relative_path,
            file_infos.last_modified_timestamp,
            Some(content_id),
        );

        Ok(())
    }

    fn modified_local_file(
        &mut self,
        relative_path: RelativeFilePath,
    ) -> Result<(), OperationError> {
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
        )?;

        // Update database
        database_operation
            .update_file(file_infos.relative_path, file_infos.last_modified_timestamp);

        Ok(())
    }

    fn deleted_local_file(&mut self, relative_path: String) -> Result<(), OperationError> {
        let database_operation = DatabaseOperation::new(&self.connection);

        // Grab file infos
        let content_id = database_operation.get_content_id_from_path(relative_path);

        // Delete on remote
        self.client.trash_content(content_id)?;

        // Prepare to ignore remote trashed event
        self.ignore_messages
            .push(OperationalMessage::DeletedRemoteFile(content_id));

        // Update database
        database_operation.delete_file(content_id);

        Ok(())
    }

    fn new_remote_file(&mut self, content_id: i32) -> Result<(), OperationError> {
        // Grab file infos
        let remote_content = self.client.get_remote_content(content_id)?;
        let relative_path = self.client.build_relative_path(&remote_content)?;
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
                .get_file_content_response(remote_content.content_id, remote_content.filename)?;
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

        Ok(())
    }

    fn modified_remote_file(&mut self, content_id: i32) -> Result<(), OperationError> {
        let database_operation = DatabaseOperation::new(&self.connection);

        // Grab file infos
        let remote_content = self.client.get_remote_content(content_id)?;
        let relative_path = self.client.build_relative_path(&remote_content)?;
        let absolute_path = self.path.join(&relative_path);

        // TODO : use enum for content_type
        if remote_content.content_type == "folder" {
            // TODO : manage case where file doesn't exist (in db and on disk)
            let relative_path =
                DatabaseOperation::new(&self.connection).get_path_from_content_id(content_id);
            let old_absolute_path = self.path.join(relative_path);
            let new_absolute_path = old_absolute_path
                .parent()
                .unwrap()
                .join(remote_content.filename);
            fs::rename(old_absolute_path, new_absolute_path).unwrap();
            return Ok(());
        }

        // Prepare to ignore modified local file
        self.ignore_messages
            .push(OperationalMessage::ModifiedLocalFile(relative_path.clone()));

        // Write file on disk
        let mut response = self
            .client
            .get_file_content_response(content_id, remote_content.filename)?;
        // TODO : Manage case where file don't exist on disk
        let mut out = File::open(absolute_path).unwrap();
        io::copy(&mut response, &mut out).unwrap();

        // Update database
        let file_infos = FileInfos::from(&self.path, relative_path);
        database_operation
            .update_file(file_infos.relative_path, file_infos.last_modified_timestamp);

        Ok(())
    }

    fn deleted_remote_file(&mut self, content_id: i32) -> Result<(), OperationError> {
        let database_operation = DatabaseOperation::new(&self.connection);

        // Grab file infos (by chance, Tracim don't remove really file and we can access to these infos)
        let remote_content = self.client.get_remote_content(content_id)?;
        let relative_path = self.client.build_relative_path(&remote_content)?;
        let absolute_path = self.path.join(&relative_path);

        // Prepare to ignore deleted local file
        self.ignore_messages
            .push(OperationalMessage::DeletedLocalFile(relative_path.clone()));

        // Delete disk file
        fs::remove_file(absolute_path).unwrap();
        database_operation.delete_file(content_id);

        Ok(())
    }
}
