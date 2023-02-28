use eframe::{
    egui::{Grid, Layout, Spinner, TextEdit, Ui},
    emath::Align,
    epaint::Color32,
};
use trsync_core::instance::{Instance, InstanceId, Workspace, WorkspaceId};

use crate::event::Event;

// FIXME : instance as attribute
pub struct InstancePainter {
    updating: bool,
    errors: Vec<String>,
}

const MIN_COL_WIDTH: f32 = 250.;

impl InstancePainter {
    pub fn new(updating: bool, errors: Vec<String>) -> Self {
        Self { updating, errors }
    }

    pub fn draw(&mut self, ui: &mut Ui, instance: &mut GuiInstance) -> Vec<Event> {
        let mut events = vec![];

        ui.vertical(|ui| {
            for error in &self.errors {
                ui.colored_label(Color32::RED, error);
            }

            events.extend(self.credentials(ui, instance));
            ui.separator();
            events.extend(self.workspaces(ui, instance));
        });

        events
    }

    fn credentials(&self, ui: &mut Ui, instance: &mut GuiInstance) -> Vec<Event> {
        let mut events = vec![];

        ui.vertical(|ui| {
            Grid::new(instance.name.to_string())
                .num_columns(3)
                .spacing([40.0, 4.0])
                .striped(true)
                .min_col_width(MIN_COL_WIDTH)
                .show(ui, |ui| {
                    let address_label = ui.label("Adresse (ex. mon.tracim.fr)");
                    ui.text_edit_singleline(&mut instance.address)
                        .labelled_by(address_label.id);
                    ui.end_row();

                    let username_label = ui.label("Identifiant (username ou email)");
                    ui.text_edit_singleline(&mut instance.username)
                        .labelled_by(username_label.id);
                    ui.end_row();

                    let password_label = ui.label("Mot de passe");
                    ui.add(TextEdit::singleline(&mut instance.password).password(true))
                        .labelled_by(password_label.id);
                    ui.end_row();

                    ui.label("");

                    ui.with_layout(Layout::right_to_left(Align::TOP), |ui| {
                        if ui.button("Valider").clicked() {
                            events.push(Event::InstanceCredentialsUpdated(instance.clone()));
                        }
                    });
                    ui.end_row();
                });
        });

        events
    }

    fn workspaces(&self, ui: &mut Ui, instance: &mut GuiInstance) -> Vec<Event> {
        if self.updating {
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                ui.add(Spinner::new());
                ui.add_space(4.0);
                ui.label("La liste des espaces est en cours de chargement");
            });
        }

        if let Some(workspaces) = &instance.workspaces {
            for workspace in workspaces {
                ui.label(&workspace.label);
            }
        }

        vec![]
    }
}

#[derive(Debug, Clone)]
pub struct GuiInstance {
    pub name: InstanceId,
    pub address: String,
    pub unsecure: bool,
    pub username: String,
    pub password: String,
    pub workspaces: Option<Vec<Workspace>>,
    pub selected_workspaces_ids: Vec<WorkspaceId>,
}

impl GuiInstance {
    pub fn from_instance(instance: &Instance) -> Self {
        Self::new(
            instance.name.clone(),
            instance.address.clone(),
            instance.unsecure.clone(),
            instance.username.clone(),
            instance.password.clone(),
            None,
            instance.workspaces_ids.clone(),
        )
    }

    pub fn new(
        name: InstanceId,
        address: String,
        unsecure: bool,
        username: String,
        password: String,
        workspaces: Option<Vec<Workspace>>,
        selected_workspaces_ids: Vec<WorkspaceId>,
    ) -> Self {
        Self {
            name,
            address,
            unsecure,
            username,
            password,
            workspaces,
            selected_workspaces_ids,
        }
    }

    pub fn api_url(&self, suffix: Option<&str>) -> String {
        let suffix = suffix.unwrap_or("");
        let scheme = if self.unsecure { "http" } else { "https" };
        format!("{}://{}/api{}", scheme, self.address, suffix)
    }
}
