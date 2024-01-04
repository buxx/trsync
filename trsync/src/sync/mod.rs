use std::path::PathBuf;

use anyhow::Result;
use trsync_core::{local::LocalChange, remote::RemoteChange};

pub mod local;
pub mod remote;

pub struct StartupSyncResolver {
    remote_changes: Vec<RemoteChange>,
    local_changes: Vec<LocalChange>,
    method: ResolveMethod,
}

impl StartupSyncResolver {
    pub fn new(
        remote_changes: Vec<RemoteChange>,
        local_changes: Vec<LocalChange>,
        method: ResolveMethod,
    ) -> Self {
        Self {
            remote_changes,
            local_changes,
            method,
        }
    }

    pub fn resolve(&self) -> Result<(Vec<RemoteChange>, Vec<LocalChange>)> {
        let local_changes_paths = self
            .local_changes
            .iter()
            .map(|change| change.path())
            .collect::<Vec<PathBuf>>();
        // FIXME : need to check conflicts by parents path too
        let paths_in_conflicts = self
            .remote_changes
            .iter()
            .map(|change| change.path())
            .filter(|path| local_changes_paths.contains(path))
            .collect::<Vec<PathBuf>>();

        let (keep_remote_changes, keep_local_changes) = match self.method {
            ResolveMethod::ForceLocal => (
                self.remote_changes
                    .iter()
                    .filter(|change| !paths_in_conflicts.contains(&change.path()))
                    .cloned()
                    .collect(),
                self.local_changes.clone(),
            ),
            ResolveMethod::ForceRemote => (
                self.remote_changes.clone(),
                self.local_changes
                    .iter()
                    .filter(|change| !paths_in_conflicts.contains(&change.path()))
                    .cloned()
                    .collect(),
            ),
        };

        Ok((keep_remote_changes, keep_local_changes))
    }
}

pub enum ResolveMethod {
    ForceLocal,
    ForceRemote,
}

#[cfg(test)]
mod test {
    use super::*;
    use rstest::*;
    use trsync_core::instance::ContentId;

    #[rstest]
    // Empty
    #[case(
        ResolveMethod::ForceLocal,
        vec![],
        vec![],
        (vec![], vec![]),
    )]
    // No conflict
    #[case(
        ResolveMethod::ForceLocal,
        vec![RemoteChange::New(ContentId(1), PathBuf::from("a.txt"))],
        vec![LocalChange::New(PathBuf::from("b.txt"))],
        (vec![RemoteChange::New(ContentId(1), PathBuf::from("a.txt"))], vec![LocalChange::New(PathBuf::from("b.txt"))]),
    )]
    // Direct conflict
    #[case(
        ResolveMethod::ForceLocal,
        vec![RemoteChange::New(ContentId(1), PathBuf::from("a.txt"))],
        vec![LocalChange::New(PathBuf::from("a.txt"))],
        (vec![], vec![LocalChange::New(PathBuf::from("a.txt"))]),
    )]
    // Direct conflict
    #[case(
        ResolveMethod::ForceRemote,
        vec![RemoteChange::New(ContentId(1), PathBuf::from("a.txt"))],
        vec![LocalChange::New(PathBuf::from("a.txt"))],
        (vec![RemoteChange::New(ContentId(1), PathBuf::from("a.txt"))], vec![]),
    )]
    fn test_resolve(
        #[case] method: ResolveMethod,
        #[case] remote_changes: Vec<RemoteChange>,
        #[case] local_changes: Vec<LocalChange>,
        #[case] expected: (Vec<RemoteChange>, Vec<LocalChange>),
    ) {
        // Given
        let resolver = StartupSyncResolver::new(remote_changes, local_changes, method);

        // When
        let (keep_remote_changes, keep_local_changes) = resolver.resolve().unwrap();

        // Then
        assert_eq!(keep_remote_changes, expected.0);
        assert_eq!(keep_local_changes, expected.1);
    }
}
