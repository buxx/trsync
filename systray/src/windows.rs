use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::RecvTimeoutError;
use std::sync::{mpsc, Arc, Mutex};
use std::time::Duration;
use tray_item::TrayItem;

use crate::icon::Icon;
use crate::state::{Activity, ActivityState};

enum Message {
    Quit,
}

pub fn run_tray(
    configure_bin_path: String,
    password_setter_port: u16,
    password_setter_token: &str,
    activity_state: Arc<Mutex<ActivityState>>,
    stop_signal: Arc<AtomicBool>,
) -> Result<(), String> {
    let mut tray = match TrayItem::new("Tracim", "trsync_idle") {
        Ok(tray_) => tray_,
        Err(error) => return Err(format!("Unable to create tray item : '{}'", error)),
    };

    let mut current_icon = Icon::Idle;
    let password_setter_token_ = password_setter_token.to_string();
    match tray.add_menu_item("Configurer", move || {
        log::info!("Run {}", configure_bin_path);
        match Command::new("cmd")
            .arg("/c")
            .arg(&configure_bin_path)
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

    let (tx, rx) = mpsc::channel();

    match tray.add_menu_item("Quitter", move || {
        tx.send(Message::Quit)
            .expect("This channel must not been closed");
    }) {
        Err(error) => return Err(format!("Unable to send quit message : '{:?}'", error)),
        _ => {}
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
