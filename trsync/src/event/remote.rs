use trsync_core::instance::ContentId;

#[derive(Debug)]
pub enum RemoteEvent {
    Deleted(ContentId),
    Created(ContentId),
    Updated(ContentId),
    Renamed(ContentId),
}
