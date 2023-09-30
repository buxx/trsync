use std::{fmt::Display, iter::FromIterator, path::PathBuf};

use trsync_core::content::Content;

pub struct ContentPath<'a> {
    parts: Vec<&'a Content>,
}

impl<'a> ContentPath<'a> {
    pub fn new(parts: Vec<&'a Content>) -> Self {
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

impl<'a> Into<PathBuf> for ContentPath<'a> {
    fn into(self) -> PathBuf {
        self.to_path_buf()
    }
}

impl<'a> Display for ContentPath<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.to_path_buf().display().to_string())
    }
}
