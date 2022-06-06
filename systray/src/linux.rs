use std::process::Command;

use gtk;
use tray_item::TrayItem;

pub fn run_tray(
    configure_bin_path: String,
    password_setter_port: u16,
    password_setter_token: &str,
) -> Result<(), String> {
    match gtk::init() {
        Err(error) => return Err(format!("Unable to initialize gtk : '{}'", error)),
        _ => {}
    };

    let mut tray = match TrayItem::new("Tracim", "emblem-shared") {
        Ok(tray_) => tray_,
        Err(error) => return Err(format!("Unable to create tray item : '{}'", error)),
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

    gtk::main();
    Ok(())
}
