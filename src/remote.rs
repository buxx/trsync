use async_std::task;
use std::{
    fs,
    path::{Path, PathBuf},
    sync::mpsc::Sender,
    thread::sleep,
    time::Duration,
};

use chrono::DateTime;
use futures_util::StreamExt;
use serde_derive::{Deserialize, Serialize};
use serde_json::Value;
use std::str;

use reqwest::Method;
use rusqlite::{params, Connection};

use crate::operation::OperationalMessage;

const ACCEPTED_CONTENT_TYPES: &'static [&'static str] = &["html-document", "file", "folder"];
const ACCEPTED_EVENT_TYPES: &'static [&'static str] = &[
    "content.modified.html-document",
    "content.modified.file",
    "content.modified.folder",
    "content.created.html-document",
    "content.created.file",
    "content.created.folder",
    "content.deleted.html-document",
    "content.deleted.file",
    "content.deleted.folder",
    "content.undeleted.html-document",
    "content.undeleted.file",
    "content.undeleted.folder",
];

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
                                    println!("TYPE : {}", &remote_event.event_type.as_str());
                                    if ACCEPTED_EVENT_TYPES
                                        .contains(&remote_event.event_type.as_str())
                                    {
                                        let content_id = remote_event.fields["content"]
                                            .as_object()
                                            .unwrap()["content_id"]
                                            .as_i64()
                                            .unwrap();
                                        println!("EVENT: {:?}", &remote_event);
                                        println!("EVENT content_id: {:?}", content_id);
                                        let message = match remote_event.event_type.as_str() {
                                            "content.modified.html-document"
                                            | "content.modified.file"
                                            | "content.modified.folder" => {
                                                OperationalMessage::IndexedRemoteFileModified(
                                                    content_id as i32,
                                                )
                                            }
                                            "content.created.html-document"
                                            | "content.created.file"
                                            | "content.created.folder" => {
                                                OperationalMessage::UnIndexedRemoteFileAppear(
                                                    content_id as i32,
                                                )
                                            }
                                            "content.deleted.html-document"
                                            | "content.deleted.file"
                                            | "content.deleted.folder" => {
                                                OperationalMessage::IndexedRemoteFileDeleted(
                                                    content_id as i32,
                                                )
                                            }
                                            "content.undeleted.html-document"
                                            | "content.undeleted.file"
                                            | "content.undeleted.folder" => {
                                                OperationalMessage::UnIndexedRemoteFileAppear(
                                                    content_id as i32,
                                                )
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

#[derive(Serialize, Deserialize, Debug)]
pub struct RemoteContent {
    content_id: i32,
    parent_id: Option<u32>,
    content_type: String,
    modified: String,
    filename: String,
}

pub struct RemoteSync {
    connection: Connection,
    client: reqwest::blocking::Client,
    path: PathBuf,
    operational_sender: Sender<OperationalMessage>,
    tracim_api_key: String,
    tracim_user_name: String,
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
            client: reqwest::blocking::Client::new(),
            path: fs::canonicalize(&path).unwrap(),
            operational_sender,
            tracim_api_key,
            tracim_user_name,
        }
    }

    pub fn sync(&mut self) {
        let contents = self
            .client
            .request(
                Method::GET,
                "https://tracim.bux.fr/api/workspaces/3/contents",
            )
            .header("Tracim-Api-Key", &self.tracim_api_key)
            .header("Tracim-Api-Login", &self.tracim_user_name)
            .send()
            .unwrap()
            .json::<Vec<RemoteContent>>()
            .unwrap()
            .into_iter()
            .filter(|c| ACCEPTED_CONTENT_TYPES.contains(&c.content_type.as_str()))
            .collect::<Vec<RemoteContent>>();
        let content_ids: Vec<i32> = contents.iter().map(|c| c.content_id).collect();

        for content in &contents {
            match self.connection.query_row::<i64, _, _>(
                "SELECT last_modified_timestamp FROM file WHERE content_id = ?",
                params![content.content_id],
                |row| row.get(0),
            ) {
                Ok(last_modified_timestamp) => {
                    let modified_timestamp = DateTime::parse_from_rfc3339(&content.modified)
                        .unwrap()
                        .timestamp_millis();
                    // File is known but have been modified ?
                    if last_modified_timestamp != modified_timestamp {
                        // TODO : This update must be done in Operation !
                        self.connection
                            .execute(
                                "UPDATE file SET last_modified_timestamp = ?1 WHERE content_id = ?2",
                                params![modified_timestamp, content.content_id],
                            )
                            .unwrap();
                        self.operational_sender
                            .send(OperationalMessage::IndexedRemoteFileModified(
                                content.content_id,
                            ))
                            .unwrap();
                    }
                }
                Err(_) => {
                    let relative_path = self.build_relative_path(content);
                    let modified_timestamp = DateTime::parse_from_rfc3339(&content.modified)
                        .unwrap()
                        .timestamp_millis();
                    // TODO : This update must be done in Operation !
                    self.connection
                    .execute(
                        "INSERT INTO file (relative_path, last_modified_timestamp, content_id) VALUES (?1, ?2, ?3)",
                        params![relative_path, modified_timestamp, content.content_id],
                    )
                    .unwrap();
                    self.operational_sender
                        .send(OperationalMessage::UnIndexedRemoteFileAppear(
                            content.content_id,
                        ))
                        .unwrap();
                }
            }
        }

        // Search for remote deleted files
        let mut stmt = self
            .connection
            .prepare("SELECT content_id FROM file WHERE content_id IS NOT NULL")
            .unwrap();
        let local_iter = stmt.query_map([], |row| Ok(row.get(0).unwrap())).unwrap();
        for result in local_iter {
            let content_id: i32 = result.unwrap();
            if !content_ids.contains(&content_id) {
                println!("remotely deleted {:?}", content_id);
                // TODO : This update must be done in Operation !
                self.connection
                    .execute(
                        "DELETE FROM file WHERE content_id = ?1",
                        params![content_id],
                    )
                    .unwrap();
                self.operational_sender
                    .send(OperationalMessage::IndexedRemoteFileDeleted(content_id))
                    .unwrap();
            }
        }
    }

    fn build_relative_path(&self, content: &RemoteContent) -> String {
        if let Some(parent_id) = content.parent_id {
            let mut path_parts: Vec<String> = vec![content.filename.clone()];
            let mut last_seen_parent_id = parent_id;
            loop {
                let response = self
                    .client
                    .request(
                        Method::GET,
                        format!(
                            "https://tracim.bux.fr/api/workspaces/3/folders/{}",
                            last_seen_parent_id
                        ),
                    )
                    .header("Tracim-Api-Key", &self.tracim_api_key)
                    .header("Tracim-Api-Login", &self.tracim_user_name)
                    .send()
                    .unwrap();
                let folder = response.json::<RemoteContent>().unwrap();
                path_parts.push(folder.filename);
                if let Some(folder_parent_id) = folder.parent_id {
                    last_seen_parent_id = folder_parent_id;
                } else {
                    // TODO : this is very ugly code !
                    let mut relative_path_string = "".to_string();
                    for path_part in path_parts.iter().rev() {
                        let relative_path = Path::new(&relative_path_string).join(path_part);
                        relative_path_string = relative_path.to_str().unwrap().to_string();
                    }
                    return relative_path_string;
                }
            }
        } else {
            content.filename.clone()
        }
    }
}
