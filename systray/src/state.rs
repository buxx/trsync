use crossbeam_channel::{Receiver, RecvTimeoutError};
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};
use trsync::operation::{Job, JobIdentifier};

pub enum Activity {
    Idle,
    Working,
}

pub struct ActivityState {
    // (instance_name, workspace_id), counter
    counter: HashMap<JobIdentifier, i32>,
}

impl ActivityState {
    pub fn new() -> Self {
        Self {
            counter: HashMap::new(),
        }
    }

    pub fn activity(&self) -> Activity {
        for (_, count) in &self.counter {
            if count > &0 {
                return Activity::Working;
            }
        }

        Activity::Idle
    }

    pub fn new_job(&mut self, job_identifier: JobIdentifier) {
        *self.counter.entry(job_identifier).or_insert(0) += 1;
    }

    pub fn finished_job(&mut self, job_identifier: JobIdentifier) {
        *self.counter.get_mut(&job_identifier).unwrap() -= 1;
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
                        self.state.lock().unwrap().new_job(job_identifier);
                    }
                    Job::End(job_identifier) => {
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
}
