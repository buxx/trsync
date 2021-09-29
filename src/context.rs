use std::path::Path;

#[derive(Debug, Clone)]
pub struct Context {
    pub base_address: String,
    pub username: String,
    pub password: String,
    pub folder_path: String,
    pub database_path: String,
    pub workspace_id: i32,
}

impl Context {
    pub fn new(
        ssl: bool,
        address: String,
        username: String,
        password: String,
        folder_path: String,
        workspace_id: i32,
    ) -> Self {
        let protocol = if ssl { "https" } else { "http" };
        let base_address = format!("{}://{}/api/", protocol, address);
        let database_path = Path::new(&folder_path)
            .join(".trsync.db")
            .to_str()
            .unwrap()
            .to_string();
        Self {
            base_address,
            username,
            password,
            folder_path,
            database_path,
            workspace_id,
        }
    }

    pub fn workspace_url(&self, suffix: &str) -> String {
        format!(
            "{}workspaces/{}/{}",
            self.base_address, self.workspace_id, suffix
        )
    }
}