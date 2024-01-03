use std::{fs, path::PathBuf};

use anyhow::{Context, Result};
use trsync_core::{
    client::TracimClient,
    content::Content,
    instance::{ContentId, DiskTimestamp},
    types::ContentType,
};

use crate::{
    event::Event,
    local::DiskEvent,
    local2::reducer::DiskEventWrap,
    operation2::executor::Executor,
    state::{modification::StateModification, State},
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
        state: &Box<dyn State>,
        tracim: &Box<dyn TracimClient>,
        ignore_events: &mut Vec<Event>,
    ) -> Result<Vec<StateModification>> {
        let content = Content::from_remote(
            &tracim
                .get_content(self.content_id)
                .context(format!("Get content {}", self.content_id))?,
        )?;

        // FIXME BS NOW : How to be sure than parent is always already present ?!
        let content_path_buf: PathBuf = if let Some(parent_id) = content.parent_id() {
            state
                .path(parent_id)
                .context(format!("Get parent {} path", parent_id))?
                .context(format!("Expect parent {} path", parent_id))?
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
                fs::create_dir_all(&absolute_path)
                    .context(format!("Create folder {}", absolute_path.display()))?;

                let mut current = content_path_buf.clone();
                while let Some(parent) = current.parent() {
                    current = parent.to_path_buf();
                    if !current.exists() {
                        ignore_events.push(Event::Local(DiskEventWrap::new(
                            current.clone(),
                            DiskEvent::Created(current.clone()),
                        )))
                    }
                }
            }
            ContentType::File => {
                fs::File::create(&absolute_path)
                    .context(format!("Create file {}", absolute_path.display()))?;
                tracim
                    .fill_file_with_content(self.content_id, ContentType::File, &absolute_path)
                    .context(format!("Write into file {}", absolute_path.display()))?;

                ignore_events.push(Event::Local(DiskEventWrap::new(
                    content_path_buf.clone(),
                    DiskEvent::Modified(content_path_buf.clone()),
                )))
            }
            ContentType::HtmlDocument => todo!(),
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
