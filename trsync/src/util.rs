use std::{
    ffi::OsStr,
    io,
    path::{Component, Path, PathBuf},
    time::UNIX_EPOCH,
};

use rusqlite::Connection;
use std::fs;

use crate::{
    database::DatabaseOperation,
    error::Error,
    types::{AbsoluteFilePath, ContentId, ContentType, LastModifiedTimestamp, RelativeFilePath},
    util,
};

// This extension must match with Tracim content "filename"
pub const HTML_DOCUMENT_LOCAL_EXTENSION: &'static str = ".document.html";

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
        let parent_relative_path = parent_relative_path_from_path_components(
            &path_components,
            &absolute_path,
            &workspace_path,
        )?;

        let content_type = if absolute_path.is_dir() {
            ContentType::Folder
        } else if absolute_path
            .file_name()
            .and_then(OsStr::to_str)
            .unwrap_or("")
            .ends_with(HTML_DOCUMENT_LOCAL_EXTENSION)
        {
            ContentType::HtmlDocument
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
            absolute_path: util::path_to_string(absolute_path)?,
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

pub fn parent_relative_path_from_path_components(
    path_components: &Vec<Component>,
    absolute_path: &Path,
    workspace_path: &str,
) -> Result<Option<String>, Error> {
    if path_components.len() > 1 {
        Ok(Some(
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
        ))
    } else {
        Ok(None)
    }
}

pub fn canonicalize_to_string(path: &PathBuf) -> Result<String, Error> {
    Ok(fs::canonicalize(path)?
        .to_str()
        .ok_or(Error::PathCastingError(format!(
            "Error when interpreting path '{:?}'",
            path
        )))?
        .to_string())
}

pub fn string_path_file_name(path: &str) -> Result<String, Error> {
    Ok(Path::new(path)
        .file_name()
        .ok_or(Error::PathCastingError(format!(
            "Fail to get file name of {:?}",
            path
        )))?
        .to_str()
        .ok_or(Error::PathCastingError(format!(
            "Fail to str type of file name from {:?}",
            path
        )))?
        .to_string())
}

pub fn path_to_string(path: &Path) -> Result<String, Error> {
    Ok(path
        .to_str()
        .ok_or(Error::PathManipulationError(format!(
            "Error when manipulate path {:?}",
            path,
        )))?
        .to_string())
}

pub fn io_error_to_log_level(error: &io::Error) -> log::Level {
    match error.kind() {
        io::ErrorKind::AlreadyExists => log::Level::Info,
        _ => log::Level::Error,
    }
}

pub fn extract_html_body_content(content: &str) -> Result<String, String> {
    // let dom = Dom::parse(content)?;

    // for node in dom.children {
    //     match node {
    //         html_parser::Node::Element(element) => {
    //             if element.name == "body" {
    //                 return Ok(element.children.join(""));
    //             }
    //         }
    //         _ => {}
    //     }
    //         },
    //         _ => {}
    //     }
    // }

    // If body node not found, consider given content is body content
    Ok(content.to_string())
}
