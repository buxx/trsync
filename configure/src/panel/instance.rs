use std::cmp::Ordering;

use eframe::{
    egui::{Grid, Layout, ScrollArea, Spinner, TextEdit, Ui},
    emath::Align,
    epaint::Color32,
};
use trsync_core::instance::{Instance, InstanceId, Workspace, WorkspaceId};

use crate::event::Event;

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
                .num_columns(2)
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
                        if ui.button("Supprimer").clicked() {
                            events.push(Event::DeleteInstanceWanted(instance.name.clone()));
                        }
                    });
                    ui.end_row();
                });
        });

        events
    }

    fn workspaces(&self, ui: &mut Ui, instance: &mut GuiInstance) -> Vec<Event> {
        let mut events = vec![];

        if self.updating {
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                ui.add(Spinner::new());
                ui.add_space(4.0);
                ui.label("La liste des espaces est en cours de chargement");
            });
        }

        if instance.workspaces.is_some() {
            Grid::new(format!("{}_workspaces", instance.name.to_string()))
                .num_columns(2)
                .spacing([40.0, 4.0])
                .striped(true)
                .min_col_width(MIN_COL_WIDTH)
                .show(ui, |ui| {
                    ui.label("Espaces à synchroniser");
                    ui.set_height(320.);
                    ScrollArea::vertical()
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            ui.vertical(|ui| {
                                for (checked, _, label) in &mut instance.workspaces_ids_checkboxes {
                                    ui.checkbox(checked, label.clone());
                                }
                            })
                        });
                    ui.end_row();

                    ui.label("");
                    ui.with_layout(Layout::right_to_left(Align::TOP), |ui| {
                        if ui.button("Valider").clicked() {
                            events
                                .push(Event::InstanceSelectedWorkspacesValidated(instance.clone()));
                        }
                    });
                });
        }

        events
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
    pub workspaces_ids_checkboxes: Vec<(bool, WorkspaceId, String)>,
}

impl Default for GuiInstance {
    fn default() -> Self {
        Self {
            name: InstanceId("".to_string()),
            address: Default::default(),
            unsecure: Default::default(),
            username: Default::default(),
            password: Default::default(),
            workspaces: Default::default(),
            workspaces_ids_checkboxes: Default::default(),
        }
    }
}

impl GuiInstance {
    pub fn new(
        name: InstanceId,
        address: String,
        unsecure: bool,
        username: String,
        password: String,
        workspaces: Option<Vec<Workspace>>,
        selected_workspaces_ids: Vec<WorkspaceId>,
    ) -> Self {
        let mut self_ = Self {
            name,
            address,
            unsecure,
            username,
            password,
            workspaces,
            workspaces_ids_checkboxes: vec![],
        };
        self_.rebuild_workspaces_ids_checkboxes(&selected_workspaces_ids);
        self_
    }

    pub fn rebuild_workspaces_ids_checkboxes(&mut self, selected_workspaces: &Vec<WorkspaceId>) {
        self.workspaces_ids_checkboxes = vec![];
        if let Some(workspaces) = &self.workspaces {
            for workspace in workspaces {
                self.workspaces_ids_checkboxes.push((
                    selected_workspaces.contains(&workspace.workspace_id),
                    workspace.workspace_id.clone(),
                    workspace.label.clone(),
                ));
            }
        }
        self.workspaces_ids_checkboxes
            .sort_by(|a, b| a.2.partial_cmp(&b.2).unwrap_or(Ordering::Greater));
    }

    pub fn api_url(&self, suffix: Option<&str>) -> String {
        let suffix = suffix.unwrap_or("");
        let scheme = if self.unsecure { "http" } else { "https" };
        format!("{}://{}/api{}", scheme, self.address, suffix)
    }

    pub fn selected_workspace_ids(&self) -> Vec<WorkspaceId> {
        self.workspaces_ids_checkboxes
            .clone()
            .iter()
            .filter_map(
                |(checked, id, _)| {
                    if *checked {
                        Some(id.clone())
                    } else {
                        None
                    }
                },
            )
            .collect()
    }
}

impl From<&Instance> for GuiInstance {
    fn from(instance: &Instance) -> Self {
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
}

impl Into<Instance> for GuiInstance {
    fn into(self) -> Instance {
        Instance {
            name: self.name.clone(),
            address: self.address.clone(),
            unsecure: false,
            username: self.username.clone(),
            password: self.password.clone(),
            workspaces_ids: self.selected_workspace_ids(),
        }
    }
}
