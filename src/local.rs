use notify::DebouncedEvent;
use notify::{watcher, RecursiveMode, Watcher};
use rusqlite::Connection;
use std::path::{Path, PathBuf};
use std::sync::mpsc::channel;
use std::sync::mpsc::Sender;
use std::time::Duration;
use walkdir::{DirEntry, WalkDir};

use crate::operation::OperationalMessage;

pub struct LocalWatcher {
    operational_sender: Sender<OperationalMessage>,
}

impl LocalWatcher {
    pub fn new(operational_sender: Sender<OperationalMessage>) -> Self {
        Self { operational_sender }
    }

    pub fn listen(&mut self, path: &PathBuf) {
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
pub struct LocalSync {
    connection: Connection,
    path: PathBuf,
}

impl LocalSync {
    pub fn new(connection: Connection, path: PathBuf) -> Self {
        Self { connection, path }
    }

    pub fn sync(&mut self) {
        WalkDir::new(".")
            .into_iter()
            .filter_entry(|e| self.ignore_entry(e))
            .for_each(|x| println!("sync {:?}", x));
    }

    fn ignore_entry(&self, file: &DirEntry) -> bool {
        false // TODO : according to pattern
    }
}
