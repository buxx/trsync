use std::collections::HashMap;

use anyhow::{Context, Result};
use trsync_core::{
    client::{RemoteContent, TracimClient},
    content::Content,
    instance::ContentId,
};

use crate::state::memory::MemoryState;

pub struct RemoteSync {
    client: Box<dyn TracimClient>,
}

impl RemoteSync {
    pub fn new(client: Box<dyn TracimClient>) -> Self {
        Self { client }
    }

    pub fn state(&self) -> Result<MemoryState> {
        let mut contents = HashMap::new();
        let all_remote_contents = self.all_remote_contents()?;

        for remote_content in &all_remote_contents {
            if !self
                .is_deleted(remote_content, &all_remote_contents)
                .context(format!(
                    "Try to determine if {} is deleted",
                    remote_content.content_id
                ))?
            {
                let content: Content = Content::from_remote(remote_content)?;
                contents.insert(content.id(), content);
            }
        }

        MemoryState::new(contents, HashMap::new())
            .context("Build memory state from remote contents")
    }

    fn all_remote_contents(&self) -> Result<Vec<RemoteContent>> {
        self.client
            .get_contents()
            .context("Read contents from remote")
    }

    fn is_deleted(
        &self,
        remote_content: &RemoteContent,
        all_remote_contents: &[RemoteContent],
    ) -> Result<bool> {
        if remote_content.is_deleted || remote_content.is_archived {
            return Ok(true);
        }

        let mut current = remote_content;
        while let Some(parent_id) = current.parent_id {
            current = all_remote_contents
                .iter()
                .find(|c| c.content_id == ContentId(parent_id))
                .context(format!(
                    "Find parent {} of content {}",
                    parent_id, current.content_id
                ))?;

            if current.is_deleted || current.is_archived {
                return Ok(true);
            }
        }

        Ok(false)
    }
}

#[cfg(test)]
mod test {
    use trsync_core::{client::MockTracimClient, instance::RevisionId};

    use crate::state::State;

    use super::*;

    #[test]
    fn test_empty() {
        // Given
        let mut client = MockTracimClient::new();
        client
            .expect_get_contents()
            .times(1)
            .returning(|| Ok(vec![]));
        let remote_sync = RemoteSync::new(Box::new(client));

        // When
        let state = remote_sync.state().unwrap();

        // Then
        assert_eq!(state.contents().unwrap(), vec![])
    }

    #[test]
    fn test_flat() {
        // Given
        let mut client = MockTracimClient::new();
        client.expect_get_contents().times(1).returning(|| {
            Ok(vec![
                RemoteContent {
                    content_id: ContentId(1),
                    current_revision_id: RevisionId(1),
                    parent_id: None,
                    content_type: "file".to_string(),
                    modified: "".to_string(),
                    raw_content: None,
                    filename: "a.txt".to_string(),
                    is_deleted: false,
                    is_archived: false,
                    sub_content_types: vec![],
                },
                RemoteContent {
                    content_id: ContentId(2),
                    current_revision_id: RevisionId(2),
                    parent_id: None,
                    content_type: "file".to_string(),
                    modified: "".to_string(),
                    raw_content: None,
                    filename: "b.txt".to_string(),
                    is_deleted: false,
                    is_archived: false,
                    sub_content_types: vec![],
                },
            ])
        });
        let remote_sync = RemoteSync::new(Box::new(client));

        // When
        let state = remote_sync.state().unwrap();

        // Then
        let contents = state.contents().unwrap();
        assert_eq!(contents.len(), 2);
        assert_eq!(contents[0].id(), ContentId(1));
        assert_eq!(contents[1].id(), ContentId(2));
    }

    #[test]
    fn test_tree() {
        // Given
        let mut client = MockTracimClient::new();
        client.expect_get_contents().times(1).returning(|| {
            Ok(vec![
                RemoteContent {
                    content_id: ContentId(1),
                    current_revision_id: RevisionId(1),
                    parent_id: None,
                    content_type: "folder".to_string(),
                    modified: "".to_string(),
                    raw_content: None,
                    filename: "Folder".to_string(),
                    is_deleted: false,
                    is_archived: false,
                    sub_content_types: vec![],
                },
                RemoteContent {
                    content_id: ContentId(2),
                    current_revision_id: RevisionId(2),
                    parent_id: Some(1),
                    content_type: "file".to_string(),
                    modified: "".to_string(),
                    raw_content: None,
                    filename: "a.txt".to_string(),
                    is_deleted: false,
                    is_archived: false,
                    sub_content_types: vec![],
                },
            ])
        });
        let remote_sync = RemoteSync::new(Box::new(client));

        // When
        let state = remote_sync.state().unwrap();

        // Then
        let contents = state.contents().unwrap();
        assert_eq!(contents.len(), 2);
        assert_eq!(contents[0].id(), ContentId(1));
        assert_eq!(contents[1].id(), ContentId(2));
    }

    #[test]
    fn test_flat_with_deleted() {
        // Given
        let mut client = MockTracimClient::new();
        client.expect_get_contents().times(1).returning(|| {
            Ok(vec![
                RemoteContent {
                    content_id: ContentId(1),
                    current_revision_id: RevisionId(1),
                    parent_id: None,
                    content_type: "file".to_string(),
                    modified: "".to_string(),
                    raw_content: None,
                    filename: "a.txt".to_string(),
                    is_deleted: false,
                    is_archived: false,
                    sub_content_types: vec![],
                },
                RemoteContent {
                    content_id: ContentId(2),
                    current_revision_id: RevisionId(2),
                    parent_id: None,
                    content_type: "file".to_string(),
                    modified: "".to_string(),
                    raw_content: None,
                    filename: "b.txt".to_string(),
                    is_deleted: true,
                    is_archived: false,
                    sub_content_types: vec![],
                },
            ])
        });
        let remote_sync = RemoteSync::new(Box::new(client));

        // When
        let state = remote_sync.state().unwrap();

        // Then
        let contents = state.contents().unwrap();
        assert_eq!(contents.len(), 1);
        assert_eq!(contents[0].id(), ContentId(1));
    }

    #[test]
    fn test_tree_with_parent_deleted() {
        // Given
        let mut client = MockTracimClient::new();
        client.expect_get_contents().times(1).returning(|| {
            Ok(vec![
                RemoteContent {
                    content_id: ContentId(1),
                    current_revision_id: RevisionId(1),
                    parent_id: None,
                    content_type: "folder".to_string(),
                    modified: "".to_string(),
                    raw_content: None,
                    filename: "Folder".to_string(),
                    is_deleted: true,
                    is_archived: false,
                    sub_content_types: vec![],
                },
                RemoteContent {
                    content_id: ContentId(2),
                    current_revision_id: RevisionId(2),
                    parent_id: Some(1),
                    content_type: "file".to_string(),
                    modified: "".to_string(),
                    raw_content: None,
                    filename: "a.txt".to_string(),
                    is_deleted: false,
                    is_archived: false,
                    sub_content_types: vec![],
                },
            ])
        });
        let remote_sync = RemoteSync::new(Box::new(client));

        // When
        let state = remote_sync.state().unwrap();

        // Then
        let contents = state.contents().unwrap();
        assert_eq!(contents.len(), 0);
    }
}
