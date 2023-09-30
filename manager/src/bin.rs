use anyhow::Result;
use crossbeam_channel::{unbounded, Receiver, Sender};
use env_logger::Env;
use trsync::operation::Job;
use trsync_core::config::ManagerConfig;

mod client;
mod daemon;
mod error;
mod message;
mod types;

fn main_() -> Result<()> {
    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();

    // As standalone server (no systray), we never send daemon messages
    // TODO : listen to SIG_HUP signal to send reload daemon message
    let (_, main_channel_receiver): (
        Sender<message::DaemonMessage>,
        Receiver<message::DaemonMessage>,
    ) = unbounded();

    log::info!("Read config");
    let config = ManagerConfig::from_env(true)?;

    log::info!("Start daemon");
    let config_ = config.clone();
    let (activity_sender, activity_receiver): (Sender<Job>, Receiver<Job>) = unbounded();
    std::thread::spawn(move || while activity_receiver.recv().is_ok() {});
    daemon::Daemon::new(config_, main_channel_receiver, activity_sender).run()?;
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
