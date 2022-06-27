use crate::context::Context;
use crate::database::{Database, DatabaseOperation};
use notify::DebouncedEvent;
use notify::{watcher, RecursiveMode, Watcher};
use rusqlite::Connection;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::sync::mpsc::{channel, RecvTimeoutError};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::{Duration, UNIX_EPOCH};
use std::{fs, thread};
use walkdir::{DirEntry, WalkDir};

use crate::error::Error;
use crate::operation::OperationalMessage;
use crate::util;

pub struct LocalWatcher {
    stop_signal: Arc<AtomicBool>,
    restart_signal: Arc<AtomicBool>,
    operational_sender: Sender<OperationalMessage>,
    workspace_folder_path: PathBuf,
}

impl LocalWatcher {
    pub fn new(
        stop_signal: Arc<AtomicBool>,
        restart_signal: Arc<AtomicBool>,
        operational_sender: Sender<OperationalMessage>,
        workspace_folder_path: String,
    ) -> Result<Self, Error> {
        Ok(Self {
            stop_signal,
            restart_signal,
            operational_sender,
            workspace_folder_path: fs::canonicalize(&workspace_folder_path)?,
        })
    }

    pub fn listen(&mut self, path: String) -> Result<(), Error> {
        let (inotify_sender, inotify_receiver) = channel();
        let mut inotify_watcher = watcher(inotify_sender, Duration::from_secs(1))?;
        inotify_watcher.watch(path, RecursiveMode::Recursive)?;

        loop {
            match inotify_receiver.recv_timeout(Duration::from_millis(250)) {
                Ok(event) => match self.digest_event(&event) {
                    Err(error) => {
                        log::error!("Error when digest event {:?} : {:?}", &event, error)
                    }
                    _ => {}
                },
                Err(RecvTimeoutError::Timeout) => {
                    if self.stop_signal.load(Ordering::Relaxed) {
                        log::info!("Finished local listening (on stop signal)");
                        break;
                    }
                    if self.restart_signal.load(Ordering::Relaxed) {
                        log::info!("Finished local listening (on restart signal)");
                        break;
                    }
                }
                Err(RecvTimeoutError::Disconnected) => {
                    log::error!("Finished local listening (on channel closed)");
                    break;
                }
            }
        }

        Ok(())
    }

    pub fn digest_event(&self, event: &DebouncedEvent) -> Result<(), Error> {
        log::debug!("Local event: {:?}", event);

        let messages: Vec<OperationalMessage> = match event {
            DebouncedEvent::Create(absolute_path) => {
                vec![OperationalMessage::NewLocalFile(util::path_to_string(
                    absolute_path.strip_prefix(&self.workspace_folder_path)?,
                )?)]
            }
            DebouncedEvent::Write(absolute_path) => {
                vec![OperationalMessage::ModifiedLocalFile(util::path_to_string(
                    absolute_path.strip_prefix(&self.workspace_folder_path)?,
                )?)]
            }
            DebouncedEvent::Remove(absolute_path) => {
                vec![OperationalMessage::DeletedLocalFile(util::path_to_string(
                    absolute_path.strip_prefix(&self.workspace_folder_path)?,
                )?)]
            }
            DebouncedEvent::Rename(absolute_source_path, absolute_dest_path) => {
                vec![OperationalMessage::RenamedLocalFile(
                    util::path_to_string(
                        absolute_source_path.strip_prefix(&self.workspace_folder_path)?,
                    )?,
                    util::path_to_string(
                        absolute_dest_path.strip_prefix(&self.workspace_folder_path)?,
                    )?,
                )]
            }
            // Ignore these
            DebouncedEvent::NoticeWrite(_)
            | DebouncedEvent::NoticeRemove(_)
            | DebouncedEvent::Chmod(_)
            | DebouncedEvent::Rescan => {
                vec![]
            }
            // Consider Error as to log it
            DebouncedEvent::Error(err, path) => {
                log::error!("Error {} on {:?}", err, path);
                vec![]
            }
        };

        for message in messages {
            match self.operational_sender.send(message) {
                Ok(_) => (),
                Err(err) => {
                    log::error!(
                        "Error when send operational message from local watcher : '{}'",
                        err
                    )
                }
            };
        }

        Ok(())
    }
}

// Represent known local files. When trsync start, it use this index to compare
// with real local files state and produce change messages.
pub struct LocalSync {
    connection: Connection,
    path: PathBuf,
    operational_sender: Sender<OperationalMessage>,
}

impl LocalSync {
    pub fn new(
        connection: Connection,
        path: String,
        operational_sender: Sender<OperationalMessage>,
    ) -> Result<Self, Error> {
        Ok(Self {
            connection,
            path: fs::canonicalize(&path)?,
            operational_sender,
        })
    }

    pub fn sync(&self) -> Result<(), Error> {
        // Look at disk files and compare to db
        self.sync_from_disk();
        // TODO : look ate db to search deleted files
        self.sync_from_db()?;

        Ok(())
    }

