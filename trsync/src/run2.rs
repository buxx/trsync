extern crate notify;
use crate::context::Context as TrSyncContext;
use crate::database::connection;
use crate::event::remote::RemoteEvent;
use crate::event::Event;
use crate::local::{DiskEvent, LocalWatcher};
use crate::local2::reducer::LocalReceiverReducer;
use crate::operation2::executor::ExecutorError;
use crate::operation2::operator::Operator;
use crate::remote::RemoteWatcher;
use crate::state::disk::DiskState;
use crate::state::State;
use crate::sync::local::LocalSync;
use crate::sync::remote::RemoteSync;
use crate::sync::{ResolveMethod, StartupSyncResolver};
use anyhow::{bail, Context, Result};
use crossbeam_channel::{unbounded, Receiver, RecvTimeoutError, Sender};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use std::{fs, thread};
use trsync_core::client::{Tracim, TracimClient};
use trsync_core::error::{Decision, ErrorChannels};
use trsync_core::job::{Job, JobIdentifier};
use trsync_core::local::LocalChange;
use trsync_core::remote::RemoteChange;
use trsync_core::sync::SyncPolitic;

struct Runner {
    context: TrSyncContext,
    stop_signal: Arc<AtomicBool>,
    restart_signal: Arc<AtomicBool>,
    activity_sender: Option<Sender<Job>>,
    operational_sender: Sender<Event>,
    operational_receiver: Receiver<Event>,
    remote_sender: Sender<RemoteEvent>,
    remote_receiver: Receiver<RemoteEvent>,
    local_sender: Sender<DiskEvent>,
    local_receiver_reducer: LocalReceiverReducer,
    sync_politic: Box<dyn SyncPolitic>,
    error_channels: ErrorChannels,
}

