use structopt::StructOpt;
extern crate notify;

use std::sync::mpsc::channel;
use std::{fs, thread};

use crate::context::Context;
use crate::database::{Database, DatabaseOperation};
use crate::local::{LocalSync, LocalWatcher};
use crate::operation::OperationalHandler;
use crate::remote::{RemoteSync, RemoteWatcher};

pub mod client;
pub mod context;
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

    #[structopt(name = "tracim_address")]
    tracim_address: String,

    #[structopt(name = "workspace_id")]
    workspace_id: i32,

    #[structopt(name = "username")]
    username: String,

    #[structopt(short, long)]
    no_ssl: bool,
}

fn main() {
    let opt = Opt::from_args();
    let folder_path = fs::canonicalize(&opt.path)
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
    let context = Context::new(
        !opt.no_ssl,
        opt.tracim_address,
        opt.username,
        rpassword::read_password_from_tty(Some("Tracim user password ? ")).unwrap(),
        folder_path,
        opt.workspace_id,
    );

    println!("Watch {:?}", &opt.path);
    let (operational_sender, operational_receiver) = channel();

    // Initialize database if needed
    Database::new(context.database_path.clone()).with_new_connection(|connection| {
        match DatabaseOperation::new(&connection).create_tables() {
            Ok(_) => {}
            Err(error) => panic!("{:?}", error),
        }
    });

    // First, start local sync to know changes since last start
    let local_sync_operational_sender = operational_sender.clone();
    let local_sync_context = context.clone();
    let local_sync_handle = thread::spawn(move || {
        Database::new(local_sync_context.database_path.clone()).with_new_connection(|connection| {
            LocalSync::new(
                connection,
                local_sync_context.folder_path.clone(),
                local_sync_operational_sender,
            )
            .sync();
        });
    });

    // Second, start remote sync to know remote changes since last run
    let remote_sync_operational_sender = operational_sender.clone();
    let remote_sync_context = context.clone();
    let remote_sync_handle = thread::spawn(move || {
        Database::new(remote_sync_context.database_path.clone()).with_new_connection(
            |connection| {
                RemoteSync::new(
                    remote_sync_context.clone(),
                    connection,
                    remote_sync_operational_sender,
                )
                .sync();
            },
        );
    });

    // Start local watcher
    let local_watcher_operational_sender = operational_sender.clone();
    let local_watcher_context = context.clone();
    let mut local_watcher = LocalWatcher::new(
        local_watcher_operational_sender,
        local_watcher_context.folder_path.clone(),
    );
    let local_handle =
        thread::spawn(move || local_watcher.listen(local_watcher_context.folder_path.clone()));

    // Start remote watcher
    let remote_watcher_operational_sender = operational_sender.clone();
    let mut remote_watcher = RemoteWatcher::new(context.clone(), remote_watcher_operational_sender);
    let remote_handle = thread::spawn(move || remote_watcher.listen());

    // Wait end of local and remote  sync
    local_sync_handle.join().unwrap();
    remote_sync_handle.join().unwrap();

    // Operational
    let operational_context = context.clone();
    let operational_handle = thread::spawn(move || {
        Database::new(context.database_path.clone()).with_new_connection(|connection| {
            OperationalHandler::new(operational_context, connection).listen(operational_receiver);
        })
    });

    local_handle.join().unwrap();
    remote_handle.join().unwrap();
    operational_handle.join().unwrap();
}
