use std::{collections::HashMap, path::PathBuf};

use anyhow::{bail, Context, Result};
use rusqlite::{params, Connection};
use trsync_core::{
    change::remote::RemoteChange,
    client::{RemoteContent, TracimClient},
    content::Content,
    instance::{ContentId, RevisionId},
};

use crate::state::{memory::MemoryState, State};

pub struct RemoteSync {
    connection: Connection,
    client: Box<dyn TracimClient>,
}

impl RemoteSync {
    pub fn new(connection: Connection, client: Box<dyn TracimClient>) -> Self {
        Self { connection, client }
    }

    fn state(&self) -> Result<MemoryState> {
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

    pub fn changes(&self) -> Result<Vec<RemoteChange>> {
        let mut changes = vec![];
        let remote_state = self.state().context("Determine remote state")?;

        for content in remote_state.contents()? {
            let path = remote_state
                .path(content.id())
                .context(format!(
                    "Determine path for remotely modified content {}",
                    content.id()
                ))?
                .to_path_buf();
            if self.previously_known(content.id()).context(format!(
                "Determine if content {} is previously known",
                content.id()
            ))? {
                if content.revision_id()
                    != self.known_revision_id(content.id()).context(format!(
                        "Read previously known content {} revision_id",
                        content.id()
                    ))?
                {
                    changes.push(RemoteChange::Updated(content.id(), path));
                }
            } else {
                changes.push(RemoteChange::New(content.id(), path));
            }
        }

        for content_id in self
            .previously_known_content_ids()
            .context("Read previously known content ids")?
        {
            let path = self.previously_known_path(content_id).context(format!(
                "Determine previously known path for content {}",
                content_id
            ))?;
            if !remote_state.known(content_id).context(format!(
                "Check if previously known content {} is known in remote state",
                content_id
            ))? {
                changes.push(RemoteChange::Disappear(content_id, path));
            }
        }

        Ok(changes)
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
            current = match all_remote_contents
                .iter()
                .find(|c| c.content_id == ContentId(parent_id))
            {
                Some(content) => content,
                // If not found, consider content as deleted because no parent
                None => return Ok(true),
            };

            if current.is_deleted || current.is_archived {
                return Ok(true);
            }
        }

        Ok(false)
    }

    fn previously_known(&self, id: ContentId) -> Result<bool> {
        match self.connection.query_row::<u64, _, _>(
            "SELECT 1 FROM file WHERE content_id = ?",
            params![id.0],
            |row| row.get(0),
        ) {
            Ok(_) => Ok(true),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(false),
            Err(error) => bail!(error),
        }
    }

    fn known_revision_id(&self, id: ContentId) -> Result<RevisionId> {
        match self.connection.query_row::<i32, _, _>(
            "SELECT revision_id FROM file WHERE content_id = ?",
            params![id.0],
            |row| Ok(row.get(0).unwrap()),
        ) {
            Ok(id) => Ok(RevisionId(id)),
            Err(rusqlite::Error::QueryReturnedNoRows) => {
                bail!("No row found for content {}", id)
            }
            Err(error) => bail!("Read revision_id for {} from db but : {}", id, error),
        }
    }

    fn previously_known_content_ids(&self) -> Result<Vec<ContentId>> {
        let mut content_ids = vec![];

        for raw_content_id in self
            .connection
            .prepare("SELECT content_id FROM file")?
            .query_map([], |row| row.get(0))?
        {
            content_ids.push(ContentId(raw_content_id?))
        }

        Ok(content_ids)
    }

    fn previously_known_path(&self, id: ContentId) -> Result<PathBuf> {
        match self.connection.query_row::<String, _, _>(
            "SELECT relative_path FROM file WHERE content_id = ?",
            params![id.0],
            |row| Ok(row.get(0).unwrap()),
        ) {
            Ok(relative_path) => Ok(PathBuf::from(relative_path)),
            Err(rusqlite::Error::QueryReturnedNoRows) => {
                bail!("No row found for content {}", id)
            }
            Err(error) => bail!("Read relative_path for {} from db but : {}", id, error),
        }
    }
}

#[cfg(test)]
mod test {
    use trsync_core::change::remote::RemoteChange;
    use trsync_core::{client::MockTracimClient, instance::RevisionId};

    use crate::state::disk::DiskState;
    use crate::state::State;
    use crate::tests::*;

    use super::*;

    #[test]
    fn test_state_empty() {
        // Given
        let tmpdir_ = tmpdir();
        let mut client = MockTracimClient::new();
        client
            .expect_get_contents()
            .times(1)
            .returning(|| Ok(vec![]));
        let remote_sync = RemoteSync::new(connection(&tmpdir_), Box::new(client));

        // When
        let state = remote_sync.state().unwrap();

        // Then
        assert_eq!(state.contents().unwrap(), vec![])
    }

