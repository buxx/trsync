use structopt::StructOpt;
extern crate notify;

use std::sync::mpsc::channel;
use std::thread;

use crate::local::{LocalSync, LocalWatcher};
use crate::remote::{RemoteSync, RemoteWatcher};

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
    // TODO : Must have a local index with inode,modified and same for remote to compare when sync

    let opt = Opt::from_args();
    println!("Watch {:?}", opt.path);
    let (operational_sender, operational_receiver) = channel();

    // First, start local sync to know changes since last start
    let mut local_sync = LocalSync::new();
    let local_sync_handle = thread::spawn(move || local_sync.sync());

    // Second, start remote sync to know remote changes since last run
    let mut remote_sync = RemoteSync::new();
    let remote_sync_handle = thread::spawn(move || remote_sync.sync());

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

    // Wait end of local and remote  sync
    local_sync_handle.join().unwrap();
    remote_sync_handle.join().unwrap();

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
