use std::path::PathBuf;

use anyhow::{Context, Result};

use trsync_core::{
    client::TracimClient,
    instance::{ContentId, DiskTimestamp},
    types::ContentType,
};

use crate::{
    operation2::executor::Executor,
    state::{modification::StateModification, State},
    util::last_modified_timestamp,
};

pub struct ModifiedOnRemoteExecutor {
    workspace_folder: PathBuf,
    content_id: ContentId,
}

impl ModifiedOnRemoteExecutor {
    pub fn new(workspace_folder: PathBuf, content_id: ContentId) -> Self {
        Self {
            workspace_folder,
            content_id,
        }
    }

    fn absolute_path(&self, state: &Box<dyn State>) -> Result<PathBuf> {
        let content_path = state
            .path(self.content_id)
            .context(format!("Get content {} path", self.content_id))?
            .to_path_buf();
        Ok(self.workspace_folder.join(content_path))
    }

    fn content_type(&self, state: &Box<dyn State>) -> Result<ContentType> {
        let content = state
            .get(self.content_id)
            .context(format!("Get content {}", self.content_id))?
            .context(format!("Expected content {}", self.content_id))?;
        Ok(*content.type_())
    }

    fn last_modified(&self, state: &Box<dyn State>) -> Result<DiskTimestamp> {
        let absolute_path = self.absolute_path(state)?;
        let since_epoch = last_modified_timestamp(&absolute_path)?;
        Ok(DiskTimestamp(since_epoch.as_millis()))
    }
}

impl Executor for ModifiedOnRemoteExecutor {
    fn execute(
        &self,
        state: &Box<dyn State>,
        tracim: &Box<dyn TracimClient>,
    ) -> Result<StateModification> {
        let absolute_path = self.absolute_path(state)?;
        let content_type = self.content_type(state)?;

        if content_type.fillable() {
            tracim
                .fill_content_with_file(self.content_id, &absolute_path)
                .context(format!(
                    "Fill remote file {} with {}",
                    self.content_id,
                    &absolute_path.display(),
                ))?;
        }

        let content = tracim
            .get_content(self.content_id)
            .context(format!("Get just created content {}", self.content_id))?;
        let last_modified = self.last_modified(state).context(format!(
            "Get last modified datetime of {}",
            absolute_path.display()
        ))?;

        Ok(StateModification::Update(
            content.id(),
            content.file_name().clone(),
            content.revision_id(),
            content.parent_id(),
            last_modified,
        ))
    }
}
