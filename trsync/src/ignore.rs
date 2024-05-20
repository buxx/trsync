use std::{
    convert::TryFrom,
    fs::{self, OpenOptions},
    io::{self, Write},
};

use async_std::path::Path;
use thiserror::Error;
use trsync_core::instance::ContentId;

use crate::context::Context;

#[derive(Error, Debug)]
pub enum IgnoreError {
    #[error("Io error: {0}")]
    IoError(#[from] io::Error),
}

#[derive(Clone)]
pub struct Ignore {
    content_ids: Vec<ContentId>,
}

impl TryFrom<&Context> for Ignore {
    type Error = IgnoreError;

    fn try_from(value: &Context) -> Result<Self, Self::Error> {
        let path = Path::new(&value.folder_path).join(".trsyncignore");
        let file_content = match fs::read_to_string(path) {
            Ok(content) => content,
            Err(error) => match error.kind() {
                io::ErrorKind::NotFound => "".to_string(),
                _ => return Err(IgnoreError::IoError(error)),
            },
        };

        let mut content_ids = vec![];
        for line in file_content.lines() {
            if line.starts_with('#') {
                if let Ok(content_id_raw) = line.strip_prefix('#').unwrap_or("").parse() {
                    content_ids.push(ContentId(content_id_raw));
                }
            }
        }

        Ok(Self { content_ids })
    }
}

impl From<&Ignore> for String {
    fn from(value: &Ignore) -> Self {
        let mut lines = vec![];
        for content_id in value.content_ids() {
            lines.push(format!("#{}", content_id.0))
        }
        lines.join("\n")
    }
}

impl Ignore {
    pub fn empty() -> Self {
        Self {
            content_ids: vec![],
        }
    }

    pub fn push(&mut self, content_id: ContentId) {
        self.content_ids.push(content_id)
    }

    pub fn content_ids(&self) -> &[ContentId] {
        &self.content_ids
    }

    pub fn is_ignored(&self, content_id: &ContentId) -> bool {
        self.content_ids.contains(content_id)
    }

    pub fn write(&self, context: &Context) -> Result<(), io::Error> {
        let path = Path::new(&context.folder_path).join(".trsyncignore");
        let content: String = self.into();
        OpenOptions::new()
            .write(true)
            .create(true)
            .open(&path)?
            .write_all(content.as_bytes())
    }
}
