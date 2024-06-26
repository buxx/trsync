use std::path::PathBuf;

use crate::{
    event::{remote::RemoteEvent, Event},
    local::{reducer::DiskEventWrap, watcher::DiskEvent},
    state::State,
};
use trsync_core::{
    client::{TracimClient, TracimClientError},
    error::{ExecutorError, OperatorError},
    instance::ContentId,
    types::ContentType,
};

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

const RETRY_COUNT_MAX: usize = 5;

pub struct Operator<'a> {
    state: &'a mut Box<dyn State>,
    workspace_folder: PathBuf,
    tracim: Box<dyn TracimClient>,
    ignore_events: Vec<Event>,
    avoid_same_sums: bool,
}

impl<'a> Operator<'a> {
    pub fn new(
        state: &'a mut Box<dyn State>,
        workspace_folder: PathBuf,
        tracim: Box<dyn TracimClient>,
    ) -> Self {
        Self {
            state,
            workspace_folder,
            tracim,
            ignore_events: vec![],
            avoid_same_sums: false,
        }
    }

    pub fn avoid_same_sums(mut self, value: bool) -> Self {
        self.avoid_same_sums = value;
        self
    }

    fn is_attachment(&self, event: &Event) -> Result<bool, OperatorError> {
        match event {
            Event::Remote(event) => match event {
                RemoteEvent::Created(content_id) => {
                    //
                    if let Some(parent_id) = self.tracim.get_content(*content_id)?.parent_id {
                        if let Some(parent) = self.state.get(ContentId(parent_id))? {
                            return Ok(parent.type_() != &ContentType::Folder);
                        }
                    }
                }
                RemoteEvent::Deleted(content_id)
                | RemoteEvent::Updated(content_id)
                | RemoteEvent::Renamed(content_id) => {
                    //
                    if let Some(content) = self.state.get(*content_id)? {
                        if let Some(parent_id) = content.parent_id() {
                            if let Some(parent) = self.state.get(parent_id)? {
                                return Ok(parent.type_() != &ContentType::Folder);
                            }
                        }
                    }
                }
            },
            Event::Local(_) => {}
        };

        Ok(false)
    }

    pub fn operate(&mut self, event: &Event) -> Result<(), OperatorError> {
        if self.ignore_events.contains(event) {
            self.ignore_events.retain(|x| x != event);
            log::info!("Ignore event (planned ignore) : {:?}", &event);
            return Ok(());
        };

        if self.is_attachment(&event)? {
            log::info!("Ignore event (attachment) : {:?}", &event);
            return Ok(());
        }

        log::info!("Proceed event : {:?}", &event);
        let event = self.qualify(event)?;

        let executors = self.executors(&event);
        let mut retry_count = 0;
        let mut state_changes = vec![];

        for executor in &executors {
            loop {
                if retry_count >= RETRY_COUNT_MAX {
                    // TODO: keep event to proceed it when connection is back
                    return Err(OperatorError::ExecutorError(
                        ExecutorError::MaximumRetryCount(event.display(self.tracim.as_ref())),
                    ));
                }
                match executor.execute(
                    self.state.as_ref(),
                    self.tracim.as_ref(),
                    &mut self.ignore_events,
                ) {
                    Ok(state_changes_) => {
                        state_changes.extend(state_changes_);
                        break;
                    }
                    Err(ExecutorError::Tracim(TracimClientError::TimeoutError)) => {
                        retry_count += 1;
                        continue;
                    }
                    Err(err) => return Err(OperatorError::ExecutorError(err)),
                };
            }
        }

        for state_change in state_changes {
            self.state.change(state_change)?
        }

        Ok(())
    }

    fn qualify(&self, event: &Event) -> Result<Event, ExecutorError> {
        if let Event::Remote(RemoteEvent::Updated(content_id)) = event {
            // When content is moved from workspace to another, Tracim indicate an update.
            // But for trsync its a creation. So, consider unknown modified content id as a creation
            if !self.state.known(*content_id)? {
                return Ok(Event::Remote(RemoteEvent::Created(*content_id)));
            }
        }

        Ok(event.clone())
    }

    fn executors(&self, event: &Event) -> Vec<Box<dyn Executor>> {
        match event {
            Event::Remote(event) => match event {
                RemoteEvent::Deleted(id) => vec![Box::new(self.absent_from_disk_executor(*id))],
                RemoteEvent::Created(id) => vec![Box::new(self.present_on_disk_executor(*id))],
                RemoteEvent::Updated(id) => {
                    vec![Box::new(self.updated_on_disk_executor(*id, true))]
                }
                RemoteEvent::Renamed(id) => {
                    vec![Box::new(self.updated_on_disk_executor(*id, false))]
                }
            },
            Event::Local(disk_event) => match disk_event {
                DiskEventWrap(db_path, DiskEvent::Deleted(_)) => {
                    vec![Box::new(self.absent_from_remote_executor(db_path.clone()))]
                }
                DiskEventWrap(_, DiskEvent::Created(disk_path)) => {
                    vec![Box::new(self.created_on_remote_executor(disk_path.clone()))]
                }
                DiskEventWrap(db_path, DiskEvent::Modified(disk_path)) => {
                    let mut executors: Vec<Box<dyn Executor>> =
                        vec![Box::new(self.modified_on_remote_executor(db_path.clone()))];

                    if db_path != disk_path {
                        executors.push(Box::new(
                            self.named_on_remote_executor(db_path.clone(), disk_path.clone()),
                        ));
                    }

                    executors
                }
                DiskEventWrap(db_path, DiskEvent::Renamed(_, after_disk_path)) => vec![Box::new(
                    self.named_on_remote_executor(db_path.clone(), after_disk_path.clone()),
                )],
            },
        }
    }

