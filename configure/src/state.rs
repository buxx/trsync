use std::path::PathBuf;

use crate::panel::Panel;

pub struct State {
    pub current_panel: Panel,
    pub available_panels: Vec<Panel>,
    pub base_folder: Option<String>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            current_panel: Panel::Root,
            available_panels: vec![Panel::Root],
            base_folder: None,
        }
    }
}
