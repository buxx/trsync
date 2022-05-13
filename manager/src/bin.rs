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

fn main() -> Result<(), error::Error> {
    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();

    let (main_channel_sender, main_channel_receiver): (
        Sender<DaemonControlMessage>,
        Receiver<DaemonControlMessage>,
    ) = unbounded();

    log::info!("Read config");
    let config = Config::from_env()?;

    log::info!("Build and run reload watcher");
    reload::ReloadWatcher::new(config.clone(), main_channel_sender.clone()).start()?;

    log::info!("Start daemon");
    // FIXME : si erreur la pas de print :(
    daemon::Daemon::new(config, main_channel_receiver).run()?;
    log::info!("Daemon finished, exit");

    Ok(())
}
