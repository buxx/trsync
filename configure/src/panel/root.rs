use eframe::egui::{Context, Grid, Ui};

use crate::App;

impl App {
    pub fn root(&mut self, ui: &mut Ui, _context: &Context) {
        Grid::new("configuration")
            .num_columns(2)
            .spacing([40.0, 4.0])
            .striped(true)
            .show(ui, |ui| {
                self.base_folder(ui);
            });
    }

    pub fn base_folder(&mut self, ui: &mut Ui) {
        ui.label("Dossier de synchronisation");
        ui.horizontal_wrapped(|ui| {
            if let Some(base_folder) = &self.state.base_folder {
                ui.label(base_folder);
            }
            if ui.button("Sélectionner").clicked() {
                if let Some(path) = rfd::FileDialog::new().pick_folder() {
                    self.state.base_folder = Some(path.display().to_string());
                }
            }
        });
    }
}
