use crate::database::Database;
use crate::error::Error;
use async_std::task;
use bytes::Bytes;
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::Sender,
        Arc,
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use futures_util::StreamExt;
use serde_derive::{Deserialize, Serialize};
use serde_json::Value;
use std::str;
use tokio::time::timeout;

use rusqlite::Connection;

use crate::{
    client::{self, Client},
    context::Context,
    database::DatabaseOperation,
    operation::OperationalMessage,
    types::{ContentId, RemoteEventType, RevisionId},
};

const LAST_ACTIVITY_TIMEOUT: u64 = 60;

#[derive(Serialize, Deserialize, Debug)]
pub struct RemoteEvent {
    event_id: i32,
    event_type: String,
    fields: Value,
}

impl RemoteEvent {
    pub fn from_str(json_as_str: &str) -> Result<Self, serde_json::Error> {
        let event: Self = match serde_json::from_str(json_as_str) {
            Ok(event) => event,
            Err(error) => return Err(error),
        };

        Ok(event)
    }
}

pub struct RemoteWatcher {
    context: Context,
    stop_signal: Arc<AtomicBool>,
    restart_signal: Arc<AtomicBool>,
    operational_sender: Sender<OperationalMessage>,
}

// TODO : Must have a local db with tuple (content_id,modified_timestamp)

// Jon of this watcher is to react on remote changes : for now it is a simple
// pull of content list and comparison with cache. Future is to use TLM
impl RemoteWatcher {
    pub fn new(
        context: Context,
        stop_signal: Arc<AtomicBool>,
        restart_signal: Arc<AtomicBool>,
        operational_sender: Sender<OperationalMessage>,
    ) -> Self {
        Self {
            context,
            stop_signal,
            restart_signal,
            operational_sender,
        }
    }

    pub fn listen(&mut self) -> Result<(), Error> {
        task::block_on::<_, Result<(), Error>>(async {
            let client = client::Client::new(self.context.clone())?;
            let user_id = client.get_user_id()?;
            let response = client.get_user_live_messages_response(user_id).await?;
            let mut stream = response.bytes_stream();

            let mut last_activity = Instant::now();
            loop {
                match timeout(Duration::from_millis(250), stream.next()).await {
                    Ok(Some(things)) => {
                        last_activity = Instant::now();
                        match &things {
                            Ok(lines) => match self.proceed_event_lines(lines) {
                                Err(error) => {
                                    log::error!(
                                        "Error when proceed remote event lines: {:?}",
                                        error
                                    )
                                }
                                _ => {}
                            },
                            Err(err) => {
                                log::error!("Error when reading remote TLM : {:?}", err);
                                // TODO : What to do here ?
                            }
                        }
                    }
                    _ => {
                        if last_activity.elapsed().as_secs() > LAST_ACTIVITY_TIMEOUT {
                            log::info!(
                                "No activity since '{}' seconds, break",
                                LAST_ACTIVITY_TIMEOUT
                            );
                            self.restart_signal.swap(true, Ordering::Relaxed);
                            break;
                        }
                    }
                }

                if self.stop_signal.load(Ordering::Relaxed) {
                    log::info!("Finished remote listening (on stop signal)");
                    break;
                }
            }

            Ok(())
        })?;

        Ok(())
    }

    fn proceed_event_lines(&self, lines: &Bytes) -> Result<(), Error> {
        if lines.starts_with(b"event: message") {
            for line in str::from_utf8(lines)?.lines() {
                if line.starts_with("data: ") {
                    let json_as_str = &line[6..];
                    match RemoteEvent::from_str(json_as_str) {
                        Ok(remote_event) => self.proceed_remote_event(remote_event)?,
                        Err(error) => {
                            log::error!(
                                "Error when decoding event : '{}'. Event as str was: '{}'",
                                error,
                                json_as_str
                            )
                        }
                    };
                }
            }
        }

        Ok(())
    }

