use eframe::{
    egui::{Grid, Layout, Spinner, TextEdit, Ui},
    emath::Align,
    epaint::Color32,
};

use crate::event::Event;

use super::instance::GuiInstance;

pub struct AddInstancePainter<'a> {
    updating: bool,
    errors: Vec<String>,
    instance: &'a mut GuiInstance,
}

const MIN_COL_WIDTH: f32 = 250.;

impl<'a> AddInstancePainter<'a> {
    pub fn new(updating: bool, errors: Vec<String>, instance: &'a mut GuiInstance) -> Self {
        Self {
            updating,
            errors,
            instance,
        }
    }

    pub fn draw(&mut self, ui: &mut Ui) -> Vec<Event> {
        let mut events = vec![];

        ui.vertical(|ui| {
            for error in &self.errors {
                ui.colored_label(Color32::RED, error);
            }

            events.extend(self.credentials(ui));
        });

        events
    }

    fn credentials(&mut self, ui: &mut Ui) -> Vec<Event> {
        let mut events = vec![];

        ui.vertical(|ui| {
            Grid::new(self.instance.name.to_string())
                .num_columns(2)
                .spacing([40.0, 4.0])
                .striped(true)
                .min_col_width(MIN_COL_WIDTH)
                .show(ui, |ui| {
                    let address_label = ui.label("Adresse (ex. mon.tracim.fr)");
                    ui.text_edit_singleline(&mut self.instance.address)
                        .labelled_by(address_label.id);
                    ui.end_row();

                    let username_label = ui.label("Identifiant (username ou email)");
                    ui.text_edit_singleline(&mut self.instance.username)
                        .labelled_by(username_label.id);
                    ui.end_row();

                    let password_label = ui.label("Mot de passe");
                    ui.add(TextEdit::singleline(&mut self.instance.password).password(true))
                        .labelled_by(password_label.id);
                    ui.end_row();

                    ui.label("");

                    ui.with_layout(Layout::right_to_left(Align::TOP), |ui| {
                        if ui.button("Ajouter").clicked() {
                            events.push(Event::ValidateNewInstance(self.instance.clone()));
                        }
                    });
                    ui.end_row();
                });
        });

        if self.updating {
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                ui.add(Spinner::new());
                ui.add_space(4.0);
                ui.label("Test de vos identifiants ...");
            });
        }

        events
    }
}
