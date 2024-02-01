use std::{fs, path::PathBuf};

use anyhow::{Context, Result};
use log::debug;
use trsync_core::{client::TracimClient, instance::ContentId, types::ContentType};

use crate::{
    event::Event,
    local2::reducer::DiskEventWrap,
    local2::watcher::DiskEvent,
    operation::executor::{Executor, ExecutorError},
    state::{modification::StateModification, State},
};

pub struct AbsentFromDiskExecutor {
    workspace_folder: PathBuf,
    content_id: ContentId,
}

impl AbsentFromDiskExecutor {
    pub fn new(workspace_folder: PathBuf, content_id: ContentId) -> Self {
        Self {
            workspace_folder,
            content_id,
        }
    }
}

impl Executor for AbsentFromDiskExecutor {
    fn execute(
        &self,
        state: &dyn State,
        _tracim: &dyn TracimClient,
        ignore_events: &mut Vec<Event>,
    ) -> Result<Vec<StateModification>, ExecutorError> {
        let content = state
            .get(self.content_id)
            .context(format!("Get content {}", self.content_id))?
            .context(format!("Expect content {}", self.content_id))?;
        let content_path: PathBuf = state
            .path(self.content_id)
            .context(format!("Get content {} path", self.content_id))?
            .into();
        let absolute_path = self.workspace_folder.join(&content_path);

        if absolute_path.exists() {
            ignore_events.push(Event::Local(DiskEventWrap::new(
                content_path.clone(),
                DiskEvent::Deleted(content_path.clone()),
            )))
        }

        if let Err(_err) = match content.type_() {
            ContentType::Folder => fs::remove_dir_all(&absolute_path),
            _ => fs::remove_file(&absolute_path),
        } {
            debug!("File/folder {} was already absent", content_path.display());
        }

        Ok(vec![StateModification::Forgot(self.content_id)])
    }
}
