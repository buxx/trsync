use std::path::PathBuf;

use crate::database::DB_NAME;
use crate::{
    state::State,
    sync::{local::LocalSync, remote::RemoteSync},
};
use anyhow::{Context, Result};
use rusqlite::Connection;
use trsync_core::client::TracimClient;

pub mod local;
pub mod remote;

pub struct StartupSyncResolver {
    workspace_path: PathBuf,
    client: Box<dyn TracimClient>,
}

impl StartupSyncResolver {
    pub fn resolve(&self) -> Result<Box<dyn State>> {
        // TODO : parallelize
        let remote_changes = RemoteSync::new(self.connection()?, self.client.clone())
            .changes()
            .context("Build remote changes")?;
        let local_changes = LocalSync::new(self.connection()?, self.workspace_path.clone())
            .changes()
            .context("Build local changes")?;
        todo!()
    }

    fn connection(&self) -> Result<Connection> {
        let db_path = self.workspace_path.join(DB_NAME);
        Connection::open(&db_path)
            .context(format!("Open database connection on {}", db_path.display()))
    }
}
