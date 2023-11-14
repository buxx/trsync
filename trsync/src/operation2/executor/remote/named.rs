use std::path::PathBuf;

use anyhow::{Context, Result};

use trsync_core::{
    client::TracimClient,
    instance::{ContentFileName, ContentId, DiskTimestamp, RevisionId},
    types::ContentType,
};

use crate::{
    operation2::executor::Executor,
    state::{modification::StateModification, State},
    util::last_modified_timestamp,
};

pub struct NamedOnRemoteExecutor {
    workspace_folder: PathBuf,
    previous_db_path: PathBuf,
    after_disk_path: PathBuf,
}

impl NamedOnRemoteExecutor {
    pub fn new(
        workspace_folder: PathBuf,
        previous_db_path: PathBuf,
        after_disk_path: PathBuf,
    ) -> Self {
        Self {
            workspace_folder,
            previous_db_path,
            after_disk_path,
        }
    }

    fn content_id(&self, state: &Box<dyn State>) -> Result<Option<ContentId>> {
        state
            .content_id_for_path(self.previous_db_path.clone())
            .context(format!(
                "Get content_id for {}",
                self.previous_db_path.display()
            ))
    }

    fn before_absolute_path(&self, state: &Box<dyn State>) -> Result<PathBuf> {
        let content_id = self.content_id(state)?.context(format!(
            "Path {} must match to a content_id",
            self.previous_db_path.display()
        ))?;
        let content_path = state
            .path(content_id)
            .context(format!("Get content {} path", content_id))?
            .context(format!("Expect content {} path", content_id))?
            .to_path_buf();
        Ok(self.workspace_folder.join(content_path))
    }

    fn after_absolute_path(&self) -> Result<PathBuf> {
        Ok(self.workspace_folder.join(&self.after_disk_path))
    }

    fn after_file_name(&self) -> Result<ContentFileName> {
        let after_absolute_path = self.after_absolute_path()?;
        Ok(ContentFileName(
            after_absolute_path
                .file_name()
                .context(format!(
                    "Get file name of {}",
                    after_absolute_path.display()
                ))?
                .to_str()
                .context(format!(
                    "Decode file name of {}",
                    after_absolute_path.display()
                ))?
                .to_string(),
        ))
    }

    fn before_file_name(&self, state: &Box<dyn State>) -> Result<ContentFileName> {
        let content_id = self.content_id(state)?.context(format!(
            "Path {} must match to a content_id",
            self.previous_db_path.display()
        ))?;
        Ok(state
            .get(content_id)?
            .context(format!("Get content {}", content_id))?
            .file_name()
            .clone())
    }

    fn before_revision_id(&self, state: &Box<dyn State>) -> Result<RevisionId> {
        let content_id = self.content_id(state)?.context(format!(
            "Path {} must match to a content_id",
            self.previous_db_path.display()
        ))?;
        Ok(state
            .get(content_id)?
            .context(format!("Get content {}", content_id))?
            .revision_id()
            .clone())
    }

    fn after_parent(&self, state: &Box<dyn State>) -> Result<Option<ContentId>> {
        if let Some(parent_path) = self.after_disk_path.parent() {
            return state
                .content_id_for_path(parent_path.to_path_buf())
                .context(format!("Search content for path {}", parent_path.display()))
                .context(format!(
                    "Expect a parent content for parent path {}",
                    parent_path.display(),
                ));
        }

        Ok(None)
    }

    fn before_content_type(&self, state: &Box<dyn State>) -> Result<ContentType> {
        Ok(ContentType::from_path(&self.before_absolute_path(state)?))
    }

    fn after_content_type(&self, _state: &Box<dyn State>) -> Result<ContentType> {
        Ok(ContentType::from_path(&self.after_absolute_path()?))
    }

    fn last_modified(&self) -> Result<DiskTimestamp> {
        let absolute_path = self.after_absolute_path()?;
        let since_epoch = last_modified_timestamp(&absolute_path)?;
        Ok(DiskTimestamp(since_epoch.as_millis() as u64))
    }
}

impl Executor for NamedOnRemoteExecutor {
    fn execute(
        &self,
        state: &Box<dyn State>,
        tracim: &Box<dyn TracimClient>,
    ) -> Result<StateModification> {
        let before_absolute_path = self.before_absolute_path(state)?;
        let after_absolute_path = self.after_absolute_path()?;
        let before_file_name = self.before_file_name(state)?;
        let after_file_name = self.after_file_name()?;
        let after_parent = self.after_parent(state)?;
        let before_content_type = self.before_content_type(state)?;
        let after_content_type = self.after_content_type(state)?;
        let mut revision_id = self.before_revision_id(state)?;
        let content_id = self.content_id(state)?.context(format!(
            "Path {} must match to a content_id",
            self.previous_db_path.display()
        ))?;

        if before_content_type != after_content_type {
            todo!()
        }

        if before_file_name != after_file_name {
            revision_id = tracim
                .set_label(content_id, after_content_type, after_file_name.clone())
                .context(format!("Set new label on remote for {}", content_id))?;
        }

        if after_absolute_path.parent() != before_absolute_path.parent() {
            revision_id = tracim.set_parent(content_id, after_content_type, after_parent)?;
        }

        let last_modified = self.last_modified().context(format!(
            "Get last modified datetime of {}",
            after_absolute_path.display()
        ))?;

        Ok(StateModification::Update(
            content_id,
            after_file_name,
            revision_id,
            after_parent,
            last_modified,
        ))
    }
}
