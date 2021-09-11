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
