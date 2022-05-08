use std::process::Command;
use structopt::StructOpt;

#[cfg(target_os = "linux")]
mod linux;

#[cfg(target_os = "windows")]
mod windows;

mod config;
mod error;

#[derive(StructOpt, Debug)]
#[structopt(name = "basic")]
pub struct Opt {
    #[structopt(short, long = "--manager-bin-path")]
    manager_bin_path: Option<String>,
    #[structopt(short, long = "--configure-bin-path")]
    configure_bin_path: Option<String>,
}

fn main() {
    let opt = Opt::from_args();
    let config = match config::Config::from_env() {
        Ok(config_) => config_,
        Err(error) => {
            eprintln!("{:?}", error);
            std::process::exit(1);
        }
    };

    let trsync_manager_bin_path = if let Some(trsync_manager_bin_path_) = opt.manager_bin_path {
        trsync_manager_bin_path_.clone()
    } else {
        config.trsync_manager_bin_path.clone()
    };

    let trsync_manager_configure_bin_path =
        if let Some(trsync_manager_configure_bin_path_) = opt.configure_bin_path {
            trsync_manager_configure_bin_path_.clone()
        } else {
            config.trsync_manager_configure_bin_path.clone()
        };

    let mut manager_child = if cfg!(target_os = "windows") {
        Command::new("cmd")
            .arg("/c")
            .arg(trsync_manager_bin_path)
            .spawn()
            .unwrap()
    } else {
        Command::new(trsync_manager_bin_path).spawn().unwrap()
    };

    #[cfg(target_os = "linux")]
    {
        match linux::run_tray(trsync_manager_configure_bin_path.clone()) {
            Err(error) => {
                eprintln!("{}", error)
            }
            _ => {}
        }
    }

    #[cfg(target_os = "windows")]
    {
        match windows::run_tray(trsync_manager_configure_bin_path.clone()) {
            Err(error) => {
                eprintln!("{}", error)
            }
            _ => {}
        }
    }

    println!("Stop manager");
    manager_child.kill().unwrap();
    manager_child.wait().unwrap();
    println!("Finished")
}
