use rusqlite::Connection;

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
            "CREATE TABLE IF NOT EXISTS local (
            relative_path TEXT PRIMARY KEY,
            last_modified_timestamp INTEGER NOT NULL,
            remote_content_id INTEGER
        );
        CREATE UNIQUE INDEX IF NOT EXISTS idx_local_relative_path ON local (relative_path);
        CREATE UNIQUE INDEX IF NOT EXISTS idx_local_remote_content_id ON local (remote_content_id);
        CREATE TABLE IF NOT EXISTS remote (
            content_id TEXT PRIMARY KEY,
            last_modified_timestamp INTEGER NOT NULL
        );
        CREATE UNIQUE INDEX IF NOT EXISTS idx_local ON local (content_id);",
            [],
        )
        .unwrap();
}