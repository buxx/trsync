use notify::DebouncedEvent;
use notify::{watcher, RecursiveMode, Watcher};
use std::path::Path;
use std::sync::mpsc::channel;
use std::sync::mpsc::Sender;
use std::time::Duration;

use crate::operation::OperationalMessage;

pub struct LocalWatcher {
    operational_sender: Sender<OperationalMessage>,
}

impl LocalWatcher {
    pub fn new(operational_sender: Sender<OperationalMessage>) -> Self {
        Self { operational_sender }
    }

    pub fn listen(&mut self, path: &Path) {
        let (inotify_sender, inotify_receiver) = channel();
        let mut inotify_watcher = watcher(inotify_sender, Duration::from_secs(1)).unwrap();
        inotify_watcher
            .watch(path, RecursiveMode::Recursive)
            .unwrap();

        loop {
            match inotify_receiver.recv() {
                Ok(event) => self.digest_event(event),
                Err(e) => println!("watch error: {:?}", e),
            }
        }
    }

    pub fn digest_event(&self, event: DebouncedEvent) {
        println!("Received local event: {:?}", event);
        match self
            .operational_sender
            .send(OperationalMessage::NewLocalRevision)
        {
            Ok(_) => (),
            Err(err) => {
                eprintln!(
                    "Error when send operational message from local watcher : {}",
                    err
                )
            }
        };
    }
}

// Represent known local files. When trsync start, it use this index to compare
// with real local files state and produce change messages.
pub struct LocalSync {}

impl LocalSync {
    pub fn new() -> Self {
        Self {}
    }

    pub fn sync(&mut self) {}
}
