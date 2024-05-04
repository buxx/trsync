use std::{
    collections::HashMap,
    io,
    path::PathBuf,
    sync::{Arc, Mutex},
};

use crossbeam_channel::{unbounded, Receiver, Sender};
use thiserror::Error;

use crate::{
    client::TracimClientError, control::RemoteControlError, instance::ContentId,
    job::JobIdentifier, sync::SyncPoliticError,
};

#[derive(Debug, Clone)]
pub enum Decision {
    RestartSpaceSync,
    IgnoreAndRestartSpaceSync(ContentId),
}

#[derive(Debug, Error)]
pub enum RunnerError {
    #[error("Operator error: {0}")]
    OperatorError(#[from] OperatorError),
    #[error("Remote control error: {0:#}")]
    RemoteControlError(#[from] RemoteControlError),
    #[error("Sync politic error: {0:#}")]
    SyncPoliticError(#[from] SyncPoliticError),
    #[error("Unexpected error: {0:#}")]
    Unexpected(#[from] anyhow::Error),
}

#[derive(Debug, Error)]
pub enum OperatorError {
    #[error("Executor error: {0}")]
    ExecutorError(#[from] ExecutorError),
    #[error("State error: {0}")]
    StateError(#[from] StateError),
    #[error("Activity error: {0}")]
    ActivityError(String),
    #[error("Missing parent error: {0}")]
    MissingParentError(String),
}

#[derive(Error, Debug)]
pub enum ExecutorError {
    #[error("Unexpected error: {0:#}")]
    Unexpected(#[from] anyhow::Error),
    #[error("Unexpected error: {0}")]
    Unexpected2(String),
    #[error("Tracim error: {0}")]
    Tracim(#[from] TracimClientError),
    #[error("State manipulation error: {0}")]
    State(#[from] StateError),
    #[error("Missing parent {1} for content {0}")]
    MissingParent(ContentId, ContentId),
    #[error("Programmatic error : {0}")]
    Programmatic(String),
    #[error(
        "After receive an Tracim ContentAlreadyExist error, unable to found the content ({0})"
    )]
    NotFoundAfterContentAlreadyExist(String),
    #[error("Maximum retry reached for : {0} (because time out)")]
    MaximumRetryCount(String),
    #[error("Related file io error : {0}")]
    RelatedLocalFileIoError(PathBuf, io::Error),
}

#[derive(Error, Debug)]
pub enum StateError {
    #[error("Unexpected error: {0:#}")]
    UnexpectedError(#[from] anyhow::Error),
    #[error("Unknown error: {0:#}")]
    UnknownError(String),
    #[error("Unknown content: {0}")]
    UnknownContent(ContentId),
    #[error("Path already exist: {0}")]
    PathAlreadyExist(PathBuf, ContentId),
}

#[derive(Debug, Clone)]
pub struct ErrorChannels {
    error: Arc<Mutex<Option<RunnerError>>>,
    seen: Arc<Mutex<bool>>,
    decision_sender: Sender<Decision>,
    decision_receiver: Receiver<Decision>,
}

impl ErrorChannels {
    pub fn new(decision_sender: Sender<Decision>, decision_receiver: Receiver<Decision>) -> Self {
        Self {
            error: Arc::new(Mutex::new(None)),
            seen: Arc::new(Mutex::new(false)),
            decision_sender,
            decision_receiver,
        }
    }

    pub fn decision_sender(&self) -> &Sender<Decision> {
        &self.decision_sender
    }

    pub fn decision_receiver(&self) -> &Receiver<Decision> {
        &self.decision_receiver
    }

    pub fn error(&self) -> &Mutex<Option<RunnerError>> {
        self.error.as_ref()
    }

    pub fn seen(&self) -> bool {
        // TODO : no unwrap
        *self.seen.lock().unwrap()
    }

    pub fn set_seen(&self) {
        *self.seen.lock().unwrap() = true;
    }
}

#[derive(Debug, Clone)]
pub struct ErrorExchanger {
    channels: HashMap<JobIdentifier, ErrorChannels>,
}

impl ErrorExchanger {
    pub fn new() -> Self {
        Self {
            channels: HashMap::new(),
        }
    }

    pub fn insert(&mut self, job_identifier: JobIdentifier) -> ErrorChannels {
        let (decision_sender, decision_receiver) = unbounded();
        let error_channels = ErrorChannels::new(decision_sender, decision_receiver);
        self.channels.insert(job_identifier, error_channels.clone());

        error_channels
    }

    pub fn channels(&self) -> &HashMap<JobIdentifier, ErrorChannels> {
        &self.channels
    }
}

impl Default for ErrorExchanger {
    fn default() -> Self {
        Self::new()
    }
}
