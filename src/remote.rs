use async_std::task;
use std::{fs, path::PathBuf, sync::mpsc::Sender};

use chrono::DateTime;
use futures_util::StreamExt;
use serde_derive::{Deserialize, Serialize};
use serde_json::Value;
use std::str;

use reqwest::Method;
use rusqlite::{params, Connection};

use crate::{
    client::Client,
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

pub struct RemoteWatcher {
    operational_sender: Sender<OperationalMessage>,
    tracim_api_key: String,
    tracim_user_name: String,
}

// TODO : Must have a local db with tuple (content_id,modified_timestamp)

// Jon of this watcher is to react on remote changes : for now it is a simple
// pull of content list and comparison with cache. Future is to use TLM
impl RemoteWatcher {
    pub fn new(
        operational_sender: Sender<OperationalMessage>,
        tracim_api_key: String,
        tracim_user_name: String,
    ) -> Self {
        Self {
            operational_sender,
            tracim_api_key,
            tracim_user_name,
        }
    }

    pub fn listen(&mut self) {
        task::block_on(async {
            // TODO : Move into client (which provide a channel to listen or something like that)
            let response = reqwest::Client::new()
                .request(
                    Method::GET,
                    // TODO : Attention, quand erreur d'url, pas d'erreur ! attente infinis de event
                    "https://tracim.bux.fr/api/users/2/live_messages",
                )
                .header("Tracim-Api-Key", &self.tracim_api_key)
                .header("Tracim-Api-Login", &self.tracim_user_name)
                .send()
                .await
                .unwrap();
            let mut stream = response.bytes_stream();
            while let Some(thing) = stream.next().await {
                match &thing {
                    Ok(lines) => {
                        if lines.starts_with(b"event: message") {
                            for line in str::from_utf8(lines).unwrap().lines() {
                                if line.starts_with("data: ") {
                                    let json_as_str = &line[6..];
                                    let remote_event: RemoteEvent =
                                        serde_json::from_str(json_as_str).unwrap();
                                    println!(
                                        "REMOTE EVENT : {}",
                                        &remote_event.event_type.as_str()
                                    );
                                    if RemoteEventType::from_str(&remote_event.event_type.as_str())
                                        .is_some()
                                    {
                                        let content_id = remote_event.fields["content"]
                                            .as_object()
                                            .unwrap()["content_id"]
                                            .as_i64()
                                            .unwrap();
                                        println!("REMOTE EVENT content_id: {:?}", content_id);
                                        let message = match remote_event.event_type.as_str() {
                                            "content.modified.html-document"
                                            | "content.modified.file"
                                            | "content.modified.folder" => {
                                                OperationalMessage::ModifiedRemoteFile(
                                                    content_id as i32,
                                                )
                                            }
                                            "content.created.html-document"
                                            | "content.created.file"
                                            | "content.created.folder" => {
                                                OperationalMessage::NewRemoteFile(content_id as i32)
                                            }
                                            "content.deleted.html-document"
                                            | "content.deleted.file"
                                            | "content.deleted.folder" => {
                                                OperationalMessage::DeletedRemoteFile(
                                                    content_id as i32,
                                                )
                                            }
                                            "content.undeleted.html-document"
                                            | "content.undeleted.file"
                                            | "content.undeleted.folder" => {
                                                OperationalMessage::NewRemoteFile(content_id as i32)
                                            }
                                            _ => {
                                                panic!(
                                                "Source code must cover all ACCEPTED_EVENT_TYPES"
                                            )
                                            }
                                        };
                                        match self.operational_sender.send(message) {
                                            Ok(_) => (),
                                            Err(err) => {
                                                eprintln!(
                                                    "Error when send operational message from remote watcher : {}",
                                                    err
                                                )
                                            }
                                        };
                                    }
                                }
                            }
                        }
                    }
                    Err(err) => {
                        eprintln!("Err when reading remote TLM : {:?}", err)
                    }
                }
            }
        });
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RemoteContent {
    pub content_id: ContentId,
    pub revision_id: RevisionId,
    pub parent_id: Option<i32>,
    pub content_type: String,
    pub modified: String,
    pub filename: String,
}

pub struct RemoteSync {
    connection: Connection,
    client: Client,
    path: PathBuf,
    operational_sender: Sender<OperationalMessage>,
}

impl RemoteSync {
    pub fn new(
        connection: Connection,
        path: PathBuf,
        operational_sender: Sender<OperationalMessage>,
        tracim_api_key: String,
        tracim_user_name: String,
    ) -> Self {
        Self {
            connection,
            client: Client::new(tracim_api_key, tracim_user_name),
            path: fs::canonicalize(&path).unwrap(),
            operational_sender,
        }
    }

    pub fn sync(&mut self) {
        // TODO : move into client
        let contents = self.client.get_remote_contents(None).unwrap();
        let content_ids: Vec<i32> = contents.iter().map(|c| c.content_id).collect();

        for content in &contents {
            // TODO : use database module
            match DatabaseOperation::new(&self.connection)
                .get_revision_id_from_content_id(content.content_id)
            {
                Ok(known_revision_id) => {
                    // File is known but have been modified ?
                    if known_revision_id != content.revision_id {
                        self.operational_sender
                            .send(OperationalMessage::ModifiedRemoteFile(content.content_id))
                            .unwrap();
                    }
                }
                Err(rusqlite::Error::QueryReturnedNoRows) => {
                    self.operational_sender
                        .send(OperationalMessage::NewRemoteFile(content.content_id))
                        .unwrap();
                }
                Err(error) => {
                    eprintln!("Error when comparing revision : {}", error)
                }
            }
        }

        // Search for remote deleted files
        // TODO : move into database module
        let mut stmt = self
            .connection
            .prepare("SELECT content_id FROM file WHERE content_id IS NOT NULL")
            .unwrap();
        let local_iter = stmt.query_map([], |row| Ok(row.get(0).unwrap())).unwrap();
        for result in local_iter {
            let content_id: i32 = result.unwrap();
            if !content_ids.contains(&content_id) {
                println!("remotely deleted {:?}", content_id);
                self.operational_sender
                    .send(OperationalMessage::DeletedRemoteFile(content_id))
                    .unwrap();
            }
        }
    }
}
