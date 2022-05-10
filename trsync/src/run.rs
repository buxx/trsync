extern crate notify;
use crate::client::ensure_availability;
use crate::context::Context;
use crate::database::{Database, DatabaseOperation};
use crate::error::Error;
use crate::local::{start_local_sync, start_local_watch};
use crate::message::MainMessage;
use crate::operation::start_operation;
use crate::operation::OperationalMessage;
use crate::remote::{start_remote_sync, start_remote_watch};
use crossbeam_channel::{unbounded, Receiver as CrossbeamReceiver, Sender as CrossbeamSender};
use log;
use std::fs;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;

pub fn run(context: Context, stop_signal: Arc<AtomicBool>) -> Result<(), Error> {
    // Digest input folder to watch
    log::info!("Prepare to sync {:?}", &context.folder_path);
    fs::create_dir_all(&context.folder_path)?;
    let exit_after_sync = context.exit_after_sync;

    // Initialize database if needed
    log::info!("Initialize index");
    Database::new(context.database_path.clone()).with_new_connection(|connection| {
        DatabaseOperation::new(&connection).create_tables()?;
        Ok(())
    })?;

    loop {
        let (operational_sender, operational_receiver): (
            Sender<OperationalMessage>,
            Receiver<OperationalMessage>,
        ) = channel();

        let restart_signal = Arc::new(AtomicBool::new(false));

        // Blocks until remote api successfully responded
        ensure_availability(&context)?;

        log::info!("Start synchronization");
        // First, start local sync to know local changes since last start
        let local_sync_handle = start_local_sync(&context, &operational_sender);
        // Second, start remote sync to know remote changes since last run
        let remote_sync_handle = start_remote_sync(&context, &operational_sender);

        log::info!("Start watchers");
        // Start local watcher
        let local_watch_handle =
            start_local_watch(&context, &operational_sender, &stop_signal, &restart_signal)?;
        // Start remote watcher
        let remote_watch_handle =
            start_remote_watch(&context, &operational_sender, &stop_signal, &restart_signal)?;

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

        // FIXME BS NOW : close channel ?

        // Operational
        let operational_handle = start_operation(
            &context,
            operational_receiver,
            &stop_signal,
            &restart_signal,
        );

        local_watch_handle
            .join()
            .expect("Fail to join local listener handler")?;
        remote_watch_handle
            .join()
            .expect("Fail to join remote listener handler")?;
        operational_handle
            .join()
            .expect("Fail to join operational handler")?;

        if stop_signal.load(Ordering::Relaxed) {
            break;
        }
    }

    Ok(())
}
