use crate::error::Error;
use crate::event::remote::RemoteEvent;
use async_std::task;
use bytes::Bytes;
use crossbeam_channel::Sender;
use std::{
    fmt::Display,
    str::FromStr,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};
use thiserror::Error;
use trsync_core::instance::ContentId as ContentId2;
use trsync_core::types::{ContentId, RemoteEventType, RevisionId};

use futures_util::StreamExt;
use serde_derive::{Deserialize, Serialize};
use serde_json::Value;
use std::str;
use tokio::time::timeout;

use rusqlite::Connection;

use crate::{context::Context, database::DatabaseOperation};

const LAST_ACTIVITY_TIMEOUT: u64 = 60;

#[derive(Serialize, Deserialize, Debug)]
pub struct TracimLiveEvent {
    event_id: i32,
    event_type: String,
    fields: Value,
}

#[derive(Error, Debug)]
pub struct ParseTracimLiveEventError(String, serde_json::Error);

impl Display for ParseTracimLiveEventError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&format!(
            "Error when parsing Tracim live event '{}' : {}",
            self.0, self.1
        ))
    }
}

impl FromStr for TracimLiveEvent {
    type Err = ParseTracimLiveEventError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match serde_json::from_str(value) {
            Ok(event) => Ok(event),
            Err(error) => Err(ParseTracimLiveEventError(value.to_string(), error)),
        }
    }
}

pub struct RemoteWatcher {
    connection: Connection,
    context: Context,
    stop_signal: Arc<AtomicBool>,
    restart_signal: Arc<AtomicBool>,
    operational_sender: Sender<RemoteEvent>,
}

// TODO : Must have a local db with tuple (content_id,modified_timestamp)

// Jon of this watcher is to react on remote changes : for now it is a simple
// pull of content list and comparison with cache. Future is to use TLM
impl RemoteWatcher {
    pub fn new(
        connection: Connection,
        context: Context,
        stop_signal: Arc<AtomicBool>,
        restart_signal: Arc<AtomicBool>,
        operational_sender: Sender<RemoteEvent>,
    ) -> Self {
        Self {
            connection,
            context,
            stop_signal,
            restart_signal,
            operational_sender,
        }
    }

    pub fn listen(&mut self) -> Result<(), Error> {
        task::block_on::<_, Result<(), Error>>(async {
            let client = self.context.client().map_err(|err| {
                Error::UnexpectedError(format!("Error when create Tracim client : {}", err))
            })?;
            let user_id = client.get_user_id().map_err(|err| {
                Error::UnexpectedError(format!("Error when create get user id : {}", err))
            })?;
            let response = client
                .get_user_live_messages_response(user_id)
                .await
                .map_err(|err| {
                    Error::UnexpectedError(format!(
                        "Error when get live message response : {}",
                        err
                    ))
                })?;
            let mut stream = response.bytes_stream();

            let mut last_activity = Instant::now();
            loop {
                match timeout(Duration::from_millis(250), stream.next()).await {
                    Ok(Some(things)) => {
                        last_activity = Instant::now();
                        match &things {
                            Ok(lines) => {
                                if let Err(error) = self.proceed_event_lines(lines) {
                                    log::error!(
                                        "Error when proceed remote event lines: {:?}",
                                        error
                                    )
                                }
                            }
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

    #[allow(clippy::manual_strip)]
    fn proceed_event_lines(&self, lines: &Bytes) -> Result<(), Error> {
        if lines.starts_with(b"event: message") {
            for line in str::from_utf8(lines)?.lines() {
                if line.starts_with("data: ") {
                    let json_as_str = &line[6..];
                    match TracimLiveEvent::from_str(json_as_str) {
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

    fn proceed_remote_event(&self, remote_event: TracimLiveEvent) -> Result<(), Error> {
        log::debug!("Proceed remote event {:?}", remote_event);

        if RemoteEventType::from_str(&remote_event.event_type).is_err() {
            let content_id =
                remote_event.fields["content"]
                    .as_object()
                    .ok_or(Error::UnexpectedError(
                        "Remote event content not appear to not be object".to_string(),
                    ))?["content_id"]
                    .as_i64()
                    .ok_or(Error::UnexpectedError(
                        "Remote event content content_id appear to not be integer".to_string(),
                    ))? as i32;
            let workspace_id =
                remote_event.fields["workspace"]
                    .as_object()
                    .ok_or(Error::UnexpectedError(
                        "Remote event workspace not appear to not be object".to_string(),
                    ))?["workspace_id"]
                    .as_i64()
                    .ok_or(Error::UnexpectedError(
                        "Remote event workspace workspace_id appear to not be integer".to_string(),
                    ))?;

            if let Some(message) = {
                if self.context.workspace_id.0 != workspace_id as i32 {
                    // If content exist locally that means content has change its workspace id
                    if DatabaseOperation::new(&self.connection).content_id_is_known(content_id)? {
                        Some(RemoteEvent::Deleted(ContentId2(content_id)))
                    } else {
                        log::debug!("Remote event is not for current workspace, skip");
                        None
                    }
                } else {
                    log::info!(
                        "remote event : {:} ({})",
                        &remote_event.event_type.as_str(),
                        content_id,
                    );
                    let event_type = remote_event.event_type.as_str();
                    let message = match event_type {
                        "content.modified.html-document"
                        | "content.modified.file"
                        | "content.modified.folder" => RemoteEvent::Updated(ContentId2(content_id)),
                        "content.created.html-document"
                        | "content.created.file"
                        | "content.created.folder" => RemoteEvent::Created(ContentId2(content_id)),
                        "content.deleted.html-document"
                        | "content.deleted.file"
                        | "content.deleted.folder" => RemoteEvent::Deleted(ContentId2(content_id)),
                        "content.undeleted.html-document"
                        | "content.undeleted.file"
                        | "content.undeleted.folder" => {
                            RemoteEvent::Created(ContentId2(content_id))
                        }
                        _ => {
                            return Err(Error::UnexpectedError(format!(
                                "Not managed event type : '{}'",
                                event_type
                            )))
                        }
                    };

                    Some(message)
                }
            } {
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
            }
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
    pub sub_content_types: Vec<String>,
}
