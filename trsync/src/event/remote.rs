use trsync_core::instance::ContentId;

#[derive(Debug, PartialEq, Eq)]
pub enum RemoteEvent {
    Deleted(ContentId),
    Created(ContentId),
    Updated(ContentId),
    Renamed(ContentId),
}
