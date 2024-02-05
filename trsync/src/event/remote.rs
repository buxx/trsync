use trsync_core::instance::ContentId;

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum RemoteEvent {
    Deleted(ContentId),
    Created(ContentId),
    Updated(ContentId),
    Renamed(ContentId),
}
impl RemoteEvent {
    pub fn content_id(&self) -> ContentId {
        match self {
            RemoteEvent::Deleted(content_id)
            | RemoteEvent::Created(content_id)
            | RemoteEvent::Updated(content_id)
            | RemoteEvent::Renamed(content_id) => *content_id,
        }
    }
}
