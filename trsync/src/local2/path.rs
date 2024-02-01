// use std::path::PathBuf;

// use crate::remote::path::RemotePath;

// #[derive(Clone, Debug, PartialEq, Eq, Hash)]
// pub struct LocalPath {
//     value: PathBuf,
// }

// impl LocalPath {
//     pub fn new(value: PathBuf) -> Self {
//         Self { value }
//     }
// }

// impl From<&str> for LocalPath {
//     fn from(value: &str) -> Self {
//         Self {
//             value: PathBuf::from(value),
//         }
//     }
// }

// impl From<RemotePath> for LocalPath {
//     fn from(value: RemotePath) -> Self {
//         Self {
//             value: value.path(),
//         }
//     }
// }