impl Runner {
    fn new(
        context: TrSyncContext,
        stop_signal: Arc<AtomicBool>,
        activity_sender: Option<Sender<Job>>,
        sync_politic: Box<dyn SyncPolitic>,
        error_channels: ErrorChannels,
    ) -> Self {
        let restart_signal = Arc::new(AtomicBool::new(false));
        let (operational_sender, operational_receiver): (Sender<Event>, Receiver<Event>) =
            unbounded();
        let (remote_sender, remote_receiver): (Sender<RemoteEvent>, Receiver<RemoteEvent>) =
            unbounded();
        let (local_sender, local_receiver): (Sender<DiskEvent>, Receiver<DiskEvent>) = unbounded();
        let local_receiver_reducer = LocalReceiverReducer::new(local_receiver);

        Self {
            context,
            stop_signal,
            restart_signal,
            activity_sender,
            operational_sender,
            operational_receiver,
            remote_sender,
            remote_receiver,
            local_sender,
            local_receiver_reducer,
            sync_politic,
            error_channels,
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

    fn watchers(&self) -> Result<()> {
        self.remote_watcher()?;
        self.local_watcher()?;
        Ok(())
    }

    fn remote_watcher(&self) -> Result<()> {
        let remote_watcher_context = self.context.clone();
        let remote_watcher_stop_signal = self.stop_signal.clone();
        let remote_watcher_restart_signal = self.restart_signal.clone();
        let remote_watcher_operational_sender = self.remote_sender.clone();
        let remote_watcher_connection = connection(&PathBuf::from(&self.context.folder_path))?;

        thread::spawn(move || {
            let mut remote_watcher = RemoteWatcher::new(
                remote_watcher_connection,
                remote_watcher_context,
                remote_watcher_stop_signal,
                remote_watcher_restart_signal,
                remote_watcher_operational_sender,
            );
            if let Err(error) = remote_watcher.listen() {
                log::error!("{}", error);
                // FIXME BS : stop_signal ? restart_signal ?
            }
        });

        Ok(())
    }

    fn local_watcher(&self) -> Result<()> {
        let local_watcher_context = self.context.clone();
        let local_watcher_operational_sender = self.local_sender.clone();
        let local_watcher_stop_signal = self.stop_signal.clone();
        let local_watcher_restart_signal = self.restart_signal.clone();

        let mut local_watcher = LocalWatcher::new(
            local_watcher_context,
            local_watcher_stop_signal,
            local_watcher_restart_signal,
            local_watcher_operational_sender,
        )?;

        thread::spawn(move || {
            if let Err(error) = local_watcher.listen() {
                log::error!("{}", error);
                // FIXME BS : stop_signal ? restart_signal ?
            }
        });
        Ok(())
    }

    fn signal_job_start(&self) -> Result<()> {
        if let Some(activity_sender) = &self.activity_sender {
            log::info!(
                "[{}::{}] Start job",
                self.context.instance_name,
                self.context.workspace_id,
            );
            if let Err(error) = activity_sender.send(Job::Begin(JobIdentifier::new(
                self.context.instance_name.clone(),
                self.context.workspace_id.0,
                self.context.workspace_name.clone(),
            ))) {
                log::error!(
                    "[{}::{}] Error when sending activity begin : {:?}",
                    self.context.instance_name,
                    self.context.workspace_id,
                    error
                );
            }
        }
        Ok(())
    }

    fn signal_job_end(&self) -> Result<()> {
        if let Some(activity_sender) = &self.activity_sender {
            log::info!(
                "[{}::{}] Ended job",
                self.context.instance_name,
                self.context.workspace_id,
            );
            if let Err(error) = activity_sender.send(Job::End(JobIdentifier::new(
                self.context.instance_name.clone(),
                self.context.workspace_id.0,
                self.context.workspace_name.clone(),
            ))) {
                log::error!(
                    "[{}::{}] Error when sending activity end : {:?}",
                    self.context.instance_name,
                    self.context.workspace_id,
                    error
                );
            }
        }
        Ok(())
    }

    fn sync(&self, operator: &mut Operator) -> Result<()> {
        self.signal_job_start()?;
        if let Err(error) = self.sync_(operator) {
            self.signal_job_end()?;
            return Err(error);
        }
        self.signal_job_end()?;
        Ok(())
    }

    fn sync_(&self, operator: &mut Operator) -> Result<()> {
        let remote_changes = self.remote_changes()?;
        let local_changes = self.local_changes()?;
        let (remote_changes, local_changes) =
            StartupSyncResolver::new(remote_changes, local_changes, ResolveMethod::ForceLocal)
                .resolve()?;

        if (!remote_changes.is_empty() || !local_changes.is_empty())
            && !self
                .sync_politic
                .deal(remote_changes.clone(), local_changes.clone())?
        {
            bail!("TODO")
        }

        // FIXME BS NOW : move this code in separate struct and unit test it
        let mut try_remote_changes_len = remote_changes.len();
        let mut try_remote_changes = remote_changes.clone();
        loop {
            let mut remaining_remote_changes = vec![];
            for remote_change in &try_remote_changes.clone() {
                match operator.operate(&Event::from(remote_change)) {
                    Ok(_) => {}
                    Err(ExecutorError::MissingParent(_, _)) => {
                        remaining_remote_changes.push(remote_change.clone())
                    }
                    Err(err) => bail!("Error on operate on remote event : {:#}", err),
                };
            }

            // No retry needed, don't retry
            if remaining_remote_changes.is_empty() {
                break;
            // Retried but nothing changed, stop all
            } else if remaining_remote_changes.len() == try_remote_changes_len {
                let detail: Vec<String> = try_remote_changes
                    .iter()
                    .map(|event| event.to_string())
                    .collect();
                bail!(
                    "Unable to operate on following remote changes (missing parents): {}",
                    detail.join(", ")
                );
            }
            try_remote_changes = remaining_remote_changes.clone();
            try_remote_changes_len = try_remote_changes.len();
        }

        let mut try_local_changes_len = local_changes.len();
        let mut try_local_changes = local_changes.clone();
        loop {
            let mut remaining_local_changes = vec![];
            for local_change in &try_local_changes.clone() {
                match operator.operate(&Event::from(local_change)) {
                    Ok(_) => {}
                    Err(ExecutorError::MissingParent(_, _)) => {
                        remaining_local_changes.push(local_change.clone())
                    }
                    Err(err) => bail!("Error operate on local event : {:#}", err),
                };
            }

            // No retry needed, don't retry
            if remaining_local_changes.is_empty() {
                break;
            // Retried but nothing changed, stop all
            } else if remaining_local_changes.len() == try_local_changes_len {
                let detail: Vec<String> = try_local_changes
                    .iter()
                    .map(|event| event.to_string())
                    .collect();
                bail!(
                    "Unable to operate on following local changes (missing parents): {}",
                    detail.join(", ")
                );
            }
            try_local_changes = remaining_local_changes.clone();
            try_local_changes_len = try_local_changes.len();
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

    fn listen(&self) -> Result<()> {
        self.listen_remote()?;
        self.listen_local()?;
        Ok(())
    }

    fn listen_remote(&self) -> Result<()> {
        let operational_sender = self.operational_sender.clone();
        let remote_receiver = self.remote_receiver.clone();

        thread::spawn(move || {
            while let Ok(remote_event) = remote_receiver.recv() {
                if operational_sender
                    .send(Event::Remote(remote_event))
                    .is_err()
                {
                    log::info!("Terminate remote listener");
                }
            }
        });

        Ok(())
    }

    fn listen_local(&self) -> Result<()> {
        let operational_sender = self.operational_sender.clone();
        let mut local_receiver_reducer = self.local_receiver_reducer.clone();

        thread::spawn(move || {
            while let Ok(disk_event) = local_receiver_reducer.recv() {
                if operational_sender.send(Event::Local(disk_event)).is_err() {
                    log::info!("Terminate locate listener");
                }
            }
        });

        Ok(())
    }

    fn is_stop_requested(&self) -> bool {
        self.stop_signal.load(Ordering::Relaxed)
    }

    fn is_restart_requested(&self) -> bool {
        self.restart_signal.load(Ordering::Relaxed)
    }

    fn operate(&self, operator: &mut Operator) -> Result<()> {
        let client: Box<dyn TracimClient> = Box::new(self.client()?);

        loop {
            match self
                .operational_receiver
                .recv_timeout(Duration::from_millis(150))
            {
                Err(RecvTimeoutError::Timeout) => {
                    if self.is_stop_requested() {
                        log::info!(
                            "[{}::{}] Finished operational (on stop signal)",
                            self.context.instance_name,
                            self.context.workspace_id,
                        );
                        break;
                    }
                    if self.is_restart_requested() {
                        log::info!(
                            "[{}::{}] Finished operational (on restart signal)",
                            self.context.instance_name,
                            self.context.workspace_id,
                        );
                        break;
                    }
                }
                Err(RecvTimeoutError::Disconnected) => {
                    log::error!(
                        "[{}::{}] Finished operational (on channel closed)",
                        self.context.instance_name,
                        self.context.workspace_id,
                    );
                    break;
                }
                Ok(event) => {
                    if self.is_stop_requested() {
                        log::info!(
                            "[{}::{}] Finished operational (on stop signal)",
                            self.context.instance_name,
                            self.context.workspace_id,
                        );
                        break;
                    }
                    if self.is_restart_requested() {
                        log::info!(
                            "[{}::{}] Finished operational (on restart signal)",
                            self.context.instance_name,
                            self.context.workspace_id,
                        );
                        break;
                    }

                    log::info!("Proceed event {:?}", &event);
                    let context_message = format!("Operate on event '{}'", event.display(&client));
                    self.signal_job_start()?;
                    operator.operate(&event).context(context_message)?;
                    self.signal_job_end()?;
                }
            }
        }

        log::info!("Terminate operational listener");
        Ok(())
    }

    fn state(&self) -> Result<Box<dyn State>> {
        let workspace_path = PathBuf::from(&self.context.folder_path);
        Ok(Box::new(DiskState::new(
            connection(&workspace_path).context(format!(
                "Create connection for startup sync for {}",
                workspace_path.display()
            ))?,
            workspace_path.clone(),
        )))
    }

    fn client(&self) -> Result<Tracim> {
        self.context
            .client()
            .context("Create tracim client for startup sync")
    }

    pub fn run(&mut self) -> Result<()> {
        self.ensure_folders()?;
        self.ensure_db()?;

        let mut state = self.state()?;
        let mut operator = Operator::new(
            &mut state,
            PathBuf::from(&self.context.folder_path),
            Box::new(self.client()?),
        );

        self.watchers()?;
        self.sync(&mut operator)?;

        if self.context.exit_after_sync {
            return Ok(());
        }

        self.listen()?;
        self.operate(&mut operator)?;

        Ok(())
    }
}

pub fn run(
    context: TrSyncContext,
    stop_signal: Arc<AtomicBool>,
    activity_sender: Option<Sender<Job>>,
    sync_politic: Box<dyn SyncPolitic>,
    error_channels: ErrorChannels,
) -> Result<()> {
    let mut runner = Runner::new(
        context,
        stop_signal.clone(),
        activity_sender,
        sync_politic,
        error_channels.clone(),
    );
    loop {
        if let Err(error) = runner.run() {
            log::error!("Operate error : {:#}", &error);
            *error_channels.error().lock().unwrap() = Some(format!("{:#}", error));
            match error_channels.decision_receiver().recv() {
                Ok(Decision::RestartSpaceSync) => {}
                Err(_) => {
                    log::error!("Unable to communicate from trsync run to error decision receiver");
                    break;
                }
            }
        }
        if stop_signal.load(Ordering::Relaxed) {
            stop_signal.swap(false, Ordering::Relaxed);
            break;
        }
    }

    Ok(())
}
