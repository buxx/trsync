use std::path::PathBuf;

use crate::{
    event::{local::LocalEvent, remote::RemoteEvent, Event},
    state::State,
};
use anyhow::{Context, Result};
use trsync_core::{client::TracimClient, instance::ContentId};

use super::{
    executor::Executor,
    executor::{
        disk::{
            absent::AbsentFromDiskExecutor, present::PresentOnDiskExecutor,
            updated::UpdatedOnDiskExecutor,
        },
        remote::{
            absent::AbsentFromRemoteExecutor, created::CreatedOnRemoteExecutor,
            modified::ModifiedOnRemoteExecutor, named::NamedOnRemoteExecutor,
        },
    },
};

pub struct Operator<'a> {
    state: &'a mut Box<dyn State>,
    workspace_folder: &'a PathBuf,
    tracim: Box<dyn TracimClient>,
}

impl<'a> Operator<'a> {
    pub fn new(
        state: &'a mut Box<dyn State>,
        workspace_folder: &'a PathBuf,
        tracim: Box<dyn TracimClient>,
    ) -> Self {
        Self {
            state,
            workspace_folder,
            tracim,
        }
    }

    pub fn operate(&mut self, event: Event) -> Result<()> {
        // FIXME BS : il faut que l'appel au dessus choisisse quoi faire en cas d'erreur
        // En gros, ressayer si c'est un problème reseau, etc
        match self
            .executor(event)
            .execute(self.state, &self.tracim)
            .context("Run executor")
        {
            Ok(state_change) => self.state.change(state_change)?,
            Err(error) => return Err(error),
        };

        Ok(())
    }

    fn executor(&self, event: Event) -> Box<dyn Executor> {
        match event {
            Event::Remote(event) => match event {
                RemoteEvent::Deleted(id) => Box::new(self.absent_from_disk_executor(id)),
                RemoteEvent::Created(id) => Box::new(self.present_on_disk_executor(id)),
                RemoteEvent::Updated(id) => Box::new(self.updated_on_disk_executor(id, true)),
                RemoteEvent::Renamed(id) => Box::new(self.updated_on_disk_executor(id, false)),
            },
            Event::Local(event) => match event {
                LocalEvent::Deleted(id) => Box::new(self.absent_from_remote_executor(id)),
                LocalEvent::Created(path) => Box::new(self.created_on_remote_executor(path)),
                LocalEvent::Modified(id) => Box::new(self.modified_on_remote_executor(id)),
                LocalEvent::Renamed(id, path) => Box::new(self.named_on_remote_executor(id, path)),
            },
        }
    }

    fn absent_from_disk_executor(&self, content_id: ContentId) -> AbsentFromDiskExecutor {
        AbsentFromDiskExecutor::new(self.workspace_folder.clone(), content_id)
    }

    fn absent_from_remote_executor(&self, content_id: ContentId) -> AbsentFromRemoteExecutor {
        AbsentFromRemoteExecutor::new(content_id)
    }

    fn present_on_disk_executor(&self, content_id: ContentId) -> PresentOnDiskExecutor {
        PresentOnDiskExecutor::new(self.workspace_folder.clone(), content_id)
    }

    fn created_on_remote_executor(&self, path: PathBuf) -> CreatedOnRemoteExecutor {
        CreatedOnRemoteExecutor::new(self.workspace_folder.clone(), path)
    }

    fn modified_on_remote_executor(&self, content_id: ContentId) -> ModifiedOnRemoteExecutor {
        ModifiedOnRemoteExecutor::new(self.workspace_folder.clone(), content_id)
    }

    fn updated_on_disk_executor(
        &self,
        content_id: ContentId,
        download: bool,
    ) -> UpdatedOnDiskExecutor {
        UpdatedOnDiskExecutor::new(self.workspace_folder.clone(), content_id, download)
    }

    fn named_on_remote_executor(
        &self,
        content_id: ContentId,
        new_path: PathBuf,
    ) -> NamedOnRemoteExecutor {
        NamedOnRemoteExecutor::new(self.workspace_folder.clone(), content_id, new_path)
    }
}

#[cfg(test)]
mod test {
    use mockall::predicate::*;
    use trsync_core::client::MockTracimClient;
    use trsync_core::instance::ContentId;

    use super::*;
    use crate::tests::*;
    use rstest::*;

