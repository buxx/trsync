use std::{
    env,
    sync::{atomic::AtomicBool, Arc},
};

use env_logger::Env;
use error::Error;
use structopt::StructOpt;
extern crate notify;

pub mod client;
pub mod context;
pub mod database;
pub mod error;
pub mod local;
pub mod message;
pub mod operation;
pub mod remote;
pub mod run;
pub mod types;
pub mod util;

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
}

impl Opt {
    fn to_context(&self, password: String) -> Result<context::Context, Error> {
        Ok(context::Context::new(
            !self.no_ssl,
            self.tracim_address.clone(),
            self.username.clone(),
            password.clone(),
            util::canonicalize_to_string(&self.path)?,
            self.workspace_id,
            self.exit_after_sync,
        )?)
    }
}

fn main() -> Result<(), Error> {
    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();
    let opt = Opt::from_args();

    // Ask password by input or get it from env var
    let password = if let Some(env_var_pass) = &opt.env_var_pass {
        match env::var(&env_var_pass) {
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
    run::run(context, _stop_signal, None)?;
    log::info!("Exit application");
    Ok(())
}
