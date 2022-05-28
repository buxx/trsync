use std::{fs::OpenOptions, sync::mpsc::channel, thread, time::Duration};

use crossbeam_channel::Sender;
use notify::{watcher, DebouncedEvent, RecursiveMode, Watcher};

use crate::{config::Config, error::Error, message::DaemonControlMessage};

pub struct ReloadWatcher {
    config: Config,
    main_channel_sender: Sender<DaemonControlMessage>,
}

impl ReloadWatcher {
    pub fn new(config: Config, main_channel_sender: Sender<DaemonControlMessage>) -> Self {
        Self {
            config,
            main_channel_sender,
        }
    }

    pub fn start(&mut self) -> Result<(), Error> {
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

        let main_channel_sender = self.main_channel_sender.clone();
        let allow_raw_passwords = self.config.allow_raw_passwords;
        thread::spawn(move || {
            // FIXME error
            let mut inotify_watcher = watcher(inotify_sender, Duration::from_secs(1)).unwrap();
            inotify_watcher
                .watch(tracked_file_path, RecursiveMode::NonRecursive)
                .unwrap(); // FIXME

            loop {
                match inotify_receiver.recv() {
                    Ok(DebouncedEvent::Write(_)) => {
                        let config = match Config::from_env(allow_raw_passwords) {
                            Ok(config_) => config_,
                            Err(error) => {
                                // FIXME more elegant message
                                log::error!("{:?}", error);
                                continue;
                            }
                        };
                        match main_channel_sender.send(DaemonControlMessage::Reload(config)) {
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

        Ok(())
    }
}