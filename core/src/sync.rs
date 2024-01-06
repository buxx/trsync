use thiserror::Error;

use crate::{local::LocalChange, remote::RemoteChange};

pub trait SyncPolitic {
    fn deal(
        &self,
        remote_changes: Vec<RemoteChange>,
        local_changes: Vec<LocalChange>,
    ) -> Result<(Vec<RemoteChange>, Vec<LocalChange>), SyncPoliticError>;
}

#[derive(Debug, Error)]
pub enum SyncPoliticError {}

/// A sync politic which accept all remote and local change without any human intervention
pub struct AcceptAllSyncPolitic;

impl SyncPolitic for AcceptAllSyncPolitic {
    fn deal(
        &self,
        remote_changes: Vec<RemoteChange>,
        local_changes: Vec<LocalChange>,
    ) -> Result<(Vec<RemoteChange>, Vec<LocalChange>), SyncPoliticError> {
        Ok((remote_changes, local_changes))
    }
}
