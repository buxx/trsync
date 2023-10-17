use crate::{
    client::RemoteContent,
    instance::{ContentFileName, ContentId, RevisionId},
};
use anyhow::{bail, Context, Result};

use crate::types::ContentType;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Content {
    id: ContentId,
    revision_id: RevisionId,
    file_name: ContentFileName,
    parent_id: Option<ContentId>,
    type_: ContentType,
}

impl Content {
    pub fn new(
        id: ContentId,
        revision_id: RevisionId,
        file_name: ContentFileName,
        parent_id: Option<ContentId>,
        type_: ContentType,
    ) -> Result<Self> {
        if let Some(parent_id) = parent_id {
            if parent_id == id {
                bail!(format!("Content {} parent_id cannot reference itself", id))
            }
        }

        Ok(Self {
            id,
            revision_id,
            file_name,
            parent_id,
            type_,
        })
    }

    pub fn from_remote(value: &RemoteContent) -> Result<Self> {
        Self::new(
            value.content_id,
            value.current_revision_id,
            ContentFileName(value.filename.clone()),
            value.parent_id.map(ContentId),
            ContentType::from_str(&value.content_type).context(format!(
                "Cast content type for {} from {}",
                value.content_id, value.content_type
            ))?,
        )
        .context(format!(
            "Cast remote content {} into content",
            value.content_id
        ))
    }

    pub fn id(&self) -> ContentId {
        self.id
    }

    pub fn revision_id(&self) -> RevisionId {
        self.revision_id
    }

    pub fn file_name(&self) -> &ContentFileName {
        &self.file_name
    }

    pub fn parent_id(&self) -> Option<ContentId> {
        self.parent_id
    }

    pub fn type_(&self) -> &ContentType {
        &self.type_
    }

    pub fn set_parent_id(&mut self, parent_id: Option<ContentId>) {
        self.parent_id = parent_id;
    }

    pub fn set_file_name(&mut self, file_name: ContentFileName) {
        self.file_name = file_name;
    }

    pub fn set_revision_id(&mut self, revision_id: RevisionId) {
        self.revision_id = revision_id;
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_content_fail_because_same_parent() {
        assert!(Content::new(
            ContentId(42),
            RevisionId(42),
            ContentFileName("toto".into()),
            Some(ContentId(42)),
            ContentType::File,
        )
        .is_err())
    }
}
