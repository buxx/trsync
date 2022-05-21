use std::process::Command;
use std::sync::mpsc;
use tray_item::TrayItem;

enum Message {
    Quit,
}

pub fn run_tray(configure_bin_path: String) -> Result<(), String> {
    let mut tray = match TrayItem::new("Tracim", "my-icon-name") {
        Ok(tray_) => tray_,
        Err(error) => return Err(format!("Unable to create tray item : '{}'", error)),
    };

    match tray.add_menu_item("Configurer", move || {
        log::info!("Run {}", configure_bin_path);
        if let Some(password_setter_port_) = password_setter_port {
            Command::new("cmd")
                .arg("/c")
                .arg(&configure_bin_path)
                .arg(format!("--password-setter-port={}", password_setter_port_))
                .spawn()
                .unwrap();
        } else {
            Command::new("cmd")
                .arg("/c")
                .arg(&configure_bin_path)
                .spawn()
                .unwrap();
        };
    }) {
        Err(error) => return Err(format!("Unable to add menu item : '{:?}'", error)),
        _ => {}
    };

    let (tx, rx) = mpsc::channel();

    tray.add_menu_item("Quitter", move || {
        tx.send(Message::Quit)
            .expect("This channel must not been closed");
    })
    .unwrap();

    loop {
        match rx.recv() {
            Ok(Message::Quit) => break,
            _ => {}
        }
    }

    Ok(())
}
