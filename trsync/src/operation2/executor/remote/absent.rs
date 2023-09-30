use anyhow::{Context, Result};

use trsync_core::{client::TracimClient, instance::ContentId};

use crate::{
    operation2::executor::Executor,
    state::{modification::StateModification, State},
};

pub struct AbsentFromRemoteExecutor {
    content_id: ContentId,
}

impl AbsentFromRemoteExecutor {
    pub fn new(content_id: ContentId) -> Self {
        Self { content_id }
    }
}

impl Executor for AbsentFromRemoteExecutor {
    fn execute(
        &self,
        state: &Box<dyn State>,
        tracim: &Box<dyn TracimClient>,
    ) -> Result<StateModification> {
        let _content = state
            .get(self.content_id)
            .context(format!("Get content {}", self.content_id))?;

        tracim
            .trash_content(self.content_id)
            .context(format!("Trash content {}", self.content_id))?;

        return Ok(StateModification::Forgot(self.content_id));
    }
}
