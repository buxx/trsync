use std::{
    io,
    path::{Component, Path, PathBuf},
    time::{Duration, UNIX_EPOCH},
};

use anyhow::Result as AnyHowResult;
use minidom::Element;

use std::fs;

use crate::error::Error;

pub fn last_modified_timestamp(path: &Path) -> AnyHowResult<Duration> {
    let metadata = path.metadata()?;
    let modified = metadata.modified()?;
    Ok(modified.duration_since(UNIX_EPOCH)?)
}

pub trait TryRemove<T> {
    fn try_remove(&mut self, index: usize) -> Option<T>;
}

impl<T> TryRemove<T> for Vec<T> {
    fn try_remove(&mut self, index: usize) -> Option<T> {
        if self.len() > index {
            Some(self.remove(index))
        } else {
            None
        }
    }
}

pub fn ignore_file(relative_path: &Path) -> bool {
    // TODO : patterns from config object
    if let Some(file_name) = relative_path.file_name() {
        if let Some(file_name_) = file_name.to_str() {
            let file_name_as_str = file_name_.to_string();
            if file_name_as_str.starts_with('.')
                || file_name_as_str.starts_with('~')
                || file_name_as_str.ends_with('~')
                || file_name_as_str.starts_with('#')
            {
                return true;
            }
        }
    }
    false
}

// FIXME BS NOW : a dir rename in offline will be lost : store on disk seen changes: test rename in offline mode (or all other changes ?) and do the "waiting changes) operations"
