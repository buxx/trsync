use std::path::PathBuf;

use crate::config::Config;

#[derive(PartialEq)]
pub enum Icon {
    Idle,
    Working1,
    Working2,
    Working3,
    Working4,
}

impl Icon {
    fn file_name(&self) -> &str {
        match self {
            Icon::Idle => "trsync_idle.png",
            Icon::Working1 => "trsync_working1.png",
            Icon::Working2 => "trsync_working2.png",
            Icon::Working3 => "trsync_working3.png",
            Icon::Working4 => "trsync_working4.png",
        }
    }

    #[cfg(target_os = "windows")]
    pub fn value(&self) -> &str {
        self.file_name()
    }

    pub fn value(&self, config: &Config) -> PathBuf {
        PathBuf::from(config.icons_path.clone()).join(self.file_name())
    }
}
