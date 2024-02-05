use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use crossbeam_channel::{unbounded, Receiver, Sender};

use crate::job::JobIdentifier;

#[derive(Debug, Clone)]
pub enum Decision {
    RestartSpaceSync,
}

#[derive(Debug, Clone)]
pub struct ErrorChannels {
    error: Arc<Mutex<Option<String>>>,
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

    pub fn error(&self) -> &Mutex<Option<String>> {
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
