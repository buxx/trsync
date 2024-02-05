use std::fmt::Display;

use self::{local::LocalChange, remote::RemoteChange};

pub mod local;
pub mod remote;

#[derive(Debug, Eq, PartialEq, Clone)]
pub enum Change {
    Local(LocalChange),
    Remote(RemoteChange),
}

impl Display for Change {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Change::Local(change) => f.write_str(&change.to_string()),
            Change::Remote(change) => f.write_str(&change.to_string()),
        }
    }
}

impl From<&LocalChange> for Change {
    fn from(value: &LocalChange) -> Self {
        Self::Local(value.clone())
    }
}

impl From<&RemoteChange> for Change {
    fn from(value: &RemoteChange) -> Self {
        Self::Remote(value.clone())
    }
}
