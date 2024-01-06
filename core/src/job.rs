#[derive(PartialEq, Eq, Hash, Debug)]
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

pub enum Job {
    Begin(JobIdentifier),
    End(JobIdentifier),
}
