use std::fmt::Display;

use trsync_core::instance::InstanceId;

pub mod root;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Panel {
    Root,
    Instance(InstanceId),
}

impl Display for Panel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Panel::Root => f.write_str("Configuration"),
            Panel::Instance(id) => f.write_str(&id.to_string()),
        }
    }
}
