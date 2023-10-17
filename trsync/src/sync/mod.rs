use crate::{
    state::State,
    sync::{local::LocalSync, remote::RemoteSync},
};
use anyhow::{Context, Result};
use trsync_core::client::TracimClient;

pub mod local;
pub mod remote;

pub struct StartupSyncResolver {
    client: Box<dyn TracimClient>,
}

impl StartupSyncResolver {
    pub fn resolve(&self) -> Result<Box<dyn State>> {
        // TODO : parallelize
        let remote_state = RemoteSync::new(self.client.clone())
            .state()
            .context("Build remote state")?;
        // let local_state = LocalSync::new().state().context("Build local state")?;
        todo!()
    }
}
