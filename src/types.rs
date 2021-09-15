pub type RelativeFilePath = String;
pub type AbsoluteFilePath = String;
pub type ContentId = i32;
pub type LastModifiedTimestamp = i32;
pub type EventType = String;

pub enum ContentType {
    File,
    HtmlDocument,
    Folder,
}

impl ContentType {
    pub fn from_str(str_: &str) -> Self {
        match str_ {
            "file" => Self::File,
            "html-document" => Self::HtmlDocument,
            "folder" => Self::Folder,
            _ => panic!("Content type {} not managed", str_),
        }
    }

    pub fn to_string(&self) -> String {
        match self {
            ContentType::File => "file".to_string(),
            ContentType::HtmlDocument => "html-document".to_string(),
            ContentType::Folder => "folder".to_string(),
        }
    }
}
