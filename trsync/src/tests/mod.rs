use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::path::PathBuf;

use crate::database::DB_NAME;
use crate::state::memory::MemoryState;
use crate::state::State;
use crate::util::last_modified_timestamp;
use anyhow::Context;

use mockall::predicate::{self, *};
use rusqlite::params;
use rusqlite::Connection;
use testdir::testdir;
use trsync_core::client::MockTracimClient;
use trsync_core::client::RemoteContent;
use trsync_core::content::Content;
use trsync_core::instance::ContentFileName;
use trsync_core::instance::ContentId;
use trsync_core::instance::DiskTimestamp;
use trsync_core::instance::RevisionId;
use trsync_core::types::ContentType;
use uuid::Uuid;
use walkdir::WalkDir;

pub fn tmpdir() -> PathBuf {
    let path = testdir!().join(PathBuf::from(Uuid::new_v4().to_string()));
    fs::create_dir_all(&path).unwrap();
    path
}

pub fn build_memory_state(
    raw_contents: &Vec<(i32, i32, &str, Option<i32>)>,
    tmpdir: Option<&PathBuf>,
) -> Box<dyn State> {
    let mut contents = HashMap::new();

    for (raw_content_id, raw_revision_id, raw_file_name, raw_parent_id) in raw_contents {
        let id = ContentId(*raw_content_id);
        let revision_id = RevisionId(*raw_revision_id);
        let file_name = ContentFileName(raw_file_name.to_string());
        let parent = raw_parent_id.and_then(|raw_parent_id| Some(ContentId(raw_parent_id)));
        let content_type = content_type(&file_name.0);
        let content = Content::new(id, revision_id, file_name, parent, content_type).unwrap();
        contents.insert(id, content);
    }

    let state = MemoryState::new(contents, HashMap::new()).unwrap();

    let mut contents: HashMap<ContentId, Content> = HashMap::new();
    let mut timestamps: HashMap<ContentId, DiskTimestamp> = HashMap::new();
    if let Some(tmpdir) = tmpdir {
        for content in state.contents().unwrap() {
            let content_path = state.path(content.id()).unwrap();
            let absolute_path = tmpdir.join(content_path.to_path_buf());
            let timestamp = last_modified_timestamp(&absolute_path).unwrap();
            contents.insert(content.id(), content.clone());
            timestamps.insert(content.id(), DiskTimestamp(timestamp.as_millis() as u64));
        }
        return Box::new(MemoryState::new(contents, timestamps).unwrap());
    }

    Box::new(state)
}

pub fn ensure_disk(raw_contents: &Vec<(i32, i32, &str, Option<i32>)>, tmpdir: &PathBuf) {
    let hybrid_state = build_memory_state(raw_contents, None);
    ensure_state_on_disk(&hybrid_state, tmpdir);
}

pub fn ensure_state_on_disk(state: &Box<dyn State>, tmpdir: &PathBuf) {
    for content in state
        .contents()
        .context("Read all contents from state")
        .unwrap()
    {
        let content_path: PathBuf = state.path(content.id()).unwrap().into();
        let absolute_path = tmpdir.join(content_path);
        match content.type_() {
            ContentType::Folder => {
                fs::create_dir_all(&absolute_path)
                    .context(format!("Create folder {}", &absolute_path.display()))
                    .unwrap();
            }
            _ => {
                fs::File::create(&absolute_path)
                    .context(format!("Create file {}", &absolute_path.display()))
                    .unwrap();
            }
        };
    }
}

pub fn apply_on_disk(operations: &Vec<OperateOnDisk>, tmpdir: &PathBuf) {
    for operation in operations {
        match operation {
            OperateOnDisk::Create(file_path) => {
                let absolute_path = tmpdir.join(file_path);
                match content_type(&file_path) {
                    ContentType::File => {
                        fs::File::create(&absolute_path)
                            .context(format!("Create file {}", &absolute_path.display()))
                            .unwrap();
                    }
                    ContentType::Folder => {
                        fs::create_dir_all(&absolute_path)
                            .context(format!("Create folder {}", &absolute_path.display()))
                            .unwrap();
                    }
                    ContentType::HtmlDocument => unreachable!(),
                };
            }
            OperateOnDisk::Rename(raw_old_path, raw_new_path) => {
                let absolute_old_path = tmpdir.join(raw_old_path);
                let absolute_new_path = tmpdir.join(raw_new_path);
                fs::rename(absolute_old_path, absolute_new_path).unwrap();
            }
        }
    }
}

pub fn disk_files(tmpdir: &PathBuf) -> Vec<String> {
    WalkDir::new(&tmpdir)
        .into_iter()
        .map(|entry| {
            entry
                .unwrap()
                .path()
                .strip_prefix(&tmpdir)
                .unwrap()
                .display()
                .to_string()
        })
        .filter(|p| !p.is_empty())
        .collect::<Vec<String>>()
}

pub fn state_files(state: &Box<dyn State>) -> Vec<String> {
    state
        .contents()
        .unwrap()
        .into_iter()
        .map(|content| state.path(content.id()).unwrap())
        .map(|path| path.to_string())
        .collect::<Vec<String>>()
}

