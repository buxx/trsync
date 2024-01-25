extern crate notify;
use crate::context::Context as TrSyncContext;
use crate::database::{connection, db_path};
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
use trsync_core::activity::{Activity, WrappedActivity};
use trsync_core::change::local::LocalChange;
use trsync_core::change::remote::RemoteChange;
use trsync_core::change::Change;
use trsync_core::client::{Tracim, TracimClient};
use trsync_core::control::RemoteControl;
use trsync_core::error::Decision;

struct Runner {
    context: TrSyncContext,
    remote_control: RemoteControl,
    restart_signal: Arc<AtomicBool>,
    operational_sender: Sender<Event>,
    operational_receiver: Receiver<Event>,
    remote_sender: Sender<RemoteEvent>,
    remote_receiver: Receiver<RemoteEvent>,
    local_sender: Sender<DiskEvent>,
    local_receiver_reducer: LocalReceiverReducer,
}

impl Runner {
    fn new(context: TrSyncContext, remote_control: RemoteControl) -> Self {
        let restart_signal = Arc::new(AtomicBool::new(false));
        let (operational_sender, operational_receiver): (Sender<Event>, Receiver<Event>) =
            unbounded();
        let (remote_sender, remote_receiver): (Sender<RemoteEvent>, Receiver<RemoteEvent>) =
            unbounded();
        let (local_sender, local_receiver): (Sender<DiskEvent>, Receiver<DiskEvent>) = unbounded();
        let local_receiver_reducer = LocalReceiverReducer::new(local_receiver);

        Self {
            context,
            remote_control,
            restart_signal,
            operational_sender,
            operational_receiver,
            remote_sender,
            remote_receiver,
            local_sender,
            local_receiver_reducer,
        }
    }

    fn ensure_folders(&self) -> Result<()> {
        fs::create_dir_all(&self.context.folder_path)?;
        Ok(())
    }

