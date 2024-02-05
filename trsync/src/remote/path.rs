// use anyhow::{bail, Result};
// use std::{iter::FromIterator, path::PathBuf};

// use trsync_core::instance::{ContentFileName, RevisionId};

// use crate::types::ContentIdentifier;

// #[derive(Clone, Debug, PartialEq, Eq, Hash)]
// pub struct RemotePath {
//     parts: Vec<RemotePathPart>,
// }

// impl RemotePath {
//     pub fn new(parts: Vec<RemotePathPart>) -> Result<Self> {
//         if parts.is_empty() {
//             bail!("Remote path must include one item minimum")
//         }

//         Ok(Self { parts })
//     }

//     pub fn path(&self) -> PathBuf {
//         PathBuf::from_iter(self.parts.iter().map(|part| part.file_name().0.clone()))
//     }

//     pub fn last(&self) -> &RemotePathPart {
//         self.parts
//             .last()
//             .expect("Initialization imply at least one element")
//     }

//     pub fn parts(&self) -> &[RemotePathPart] {
//         &self.parts
//     }
// }

// #[derive(Clone, Debug, PartialEq, Eq, Hash)]
// pub struct RemotePathPart {
//     id: ContentIdentifier,
//     file_name: ContentFileName,
// }

// impl RemotePathPart {
//     pub fn new(id: ContentIdentifier, file_name: ContentFileName) -> Self {
//         Self { id, file_name }
//     }

//     pub fn id(&self) -> &ContentIdentifier {
//         &self.id
//     }

//     pub fn file_name(&self) -> &ContentFileName {
//         &self.file_name
//     }
// }

// impl From<(ContentIdentifier, ContentFileName)> for RemotePathPart {
//     fn from(value: (ContentIdentifier, ContentFileName)) -> Self {
//         Self {
//             id: value.0,
//             file_name: value.1,
//         }
//     }
// }
