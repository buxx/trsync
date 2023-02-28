#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release

use anyhow::Result;

mod app;
mod event;
mod job;
mod panel;
mod run;
mod state;
mod utils;

fn main() -> Result<()> {
    Ok(run::run()?)
}
