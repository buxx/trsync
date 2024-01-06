use std::{
    env,
    sync::{atomic::AtomicBool, Arc},
};

use anyhow::Result;
use env_logger::Env;
use error::Error;
use structopt::StructOpt;
use trsync_core::{instance::WorkspaceId, sync::AcceptAllSyncPolitic};
extern crate notify;

pub mod client;
pub mod conflict;
pub mod context;
pub mod database;
pub mod error;
pub mod event;
pub mod knowledge;
pub mod local;
pub mod local2;
pub mod message;
pub mod operation;
pub mod operation2;
pub mod path;
pub mod remote;
pub mod run;
pub mod run2;
pub mod state;
pub mod sync;
pub mod util;

#[cfg(test)]
mod tests;

#[derive(StructOpt, Debug)]
#[structopt(name = "basic")]
struct Opt {
    #[structopt(parse(from_os_str))]
    path: std::path::PathBuf,

    #[structopt(name = "tracim_address")]
    tracim_address: String,

    #[structopt(name = "workspace_id")]
    workspace_id: i32,

    #[structopt(name = "username")]
    username: String,

    #[structopt(name = "--no-ssl", short, long)]
    no_ssl: bool,

    #[structopt(name = "--env-var-pass", long, short)]
    env_var_pass: Option<String>,

    #[structopt(name = "--exit-after-sync", long)]
    exit_after_sync: bool,

    #[structopt(name = "--prevent-delete-sync", long)]
    prevent_delete_sync: bool,
}

impl Opt {
    fn to_context(&self, password: String) -> Result<context::Context, Error> {
        // TODO: no unwrap ...
        let workspace_name = self.path.file_name().unwrap().to_str().unwrap().to_string();
        context::Context::new(
            !self.no_ssl,
            self.tracim_address.clone(),
            self.username.clone(),
            password.clone(),
            util::canonicalize_to_string(&self.path)?,
            WorkspaceId(self.workspace_id),
            workspace_name,
            self.exit_after_sync,
            self.prevent_delete_sync,
        )
    }
}

fn main() -> Result<(), Error> {
    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();
    let opt = Opt::from_args();

    // Ask password by input or get it from env var
    let password = if let Some(env_var_pass) = &opt.env_var_pass {
        match env::var(env_var_pass) {
            Ok(password) => password,
            Err(_) => {
                return Err(Error::UnexpectedError(format!(
                    "No en var set for name {}",
                    &env_var_pass
                )))
            }
        }
    } else {
        rpassword::prompt_password("Tracim user password ? ")?
    };

    let context = opt.to_context(password.clone())?;
    let _stop_signal = Arc::new(AtomicBool::new(false));
    if let Err(error) = run2::run(context, _stop_signal, None, Box::new(AcceptAllSyncPolitic)) {
        return Err(Error::UnexpectedError(format!("{:#}", error)));
    }
    log::info!("Exit application");
    Ok(())
}
