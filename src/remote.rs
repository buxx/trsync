use std::{collections::HashMap, ops::Rem, sync::mpsc::Sender, thread::sleep, time::Duration};

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

pub struct RemoteSync {
    connection: Connection,
}

impl RemoteSync {
    pub fn new(connection: Connection) -> Self {
        Self { connection }
    }

    pub fn sync(&mut self) {}
}
