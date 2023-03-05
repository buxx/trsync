use std::thread;

use anyhow::{Context, Result};
use crossbeam_channel::{unbounded, Receiver, Sender};
use eframe::{
    egui::{CentralPanel, Context as EguiContext, Layout, Ui, Window},
    emath::{Align, Align2},
    epaint::vec2,
};
use trsync_core::{
    instance::{InstanceId, Workspace},
    security::set_password,
};

use crate::{
    event::Event,
    job::{credentials::CredentialUpdater, workspace::WorkspacesGrabber},
    panel::{
        instance::{GuiInstance, InstancePainter},
        root::ConfigurationPainter,
        Panel,
    },
    state::State,
};

const PIXELS_PER_POINT: f32 = 1.25;

pub struct App {
    state: State,
    error_window: Option<String>,
    instance_errors: Vec<(InstanceId, String)>,
    event_receiver: Receiver<Event>,
    event_sender: Sender<Event>,
    updating: Vec<InstanceId>,
}

impl eframe::App for App {
    fn update(&mut self, ctx: &EguiContext, _frame: &mut eframe::Frame) {
        ctx.set_pixels_per_point(PIXELS_PER_POINT);
        let mut events: Vec<Event> = self.event_receiver.try_iter().collect();

        if self.error_window.is_none() {
            CentralPanel::default().show(ctx, |ui| {
                events.extend(self.header(ui));
                ui.separator();
                events.extend(self.body(ui));
            });
        }

        if let Err(error) = self.react(events) {
            self.error_window = Some(format!("{:#}", error))
        };
        self.error_window(ctx);
    }
}

impl App {
    pub fn new(state: State) -> Self {
        let (event_sender, event_receiver) = unbounded();
        Self {
            state,
            error_window: None,
            instance_errors: vec![],
            event_receiver,
            event_sender,
            updating: vec![],
        }
    }

    pub fn start(&mut self) -> Result<()> {
        for instance in &self.state.instances {
            self.updating.push(instance.name.clone());

            let event_sender = self.event_sender.clone();
            let instance_ = GuiInstance::from_instance(instance);
            let instance_name = instance.name.clone();
            thread::Builder::new()
                .name(format!("workspace_grabber"))
                .spawn(|| WorkspacesGrabber::new(event_sender, instance_).execute())
                .context(format!(
                    "Start workspace grabber for '{}'",
                    &instance_name.to_string()
                ))?;
        }

        Ok(())
    }

    fn reset_instance_errors(&mut self, id: &InstanceId) {
        self.instance_errors.retain(|(id_, _)| id_ != id);
    }

    fn add_instance_errors(&mut self, id: InstanceId, error: String) {
        self.instance_errors.push((id, error))
    }

    fn header(&mut self, ui: &mut Ui) -> Vec<Event> {
        let mut change = false;

        ui.horizontal_wrapped(|ui| {
            for available_panel in &self.state.available_panels {
                let text = available_panel.to_string();
                change = ui
                    .selectable_value(&mut self.state.current_panel, available_panel.clone(), text)
                    .changed();
            }
        });

        vec![]
    }

    fn body(&mut self, ui: &mut Ui) -> Vec<Event> {
        let mut events = vec![];

        ui.with_layout(Layout::left_to_right(Align::Min), |ui| {
            events.extend(match &mut self.state.current_panel {
                Panel::Root => ConfigurationPainter::new().draw(ui, &mut self.state),
                Panel::Instance(instance) => {
                    let updating = self.updating.contains(&instance.name);
                    let errors = self
                        .instance_errors
                        .iter()
                        .filter(|(id_, _)| id_ == &instance.name)
                        .map(|(_, error)| error.clone())
                        .collect();
                    InstancePainter::new(updating, errors).draw(ui, instance)
                }
            });
        });

        events
    }

