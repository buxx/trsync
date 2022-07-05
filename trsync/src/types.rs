pub type RelativeFilePath = String;
pub type AbsoluteFilePath = String;
pub type ContentId = i32;
pub type RevisionId = i32;
pub type LastModifiedTimestamp = i64;
pub type EventType = String;

#[derive(PartialEq, Clone)]
pub enum ContentType {
    File,
    Folder,
    HtmlDocument,
}

impl ContentType {
    pub fn from_str(str_: &str) -> Option<Self> {
        match str_ {
            "file" => Some(Self::File),
            "folder" => Some(Self::Folder),
            "html-document" => Some(Self::HtmlDocument),
            _ => None,
        }
    }

    pub fn to_string(&self) -> String {
        match self {
            ContentType::File => "file".to_string(),
            ContentType::Folder => "folder".to_string(),
            ContentType::HtmlDocument => "html-document".to_string(),
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
