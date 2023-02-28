use trsync_core::{config::ManagerConfig, instance::Instance};

use crate::panel::Panel;

pub struct State {
    pub current_panel: Panel,
    pub available_panels: Vec<Panel>,
    pub base_folder: String,
    pub icons_path: Option<String>,
    pub prevent_startup_remote_delete: bool,
    pub instances: Vec<Instance>,
}

impl State {
    pub fn from_config(config: &ManagerConfig) -> Self {
        let available_panels = vec![
            vec![Panel::Root],
            config
                .instances
                .iter()
                .map(|i| Panel::Instance(i.clone()))
                .collect(),
        ]
        .concat();

        Self {
            current_panel: Panel::Root,
            available_panels,
            base_folder: config.local_folder.clone(),
            icons_path: config.icons_path.clone(),
            prevent_startup_remote_delete: config.prevent_delete_sync,
            instances: config.instances.clone(),
        }
    }

    pub fn to_config(&self) -> ManagerConfig {
        ManagerConfig {
            local_folder: self.base_folder.clone(),
            icons_path: self.icons_path.clone(),
            instances: self.instances.clone(),
            allow_raw_passwords: false,
            prevent_delete_sync: self.prevent_startup_remote_delete,
        }
    }
}
