use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};

use trsync_manager_configure::run::run as run_configure;

use gtk;
use tray_item::TrayItem;

use crate::{
    config::Config,
    icon::Icon,
    state::{Activity, ActivityState},
};

pub fn run_tray(
    config: Config,
    activity_state: Arc<Mutex<ActivityState>>,
    stop_signal: Arc<AtomicBool>,
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
    match tray.add_menu_item("Configurer", move || {
        log::info!("Run configure window");
        if let Err(error) = run_configure() {
            return log::error!("Unable to run configure window : '{}'", error);
        };
    }) {
        Err(error) => return Err(format!("Unable to add menu item : '{:?}'", error)),
        _ => {}
    };

    let menu_stop_signal = stop_signal.clone();
    match tray.add_menu_item("Quitter", move || {
        menu_stop_signal.store(true, Ordering::Relaxed);
        gtk::main_quit();
    }) {
        Err(error) => return Err(format!("Unable to add menu item : '{:?}'", error)),
        _ => {}
    };

    let glib_stop_signal = stop_signal.clone();
    glib::timeout_add_local(Duration::from_millis(250), move || {
        if glib_stop_signal.load(Ordering::Relaxed) {
            return glib::Continue(false);
        }

        let activity_icon = match activity_state.lock().unwrap().activity() {
            Activity::Idle => Icon::Idle,
            Activity::Working => match current_icon {
                Icon::Idle => Icon::Working1,
                Icon::Working1 => Icon::Working2,
                Icon::Working2 => Icon::Working3,
                Icon::Working3 => Icon::Working4,
                Icon::Working4 => Icon::Working5,
                Icon::Working5 => Icon::Working6,
                Icon::Working6 => Icon::Working7,
                Icon::Working7 => Icon::Working8,
                Icon::Working8 => Icon::Working1,
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
