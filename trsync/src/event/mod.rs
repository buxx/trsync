use crate::{
    local::DiskEvent,
    local2::reducer::DiskEventWrap,
    sync::{local::LocalChange, remote::RemoteChange},
};

use self::remote::RemoteEvent;

pub mod local;
pub mod remote;

#[derive(Debug, PartialEq, Eq)]
pub enum Event {
    Remote(RemoteEvent),
    Local(DiskEventWrap),
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
            LocalChange::New(path) => {
                Self::Local(DiskEventWrap::new(path.clone(), DiskEvent::Created(path)))
            }
            LocalChange::Disappear(path) => {
                Self::Local(DiskEventWrap::new(path.clone(), DiskEvent::Deleted(path)))
            }
            LocalChange::Updated(path) => {
                Self::Local(DiskEventWrap::new(path.clone(), DiskEvent::Modified(path)))
            }
        }
    }
}
