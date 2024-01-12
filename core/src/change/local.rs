use std::{fmt::Display, path::PathBuf};

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

    pub fn utf8_icon(&self) -> &str {
        match self {
            LocalChange::New(_) => "🖴🆕",
            LocalChange::Disappear(_) => "🖴❌",
            LocalChange::Updated(_) => "🖴⬆",
        }
    }
}

impl Display for LocalChange {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&format!("{} {}", self.utf8_icon(), self.path().display()))
    }
}
