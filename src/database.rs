use rusqlite::{params, Connection};
use serde::__private::de::Content;

use crate::types::{ContentId, LastModifiedTimestamp};

pub struct Database {
    database_file_path: String,
}

impl Database {
    pub fn new(database_file_path: String) -> Self {
        Self { database_file_path }
    }

    pub fn with_new_connection<F>(&self, f: F)
    where
        F: FnOnce(Connection),
    {
        let connection = Connection::open(self.database_file_path.clone()).unwrap();
        f(connection);
    }
}

pub struct DatabaseOperation<'d> {
    connection: &'d Connection,
}

impl<'d> DatabaseOperation<'d> {
    pub fn new<'a>(connection: &'d Connection) -> Self {
        Self { connection }
    }

    pub fn create_tables(&self) {
        self.connection
            .execute(
                "CREATE TABLE IF NOT EXISTS file (
                relative_path TEXT PRIMARY KEY,
                last_modified_timestamp INTEGER NOT NULL,
                content_id INTEGER NOT NULL
            );
            CREATE UNIQUE INDEX IF NOT EXISTS idx_local_relative_path ON local (relative_path);
            CREATE UNIQUE INDEX IF NOT EXISTS idx_local_remote_content_id ON local (remote_content_id);",
                [],
            )
            .unwrap();
    }

    pub fn content_id_is_known(&self, content_id: ContentId) -> bool {
        self.connection
            .query_row::<u64, _, _>(
                "SELECT 1 FROM file WHERE content_id = ?",
                params![content_id],
                |row| row.get(0),
            )
            .is_ok()
    }

    pub fn get_content_id_from_path(&self, relative_path: String) -> ContentId {
        self.connection
            .query_row::<u64, _, _>(
                "SELECT content_id FROM file WHERE relative_path = ?",
                params![relative_path],
                |row| row.get(0),
            )
            .unwrap() as ContentId
    }

    pub fn get_path_from_content_id(&self, content_id: ContentId) -> String {
        self.connection
            .query_row::<String, _, _>(
                "SELECT relative_path FROM file WHERE content_id = ?",
                params![content_id],
                |row| row.get(0),
            )
            .unwrap()
    }

    pub fn insert_new_file(
        &self,
        relative_path: String,
        last_modified_timestamp: LastModifiedTimestamp,
        content_id: Option<ContentId>,
    ) {
        self.connection
            .execute(
                "INSERT INTO file (relative_path, last_modified_timestamp, content_id) VALUES (?1, ?2, ?3)",
                params![relative_path, last_modified_timestamp, content_id],
            )
            .unwrap();
    }

    pub fn update_file(
        &self,
        relative_path: String,
        last_modified_timestamp: LastModifiedTimestamp,
    ) {
        self.connection
            .execute(
                "UPDATE file SET last_modified_timestamp = ?1 WHERE relative_path = ?2",
                params![last_modified_timestamp, relative_path],
            )
            .unwrap();
    }

    pub fn delete_file(&self, content_id: ContentId) {
        self.connection
            .execute(
                "DELETE FROM file WHERE content_id = ?1",
                params![content_id],
            )
            .unwrap();
    }
}
