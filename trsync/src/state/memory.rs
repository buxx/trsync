use anyhow::{bail, Context, Result as AnyhowResult};
use std::{cmp::Ordering, collections::HashMap, path::PathBuf};

use trsync_core::{
    content::Content,
    instance::{ContentFileName, ContentId, DiskTimestamp, RevisionId},
    types::ContentType,
};

use crate::path::ContentPath;

use super::{State, StateError};

pub struct MemoryState {
    contents: HashMap<ContentId, Content>,
    timestamps: HashMap<ContentId, DiskTimestamp>,
}

impl MemoryState {
    pub fn new(
        contents: HashMap<ContentId, Content>,
        timestamps: HashMap<ContentId, DiskTimestamp>,
    ) -> AnyhowResult<Self> {
        for content in contents.values() {
            if let Some(parent_id) = content.parent_id() {
                if !contents.contains_key(&parent_id) {
                    bail!(format!(
                        "Content {} is absent (parent of {})",
                        parent_id,
                        content.id()
                    ))
                }
            }
        }

        Ok(Self {
            contents,
            timestamps,
        })
    }
}

impl State for MemoryState {
    fn known(&self, id: ContentId) -> AnyhowResult<bool> {
        Ok(self.contents.contains_key(&id))
    }

    fn get(&self, id: ContentId) -> AnyhowResult<Option<Content>> {
        Ok(self.contents.get(&id).cloned())
    }

    fn content_id_for_path(&self, path: PathBuf) -> AnyhowResult<Option<ContentId>> {
        // TODO : cache a hashmap with all paths instead compute it here
        for content in self.contents.values() {
            let content_path = self
                .path(content.id())
                .context(format!("Get par for content {}", content.id()))?;
            if content_path.to_path_buf() == path {
                return Ok(Some(content.id()));
            }
        }

        Ok(None)
    }

    // Path must be build on demand because parent hierarchy can change
    fn path(&self, id: ContentId) -> AnyhowResult<ContentPath, StateError> {
        let content = self
            .contents
            .get(&id)
            .context(format!("Search content {} in state", id))?;
        let mut parts = vec![content.clone()];

        let mut current = content;
        while let Some(parent_id) = current.parent_id() {
            let parent = self
                .contents
                .get(&parent_id)
                .context(format!("Search content {} in state", id))?;
            parts.insert(0, parent.clone());
            current = parent;
        }

        Ok(ContentPath::new(parts))
    }

    // TODO : Iter
    fn contents(&self) -> AnyhowResult<Vec<Content>> {
        // TODO : Risky for memory overload
        let mut contents = self.contents.values().cloned().collect::<Vec<Content>>();
        contents.sort_by(|a, b| match (a.type_(), b.type_()) {
            (ContentType::Folder, ContentType::Folder) => Ordering::Equal,
            (_, ContentType::Folder) => Ordering::Greater,
            (ContentType::Folder, _) => Ordering::Less,
            (_, _) => Ordering::Less,
        });
        Ok(contents)
    }

    // TODO : Iter
    fn direct_children_ids(&self, content_id: ContentId) -> AnyhowResult<Vec<ContentId>> {
        Ok(self
            .contents
            .values()
            .filter(|content| content.parent_id() == Some(content_id))
            .map(|content| content.id())
            .collect::<Vec<ContentId>>())
    }

    fn forgot(&mut self, content_id: ContentId) -> AnyhowResult<()> {
        self.contents
            .remove(&content_id)
            .context(format!("Remove content {} from state", content_id))?;
        Ok(())
    }

    fn add(
        &mut self,
        content: Content,
        _: PathBuf,
        timestamp: DiskTimestamp,
    ) -> Result<(), StateError> {
        self.timestamps.insert(content.id(), timestamp);
        self.contents.insert(content.id(), content);

        Ok(())
    }

    fn update(
        &mut self,
        content_id: ContentId,
        file_name: ContentFileName,
        revision_id: RevisionId,
        parent_id: Option<ContentId>,
        timestamp: DiskTimestamp,
    ) -> AnyhowResult<()> {
        let content = self
            .contents
            .get_mut(&content_id)
            .context(format!("Get content {}", content_id))?;

        content.set_revision_id(revision_id);
        content.set_parent_id(parent_id);
        content.set_file_name(file_name);
        self.timestamps.insert(content_id, timestamp);

        Ok(())
    }
}
