use crossbeam_channel::Receiver;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};
use trsync::operation::{Job, JobIdentifier};

use crate::config::Config;

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
    config: Config,
    receiver: Receiver<Job>,
    state: Arc<Mutex<ActivityState>>,
}

impl ActivityMonitor {
    pub fn new(config: Config, receiver: Receiver<Job>, state: Arc<Mutex<ActivityState>>) -> Self {
        Self {
            config,
            receiver,
            state,
        }
    }

    pub fn run(&self) {
        loop {
            match self.receiver.recv() {
                Ok(Job::Begin(job_identifier)) => {
                    self.state.lock().unwrap().new_job(job_identifier);
                }
                Ok(Job::End(job_identifier)) => {
                    self.state.lock().unwrap().finished_job(job_identifier);
                }
                Err(error) => {
                    log::error!("Error wen reading activity monitor channel '{}'", error);
                    break;
                }
            }
        }
    }
}
