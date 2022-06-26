use std::process::Command;
use std::sync::mpsc;
use tray_item::TrayItem;

enum Message {
    Quit,
}

pub fn run_tray(
    configure_bin_path: String,
    password_setter_port: u16,
    password_setter_token: &str,
) -> Result<(), String> {
    let mut tray = match TrayItem::new("Tracim", "my-icon-name") {
        Ok(tray_) => tray_,
        Err(error) => return Err(format!("Unable to create tray item : '{}'", error)),
    };

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
        match rx.recv() {
            Ok(Message::Quit) => break,
            _ => {}
        }
    }

    Ok(())
}
