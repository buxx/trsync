use crossbeam_channel::{unbounded, Receiver, Sender};
use env_logger::Env;
use error::Error;
use std::process::exit;
use trsync_manager;
use uuid::Uuid;

#[cfg(target_os = "linux")]
mod linux;

#[cfg(target_os = "windows")]
mod windows;

mod config;
mod error;
mod password;
mod utils;

fn run() -> Result<(), Error> {
    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();

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
    let config = trsync_manager::config::Config::from_env(false)?;
    trsync_manager::reload::ReloadWatcher::new(config.clone(), main_channel_sender.clone())
        .start()?;
    let manager_child = std::thread::spawn(move || {
        // TODO : manage error at manager start
        trsync_manager::daemon::Daemon::new(config, main_channel_receiver)
            .run()
            .unwrap();
    });

    // Start password http receiver
    log::info!("Raw password disabled, prepare to start password receiver");
    let password_port = utils::get_available_port().unwrap();
    let password_token = Uuid::new_v4().to_string();
    password::start_password_receiver_server(password_port, &password_token);
    log::info!("Password receiver started on port: {}", &password_port);

    log::info!("Start systray");
    #[cfg(target_os = "linux")]
    {
        match linux::run_tray(
            trsync_manager_configure_bin_path.clone(),
            password_port,
            &password_token,
        ) {
            Err(error) => {
                log::error!("{}", error)
            }
            _ => {}
        }
    }

    #[cfg(target_os = "windows")]
    {
        match windows::run_tray(
            trsync_manager_configure_bin_path.clone(),
            password_port,
            &password_token,
        ) {
            Err(error) => {
                log::error!("{}", error)
            }
            _ => {}
        }
    }

    log::info!("Stop manager");
    // TODO : manage error cases
    main_channel_sender
        .send(trsync_manager::message::DaemonControlMessage::Stop)
        .unwrap();
    // TODO : manage error cases
    manager_child.join().unwrap();
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
