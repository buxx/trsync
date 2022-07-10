use std::{
    fs::OpenOptions,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::{channel, RecvTimeoutError},
        Arc,
    },
    thread,
    time::Duration,
};

use crossbeam_channel::Sender;
use notify::{watcher, DebouncedEvent, RecursiveMode, Watcher};

use crate::{config::Config, error::Error, message::DaemonMessage};

pub struct ReloadWatcher {
    config: Config,
    main_sender: Sender<DaemonMessage>,
    stop_signal: Arc<AtomicBool>,
}

impl ReloadWatcher {
    pub fn new(
        config: Config,
        main_channel_sender: Sender<DaemonMessage>,
        stop_signal: Arc<AtomicBool>,
    ) -> Self {
        Self {
            config,
            main_sender: main_channel_sender,
            stop_signal,
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

        let main_sender = self.main_sender.clone();
        let allow_raw_passwords = self.config.allow_raw_passwords;
        let thread_stop_signal = self.stop_signal.clone();
        thread::spawn(move || {
            let mut inotify_watcher = match watcher(inotify_sender, Duration::from_secs(1)) {
                Ok(watcher) => watcher,
                Err(error) => {
                    // TODO : display error to the user
                    log::error!("Unable to create inotify watcher: '{}'", error);
                    return;
                }
            };
            if let Err(error) =
                inotify_watcher.watch(tracked_file_path, RecursiveMode::NonRecursive)
            {
                // TODO : display error to the user
                log::error!("Unable to start reload watcher : '{}'", error);
                return;
            }

            loop {
                match inotify_receiver.recv_timeout(Duration::from_millis(250)) {
                    Ok(DebouncedEvent::Write(_)) => {
                        let config = match Config::from_env(allow_raw_passwords) {
                            Ok(config_) => config_,
                            Err(error) => {
                                // TODO : Notify user of error
                                log::error!("Error during config reading : '{:?}'", error);
                                continue;
                            }
                        };
                        match main_sender.send(DaemonMessage::Reload(config)) {
                            Err(error) => {
                                log::error!("Unable to send reload message : '{:?}'", error);
                                // TODO : Notify user of error
                                continue;
                            }
                            _ => {}
                        };
                    }
                    Ok(_) => {}
                    Err(RecvTimeoutError::Timeout) => {
                        if thread_stop_signal.load(Ordering::Relaxed) {
                            log::error!("Stop reload watcher (on stop signal)");
                            break;
                        }
                        continue;
                    }
                    Err(RecvTimeoutError::Disconnected) => {
                        log::error!("Stop reload watcher (on channel disconnected)");
                        break;
                    }
                }
            }
        });

        Ok(())
    }
}
