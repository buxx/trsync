use std::{fmt::Display, path::PathBuf};

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

    pub fn utf8_icon(&self) -> &str {
        match self {
            RemoteChange::New(_, _) => "☁🆕",
            RemoteChange::Disappear(_, _) => "☁❌",
            RemoteChange::Updated(_, _) => "☁⬇",
        }
    }
}

impl Display for RemoteChange {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&format!("{} {}", self.utf8_icon(), self.path().display()))
    }
}
