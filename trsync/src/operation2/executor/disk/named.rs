use std::{fs, path::PathBuf};

use anyhow::{Context, Result};
use trsync_core::{client::TracimClient, instance::ContentId};

use crate::{
    operation2::executor::Executor,
    state::{modification::StateModification, State},
};

pub struct NamedOnDiskExecutor {
    workspace_folder: PathBuf,
    content_id: ContentId,
}

impl NamedOnDiskExecutor {
    pub fn new(workspace_folder: PathBuf, content_id: ContentId) -> Self {
        Self {
            workspace_folder,
            content_id,
        }
    }
}

impl Executor for NamedOnDiskExecutor {
    fn execute(
        &self,
        state: &Box<dyn State>,
        tracim: &Box<dyn TracimClient>,
    ) -> Result<StateModification> {
        let local_content_path = state
            .path(self.content_id)
            .context(format!("Get local content {} path", self.content_id))?
            .to_path_buf();
        let remote_content = tracim
            .get_content(self.content_id)
            .context(format!("Get remote content {}", self.content_id))?;
        // FIXME BS NOW : How to be sure than parent is always already present ?!
        let remote_content_path: PathBuf = if let Some(parent_id) = remote_content.parent_id() {
            state
                .path(parent_id)
                .context(format!("Get parent {} path", parent_id))?
                .to_path_buf()
                .join(PathBuf::from(remote_content.file_name().to_string()))
        } else {
            PathBuf::from(remote_content.file_name().to_string())
        };
        let previous_absolute_path = self.workspace_folder.join(local_content_path);
        let new_absolute_path = self.workspace_folder.join(remote_content_path);

        fs::rename(&previous_absolute_path, &new_absolute_path).context(format!(
            "Move {} to {}",
            previous_absolute_path.display(),
            new_absolute_path.display()
        ))?;

        Ok(StateModification::Rename(
            self.content_id,
            remote_content.file_name().clone(),
            remote_content.parent_id(),
        ))
    }
}
