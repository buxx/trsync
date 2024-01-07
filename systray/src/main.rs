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
    thread,
    time::Duration,
};
use trsync_core::{
    activity::{ActivityMonitor, ActivityState},
    config::ManagerConfig,
    job::Job,
    sync::SyncExchanger,
    user::UserRequest,
};
use trsync_manager::{self, daemon::Daemon, message::DaemonMessage};
use trsync_manager_configure::run::run as run_configure;
use trsync_manager_monitor::run::run as run_monitor;

#[cfg(target_os = "linux")]
mod linux;

#[cfg(target_os = "windows")]
mod windows;

mod config;
mod error;
mod icon;

type DaemonMessageChannels = (Sender<DaemonMessage>, Receiver<DaemonMessage>);
type ActivityChannels = (Sender<Job>, Receiver<Job>);

fn run() -> Result<()> {
    // Some initialize
    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();
    let stop_signal = Arc::new(AtomicBool::new(false));
    let activity_state = Arc::new(Mutex::new(ActivityState::new()));
    let config = config::Config::from_env()?;
    let manager_config = ManagerConfig::from_env(false)?;
    let (user_request_sender, user_request_receiver): (Sender<UserRequest>, Receiver<UserRequest>) =
        unbounded();
    let sync_exchanger = Arc::new(Mutex::new(SyncExchanger::new()));

    // Start manager
    log::info!("Start manager");
    let (main_sender, main_receiver): DaemonMessageChannels = unbounded();
    let (activity_sender, activity_receiver): ActivityChannels = unbounded();
    Daemon::new(
        manager_config,
        main_receiver,
        activity_sender,
        user_request_sender.clone(),
        sync_exchanger.clone(),
    )
    .start()?;

    // Start activity monitor
    let activity_receiver_ = activity_receiver.clone();
    let activity_state_ = activity_state.clone();
    let stop_signal_ = stop_signal.clone();
    ActivityMonitor::new(activity_receiver_, activity_state_, stop_signal_).start();

    // Systray
    let activity_state_ = activity_state.clone();
    let stop_signal_ = stop_signal.clone();
    let main_sender_ = main_sender.clone();
    let user_request_sender_ = user_request_sender.clone();
    thread::spawn(move || {
        log::info!("Start systray");
        #[cfg(target_os = "linux")]
        {
            let tray_config = config.clone();
            let tray_activity_state = activity_state_.clone();
            let tray_stop_signal_ = stop_signal_.clone();
            let main_sender_ = main_sender_.clone();
            if let Err(error) = linux::run_tray(
                tray_config,
                main_sender_,
                tray_activity_state,
                tray_stop_signal_,
                user_request_sender_,
            ) {
                log::error!("{}", error)
            }
        }

        #[cfg(target_os = "windows")]
        {
            let tray_stop_signal = stop_signal.clone();
            let main_sender_ = main_sender.clone();
            match windows::run_tray(
                main_sender_,
                activity_state,
                tray_stop_signal,
                user_request_sender_,
            ) {
                Err(error) => {
                    log::error!("{}", error)
                }
                _ => {}
            }
        }
    });

    let activity_state_ = activity_state.clone();
    let main_sender_ = main_sender.clone();
    loop {
        match user_request_receiver.recv_timeout(Duration::from_millis(150)) {
            Err(RecvTimeoutError) => {}
            Err(_) => break,
            Ok(request) => match request {
                UserRequest::OpenMonitorWindow(panel) => {
                    if let Err(error) = run_monitor(
                        activity_state_.clone(),
                        user_request_receiver.clone(),
                        sync_exchanger.clone(),
                        panel,
                    ) {
                        log::error!("Unable to run configure window : '{}'", error)
                    }
                }
                UserRequest::OpenConfigurationWindow => {
                    if let Err(error) =
                        run_configure(main_sender_.clone(), user_request_receiver.clone())
                    {
                        log::error!("Unable to run configure window : '{}'", error)
                    }
                }
                UserRequest::Quit => break,
            },
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
