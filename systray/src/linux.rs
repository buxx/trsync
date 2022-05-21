use std::process::Command;

use gtk;
use tray_item::TrayItem;

pub fn run_tray(
    configure_bin_path: String,
    password_setter_port: Option<u16>,
) -> Result<(), String> {
    match gtk::init() {
        Err(error) => return Err(format!("Unable to initialize gtk : '{}'", error)),
        _ => {}
    };

    let mut tray = match TrayItem::new("Tracim", "emblem-shared") {
        Ok(tray_) => tray_,
        Err(error) => return Err(format!("Unable to create tray item : '{}'", error)),
    };

    match tray.add_menu_item("Configurer", move || {
        log::info!("Run {}", configure_bin_path);
        if let Some(password_setter_port_) = password_setter_port {
            Command::new(&configure_bin_path)
                .arg(format!("--password-setter-port={}", password_setter_port_))
                .spawn()
                .unwrap()
        } else {
            Command::new(&configure_bin_path).spawn().unwrap()
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
