use anyhow::Result;
use crossbeam_channel::{unbounded, Receiver, Sender};
use env_logger::Env;
use trsync_core::{activity::WrappedActivity, config::ManagerConfig};

pub mod client;
pub mod daemon;
pub mod error;
pub mod message;
pub mod types;

fn main_() -> Result<()> {
    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();

    // As standalone server (no systray), we never send daemon messages
    // TODO : listen to SIG_HUP signal to send reload daemon message
    let (_, _main_channel_receiver): (
        Sender<message::DaemonMessage>,
        Receiver<message::DaemonMessage>,
    ) = unbounded();

    log::info!("Read config");
    let config = ManagerConfig::from_env(true)?;

    log::info!("Start daemon");
    let _config_ = config.clone();
    let (_activity_sender, activity_receiver): (
        Sender<WrappedActivity>,
        Receiver<WrappedActivity>,
    ) = unbounded();
    std::thread::spawn(move || while activity_receiver.recv().is_ok() {});
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
