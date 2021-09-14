use std::path::PathBuf;

use rusqlite::{params, Connection};

use crate::operation::ContentId;

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

pub fn create_tables(connection: Connection) {
    connection
        .execute(
            "CREATE TABLE IF NOT EXISTS file (
            relative_path TEXT PRIMARY KEY,
            last_modified_timestamp INTEGER NOT NULL,
            content_id INTEGER
        );
        CREATE UNIQUE INDEX IF NOT EXISTS idx_local_relative_path ON local (relative_path);
        CREATE UNIQUE INDEX IF NOT EXISTS idx_local_remote_content_id ON local (remote_content_id);",
            [],
        )
        .unwrap();
}

pub fn get_parent_content_id_with_path(
    connection: &Connection,
    relative_path: String,
) -> ContentId {
    connection
        .query_row::<u64, _, _>(
            "SELECT content_id FROM file WHERE relative_path = ?",
            params![relative_path],
            |row| row.get(0),
        )
        .unwrap() as ContentId
}
