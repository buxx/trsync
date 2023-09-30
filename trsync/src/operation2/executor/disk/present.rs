use std::{fs, path::PathBuf};

use anyhow::{Context, Result};
use trsync_core::{client::TracimClient, instance::ContentId, types::ContentType};

use crate::{
    operation2::executor::Executor,
    state::{modification::StateModification, State},
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
    ) -> Result<StateModification> {
        let content = tracim
            .get_content(self.content_id)
            .context(format!("Get content {}", self.content_id))?;

        // FIXME BS NOW : How to be sure than parent is always already present ?!
        let content_path: PathBuf = if let Some(parent_id) = content.parent_id() {
            state
                .path(parent_id)
                .context(format!("Get parent {} path", parent_id))?
                .to_path_buf()
                .join(PathBuf::from(content.file_name().to_string()))
        } else {
            PathBuf::from(content.file_name().to_string())
        };
        let absolute_path = self.workspace_folder.join(content_path);

        match content.type_() {
            ContentType::Folder => fs::create_dir_all(&absolute_path)
                .context(format!("Create folder {}", absolute_path.display()))?,
            ContentType::File => {
                // FIXME BS NOW : fill for real (by giving file path to client to write into ?)
                // FIXME BS NOW : fill file & manage if already exist as ok
                fs::File::create(&absolute_path)
                    .context(format!("Create file {}", absolute_path.display()))?;
                tracim
                    .fill_file_with_content(self.content_id, &absolute_path)
                    .context(format!("Write into file {}", absolute_path.display()))?;
            }
            ContentType::HtmlDocument => todo!(),
        }

        Ok(StateModification::Add(content))
    }
}
