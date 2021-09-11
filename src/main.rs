use structopt::StructOpt;
extern crate notify;

use std::sync::mpsc::channel;
use std::thread;

use crate::database::Database;
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
    println!("Watch {:?}", opt.path);
    let (operational_sender, operational_receiver) = channel();

    // First, start local sync to know changes since last start
    let local_sync_handle = thread::spawn(move || {
        Database::new(database_file_path.to_string()).with_new_connection(|connection| {
            LocalSync::new(connection).sync();
        });
    });

    // Second, start remote sync to know remote changes since last run
    let remote_sync_handle = thread::spawn(move || {
        Database::new(database_file_path.to_string()).with_new_connection(|connection| {
            RemoteSync::new(connection).sync();
        });
    });

    // Start local watcher
    let mut local_watcher = LocalWatcher::new(operational_sender.clone());
    let local_handle = thread::spawn(move || local_watcher.listen(&opt.path));

    // Start remote watcher
    let mut remote_watcher = RemoteWatcher::new(operational_sender.clone());
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
