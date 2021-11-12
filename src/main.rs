use env_logger::Env;
use error::Error;
use structopt::StructOpt;
extern crate notify;
use log;

use std::sync::mpsc::channel;
use std::thread;

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

fn main() -> Result<(), Error> {
    // Initialize static things
    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();
    let opt = Opt::from_args();

    // Digest input folder to watch
    log::info!("Prepare to sync {:?}", &opt.path);
    let folder_path = util::canonicalize_to_string(&opt.path)?;

    // Ask password by input
    let password = rpassword::read_password_from_tty(Some("Tracim user password ? "))?;

    // Prepare context object
    let context = Context::new(
        !opt.no_ssl,
        opt.tracim_address,
        opt.username,
        password,
        folder_path,
        opt.workspace_id,
    )?;

    // Prepare main channel
    let (operational_sender, operational_receiver) = channel();

    // Initialize database if needed
    log::info!("Initialize index");
    Database::new(context.database_path.clone()).with_new_connection(|connection| {
        match DatabaseOperation::new(&connection).create_tables() {
            Ok(_) => {}
            // FIXME : implement stop application signal
            Err(error) => panic!("{:?}", error),
        }
    })?;

    log::info!("Start synchronization");

    // First, start local sync to know changes since last start
    let local_sync_operational_sender = operational_sender.clone();
    let local_sync_context = context.clone();
    let local_sync_handle = thread::spawn(move || {
        Database::new(local_sync_context.database_path.clone())
            .with_new_connection(|connection| {
                LocalSync::new(
                    connection,
                    local_sync_context.folder_path.clone(),
                    local_sync_operational_sender,
                )
                .expect("Fail to create local sync")
                .sync()
                .expect("Fail to local sync");
            })
            .expect("Fail to make database connection when start local sync");
    });

    // Second, start remote sync to know remote changes since last run
    let remote_sync_operational_sender = operational_sender.clone();
    let remote_sync_context = context.clone();
    let remote_sync_handle = thread::spawn(move || {
        Database::new(remote_sync_context.database_path.clone())
            .with_new_connection(|connection| {
                RemoteSync::new(
                    remote_sync_context.clone(),
                    connection,
                    remote_sync_operational_sender,
                )
                .expect("Fail to create remote sync")
                .sync()
                .expect("Fail to make remote sync");
            })
            .expect("Fail to make database connection when start remote sync");
    });

    log::info!("Start watchers");

    // Start local watcher
    let local_watcher_operational_sender = operational_sender.clone();
    let local_watcher_context = context.clone();
    let mut local_watcher = LocalWatcher::new(
        local_watcher_operational_sender,
        local_watcher_context.folder_path.clone(),
    )
    .expect("Fail to initialize LocalWatcher");
    let local_handle = thread::spawn(move || {
        local_watcher
            .listen(local_watcher_context.folder_path.clone())
            .expect("Fail to start listening with local watcher")
    });

    // Start remote watcher
    let remote_watcher_operational_sender = operational_sender.clone();
    let mut remote_watcher = RemoteWatcher::new(context.clone(), remote_watcher_operational_sender);
    let remote_handle = thread::spawn(move || {
        remote_watcher
            .listen()
            .expect("Fail to listen from remote watcher")
    });

    // Wait end of local and remote  sync
    log::info!("Wait synchronizations to finish their jobs");
    local_sync_handle
        .join()
        .expect("Fail to join local sync handler");
    remote_sync_handle
        .join()
        .expect("Fail to join remote sync handler");

    log::info!("Synchronization finished, start changes resolver");

    // Operational
    let operational_context = context.clone();
    let operational_handle = thread::spawn(move || {
        Database::new(context.database_path.clone())
            .with_new_connection(|connection| {
                OperationalHandler::new(operational_context, connection)
                    .expect("Fail to create operational handler")
                    .listen(operational_receiver);
            })
            .expect("Fail to make database connection when start operational handler")
    });

    local_handle
        .join()
        .expect("Fail to join local listener handler");
    remote_handle
        .join()
        .expect("Fail to join remote listener handler");
    operational_handle
        .join()
        .expect("Fail to join operational handler");

    log::info!("Exit application");
    Ok(())
}
