use trsync_core::config::ManagerConfig;

use crate::panel::Panel;

pub struct State {
    pub current_panel: Panel,
    pub available_panels: Vec<Panel>,
    pub base_folder: Option<String>,
    pub prevent_startup_remote_delete: bool,
}

impl State {
    pub fn from_config(config: &ManagerConfig) -> Self {
        let available_panels = vec![
            vec![Panel::Root],
            config
                .instances
                .iter()
                .map(|i| Panel::Instance(i.name.clone()))
                .collect(),
        ]
        .concat();

        Self {
            current_panel: Panel::Root,
            available_panels,
            base_folder: config.local_folder.clone(),
            prevent_startup_remote_delete: config.prevent_delete_sync,
        }
    }
}
