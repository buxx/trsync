use std::path::PathBuf;

use anyhow::{Context, Result};

use trsync_core::{client::TracimClient, instance::ContentId};

use crate::{
    event::{remote::RemoteEvent, Event},
    operation2::executor::{Executor, ExecutorError},
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
        ignore_events: &mut Vec<Event>,
    ) -> Result<Vec<StateModification>, ExecutorError> {
        let content_id = self.content_id(state)?.context(format!(
            "Path {} must match to a content_id",
            self.db_path.display()
        ))?;
        let _content = state
            .get(content_id)
            .context(format!("Get content {}", content_id))?;

        let remote_content = tracim
            .get_content(content_id)
            .context(format!("Get content {}", content_id))?;

        tracim
            .trash_content(content_id)
            .context(format!("Trash content {}", content_id))?;

        if !remote_content.is_deleted {
            ignore_events.push(Event::Remote(RemoteEvent::Deleted(content_id)));
        }

        return Ok(vec![StateModification::Forgot(content_id)]);
    }
}
