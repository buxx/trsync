use std::path::PathBuf;

use anyhow::{bail, Context, Result};
use rusqlite::{params, Connection};
use trsync_core::instance::DiskTimestamp;
use walkdir::{DirEntry, WalkDir};

use crate::util::last_modified_timestamp;

pub struct LocalSync {
    connection: Connection,
    workspace_path: PathBuf,
}

impl LocalSync {
    pub fn new(connection: Connection, workspace_path: PathBuf) -> Self {
        Self {
            connection,
            workspace_path,
        }
    }

    pub fn changes(&self) -> Result<Vec<LocalChange>> {
        let mut changes = vec![];
        let mut disk_relative_paths = vec![];

        // Read from disk to see changes or new
        for entry in WalkDir::new(&self.workspace_path)
            .into_iter()
            .filter_entry(|e| !self.ignore_entry(e))
        {
            let entry_debug = format!("{:?}", &entry);
            let entry = entry.context(format!("Read disk entry {:?}", entry_debug))?;

            if self.workspace_path == entry.path() {
                continue;
            }
            disk_relative_paths.push(
                entry
                    .path()
                    .to_path_buf()
                    .strip_prefix(&self.workspace_path)
                    .expect("Manipulated path are in the workspace folder")
                    .to_path_buf(),
            );

            if let Some(change) = self
                .change(&entry)
                .context(format!("Determine change for {:?}", &entry))?
            {
                changes.push(change);
            }
        }

        // Read from database to see changes or deleted
        changes.extend(
            self.changes_from_db(&disk_relative_paths)
                .context("Determine changes from db".to_string())?,
        );

        Ok(changes)
    }

    fn ignore_entry(&self, entry: &DirEntry) -> bool {
        let is_root = self.workspace_path == entry.path();

        if !is_root && entry.file_type().is_dir() {
            // Ignore directory from local sync : changes can only be rename.
            // And modification time is problematic :https://github.com/buxx/trsync/issues/60
            return true;
        }

        // TODO : patterns from config object
        if let Some(file_name) = entry.path().file_name() {
            if let Some(file_name_) = file_name.to_str() {
                let file_name_as_str = file_name_.to_string();
                if file_name_as_str.starts_with('.')
                    || file_name_as_str.starts_with('~')
                    || file_name_as_str.starts_with('#')
                {
                    return true;
                }
            }
        }

        false
    }

    fn change(&self, entry: &DirEntry) -> Result<Option<LocalChange>> {
        let absolute_path = entry.path().to_path_buf();
        let relative_path = absolute_path
            .strip_prefix(&self.workspace_path)
            .expect("Manipulated path are in the workspace folder")
            .to_path_buf();
        if self.previously_known(&relative_path).context(format!(
            "Test if path {} is previously known",
            relative_path.display()
        ))? {
            let modified = DiskTimestamp(
                last_modified_timestamp(&absolute_path)
                    .context(format!("Get disk timestamp of {}", relative_path.display()))?
                    .as_millis() as u64,
            );
            if modified
                != self
                    .previously_known_disk_timestamp(&relative_path)
                    .context(format!(
                        "Get previously disk timestamp for {}",
                        &relative_path.display()
                    ))?
            {
                Ok(Some(LocalChange::Updated(relative_path)))
            } else {
                Ok(None)
            }
        } else {
            Ok(Some(LocalChange::New(relative_path)))
        }
    }

    fn previously_known(&self, path: &PathBuf) -> Result<bool> {
        match self.connection.query_row::<u64, _, _>(
            "SELECT 1 FROM file WHERE relative_path = ?",
            params![path.display().to_string()],
            |row| row.get(0),
        ) {
            Ok(_) => Ok(true),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(false),
            Err(error) => bail!(error),
        }
    }

    fn previously_known_disk_timestamp(&self, path: &PathBuf) -> Result<DiskTimestamp> {
        match self.connection.query_row::<u64, _, _>(
            "SELECT last_modified_timestamp FROM file WHERE relative_path = ?",
            params![path.display().to_string()],
            |row| Ok(row.get(0).unwrap()),
        ) {
            Ok(timestamp) => Ok(DiskTimestamp(timestamp)),
            Err(rusqlite::Error::QueryReturnedNoRows) => {
                bail!("No row found for {}", path.display())
            }
            Err(error) => bail!("Read path {} from db but : {}", path.display(), error),
        }
    }

