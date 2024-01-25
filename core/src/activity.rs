use crossbeam_channel::{Receiver, RecvTimeoutError};
use std::{
    collections::HashMap,
    fmt::Display,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};

use crate::{change::Change, job::JobIdentifier, sync::SyncChannels};

#[derive(Debug)]
pub enum State {
    Idle,
    Working,
}

#[derive(Debug, Clone)]
pub enum Activity {
    Idle,
    Job(String),
    StartupSync(Option<Change>),
    WaitingStartupSyncConfirmation,
    WaitingConnection,
    Error,
}

impl Activity {
    fn is_job(&self) -> bool {
        matches!(self, Activity::Job(_))
    }

    fn is_startup_sync(&self) -> bool {
        matches!(self, Activity::StartupSync(_))
    }
}

impl Display for Activity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Activity::Idle => f.write_str("En veille"),
            Activity::Job(message) => f.write_str(message),
            Activity::StartupSync(change) => match change {
                Some(change) => f.write_str(&format!("Synchronisation ({})", change)),
                None => f.write_str("Synchronisation"),
            },
            Activity::WaitingStartupSyncConfirmation => f.write_str("Attend confirmation"),
            Activity::WaitingConnection => f.write_str("Attend connection"),
            Activity::Error => f.write_str("Erreur"),
        }
    }
}

pub struct ActivityState {
    activities: HashMap<JobIdentifier, Activity>,
    pending_startup_sync: Vec<(JobIdentifier, SyncChannels)>,
}

impl ActivityState {
    pub fn new() -> Self {
        Self {
            activities: HashMap::new(),
            pending_startup_sync: vec![],
        }
    }

    pub fn set_activity(&mut self, job_identifier: JobIdentifier, activity: Activity) {
        self.activities.insert(job_identifier, activity);
    }

    pub fn is_working(&self) -> bool {
        for activity in self.activities.values() {
            if activity.is_job() || activity.is_startup_sync() {
                return true;
            }
        }

        false
    }

    pub fn activities(&self) -> &HashMap<JobIdentifier, Activity> {
        &self.activities
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

pub struct WrappedActivity {
    job_identifier: JobIdentifier,
    activity: Activity,
}

impl WrappedActivity {
    pub fn new(job: JobIdentifier, activity: Activity) -> Self {
        Self {
            job_identifier: job,
            activity,
        }
    }

    pub fn job_identifier(&self) -> &JobIdentifier {
        &self.job_identifier
    }

    pub fn activity(&self) -> &Activity {
        &self.activity
    }
}

pub struct ActivityMonitor {
    receiver: Receiver<WrappedActivity>,
    state: Arc<Mutex<ActivityState>>,
    stop_signal: Arc<AtomicBool>,
}

impl ActivityMonitor {
    pub fn new(
        receiver: Receiver<WrappedActivity>,
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
                // TODO : no unwrap
                Ok(wrapped_activity) => self.state.lock().unwrap().set_activity(
                    wrapped_activity.job_identifier().clone(),
                    wrapped_activity.activity().clone(),
                ),
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
