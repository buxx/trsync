use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

use rusqlite::{params, Connection};

use crate::error::Error;
use trsync_core::types::{ContentId, LastModifiedTimestamp, RelativeFilePath, RevisionId};

pub const DB_NAME: &str = ".trsync.db";

pub struct Database {
    database_file_path: String,
}

impl Database {
    pub fn new(database_file_path: String) -> Self {
        Self { database_file_path }
    }

    pub fn with_new_connection<F>(&self, f: F) -> Result<(), Error>
    where
        F: FnOnce(Connection) -> Result<(), Error>,
    {
        // FIXME : need to close ?
        let connection = Connection::open(self.database_file_path.clone())?;
        f(connection)?;
        Ok(())
    }
}

pub struct DatabaseOperation<'d> {
    connection: &'d Connection,
}

impl<'d> DatabaseOperation<'d> {
    pub fn new(connection: &'d Connection) -> Self {
        Self { connection }
    }

    pub fn create_tables(&self) -> Result<(), rusqlite::Error> {
        self.connection.execute(
            "CREATE TABLE IF NOT EXISTS file (
                relative_path TEXT PRIMARY KEY,
                last_modified_timestamp INTEGER NOT NULL,
                content_id INTEGER NOT NULL,
                revision_id INTEGER NOT NULL
            );",
            [],
        )?;
        self.connection.execute(
            "CREATE UNIQUE INDEX IF NOT EXISTS idx_local_relative_path ON file (relative_path)",
            [],
        )?;
        self.connection.execute(
            "CREATE UNIQUE INDEX IF NOT EXISTS idx_local_content_id ON file (content_id)",
            [],
        )?;
        Ok(())
    }

    pub fn content_id_is_known(&self, content_id: ContentId) -> Result<bool, rusqlite::Error> {
        match self.connection.query_row::<u64, _, _>(
            "SELECT 1 FROM file WHERE content_id = ?",
            params![content_id],
            |row| row.get(0),
        ) {
            Ok(_) => Ok(true),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(false),
            Err(error) => Err(error),
        }
    }

    pub fn relative_path_is_known(
        &self,
        relative_path: &RelativeFilePath,
    ) -> Result<bool, rusqlite::Error> {
        match self.connection.query_row::<u64, _, _>(
            "SELECT 1 FROM file WHERE relative_path = ?",
            params![relative_path],
            |row| row.get(0),
        ) {
            Ok(_) => Ok(true),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(false),
            Err(error) => Err(error),
        }
    }

    pub fn get_content_id_from_path(&self, relative_path: String) -> Result<ContentId, Error> {
        match self.connection.query_row::<ContentId, _, _>(
            "SELECT content_id FROM file WHERE relative_path = ?",
            params![relative_path],
            |row| row.get(0),
        ) {
            Ok(content_id) => Ok(content_id),
            Err(rusqlite::Error::QueryReturnedNoRows) => {
                Err(Error::UnIndexedRelativePath(relative_path))
            }
            Err(error) => Err(Error::UnexpectedError(format!("{:?}", error))),
        }
    }

    pub fn get_revision_id_from_content_id(
        &self,
        content_id: ContentId,
    ) -> Result<RevisionId, rusqlite::Error> {
        self.connection.query_row::<RevisionId, _, _>(
            "SELECT revision_id FROM file WHERE content_id = ?",
            params![content_id],
            |row| row.get(0),
        )
    }

    pub fn get_path_from_content_id(
        &self,
        content_id: ContentId,
    ) -> Result<String, rusqlite::Error> {
        self.connection.query_row::<String, _, _>(
            "SELECT relative_path FROM file WHERE content_id = ?",
            params![content_id],
            |row| row.get(0),
        )
    }

    pub fn insert_new_file(
        &self,
        relative_path: String,
        last_modified_timestamp: LastModifiedTimestamp,
        content_id: ContentId,
        revision_id: RevisionId,
    ) -> Result<(), rusqlite::Error> {
        log::debug!(
            "Insert new file with path {:?} and timestamp {}",
            relative_path,
            last_modified_timestamp,
        );

        match self.connection
            .execute(
                "INSERT INTO file (relative_path, last_modified_timestamp, content_id, revision_id) VALUES (?1, ?2, ?3, ?4)",
                params![relative_path, last_modified_timestamp, content_id, revision_id],
            ) {
                Ok(_) => {},
                Err(error) => {
                    match &error {
                        rusqlite::Error::SqliteFailure(sqlite_error, message) => {
                            match sqlite_error.code {
                                rusqlite::ErrorCode::ConstraintViolation => {
                                    if message == &Some("UNIQUE constraint failed: file.relative_path".to_string()) {
                                        log::debug!("File with path {:?} already exists", relative_path);
                                    } else if message == &Some("UNIQUE constraint failed: file.content_id".to_string()) {
                                        log::debug!("File with content_id {:?} already exists, update its path", relative_path);
                                        self.connection.execute(
                                            "UPDATE file SET relative_path=?1, last_modified_timestamp=?2, revision_id=?3 WHERE content_id = ?4",
                                            params![relative_path, last_modified_timestamp, revision_id, content_id],
                                        )?;
                                    } else {
                                        return Err(error)
                                    }
                                },
                                _ => return Err(error),
                            }
                        }
                        _ => return Err(error),
                    }
                }
            };
        Ok(())
    }

    pub fn update_last_modified_timestamp(
        &self,
        relative_path: String,
        last_modified_timestamp: LastModifiedTimestamp,
    ) -> Result<(), rusqlite::Error> {
        log::debug!(
            "Update last modified timestamp of {:?} with {:?}",
            relative_path,
            last_modified_timestamp
        );

        self.connection.execute(
            "UPDATE file SET last_modified_timestamp = ?1 WHERE relative_path = ?2",
            params![last_modified_timestamp, relative_path],
        )?;
        Ok(())
    }

    pub fn get_last_modified_timestamp(&self, relative_path: &str) -> Result<u64, rusqlite::Error> {
        self.connection.query_row::<u64, _, _>(
            "SELECT last_modified_timestamp FROM file WHERE relative_path = ?",
            params![relative_path],
            |row| row.get(0),
        )
    }

    pub fn update_revision_id(
        &self,
        relative_path: String,
        revision_id: RevisionId,
    ) -> Result<(), rusqlite::Error> {
        log::debug!(
            "Update revision_id of {:?} with {:?}",
            relative_path,
            revision_id
        );

        self.connection.execute(
            "UPDATE file SET revision_id = ?1 WHERE relative_path = ?2",
            params![revision_id, relative_path],
        )?;
        Ok(())
    }

    pub fn update_relative_path(
        &self,
        content_id: ContentId,
        relative_path: RelativeFilePath,
    ) -> Result<(), rusqlite::Error> {
        log::debug!(
            "Update relative path relative path of content {:?} with {:?}",
            content_id,
            relative_path
        );

        self.connection.execute(
            "UPDATE file SET relative_path = ?1 WHERE content_id = ?2",
            params![relative_path, content_id],
        )?;
        Ok(())
    }

    pub fn delete_file(&self, content_id: ContentId) -> Result<(), rusqlite::Error> {
        self.connection.execute(
            "DELETE FROM file WHERE content_id = ?1",
            params![content_id],
        )?;
        Ok(())
    }

    pub fn get_relative_paths(&self) -> Result<Vec<String>, rusqlite::Error> {
        let mut relative_paths = vec![];
        let mut stmt = self.connection.prepare("SELECT relative_path FROM file")?;
        let local_iter = stmt.query_map([], |row| row.get(0))?;
        for result in local_iter {
            let relative_path: String = result?;
            relative_paths.push(relative_path);
        }
        Ok(relative_paths)
    }

    pub fn get_content_ids(&self) -> Result<Vec<ContentId>, rusqlite::Error> {
        let mut content_ids = vec![];
        let mut stmt = self.connection.prepare("SELECT content_id FROM file")?;
        let local_iter = stmt.query_map([], |row| row.get(0))?;
        for result in local_iter {
            let content_id: i32 = result?;
            content_ids.push(content_id)
        }
        Ok(content_ids)
    }
}

pub fn db_path(workspace_path: &Path) -> PathBuf {
    workspace_path.join(DB_NAME)
}

pub fn connection(workspace_path: &Path) -> Result<Connection> {
    let db_path = db_path(workspace_path);
    Connection::open(&db_path).context(format!("Open database connection on {}", db_path.display()))
}
