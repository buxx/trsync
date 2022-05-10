#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub struct TrsyncUid {
    instance_address: String,
    workspace_id: u32,
}

impl TrsyncUid {
    pub fn new(instance_address: String, workspace_id: u32) -> Self {
        Self {
            instance_address,
            workspace_id,
        }
    }

    pub fn instance_address(&self) -> &str {
        &self.instance_address
    }

    pub fn workspace_id(&self) -> &u32 {
        &self.workspace_id
    }
}

impl std::fmt::Display for TrsyncUid {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}::{}", self.instance_address, self.workspace_id)
    }
}
