use notify::DebouncedEvent;
use notify::{watcher, RecursiveMode, Watcher};
use rusqlite::{params, Connection};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc::channel;
use std::sync::mpsc::Sender;
use std::time::{Duration, UNIX_EPOCH};
use walkdir::{DirEntry, WalkDir};

use crate::operation::OperationalMessage;

pub struct LocalWatcher {
    operational_sender: Sender<OperationalMessage>,
    workspace_folder_path: PathBuf,
}

impl LocalWatcher {
    pub fn new(
        operational_sender: Sender<OperationalMessage>,
        workspace_folder_path: PathBuf,
    ) -> Self {
        Self {
            operational_sender,
            workspace_folder_path: fs::canonicalize(&workspace_folder_path).unwrap(),
        }
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

        let messages: Vec<OperationalMessage> = match event {
            DebouncedEvent::Create(absolute_path) => {
                let relative_path = absolute_path
                    .strip_prefix(&self.workspace_folder_path)
                    .unwrap();
                vec![OperationalMessage::NewLocalFile(
                    relative_path.to_str().unwrap().to_string(),
                )]
            }
            DebouncedEvent::Write(absolute_path) => {
                let relative_path = absolute_path
                    .strip_prefix(&self.workspace_folder_path)
                    .unwrap();
                vec![OperationalMessage::ModifiedLocalFile(
                    relative_path.to_str().unwrap().to_string(),
                )]
            }
            DebouncedEvent::Remove(absolute_path) => {
                let relative_path = absolute_path
                    .strip_prefix(&self.workspace_folder_path)
                    .unwrap();
                vec![OperationalMessage::DeletedLocalFile(
                    relative_path.to_str().unwrap().to_string(),
                )]
            }
            DebouncedEvent::Rename(_absolute_source_path, _absolute_dest_path) => {
                // TODO : manage this case
                vec![]
            }
            // Ignore these
            DebouncedEvent::NoticeWrite(_)
            | DebouncedEvent::NoticeRemove(_)
            | DebouncedEvent::Chmod(_)
            | DebouncedEvent::Rescan => {
                vec![]
            }
            // Consider Error as to log it
            DebouncedEvent::Error(err, path) => {
                eprintln!("Error {} on {:?}", err, path);
                vec![]
            }
        };

        for message in messages {
            match self.operational_sender.send(message) {
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
}

// Represent known local files. When trsync start, it use this index to compare
// with real local files state and produce change messages.
pub struct LocalSync {
    connection: Connection,
    path: PathBuf,
    operational_sender: Sender<OperationalMessage>,
}

impl LocalSync {
    pub fn new(
        connection: Connection,
        path: PathBuf,
        operational_sender: Sender<OperationalMessage>,
    ) -> Self {
        Self {
            connection,
            path: fs::canonicalize(&path).unwrap(),
            operational_sender,
        }
    }

    pub fn sync(&self) {
        // Look at disk files and compare to db
        self.sync_from_disk();
        // TODO : look ate db to search deleted files
        self.sync_from_db();
    }

    fn sync_from_disk(&self) {
        WalkDir::new(&self.path)
            .into_iter()
            .filter_entry(|e| !self.ignore_entry(e))
            .for_each(|x| self.sync_disk_file(&x.unwrap()))
    }

    fn ignore_entry(&self, entry: &DirEntry) -> bool {
        // TODO : patterns from config object
        // TODO : this is ugly code !!!
        let entry_path = entry.path();
        match entry_path.file_name() {
            Some(x) => format!("{}", x.to_str().unwrap()).starts_with("."),
            None => false,
        }
    }

    fn sync_disk_file(&self, entry: &DirEntry) {
        let relative_path = entry.path().strip_prefix(&self.path).unwrap();
        // TODO : prevent sync root with more clean way
        if relative_path == Path::new("") {
            return;
        }

        println!("sync {:?}", relative_path);

        let metadata = fs::metadata(self.path.join(relative_path)).unwrap();
        let disk_last_modified_timestamp = metadata
            .modified()
            .unwrap()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64; // TODO : type can contains this timestamp ?

        match self.connection.query_row::<u64, _, _>(
            "SELECT last_modified_timestamp FROM file WHERE relative_path = ?",
            params![relative_path.to_str()],
            |row| row.get(0),
        ) {
            Ok(last_modified_timestamp) => {
                // Known file (check if have been modified)
                println!("{}", last_modified_timestamp);
                if disk_last_modified_timestamp != last_modified_timestamp {
                    println!("Modified !");
                    self.operational_sender
                        .send(OperationalMessage::ModifiedLocalFile(String::from(
                            relative_path.to_str().unwrap(),
                        )))
                        .unwrap();
                }
            }
            Err(_) => {
                // Unknown file
                self.operational_sender
                    .send(OperationalMessage::NewLocalFile(String::from(
                        relative_path.to_str().unwrap(),
                    )))
                    .unwrap();
            }
        }
    }

    fn sync_from_db(&self) {
        let mut stmt = self
            .connection
            .prepare("SELECT relative_path FROM file")
            .unwrap();
        let local_iter = stmt.query_map([], |row| Ok(row.get(0).unwrap())).unwrap();
        for result in local_iter {
            let relative_path: String = result.unwrap();
            if !self.path.join(&relative_path).exists() {
                println!("deleted {:?}", relative_path);
                // TODO : This update must be done in Operation !
                self.connection
                    .execute(
                        "DELETE FROM file WHERE relative_path = ?1",
                        params![relative_path],
                    )
                    .unwrap();
                self.operational_sender
                    .send(OperationalMessage::DeletedLocalFile(relative_path))
                    .unwrap();
            }
        }
    }
}
