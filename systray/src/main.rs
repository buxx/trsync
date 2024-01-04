#![windows_subsystem = "windows"]

use anyhow::Result;
use crossbeam_channel::{unbounded, Receiver, Sender};
use env_logger::Env;
use error::Error;
use std::{
    process::exit,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
};
use trsync::operation::Job;
use trsync_core::config::ManagerConfig;
use trsync_manager::{self, daemon::Daemon, message::DaemonMessage};

use crate::state::ActivityMonitor;

#[cfg(target_os = "linux")]
mod linux;

#[cfg(target_os = "windows")]
mod windows;

mod config;
mod error;
mod icon;
mod state;
mod sync;

type DaemonMessageChannels = (Sender<DaemonMessage>, Receiver<DaemonMessage>);
type ActivityChannels = (Sender<Job>, Receiver<Job>);

fn run() -> Result<()> {
    // Some initialize
    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();
    let stop_signal = Arc::new(AtomicBool::new(false));
    let activity_state = Arc::new(Mutex::new(state::ActivityState::new()));
    let config = config::Config::from_env()?;

    // Start manager
    log::info!("Start manager");
    let (main_sender, main_receiver): DaemonMessageChannels = unbounded();
    let (activity_sender, activity_receiver): ActivityChannels = unbounded();
    let manager_config = ManagerConfig::from_env(false)?;
    Daemon::new(manager_config, main_receiver, activity_sender).start()?;

    // Start activity monitor
    let activity_receiver_ = activity_receiver.clone();
    let activity_state_ = activity_state.clone();
    let stop_signal_ = stop_signal.clone();
    ActivityMonitor::new(activity_receiver_, activity_state_, stop_signal_).start();

    log::info!("Start systray");
    #[cfg(target_os = "linux")]
    {
        let tray_config = config.clone();
        let tray_activity_state = activity_state.clone();
        let tray_stop_signal = stop_signal.clone();
        let main_sender_ = main_sender.clone();
        if let Err(error) = linux::run_tray(
            tray_config,
            main_sender_,
            tray_activity_state,
            tray_stop_signal,
        ) {
            log::error!("{}", error)
        }
    }

    #[cfg(target_os = "windows")]
    {
        let tray_stop_signal = stop_signal.clone();
        let main_sender_ = main_sender.clone();
        match windows::run_tray(main_sender_, activity_state, tray_stop_signal) {
            Err(error) => {
                log::error!("{}", error)
            }
            _ => {}
        }
    }

    // When these lines are reached, tray is finished so, close application
    log::info!("Stopping ...");
    stop_signal.swap(true, Ordering::Relaxed);
    main_sender
        .send(DaemonMessage::Stop)
        .map_err(|e| Error::UnexpectedError(format!("Unable to ask manager to stop : '{}'", e)))?;
    log::info!("Finished");

    Ok(())
}

fn main() {
    match run() {
        Ok(_) => {}
        Err(error) => {
            log::error!("Error happens during run : {:?}", error);
            exit(1)
        }
    }
}
