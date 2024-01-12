use crossbeam_channel::{Receiver, RecvTimeoutError};
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};

use crate::{
    job::{Job, JobIdentifier},
    sync::SyncChannels,
};

#[derive(Debug)]
pub enum State {
    Idle,
    Working,
}

pub struct ActivityState {
    // (instance_name, workspace_id), counter
    jobs: HashMap<JobIdentifier, i32>,
    pending_startup_sync: Vec<(JobIdentifier, SyncChannels)>,
    // errors: HashMap<JobIdentifier, JobErrors>,
}

impl ActivityState {
    pub fn new() -> Self {
        Self {
            jobs: HashMap::new(),
            pending_startup_sync: vec![],
        }
    }

    pub fn activity(&self) -> State {
        for (_, count) in &self.jobs {
            if count > &0 {
                return State::Working;
            }
        }

        State::Idle
    }

    pub fn new_job(&mut self, job_identifier: JobIdentifier) {
        *self.jobs.entry(job_identifier).or_insert(0) += 1;
    }

    pub fn finished_job(&mut self, job_identifier: JobIdentifier) {
        *self.jobs.get_mut(&job_identifier).unwrap() -= 1;
    }

    pub fn jobs(&self) -> &HashMap<JobIdentifier, i32> {
        &self.jobs
    }

    pub fn new_pending_startup_sync(&mut self, startup_sync: (JobIdentifier, SyncChannels)) {
        self.pending_startup_sync.push(startup_sync)
    }
}

impl Default for ActivityState {
    fn default() -> Self {
        Self::new()
    }
}

pub struct ActivityMonitor {
    receiver: Receiver<Job>,
    state: Arc<Mutex<ActivityState>>,
    stop_signal: Arc<AtomicBool>,
}

impl ActivityMonitor {
    pub fn new(
        receiver: Receiver<Job>,
        state: Arc<Mutex<ActivityState>>,
        stop_signal: Arc<AtomicBool>,
    ) -> Self {
        Self {
            receiver,
            state,
            stop_signal,
        }
    }

    pub fn run(&self) {
        loop {
            match self.receiver.recv_timeout(Duration::from_millis(250)) {
                Ok(message) => match message {
                    Job::Begin(job_identifier) => {
                        log::debug!("Receive Job::Begin ({:?})", job_identifier);
                        self.state.lock().unwrap().new_job(job_identifier);
                    }
                    Job::End(job_identifier) => {
                        log::debug!("Receive Job::End ({:?})", job_identifier);
                        self.state.lock().unwrap().finished_job(job_identifier);
                    }
                },
                Err(RecvTimeoutError::Timeout) => {
                    if self.stop_signal.load(Ordering::Relaxed) {
                        log::info!("Finished ActivityMonitor (on stop signal)");
                        break;
                    }
                }
                Err(RecvTimeoutError::Disconnected) => {
                    log::error!("Finished ActivityMonitor (on channel closed)");
                    break;
                }
            }
        }
    }

    pub fn start(self) {
        std::thread::spawn(move || {
            self.run();
        });
    }
}
