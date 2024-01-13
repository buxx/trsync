use std::fmt;
use std::path::Path;
use std::time::Duration;

use anyhow::Result;
use trsync_core::client::{Tracim, DEFAULT_CLIENT_TIMEOUT};
use trsync_core::instance::WorkspaceId;
use trsync_core::job::JobIdentifier;

use crate::database::DB_NAME;
use crate::error::Error;
use crate::util;

#[derive(Clone)]
pub struct Context {
    pub instance_name: String,
    pub base_address: String,
    pub username: String,
    pub password: String,
    pub folder_path: String,
    pub database_path: String,
    pub workspace_id: WorkspaceId,
    pub workspace_name: String,
    pub exit_after_sync: bool,
}

impl Context {
    pub fn new(
        ssl: bool,
        address: String,
        username: String,
        password: String,
        folder_path: String,
        workspace_id: WorkspaceId,
        workspace_name: String,
        exit_after_sync: bool,
    ) -> Result<Self, Error> {
        let protocol = if ssl { "https" } else { "http" };
        let base_address = format!("{}://{}/api/", protocol, address);
        let database_path = util::path_to_string(&Path::new(&folder_path).join(DB_NAME))?;
        Ok(Self {
            instance_name: address,
            base_address,
            username,
            password,
            folder_path,
            database_path,
            workspace_id,
            workspace_name,
            exit_after_sync,
        })
    }

    pub fn workspace_url(&self, suffix: &str) -> String {
        format!(
            "{}workspaces/{}/{}",
            self.base_address, self.workspace_id, suffix
        )
    }

    pub fn client(&self) -> Result<Tracim> {
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(DEFAULT_CLIENT_TIMEOUT))
            .build()?;
        Ok(Tracim::new(
            self.base_address.clone(),
            self.workspace_id,
            client,
            self.username.clone(),
            self.password.clone(),
        ))
    }

    pub fn job_identifier(&self) -> JobIdentifier {
        JobIdentifier::new(
            self.instance_name.clone(),
            self.workspace_id.0,
            self.workspace_name.clone(),
        )
    }
}

impl fmt::Debug for Context {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Context")
            .field("base_address", &self.base_address)
            .field("username", &self.username)
            .field("folder_path", &self.folder_path)
            .field("base_address", &self.base_address)
            .field("workspace_id", &self.workspace_id)
            .field("exit_after_sync", &self.exit_after_sync)
            .finish()
    }
}
