use std::path::Path;

use crate::error::Error;
use crate::util;

// FIXME : exclude password from debug print
#[derive(Debug, Clone)]
pub struct Context {
    pub base_address: String,
    pub username: String,
    pub password: String,
    pub folder_path: String,
    pub database_path: String,
    pub workspace_id: i32,
    pub exit_after_sync: bool,
}

impl Context {
    pub fn new(
        ssl: bool,
        address: String,
        username: String,
        password: String,
        folder_path: String,
        workspace_id: i32,
        exit_after_sync: bool,
    ) -> Result<Self, Error> {
        let protocol = if ssl { "https" } else { "http" };
        let base_address = format!("{}://{}/api/", protocol, address);
        let database_path = util::path_to_string(&Path::new(&folder_path).join(".trsync.db"))?;
        Ok(Self {
            base_address,
            username,
            password,
            folder_path,
            database_path,
            workspace_id,
            exit_after_sync,
        })
    }

    pub fn workspace_url(&self, suffix: &str) -> String {
        format!(
            "{}workspaces/{}/{}",
            self.base_address, self.workspace_id, suffix
        )
    }
}
