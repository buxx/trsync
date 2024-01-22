use trsync_core::{
    change::{local::LocalChange, remote::RemoteChange, Change},
    client::TracimClient,
};

use crate::{local::DiskEvent, local2::reducer::DiskEventWrap};

use self::remote::RemoteEvent;

pub mod local;
pub mod remote;

#[derive(Debug, PartialEq, Eq)]
pub enum Event {
    Remote(RemoteEvent),
    Local(DiskEventWrap),
}

impl From<&Change> for Event {
    fn from(value: &Change) -> Self {
        match value {
            Change::Local(change) => match change {
                LocalChange::New(path) => Self::Local(DiskEventWrap::new(
                    path.clone(),
                    DiskEvent::Created(path.clone()),
                )),
                LocalChange::Disappear(path) => Self::Local(DiskEventWrap::new(
                    path.clone(),
                    DiskEvent::Deleted(path.clone()),
                )),
                LocalChange::Updated(path) => Self::Local(DiskEventWrap::new(
                    path.clone(),
                    DiskEvent::Modified(path.clone()),
                )),
            },
            Change::Remote(change) => match change {
                RemoteChange::New(content_id, _) => {
                    Self::Remote(RemoteEvent::Created(*content_id))
                }
                RemoteChange::Disappear(content_id, _) => {
                    Self::Remote(RemoteEvent::Deleted(*content_id))
                }
                RemoteChange::Updated(content_id, _) => {
                    Self::Remote(RemoteEvent::Updated(*content_id))
                }
            },
        }
    }
}

impl Event {
    pub fn display(&self, client: &Box<dyn TracimClient>) -> String {
        match self {
            Event::Remote(event) => {
                let path = client
                    .get_content_path(event.content_id()).map(|v| format!("{}", v.display()))
                    .unwrap_or("?".to_string());
                match event {
                    RemoteEvent::Deleted(_) => format!("☁❌ {}", path),
                    RemoteEvent::Created(_) => format!("☁🆕 {}", path),
                    RemoteEvent::Updated(_) => format!("☁⬇ {}", path),
                    RemoteEvent::Renamed(_) => format!("☁⬇ {}", path),
                }
            }
            Event::Local(event) => match &event.1 {
                DiskEvent::Deleted(path) => format!("🖴❌ {}", path.display()),
                DiskEvent::Created(path) => format!("🖴🆕 {}", path.display()),
                DiskEvent::Modified(path) => format!("🖴❌ {}", path.display()),
                DiskEvent::Renamed(before_path, after_path) => {
                    format!("🖴 {} ➡ {}", before_path.display(), after_path.display())
                }
            },
        }
    }
}