    fn changes_from_db(&self, on_disk: &[PathBuf]) -> Result<Vec<LocalChange>> {
        let mut changes = vec![];

        for raw_relative_path in self
            .connection
            .prepare("SELECT relative_path FROM file")?
            .query_map([], |row| row.get(0))?
        {
            let raw_relative_path: String =
                raw_relative_path.context("Read raw relative_path from db")?;
            let relative_path = PathBuf::from(raw_relative_path);
            if !on_disk.contains(&relative_path) {
                changes.push(LocalChange::Disappear(relative_path))
            }
        }

        Ok(changes)
    }
}

#[cfg(test)]
mod test {

    use super::*;
    use crate::{state::disk::DiskState, tests::*};

    #[test]
    fn test_empty() {
        // Given
        let tmpdir_ = tmpdir();
        DiskState::new(connection(&tmpdir_), tmpdir_.clone())
            .create_tables()
            .unwrap();
        let connection = connection(&tmpdir_);
        let local_sync = LocalSync::new(connection, tmpdir_.clone());

        // When
        let state = local_sync.changes().unwrap();

        // Then
        assert_eq!(state, vec![])
    }

    #[test]
    fn test_one_new_file() {
        // Given
        let tmpdir_ = tmpdir();
        DiskState::new(connection(&tmpdir_), tmpdir_.clone())
            .create_tables()
            .unwrap();
        let connection = connection(&tmpdir_);
        let local_sync = LocalSync::new(connection, tmpdir_.clone());
        apply_on_disk(&vec![OperateOnDisk::Create("a.txt".to_string())], &tmpdir_);

        // When
        let state = local_sync.changes().unwrap();

        // Then
        assert_eq!(state, vec![LocalChange::New(PathBuf::from("a.txt"))])
    }

    #[test]
    fn test_one_file_but_not_modified() {
        // Given
        let tmpdir_ = tmpdir();
        DiskState::new(connection(&tmpdir_), tmpdir_.clone())
            .create_tables()
            .unwrap();
        let local_sync = LocalSync::new(connection(&tmpdir_), tmpdir_.clone());
        apply_on_disk(&vec![OperateOnDisk::Create("a.txt".to_string())], &tmpdir_);
        let timestamp = last_modified_timestamp(&tmpdir_.join("a.txt"))
            .unwrap()
            .as_millis() as u64;
        insert_content(&connection(&tmpdir_), "a.txt", 1, 1, None, timestamp);

        // When
        let state = local_sync.changes().unwrap();

        // Then
        assert_eq!(state, vec![])
    }

    #[test]
    fn test_one_file_changed() {
        // Given
        let tmpdir_ = tmpdir();
        DiskState::new(connection(&tmpdir_), tmpdir_.clone())
            .create_tables()
            .unwrap();
        let local_sync = LocalSync::new(connection(&tmpdir_), tmpdir_.clone());
        apply_on_disk(&vec![OperateOnDisk::Create("a.txt".to_string())], &tmpdir_);
        insert_content(&connection(&tmpdir_), "a.txt", 1, 1, None, 0);

        // When
        let state = local_sync.changes().unwrap();

        // Then
        assert_eq!(state, vec![LocalChange::Updated(PathBuf::from("a.txt"))])
    }

    #[test]
    fn test_one_file_deleted() {
        // Given
        let tmpdir_ = tmpdir();
        DiskState::new(connection(&tmpdir_), tmpdir_.clone())
            .create_tables()
            .unwrap();
        let local_sync = LocalSync::new(connection(&tmpdir_), tmpdir_.clone());
        insert_content(&connection(&tmpdir_), "a.txt", 1, 1, None, 0);

        // When
        let state = local_sync.changes().unwrap();

        // Then
        assert_eq!(state, vec![LocalChange::Disappear(PathBuf::from("a.txt"))])
    }
}

#[derive(Debug, Eq, PartialEq, Clone)]
pub enum LocalChange {
    New(PathBuf),
    Disappear(PathBuf),
    Updated(PathBuf),
}
