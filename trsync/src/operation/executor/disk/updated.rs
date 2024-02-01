use std::{fs, path::PathBuf};

use anyhow::{Context, Result};
use trsync_core::{
    client::TracimClient,
    content::Content,
    instance::{ContentId, DiskTimestamp},
};

use crate::{
    event::Event,
    local2::reducer::DiskEventWrap,
    local2::watcher::DiskEvent,
    operation::executor::{Executor, ExecutorError},
    state::{modification::StateModification, State},
    util::last_modified_timestamp,
};

pub struct UpdatedOnDiskExecutor {
    workspace_folder: PathBuf,
    content_id: ContentId,
    download: bool,
}

impl UpdatedOnDiskExecutor {
    pub fn new(workspace_folder: PathBuf, content_id: ContentId, download: bool) -> Self {
        Self {
            workspace_folder,
            content_id,
            download,
        }
    }
}

impl Executor for UpdatedOnDiskExecutor {
    fn execute(
        &self,
        state: &dyn State,
        tracim: &dyn TracimClient,
        ignore_events: &mut Vec<Event>,
    ) -> Result<Vec<StateModification>, ExecutorError> {
        let local_content_path = state
            .path(self.content_id)
            .context(format!("Get local content {} path", self.content_id))?
            .to_path_buf();
        let remote_content = Content::from_remote(
            &tracim
                .get_content(self.content_id)
                .context(format!("Get remote content {}", self.content_id))?,
        )?;
        let remote_content_path: PathBuf = if let Some(parent_id) = remote_content.parent_id() {
            state
                .path(parent_id)
                .context(format!("Get parent {} path", parent_id))?
                .to_path_buf()
                .join(PathBuf::from(remote_content.file_name().to_string()))
        } else {
            PathBuf::from(remote_content.file_name().to_string())
        };
        let previous_absolute_path = self.workspace_folder.join(&local_content_path);
        let new_absolute_path = self.workspace_folder.join(&remote_content_path);

        if previous_absolute_path != new_absolute_path {
            fs::rename(&previous_absolute_path, &new_absolute_path).context(format!(
                "Move {} to {}",
                previous_absolute_path.display(),
                new_absolute_path.display()
            ))?;
            ignore_events.push(Event::Local(DiskEventWrap::new(
                local_content_path.clone(),
                DiskEvent::Renamed(local_content_path.clone(), remote_content_path.clone()),
            )));
        }

        if self.download {
            tracim
                .fill_file_with_content(
                    self.content_id,
                    *remote_content.type_(),
                    &new_absolute_path,
                )
                .context(format!(
                    "Fill {} with content {}",
                    new_absolute_path.display(),
                    self.content_id
                ))?;

            ignore_events.push(Event::Local(DiskEventWrap::new(
                local_content_path.clone(),
                DiskEvent::Modified(local_content_path.clone()),
            )))
        }

        let disk_timestamp = DiskTimestamp(
            last_modified_timestamp(&new_absolute_path)
                .context(format!(
                    "Get disk timestamp of {}",
                    new_absolute_path.display()
                ))?
                .as_millis() as u64,
        );

        Ok(vec![StateModification::Update(
            self.content_id,
            remote_content.file_name().clone(),
            remote_content.revision_id(),
            remote_content.parent_id(),
            disk_timestamp,
        )])
    }
}
