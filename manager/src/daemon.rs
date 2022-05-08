use std::{
    collections::HashMap,
    path::Path,
    process::{Child, Command},
    sync::mpsc::Receiver,
};

use crate::{client::Client, config::Config, error::Error, message::DaemonMessage, types::*};

pub struct Daemon {
    config: Config,
    processes: HashMap<TrsyncUid, Child>,
}

impl Daemon {
    pub fn new(config: Config) -> Self {
        Self {
            config,
            processes: HashMap::new(),
        }
    }

    pub fn run(&mut self, reload_channel: Receiver<DaemonMessage>) -> Result<(), Error> {
        self.ensure_processes()?;

        loop {
            // Blocking until new message received
            match reload_channel.recv() {
                Ok(DaemonMessage::ReloadFromConfig(new_config)) => {
                    self.config = new_config;
                    self.ensure_processes()?
                }
                Ok(DaemonMessage::Stop) => break,
                Err(error) => return Err(Error::from(error)),
            }
        }

        Ok(())
    }

    pub fn ensure_processes(&mut self) -> Result<(), Error> {
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
        let instance = self
            .config
            .instances
            .iter()
            .find(|instance| instance.address == trsync_uid.instance_address())
            .expect("Start process imply that instance exists");
        let workspace =
            match Client::new(instance.clone()).get_workspace(*trsync_uid.workspace_id()) {
                Ok(workspace) => workspace,
                Err(_) => {
                    // FIXME BS NOW : more details
                    return Err(Error::FailToSpawnTrsyncProcess);
                }
            };
        let folder_path = Path::new(&self.config.local_folder)
            .join(&instance.address)
            .join(workspace.label);

        let child = if cfg!(target_os = "windows") {
            let sub_command = [
                &self.config.trsync_bin_path,
                &folder_path.to_str().unwrap().to_string(),
                &instance.address,
                &format!("{}", workspace.workspace_id),
                &instance.username,
                "--env-var-pass",
                "PASSWORD",
            ]
            .join(" ");
            match Command::new("cmd")
                .arg("/c")
                .arg(sub_command)
                .env("PASSWORD", &instance.password)
                // FIXME : output to file ?
                .spawn()
            {
                // FIXME details (specific error to spawn, stop ...)
                Err(_) => return Err(Error::FailToSpawnTrsyncProcess),
                Ok(child) => child,
            }
        } else {
            // FIXME BS NOW : bin path ?
            match Command::new(&self.config.trsync_bin_path)
                .arg(folder_path)
                .arg(&instance.address)
                // FIXME BS NOW : add unsecure option
                .arg(format!("{}", workspace.workspace_id))
                .arg(&instance.username)
                .arg("--env-var-pass")
                .arg("PASSWORD")
                .env("PASSWORD", &instance.password)
                // FIXME : output to file ?
                .spawn()
            {
                // FIXME details (specific error to spawn, stop ...)
                Err(_) => return Err(Error::FailToSpawnTrsyncProcess),
                Ok(child) => child,
            }
        };

        self.processes.insert(trsync_uid, child);
        Ok(())
    }

    fn stop_process(&mut self, trsync_uid: TrsyncUid) -> Result<(), Error> {
        let child = self
            .processes
            .get_mut(&trsync_uid)
            .expect("Stop process imply that process exists");

        log::info!("Stop process with pid '{}'", child.id());
        // FIXME BS NOW : manage errors
        child.kill().unwrap();
        child.wait().unwrap();
        log::info!("Stopped process with pid '{}'", child.id());

        self.processes.remove(&trsync_uid);
        Ok(())
    }
}
