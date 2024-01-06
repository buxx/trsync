use std::thread;

use anyhow::{Context, Result};
use crossbeam_channel::{unbounded, Receiver, Sender};
use eframe::{
    egui::{CentralPanel, Context as EguiContext, Layout, Ui, Window},
    emath::{Align, Align2},
    epaint::vec2,
};
use trsync_core::{
    instance::{Instance, InstanceId, Workspace},
    security::set_password,
    user::UserRequest,
};
use trsync_manager::message::DaemonMessage;

use crate::{
    event::Event,
    job::{credentials::CredentialUpdater, workspace::WorkspacesGrabber},
    panel::{
        add::AddInstancePainter,
        instance::{GuiInstance, InstancePainter},
        root::ConfigurationPainter,
        Panel,
    },
    state::State,
};

const PIXELS_PER_POINT: f32 = 1.25;

pub struct App {
    state: State,
    main_sender: Sender<DaemonMessage>,
    windowed_error: Option<String>,
    instance_errors: Vec<(InstanceId, String)>,
    event_receiver: Receiver<Event>,
    event_sender: Sender<Event>,
    updating: Vec<InstanceId>,
    delete_instance: Option<InstanceId>,
    user_request_receiver: Receiver<UserRequest>,
}

impl eframe::App for App {
    fn update(&mut self, ctx: &EguiContext, frame: &mut eframe::Frame) {
        ctx.set_pixels_per_point(PIXELS_PER_POINT);
        let mut events: Vec<Event> = self.event_receiver.try_iter().collect();

        if self.windowed_error.is_none() && self.delete_instance.is_none() {
            CentralPanel::default().show(ctx, |ui| {
                events.extend(self.header(ui));
                ui.separator();
                events.extend(self.body(ui));
            });
        }

        if let Err(error) = self.react(events) {
            self.windowed_error = Some(format!("{:#}", error))
        };

        self.error_window(ctx);

        if let Err(error) = self.deletion_window(ctx) {
            self.windowed_error = Some(format!("{:#}", error))
        };

        // Exit window if user request something from systray
        if !self.user_request_receiver.is_empty() {
            frame.close()
        }
    }
}

impl App {
    pub fn new(
        state: State,
        main_sender: Sender<DaemonMessage>,
        user_request_receiver: Receiver<UserRequest>,
    ) -> Self {
        let (event_sender, event_receiver) = unbounded();
        Self {
            state,
            main_sender,
            windowed_error: None,
            instance_errors: vec![],
            event_receiver,
            event_sender,
            updating: vec![],
            delete_instance: None,
            user_request_receiver,
        }
    }

