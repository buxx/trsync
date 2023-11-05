use crate::sync::{local::LocalChange, remote::RemoteChange};

use self::{local::LocalEvent, remote::RemoteEvent};

pub mod local;
pub mod remote;

#[derive(Debug)]
pub enum Event {
    Remote(RemoteEvent),
    Local(LocalEvent),
}

impl From<RemoteChange> for Event {
    fn from(value: RemoteChange) -> Self {
        match value {
            RemoteChange::New(content_id, _) => Self::Remote(RemoteEvent::Created(content_id)),
            RemoteChange::Disappear(content_id, _) => {
                Self::Remote(RemoteEvent::Deleted(content_id))
            }
            RemoteChange::Updated(content_id, _) => Self::Remote(RemoteEvent::Updated(content_id)),
        }
    }
}

impl From<LocalChange> for Event {
    fn from(value: LocalChange) -> Self {
        match value {
            LocalChange::New(path) => Self::Local(LocalEvent::Created(path)),
            LocalChange::Disappear(path, content_id) => {
                Self::Local(LocalEvent::Deleted(content_id))
            }
            LocalChange::Updated(path, content_id) => Self::Local(LocalEvent::Modified(content_id)),
        }
    }
}
