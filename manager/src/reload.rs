use std::{
    fs::OpenOptions,
    sync::mpsc::{channel, Receiver},
    thread,
    time::Duration,
};

use notify::{watcher, DebouncedEvent, RecursiveMode, Watcher};

use crate::{config::Config, error::Error, message::DaemonMessage};

pub struct ReloadWatcher {
    _config: Config,
}

impl ReloadWatcher {
    pub fn new(config: Config) -> Self {
        Self { _config: config }
    }

    pub fn watch(&mut self) -> Result<Receiver<DaemonMessage>, Error> {
        let (sender, receiver) = channel();

        let user_home_folder_path = match dirs::home_dir() {
            Some(folder) => folder,
            None => return Err(Error::UnableToFindHomeUser),
        };
        let tracked_file_path = if cfg!(target_os = "windows") {
            user_home_folder_path
                .join("AppData")
                .join("Local")
                .join("trsync.conf.track")
        } else {
            user_home_folder_path.join(".trsync.conf.track")
        };

        log::info!(
            "Track config file changes with {}",
            tracked_file_path.display()
        );
        {
            // Ensure tracked file exist
            OpenOptions::new()
                .write(true)
                .create(true)
                .open(&tracked_file_path)?;
        }
        let (inotify_sender, inotify_receiver) = channel();

        thread::spawn(move || {
            // FIXME error
            let mut inotify_watcher = watcher(inotify_sender, Duration::from_secs(1)).unwrap();
            inotify_watcher
                .watch(tracked_file_path, RecursiveMode::NonRecursive)
                .unwrap(); // FIXME

            loop {
                match inotify_receiver.recv() {
                    Ok(DebouncedEvent::Write(_)) => {
                        let config = match Config::from_env() {
                            Ok(config_) => config_,
                            Err(error) => {
                                // FIXME more elegant message
                                log::error!("{:?}", error);
                                continue;
                            }
                        };
                        match sender.send(DaemonMessage::ReloadFromConfig(config)) {
                            Err(error) => {
                                log::error!("Unable to send reload message : {:?}", error);
                                // FIXME : Should interupt or restart daemon ?
                                break;
                            }
                            _ => {}
                        };
                    }
                    Ok(_) => {}
                    Err(error) => {
                        log::error!("Unable to send reload message : {:?}", error);
                        // FIXME : Should interupt or restart daemon ?
                        break;
                    }
                }
            }

            log::info!("End inotify thread");
        });

        Ok(receiver)
    }
}
