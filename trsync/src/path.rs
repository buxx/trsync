use std::{fmt::Display, iter::FromIterator, path::PathBuf};

use trsync_core::content::Content;

#[derive(Debug, Eq, PartialEq, Clone)]
pub struct ContentPath {
    parts: Vec<Content>,
}

impl ContentPath {
    pub fn new(parts: Vec<Content>) -> Self {
        Self { parts }
    }

    pub fn to_path_buf(&self) -> PathBuf {
        PathBuf::from_iter(
            self.parts
                .iter()
                .map(|content| content.file_name().0.clone()),
        )
    }
}

impl Into<PathBuf> for ContentPath {
    fn into(self) -> PathBuf {
        self.to_path_buf()
    }
}

impl Display for ContentPath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.to_path_buf().display().to_string())
    }
}
