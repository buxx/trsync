use std::{
    fs::{self, File},
    io,
    path::{Path, PathBuf},
    sync::mpsc::Receiver,
};

use rusqlite::Connection;

use crate::{
    client::{Client, ParentIdParameter},
    context::Context,
    database::DatabaseOperation,
    error::{ClientError, OperationError},
    types::{ContentId, ContentType, RelativeFilePath},
    util::FileInfos,
};

#[derive(Debug, PartialEq)]
pub enum OperationalMessage {
    // Local files messages
    NewLocalFile(RelativeFilePath),
    ModifiedLocalFile(RelativeFilePath),
    DeletedLocalFile(RelativeFilePath),
    RenamedLocalFile(RelativeFilePath, RelativeFilePath), // before, after
    // Remote files messages
    NewRemoteFile(ContentId),
    ModifiedRemoteFile(ContentId),
    DeletedRemoteFile(ContentId),
}

// TODO : Manage a flag set to true when program start to indicate to manage conflicts.
// When resolution done, set flag to false and proceed local and remote messages without
// taking care of conflicts
pub struct OperationalHandler {
    context: Context,
    connection: Connection,
    client: Client,
    ignore_messages: Vec<OperationalMessage>,
}

impl OperationalHandler {
    pub fn new(context: Context, connection: Connection) -> Self {
        Self {
            context: context.clone(),
            connection,
            client: Client::new(context),
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
                    OperationalMessage::RenamedLocalFile(
                        before_relative_path,
                        after_relative_path,
                    ) => match self.renamed_local_file(before_relative_path, after_relative_path) {
                        Err(err) => {
                            eprintln!("{:?}", err)
                        }
                        _ => {}
                    },
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
        // Prevent known bug : new local file is sometime an existing file
        if DatabaseOperation::new(&self.connection).relative_path_is_known(&relative_path)? {
            return self.modified_local_file(relative_path.clone());
        }

        // Grab file infos
        let file_infos = FileInfos::from(self.context.folder_path.clone(), relative_path);
        let parent_id = match file_infos.parent_id(&self.connection) {
            Ok(parent_id) => parent_id,
            Err(error) => match error {
                // Parent is currently not indexed
                OperationError::UnIndexedRelativePath(parent_relative_path) => {
                    self.new_local_file(parent_relative_path.clone())?;
                    Some(
                        DatabaseOperation::new(&self.connection)
                            .get_content_id_from_path(parent_relative_path)?,
                    )
                }
                _ => return Err(error),
            },
        };

        // Create it on remote
        let (content_id, revision_id) = match self.client.create_content(
            file_infos.absolute_path,
            file_infos.content_type.clone(),
            parent_id,
        ) {
            Ok((content_id, revision_id)) => {
                // Prepare to ignore remote create event
                self.ignore_messages
                    .push(OperationalMessage::NewRemoteFile(content_id));
                // Tracim generate additional modified event when it is a file
                if file_infos.content_type == ContentType::File {
                    self.ignore_messages
                        .push(OperationalMessage::ModifiedRemoteFile(content_id));
                }
                (content_id, revision_id)
            }
            Err(ClientError::AlreadyExistResponse(existing_content_id, existing_revision_id)) => {
                (existing_content_id, existing_revision_id)
            }
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
            content_id,
            revision_id,
        )?;

        Ok(())
    }

    fn modified_local_file(
        &mut self,
        relative_path: RelativeFilePath,
    ) -> Result<(), OperationError> {
        let database_operation = DatabaseOperation::new(&self.connection);

        // Grab file infos
        let file_infos = FileInfos::from(self.context.folder_path.clone(), relative_path);
        let content_id =
            database_operation.get_content_id_from_path(file_infos.relative_path.clone())?;

        // Prepare to ignore remote create event
        self.ignore_messages
            .push(OperationalMessage::ModifiedRemoteFile(content_id));

        // Update file on remote
        let revision_id = self.client.update_content(
            file_infos.absolute_path,
            file_infos.file_name,
            file_infos.content_type,
            content_id,
        )?;

        // Update database
        database_operation.update_last_modified_timestamp(
            file_infos.relative_path.clone(),
            file_infos.last_modified_timestamp,
        )?;
        database_operation.update_revision_id(file_infos.relative_path, revision_id)?;

        Ok(())
    }

    fn deleted_local_file(&mut self, relative_path: String) -> Result<(), OperationError> {
        let database_operation = DatabaseOperation::new(&self.connection);

        // Grab file infos
        let content_id = database_operation.get_content_id_from_path(relative_path)?;

        // Delete on remote
        self.client.trash_content(content_id)?;

        // Prepare to ignore remote trashed event
        self.ignore_messages
            .push(OperationalMessage::DeletedRemoteFile(content_id));

        // Update database
        database_operation.delete_file(content_id)?;

        Ok(())
    }

    fn renamed_local_file(
        &mut self,
        before_relative_path: String,
        after_relative_path: String,
    ) -> Result<(), OperationError> {
        let before_parent_relative_path = Path::new(&before_relative_path).parent();
        let after_parent_relative_path = Path::new(&after_relative_path).parent();
        let content_id = DatabaseOperation::new(&self.connection)
            .get_content_id_from_path(before_relative_path.clone())?;
        let file_infos = FileInfos::from(
            self.context.folder_path.clone(),
            after_relative_path.clone(),
        );

        // If path changes
        if before_parent_relative_path != after_parent_relative_path {
            // If path changes for a folder
            if let Some(after_parent_relative_path_) = after_parent_relative_path {
                let after_parent_relative_path_str =
                    after_parent_relative_path_.to_str().unwrap().to_string();
                match DatabaseOperation::new(&self.connection)
                    .get_content_id_from_path(after_parent_relative_path_str.clone())
                {
                    // New parent folder is indexed, update remote with it
                    Ok(after_parent_content_id) => self.client.move_content(
                        content_id,
                        ParentIdParameter::Some(after_parent_content_id),
                    )?,
                    // New parent folder is not indexed, create it on remote
                    Err(OperationError::UnIndexedRelativePath(_)) => {
                        self.new_local_file(after_parent_relative_path_str.clone())?;
                        let after_parent_content_id = DatabaseOperation::new(&self.connection)
                            .get_content_id_from_path(after_parent_relative_path_str.clone())?;
                        self.client.move_content(
                            content_id,
                            ParentIdParameter::Some(after_parent_content_id),
                        )?
                    }
                    Err(error) => {
                        return Err(OperationError::UnexpectedError(format!("{:?}", error)))
                    }
                }
            // Or change for root
            } else {
                self.client
                    .move_content(content_id, ParentIdParameter::Root)?
            }
        }

        let before_file_name = Path::new(&before_relative_path).file_name().unwrap();
        let after_file_name = Path::new(&after_relative_path).file_name().unwrap();

        // Rename file name if changes
        if before_file_name != after_file_name {
            let new_revision_id = self.client.update_content_file_name(
                content_id,
                after_file_name.to_str().unwrap().to_string(),
                file_infos.content_type,
            )?;
            DatabaseOperation::new(&self.connection)
                .update_revision_id(after_relative_path, new_revision_id)?;
        }

        Ok(())
    }

    fn new_remote_file(&mut self, content_id: i32) -> Result<(), OperationError> {
        // Grab file infos
        let remote_content = self.client.get_remote_content(content_id)?;
        let relative_path = self.client.build_relative_path(&remote_content)?;
        let absolute_path = Path::new(&self.context.folder_path).join(&relative_path);

        // Prepare to ignore new local file
        self.ignore_messages
            .push(OperationalMessage::NewLocalFile(relative_path.clone()));

        // Check tree before create new file
        if let Some(parent_id) = remote_content.parent_id {
            // If parent content id is unknown, folder is not on disk
            if !DatabaseOperation::new(&self.connection).content_id_is_known(parent_id)? {
                // Use recursive to create this parent and possible parents parent
                self.new_remote_file(parent_id)?;
            }
        }

        // Write file/folder on disk
        if remote_content.content_type == "folder" {
            match fs::create_dir(&absolute_path) {
                Ok(_) => {}
                Err(error) => eprintln!("Error during creation of {:?} : {}", absolute_path, error),
            }
        } else {
            let mut response = self
                .client
                .get_file_content_response(remote_content.content_id, remote_content.filename)?;
            let mut out = File::create(absolute_path).unwrap();
            io::copy(&mut response, &mut out).unwrap();
        }

        // Update database
        let file_infos = FileInfos::from(self.context.folder_path.clone(), relative_path);
        let content = self.client.get_remote_content(content_id)?;
        DatabaseOperation::new(&self.connection).insert_new_file(
            file_infos.relative_path,
            file_infos.last_modified_timestamp,
            content_id,
            content.current_revision_id,
        )?;

        Ok(())
    }

    fn modified_remote_file(&mut self, content_id: i32) -> Result<(), OperationError> {
        let database_operation = DatabaseOperation::new(&self.connection);

        // Grab file infos
        let remote_content = self.client.get_remote_content(content_id)?;
        let relative_path = self.client.build_relative_path(&remote_content)?;
        let absolute_path = Path::new(&self.context.folder_path).join(&relative_path);

        // TODO : use enum for content_type
        if remote_content.content_type == "folder" {
            // TODO : manage case where file doesn't exist (in db and on disk)
            let relative_path =
                DatabaseOperation::new(&self.connection).get_path_from_content_id(content_id)?;
            let old_absolute_path = Path::new(&self.context.folder_path).join(relative_path);
            let new_absolute_path = old_absolute_path
                .parent()
                .unwrap()
                .join(remote_content.filename);
            fs::rename(old_absolute_path, &new_absolute_path).unwrap();
            // Prepare to ignore modified local file
            let new_relative_path = new_absolute_path
                .strip_prefix(&self.context.folder_path)
                .unwrap()
                .to_str()
                .unwrap()
                .to_string();
            self.ignore_messages
                .push(OperationalMessage::ModifiedLocalFile(new_relative_path));
            return Ok(());
        }

        // Manage renamed case
        let current_relative_path =
            DatabaseOperation::new(&self.connection).get_path_from_content_id(content_id)?;
        let file_infos = FileInfos::from(self.context.folder_path.clone(), current_relative_path);
        if remote_content.filename != file_infos.file_name {
            println!(
                "Rename {} into {:?}",
                file_infos.absolute_path, &absolute_path
            );
            match fs::rename(file_infos.absolute_path, &absolute_path) {
                Ok(_) => {
                    DatabaseOperation::new(&self.connection)
                        .update_relative_path(content_id, relative_path.clone())?
                    // TODO : manage local event rename by ignoring renamed event
                }
                Err(error) => return Err(OperationError::UnexpectedError(format!("{:?}", error))),
            }
        }

        // Prepare to ignore modified local file
        self.ignore_messages
            .push(OperationalMessage::ModifiedLocalFile(relative_path.clone()));

        // Write file on disk
        let mut response = self
            .client
            .get_file_content_response(content_id, remote_content.filename)?;
        // TODO : Manage case where file don't exist on disk
        println!(
            "Update disk file {:?} with content {}",
            &absolute_path, content_id
        );
        let mut out = File::create(absolute_path)?;
        io::copy(&mut response, &mut out)?;

        // Update database
        let file_infos = FileInfos::from(self.context.folder_path.clone(), relative_path);
        database_operation.update_last_modified_timestamp(
            file_infos.relative_path.clone(),
            file_infos.last_modified_timestamp,
        )?;
        database_operation
            .update_revision_id(file_infos.relative_path, remote_content.current_revision_id)?;

        Ok(())
    }

    fn deleted_remote_file(&mut self, content_id: i32) -> Result<(), OperationError> {
        let database_operation = DatabaseOperation::new(&self.connection);

        // Grab file infos (from local index, remote content has name changes)

        let relative_path =
            DatabaseOperation::new(&self.connection).get_path_from_content_id(content_id)?;
        let file_infos = FileInfos::from(self.context.folder_path.clone(), relative_path);

        // Prepare to ignore deleted local file
        self.ignore_messages
            .push(OperationalMessage::DeletedLocalFile(
                file_infos.relative_path,
            ));

        // Delete disk file
        println!("Remove disk file {:?}", &file_infos.absolute_path);
        if file_infos.is_directory {
            fs::remove_dir_all(&file_infos.absolute_path)?;
        } else {
            fs::remove_file(&file_infos.absolute_path)?;
        };

        database_operation.delete_file(content_id)?;

        Ok(())
    }
}
