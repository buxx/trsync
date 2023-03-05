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
use trsync_manager::{self, daemon::Daemon, message::DaemonMessage, reload::ReloadWatcher};

use crate::state::ActivityMonitor;

#[cfg(target_os = "linux")]
mod linux;

#[cfg(target_os = "windows")]
mod windows;

mod config;
mod error;
mod icon;
mod state;

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
    let manager_config_ = manager_config.clone();
    let main_sender_ = main_sender.clone();
    let stop_signal_ = stop_signal.clone();
    ReloadWatcher::new(manager_config_, main_sender_, stop_signal_).start()?;
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
        match linux::run_tray(tray_config, tray_activity_state, tray_stop_signal) {
            Err(error) => {
                log::error!("{}", error)
            }
            _ => {}
        }
    }

    #[cfg(target_os = "windows")]
    {
        let tray_stop_signal = stop_signal.clone();
        match windows::run_tray(
            password_port,
            &password_token,
            activity_state,
            tray_stop_signal,
        ) {
            Err(error) => {
                log::error!("{}", error)
            }
            _ => {}
        }
    }

    // When these lines are reached, tray is finished so, close application
    log::info!("Stopping ...");
    stop_signal.swap(true, Ordering::Relaxed);
    main_sender.send(DaemonMessage::Stop).or_else(|e| {
        Err(Error::UnexpectedError(format!(
            "Unable to ask manager to stop : '{}'",
            e
        )))
    })?;
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
