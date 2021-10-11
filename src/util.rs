use std::{
    path::{Component, Path},
    time::UNIX_EPOCH,
};

use rusqlite::Connection;

use crate::{
    database::DatabaseOperation,
    error::Error,
    types::{AbsoluteFilePath, ContentId, ContentType, LastModifiedTimestamp, RelativeFilePath},
};

pub struct FileInfos {
    pub file_name: String,
    pub is_directory: bool,
    pub last_modified_timestamp: LastModifiedTimestamp,
    pub relative_path: RelativeFilePath,
    pub absolute_path: AbsoluteFilePath,
    pub parent_relative_path: Option<RelativeFilePath>,
    pub content_type: ContentType,
}

impl FileInfos {
    pub fn from(
        workspace_path: String,
        relative_file_path: RelativeFilePath,
    ) -> Result<Self, Error> {
        let absolute_path_buf = Path::new(&workspace_path).join(&relative_file_path);
        let absolute_path = absolute_path_buf.as_path();
        log::debug!("Build file infos from {:?}", &absolute_path);
        let file_name = absolute_path
            .file_name()
            .ok_or(Error::PathManipulationError(format!(
                "Unable to read file name of {:?}",
                &absolute_path
            )))?
            .to_str()
            .ok_or(Error::PathCastingError(format!(
                "Unable to convert {:?} file name into str",
                &absolute_path
            )))?
            .to_string();
        let relative_path_path = Path::new(&relative_file_path);
        let path_components: Vec<Component> = relative_path_path.components().collect();
        // TODO : to utils
        let parent_relative_path = if path_components.len() > 1 {
            Some(
                absolute_path
                    .parent()
                    .ok_or(Error::PathManipulationError(format!(
                        "Unable to find parent of {:?}",
                        &absolute_path
                    )))?
                    .strip_prefix(workspace_path)?
                    .to_str()
                    .ok_or(Error::PathCastingError(format!(
                        "Unable to convert {:?} into str",
                        &absolute_path
                    )))?
                    .to_string(),
            )
        } else {
            None
        };
        let content_type = if absolute_path.is_dir() {
            ContentType::Folder
        } else {
            ContentType::File
        };
        let metadata = absolute_path.metadata()?;
        let modified = metadata.modified()?;
        let since_epoch = modified.duration_since(UNIX_EPOCH)?;
        let last_modified_timestamp = since_epoch.as_millis() as LastModifiedTimestamp;
        let is_directory = absolute_path.is_dir();

        Ok(Self {
            file_name,
            is_directory,
            last_modified_timestamp,
            relative_path: relative_file_path,
            absolute_path: absolute_path.to_str().unwrap().to_string(),
            parent_relative_path,
            content_type,
        })
    }

    pub fn parent_id(&self, connection: &Connection) -> Result<Option<ContentId>, Error> {
        if let Some(parent_relative_path) = &self.parent_relative_path {
            Ok(Some(
                DatabaseOperation::new(connection)
                    .get_content_id_from_path(parent_relative_path.to_string())?,
            ))
        } else {
            Ok(None)
        }
    }
}
