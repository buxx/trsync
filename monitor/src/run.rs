use std::sync::{Arc, Mutex};

use anyhow::{bail, Result};

use crossbeam_channel::Receiver;
use eframe::epaint::vec2;
use trsync_core::{activity::ActivityState, config::ManagerConfig, user::UserRequest};

use crate::app::App;

pub fn run(
    activity_state: Arc<Mutex<ActivityState>>,
    user_request_receiver: Receiver<UserRequest>,
) -> Result<()> {
    let options = eframe::NativeOptions {
        initial_window_size: Some(vec2(710.0, 600.0)),
        ..Default::default()
    };
    let config = ManagerConfig::from_env(false)?;
    // let state = State::from_config(&config);

    let mut app = App::new(activity_state, user_request_receiver);
    app.start()?;

    if let Err(error) = eframe::run_native("TrSync monitor", options, Box::new(|_cc| Box::new(app)))
    {
        bail!("Running error : {}", error)
    };
    Ok(())
}
