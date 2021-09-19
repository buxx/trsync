use std::{
    path::{Component, Path, PathBuf},
    time::UNIX_EPOCH,
};

use rusqlite::Connection;

use crate::{
    database::DatabaseOperation,
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
    pub fn from(workspace_path: &PathBuf, relative_file_path: RelativeFilePath) -> Self {
        let absolute_path_buf = workspace_path.join(&relative_file_path);
        let absolute_path = absolute_path_buf.as_path();
        let file_name = absolute_path
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .to_string();
        let relative_path_path = Path::new(&relative_file_path);
        let path_components: Vec<Component> = relative_path_path.components().collect();
        let parent_relative_path = if path_components.len() > 1 {
            Some(
                absolute_path
                    .parent()
                    .unwrap()
                    .strip_prefix(workspace_path)
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .to_string(),
            )
        } else {
            None
        };
        let content_type = if absolute_path.is_dir() {
            ContentType::Folder
        } else if absolute_path.ends_with(".html") {
            ContentType::HtmlDocument
        } else {
            ContentType::File
        };
        let metadata = absolute_path.metadata().unwrap();
        let modified = metadata.modified().unwrap();
        let since_epoch = modified.duration_since(UNIX_EPOCH).unwrap();
        let last_modified_timestamp = since_epoch.as_millis() as LastModifiedTimestamp;
        let is_directory = absolute_path.is_dir();

        Self {
            file_name,
            is_directory,
            last_modified_timestamp,
            relative_path: relative_file_path,
            absolute_path: absolute_path.to_str().unwrap().to_string(),
            parent_relative_path,
            content_type,
        }
    }

    pub fn parent_id(&self, connection: &Connection) -> Option<ContentId> {
        if let Some(parent_relative_path) = &self.parent_relative_path {
            let content_id = DatabaseOperation::new(connection)
                .get_content_id_from_path(parent_relative_path.to_string());
            Some(content_id)
        } else {
            None
        }
    }
}
