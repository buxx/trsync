use crate::{config::Config, message::DaemonControlMessage};
use crossbeam_channel::{unbounded, Receiver, Sender};
use env_logger::Env;

mod client;
mod config;
mod daemon;
mod error;
mod message;
mod model;
mod reload;
mod security;
mod types;

fn main_() -> Result<(), error::Error> {
    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();

    let (main_channel_sender, main_channel_receiver): (
        Sender<DaemonControlMessage>,
        Receiver<DaemonControlMessage>,
    ) = unbounded();

    log::info!("Read config");
    let config = Config::from_env(true)?;

    log::info!("Build and run reload watcher");
    reload::ReloadWatcher::new(config.clone(), main_channel_sender.clone()).start()?;

    log::info!("Start daemon");
    daemon::Daemon::new(config, main_channel_receiver).run()?;
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
