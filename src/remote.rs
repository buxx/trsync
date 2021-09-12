use std::{
    collections::HashMap,
    fs,
    ops::Rem,
    path::{Path, PathBuf},
    sync::mpsc::Sender,
    thread::sleep,
    time::Duration,
};

use serde_derive::{Deserialize, Serialize};

use reqwest::{blocking::Response, Method};
use rusqlite::Connection;

use crate::operation::OperationalMessage;

pub struct RemoteWatcher {
    operational_sender: Sender<OperationalMessage>,
}

// TODO : Must have a local db with tuple (content_id,modified_timestamp)

// Jon of this watcher is to react on remote changes : for now it is a simple
// pull of content list and comparison with cache. Future is to use TLM
impl RemoteWatcher {
    pub fn new(operational_sender: Sender<OperationalMessage>) -> Self {
        Self { operational_sender }
    }

    pub fn listen(&mut self) {
        loop {
            // Consume all content from api and look about changes
            sleep(Duration::from_secs(2));
            println!("Simulate remote event");
            match self
                .operational_sender
                .send(OperationalMessage::FakeMessage)
            {
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

#[derive(Serialize, Deserialize, Debug)]
pub struct RemoteContent {
    content_id: u32,
    parent_id: Option<u32>,
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
            .unwrap();

        for content in &contents {
            let relative_path = self.build_relative_path(content);
            println!("REMOTE {:?}", relative_path)
        }
    }

    fn build_relative_path(&self, content: &RemoteContent) -> String {
        if let Some(parent_id) = content.parent_id {
            let mut path_parts: Vec<String> = vec![content.filename.clone()];
            let mut last_seen_parent_id = parent_id;
            loop {
                let folder = self
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
                    .unwrap()
                    .json::<RemoteContent>()
                    .unwrap();

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
