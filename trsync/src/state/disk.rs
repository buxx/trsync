use std::{cmp::Ordering, path::PathBuf};

use anyhow::{bail, Context, Result};
use rusqlite::{params, Connection};
use trsync_core::{
    content::Content,
    instance::{ContentFileName, ContentId, DiskTimestamp, RevisionId},
    types::ContentType,
};

use crate::path::ContentPath;

use super::{State, StateError};

pub struct DiskState {
    connection: Connection,
    workspace_path: PathBuf,
}

impl DiskState {
    pub fn new(connection: Connection, workspace_path: PathBuf) -> Self {
        Self {
            connection,
            workspace_path,
        }
    }

    pub fn create_tables(&self) -> Result<()> {
        self.connection
            .execute(
                "CREATE TABLE IF NOT EXISTS file (
                relative_path TEXT PRIMARY KEY,
                content_id INTEGER NOT NULL,
                revision_id INTEGER NOT NULL,
                parent_id INTEGER,
                last_modified_timestamp INTEGER NOT NULL
            );",
                [],
            )
            .context("Create tables")?;
        self.connection
            .execute(
                "CREATE UNIQUE INDEX IF NOT EXISTS idx_local_relative_path ON file (relative_path)",
                [],
            )
            .context("Create relative_path index")?;
        self.connection
            .execute(
                "CREATE UNIQUE INDEX IF NOT EXISTS idx_local_content_id ON file (content_id)",
                [],
            )
            .context("Create content_id index")?;
        Ok(())
    }

    fn content_from_raw(
        &self,
        id: ContentId,
        relative_path: String,
        revision_id: i32,
        parent_id: Option<i32>,
    ) -> Result<Content> {
        let path = PathBuf::from(&relative_path);
        let file_name = path
            .file_name()
            .context(format!("Get file name from {}", path.display()))?
            .to_str()
            .context(format!("Decode file name from {}", path.display()))?
            .to_string();
        let type_ = ContentType::from_path(&self.workspace_path.join(relative_path));

        Content::new(
            id,
            RevisionId(revision_id),
            ContentFileName(file_name),
            parent_id.map(ContentId),
            type_,
        )
        .context("Construct Content struct")
    }
}

impl State for DiskState {
    fn known(&self, id: ContentId) -> Result<bool> {
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

    fn get(&self, id: ContentId) -> Result<Option<Content>> {
        let (relative_path, revision_id, parent_id) =
            match self
                .connection
                .query_row::<(String, i32, Option<i32>), _, _>(
                    "SELECT relative_path, revision_id, parent_id FROM file WHERE content_id = ?",
                    params![id.0],
                    |row| {
                        Ok((
                            row.get(0).unwrap(),
                            row.get(1).unwrap(),
                            row.get(2).unwrap(),
                        ))
                    },
                ) {
                Ok(row) => row,
                Err(rusqlite::Error::QueryReturnedNoRows) => return Ok(None),
                Err(error) => bail!("Read Content {} from db but : {}", id, error),
            };

        Ok(Some(self.content_from_raw(
            id,
            relative_path,
            revision_id,
            parent_id,
        )?))
    }

    fn content_id_for_path(&self, path: PathBuf) -> Result<Option<ContentId>> {
        match self.connection.query_row::<i32, _, _>(
            "SELECT content_id FROM file WHERE relative_path = ?",
            params![path.display().to_string()],
            |row| row.get(0),
        ) {
            Ok(raw_content_id) => Ok(Some(ContentId(raw_content_id))),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(error) => bail!(error),
        }
    }

    fn path(&self, id: ContentId) -> Result<ContentPath, StateError> {
        let content = match self.get(id).context(format!("Get content {}", id))? {
            Some(content) => content,
            None => return Err(StateError::UnknownContent(id)),
        };
        let mut parts = vec![content.clone()];

        let mut current = content;
        while let Some(parent_id) = current.parent_id() {
            let parent = self
                .get(parent_id)
                .context(format!("Get content {}", id))?
                .context(format!("Expect content for parent {}", parent_id))?;
            parts.insert(0, parent.clone());
            current = parent;
        }

        Ok(ContentPath::new(parts))
    }

    // FIXME BS NOW : Iter
    fn contents(&self) -> Result<Vec<Content>> {
        let mut contents = vec![];

        for raw_content in self
            .connection
            .prepare("SELECT content_id, relative_path, revision_id, parent_id FROM file")?
            .query_map([], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
            })?
        {
            let (raw_id, raw_relative_path, raw_revision_id, raw_parent_id): (
                i32,
                String,
                i32,
                Option<i32>,
            ) = raw_content.context("Read raw content from db")?;

            // TODO : Risky for memory overload
            contents.push(
                self.content_from_raw(
                    ContentId(raw_id),
                    raw_relative_path.clone(),
                    raw_revision_id,
                    raw_parent_id,
                )
                .context(format!(
                    "Create content struct from raw values : {:?}",
                    (raw_id, raw_relative_path, raw_revision_id, raw_parent_id)
                ))?,
            )
        }

        contents.sort_by(|a, b| match (a.type_(), b.type_()) {
            (ContentType::Folder, ContentType::Folder) => Ordering::Equal,
            (_, ContentType::Folder) => Ordering::Greater,
            (ContentType::Folder, _) => Ordering::Less,
            (_, _) => Ordering::Less,
        });

        Ok(contents)
    }

