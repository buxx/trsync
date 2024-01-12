use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    thread,
    time::Duration,
};

use crossbeam_channel::Sender;
use trsync_core::{
    activity::{ActivityState, State},
    error::ErrorExchanger,
    sync::SyncExchanger,
    user::{MonitorWindowPanel, UserRequest},
};
use trsync_manager::message::DaemonMessage;

use tray_item::TrayItem;

use crate::{config::Config, icon::Icon};

pub fn run_tray(
    config: Config,
    main_sender: Sender<DaemonMessage>,
    activity_state: Arc<Mutex<ActivityState>>,
    stop_signal: Arc<AtomicBool>,
    user_request_sender: Sender<UserRequest>,
    sync_exchanger: Arc<Mutex<SyncExchanger>>,
    error_exchanger: Arc<Mutex<ErrorExchanger>>,
) -> Result<(), String> {
    match gtk::init() {
        Err(error) => return Err(format!("Unable to initialize gtk : '{}'", error)),
        _ => {}
    };

    let main_sender_monitor = main_sender.clone();
    let main_sender_configure = main_sender.clone();
    let main_sender_quit = main_sender.clone();

    // Icon
    let mut current_icon = Icon::Idle;
    let mut tray = match current_icon.value(&config).to_str() {
        Some(icon_value) => match TrayItem::new("Tracim", icon_value) {
            Ok(tray_) => tray_,
            Err(error) => return Err(format!("Unable to create tray item : '{}'", error)),
        },
        None => return Err("Unable to get icon value".to_string()),
    };

    // Monitor item
    let activity_state_ = activity_state.clone();
    let window_sender_ = user_request_sender.clone();
    match tray.add_menu_item("Moniteur", move || {
        let activity_state__ = activity_state_.clone();
        log::info!("Request monitor window open");
        if let Err(_) =
            window_sender_.send(UserRequest::OpenMonitorWindow(MonitorWindowPanel::Root))
        {}
    }) {
        Err(error) => return Err(format!("Unable to add menu item : '{:?}'", error)),
        _ => {}
    };

    // Configure item
    let window_sender_ = user_request_sender.clone();
    match tray.add_menu_item("Configurer", move || {
        log::info!("Request configure window open");
        if let Err(_) = window_sender_.send(UserRequest::OpenConfigurationWindow) {}
        let main_sender_ = main_sender_configure.clone();
        // thread::spawn(move || {
        //     if let Err(error) = run_configure(main_sender_) {
        //         log::error!("Unable to run configure window : '{}'", error)
        //     }
        // });
    }) {
        Err(error) => return Err(format!("Unable to add menu item : '{:?}'", error)),
        _ => {}
    };

    // Quit item
    let menu_stop_signal = stop_signal.clone();
    let main_sender_ = main_sender_quit.clone();
    let window_sender_ = user_request_sender.clone();
    match tray.add_menu_item("Quitter", move || {
        main_sender_.send(DaemonMessage::Stop).unwrap_or(());
        menu_stop_signal.store(true, Ordering::Relaxed);
        if let Err(_) = window_sender_.send(UserRequest::Quit) {}
        gtk::main_quit();
    }) {
        Err(error) => return Err(format!("Unable to add menu item : '{:?}'", error)),
        _ => {}
    };

    let glib_stop_signal = stop_signal.clone();
    let activity_state_ = activity_state.clone();
    glib::timeout_add_local(Duration::from_millis(250), move || {
        if glib_stop_signal.load(Ordering::Relaxed) {
            return glib::Continue(false);
        }

        let activity_icon = {
            // FIXME BS NOW: mettre ce Arc Mutex SyncExchanger dans une struct container pour simplifier
            let is_waiting_spaces = sync_exchanger
                .lock()
                .unwrap()
                .channels()
                .iter()
                .any(|channel| channel.1.changes().lock().unwrap().is_some());
            let is_error_spaces = error_exchanger
                .lock()
                .unwrap()
                .channels()
                .iter()
                .any(|channel| channel.1.error().lock().unwrap().is_some() && !channel.1.seen());
            if is_error_spaces {
                match current_icon {
                    Icon::Idle => Icon::Error,
                    Icon::Error => Icon::Idle,
                    _ => Icon::Idle,
                }
            } else if is_waiting_spaces {
                match current_icon {
                    Icon::Idle => Icon::Ask,
                    Icon::Ask => Icon::Idle,
                    _ => Icon::Idle,
                }
            } else if activity_state_.lock().unwrap().is_working() {
                match current_icon {
                    Icon::Idle => Icon::Working1,
                    Icon::Working1 => Icon::Working2,
                    Icon::Working2 => Icon::Working3,
                    Icon::Working3 => Icon::Working4,
                    Icon::Working4 => Icon::Working5,
                    Icon::Working5 => Icon::Working6,
                    Icon::Working6 => Icon::Working7,
                    Icon::Working7 => Icon::Working8,
                    Icon::Working8 => Icon::Working1,
                    _ => Icon::Working1,
                }
            } else {
                Icon::Idle
            }
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
