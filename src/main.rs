use env_logger::Env;
use error::Error;
use operation::{OperationalMessage, SupervisorMessage};
use structopt::StructOpt;
extern crate notify;
use log;

use std::sync::mpsc::{channel, Receiver, Sender};
use std::time::Instant;
use std::{env, fs, thread};

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

    #[structopt(name = "--no-ssl", short, long)]
    no_ssl: bool,

    #[structopt(name = "--env-var-pass", long, short)]
    env_var_pass: Option<String>,

    #[structopt(name = "--exit-after-sync", long)]
    exit_after_sync: bool,
}

fn local_sync(
    local_sync_context: Context,
    local_sync_operational_sender: Sender<OperationalMessage>,
) -> Result<(), Error> {
    Database::new(local_sync_context.database_path.clone()).with_new_connection(|connection| {
        LocalSync::new(
            connection,
            local_sync_context.folder_path.clone(),
            local_sync_operational_sender,
        )?
        .sync()?;
        Ok(())
    })?;

    Ok(())
}

fn remote_sync(
    remote_sync_context: Context,
    remote_sync_operational_sender: Sender<OperationalMessage>,
) -> Result<(), Error> {
    Database::new(remote_sync_context.database_path.clone()).with_new_connection(|connection| {
        RemoteSync::new(
            remote_sync_context,
            connection,
            remote_sync_operational_sender,
        )?
        .sync()?;
        Ok(())
    })?;

    Ok(())
}

pub fn supervisor(
    context: Context,
    operational_sender: Sender<OperationalMessage>,
    supervisor_receiver: Receiver<SupervisorMessage>,
) -> Result<(), Error> {
    let mut last_remote_activity: Instant = Instant::now();

    loop {
        match supervisor_receiver.recv() {
            Ok(SupervisorMessage::RemoteActivityReceived) => {
                // FIXME as param
                let no_activity_since = last_remote_activity.elapsed().as_secs();
                log::debug!(
                    "Remote activity received (no_activity_since : {})",
                    no_activity_since
                );
                last_remote_activity = Instant::now();

                if no_activity_since > 60 {
                    log::info!(
                        "Remote activity received after '{}' seconds, make a sync",
                        no_activity_since
                    );
                    match remote_sync(context.clone(), operational_sender.clone()) {
                        Err(error) => {
                            log::error!("Error when sync with remote : {:?}", error);
                        }
                        _ => {}
                    }
                }
            }
            Err(error) => {
                log::error!("Error when supervisor receive message : {}", error);
                break;
            }
        }
    }

    Ok(())
}

fn main() -> Result<(), Error> {
    // Initialize static things
    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();
    let opt = Opt::from_args();

    // Digest input folder to watch
    log::info!("Prepare to sync {:?}", &opt.path);
    fs::create_dir_all(&opt.path)?;
    let folder_path = util::canonicalize_to_string(&opt.path)?;

    // Ask password by input or get it from env var
    let password = if let Some(env_var_pass) = opt.env_var_pass {
        match env::var(&env_var_pass) {
            Ok(password) => password,
            Err(_) => {
                return Err(Error::UnexpectedError(format!(
                    "No en var set for name {}",
                    &env_var_pass
                )))
            }
        }
    } else {
        rpassword::read_password_from_tty(Some("Tracim user password ? "))?
    };

    // Prepare context object
    let context = Context::new(
        !opt.no_ssl,
        opt.tracim_address,
        opt.username,
        password,
        folder_path,
        opt.workspace_id,
        opt.exit_after_sync,
    )?;

    // Prepare main channel
    let (operational_sender, operational_receiver) = channel();
    let (supervisor_sender, supervisor_receiver) = channel();

    // Initialize database if needed
    log::info!("Initialize index");
    Database::new(context.database_path.clone()).with_new_connection(|connection| {
        DatabaseOperation::new(&connection).create_tables()?;
        Ok(())
    })?;

    log::info!("Start synchronization");

    // First, start local sync to know changes since last start
    let local_sync_operational_sender = operational_sender.clone();
    let local_sync_context = context.clone();
    let local_sync_handle =
        thread::spawn(move || local_sync(local_sync_context, local_sync_operational_sender));

    // Second, start remote sync to know remote changes since last run
    let remote_sync_operational_sender = operational_sender.clone();
    let remote_sync_context = context.clone();
    let remote_sync_handle =
        thread::spawn(move || remote_sync(remote_sync_context, remote_sync_operational_sender));

    log::info!("Start watchers");

    // Start local watcher
    let local_watcher_operational_sender = operational_sender.clone();
    let local_watcher_context = context.clone();
    let mut local_watcher = LocalWatcher::new(
        local_watcher_operational_sender,
        local_watcher_context.folder_path.clone(),
    )?;
    let local_handle = thread::spawn(move || {
        if !local_watcher_context.exit_after_sync {
            local_watcher.listen(local_watcher_context.folder_path.clone())
        } else {
            Ok(())
        }
    });

    // Start remote watcher
    let remote_watcher_operational_sender = operational_sender.clone();
    let remote_watcher_supervisor_sender = supervisor_sender.clone();
    let remote_watcher_context = context.clone();
    let mut remote_watcher = RemoteWatcher::new(
        context.clone(),
        remote_watcher_operational_sender,
        remote_watcher_supervisor_sender,
    );
    let remote_handle = thread::spawn(move || {
        if !remote_watcher_context.exit_after_sync {
            remote_watcher.listen()
        } else {
            Ok(())
        }
    });

    // Start supervisor
    let supervisor_context = context.clone();
    let supervisor_operational_sender = operational_sender.clone();
    let supervisor_handle = thread::spawn(move || {
        supervisor(
            supervisor_context,
            supervisor_operational_sender,
            supervisor_receiver,
        )
    });

    // FIXME BS NOW : il faut check si il y a une erreur quelque soit le thread qui plante ne premier !
    // Wait end of local and remote  sync
    log::info!("Wait synchronizations to finish their jobs");
    let local_sync_result = local_sync_handle
        .join()
        .expect("Fail to join local sync handler");
    let remote_sync_result = remote_sync_handle
        .join()
        .expect("Fail to join remote sync handler");

    if let Err(error) = &local_sync_result {
        log::error!("Local sync failed: {:?}", error);
    }
    if let Err(error) = &remote_sync_result {
        log::error!("Remote sync failed: {:?}", error);
    }
    if local_sync_result.is_err() || remote_sync_result.is_err() {
        return Err(Error::StartupError(format!(
            "Synchronization fail, interrupt now"
        )));
    }

    if context.exit_after_sync {
        log::info!("Synchronization finished");
        operational_sender.send(OperationalMessage::Exit).unwrap();
    } else {
        log::info!("Synchronization finished, start changes resolver");
    }

    // Operational
    let operational_context = context.clone();
    let operational_handle = thread::spawn(move || {
        Database::new(context.database_path.clone()).with_new_connection(|connection| {
            OperationalHandler::new(operational_context, connection)?.listen(operational_receiver);
            Ok(())
        })
    });

    local_handle
        .join()
        .expect("Fail to join local listener handler")?;
    remote_handle
        .join()
        .expect("Fail to join remote listener handler")?;
    operational_handle
        .join()
        .expect("Fail to join operational handler")?;

    log::info!("Exit application");
    Ok(())
}
