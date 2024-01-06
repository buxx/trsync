use trsync_core::{local::LocalChange, remote::RemoteChange, sync::SyncPolitic};

pub struct ConfirmationSyncPolitic {}

impl SyncPolitic for ConfirmationSyncPolitic {
    fn deal(
        &self,
        remote_changes: Vec<RemoteChange>,
        local_changes: Vec<LocalChange>,
    ) -> Result<(Vec<RemoteChange>, Vec<LocalChange>), trsync_core::sync::SyncPoliticError> {
        todo!()
    }
}
