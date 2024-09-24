use std::{
    sync::{Arc, Mutex},
    thread,
};

use anyhow::Result;
use crossbeam_channel::{unbounded, Receiver, Sender};
use daemon::Daemon;
use env_logger::Env;
use message::DaemonMessage;
use trsync_core::{
    activity::WrappedActivity, config::ManagerConfig, error::ErrorExchanger, sync::SyncExchanger,
    user::UserRequest,
};

pub mod client;
pub mod daemon;
pub mod error;
pub mod message;
pub mod types;

type DaemonMessageChannels = (Sender<DaemonMessage>, Receiver<DaemonMessage>);
type ActivityChannels = (Sender<WrappedActivity>, Receiver<WrappedActivity>);

fn main_() -> Result<()> {
    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();

    // As standalone server (no systray), we never send daemon messages
    // TODO : listen to SIG_HUP signal to send reload daemon message
    let (_, _main_channel_receiver): (
        Sender<message::DaemonMessage>,
        Receiver<message::DaemonMessage>,
    ) = unbounded();

    log::info!("Read config");
    let mut config = ManagerConfig::from_env(true)?;
    config.confirm_startup_sync = false;
    config.popup_confirm_startup_sync = false;

    let sync_exchanger = Arc::new(Mutex::new(SyncExchanger::new()));
    let error_exchanger = Arc::new(Mutex::new(ErrorExchanger::new()));
    let (_main_sender, main_receiver): DaemonMessageChannels = unbounded();
    let (activity_sender, activity_receiver): ActivityChannels = unbounded();
    let sync_exchanger = sync_exchanger.clone();
    let error_exchanger = error_exchanger.clone();
    let (user_request_sender, _): (Sender<UserRequest>, Receiver<UserRequest>) = unbounded();

    thread::spawn(move || {
        while let Ok(activity) = activity_receiver.recv() {
            log::debug!("Activity: {:?}", activity)
        }
    });

    log::info!("Start daemon");
    Daemon::new(
        config,
        main_receiver,
        activity_sender,
        user_request_sender,
        sync_exchanger,
        error_exchanger,
    )
    .run()?;
    log::info!("Daemon finished, exit");

    Ok(())
}

fn main() -> Result<(), error::Error> {
    match main_() {
        Ok(_) => {}
        Err(error) => {
            log::error!("{}", error);
            std::process::exit(1);
        }
    }

    Ok(())
}