    fn proceed_remote_event(&self, remote_event: RemoteEvent) -> Result<(), Error> {
        log::debug!("Proceed remote event {:?}", remote_event);

        if RemoteEventType::from_str(&remote_event.event_type.as_str()).is_some() {
            let content_id =
                remote_event.fields["content"]
                    .as_object()
                    .ok_or(Error::UnexpectedError(format!(
                        "Remote event content not appear to not be object"
                    )))?["content_id"]
                    .as_i64()
                    .ok_or(Error::UnexpectedError(format!(
                        "Remote event content content_id appear to not be integer"
                    )))?;
            let workspace_id =
                remote_event.fields["workspace"]
                    .as_object()
                    .ok_or(Error::UnexpectedError(format!(
                        "Remote event workspace not appear to not be object"
                    )))?["workspace_id"]
                    .as_i64()
                    .ok_or(Error::UnexpectedError(format!(
                        "Remote event workspace workspace_id appear to not be integer"
                    )))?;

            if self.context.workspace_id != workspace_id as i32 {
                log::debug!("Remote event is not for current workspace, skip");
                return Ok(());
            }

            log::info!(
                "remote event : {:} ({})",
                &remote_event.event_type.as_str(),
                content_id,
            );
            let event_type = remote_event.event_type.as_str();
            let message = match event_type {
                "content.modified.html-document"
                | "content.modified.file"
                | "content.modified.folder" => {
                    OperationalMessage::ModifiedRemoteFile(content_id as i32)
                }
                "content.created.html-document"
                | "content.created.file"
                | "content.created.folder" => OperationalMessage::NewRemoteFile(content_id as i32),
                "content.deleted.html-document"
                | "content.deleted.file"
                | "content.deleted.folder" => {
                    OperationalMessage::DeletedRemoteFile(content_id as i32)
                }
                "content.undeleted.html-document"
                | "content.undeleted.file"
                | "content.undeleted.folder" => {
                    OperationalMessage::NewRemoteFile(content_id as i32)
                }
                _ => {
                    return Err(Error::UnexpectedError(format!(
                        "Not managed event type : '{}'",
                        event_type
                    )))
                }
            };
            match self.operational_sender.send(message) {
                Ok(_) => (),
                // TODO : stop trsync ?
                Err(err) => {
                    log::error!(
                        "Error when send operational message from remote watcher : '{}'",
                        err
                    )
                }
            };
        } else {
            log::debug!(
                "Ignore remote event : '{}'",
                &remote_event.event_type.as_str()
            )
        }

        Ok(())
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RemoteContent {
    pub content_id: ContentId,
    pub current_revision_id: RevisionId,
    pub parent_id: Option<i32>,
    pub content_type: String,
    pub modified: String,
    pub raw_content: Option<String>,
    pub filename: String,
    pub is_deleted: bool,
    pub is_archived: bool,
}

pub struct RemoteSync {
    _context: Context,
    connection: Connection,
    client: Client,
    operational_sender: Sender<OperationalMessage>,
}

impl RemoteSync {
    pub fn new(
        context: Context,
        connection: Connection,
        operational_sender: Sender<OperationalMessage>,
    ) -> Result<Self, Error> {
        Ok(Self {
            _context: context.clone(),
            connection,
            client: Client::new(context)?,
            operational_sender,
        })
    }

    pub fn sync(&mut self) -> Result<(), Error> {
        let contents = self.client.get_remote_contents(None)?;
        let remote_content_ids: Vec<i32> = contents.iter().map(|c| c.content_id).collect();

        for content in &contents {
            match DatabaseOperation::new(&self.connection)
                .get_revision_id_from_content_id(content.content_id)
            {
                Ok(known_revision_id) => {
                    // File is known but have been modified ?
                    if known_revision_id != content.current_revision_id {
                        match self
                            .operational_sender
                            .send(OperationalMessage::ModifiedRemoteFile(content.content_id))
                        {
                            Err(error) => {
                                log::error!(
                                    "Error when send operational message from remote sync : '{}'",
                                    error
                                )
                            }
                            _ => {}
                        }
                    }
                }
                Err(rusqlite::Error::QueryReturnedNoRows) => {
                    match self
                        .operational_sender
                        .send(OperationalMessage::NewRemoteFile(content.content_id))
                    {
                        Err(error) => {
                            log::error!(
                                "Error when send operational message from remote sync : '{}'",
                                error
                            )
                        }
                        _ => {}
                    }
                }
                Err(error) => {
                    log::error!("Error when comparing revision : '{}'", error)
                }
            }
        }

        // Search for remote deleted files
        let content_ids = DatabaseOperation::new(&self.connection).get_content_ids()?;
        for content_id in &content_ids {
            if !remote_content_ids.contains(content_id) {
                match self
                    .operational_sender
                    .send(OperationalMessage::DeletedRemoteFile(*content_id))
                {
                    Err(error) => {
                        log::error!(
                            "Error when send operational message from remote sync : '{}'",
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

pub fn start_remote_sync(
    context: &Context,
    operational_sender: &Sender<OperationalMessage>,
) -> JoinHandle<Result<(), Error>> {
    let remote_sync_context = context.clone();
    let remote_sync_operational_sender = operational_sender.clone();

    thread::spawn(move || {
        Database::new(remote_sync_context.database_path.clone()).with_new_connection(
            |connection| {
                RemoteSync::new(
                    remote_sync_context,
                    connection,
                    remote_sync_operational_sender,
                )?
                .sync()?;
                Ok(())
            },
        )?;

        Ok(())
    })
}

pub fn start_remote_watch(
    context: &Context,
    operational_sender: &Sender<OperationalMessage>,
    stop_signal: &Arc<AtomicBool>,
    restart_signal: &Arc<AtomicBool>,
) -> Result<JoinHandle<Result<(), Error>>, Error> {
    let exit_after_sync = context.exit_after_sync;
    let remote_watcher_context = context.clone();
    let remote_watcher_stop_signal = stop_signal.clone();
    let remote_watcher_restart_signal = restart_signal.clone();
    let remote_watcher_operational_sender = operational_sender.clone();

    let mut remote_watcher = RemoteWatcher::new(
        remote_watcher_context,
        remote_watcher_stop_signal,
        remote_watcher_restart_signal,
        remote_watcher_operational_sender,
    );

    Ok(thread::spawn(move || {
        if !exit_after_sync {
            remote_watcher.listen()
        } else {
            Ok(())
        }
    }))
}
