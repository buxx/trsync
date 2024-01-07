use std::path::PathBuf;

#[derive(Debug, Eq, PartialEq, Clone)]
pub enum LocalChange {
    New(PathBuf),
    Disappear(PathBuf),
    Updated(PathBuf),
}

impl LocalChange {
    pub fn path(&self) -> PathBuf {
        match self {
            LocalChange::New(path) | LocalChange::Disappear(path) | LocalChange::Updated(path) => {
                path.clone()
            }
        }
    }

    pub fn utf8_icon(&self) -> char {
        match self {
            LocalChange::New(_) => '⬆',
            LocalChange::Disappear(_) => '❌',
            LocalChange::Updated(_) => '⬆',
        }
    }
}
