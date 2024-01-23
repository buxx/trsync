use std::{fmt::Display, path::Path, str::FromStr};

use thiserror::Error;

use crate::HTML_DOCUMENT_LOCAL_EXTENSION;

pub type RelativeFilePath = String;
pub type AbsoluteFilePath = String;
pub type ContentId = i32;
pub type RevisionId = i32;
pub type LastModifiedTimestamp = i64;
pub type EventType = String;

#[derive(Eq, PartialEq, Clone, Debug, Copy)]
pub enum ContentType {
    File,
    Folder,
    HtmlDocument,
}

impl Display for ContentType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ContentType::File => f.write_str("file"),
            ContentType::Folder => f.write_str("folder"),
            ContentType::HtmlDocument => f.write_str("html-document"),
        }
    }
}

#[derive(Error, Debug)]
pub struct ParseContentTypeStrError(String);

impl Display for ParseContentTypeStrError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&format!("Unknown content type '{}'", self.0))
    }
}

impl FromStr for ContentType {
    type Err = ParseContentTypeStrError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "file" => Ok(Self::File),
            "folder" => Ok(Self::Folder),
            "html-document" => Ok(Self::HtmlDocument),
            _ => Err(ParseContentTypeStrError(s.to_string())),
        }
    }
}

impl ContentType {
    pub fn from_path(path: &Path) -> Self {
        if path.is_dir() {
            Self::Folder
        } else if path
            .display()
            .to_string()
            .ends_with(HTML_DOCUMENT_LOCAL_EXTENSION)
        {
            Self::HtmlDocument
        } else {
            Self::File
        }
    }

    pub fn fillable(&self) -> bool {
        match self {
            ContentType::File | ContentType::HtmlDocument => true,
            ContentType::Folder => false,
        }
    }

    pub fn url_prefix(&self) -> String {
        match self {
            ContentType::Folder => "folders",
            ContentType::File => "files",
            ContentType::HtmlDocument => "html-documents",
        }
        .to_string()
    }

    pub fn label_minus_pos(&self) -> usize {
        match self {
            ContentType::HtmlDocument => 3,
            ContentType::File | ContentType::Folder => 2,
        }
    }
}

#[derive(PartialEq)]
pub enum RemoteEventType {
    Created,
    Modified,
    Deleted,
}

impl RemoteEventType {
    pub fn from_str(str_: &str) -> Option<Self> {
        match str_ {
            "content.modified.file" => Some(Self::Modified),
            "content.modified.html-document" => Some(Self::Modified),
            "content.modified.folder" => Some(Self::Modified),
            "content.created.file" => Some(Self::Created),
            "content.created.html-document" => Some(Self::Created),
            "content.created.folder" => Some(Self::Created),
            "content.deleted.html-document" => Some(Self::Deleted),
            "content.deleted.file" => Some(Self::Deleted),
            "content.deleted.folder" => Some(Self::Deleted),
            "content.undeleted.html-document" => Some(Self::Created),
            "content.undeleted.file" => Some(Self::Created),
            "content.undeleted.folder" => Some(Self::Created),
            _ => None,
        }
    }
}
