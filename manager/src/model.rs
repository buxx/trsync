use serde_derive::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub struct Instance {
    pub name: String,
    pub address: String,
    pub unsecure: bool,
    pub username: String,
    pub password: String,
    pub workspaces_ids: Vec<u32>,
}

impl Instance {
    pub fn url(&self, suffix: Option<&str>) -> String {
        let suffix = suffix.unwrap_or("");
        let scheme = if self.unsecure { "http" } else { "https" };
        format!("{}://{}/api{}", scheme, self.address, suffix)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Workspace {
    pub label: String,
    pub workspace_id: u32,
}
