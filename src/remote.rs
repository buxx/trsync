use crate::Error;
use async_std::task;
use bytes::Bytes;
use std::sync::mpsc::Sender;

use futures_util::StreamExt;
use serde_derive::{Deserialize, Serialize};
use serde_json::Value;
use std::str;

use rusqlite::Connection;

use crate::{
    client::{self, Client},
    context::Context,
    database::DatabaseOperation,
    operation::OperationalMessage,
    types::{ContentId, RemoteEventType, RevisionId},
};

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
    operational_sender: Sender<OperationalMessage>,
}

// TODO : Must have a local db with tuple (content_id,modified_timestamp)

// Jon of this watcher is to react on remote changes : for now it is a simple
// pull of content list and comparison with cache. Future is to use TLM
impl RemoteWatcher {
    pub fn new(context: Context, operational_sender: Sender<OperationalMessage>) -> Self {
        Self {
            context,
            operational_sender,
        }
    }

    pub fn listen(&mut self) -> Result<(), Error> {
        task::block_on::<_, Result<(), Error>>(async {
            let client = client::Client::new(self.context.clone())?;
            let user_id = client.get_user_id()?;
            let response = client.get_user_live_messages_response(user_id).await?;
            let mut stream = response.bytes_stream();
            while let Some(thing) = stream.next().await {
                match &thing {
                    Ok(lines) => match self.proceed_event_lines(lines) {
                        Err(error) => {
                            log::error!("Error when proceed remote event lines: {:?}", error)
                        }
                        _ => {}
                    },
                    Err(err) => {
                        log::error!("Error when reading remote TLM : {:?}", err)
                    }
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
                                "Error when decoding event : {}. Event as str was: {}",
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
                        "Remote event content not appear to not be obect"
                    )))?["content_id"]
                    .as_i64()
                    .ok_or(Error::UnexpectedError(format!(
                        "Remote event content content_id appear to not be integer"
                    )))?;
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
                        "Not managed event type : {}",
                        event_type
                    )))
                }
            };
            match self.operational_sender.send(message) {
                Ok(_) => (),
                // FIXME : stop trsync
                Err(err) => {
                    log::error!(
                        "Error when send operational message from remote watcher : {}",
                        err
                    )
                }
            };
        } else {
            log::debug!(
                "Ignore remote event : {}",
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
    pub filename: String,
    pub is_deleted: bool,
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
                                    "Error when send operational message from remote sync : {}",
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
                                "Error when send operational message from remote sync : {}",
                                error
                            )
                        }
                        _ => {}
                    }
                }
                Err(error) => {
                    log::error!("Error when comparing revision : {}", error)
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
                            "Error when send operational message from remote sync : {}",
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
