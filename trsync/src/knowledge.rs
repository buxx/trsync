use crate::{
    client::Client, context::Context, database::DatabaseOperation, error::ClientError,
    types::RelativeFilePath,
};
use mockall::automock;
use rusqlite::Connection;

#[automock]
pub trait Knowledge {
    fn instance_name(&self) -> &str;
    fn workspace_id(&self) -> i32;
    fn get_remote_relative_path(&self, content_id: i32) -> Result<RelativeFilePath, ClientError>;
    fn get_local_relative_path(&self, content_id: i32) -> Result<RelativeFilePath, String>;
}

pub struct ExternalKnowledge {
    context: Context,
    client: Client,
}

impl ExternalKnowledge {
    pub fn new(context: Context, client: Client) -> Self {
        Self { context, client }
    }
}

impl Knowledge for ExternalKnowledge {
    fn instance_name(&self) -> &str {
        &self.context.instance_name
    }

    fn workspace_id(&self) -> i32 {
        self.context.workspace_id
    }

    fn get_remote_relative_path(&self, content_id: i32) -> Result<RelativeFilePath, ClientError> {
        let remote_content = self.client.get_remote_content(content_id)?;
        Ok(self.client.build_relative_path(&remote_content)?)
    }

    fn get_local_relative_path(&self, content_id: i32) -> Result<RelativeFilePath, String> {
        // TODO : open as demand is not optimal but how to deal witch test mock ?
        let connection = match Connection::open(&self.context.database_path) {
            Ok(connection_) => connection_,
            Err(error) => return Err(format!("{}", error)),
        };
        match DatabaseOperation::new(&connection).get_path_from_content_id(content_id) {
            Ok(relative_path_) => Ok(relative_path_),
            Err(error) => Err(format!("{}", error)),
        }
    }
}