    fn absent_from_disk_executor(&self, content_id: ContentId) -> AbsentFromDiskExecutor {
        AbsentFromDiskExecutor::new(self.workspace_folder.clone(), content_id)
    }

    fn absent_from_remote_executor(&self, db_path: PathBuf) -> AbsentFromRemoteExecutor {
        AbsentFromRemoteExecutor::new(db_path)
    }

    fn present_on_disk_executor(&self, content_id: ContentId) -> PresentOnDiskExecutor {
        PresentOnDiskExecutor::new(self.workspace_folder.clone(), content_id)
    }

    fn created_on_remote_executor(&self, disk_path: PathBuf) -> CreatedOnRemoteExecutor {
        CreatedOnRemoteExecutor::new(self.workspace_folder.clone(), disk_path)
            .avoid_same_sums(self.avoid_same_sums)
    }

    fn modified_on_remote_executor(&self, db_path: PathBuf) -> ModifiedOnRemoteExecutor {
        ModifiedOnRemoteExecutor::new(self.workspace_folder.clone(), db_path)
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
        previous_db_path: PathBuf,
        after_disk_path: PathBuf,
    ) -> NamedOnRemoteExecutor {
        NamedOnRemoteExecutor::new(
            self.workspace_folder.clone(),
            previous_db_path,
            after_disk_path,
        )
    }
}

#[cfg(test)]
mod test {
    use crate::local::reducer::DiskEventWrap;
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
        let result = Operator::new(&mut state, tmpdir_.clone(), Box::new(client)).operate(&event);

        // Then
        assert_eq!(result.is_err(), error);
        let disk_files = disk_files(&tmpdir_);
        assert_eq!(disk_files, expected_on_disk);
        let state_files = state_files(state.as_ref());
        assert_eq!(state_files, expected_state);
    }

    #[rstest]
    // LOCAL DELETE
    // Delete a file
    #[case(
        vec![(1, 1, "a.txt", None)],
        vec![],
        Event::Local(DiskEventWrap::new(PathBuf::from("a.txt"), DiskEvent::Deleted(PathBuf::from("a.txt")))),
        false,
        vec![MockTracimClientCase::GetOk((1, 1, "a.txt".to_string(), None)), MockTracimClientCase::TrashOk(ContentId(1))],
        vec![],
    )]
    #[case(
        vec![(1, 1, "a.txt", None), (2, 2, "b.txt", None)],
        vec![],
        Event::Local(DiskEventWrap::new(PathBuf::from("a.txt"), DiskEvent::Deleted(PathBuf::from("a.txt")))),
        false,
        vec![MockTracimClientCase::GetOk((1, 1, "a.txt".to_string(), None)), MockTracimClientCase::TrashOk(ContentId(1))],
        vec!["b.txt"],
    )]
    // Delete a file in a folder
    #[case(
        vec![(1, 1, "Folder", None), (2, 2, "a.txt", Some(1))],
        vec![],
        Event::Local(DiskEventWrap::new(PathBuf::from("Folder/a.txt"), DiskEvent::Deleted(PathBuf::from("Folder/a.txt")))),
        false,
        vec![MockTracimClientCase::GetOk((2, 2, "a.txt".to_string(), None)), MockTracimClientCase::TrashOk(ContentId(2))],
        vec!["Folder"],
    )]
    // Delete a folder containing a file
    #[case(
        vec![(1, 1, "Folder", None), (2, 2, "a.txt", Some(1))],
        vec![],
        Event::Local(DiskEventWrap::new(PathBuf::from("Folder"), DiskEvent::Deleted(PathBuf::from("Folder")))),        false,
        vec![MockTracimClientCase::GetOk((1, 1, "Folder".to_string(), None)), MockTracimClientCase::TrashOk(ContentId(1))],
        vec![],
    )]
    // LOCAL CREATED
    // File created
    #[case(
        vec![],
        vec![OperateOnDisk::Create("a.txt".to_string())],
        Event::Local(DiskEventWrap::new(PathBuf::from("a.txt"), DiskEvent::Created(PathBuf::from("a.txt")))),
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
        Event::Local(DiskEventWrap::new(PathBuf::from("Folder/a.txt"), DiskEvent::Created(PathBuf::from("Folder/a.txt")))),
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
        Event::Local(DiskEventWrap::new(PathBuf::from("Folder"), DiskEvent::Created(PathBuf::from("Folder")))),
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
        Event::Local(DiskEventWrap::new(PathBuf::from("a.txt"), DiskEvent::Modified(PathBuf::from("a.txt")))),
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
        Event::Local(DiskEventWrap::new(PathBuf::from("Folder/a.txt"), DiskEvent::Modified(PathBuf::from("Folder/a.txt")))),
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
        Event::Local(DiskEventWrap::new(PathBuf::from("a.txt"), DiskEvent::Renamed(PathBuf::from("a.txt"), PathBuf::from("b.txt")))),
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
        Event::Local(DiskEventWrap::new(PathBuf::from("Folder/a.txt"), DiskEvent::Renamed(PathBuf::from("Folder/a.txt"), PathBuf::from("Folder/b.txt")))),
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
        Event::Local(DiskEventWrap::new(PathBuf::from("a.txt"), DiskEvent::Renamed(PathBuf::from("a.txt"), PathBuf::from("Folder/a.txt")))),
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
        Event::Local(DiskEventWrap::new(PathBuf::from("a.txt"), DiskEvent::Renamed(PathBuf::from("a.txt"), PathBuf::from("Folder/b.txt")))),
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
            Operator::new(&mut previous_event_state, tmpdir_, Box::new(client)).operate(&event);

        // Then
        assert_eq!(result.is_err(), error);
        let state_files = state_files(previous_event_state.as_ref());
        assert_eq!(state_files, expected_state);
    }
}
