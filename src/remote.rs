use std::{collections::HashMap, sync::mpsc::Sender, thread::sleep, time::Duration};

use crate::operation::OperationalMessage;

pub struct RemoteWatcher {
    operational_sender: Sender<OperationalMessage>,
}

// TODO : Must have a local db with tuple (content_id,modified_timestamp)

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
                .send(OperationalMessage::NewRemoteRevision)
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
