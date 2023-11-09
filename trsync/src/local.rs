use crate::context::Context;
use crate::database::{Database, DatabaseOperation};
use crate::event::local::LocalEvent;
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
use crate::util;

pub struct LocalWatcher {
    context: Context,
    stop_signal: Arc<AtomicBool>,
    restart_signal: Arc<AtomicBool>,
    operational_sender: Sender<LocalEvent>,
}

impl LocalWatcher {
    pub fn new(
        context: Context,
        stop_signal: Arc<AtomicBool>,
        restart_signal: Arc<AtomicBool>,
        operational_sender: Sender<LocalEvent>,
    ) -> Result<Self, Error> {
        Ok(Self {
            context,
            stop_signal,
            restart_signal,
            operational_sender,
        })
    }

    pub fn listen(&mut self) -> Result<(), Error> {
        log::debug!(
            "[{}::{}] Start listening for local changes",
            self.context.instance_name,
            self.context.workspace_id,
        );
        let workspace_folder_path = fs::canonicalize(&self.context.folder_path)?;
        let (inotify_sender, inotify_receiver) = channel();
        let mut inotify_watcher = watcher(inotify_sender, Duration::from_secs(1))?;
        let inotify_workspace_folder_path = workspace_folder_path.clone();
        inotify_watcher.watch(inotify_workspace_folder_path, RecursiveMode::Recursive)?;

        loop {
            match inotify_receiver.recv_timeout(Duration::from_millis(250)) {
                Ok(event) => match self.digest_event(&event, &workspace_folder_path) {
                    Err(error) => {
                        log::error!(
                            "[{}::{}] Error when digest event {:?} : {:?}",
                            self.context.instance_name,
                            self.context.workspace_id,
                            error,
                            &event,
                        )
                    }
                    _ => {}
                },
                Err(RecvTimeoutError::Timeout) => {
                    if self.stop_signal.load(Ordering::Relaxed) {
                        log::info!(
                            "[{}::{}] Finished local listening (on stop signal)",
                            self.context.instance_name,
                            self.context.workspace_id,
                        );
                        break;
                    }
                    if self.restart_signal.load(Ordering::Relaxed) {
                        log::info!(
                            "[{}::{}] Finished local listening (on restart signal)",
                            self.context.instance_name,
                            self.context.workspace_id,
                        );
                        break;
                    }
                }
                Err(RecvTimeoutError::Disconnected) => {
                    log::error!(
                        "[{}::{}] Finished local listening (on channel closed)",
                        self.context.instance_name,
                        self.context.workspace_id,
                    );
                    break;
                }
            }
        }

        Ok(())
    }

