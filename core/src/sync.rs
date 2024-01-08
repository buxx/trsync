use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use crossbeam_channel::{unbounded, Receiver, Sender};
use thiserror::Error;

use crate::{
    job::JobIdentifier,
    local::LocalChange,
    remote::RemoteChange,
    user::{MonitorWindowPanel, UserRequest},
};

pub trait SyncPolitic: Send {
    fn deal(
        &self,
        remote_changes: Vec<RemoteChange>,
        local_changes: Vec<LocalChange>,
    ) -> Result<bool, SyncPoliticError>;
}

#[derive(Debug, Error)]
pub enum SyncPoliticError {
    #[error("Unable to send changes")]
    UnableToSendChanges,
    #[error("Unable to send user confirmation request")]
    UnableToSendUserConfirmationRequest,
    #[error("Unable to receive changes")]
    UnableToReceiveChanges,
}

/// A sync politic which accept all remote and local change without any human intervention
#[derive(Debug, Clone)]
pub struct AcceptAllSyncPolitic;

impl SyncPolitic for AcceptAllSyncPolitic {
    fn deal(
        &self,
        _remote_changes: Vec<RemoteChange>,
        _local_changes: Vec<LocalChange>,
    ) -> Result<bool, SyncPoliticError> {
        Ok(true)
    }
}

#[derive(Debug, Clone)]
pub struct ConfirmationSyncPolitic {
    sync_channels: SyncChannels,
    user_request_sender: Sender<UserRequest>,
    popup: bool,
}

impl ConfirmationSyncPolitic {
    pub fn new(
        sync_channels: SyncChannels,
        user_request_sender: Sender<UserRequest>,
        popup: bool,
    ) -> Self {
        Self {
            sync_channels,
            user_request_sender,
            popup,
        }
    }
}

impl SyncPolitic for ConfirmationSyncPolitic {
    fn deal(
        &self,
        remote_changes: Vec<RemoteChange>,
        local_changes: Vec<LocalChange>,
    ) -> Result<bool, SyncPoliticError> {
        // TODO: no unwrap ... -> SyncPoliticError::UnableToSendChanges
        *self.sync_channels.changes.lock().unwrap() = Some((remote_changes, local_changes));

        if self.popup
            && self
                .user_request_sender
                .send(UserRequest::OpenMonitorWindow(
                    MonitorWindowPanel::StartupConfirmations,
                ))
                // Error means channel is closed
                .is_err()
        {
            return Err(SyncPoliticError::UnableToSendUserConfirmationRequest);
        }

        match self.sync_channels.confirm_sync_receiver.recv() {
            Ok(decision) => Ok(decision),
            Err(_) => Err(SyncPoliticError::UnableToReceiveChanges),
        }
    }
}

#[derive(Debug, Clone)]
pub struct SyncChannels {
    changes: Arc<Mutex<Option<(Vec<RemoteChange>, Vec<LocalChange>)>>>,
    confirm_sync_sender: Sender<bool>,
    confirm_sync_receiver: Receiver<bool>,
}

impl SyncChannels {
    pub fn new(
        changes: Arc<Mutex<Option<(Vec<RemoteChange>, Vec<LocalChange>)>>>,
        confirm_sync_sender: Sender<bool>,
        confirm_sync_receiver: Receiver<bool>,
    ) -> Self {
        Self {
            changes,
            confirm_sync_sender,
            confirm_sync_receiver,
        }
    }

    pub fn confirm_sync_sender(&self) -> &Sender<bool> {
        &self.confirm_sync_sender
    }

    pub fn confirm_sync_receiver(&self) -> &Receiver<bool> {
        &self.confirm_sync_receiver
    }

    pub fn changes(&self) -> &Mutex<Option<(Vec<RemoteChange>, Vec<LocalChange>)>> {
        self.changes.as_ref()
    }
}

#[derive(Debug, Clone)]
pub struct SyncExchanger {
    channels: HashMap<JobIdentifier, SyncChannels>,
}

impl SyncExchanger {
    pub fn new() -> Self {
        Self {
            channels: HashMap::new(),
        }
    }

    pub fn insert(&mut self, job_identifier: JobIdentifier) -> SyncChannels {
        let changes = Arc::new(Mutex::new(None));
        let (confirm_sync_sender, confirm_sync_receiver) = unbounded();
        let sync_channels = SyncChannels::new(changes, confirm_sync_sender, confirm_sync_receiver);
        self.channels.insert(job_identifier, sync_channels.clone());

        sync_channels
    }

    pub fn channels(&self) -> &HashMap<JobIdentifier, SyncChannels> {
        &self.channels
    }
}

impl Default for SyncExchanger {
    fn default() -> Self {
        Self::new()
    }
}