    fn direct_children_ids(&self, content_id: ContentId) -> Result<Vec<ContentId>> {
        let mut content_ids = vec![];

        for raw_content in self
            .connection
            .prepare("SELECT content_id FROM file WHERE parent_id = ?")?
            .query_map(params![content_id.0], |row| row.get(0))?
        {
            let raw_content_id: i32 = raw_content.context("Read raw content_id from db")?;
            content_ids.push(ContentId(raw_content_id));
        }

        Ok(content_ids)
    }

    fn forgot(&mut self, content_id: ContentId) -> Result<()> {
        self.connection
            .prepare("DELETE FROM file WHERE content_id = ?")?
            .execute(params![content_id.0])?;
        Ok(())
    }

    fn add(
        &mut self,
        content: Content,
        relative_path: PathBuf,
        timestamp: DiskTimestamp,
    ) -> Result<()> {
        self.connection
            .prepare("INSERT INTO file (relative_path, content_id, revision_id, parent_id, last_modified_timestamp) VALUES (?, ?, ?, ?, ?)")?
            .execute(params![
                relative_path.display().to_string(),
                content.id().0,
                content.revision_id().0,
                content.parent_id().map(|i| i.0),
                timestamp.0,
            ])?;

        Ok(())
    }

    fn update(
        &mut self,
        content_id: ContentId,
        file_name: ContentFileName,
        revision_id: RevisionId,
        parent_id: Option<ContentId>,
        timestamp: DiskTimestamp,
    ) -> Result<()> {
        // TODO : new_path should computed by caller of this method
        let new_path = parent_id
            .and_then(|parent_id| {
                Some(
                    self.path(parent_id)
                        .context(format!("Get content {} path", content_id))
                        .ok()?
                        .to_path_buf()
                        .join(&file_name.0),
                )
            })
            .unwrap_or(PathBuf::from(file_name.0));

        self.connection.execute(
            "UPDATE file SET relative_path = ?, revision_id = ?, parent_id = ?, last_modified_timestamp = ? WHERE content_id = ?",
            params![
                new_path.display().to_string(),
                revision_id.0,
                parent_id.map(|i| i.0),
                timestamp.0,
                content_id.0,
            ],
        )?;

        Ok(())
    }
}

#[cfg(test)]
mod test {
    use std::fs;

    use trsync_core::instance::ContentId;

    use super::*;
    use crate::tests::*;

    #[test]
    fn test_create_tables() {
        // Given
        let tmpdir_ = tmpdir();
        let state = DiskState::new(connection(&tmpdir_), tmpdir_.clone());

        // When-Then
        state.create_tables().unwrap();
    }

    #[test]
    fn test_known() {
        // Given
        let tmpdir_ = tmpdir();
        let state = DiskState::new(connection(&tmpdir_), tmpdir_.clone());
        state.create_tables().unwrap();
        insert_content(&connection(&tmpdir_), "a.txt", 1, 1, None, 0);

        // When
        let know = state.known(ContentId(1)).unwrap();

        assert!(know)
    }

    #[test]
    fn test_get() {
        // Given
        let tmpdir_ = tmpdir();
        let state = DiskState::new(connection(&tmpdir_), tmpdir_.clone());
        state.create_tables().unwrap();
        insert_content(&connection(&tmpdir_), "a.txt", 1, 2, None, 0);

        // When
        let content = state.get(ContentId(1)).unwrap().unwrap();

        // Then
        assert_eq!(content.id(), ContentId(1));
        assert_eq!(content.revision_id(), RevisionId(2));
        assert_eq!(content.parent_id(), None)
    }

