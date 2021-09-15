use structopt::StructOpt;
extern crate notify;

use std::sync::mpsc::channel;
use std::thread;

use crate::client::Client;
use crate::database::{create_tables, Database};
use crate::local::{LocalSync, LocalWatcher};
use crate::operation::OperationalHandler;
use crate::remote::{RemoteSync, RemoteWatcher};

pub mod client;
pub mod database;
pub mod error;
pub mod local;
pub mod operation;
pub mod remote;
pub mod types;
pub mod util;

#[derive(StructOpt, Debug)]
#[structopt(name = "basic")]
pub struct Opt {
    #[structopt(parse(from_os_str))]
    path: std::path::PathBuf,

    #[structopt(name = "tracim_api_key")]
    tracim_api_key: String,

    #[structopt(name = "tracim_user_name")]
    tracim_user_name: String,
}

fn main() {
    // TODO : Must have a local index with inode,modified and same for remote to compare when sync

    let database_file_path = ".trsync.db"; // TODO : file relative to folder to sync
    let opt = Opt::from_args();
    println!("Watch {:?}", &opt.path);
    let (operational_sender, operational_receiver) = channel();

    // Initialize database if needed
    Database::new(database_file_path.to_string()).with_new_connection(|connection| {
        create_tables(connection);
    });

    // First, start local sync to know changes since last start
    let local_sync_operational_sender = operational_sender.clone();
    let local_sync_path = opt.path.clone();
    let local_sync_handle = thread::spawn(move || {
        Database::new(database_file_path.to_string()).with_new_connection(|connection| {
            LocalSync::new(connection, local_sync_path, local_sync_operational_sender).sync();
        });
    });

    // Second, start remote sync to know remote changes since last run
    let remote_sync_operational_sender = operational_sender.clone();
    let remote_sync_path = opt.path.clone();
    let tracim_api_key = opt.tracim_api_key.clone();
    let tracim_user_name = opt.tracim_user_name.clone();
    let remote_sync_handle = thread::spawn(move || {
        Database::new(database_file_path.to_string()).with_new_connection(|connection| {
            RemoteSync::new(
                connection,
                remote_sync_path,
                remote_sync_operational_sender,
                tracim_api_key,
                tracim_user_name,
            )
            .sync();
        });
    });

    // Start local watcher
    let local_watcher_operational_sender = operational_sender.clone();
    let local_watcher_path = opt.path.clone();
    let mut local_watcher =
        LocalWatcher::new(local_watcher_operational_sender, local_watcher_path.clone());
    let local_handle = thread::spawn(move || local_watcher.listen(&local_watcher_path));

    // Start remote watcher
    let remote_watcher_operational_sender = operational_sender.clone();
    let tracim_api_key = opt.tracim_api_key.clone();
    let tracim_user_name = opt.tracim_user_name.clone();
    let mut remote_watcher = RemoteWatcher::new(
        remote_watcher_operational_sender,
        tracim_api_key,
        tracim_user_name,
    );
    let remote_handle = thread::spawn(move || remote_watcher.listen());

    // Wait end of local and remote  sync
    local_sync_handle.join().unwrap();
    remote_sync_handle.join().unwrap();

    // Operational
    let tracim_api_key = opt.tracim_api_key.clone();
    let tracim_user_name = opt.tracim_user_name.clone();
    let operational_path = opt.path.clone();
    let operational_handle = thread::spawn(move || {
        Database::new(database_file_path.to_string()).with_new_connection(|connection| {
            OperationalHandler::new(
                connection,
                Client::new(tracim_api_key, tracim_user_name),
                operational_path,
            )
            .listen(operational_receiver);
        })
    });

    local_handle.join().unwrap();
    remote_handle.join().unwrap();
    operational_handle.join().unwrap();
}