pub enum MockTracimClientCase {
    TrashOk(ContentId),
    GetOk((i32, i32, String, Option<i32>)),
    FillLocalOk(i32, String),
    CreateOk((String, Option<i32>, i32)),
    FillRemoteOk(i32, String, i32),
    SetLabel(i32, String, i32),
    SetParent(i32, String, Option<i32>, i32),
}

impl MockTracimClientCase {
    pub fn apply_multiples(
        workspace_folder: &PathBuf,
        mock: &mut MockTracimClient,
        cases: Vec<Self>,
    ) {
        for case in cases {
            case.apply(workspace_folder, mock)
        }
    }

    pub fn apply(self, workspace_folder: &PathBuf, mock: &mut MockTracimClient) {
        match self {
            MockTracimClientCase::TrashOk(id) => {
                mock.expect_trash_content()
                    .with(predicate::eq(id))
                    .times(1)
                    .returning(|_| Ok(()));
            }
            MockTracimClientCase::GetOk((
                raw_content_id,
                raw_revision_id,
                raw_file_name,
                raw_parent_id,
            )) => {
                mock.expect_get_content()
                    .with(predicate::eq(ContentId(raw_content_id)))
                    .times(1)
                    .returning(move |_| {
                        let content_type = content_type(&raw_file_name);
                        let content = RemoteContent {
                            content_id: ContentId(raw_content_id),
                            current_revision_id: RevisionId(raw_revision_id),
                            filename: raw_file_name.clone(),
                            parent_id: raw_parent_id,
                            content_type: content_type.to_string(),
                            modified: "".to_string(),
                            raw_content: None,
                            is_deleted: false,
                            is_archived: false,
                            sub_content_types: vec![],
                        };

                        Ok(content)
                    });
            }
            MockTracimClientCase::FillLocalOk(id, path) => {
                let content_type = content_type(&path);
                mock.expect_fill_file_with_content()
                    .with(
                        predicate::eq(ContentId(id)),
                        predicate::eq(content_type),
                        predicate::eq(workspace_folder.join(Path::new(&path))),
                    )
                    .times(1)
                    .returning(|_, _, _| Ok(()));
            }
            MockTracimClientCase::CreateOk((raw_path, raw_parent_id, raw_returned_content_id)) => {
                let content_type = content_type(&raw_path);
                mock.expect_create_content()
                    .with(
                        predicate::eq(ContentFileName(
                            PathBuf::from(&raw_path)
                                .file_name()
                                .unwrap()
                                .to_str()
                                .unwrap()
                                .to_string(),
                        )),
                        predicate::eq(content_type),
                        predicate::eq(raw_parent_id.map(ContentId)),
                        predicate::eq(workspace_folder.join(raw_path)),
                    )
                    .times(1)
                    .returning(move |_, _, _, _| Ok(ContentId(raw_returned_content_id)));
            }
            MockTracimClientCase::FillRemoteOk(raw_content_id, raw_path, new_revision_id) => {
                let content_type = content_type(&raw_path);
                mock.expect_fill_content_with_file()
                    .with(
                        predicate::eq(ContentId(raw_content_id)),
                        predicate::eq(content_type),
                        predicate::eq(workspace_folder.join(Path::new(&raw_path))),
                    )
                    .times(1)
                    .returning(move |_, _, _| Ok(RevisionId(new_revision_id)));
            }
            MockTracimClientCase::SetLabel(raw_content_id, raw_file_name, raw_new_revision_id) => {
                let content_type = content_type(&raw_file_name);
                mock.expect_set_label()
                    .with(
                        predicate::eq(ContentId(raw_content_id)),
                        predicate::eq(content_type),
                        predicate::eq(ContentFileName(raw_file_name)),
                    )
                    .times(1)
                    .returning(move |_, _, _| Ok(RevisionId(raw_new_revision_id)));
            }
            MockTracimClientCase::SetParent(
                raw_content_id,
                raw_file_name,
                raw_parent_id,
                raw_new_revision_id,
            ) => {
                mock.expect_set_parent()
                    .with(
                        predicate::eq(ContentId(raw_content_id)),
                        predicate::eq(raw_parent_id.map(ContentId)),
                        predicate::eq(None),
                    )
                    .times(1)
                    .returning(move |_, _, _| Ok(RevisionId(raw_new_revision_id)));
            }
        }
    }
}

pub fn insert_content(
    connection: &Connection,
    relative_path: &str,
    content_id: i32,
    revision_id: i32,
    parent_id: Option<i32>,
    last_modified_timestamp: u64,
) {
    connection
        .prepare(
            "INSERT INTO file (relative_path, content_id, revision_id, parent_id, last_modified_timestamp) VALUES (?, ?, ?, ?, ?)"
        ).unwrap().execute(params![
            relative_path,
            content_id,
            revision_id,
            parent_id,
            last_modified_timestamp,
        ]).unwrap();
}

pub fn connection(workspace_path: &PathBuf) -> Connection {
    Connection::open(workspace_path.join(DB_NAME)).unwrap()
}

pub enum OperateOnDisk {
    Create(String),
    Rename(String, String),
}

pub fn content_type(file_name: &str) -> ContentType {
    if file_name.ends_with(".txt") {
        ContentType::File
    } else {
        ContentType::Folder
    }
}