    #[test]
    fn test_content_id_for_path() {
        // Given
        let tmpdir_ = tmpdir();
        let state = DiskState::new(connection(&tmpdir_), tmpdir_.clone());
        state.create_tables().unwrap();
        insert_content(&connection(&tmpdir_), "a.txt", 1, 2, None, 0);

        // When
        let content = state
            .content_id_for_path(PathBuf::from("a.txt"))
            .unwrap()
            .unwrap();

        // Then
        assert_eq!(content, ContentId(1));
    }

    #[test]
    fn test_path() {
        // Given
        let tmpdir_ = tmpdir();
        let state = DiskState::new(connection(&tmpdir_), tmpdir_.clone());
        state.create_tables().unwrap();
        insert_content(&connection(&tmpdir_), "a.txt", 1, 2, None, 0);

        // When
        let path = state.path(ContentId(1)).unwrap();

        // Then
        assert_eq!(path.to_string(), "a.txt".to_string());
    }

    #[test]
    fn test_path2() {
        // Given
        let tmpdir_ = tmpdir();
        let state = DiskState::new(connection(&tmpdir_), tmpdir_.clone());
        state.create_tables().unwrap();
        insert_content(&connection(&tmpdir_), "Folder", 1, 2, None, 0);
        insert_content(&connection(&tmpdir_), "a.txt", 3, 4, Some(1), 0);

        // When
        let path = state.path(ContentId(3)).unwrap();

        // Then
        assert_eq!(path.to_string(), "Folder/a.txt".to_string());
    }

    #[test]
    fn test_contents() {
        // Given
        let tmpdir_ = tmpdir();
        fs::create_dir_all(tmpdir_.join("Folder")).unwrap();
        let state = DiskState::new(connection(&tmpdir_), tmpdir_.clone());
        state.create_tables().unwrap();
        insert_content(&connection(&tmpdir_), "Folder", 1, 2, None, 0);
        insert_content(&connection(&tmpdir_), "a.txt", 3, 4, Some(1), 0);

        // When
        let contents = state.contents().unwrap();

        // Then
        assert_eq!(contents.len(), 2);
        let content1 = contents.get(0).unwrap();
        let content2 = contents.get(1).unwrap();
        assert_eq!(content1.id(), ContentId(1));
        assert_eq!(content1.revision_id(), RevisionId(2));
        assert_eq!(content1.parent_id(), None);
        assert_eq!(content2.id(), ContentId(3));
        assert_eq!(content2.revision_id(), RevisionId(4));
        assert_eq!(content2.parent_id(), Some(ContentId(1)));
    }

    #[test]
    fn test_direct_children_ids() {
        // Given
        let tmpdir_ = tmpdir();
        let state = DiskState::new(connection(&tmpdir_), tmpdir_.clone());
        state.create_tables().unwrap();
        insert_content(&connection(&tmpdir_), "Folder", 1, 2, None, 0);
        insert_content(&connection(&tmpdir_), "a.txt", 3, 4, Some(1), 0);

        // When
        let children_ids = state.direct_children_ids(ContentId(1)).unwrap();

        // Then
        assert_eq!(children_ids, vec![ContentId(3)]);
    }

    #[test]
    fn test_add() {
        // Given
        let tmpdir_ = tmpdir();
        let mut state = DiskState::new(connection(&tmpdir_), tmpdir_.clone());
        state.create_tables().unwrap();

        // When
        let _ = state.add(
            Content::new(
                ContentId(1),
                RevisionId(2),
                ContentFileName("a.txt".to_string()),
                None,
                ContentType::File,
            )
            .unwrap(),
            PathBuf::from("a.txt"),
            DiskTimestamp(0),
        );

        // Then
        let content = state.get(ContentId(1)).unwrap().unwrap();
        assert_eq!(content.id(), ContentId(1));
        assert_eq!(content.revision_id(), RevisionId(2));
        assert_eq!(content.parent_id(), None)
    }

    #[test]
    fn test_update() {
        // Given
        let tmpdir_ = tmpdir();
        let mut state = DiskState::new(connection(&tmpdir_), tmpdir_.clone());
        state.create_tables().unwrap();
        insert_content(&connection(&tmpdir_), "a.txt", 1, 2, None, 0);

        // When
        state
            .update(
                ContentId(1),
                ContentFileName("b.txt".to_string()),
                RevisionId(3),
                None,
                DiskTimestamp(42),
            )
            .unwrap();

        // Then
        let content = state.get(ContentId(1)).unwrap().unwrap();
        assert_eq!(content.id(), ContentId(1));
        assert_eq!(content.revision_id(), RevisionId(3));
        assert_eq!(content.parent_id(), None);
    }
}
