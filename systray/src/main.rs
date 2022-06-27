use crossbeam_channel::{unbounded, Receiver, Sender};
use env_logger::Env;
use error::Error;
use std::{
    process::exit,
    sync::{atomic::AtomicBool, Arc, Mutex},
};
use trsync::operation::Job;
use trsync_manager;
use uuid::Uuid;

use crate::state::ActivityMonitor;

#[cfg(target_os = "linux")]
mod linux;

#[cfg(target_os = "windows")]
mod windows;

mod config;
mod error;
mod icon;
mod password;
mod state;
mod utils;

fn run() -> Result<(), Error> {
    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();

    let stop_signal = Arc::new(AtomicBool::new(false));
    let activity_state = Arc::new(Mutex::new(state::ActivityState::new()));
    let config = match config::Config::from_env() {
        Ok(config_) => config_,
        Err(error) => {
            log::error!("{:?}", error);
            std::process::exit(1);
        }
    };

    let trsync_manager_configure_bin_path = config.trsync_manager_configure_bin_path.clone();

    // Start manager
    log::info!("Start manager");
    let (main_channel_sender, main_channel_receiver): (
        Sender<trsync_manager::message::DaemonControlMessage>,
        Receiver<trsync_manager::message::DaemonControlMessage>,
    ) = unbounded();
    let (activity_sender, activity_receiver): (Sender<Job>, Receiver<Job>) = unbounded();
    let manager_config = trsync_manager::config::Config::from_env(false)?;

    // Start manager
    // FIXME : How it is stopped ?
    trsync_manager::reload::ReloadWatcher::new(manager_config.clone(), main_channel_sender.clone())
        .start()?;
    let manager_child = std::thread::spawn(move || {
        match trsync_manager::daemon::Daemon::new(
            manager_config,
            main_channel_receiver,
            activity_sender,
        )
        .run()
        {
            Err(error) => {
                log::error!("Unable to start manager : '{:?}'", error);
            }
            _ => {}
        };
    });

    // Start activity monitor
    // FIXME : How it is stopped ?
    let activity_monitor_stop_signal = stop_signal.clone();
    let activity_monitor_state = activity_state.clone();
    let activity_monitor_child = std::thread::spawn(move || {
        ActivityMonitor::new(
            activity_receiver.clone(),
            activity_monitor_state,
            activity_monitor_stop_signal,
        )
        .run()
    });

    // Start password http receiver
    log::info!("Raw password disabled, prepare to start password receiver");
    let password_port = match utils::get_available_port() {
        Some(port) => port,
        None => {
            return Err(Error::UnexpectedError(
                "Unable to find available port".to_string(),
            ))
        }
    };
    let password_token = Uuid::new_v4().to_string();
    password::start_password_receiver_server(password_port, &password_token);
    log::info!("Password receiver started on port: '{}'", &password_port);

    log::info!("Start systray");
    let tray_stop_signal = stop_signal.clone();
    #[cfg(target_os = "linux")]
    {
        let tray_config = config.clone();
        let tray_activity_state = activity_state.clone();
        match linux::run_tray(
            tray_config,
            trsync_manager_configure_bin_path.clone(),
            password_port,
            &password_token,
            tray_activity_state,
            tray_stop_signal,
        ) {
            Err(error) => {
                log::error!("{}", error)
            }
            _ => {}
        }
    }

    #[cfg(target_os = "windows")]
    {
        // FIXME stop signal too
        match windows::run_tray(
            trsync_manager_configure_bin_path.clone(),
            password_port,
            &password_token,
            activity_state,
        ) {
            Err(error) => {
                log::error!("{}", error)
            }
            _ => {}
        }
    }

    log::info!("Stop manager");
    main_channel_sender
        .send(trsync_manager::message::DaemonControlMessage::Stop)
        .or_else(|e| {
            Err(Error::UnexpectedError(format!(
                "Unable to ask manager to stop : '{}'",
                e
            )))
        })?;
    match manager_child.join() {
        Err(error) => {
            return Err(Error::UnexpectedError(format!(
                "Unable to join manager thread : '{:?}'",
                error
            )))
        }
        _ => {}
    };

    // FIXME Close properly
    activity_monitor_child.join().unwrap();
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