    #[test]
    fn test_state_flat() {
        // Given
        let tmpdir_ = tmpdir();
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
        let remote_sync = RemoteSync::new(connection(&tmpdir_), Box::new(client));

        // When
        let state = remote_sync.state().unwrap();

        // Then
        let contents = state.contents().unwrap();
        assert_eq!(contents.len(), 2);
    }

    #[test]
    fn test_state_tree() {
        // Given
        let tmpdir_ = tmpdir();
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
        let remote_sync = RemoteSync::new(connection(&tmpdir_), Box::new(client));

        // When
        let state = remote_sync.state().unwrap();

        // Then
        let contents = state.contents().unwrap();
        assert_eq!(contents.len(), 2);
        assert_eq!(contents[0].id(), ContentId(1));
        assert_eq!(contents[1].id(), ContentId(2));
    }

    #[test]
    fn test_state_flat_with_deleted() {
        // Given
        let tmpdir_ = tmpdir();
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
        let remote_sync = RemoteSync::new(connection(&tmpdir_), Box::new(client));

        // When
        let state = remote_sync.state().unwrap();

        // Then
        let contents = state.contents().unwrap();
        assert_eq!(contents.len(), 1);
        assert_eq!(contents[0].id(), ContentId(1));
    }

    #[test]
    fn test_state_tree_with_parent_deleted() {
        // Given
        let tmpdir_ = tmpdir();
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
        let remote_sync = RemoteSync::new(connection(&tmpdir_), Box::new(client));

        // When
        let state = remote_sync.state().unwrap();

        // Then
        let contents = state.contents().unwrap();
        assert_eq!(contents.len(), 0);
    }

    #[test]
    fn test_changes_file_no_change() {
        // Given
        let tmpdir_ = tmpdir();
        let mut client = MockTracimClient::new();
        DiskState::new(connection(&tmpdir_), tmpdir_.clone())
            .create_tables()
            .unwrap();
        client.expect_get_contents().times(1).returning(|| {
            Ok(vec![RemoteContent {
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
            }])
        });
        insert_content(&connection(&tmpdir_), "a.txt", 1, 1, None, 0);
        let remote_sync = RemoteSync::new(connection(&tmpdir_), Box::new(client));

        // When
        let changes = remote_sync.changes().unwrap();

        // Then
        assert_eq!(changes, vec![])
    }

    #[test]
    fn test_changes_file_is_new() {
        // Given
        let tmpdir_ = tmpdir();
        let mut client = MockTracimClient::new();
        DiskState::new(connection(&tmpdir_), tmpdir_.clone())
            .create_tables()
            .unwrap();
        client.expect_get_contents().times(1).returning(|| {
            Ok(vec![RemoteContent {
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
            }])
        });
        let remote_sync = RemoteSync::new(connection(&tmpdir_), Box::new(client));

        // When
        let changes = remote_sync.changes().unwrap();

        // Then
        assert_eq!(
            changes,
            vec![RemoteChange::New(ContentId(1), PathBuf::from("a.txt"))]
        )
    }

    #[test]
    fn test_changes_file_is_updated() {
        // Given
        let tmpdir_ = tmpdir();
        let mut client = MockTracimClient::new();
        DiskState::new(connection(&tmpdir_), tmpdir_.clone())
            .create_tables()
            .unwrap();
        client.expect_get_contents().times(1).returning(|| {
            Ok(vec![RemoteContent {
                content_id: ContentId(1),
                current_revision_id: RevisionId(2),
                parent_id: None,
                content_type: "file".to_string(),
                modified: "".to_string(),
                raw_content: None,
                filename: "a.txt".to_string(),
                is_deleted: false,
                is_archived: false,
                sub_content_types: vec![],
            }])
        });
        insert_content(&connection(&tmpdir_), "a.txt", 1, 1, None, 0);
        let remote_sync = RemoteSync::new(connection(&tmpdir_), Box::new(client));

        // When
        let changes = remote_sync.changes().unwrap();

        // Then
        assert_eq!(
            changes,
            vec![RemoteChange::Updated(ContentId(1), PathBuf::from("a.txt"))]
        )
    }

    #[test]
    fn test_changes_file_is_deleted() {
        // Given
        let tmpdir_ = tmpdir();
        let mut client = MockTracimClient::new();
        DiskState::new(connection(&tmpdir_), tmpdir_.clone())
            .create_tables()
            .unwrap();
        client
            .expect_get_contents()
            .times(1)
            .returning(|| Ok(vec![]));
        insert_content(&connection(&tmpdir_), "a.txt", 1, 1, None, 0);
        let remote_sync = RemoteSync::new(connection(&tmpdir_), Box::new(client));

        // When
        let changes = remote_sync.changes().unwrap();

        // Then
        assert_eq!(
            changes,
            vec![RemoteChange::Disappear(
                ContentId(1),
                PathBuf::from("a.txt")
            )]
        )
    }
}
