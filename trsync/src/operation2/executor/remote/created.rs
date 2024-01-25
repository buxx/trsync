use anyhow::{Context, Result};
use std::{fs, path::PathBuf};
use tempfile::NamedTempFile;

use trsync_core::{
    client::{ParentIdParameter, TracimClient, TracimClientError},
    content::Content,
    instance::{ContentFileName, ContentId, DiskTimestamp},
    types::ContentType,
    utils::md5_file,
};

use crate::{
    event::{remote::RemoteEvent, Event},
    operation2::executor::{Executor, ExecutorError},
    state::{modification::StateModification, State},
    util::last_modified_timestamp,
};

pub struct CreatedOnRemoteExecutor {
    workspace_folder: PathBuf,
    path: PathBuf,
    avoid_same_sums: bool,
}

impl CreatedOnRemoteExecutor {
    pub fn new(workspace_folder: PathBuf, path: PathBuf) -> Self {
        Self {
            workspace_folder,
            path,
            avoid_same_sums: false,
        }
    }

    pub fn avoid_same_sums(mut self, value: bool) -> Self {
        self.avoid_same_sums = value;
        self
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

    fn parent(&self, state: &dyn State) -> Result<Option<ContentId>> {
        if let Some(parent_path) = self.path.parent() {
            return state
                .content_id_for_path(parent_path.to_path_buf())
                .context(format!("Search content for path {}", parent_path.display()));
        }

        Ok(None)
    }

    fn content_type(&self) -> ContentType {
        ContentType::from_path(&self.absolute_path())
    }
}

impl Executor for CreatedOnRemoteExecutor {
    fn execute(
        &self,
        state: &dyn State,
        tracim: &dyn TracimClient,
        ignore_events: &mut Vec<Event>,
    ) -> Result<Vec<StateModification>, ExecutorError> {
        let absolute_path = self.absolute_path();
        let file_name = ContentFileName(self.file_name()?);
        let parent = self.parent(state)?;
        let content_type = self.content_type();

        let mut previously_exist = false;
        let content_id = match tracim.create_content(
            file_name.clone(),
            content_type,
            parent,
            absolute_path.clone(),
        ) {
            Ok(content_id) => {
                ignore_events.push(Event::Remote(RemoteEvent::Created(content_id)));
                content_id
            }
            Err(TracimClientError::ContentAlreadyExist) => {
                previously_exist = true;
                tracim
                    .find_one(
                        &file_name,
                        parent.map_or(ParentIdParameter::Root, ParentIdParameter::Some),
                    )
                    .context(format!(
                        "Search already existing content id for name {} ({:?})",
                        &file_name.0, parent
                    ))?
                    .context(format!(
                    "After receive ContentAlreadyExist error, content is expected for {} ({:?})",
                    &file_name.0, parent
                ))?
            }
            Err(error) => return Err(ExecutorError::Tracim(error)),
        };

        if content_type.fillable() {
            let do_update = if previously_exist {
                match content_type {
                    ContentType::File => {
                        let remote_content_file = NamedTempFile::new().context(format!(
                            "Create temporary file to download content {}",
                            &content_id.0,
                        ))?;
                        let remote_content_file_path = remote_content_file.into_temp_path();
                        let remote_content_file_path_buf = remote_content_file_path.to_path_buf();
                        tracim.fill_file_with_content(
                            content_id,
                            content_type,
                            &remote_content_file_path_buf,
                        )?;
                        let same =
                            md5_file(&remote_content_file_path_buf) == md5_file(&absolute_path);
                        remote_content_file_path.close().context(format!(
                            "Close created temporary file {}",
                            &remote_content_file_path_buf.display(),
                        ))?;
                        !same
                    }
                    ContentType::HtmlDocument => {
                        let remote_content = tracim.get_content(content_id)?;
                        let local_content_raw =
                            fs::read_to_string(&absolute_path).map_err(|err| {
                                ExecutorError::RelatedLocalFileIoError(absolute_path.clone(), err)
                            })?;
                        Some(local_content_raw) != remote_content.raw_content
                    }
                    ContentType::Folder => false,
                }
            } else {
                true
            };

            if do_update {
                tracim
                    .fill_content_with_file(content_id, content_type, &absolute_path)
                    .context(format!(
                        "Fill content {} after created it with file {}",
                        content_id,
                        absolute_path.display()
                    ))?;
                ignore_events.push(Event::Remote(RemoteEvent::Updated(content_id)));
            }
        }

        let content = Content::from_remote(
            &tracim
                .get_content(content_id)
                .context(format!("Get just created content {}", content_id))?,
        )?;
        let disk_timestamp = last_modified_timestamp(&absolute_path)
            .context(format!("Get disk timestamp of {}", absolute_path.display()))?;
        Ok(vec![StateModification::Add(
            content,
            self.path.clone(),
            DiskTimestamp(disk_timestamp.as_millis() as u64),
        )])
    }
}
