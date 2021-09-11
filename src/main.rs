use structopt::StructOpt;
extern crate notify;

use std::sync::mpsc::channel;
use std::thread;

use crate::database::{create_tables, Database};
use crate::local::{LocalSync, LocalWatcher};
use crate::operation::OperationalHandler;
use crate::remote::{RemoteSync, RemoteWatcher};

pub mod database;
pub mod error;
pub mod local;
pub mod operation;
pub mod remote;

#[derive(StructOpt, Debug)]
#[structopt(name = "basic")]
pub struct Opt {
    #[structopt(parse(from_os_str))]
    path: std::path::PathBuf,
}

fn main() {
    // TODO : Must have a local index with inode,modified and same for remote to compare when sync

    let database_file_path = ".trsync.db"; // TODO : file relative to folder to sync
    let opt = Opt::from_args();
    let path = opt.path.clone();
    println!("Watch {:?}", &opt.path);
    let (operational_sender, operational_receiver) = channel();

    // Initialize database if needed
    Database::new(database_file_path.to_string()).with_new_connection(|connection| {
        create_tables(connection);
    });

    // First, start local sync to know changes since last start
    let local_sync_operational_sender = operational_sender.clone();
    let local_sync_handle = thread::spawn(move || {
        Database::new(database_file_path.to_string()).with_new_connection(|connection| {
            LocalSync::new(connection, opt.path, local_sync_operational_sender).sync();
        });
    });

    // Second, start remote sync to know remote changes since last run
    let remote_sync_handle = thread::spawn(move || {
        Database::new(database_file_path.to_string()).with_new_connection(|connection| {
            RemoteSync::new(connection).sync();
        });
    });

    // Start local watcher
    let local_watcher_operational_sender = operational_sender.clone();
    let mut local_watcher = LocalWatcher::new(local_watcher_operational_sender);
    let local_handle = thread::spawn(move || local_watcher.listen(&path));

    // Start remote watcher
    let remote_watcher_operational_sender = operational_sender.clone();
    let mut remote_watcher = RemoteWatcher::new(remote_watcher_operational_sender);
    let remote_handle = thread::spawn(move || remote_watcher.listen());

    // Wait end of local and remote  sync
    local_sync_handle.join().unwrap();
    remote_sync_handle.join().unwrap();

    // Operational
    let operational_handle = thread::spawn(move || {
        Database::new(database_file_path.to_string()).with_new_connection(|connection| {
            OperationalHandler::new(connection).listen(operational_receiver);
        })
    });

    local_handle.join().unwrap();
    remote_handle.join().unwrap();
    operational_handle.join().unwrap();
}