    pub fn digest_event(
        &self,
        event: &DebouncedEvent,
        workspace_folder_path: &PathBuf,
    ) -> Result<(), Error> {
        log::debug!(
            "[{}::{}] Local event received: {:?}",
            self.context.instance_name,
            self.context.workspace_id,
            &event,
        );

        let messages: Vec<LocalEvent> = match event {
            DebouncedEvent::Create(absolute_path) => {
                vec![LocalEvent::NewLocalFile(util::path_to_string(
                    absolute_path.strip_prefix(&workspace_folder_path)?,
                )?)]
            }
            DebouncedEvent::Write(absolute_path) => {
                vec![LocalEvent::ModifiedLocalFile(util::path_to_string(
                    absolute_path.strip_prefix(&workspace_folder_path)?,
                )?)]
            }
            DebouncedEvent::Remove(absolute_path) => {
                vec![LocalEvent::DeletedLocalFile(util::path_to_string(
                    absolute_path.strip_prefix(&workspace_folder_path)?,
                )?)]
            }
            DebouncedEvent::Rename(absolute_source_path, absolute_dest_path) => {
                vec![LocalEvent::RenamedLocalFile(
                    util::path_to_string(
                        absolute_source_path.strip_prefix(&workspace_folder_path)?,
                    )?,
                    util::path_to_string(absolute_dest_path.strip_prefix(&workspace_folder_path)?)?,
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

        log::debug!(
            "[{}::{}] Produced messages for event: {:?}",
            self.context.instance_name,
            self.context.workspace_id,
            &messages,
        );
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
    context: Context,
    connection: Connection,
    operational_sender: Sender<LocalEvent>,
}

impl LocalSync {
    pub fn new(
        context: Context,
        connection: Connection,
        operational_sender: Sender<LocalEvent>,
    ) -> Result<Self, Error> {
        Ok(Self {
            context,
            connection,
            operational_sender,
        })
    }

    pub fn sync(&self) -> Result<(), Error> {
        // Look at disk files and compare to db
        self.sync_from_disk();
        // Look from db to search deleted files
        self.sync_from_db()?;

        Ok(())
    }

    fn sync_from_disk(&self) {
        WalkDir::new(&self.context.folder_path)
            .into_iter()
            .filter_entry(|e| !self.ignore_entry(e))
            .for_each(|dir_entry| match &dir_entry {
                Ok(dir_entry_) => match self.sync_disk_file(&dir_entry_) {
                    Ok(_) => {}
                    Err(error) => {
                        log::error!(
                            "[{}::{}] Fail to sync disk file {:?} : {:?}",
                            self.context.instance_name,
                            self.context.workspace_id,
                            dir_entry_,
                            error
                        );
                    }
                },
                Err(error) => {
                    log::error!(
                        "[{}::{}] Fail to walk on dir {:?} : '{}'",
                        self.context.instance_name,
                        self.context.workspace_id,
                        &dir_entry,
                        error
                    )
                }
            })
    }

    fn ignore_entry(&self, entry: &DirEntry) -> bool {
        let is_root = self.context.folder_path == entry.path().display().to_string();
        if !is_root && entry.file_type().is_dir() {
            // Ignore directory from local sync : changes can only be rename.
            // And modification time is problematic :https://github.com/buxx/trsync/issues/60
            return true;
        }

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
        let relative_path = entry.path().strip_prefix(&self.context.folder_path)?;
        // TODO : prevent sync root with more clean way
        if relative_path == Path::new("") {
            return Ok(());
        }

        let metadata = fs::metadata(Path::new(&self.context.folder_path).join(relative_path))?;
        let modified = metadata.modified()?;
        let disk_last_modified_timestamp = modified.duration_since(UNIX_EPOCH)?.as_millis() as u64;

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
                    log::info!(
                        "[{}::{}] File '{:?}' has been modified (disk timestamp='{}' != db timestamp='{}')",
                        self.context.instance_name,
                        self.context.workspace_id,
                        relative_path,
                        disk_last_modified_timestamp,
                        last_modified_timestamp,
                    );
                    match self.operational_sender.send(LocalEvent::ModifiedLocalFile(
                        util::path_to_string(relative_path)?,
                    )) {
                        Err(error) => {
                            log::error!(
                                "[{}::{}] Fail to send operational message : {:?}",
                                self.context.instance_name,
                                self.context.workspace_id,
                                error
                            )
                        }
                        _ => {}
                    }
                }
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => {
                // Unknown file
                match self
                    .operational_sender
                    .send(LocalEvent::NewLocalFile(util::path_to_string(
                        relative_path,
                    )?)) {
                    Err(error) => {
                        log::error!(
                            "[{}::{}] Fail to send operational message : {:?}",
                            self.context.instance_name,
                            self.context.workspace_id,
                            error
                        )
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
            if !Path::new(&self.context.folder_path)
                .join(&relative_path)
                .exists()
            {
                if self.context.prevent_delete_sync {
                    log::info!(
                        "[{}::{}] Ignore deleted local file {} by configuration",
                        self.context.instance_name,
                        self.context.workspace_id,
                        &relative_path
                    );
                    continue;
                }

                match self
                    .operational_sender
                    .send(LocalEvent::DeletedLocalFile(relative_path.clone()))
                {
                    Err(error) => {
                        log::error!(
                            "[{}::{}] Fail to send operational message : '{}'",
                            self.context.instance_name,
                            self.context.workspace_id,
                            error
                        )
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
    operational_sender: &Sender<LocalEvent>,
) -> JoinHandle<Result<(), Error>> {
    let local_sync_context = context.clone();
    let local_sync_operational_sender = operational_sender.clone();

    thread::spawn(move || {
        Database::new(local_sync_context.database_path.clone()).with_new_connection(
            |connection| {
                LocalSync::new(
                    local_sync_context,
                    connection,
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
    operational_sender: &Sender<LocalEvent>,
    stop_signal: &Arc<AtomicBool>,
    restart_signal: &Arc<AtomicBool>,
) -> Result<JoinHandle<Result<(), Error>>, Error> {
    let exit_after_sync = context.exit_after_sync;
    let local_watcher_context = context.clone();
    let local_watcher_operational_sender = operational_sender.clone();
    let local_watcher_stop_signal = stop_signal.clone();
    let local_watcher_restart_signal = restart_signal.clone();

    let mut local_watcher = LocalWatcher::new(
        local_watcher_context,
        local_watcher_stop_signal,
        local_watcher_restart_signal,
        local_watcher_operational_sender,
    )?;

    Ok(thread::spawn(move || {
        if !exit_after_sync {
            local_watcher.listen()
        } else {
            Ok(())
        }
    }))
}
