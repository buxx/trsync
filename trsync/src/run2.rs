extern crate notify;
use crate::context::Context as TrSyncContext;
use crate::database::connection;
use crate::operation::Job;
use crate::operation2::operator::Operator;
use crate::state::disk::DiskState;
use crate::state::State;
use crate::sync::local::{LocalChange, LocalSync};
use crate::sync::remote::{RemoteChange, RemoteSync};
use crate::sync::{ResolveMethod, StartupSyncResolver};
use anyhow::{Context, Result};
use crossbeam_channel::Sender as CrossbeamSender;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

struct Runner {
    context: TrSyncContext,
    stop_signal: Arc<AtomicBool>,
    activity_sender: Option<CrossbeamSender<Job>>,
}

impl Runner {
    fn new(
        context: TrSyncContext,
        stop_signal: Arc<AtomicBool>,
        activity_sender: Option<CrossbeamSender<Job>>,
    ) -> Self {
        Self {
            context,
            stop_signal,
            activity_sender,
        }
    }

    fn ensure_folders(&self) -> Result<()> {
        fs::create_dir_all(&self.context.folder_path)?;
        Ok(())
    }

    fn ensure_db(&self) -> Result<()> {
        let workspace_path = PathBuf::from(&self.context.folder_path);
        DiskState::new(connection(&workspace_path)?, workspace_path.clone()).create_tables()?;
        Ok(())
    }

    fn sync(&self) -> Result<()> {
        let workspace_path = PathBuf::from(&self.context.folder_path);
        let remote_changes = self.remote_changes()?;
        let local_changes = self.local_changes()?;
        let (remote_changes, local_changes) =
            StartupSyncResolver::new(remote_changes, local_changes, ResolveMethod::ForceLocal)
                .resolve()?;

        // TODO : apply changes

        dbg!(&remote_changes);
        dbg!(&local_changes);

        let client = self
            .context
            .client()
            .context("Create tracim client for startup sync")?;
        let mut state: Box<dyn State> = Box::new(DiskState::new(
            connection(&workspace_path).context(format!(
                "Create connection for startup sync for {}",
                workspace_path.display()
            ))?,
            workspace_path.clone(),
        ));
        let mut operator = Operator::new(&mut state, &workspace_path, Box::new(client));

        for remote_change in remote_changes {
            let event_display = format!("{:?}", &remote_change);
            operator
                .operate(remote_change.into())
                .context(format!("Operate on remote change {:?}", event_display))?
        }

        Ok(())
    }

    fn remote_changes(&self) -> Result<Vec<RemoteChange>> {
        let workspace_path = PathBuf::from(&self.context.folder_path);
        RemoteSync::new(
            connection(&workspace_path)?,
            Box::new(self.context.client().context("Create Tracim client")?),
        )
        .changes()
        .context("Determine remote changes")
    }

    fn local_changes(&self) -> Result<Vec<LocalChange>> {
        let workspace_path = PathBuf::from(&self.context.folder_path);
        LocalSync::new(connection(&workspace_path)?, workspace_path.clone())
            .changes()
            .context("Determine local changes")
    }

    pub fn run(&mut self) -> Result<()> {
        self.ensure_folders()?;
        self.ensure_db()?;

        // TODO : Start listening remote TLM
        // TODO : Start listening local changes
        self.sync()?;

        Ok(())
    }
}

pub fn run(
    context: TrSyncContext,
    stop_signal: Arc<AtomicBool>,
    activity_sender: Option<CrossbeamSender<Job>>,
) -> Result<()> {
    let mut runner = Runner::new(context, stop_signal, activity_sender);
    runner.run().context("Run")?;

    Ok(())
}
