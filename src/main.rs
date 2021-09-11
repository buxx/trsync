use structopt::StructOpt;
extern crate notify;

use notify::{watcher, RecursiveMode, Watcher};
use std::sync::mpsc::channel;
use std::thread;
use std::time::Duration;

use crate::local::LocalWatcher;
use crate::remote::RemoteWatcher;

pub mod error;
pub mod local;
pub mod operation;
pub mod remote;

#[derive(StructOpt, Debug)]
#[structopt(name = "basic")]
pub struct Opt {
    #[structopt(parse(from_os_str))]
    path: std::path::PathBuf,
}

fn main() {
    let opt = Opt::from_args();
    println!("Watch {:?}", opt.path);
    let (operational_sender, operational_receiver) = channel();

    // Local watcher
    let mut local_watcher = LocalWatcher::new(operational_sender.clone());
    let local_handle = thread::spawn(move || loop {
        local_watcher.listen(&opt.path)
    });

    // Remote watcher
    let mut remote_watcher = RemoteWatcher::new(operational_sender.clone());
    let remote_handle = thread::spawn(move || loop {
        remote_watcher.listen()
    });

    // Operational
    let operational_handle = thread::spawn(move || loop {
        for message in operational_receiver.recv() {
            println!("Message : {:?}", message)
        }
    });

    local_handle.join().unwrap();
    remote_handle.join().unwrap();
    operational_handle.join().unwrap();
}