    #[rstest]
    // REMOTE DELETE
    // Delete an unknown content
    #[case(
        vec![],
        Event::Remote(RemoteEvent::Deleted(ContentId(1))),
        true,
        vec![],
        vec![],
        vec![],
    )]
    // Delete a file
    #[case(
        vec![(1, 1, "a.txt", None)],
        Event::Remote(RemoteEvent::Deleted(ContentId(1))),
        false,
        vec![],
        vec![],
        vec![],
    )]
    #[case(
        vec![(1, 1, "a.txt", None), (2, 2, "b.txt", None)],
        Event::Remote(RemoteEvent::Deleted(ContentId(1))),
        false,
        vec![],
        vec!["b.txt"],
        vec!["b.txt"],
    )]
    // Delete a file in a folder
    #[case(
        vec![(1, 1, "Folder", None), (2, 2, "a.txt", Some(1))],
        Event::Remote(RemoteEvent::Deleted(ContentId(2))),
        false,
        vec![],
        vec!["Folder"],
        vec!["Folder"],
    )]
    // Delete a folder containing a file
    #[case(
        vec![(1, 1, "Folder", None), (2, 2, "a.txt", Some(1))],
        Event::Remote(RemoteEvent::Deleted(ContentId(1))),
        false,
        vec![],
        vec![],
        vec![],
    )]
    // REMOTE CREATED
    // Receive a new file
    #[case(
        vec![],
        Event::Remote(RemoteEvent::Created(ContentId(1))),
        false,
        vec![MockTracimClientCase::GetOk((1, 1, "a.txt".to_string(), None)), MockTracimClientCase::FillLocalOk(1, "a.txt".to_string())],
        vec!["a.txt"],
        vec!["a.txt"],
    )]
    // Receive a new file in a folder
    #[case(
        vec![(1, 1, "Folder", None)],
        Event::Remote(RemoteEvent::Created(ContentId(2))),
        false,
        vec![MockTracimClientCase::GetOk((2, 2, "a.txt".to_string(), Some(1))), MockTracimClientCase::FillLocalOk(2, "Folder/a.txt".to_string())],
        vec!["Folder", "Folder/a.txt"],
        vec!["Folder", "Folder/a.txt"],
    )]
    // Receive a new folder
    #[case(
        vec![],
        Event::Remote(RemoteEvent::Created(ContentId(1))),
        false,
        vec![MockTracimClientCase::GetOk((1, 1, "Folder".to_string(), None))],
        vec!["Folder"],
        vec!["Folder"],
    )]
    // REMOTE UPDATED
    // Receive an updated file
    #[case(
        vec![(1, 1, "a.txt", None)],
        Event::Remote(RemoteEvent::Updated(ContentId(1))),
        false,
        vec![MockTracimClientCase::GetOk((1, 1, "a.txt".to_string(), None)), MockTracimClientCase::FillLocalOk(1, "a.txt".to_string())],
        vec!["a.txt"],
        vec!["a.txt"],
    )]
    // Receive an updated file in a folder
    #[case(
        vec![(1, 1, "Folder", None), (2, 2, "a.txt", Some(1))],
        Event::Remote(RemoteEvent::Updated(ContentId(2))),
        false,
        vec![MockTracimClientCase::GetOk((2, 2, "a.txt".to_string(), Some(1))), MockTracimClientCase::FillLocalOk(2, "Folder/a.txt".to_string())],
        vec!["Folder", "Folder/a.txt"],
        vec!["Folder", "Folder/a.txt"],
    )]
    // REMOTE RENAMED
    // Remote file renamed
    #[case(
        vec![(1, 1, "a.txt", None)],
        Event::Remote(RemoteEvent::Renamed(ContentId(1))),
        false,
        vec![MockTracimClientCase::GetOk((1, 1, "x.txt".to_string(), None))],
        vec!["x.txt"],
        vec!["x.txt"],
    )]
    // Remote file moved
    #[case(
        vec![(1, 1, "a.txt", None), (2, 2, "Folder", None)],
        Event::Remote(RemoteEvent::Renamed(ContentId(1))),
        false,
        vec![MockTracimClientCase::GetOk((1, 1, "a.txt".to_string(), Some(2)))],
        vec!["Folder", "Folder/a.txt"],
        vec!["Folder", "Folder/a.txt"],
    )]
    // Remote file moved and renamed
    #[case(
        vec![(1, 1, "a.txt", None), (2, 2, "Folder", None)],
        Event::Remote(RemoteEvent::Renamed(ContentId(1))),
        false,
        vec![MockTracimClientCase::GetOk((1, 1, "x.txt".to_string(), Some(2)))],
        vec!["Folder", "Folder/x.txt"],
        vec!["Folder", "Folder/x.txt"],
    )]
    fn test_operator_on_remote_event(
        #[case] raw_contents: Vec<(i32, i32, &str, Option<i32>)>,
        #[case] event: Event,
        #[case] error: bool,
        #[case] expect_tracim: Vec<MockTracimClientCase>,
        #[case] expected_on_disk: Vec<&str>,
        #[case] expected_state: Vec<&str>,
    ) {
        // Given
        let tmpdir_ = tmpdir();
        ensure_disk(&raw_contents, &tmpdir_);
        let mut state = build_memory_state(&raw_contents, Some(&tmpdir_));
        let mut client = MockTracimClient::new();
        MockTracimClientCase::apply_multiples(&tmpdir_, &mut client, expect_tracim);

        // When
        let result = Operator::new(&mut state, &tmpdir_, Box::new(client)).operate(event);

        // Then
        assert_eq!(result.is_err(), error);
        let disk_files = disk_files(&tmpdir_);
        assert_eq!(disk_files, expected_on_disk);
        let state_files = state_files(&state);
        assert_eq!(state_files, expected_state);
    }

    #[rstest]
    // LOCAL DELETE
    // Delete a file
    #[case(
        vec![(1, 1, "a.txt", None)],
        vec![],
        Event::Local(LocalEvent::Deleted(ContentId(1))),
        false,
        vec![MockTracimClientCase::TrashOk(ContentId(1))],
        vec![],
    )]
    #[case(
        vec![(1, 1, "a.txt", None), (2, 2, "b.txt", None)],
        vec![],
        Event::Local(LocalEvent::Deleted(ContentId(1))),
        false,
        vec![MockTracimClientCase::TrashOk(ContentId(1))],
        vec!["b.txt"],
    )]
    // Delete a file in a folder
    #[case(
        vec![(1, 1, "Folder", None), (2, 2, "a.txt", Some(1))],
        vec![],
        Event::Local(LocalEvent::Deleted(ContentId(2))),
        false,
        vec![MockTracimClientCase::TrashOk(ContentId(2))],
        vec!["Folder"],
    )]
    // // Delete a folder containing a file
    #[case(
        vec![(1, 1, "Folder", None), (2, 2, "a.txt", Some(1))],
        vec![],
        Event::Local(LocalEvent::Deleted(ContentId(1))),
        false,
        vec![MockTracimClientCase::TrashOk(ContentId(1))],
        vec![],
    )]
    // LOCAL CREATED
    // File created
    #[case(
        vec![],
        vec![OperateOnDisk::Create("a.txt".to_string())],
        Event::Local(LocalEvent::Created(PathBuf::from("a.txt"))),
        false,
        vec![
            MockTracimClientCase::CreateOk(("a.txt".to_string(), None, 1)),
            MockTracimClientCase::FillRemoteOk(1, "a.txt".to_string(), 2),
            MockTracimClientCase::GetOk((1, 1, "a.txt".to_string(), None)),
        ],
        vec!["a.txt"],
    )]
    // File created in folder
    #[case(
        vec![(1, 1, "Folder", None)],
        vec![OperateOnDisk::Create("Folder/a.txt".to_string())],
        Event::Local(LocalEvent::Created(PathBuf::from("Folder/a.txt"))),
        false,
        vec![
            MockTracimClientCase::CreateOk(("Folder/a.txt".to_string(), Some(1), 2)),
            MockTracimClientCase::FillRemoteOk(2, "Folder/a.txt".to_string(), 3),
            MockTracimClientCase::GetOk((2, 3, "a.txt".to_string(), Some(1))),
        ],
        vec!["Folder", "Folder/a.txt"],
    )]
    // Folder created
    #[case(
        vec![],
        vec![OperateOnDisk::Create("Folder".to_string())],
        Event::Local(LocalEvent::Created(PathBuf::from("Folder"))),
        false,
        vec![
            MockTracimClientCase::CreateOk(("Folder".to_string(), None, 1)),
            MockTracimClientCase::GetOk((1, 1, "Folder".to_string(), None)),
        ],
        vec!["Folder"],
    )]
    // LOCAL MODIFIED
    // File modified
    #[case(
        vec![(1, 1, "a.txt", None)],
        vec![],
        Event::Local(LocalEvent::Modified(ContentId(1))),
        false,
        vec![
            MockTracimClientCase::FillRemoteOk(1, "a.txt".to_string(), 2),
            MockTracimClientCase::GetOk((1, 2, "a.txt".to_string(), None)),
        ],
        vec!["a.txt"],
    )]
    // File modified in Folder
    #[case(
        vec![(1, 1, "Folder", None), (2, 2, "a.txt", Some(1))],
        vec![],
        Event::Local(LocalEvent::Modified(ContentId(2))),
        false,
        vec![
            MockTracimClientCase::FillRemoteOk(2, "Folder/a.txt".to_string(), 3),
            MockTracimClientCase::GetOk((2, 3, "a.txt".to_string(), Some(1))),
        ],
        vec!["Folder", "Folder/a.txt"],
    )]
    // LOCAL RENAMED
    // File name changed
    #[case(
        vec![(1, 1, "a.txt", None)],
        vec![OperateOnDisk::Rename("a.txt".to_string(), "b.txt".to_string())],
        Event::Local(LocalEvent::Renamed(ContentId(1), PathBuf::from("b.txt"))),
        false,
        vec![
            MockTracimClientCase::SetLabel(1, "b.txt".to_string(), 2),
        ],
        vec!["b.txt"],
    )]
    // File name in folder change
    #[case(
        vec![(1, 1, "Folder", None), (2, 2, "a.txt", Some(1))],
        vec![OperateOnDisk::Rename("Folder/a.txt".to_string(), "Folder/b.txt".to_string())],
        Event::Local(LocalEvent::Renamed(ContentId(2), PathBuf::from("Folder/b.txt"))),
        false,
        vec![
            MockTracimClientCase::SetLabel(2, "b.txt".to_string(), 3),
        ],
        vec!["Folder","Folder/b.txt"],
    )]
    // File change path
    #[case(
        vec![(1, 1, "Folder", None), (2, 2, "a.txt", None)],
        vec![OperateOnDisk::Rename("a.txt".to_string(), "Folder/a.txt".to_string())],
        Event::Local(LocalEvent::Renamed(ContentId(2), PathBuf::from("Folder/a.txt"))),
        false,
        vec![
            MockTracimClientCase::SetParent(2, "a.txt".to_string(), Some(1), 3),
        ],
        vec!["Folder", "Folder/a.txt"],
    )]
    // File change path and renamed
    #[case(
        vec![(1, 1, "Folder", None), (2, 2, "a.txt", None)],
        vec![OperateOnDisk::Rename("a.txt".to_string(), "Folder/b.txt".to_string())],
        Event::Local(LocalEvent::Renamed(ContentId(2), PathBuf::from("Folder/b.txt"))),
        false,
        vec![
            MockTracimClientCase::SetLabel(2, "b.txt".to_string(), 3),
            MockTracimClientCase::SetParent(2, "b.txt".to_string(), Some(1), 4),
        ],
        vec!["Folder", "Folder/b.txt"],
    )]
    fn test_operator_on_local_event(
        #[case] previous_event_contents: Vec<(i32, i32, &str, Option<i32>)>,
        #[case] with_event_contents: Vec<OperateOnDisk>,
        #[case] event: Event,
        #[case] error: bool,
        #[case] expect_tracim: Vec<MockTracimClientCase>,
        #[case] expected_state: Vec<&str>,
    ) {
        // Given
        let tmpdir_ = tmpdir();
        ensure_disk(&previous_event_contents, &tmpdir_);
        let mut previous_event_state = build_memory_state(&previous_event_contents, Some(&tmpdir_));
        apply_on_disk(&with_event_contents, &tmpdir_);
        let mut client = MockTracimClient::new();
        MockTracimClientCase::apply_multiples(&tmpdir_, &mut client, expect_tracim);

        // When
        let result =
            Operator::new(&mut previous_event_state, &tmpdir_, Box::new(client)).operate(event);

        // Then
        assert_eq!(result.is_err(), error);
        let state_files = state_files(&previous_event_state);
        assert_eq!(state_files, expected_state);
    }
}