    pub fn start(&mut self) -> Result<()> {
        for instance in &self.state.instances {
            self.updating.push(instance.name.clone());

            let event_sender = self.event_sender.clone();
            let instance_: GuiInstance = instance.into();
            let instance_name = instance.name.clone();
            thread::Builder::new()
                .name("workspace_grabber".to_string())
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
                    InstancePainter::new(updating, errors, instance).draw(ui)
                }
                Panel::AddInstance(instance) => {
                    let updating = self.updating.contains(&instance.name);
                    let errors = self
                        .instance_errors
                        .iter()
                        .filter(|(id_, _)| id_ == &instance.name)
                        .map(|(_, error)| error.clone())
                        .collect();
                    AddInstancePainter::new(updating, errors, instance).draw(ui)
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
                Event::InstanceCredentialsAccepted(mut instance) => {
                    if instance.name.is_new() {
                        instance = self.add_instance(&instance);
                        self.updating.retain(|instance_id| !instance_id.is_new());
                        self.clear_add_instance_panel();
                    } else {
                        self.update_instance(&instance);
                    }

                    self.save_credentials(&instance)?;
                    self.save_config()?;

                    let event_sender = self.event_sender.clone();
                    let instance_name = instance.name.clone();
                    thread::Builder::new()
                        .name("workspace_grabber".to_string())
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
                Event::ValidateNewInstance(instance) => {
                    self.check_instance_credentials(instance)?
                }
                Event::DeleteInstanceWanted(instance_id) => {
                    self.delete_instance = Some(instance_id)
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

    fn add_instance(&mut self, instance: &GuiInstance) -> GuiInstance {
        let mut instance_ = instance.clone();
        instance_.name = InstanceId(instance.address.clone());

        // Add a panel for this new instance
        self.state.available_panels.insert(
            self.state.available_panels.len() - 1,
            Panel::Instance(instance_.clone()),
        );

        // Clear new instance form by modifying panel instance object
        if let Some(gui_instance) = self
            .state
            .available_panels
            .iter_mut()
            .filter_map(|p| match p {
                Panel::AddInstance(i) => Some(i),
                _ => None,
            })
            .collect::<Vec<&mut GuiInstance>>()
            .first_mut()
        {
            gui_instance.address = "".to_string();
            gui_instance.username = "".to_string();
            gui_instance.password = "".to_string();
        }

        // Add instance to instances list
        self.state.instances.push(instance_.clone().into());

        instance_
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
        let selected_workspaces = self
            .state
            .instances
            .iter()
            .filter(|i| &i.name == id)
            .collect::<Vec<&Instance>>()
            .first()
            .map(|i| i.workspaces_ids.clone())
            .unwrap_or(vec![]);
        if let Some(gui_instance) = self
            .state
            .available_panels
            .iter_mut()
            .filter_map(|p| match p {
                Panel::Instance(i) => Some(i),
                _ => None,
            })
            .find(|i| &i.name == id)
        {
            gui_instance.workspaces = Some(workspaces.clone());
            gui_instance.rebuild_workspaces_ids_checkboxes(&selected_workspaces);
        };
    }

    fn update_instance_selected_workspaces(&mut self, instance: &GuiInstance) {
        let selected_workspace_ids = instance.selected_workspace_ids();

        if let Some(instance_) = self
            .state
            .instances
            .iter_mut()
            .find(|i| i.name == instance.name)
        {
            instance_.workspaces_ids = selected_workspace_ids.clone();
        };

        if let Some(instance_) = self
            .state
            .available_panels
            .iter_mut()
            .filter_map(|p| match p {
                Panel::Instance(i) => Some(i),
                _ => None,
            })
            .find(|i| i.name == instance.name)
        {
            instance_.rebuild_workspaces_ids_checkboxes(&selected_workspace_ids);
        };
    }

    fn save_config(&mut self) -> Result<()> {
        let config = self.state.to_config();
        config.write()?;
        if let Err(error) = self.main_sender.send(DaemonMessage::Reload(config)) {
            self.windowed_error = Some(error.to_string())
        };
        Ok(())
    }

    fn error_window(&mut self, ctx: &EguiContext) {
        let mut close = false;

        if let Some(error) = &self.windowed_error {
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
            self.windowed_error = None;
        }
    }

    fn deletion_window(&mut self, ctx: &EguiContext) -> Result<()> {
        let mut close = false;
        let mut confirm = false;

        if let Some(instance_id) = &self.delete_instance {
            Window::new(&format!("Supprimer {} ?", instance_id))
                .collapsible(false)
                .resizable(false)
                .anchor(Align2::CENTER_CENTER, vec2(0., 0.))
                .show(ctx, |ui| {
                    ui.label(format!("Voulez-vous réellement supprimer {} ?\nLes données sur votre disque dur ne seront pas supprimés.", instance_id));
                    ui.with_layout(Layout::right_to_left(Align::TOP), |ui| {
                        confirm = ui.button("Supprimer").clicked();
                        close = ui.button("Annuler").clicked();
                    })
                });

            if confirm {
                self.state.remove_instance(instance_id);
                self.save_config()?;
            }
        }

        if close || confirm {
            self.delete_instance = None;
        }
        Ok(())
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
            .name("credential_updater".to_string())
            .spawn(|| CredentialUpdater::new(event_sender, instance).execute())
            .context(format!(
                "Start credential updater for '{}'",
                &instance_name.to_string()
            ))?;

        self.updating.push(instance_name.clone());
        Ok(())
    }

    pub fn clear_add_instance_panel(&mut self) {
        if let Some(gui_instance) = match &mut self.state.current_panel {
            Panel::AddInstance(instance) => Some(instance),
            _ => None,
        } {
            gui_instance.address.clear();
            gui_instance.username.clear();
            gui_instance.password.clear();
        }
    }
}
