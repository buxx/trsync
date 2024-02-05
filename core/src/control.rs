use std::sync::{atomic::AtomicBool, Arc};

use crossbeam_channel::Sender;
use thiserror::Error;

use crate::{
    activity::WrappedActivity,
    error::ErrorChannels,
    sync::{AcceptAllSyncPolitic, ConfirmationSyncPolitic, SyncChannels, SyncPolitic},
    user::UserRequest,
};

#[derive(Clone)]
pub struct RemoteControl {
    stop_signal: Arc<AtomicBool>,
    activity_sender: Option<Sender<WrappedActivity>>,
    sync_channels: Option<SyncChannels>,
    error_channels: Option<ErrorChannels>,
    confirm_startup_sync: bool,
    popup_confirm_startup_sync: bool,
    user_request_sender: Option<Sender<UserRequest>>,
}

pub struct RemoteControlBuilder {
    stop_signal: Arc<AtomicBool>,
    activity_sender: Option<Sender<WrappedActivity>>,
    sync_channels: Option<SyncChannels>,
    error_channels: Option<ErrorChannels>,
    confirm_startup_sync: bool,
    popup_confirm_startup_sync: bool,
    user_request_sender: Option<Sender<UserRequest>>,
}

impl RemoteControlBuilder {
    fn new() -> Self {
        Self {
            stop_signal: Arc::new(AtomicBool::new(false)),
            activity_sender: None,
            sync_channels: None,
            error_channels: None,
            confirm_startup_sync: false,
            popup_confirm_startup_sync: false,
            user_request_sender: None,
        }
    }

    pub fn stop_signal(mut self, value: Arc<AtomicBool>) -> Self {
        self.stop_signal = value;
        self
    }

    pub fn activity_sender(mut self, value: Option<Sender<WrappedActivity>>) -> Self {
        self.activity_sender = value;
        self
    }

    pub fn sync_channels(mut self, value: Option<SyncChannels>) -> Self {
        self.sync_channels = value;
        self
    }

    pub fn error_channels(mut self, value: Option<ErrorChannels>) -> Self {
        self.error_channels = value;
        self
    }

    pub fn confirm_startup_sync(mut self, value: bool) -> Self {
        self.confirm_startup_sync = value;
        self
    }

    pub fn popup_confirm_startup_sync(mut self, value: bool) -> Self {
        self.popup_confirm_startup_sync = value;
        self
    }

    pub fn user_request_sender(mut self, value: Option<Sender<UserRequest>>) -> Self {
        self.user_request_sender = value;
        self
    }

    pub fn build(self) -> RemoteControl {
        RemoteControl::new(
            self.stop_signal,
            self.activity_sender,
            self.sync_channels,
            self.error_channels,
            self.confirm_startup_sync,
            self.popup_confirm_startup_sync,
            self.user_request_sender,
        )
    }
}

impl RemoteControl {
    fn new(
        stop_signal: Arc<AtomicBool>,
        activity_sender: Option<Sender<WrappedActivity>>,
        sync_channels: Option<SyncChannels>,
        error_channels: Option<ErrorChannels>,
        confirm_startup_sync: bool,
        popup_confirm_startup_sync: bool,
        user_request_sender: Option<Sender<UserRequest>>,
    ) -> Self {
        Self {
            stop_signal,
            activity_sender,
            sync_channels,
            error_channels,
            confirm_startup_sync,
            popup_confirm_startup_sync,
            user_request_sender,
        }
    }

    pub fn sync_politic(&self) -> Result<Box<dyn SyncPolitic>, RemoteControlError> {
        match self.confirm_startup_sync {
            true => {
                if let (Some(sync_channels), Some(user_request_sender)) =
                    (&self.sync_channels, &self.user_request_sender)
                {
                    Ok(Box::new(ConfirmationSyncPolitic::new(
                        sync_channels.clone(),
                        user_request_sender.clone(),
                        self.popup_confirm_startup_sync,
                    )))
                } else {
                    Err(RemoteControlError::CantMakeConfirmationSyncPolitic)
                }
            }
            false => Ok(Box::new(AcceptAllSyncPolitic)),
        }
    }

    pub fn stop_signal(&self) -> &Arc<AtomicBool> {
        &self.stop_signal
    }

    pub fn activity_sender(&self) -> Option<&Sender<WrappedActivity>> {
        self.activity_sender.as_ref()
    }

    pub fn error_channels(&self) -> Option<&ErrorChannels> {
        self.error_channels.as_ref()
    }
}

impl Default for RemoteControlBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Error)]
pub enum RemoteControlError {
    #[error("Unable to build ConfirmationSyncPolitic: sync_channels and user_request_sender must be provided")]
    CantMakeConfirmationSyncPolitic,
}
