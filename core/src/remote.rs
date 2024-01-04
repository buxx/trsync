use std::path::PathBuf;

use crate::instance::ContentId;

#[derive(Debug, Eq, PartialEq, Clone)]
pub enum RemoteChange {
    New(ContentId, PathBuf),
    Disappear(ContentId, PathBuf),
    Updated(ContentId, PathBuf),
}

impl RemoteChange {
    pub fn path(&self) -> PathBuf {
        match self {
            RemoteChange::New(_, path)
            | RemoteChange::Disappear(_, path)
            | RemoteChange::Updated(_, path) => path.clone(),
        }
    }
}
