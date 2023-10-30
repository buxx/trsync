extern crate notify;
use crate::context::Context as TrSyncContext;
use crate::database::connection;
use crate::operation::Job;
use crate::state::disk::DiskState;
use crate::sync::local::LocalSync;
use crate::sync::remote::RemoteSync;
use crate::sync::{ResolveMethod, StartupSyncResolver};
use anyhow::{Context, Result};
use crossbeam_channel::Sender as CrossbeamSender;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use trsync_core::client::TracimClient;

pub fn run(
    context: TrSyncContext,
    stop_signal: Arc<AtomicBool>,
    activity_sender: Option<CrossbeamSender<Job>>,
) -> Result<()> {
    fs::create_dir_all(&context.folder_path)?;
    let exit_after_sync = context.exit_after_sync;
    let workspace_path = PathBuf::from(context.database_path);
    let state = DiskState::new(connection(workspace_path.clone())?, workspace_path.clone());
    state.create_tables()?;

    // TODO : Start listening remote TLM

    let remote_changes =
        RemoteSync::new(connection(workspace_path.clone())?, Box::new(Tracim::new()))
            .changes()
            .context("Determine remote changes")?;
    let local_changes = LocalSync::new(connection(workspace_path.clone())?, workspace_path.clone())
        .changes()
        .context("Determine local changes")?;
    let (remote_changes, local_changes) =
        StartupSyncResolver::new(remote_changes, local_changes, ResolveMethod::ForceLocal)
            .resolve()?;

    // TODO : apply changes
    // TODO :

    Ok(())
}
