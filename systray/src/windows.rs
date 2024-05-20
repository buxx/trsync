use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::RecvTimeoutError;
use std::sync::{mpsc, Arc, Mutex};
use std::time::Duration;
use crossbeam_channel::Sender;
use tray_item::TrayItem;

use trsync_core::activity::ActivityState;
use trsync_core::error::ErrorExchanger;
use trsync_core::sync::SyncExchanger;
use trsync_core::user::{MonitorWindowPanel, UserRequest};
use trsync_manager::message::DaemonMessage;

use crate::icon::Icon;

enum Message {
    Quit,
}

pub fn run_tray(
    main_sender: Sender<DaemonMessage>,
    activity_state: Arc<Mutex<ActivityState>>,
    stop_signal: Arc<AtomicBool>,
    user_request_sender: Sender<UserRequest>,
    sync_exchanger: Arc<Mutex<SyncExchanger>>,
    error_exchanger: Arc<Mutex<ErrorExchanger>>,
) -> Result<(), String> {
    let mut current_icon = Icon::Idle;
    let activity_state_ = activity_state.clone();
    let main_sender_quit = main_sender.clone();

    // Icon
    let mut tray = match TrayItem::new("Tracim", "trsync_idle") {
        Ok(tray_) => tray_,
        Err(error) => return Err(format!("Unable to create tray item : '{}'", error)),
    };

    // Monitor item
    let window_sender_ = user_request_sender.clone();
    if let Err(error) = tray.add_menu_item("Moniteur", move || {
        log::info!("Request monitor window open");
        if window_sender_
            .send(UserRequest::OpenMonitorWindow(MonitorWindowPanel::Root))
            .is_err()
        {
            log::error!("Unable to send monitor window open request")
        }
    }) {
        return Err(format!("Unable to add menu item : '{:?}'", error));
    };

    // Configure item
    let window_sender_ = user_request_sender.clone();
    if let Err(error) = tray.add_menu_item("Configurer", move || {
        log::info!("Request configure window open");
        if window_sender_
            .send(UserRequest::OpenConfigurationWindow)
            .is_err()
        {
            log::error!("Unable to send configure window open request")
        }
    }) {
        return Err(format!("Unable to add menu item : '{:?}'", error));
    };

    // Quit item
    let (tx, rx) = mpsc::channel();
    let menu_stop_signal = stop_signal.clone();
    let main_sender_ = main_sender_quit.clone();
    let window_sender_ = user_request_sender.clone();
    if let Err(error) = tray.add_menu_item("Quitter", move || {
        main_sender_.send(DaemonMessage::Stop).unwrap_or(());
        tx.send(Message::Quit)
            .expect("This channel must not been closed");
    }) {
        return Err(format!("Unable to add menu item : '{:?}'", error));
    };

    loop {
        match rx.recv_timeout(Duration::from_millis(250)) {
            Err(RecvTimeoutError::Disconnected) => {
                log::error!("Tray channel disconnected");
                break;
            }
            Err(RecvTimeoutError::Timeout) => {
                if stop_signal.load(Ordering::Relaxed) {
                    break;
                }

                let activity_icon = {
                    let is_waiting_spaces = sync_exchanger
                        .lock()
                        .unwrap()
                        .channels()
                        .iter()
                        .any(|channel| channel.1.changes().lock().unwrap().is_some());
                    let is_error_spaces =
                        error_exchanger
                            .lock()
                            .unwrap()
                            .channels()
                            .iter()
                            .any(|channel| {
                                channel.1.error().lock().unwrap().is_some() && !channel.1.seen()
                            });
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
                    let icon_value = current_icon.value();
                    log::debug!("Set icon to {}", icon_value);
                    if let Err(error) = tray.set_icon(&icon_value) {
                        log::error!("Unable to set icon : '{:?}'", error);
                    };
                }
            }
            Ok(Message::Quit) => break,
        }
    }

    Ok(())
}
