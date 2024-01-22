use anyhow::{bail, Result};

use crossbeam_channel::{Receiver, Sender};
use trsync_core::{config::ManagerConfig, user::UserRequest};
use trsync_manager::message::DaemonMessage;

use crate::{app::App, state::State};

pub fn run(
    main_sender: Sender<DaemonMessage>,
    user_request_receiver: Receiver<UserRequest>,
) -> Result<()> {
    let options = eframe::NativeOptions {
        // initial_window_size: Some(vec2(710.0, 600.0)),
        ..Default::default()
    };
    let config = ManagerConfig::from_env(false)?;
    let state = State::from_config(&config);
    let mut app = App::new(state, main_sender, user_request_receiver);
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
