use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use trsync_core::{
    client::TracimClient,
    content::Content,
    instance::{ContentId, DiskTimestamp},
    types::ContentType,
};

use crate::{
    event::Event,
    local::reducer::DiskEventWrap,
    local::watcher::DiskEvent,
    operation::executor::{Executor, ExecutorError},
    state::{modification::StateModification, State, StateError},
    util::last_modified_timestamp,
};

pub struct PresentOnDiskExecutor {
    workspace_folder: PathBuf,
    content_id: ContentId,
}

impl PresentOnDiskExecutor {
    pub fn new(workspace_folder: PathBuf, content_id: ContentId) -> Self {
        Self {
            workspace_folder,
            content_id,
        }
    }
}

impl Executor for PresentOnDiskExecutor {
    fn execute(
        &self,
        state: &dyn State,
        tracim: &dyn TracimClient,
        ignore_events: &mut Vec<Event>,
    ) -> Result<Vec<StateModification>, ExecutorError> {
        let content = Content::from_remote(
            &tracim
                .get_content(self.content_id)
                .context(format!("Get content {}", self.content_id))?,
        )?;

        let content_path_buf: PathBuf = if let Some(parent_id) = content.parent_id() {
            match state.path(parent_id) {
                Ok(path) => path,
                Err(StateError::UnknownContent(_)) => {
                    return Err(ExecutorError::MissingParent(self.content_id, parent_id))
                }
                Err(e) => return Err(ExecutorError::from(e)),
            }
            .to_path_buf()
            .join(PathBuf::from(content.file_name().to_string()))
        } else {
            PathBuf::from(content.file_name().to_string())
        };
        let absolute_path = self.workspace_folder.join(&content_path_buf);

        if !absolute_path.exists() {
            ignore_events.push(Event::Local(DiskEventWrap::new(
                content_path_buf.clone(),
                DiskEvent::Created(content_path_buf.clone()),
            )))
        }

        match content.type_() {
            ContentType::Folder => {
                let mut current = content_path_buf.clone();
                while let Some(parent) = current.parent() {
                    // FIXME BS NOW : qd pas de parent obtient chaine vide !!
                    current = parent.to_path_buf();
                    if !current.exists() {
                        ignore_events.push(Event::Local(DiskEventWrap::new(
                            current.clone(),
                            DiskEvent::Created(current.clone()),
                        )))
                    }
                }

                match fs::create_dir_all(&absolute_path) {
                    Ok(_) => {}
                    Err(error) => {
                        // TODO : Seems difficult to ensure which type of error
                        if !Path::new(&absolute_path).exists() {
                            return Err(ExecutorError::Unexpected2(format!(
                                "Error during folder '{}' creation: {}",
                                absolute_path.display(),
                                error
                            )));
                        }
                    }
                };
            }
            ContentType::File | ContentType::HtmlDocument => {
                let exist = absolute_path.exists();
                fs::File::create(&absolute_path)
                    .context(format!("Create file {}", absolute_path.display()))?;
                if !exist {
                    ignore_events.push(Event::Local(DiskEventWrap::new(
                        content_path_buf.clone(),
                        DiskEvent::Created(content_path_buf.clone()),
                    )))
                }

                tracim
                    .fill_file_with_content(self.content_id, ContentType::File, &absolute_path)
                    .context(format!("Write into file {}", absolute_path.display()))?;
                ignore_events.push(Event::Local(DiskEventWrap::new(
                    content_path_buf.clone(),
                    DiskEvent::Modified(content_path_buf.clone()),
                )))
            }
        }

        let disk_timestamp = last_modified_timestamp(&absolute_path)
            .context(format!("Get disk timestamp of {}", absolute_path.display()))?;
        Ok(vec![StateModification::Add(
            content,
            content_path_buf,
            DiskTimestamp(disk_timestamp.as_millis() as u64),
        )])
    }
}
