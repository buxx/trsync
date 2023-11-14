use std::path::PathBuf;

use trsync_core::instance::ContentId;

#[derive(Debug)]
pub enum LocalEvent {
    // FIXME BS NOW : Disk event must be converted to LocalEvent with real ContentId just before be used (not too long!)
    // by taking care of event superposition. AND take care of things like folder deletion
    // will emit disk event for all children !
    Deleted(ContentId),
    Created(PathBuf),
    Modified(ContentId),
    Renamed(ContentId, PathBuf),
}
