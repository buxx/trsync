use crossbeam_channel::Receiver;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::{collections::HashMap, path::Path};
use trsync;

use crate::{
    client::Client, config::Config, error::Error, message::DaemonControlMessage, types::*,
};

pub struct Daemon {
    config: Config,
    processes: HashMap<TrsyncUid, Arc<AtomicBool>>,
    main_channel_receiver: Receiver<DaemonControlMessage>,
}

impl Daemon {
    pub fn new(config: Config, main_channel_receiver: Receiver<DaemonControlMessage>) -> Self {
        Self {
            config,
            processes: HashMap::new(),
            main_channel_receiver,
        }
    }

    pub fn run(&mut self) -> Result<(), Error> {
        self.ensure_processes()?;

        loop {
            // Block until new message received
            match self.main_channel_receiver.recv() {
                Ok(DaemonControlMessage::Reload(new_config)) => {
                    self.config = new_config;
                    self.ensure_processes()?
                }
                Ok(DaemonControlMessage::Stop) => break,
                Err(error) => return Err(Error::from(error)),
            }
        }

        Ok(())
    }

    pub fn ensure_processes(&mut self) -> Result<(), Error> {
        // Managing process is valid only if local folder has been configured
        if self.config.local_folder.is_none() {
            log::info!("Local folder is not configured, skipping process management");
            return Ok(());
        }

        let processes_to_start = self.get_processes_to_start();
        let processes_to_stop = self.get_processes_to_stop();

        log::info!("'{}' process to start", processes_to_start.len());
        log::info!("'{}' process to stop", processes_to_stop.len());

        for process_to_stop in processes_to_stop {
            self.stop_process(process_to_stop)?;
        }

        for process_to_start in processes_to_start {
            match self.start_process(process_to_start) {
                Err(error) => {
                    log::error!("Failed to spawn new process : '{:?}'", error)
                }
                _ => {}
            };
        }

        Ok(())
    }

    fn get_processes_to_start(&self) -> Vec<TrsyncUid> {
        let mut processes_to_start = vec![];

        for instance in self.config.instances.iter() {
            let client = Client::new(instance.clone());
            for workspace_id in &instance.workspaces_ids {
                match client.get_workspace(*workspace_id) {
                    Ok(workspace) => {
                        let process_uid =
                            TrsyncUid::new(instance.address.clone(), workspace.workspace_id);
                        if !self.processes.contains_key(&process_uid) {
                            processes_to_start.push(process_uid);
                        }
                    }
                    Err(error) => {
                        log::error!("{:?}", error);
                    }
                }
            }
        }

        processes_to_start
    }

    fn get_processes_to_stop(&self) -> Vec<TrsyncUid> {
        let mut processes_to_stop: Vec<TrsyncUid> = vec![];
        let mut expected_processes: Vec<TrsyncUid> = vec![];

        for instance in self.config.instances.iter() {
            for workspace_id in &instance.workspaces_ids {
                let process_uid = TrsyncUid::new(instance.address.clone(), *workspace_id);
                expected_processes.push(process_uid);
            }
        }

        for process_uid in self.processes.keys() {
            if !expected_processes.contains(process_uid) {
                processes_to_stop.push(process_uid.clone())
            }
        }

        processes_to_stop
    }

    fn start_process(&mut self, trsync_uid: TrsyncUid) -> Result<(), Error> {
        let local_folder = self
            .config
            .local_folder
            .clone()
            .expect("Local folder config must be configured");
        let instance = self
            .config
            .instances
            .iter()
            .find(|instance| instance.address == trsync_uid.instance_address())
            .expect("Start process imply its instance exists");
        let workspace =
            match Client::new(instance.clone()).get_workspace(*trsync_uid.workspace_id()) {
                Ok(workspace) => workspace,
                Err(error) => {
                    return Err(Error::FailToSpawnTrsyncProcess(Some(format!(
                        "Error during workspace fetching : '{error}'"
                    ))));
                }
            };

        let folder_path = match std::fs::canonicalize(
            Path::new(&local_folder)
                .join(&instance.address)
                .join(workspace.label),
        ) {
            Ok(folder_path_) => folder_path_,
            Err(error) => {
                return Err(Error::FailToSpawnTrsyncProcess(Some(format!(
                    "Error during folder path canonicalization : '{error}'"
                ))))
            }
        };

        let trsync_context = match trsync::context::Context::new(
            !instance.unsecure,
            instance.address.clone(),
            instance.username.clone(),
            instance.password.clone(),
            // TODO
            folder_path.to_str().unwrap().to_string(),
            workspace.workspace_id as i32,
            false,
        ) {
            Ok(context_) => context_,
            Err(error) => {
                return Err(Error::UnexpectedError(format!(
                    "Unable to build trsync context : {:?}",
                    error,
                )))
            }
        };

        let stop_signal = Arc::new(AtomicBool::new(false));
        let thread_stop_signal = stop_signal.clone();
        thread::spawn(move || trsync::run::run(trsync_context, thread_stop_signal));
        self.processes.insert(trsync_uid, stop_signal);
        Ok(())
    }

    fn stop_process(&mut self, trsync_uid: TrsyncUid) -> Result<(), Error> {
        let stop_signal = self
            .processes
            .get_mut(&trsync_uid)
            .expect("Stop process imply that process exists");

        log::info!("Signal '{}' to stop", trsync_uid);
        stop_signal.swap(true, Ordering::Relaxed);

        self.processes.remove(&trsync_uid);
        Ok(())
    }
}
