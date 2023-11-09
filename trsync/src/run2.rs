extern crate notify;
use crate::context::Context as TrSyncContext;
use crate::database::connection;
use crate::event::local::LocalEvent;
use crate::event::Event;
use crate::local::LocalWatcher;
use crate::operation::Job;
use crate::operation2::operator::Operator;
use crate::remote::{RemoteEvent, RemoteWatcher};
use crate::state::disk::DiskState;
use crate::state::State;
use crate::sync::local::{LocalChange, LocalSync};
use crate::sync::remote::{RemoteChange, RemoteSync};
use crate::sync::{ResolveMethod, StartupSyncResolver};
use anyhow::{Context, Result};
use crossbeam_channel::Sender as CrossbeamSender;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use std::{fs, thread};

struct Runner {
    context: TrSyncContext,
    stop_signal: Arc<AtomicBool>,
    restart_signal: Arc<AtomicBool>,
    activity_sender: Option<CrossbeamSender<Job>>,
    operational_sender: Sender<Event>,
    operational_receiver: Receiver<Event>,
    remote_sender: Sender<RemoteEvent>,
    remote_receiver: Receiver<RemoteEvent>,
    local_sender: Sender<LocalEvent>,
    local_receiver: Receiver<LocalEvent>,
}

impl Runner {
    fn new(
        context: TrSyncContext,
        stop_signal: Arc<AtomicBool>,
        activity_sender: Option<CrossbeamSender<Job>>,
    ) -> Self {
        let restart_signal = Arc::new(AtomicBool::new(false));
        let (operational_sender, operational_receiver): (Sender<Event>, Receiver<Event>) =
            channel();
        let (remote_sender, remote_receiver): (Sender<RemoteEvent>, Receiver<RemoteEvent>) =
            channel();
        let (local_sender, local_receiver): (Sender<LocalEvent>, Receiver<LocalEvent>) = channel();

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
            local_receiver,
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

    fn sync(&self) -> Result<()> {
        // FIXME BS NOW : stocker les "Ignore Event" que génère les Executors
        let workspace_path = PathBuf::from(&self.context.folder_path);
        let remote_changes = self.remote_changes()?;
        let local_changes = self.local_changes()?;
        let (remote_changes, local_changes) =
            StartupSyncResolver::new(remote_changes, local_changes, ResolveMethod::ForceLocal)
                .resolve()?;

        // DEBUG
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
        for local_change in local_changes {
            let event_display = format!("{:?}", &local_change);
            operator
                .operate(local_change.into())
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

        thread::spawn(move || {
            while let Ok(remote_event) = self.remote_receiver.recv() {
                if let Err(error) = operational_sender.send(Event::Remote(remote_event)) {
                    log::info!("Terminate remote listener");
                }
            }
        });

        Ok(())
    }

    fn listen_local(&self) -> Result<()> {
        let operational_sender = self.operational_sender.clone();

        thread::spawn(move || {
            while let Ok(local_event) = self.local_receiver.recv() {
                if let Err(error) = operational_sender.send(Event::Local(local_event)) {
                    log::info!("Terminate locate listener");
                }
            }
        });

        Ok(())
    }

    fn operate(&self) -> Result<()> {
        todo!()
    }

    pub fn run(&mut self) -> Result<()> {
        self.ensure_folders()?;
        self.ensure_db()?;

        // TODO : Start listening remote TLM
        // TODO : Start listening local changes
        // TODO : Keep eye on stop_signal to stop soon as requested

        // FIXME BS NOW : argh : il faut pouvoir recolter les events locaux et remote pendant la sync SANS etre pollué par les event généras par la sync
        self.watchers()?;
        self.sync()?;

        if self.context.exit_after_sync {
            return Ok(());
        }

        self.listen()?;
        self.operate()?;

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
