use anyhow::{bail, Result};

use eframe::epaint::vec2;
use trsync_core::config::ManagerConfig;

use crate::{app::App, state::State};

pub fn run() -> Result<()> {
    let options = eframe::NativeOptions {
        initial_window_size: Some(vec2(800.0, 600.0)),
        ..Default::default()
    };
    // FIXME BS NOW : raw password
    let config = ManagerConfig::from_env(false)?;
    let state = State::from_config(&config);
    let mut app = App::new(state);
    app.start()?;

    if let Err(error) = eframe::run_native(
        "TrSync configuration",
        options,
        Box::new(|_cc| Box::new(app)),
    ) {
        bail!("Running error : {}", error)
    };
    Ok(())
}
