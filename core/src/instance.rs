use serde_derive::{Deserialize, Serialize};
use std::fmt::Display;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct InstanceId(pub String);

impl Display for InstanceId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Hash)]
pub struct WorkspaceId(pub i32);

impl Display for WorkspaceId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0.to_string())
    }
}

#[derive(Debug, Clone)]
pub struct Instance {
    pub name: InstanceId,
    pub address: String,
    pub unsecure: bool,
    pub username: String,
    pub password: String,
    pub workspaces_ids: Vec<WorkspaceId>,
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
    pub workspace_id: WorkspaceId,
}
