use trsync_core::{
    config::ManagerConfig,
    instance::{Instance, InstanceId},
};

use crate::panel::{instance::GuiInstance, Panel};

pub struct State {
    pub current_panel: Panel,
    pub available_panels: Vec<Panel>,
    pub base_folder: String,
    pub icons_path: Option<String>,
    pub confirm_startup_sync: bool,
    pub popup_confirm_startup_sync: bool,
    pub instances: Vec<Instance>,
}

impl State {
    pub fn from_config(config: &ManagerConfig) -> Self {
        let available_panels = [
            vec![Panel::Root],
            config
                .instances
                .iter()
                .map(|i| Panel::Instance(i.into()))
                .collect(),
            vec![Panel::AddInstance(GuiInstance::default())],
        ]
        .concat();

        Self {
            current_panel: Panel::Root,
            available_panels,
            base_folder: config.local_folder.clone(),
            icons_path: config.icons_path.clone(),
            confirm_startup_sync: config.confirm_startup_sync,
            popup_confirm_startup_sync: config.popup_confirm_startup_sync,
            instances: config.instances.clone(),
        }
    }

    pub fn to_config(&self) -> ManagerConfig {
        ManagerConfig {
            local_folder: self.base_folder.clone(),
            icons_path: self.icons_path.clone(),
            instances: self.instances.clone(),
            allow_raw_passwords: false,
            confirm_startup_sync: self.confirm_startup_sync,
            popup_confirm_startup_sync: self.popup_confirm_startup_sync,
        }
    }

    pub fn remove_instance(&mut self, instance_id: &InstanceId) {
        self.available_panels.retain(|p| match p {
            Panel::Instance(instance) => &instance.name != instance_id,
            _ => true,
        });

        self.instances
            .retain(|instance| &instance.name != instance_id);

        self.current_panel = Panel::Root;
    }
}