    fn react(&mut self, events: Vec<Event>) -> Result<()> {
        for event in events {
            match event {
                Event::GlobalConfigurationUpdated => self.save_config()?,
                Event::InstanceCredentialsUpdated(instance) => {
                    self.check_instance_credentials(instance)?
                }
                Event::InstanceCredentialsAccepted(instance) => {
                    self.update_instance(&instance);
                    self.save_credentials(&instance)?;
                    self.save_config()?;

                    let event_sender = self.event_sender.clone();
                    let instance_name = instance.name.clone();
                    thread::Builder::new()
                        .name(format!("workspace_grabber"))
                        .spawn(|| WorkspacesGrabber::new(event_sender, instance).execute())
                        .context(format!(
                            "Start workspace grabber for '{}'",
                            &instance_name.to_string()
                        ))?;
                }
                Event::InstanceCredentialsRefused(instance) => {
                    // TODO : removing all matching instance id can hide parallel jobs
                    self.updating.retain(|i| i != &instance.name);
                    self.add_instance_errors(
                        instance.name.clone(),
                        "Identifiant ou mot de passe invalide".to_string(),
                    );
                }
                Event::InstanceCredentialsFailed(instance, error) => {
                    // TODO : removing all matching instance id can hide parallel jobs
                    self.updating.retain(|i| i != &instance.name);
                    self.add_instance_errors(instance.name.clone(), error);
                }
                Event::InstanceWorkspacesRetrievedSuccess(id, workspaces) => {
                    // TODO : removing all matching instance id can hide parallel jobs
                    self.updating.retain(|i| i != &id);
                    self.update_gui_instance_workspaces(&id, &workspaces);
                }
                Event::InstanceWorkspacesRetrievedFailure(id, error) => {
                    self.updating.retain(|i| i != &id);
                    self.add_instance_errors(id, error);
                }
                Event::InstanceSelectedWorkspacesValidated(instance) => {
                    self.update_instance_selected_workspaces(&instance);
                    self.save_config()?;
                }
            }
        }

        Ok(())
    }

    fn update_instance(&mut self, instance: &GuiInstance) {
        if let Some(instance_) = self
            .state
            .instances
            .iter_mut()
            .find(|i| i.name == instance.name)
        {
            instance_.address = instance.address.clone();
            instance_.username = instance.username.clone();
            instance_.password = instance.password.clone();
        };
    }

    fn save_credentials(&self, instance: &GuiInstance) -> Result<()> {
        set_password(
            &instance.name.to_string(),
            &whoami::username(),
            &instance.password,
        )
        .context(format!(
            "Save password in keyring system for '{}'",
            &instance.name.to_string()
        ))?;
        Ok(())
    }

    fn update_gui_instance_workspaces(&mut self, id: &InstanceId, workspaces: &Vec<Workspace>) {
        if let Some(gui_instance) = self
            .state
            .available_panels
            .iter_mut()
            .filter_map(|p| match p {
                Panel::Root => None,
                Panel::Instance(i) => Some(i),
            })
            .find(|i| &i.name == id)
        {
            gui_instance.workspaces = Some(workspaces.clone());
            gui_instance.rebuild_workspaces_ids_checkboxes();
        };
    }

    fn update_instance_selected_workspaces(&mut self, instance: &GuiInstance) {
        if let Some(instance_) = self
            .state
            .instances
            .iter_mut()
            .find(|i| i.name == instance.name)
        {
            instance_.workspaces_ids = instance
                .workspaces_ids_checkboxes
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
                .collect();
        };
    }

    fn save_config(&self) -> Result<()> {
        self.state.to_config().write()?;
        Ok(())
    }

    fn error_window(&mut self, ctx: &EguiContext) {
        let mut close = false;

        if let Some(error) = &self.error_window {
            Window::new("⚠ Une erreur est survenue ⚠")
                .collapsible(false)
                .resizable(false)
                .anchor(Align2::CENTER_CENTER, vec2(0., 0.))
                .show(ctx, |ui| {
                    ui.label(error.to_string());
                    ui.with_layout(Layout::right_to_left(Align::TOP), |ui| {
                        close = ui.button("Fermer").clicked();
                    })
                });
        }

        if close {
            self.error_window = None;
        }
    }

    fn check_instance_credentials(&mut self, instance: GuiInstance) -> Result<()> {
        self.reset_instance_errors(&instance.name);
        let mut errors = vec![];

        if instance.address.trim().is_empty() {
            errors.push("Veuillez saisir une adresse (ex. mon.tracim.fr)".to_string());
        }

        if instance.username.trim().is_empty() {
            errors.push("Veuillez saisir un identifiant (username ou email)".to_string());
        }

        if instance.password.trim().is_empty() {
            errors.push("Veuillez saisir un mot de passe".to_string());
        }

        if !errors.is_empty() {
            for error in errors {
                self.add_instance_errors(instance.name.clone(), error);
            }
            return Ok(());
        }

        let event_sender = self.event_sender.clone();
        let instance_name = instance.name.clone();
        thread::Builder::new()
            .name(format!("credential_updater"))
            .spawn(|| CredentialUpdater::new(event_sender, instance).execute())
            .context(format!(
                "Start credential updater for '{}'",
                &instance_name.to_string()
            ))?;

        self.updating.push(instance_name.clone());
        Ok(())
    }
}
