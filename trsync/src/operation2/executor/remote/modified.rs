use std::path::PathBuf;

use anyhow::{Context, Result};

use trsync_core::{
    client::{TracimClient, TracimClientError},
    content::Content,
    instance::{ContentId, DiskTimestamp},
    types::ContentType,
};

use crate::{
    event::{remote::RemoteEvent, Event},
    operation2::executor::{Executor, ExecutorError},
    state::{modification::StateModification, State},
    util::last_modified_timestamp,
};

pub struct ModifiedOnRemoteExecutor {
    workspace_folder: PathBuf,
    db_path: PathBuf,
    disk_path: PathBuf,
}

impl ModifiedOnRemoteExecutor {
    pub fn new(workspace_folder: PathBuf, db_path: PathBuf, disk_path: PathBuf) -> Self {
        Self {
            workspace_folder,
            db_path,
            disk_path,
        }
    }

    fn content_id(&self, state: &Box<dyn State>) -> Result<Option<ContentId>> {
        state
            .content_id_for_path(self.db_path.clone())
            .context(format!("Get content_id for {}", self.db_path.display()))
    }

    fn absolute_path(&self, state: &Box<dyn State>) -> Result<PathBuf> {
        let content_id = self.content_id(state)?.context(format!(
            "Path {} must match to a content_id",
            self.db_path.display()
        ))?;
        let content_path = state
            .path(content_id)
            .context(format!("Get content {} path", content_id))?
            .to_path_buf();
        Ok(self.workspace_folder.join(content_path))
    }

    fn content_type(&self, state: &Box<dyn State>) -> Result<ContentType> {
        let content_id = self.content_id(state)?.context(format!(
            "Path {} must match to a content_id",
            self.db_path.display()
        ))?;
        let content = state
            .get(content_id)
            .context(format!("Get content {}", content_id))?
            .context(format!("Expected content {}", content_id))?;
        Ok(*content.type_())
    }

    fn last_modified(&self, state: &Box<dyn State>) -> Result<DiskTimestamp> {
        let absolute_path = self.absolute_path(state)?;
        let since_epoch = last_modified_timestamp(&absolute_path)?;
        Ok(DiskTimestamp(since_epoch.as_millis() as u64))
    }

    fn update_content(
        &self,
        tracim: &Box<dyn TracimClient>,
        content_id: ContentId,
        content_type: ContentType,
        absolute_path: &PathBuf,
        ignore_events: &mut Vec<Event>,
    ) -> Result<(), TracimClientError> {
        tracim
            .fill_content_with_file(content_id, content_type, &absolute_path)
            .context(format!(
                "Fill remote file {} with {}",
                content_id,
                absolute_path.display(),
            ))?;
        ignore_events.push(Event::Remote(RemoteEvent::Updated(content_id)));
        Ok(())
    }

    fn restore_content(
        &self,
        tracim: &Box<dyn TracimClient>,
        content_id: ContentId,
        ignore_events: &mut Vec<Event>,
    ) -> Result<(), TracimClientError> {
        tracim
            .restore_content(content_id)
            .context(format!("Restore remote file {}", content_id,))?;
        ignore_events.push(Event::Remote(RemoteEvent::Created(content_id)));
        Ok(())
    }
}

impl Executor for ModifiedOnRemoteExecutor {
    fn execute(
        &self,
        state: &Box<dyn State>,
        tracim: &Box<dyn TracimClient>,
        ignore_events: &mut Vec<Event>,
    ) -> Result<Vec<StateModification>, ExecutorError> {
        let absolute_path = self.absolute_path(state)?;
        let content_type = self.content_type(state)?;
        let content_id = self.content_id(state)?.context(format!(
            "Path {} must match to a content_id",
            self.db_path.display()
        ))?;

        if content_type.fillable() {
            if let Err(TracimClientError::ContentDeletedOrArchived) = self.update_content(
                tracim,
                content_id,
                content_type,
                &absolute_path,
                ignore_events,
            ) {
                // TODO : manage archived case
                self.restore_content(tracim, content_id, ignore_events)?;
                self.update_content(
                    tracim,
                    content_id,
                    content_type,
                    &absolute_path,
                    ignore_events,
                )?;
            }
        }

        let content = Content::from_remote(
            &tracim
                .get_content(content_id)
                .context(format!("Get just created content {}", content_id))?,
        )?;
        let last_modified = self.last_modified(state).context(format!(
            "Get last modified datetime of {}",
            absolute_path.display()
        ))?;

        Ok(vec![StateModification::Update(
            content.id(),
            content.file_name().clone(),
            content.revision_id(),
            content.parent_id(),
            last_modified,
        )])
    }
}