    fn ensure_db(&mut self) -> Result<()> {
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
        let remote_watcher_stop_signal = self.remote_control.stop_signal().clone();
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
        let local_watcher_stop_signal = self.remote_control.stop_signal().clone();
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

    fn set_activity(&self, activity: Activity) -> Result<()> {
        if let Some(activity_sender) = self.remote_control.activity_sender() {
            log::info!(
                "[{}::{}] Set activity to {}",
                self.context.instance_name,
                self.context.workspace_id,
                activity,
            );
            if let Err(error) = activity_sender.send(WrappedActivity::new(
                self.context.job_identifier(),
                activity,
            )) {
                bail!(format!(
                    "[{}::{}] Error when sending activity end : {:?}",
                    self.context.instance_name, self.context.workspace_id, error
                ));
            }
        };

        Ok(())
    }

    fn sync(&self, operator: &mut Operator) -> Result<()> {
        self.set_activity(Activity::StartupSync(None))?;
        if let Err(error) = self.sync_(operator) {
            self.set_activity(Activity::Idle)?;
            return Err(error);
        }
        self.set_activity(Activity::Idle)?;
        Ok(())
    }

    fn sync_(&self, operator: &mut Operator) -> Result<()> {
        let remote_changes = self.remote_changes()?;
        let local_changes = self.local_changes()?;
        let (remote_changes, local_changes) =
            StartupSyncResolver::new(remote_changes, local_changes, ResolveMethod::ForceLocal)
                .resolve()?;

        if !remote_changes.is_empty() || !local_changes.is_empty() {
            self.set_activity(Activity::WaitingStartupSyncConfirmation)?;
            if !self
                .remote_control
                .sync_politic()?
                .deal(remote_changes.clone(), local_changes.clone())?
            {
                bail!("TODO")
            }
            self.set_activity(Activity::Idle)?;
        }

        let remote_changes = remote_changes
            .iter()
            .map(|remote_change| remote_change.into())
            .collect();
        OperateChanges::new(
            self.context.clone(),
            self.remote_control.activity_sender().cloned(),
            remote_changes,
        )
        .operate(operator)?;

        let local_changes = local_changes
            .iter()
            .map(|local_change| local_change.into())
            .collect();
        OperateChanges::new(
            self.context.clone(),
            self.remote_control.activity_sender().cloned(),
            local_changes,
        )
        .operate(operator)?;

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
        self.remote_control.stop_signal().load(Ordering::Relaxed)
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
                    let event_display = event.display(client.as_ref());
                    let context_message = format!("Operate on event '{}'", &event_display);
                    self.set_activity(Activity::Job(event_display.to_string()))?;
                    operator.operate(&event).context(context_message)?;
                    self.set_activity(Activity::Idle)?;
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
        let is_first_sync = !db_path(&PathBuf::from(&self.context.folder_path)).exists();
        self.ensure_folders()?;
        self.ensure_db()?;

        let mut state = self.state()?;
        let mut operator = Operator::new(
            &mut state,
            PathBuf::from(&self.context.folder_path),
            Box::new(self.client()?),
        )
        .avoid_same_sums(is_first_sync);

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

struct OperateChanges {
    context: TrSyncContext,
    activity_sender: Option<Sender<WrappedActivity>>,
    changes: Vec<Change>,
}

impl OperateChanges {
    fn new(
        context: TrSyncContext,
        activity_sender: Option<Sender<WrappedActivity>>,
        changes: Vec<Change>,
    ) -> Self {
        Self {
            context,
            activity_sender,
            changes,
        }
    }

    fn operate(&mut self, operator: &mut Operator) -> Result<()> {
        loop {
            let mut remaining_changes = vec![];
            for change in &self.changes {
                self.set_activity(Activity::StartupSync(Some(change.clone())))?;
                match operator.operate(&Event::from(change)) {
                    Ok(_) => {}
                    Err(ExecutorError::MissingParent(_, _)) => {
                        remaining_changes.push(change.clone())
                    }
                    Err(err) => bail!("Error when operating on change : {:#}", err),
                };
                self.set_activity(Activity::StartupSync(None))?;
            }

            // No retry needed, don't retry
            if remaining_changes.is_empty() {
                break;
            // Retried but nothing changed, stop all
            } else if remaining_changes.len() == self.changes.len() {
                let detail: Vec<String> = remaining_changes
                    .iter()
                    .map(|event| event.to_string())
                    .collect();
                bail!(
                    "Unable to operate on following changes (missing parents): {}",
                    detail.join(", ")
                );
            }
            self.changes = remaining_changes;
        }

        Ok(())
    }

    fn set_activity(&self, activity: Activity) -> Result<()> {
        if let Some(activity_sender) = &self.activity_sender {
            log::info!(
                "[{}::{}] Set activity to {}",
                self.context.instance_name,
                self.context.workspace_id,
                activity,
            );
            if let Err(error) = activity_sender.send(WrappedActivity::new(
                self.context.job_identifier(),
                activity,
            )) {
                bail!(format!(
                    "[{}::{}] Error when sending activity end : {:?}",
                    self.context.instance_name, self.context.workspace_id, error
                ));
            }
        };

        Ok(())
    }
}

pub fn run(context: TrSyncContext, remote: RemoteControl) -> Result<()> {
    loop {
        let mut runner = Runner::new(context.clone(), remote.clone());
        if let Err(error) = runner.run() {
            log::error!("Operate error : {:#}", &error);

            // FIXME BS NOW : absolutely ugly. I think we should drop anyhow !!
            if let Some(error) = error.chain().last() {
                if format!("{}", error).to_lowercase().contains("connection") {
                    log::info!("Connection error, retry in 30s.");
                    thread::sleep(Duration::from_secs(30));
                    continue;
                }
            }

            if let Some(error_channels) = remote.error_channels() {
                runner.set_activity(Activity::Error)?;
                *error_channels.error().lock().unwrap() = Some(format!("{:#}", error));
                match error_channels.decision_receiver().recv() {
                    Ok(Decision::RestartSpaceSync) => {}
                    Err(_) => {
                        log::error!(
                            "Unable to communicate from trsync run to error decision receiver"
                        );
                        break;
                    }
                }
                runner.set_activity(Activity::Idle)?;
            }
        }
        if remote.stop_signal().load(Ordering::Relaxed) {
            remote.stop_signal().swap(false, Ordering::Relaxed);
            break;
        }
    }

    Ok(())
}