    fn sync_from_disk(&self) {
        WalkDir::new(&self.path)
            .into_iter()
            .filter_entry(|e| !self.ignore_entry(e))
            .for_each(|dir_entry| match &dir_entry {
                Ok(dir_entry_) => match self.sync_disk_file(&dir_entry_) {
                    Ok(_) => {}
                    Err(error) => {
                        log::error!("Fail to sync disk file {:?} : {:?}", dir_entry_, error);
                    }
                },
                Err(error) => {
                    log::error!("Fail to walk on dir {:?} : '{}'", &dir_entry, error)
                }
            })
    }

    fn ignore_entry(&self, entry: &DirEntry) -> bool {
        // TODO : patterns from config object
        if let Some(file_name) = entry.path().file_name() {
            if let Some(file_name_) = file_name.to_str() {
                let file_name_as_str = format!("{}", file_name_);
                if file_name_as_str.starts_with(".")
                    || file_name_as_str.starts_with("~")
                    || file_name_as_str.starts_with("#")
                {
                    return true;
                }
            }
        }

        false
    }

    fn sync_disk_file(&self, entry: &DirEntry) -> Result<(), Error> {
        let relative_path = entry.path().strip_prefix(&self.path)?;
        // TODO : prevent sync root with more clean way
        if relative_path == Path::new("") {
            return Ok(());
        }

        let metadata = fs::metadata(self.path.join(relative_path))?;
        let disk_last_modified_timestamp =
            metadata.modified()?.duration_since(UNIX_EPOCH)?.as_millis() as u64;

        match DatabaseOperation::new(&self.connection).get_last_modified_timestamp(
            relative_path
                .to_str()
                .ok_or(Error::PathManipulationError(format!(
                    "Error when manipulate path {:?}",
                    relative_path
                )))?,
        ) {
            Ok(last_modified_timestamp) => {
                // Known file (check if have been modified)
                if disk_last_modified_timestamp != last_modified_timestamp {
                    match self
                        .operational_sender
                        .send(OperationalMessage::ModifiedLocalFile(util::path_to_string(
                            relative_path,
                        )?)) {
                        Err(error) => {
                            log::error!("Fail to send operational message : {:?}", error)
                        }
                        _ => {}
                    }
                }
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => {
                // Unknown file
                match self
                    .operational_sender
                    .send(OperationalMessage::NewLocalFile(util::path_to_string(
                        relative_path,
                    )?)) {
                    Err(error) => {
                        log::error!("Fail to send operational message : {:?}", error)
                    }
                    _ => {}
                }
            }
            Err(error) => {
                return Err(Error::UnexpectedError(format!(
                    "Error when reading database for synchronize disk file : {:?}",
                    error
                )))
            }
        };

        Ok(())
    }

    fn sync_from_db(&self) -> Result<(), Error> {
        let relative_paths = DatabaseOperation::new(&self.connection).get_relative_paths()?;
        for relative_path in &relative_paths {
            if !self.path.join(&relative_path).exists() {
                match self
                    .operational_sender
                    .send(OperationalMessage::DeletedLocalFile(relative_path.clone()))
                {
                    Err(error) => {
                        log::error!("Fail to send operational message : '{}'", error)
                    }
                    _ => {}
                }
            }
        }

        Ok(())
    }
}

pub fn start_local_sync(
    context: &Context,
    operational_sender: &Sender<OperationalMessage>,
) -> JoinHandle<Result<(), Error>> {
    let local_sync_context = context.clone();
    let local_sync_operational_sender = operational_sender.clone();

    thread::spawn(move || {
        Database::new(local_sync_context.database_path.clone()).with_new_connection(
            |connection| {
                LocalSync::new(
                    connection,
                    local_sync_context.folder_path.clone(),
                    local_sync_operational_sender,
                )?
                .sync()?;
                Ok(())
            },
        )?;

        Ok(())
    })
}

pub fn start_local_watch(
    context: &Context,
    operational_sender: &Sender<OperationalMessage>,
    stop_signal: &Arc<AtomicBool>,
    restart_signal: &Arc<AtomicBool>,
) -> Result<JoinHandle<Result<(), Error>>, Error> {
    let local_watcher_context = context.clone();
    let local_watcher_operational_sender = operational_sender.clone();
    let local_watcher_stop_signal = stop_signal.clone();
    let local_watcher_restart_signal = restart_signal.clone();

    let mut local_watcher = LocalWatcher::new(
        local_watcher_stop_signal,
        local_watcher_restart_signal,
        local_watcher_operational_sender,
        local_watcher_context.folder_path.clone(),
    )?;

    Ok(thread::spawn(move || {
        if !local_watcher_context.exit_after_sync {
            local_watcher.listen(local_watcher_context.folder_path.clone())
        } else {
            Ok(())
        }
    }))
}
