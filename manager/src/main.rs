use env_logger::Env;

use crate::config::Config;

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

    log::info!("Build config from env");
    let config = Config::from_env()?;
    log::info!("Build and run reload watcher");
    let reload_channel = reload::ReloadWatcher::new(config.clone()).watch()?;
    log::info!("Start daemon");
    // FIXME : si erreur la pas de print :(
    daemon::Daemon::new(config).run(reload_channel)?;
    log::info!("Daemon finished, exit");

    Ok(())
}
