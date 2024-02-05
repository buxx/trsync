use crate::context::Context as TrSyncContext;
use anyhow::{Context, Result};
use crossbeam_channel::Sender;
use notify::DebouncedEvent;
use notify::{watcher, RecursiveMode, Watcher};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, RecvTimeoutError};
use std::sync::Arc;
use std::time::Duration;
use trsync_core::types::ContentType;

use crate::util::ignore_file;

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum DiskEvent {
    Deleted(PathBuf),
    Created(PathBuf),
    Modified(PathBuf),
    Renamed(PathBuf, PathBuf),
}

pub struct LocalWatcher {
    context: TrSyncContext,
    stop_signal: Arc<AtomicBool>,
    restart_signal: Arc<AtomicBool>,
    operational_sender: Sender<DiskEvent>,
}

trait IntoRelative {
    fn relative(&self, prefix: &Path) -> Result<PathBuf>;
}

impl IntoRelative for PathBuf {
    fn relative(&self, prefix: &Path) -> Result<PathBuf> {
        Ok(self
            .strip_prefix(prefix)
            .context(format!(
                "Strip path prefix {} from {}",
                &prefix.display(),
                self.display(),
            ))?
            .to_path_buf())
    }
}

impl LocalWatcher {
    pub fn new(
        context: TrSyncContext,
        stop_signal: Arc<AtomicBool>,
        restart_signal: Arc<AtomicBool>,
        operational_sender: Sender<DiskEvent>,
    ) -> Result<Self> {
        Ok(Self {
            context,
            stop_signal,
            restart_signal,
            operational_sender,
        })
    }

    pub fn listen(&mut self) -> Result<()> {
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
                Ok(event) => {
                    if let Err(error) = self.digest_event(&event, &workspace_folder_path) {
                        log::error!(
                            "[{}::{}] Error when digest event {:?} : {:?}",
                            self.context.instance_name,
                            self.context.workspace_id,
                            error,
                            &event,
                        )
                    }
                }
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

    pub fn digest_event(&self, event: &DebouncedEvent, workspace: &Path) -> Result<()> {
        log::debug!(
            "[{}::{}] Local event received: {:?}",
            self.context.instance_name,
            self.context.workspace_id,
            &event,
        );

        let relative_path = match event {
            DebouncedEvent::Create(absolute_path)
            | DebouncedEvent::Write(absolute_path)
            | DebouncedEvent::Remove(absolute_path)
            | DebouncedEvent::Rename(absolute_path, _) => absolute_path.relative(workspace)?,
            // Ignore these
            DebouncedEvent::NoticeWrite(_)
            | DebouncedEvent::NoticeRemove(_)
            | DebouncedEvent::Chmod(_)
            | DebouncedEvent::Rescan => return Ok(()),
            // Consider Error as to log it
            DebouncedEvent::Error(err, path) => {
                log::error!("Error {} on {:?}", err, path);
                return Ok(());
            }
        };

        if ignore_file(&relative_path) {
            log::debug!("Ignore local event on {}", relative_path.display());
            return Ok(());
        }

        let messages: Vec<DiskEvent> = match event {
            DebouncedEvent::Create(absolute_path) => {
                vec![DiskEvent::Created(absolute_path.relative(workspace)?)]
            }
            DebouncedEvent::Write(absolute_path) => {
                vec![DiskEvent::Modified(absolute_path.relative(workspace)?)]
            }
            DebouncedEvent::Remove(absolute_path) => {
                vec![DiskEvent::Deleted(absolute_path.relative(workspace)?)]
            }
            DebouncedEvent::Rename(absolute_source_path, absolute_dest_path) => {
                let before_content_type = ContentType::from_path(absolute_source_path);
                let after_content_type = ContentType::from_path(absolute_dest_path);

                // If renaming change the content-type: consider as new file
                if before_content_type != after_content_type {
                    vec![
                        DiskEvent::Deleted(absolute_source_path.relative(workspace)?),
                        DiskEvent::Created(absolute_dest_path.relative(workspace)?),
                    ]
                } else {
                    vec![DiskEvent::Renamed(
                        absolute_source_path.relative(workspace)?,
                        absolute_dest_path.relative(workspace)?,
                    )]
                }
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
                    // FIXME BS NOW : channel closed after restart after error/lost connection ? But didn't break local sync ...
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
