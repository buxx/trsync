use std::path::PathBuf;

use anyhow::{Context, Result};

use trsync_core::{client::TracimClient, instance::ContentId};

use crate::{
    operation2::executor::Executor,
    state::{modification::StateModification, State},
};

pub struct AbsentFromRemoteExecutor {
    db_path: PathBuf,
}

impl AbsentFromRemoteExecutor {
    pub fn new(db_path: PathBuf) -> Self {
        Self { db_path }
    }

    fn content_id(&self, state: &Box<dyn State>) -> Result<Option<ContentId>> {
        state
            .content_id_for_path(self.db_path.clone())
            .context(format!("Get content_id for {}", self.db_path.display()))
    }
}

impl Executor for AbsentFromRemoteExecutor {
    fn execute(
        &self,
        state: &Box<dyn State>,
        tracim: &Box<dyn TracimClient>,
    ) -> Result<StateModification> {
        let content_id = self.content_id(state)?.context(format!(
            "Path {} must match to a content_id",
            self.db_path.display()
        ))?;
        let _content = state
            .get(content_id)
            .context(format!("Get content {}", content_id))?;

        tracim
            .trash_content(content_id)
            .context(format!("Trash content {}", content_id))?;

        return Ok(StateModification::Forgot(content_id));
    }
}
