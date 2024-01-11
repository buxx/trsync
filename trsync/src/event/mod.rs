use trsync_core::{client::TracimClient, local::LocalChange, remote::RemoteChange};

use crate::{local::DiskEvent, local2::reducer::DiskEventWrap};

use self::remote::RemoteEvent;

pub mod local;
pub mod remote;

#[derive(Debug, PartialEq, Eq)]
pub enum Event {
    Remote(RemoteEvent),
    Local(DiskEventWrap),
}

impl From<&RemoteChange> for Event {
    fn from(value: &RemoteChange) -> Self {
        match value {
            RemoteChange::New(content_id, _) => {
                Self::Remote(RemoteEvent::Created(content_id.clone()))
            }
            RemoteChange::Disappear(content_id, _) => {
                Self::Remote(RemoteEvent::Deleted(content_id.clone()))
            }
            RemoteChange::Updated(content_id, _) => {
                Self::Remote(RemoteEvent::Updated(content_id.clone()))
            }
        }
    }
}

impl From<&LocalChange> for Event {
    fn from(value: &LocalChange) -> Self {
        match value {
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
        }
    }
}

impl Event {
    pub fn display(&self, client: &Box<dyn TracimClient>) -> String {
        match self {
            Event::Remote(event) => {
                let path = client
                    .get_content_path(event.content_id())
                    .and_then(|v| Ok(format!("{}", v.display())))
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
                DiskEvent::Created(path) => format!("⬆🆕 {}", path.display()),
                DiskEvent::Modified(path) => format!("⬆❌ {}", path.display()),
                DiskEvent::Renamed(before_path, after_path) => {
                    format!("🖴 {} ➡ {}", before_path.display(), after_path.display())
                }
            },
        }
    }
}
