use std::fmt::Display;

#[derive(PartialEq, Eq, Hash, Debug, Clone)]
pub struct JobIdentifier {
    pub instance_name: String,
    pub workspace_id: i32,
    pub workspace_name: String,
}

impl JobIdentifier {
    pub fn new(instance_name: String, workspace_id: i32, workspace_name: String) -> Self {
        Self {
            instance_name,
            workspace_id,
            workspace_name,
        }
    }
}

impl Display for JobIdentifier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&format!(
            "{}::{}",
            &self.instance_name, &self.workspace_name
        ))
    }
}
