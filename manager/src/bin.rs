use std::sync::{atomic::AtomicBool, Arc};

use crate::config::Config;
use crossbeam_channel::{unbounded, Receiver, Sender};
use env_logger::Env;
use trsync::operation::Job;

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
        Sender<message::DaemonMessage>,
        Receiver<message::DaemonMessage>,
    ) = unbounded();
    let stop_signal = Arc::new(AtomicBool::new(false));

    log::info!("Read config");
    let config = Config::from_env(true)?;

    log::info!("Build and run reload watcher");
    let config_ = config.clone();
    let main_channel_sender_ = main_channel_sender.clone();
    let stop_signal_ = stop_signal.clone();
    reload::ReloadWatcher::new(config_, main_channel_sender_, stop_signal_).start()?;

    log::info!("Start daemon");
    let config_ = config.clone();
    let (activity_sender, activity_receiver): (Sender<Job>, Receiver<Job>) = unbounded();
    std::thread::spawn(move || while let Ok(_) = activity_receiver.recv() {});
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
