use eframe::egui::{Grid, Ui};

use crate::{event::Event, state::State, utils::label_with_help};

const CONFIGURATION_GRID_ID: &'static str = "configuration";

pub struct ConfigurationPainter;

impl ConfigurationPainter {
    pub fn new() -> Self {
        Self {}
    }

    pub fn draw(&mut self, ui: &mut Ui, state: &mut State) -> Vec<Event> {
        let mut events = vec![];

        Grid::new(CONFIGURATION_GRID_ID)
            .num_columns(2)
            .spacing([40.0, 4.0])
            .striped(true)
            .show(ui, |ui| {
                events.extend(self.base_folder(ui, state));
                ui.end_row();
                events.extend(self.prevent_sync_delete(ui, state));
            });

        events
    }

    pub fn base_folder(&mut self, ui: &mut Ui, state: &mut State) -> Vec<Event> {
        let mut events = vec![];

        ui.label("Dossier de synchronisation");
        ui.horizontal_wrapped(|ui| {
            let ellipsis = "...".to_string();
            let text = match state.base_folder.char_indices().nth(28) {
                None => state.base_folder.clone(),
                Some((idx, _)) => ellipsis + &state.base_folder[idx..],
            };
            ui.label(text);

            if ui.button("Sélectionner").clicked() {
                if let Some(path) = rfd::FileDialog::new().pick_folder() {
                    state.base_folder = path.display().to_string();
                    events.push(Event::GlobalConfigurationUpdated);
                }
            }
        });

        events
    }

    pub fn prevent_sync_delete(&mut self, ui: &mut Ui, state: &mut State) -> Vec<Event> {
        let mut events = vec![];

        ui.add(label_with_help(
            "Pas de suppression distante au démarrage",
            "Lorsque TrSync effectue une synchronisation de départ \
            (au démarrage ou après une interruption de connexion) \
            aucune suppression de fichier distante ne sera effectué.",
        ));
        if ui
            .checkbox(&mut state.prevent_startup_remote_delete, "")
            .changed()
        {
            events.push(Event::GlobalConfigurationUpdated);
        }

        events
    }
}
