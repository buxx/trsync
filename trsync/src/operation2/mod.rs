pub mod executor;
pub mod local;
pub mod operator;
pub mod remote;

use trsync_core::instance::ContentId;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Operation {
    RemoveFromDisk(ContentId),
    CreateFromRemote(ContentId),
}
