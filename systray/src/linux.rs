use std::{
    process::Command,
    sync::{Arc, Mutex},
    time::Duration,
};

use gtk;
use tray_item::TrayItem;

use crate::{
    config::Config,
    icon::Icon,
    state::{Activity, ActivityState},
};

pub fn run_tray(
    config: Config,
    configure_bin_path: String,
    password_setter_port: u16,
    password_setter_token: &str,
    activity_state: Arc<Mutex<ActivityState>>,
) -> Result<(), String> {
    match gtk::init() {
        Err(error) => return Err(format!("Unable to initialize gtk : '{}'", error)),
        _ => {}
    };

    let mut current_icon = Icon::Idle;
    let mut tray = match current_icon.value(&config).to_str() {
        Some(icon_value) => match TrayItem::new("Tracim", icon_value) {
            Ok(tray_) => tray_,
            Err(error) => return Err(format!("Unable to create tray item : '{}'", error)),
        },
        None => return Err(format!("Unable to get icon value")),
    };

    let password_setter_token_ = password_setter_token.to_string();
    match tray.add_menu_item("Configurer", move || {
        log::info!("Run {}", configure_bin_path);
        match Command::new(&configure_bin_path)
            .arg(format!("--password-setter-port={}", password_setter_port))
            .arg(format!(
                "--password-setter-token={}",
                password_setter_token_
            ))
            .spawn()
        {
            Err(error) => return log::error!("Unable to start configure window : '{:?}'", error),
            _ => {}
        };
    }) {
        Err(error) => return Err(format!("Unable to add menu item : '{:?}'", error)),
        _ => {}
    };

    match tray.add_menu_item("Quitter", || {
        gtk::main_quit();
    }) {
        Err(error) => return Err(format!("Unable to add menu item : '{:?}'", error)),
        _ => {}
    };

    glib::timeout_add_local(Duration::from_millis(250), move || {
        let activity_icon = match activity_state.lock().unwrap().activity() {
            Activity::Idle => Icon::Idle,
            Activity::Working => match current_icon {
                Icon::Idle => Icon::Working1,
                Icon::Working1 => Icon::Working2,
                Icon::Working2 => Icon::Working3,
                Icon::Working3 => Icon::Working4,
                Icon::Working4 => Icon::Working1,
            },
        };
        if activity_icon != current_icon {
            current_icon = activity_icon;
            match current_icon.value(&config).to_str() {
                Some(icon_value) => {
                    log::debug!("Set icon to {}", icon_value);
                    if let Err(error) = tray.set_icon(icon_value) {
                        log::error!("Unable to set icon : '{:?}'", error);
                    };
                }
                None => {
                    log::error!("Unable to get icon value");
                }
            };
        }

        glib::Continue(true)
    });

    gtk::main();
    Ok(())
}
