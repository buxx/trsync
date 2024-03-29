use std::path::PathBuf;

use anyhow::{Context, Result};

use trsync_core::{
    client::{TracimClient, TracimClientError},
    instance::ContentId,
};

use crate::{
    event::{remote::RemoteEvent, Event},
    operation::executor::{Executor, ExecutorError},
    state::{modification::StateModification, State},
};

pub struct AbsentFromRemoteExecutor {
    db_path: PathBuf,
}

impl AbsentFromRemoteExecutor {
    pub fn new(db_path: PathBuf) -> Self {
        Self { db_path }
    }

    fn content_id(&self, state: &dyn State) -> Result<Option<ContentId>> {
        state
            .content_id_for_path(self.db_path.clone())
            .context(format!("Get content_id for {}", self.db_path.display()))
    }
}

impl Executor for AbsentFromRemoteExecutor {
    fn execute(
        &self,
        state: &dyn State,
        tracim: &dyn TracimClient,
        ignore_events: &mut Vec<Event>,
    ) -> Result<Vec<StateModification>, ExecutorError> {
        let content_id = self.content_id(state)?.context(format!(
            "Path {} must match to a content_id",
            self.db_path.display()
        ))?;
        let _content = state
            .get(content_id)
            .context(format!("Get content {}", content_id))?;

        let remote_content = match tracim.get_content(content_id) {
            Ok(content) => content,
            Err(TracimClientError::ContentNotFound) => {
                log::debug!("Content {} not found when trying to delete it", content_id);
                return Ok(vec![StateModification::Forgot(content_id)]);
            }
            Err(err) => return Err(ExecutorError::from(err)),
        };

        tracim
            .trash_content(content_id)
            .context(format!("Trash content {}", content_id))?;

        if !remote_content.is_deleted {
            ignore_events.push(Event::Remote(RemoteEvent::Deleted(content_id)));
        }

        Ok(vec![StateModification::Forgot(content_id)])
    }
}
