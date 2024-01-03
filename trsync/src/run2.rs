extern crate notify;
use crate::context::Context as TrSyncContext;
use crate::database::connection;
use crate::event::remote::RemoteEvent;
use crate::event::Event;
use crate::local::{DiskEvent, LocalWatcher};
use crate::local2::reducer::LocalReceiverReducer;
use crate::operation::Job;
use crate::operation2::operator::Operator;
use crate::remote::RemoteWatcher;
use crate::state::disk::DiskState;
use crate::state::State;
use crate::sync::local::{LocalChange, LocalSync};
use crate::sync::remote::{RemoteChange, RemoteSync};
use crate::sync::{ResolveMethod, StartupSyncResolver};
use anyhow::{Context, Result};
use crossbeam_channel::{unbounded, Receiver, Sender};
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::{fs, thread};

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
}

impl Runner {
    fn new(
        context: TrSyncContext,
        stop_signal: Arc<AtomicBool>,
        activity_sender: Option<Sender<Job>>,
    ) -> Self {
        let restart_signal = Arc::new(AtomicBool::new(false));
        let (operational_sender, operational_receiver): (Sender<Event>, Receiver<Event>) =
            unbounded();
        let (remote_sender, remote_receiver): (Sender<RemoteEvent>, Receiver<RemoteEvent>) =
            unbounded();
        let (local_sender, local_receiver): (Sender<DiskEvent>, Receiver<DiskEvent>) = unbounded();
        let local_receiver_reducer = LocalReceiverReducer::new(local_receiver);

        // FIXME BS NOW : ensure_availability

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

    // FIXME BS NOW : échange d'event a ignorer (sync de départ / executors)
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

    // FIXME BS NOW : échange d'event a ignorer (sync de départ / executors)
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

    fn sync(&self, operator: &mut Operator) -> Result<()> {
        let remote_changes = self.remote_changes()?;
        let local_changes = self.local_changes()?;
        let (remote_changes, local_changes) =
            StartupSyncResolver::new(remote_changes, local_changes, ResolveMethod::ForceLocal)
                .resolve()?;

        for remote_change in remote_changes {
            let event_display = format!("{:?}", &remote_change);
            operator
                .operate(&(remote_change.into()))
                .context(format!("Operate on remote change {:?}", event_display))?
        }
        for local_change in local_changes {
            let event_display = format!("{:?}", &local_change);
            operator
                .operate(&(local_change.into()))
                .context(format!("Operate on local change {:?}", event_display))?
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

    fn operate(&self, operator: &mut Operator) -> Result<()> {
        while let Ok(event) = self.operational_receiver.recv() {
            log::info!("Proceed event {:?}", &event);
            let context_message = format!("Operate on event {:?}", &event);
            if let Err(error) = operator.operate(&event).context(context_message) {
                log::error!(
                    "Error happens during operate of '{:?}': '{:#}'",
                    &event,
                    &error,
                )
            };
        }

        log::info!("Terminate operational listener");
        Ok(())
    }

    pub fn run(&mut self) -> Result<()> {
        self.ensure_folders()?;
        self.ensure_db()?;

        let workspace_path = PathBuf::from(&self.context.folder_path);
        let mut state: Box<dyn State> = Box::new(DiskState::new(
            connection(&workspace_path).context(format!(
                "Create connection for startup sync for {}",
                workspace_path.display()
            ))?,
            workspace_path.clone(),
        ));
        let workspace_path = PathBuf::from(&self.context.folder_path);
        let client = self
            .context
            .client()
            .context("Create tracim client for startup sync")?;
        let mut operator = Operator::new(&mut state, &workspace_path, Box::new(client));

        // TODO : Start listening remote TLM
        // TODO : Start listening local changes
        // TODO : Keep eye on stop_signal to stop soon as requested

        // FIXME BS NOW : argh : il faut pouvoir recolter les events locaux et remote pendant la sync SANS etre pollué par les event généras par la sync
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
) -> Result<()> {
    let mut runner = Runner::new(context, stop_signal, activity_sender);
    runner.run().context("Run")?;

    Ok(())
}
