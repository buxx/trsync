use std::path::PathBuf;

use anyhow::{Context, Result};

use trsync_core::{
    client::TracimClient,
    content::Content,
    instance::{ContentFileName, ContentId, DiskTimestamp},
    types::ContentType,
};

use crate::{
    operation2::executor::Executor,
    state::{modification::StateModification, State},
    util::last_modified_timestamp,
};

pub struct CreatedOnRemoteExecutor {
    workspace_folder: PathBuf,
    path: PathBuf,
}

impl CreatedOnRemoteExecutor {
    pub fn new(workspace_folder: PathBuf, path: PathBuf) -> Self {
        Self {
            workspace_folder,
            path,
        }
    }

    fn absolute_path(&self) -> PathBuf {
        self.workspace_folder.join(&self.path)
    }

    fn file_name(&self) -> Result<String> {
        Ok(self
            .path
            .file_name()
            .context(format!("Cut file_name from {}", self.path.display()))?
            .to_str()
            .context(format!("Decode file_name from {}", self.path.display()))?
            .to_string())
    }

    fn parent(&self, state: &Box<dyn State>) -> Result<Option<ContentId>> {
        if let Some(parent_path) = self.path.parent() {
            return state
                .content_id_for_path(parent_path.to_path_buf())
                .context(format!("Search content for path {}", parent_path.display()));
        }

        Ok(None)
    }

    fn content_type(&self) -> ContentType {
        if self.absolute_path().is_dir() {
            ContentType::Folder
        } else {
            // FIXME : manage notes ?
            ContentType::File
        }
    }
}

impl Executor for CreatedOnRemoteExecutor {
    fn execute(
        &self,
        state: &Box<dyn State>,
        tracim: &Box<dyn TracimClient>,
    ) -> Result<StateModification> {
        let absolute_path = self.absolute_path();
        let file_name = self.file_name()?;
        let parent = self.parent(state)?;
        let content_type = self.content_type();

        let content_id = tracim
            .create_content(ContentFileName(file_name), content_type, parent)
            .context(format!(
                "Create remote content from {}",
                absolute_path.display()
            ))?;

        if content_type.fillable() {
            tracim
                .fill_content_with_file(content_id, &absolute_path)
                .context(format!(
                    "Fill remote file {} with {}",
                    content_id,
                    absolute_path.display(),
                ))?;
        }

        let content = Content::from_remote(
            &tracim
                .get_content(content_id)
                .context(format!("Get just created content {}", content_id))?,
        )?;
        let disk_timestamp = last_modified_timestamp(&absolute_path)
            .context(format!("Get disk timestamp of {}", absolute_path.display()))?;
        Ok(StateModification::Add(
            content,
            self.path.clone(),
            DiskTimestamp(disk_timestamp.as_millis() as u64),
        ))
    }
}
