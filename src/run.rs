extern crate notify;
use crate::client::ensure_availability;
use crate::context::Context;
use crate::database::{Database, DatabaseOperation};
use crate::error::Error;
use crate::local::{start_local_sync, start_local_watch};
use crate::operation::start_operation;
use crate::operation::OperationalMessage;
use crate::remote::{start_remote_sync, start_remote_watch};
use crate::{util, MainMessage, Opt};
use crossbeam_channel::{unbounded, Receiver as CrossbeamReceiver, Sender as CrossbeamSender};
use log;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::{env, fs};

pub fn start(opt: Opt) -> Result<(), Error> {
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
        rpassword::prompt_password("Tracim user password ? ")?
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
    let exit_after_sync = context.exit_after_sync;

    // Initialize database if needed
    log::info!("Initialize index");
    Database::new(context.database_path.clone()).with_new_connection(|connection| {
        DatabaseOperation::new(&connection).create_tables()?;
        Ok(())
    })?;

    loop {
        // Main channel used for communication between threads, like interruption
        let (main_sender, main_receiver): (
            CrossbeamSender<MainMessage>,
            CrossbeamReceiver<MainMessage>,
        ) = unbounded();
        let (operational_sender, operational_receiver): (
            Sender<OperationalMessage>,
            Receiver<OperationalMessage>,
        ) = channel();

        // Blocks until remote api successfully responded
        ensure_availability(&context)?;

        log::info!("Start synchronization");
        // First, start local sync to know local changes since last start
        let local_sync_handle = start_local_sync(&context, &operational_sender);
        // Second, start remote sync to know remote changes since last run
        let remote_sync_handle = start_remote_sync(&context, &operational_sender);

        log::info!("Start watchers");
        // Start local watcher
        let local_watch_handle = start_local_watch(&context, &operational_sender, &main_receiver)?;
        // Start remote watcher
        let remote_watch_handle = start_remote_watch(&context, &operational_sender, &main_sender)?;

        // Wait end of local and remote sync
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

        if exit_after_sync {
            log::info!("Synchronization finished");
            operational_sender
                .send(OperationalMessage::Exit)
                .expect("Fail to send exit message");
        } else {
            log::info!("Synchronization finished, start changes resolver");
        }

        // Operational
        let operational_handle = start_operation(&context, operational_receiver, &main_receiver);

        local_watch_handle
            .join()
            .expect("Fail to join local listener handler")?;
        remote_watch_handle
            .join()
            .expect("Fail to join remote listener handler")?;
        operational_handle
            .join()
            .expect("Fail to join operational handler")?;

        if false {
            break;
        }
    }

    log::info!("Exit application");
    Ok(())
}
