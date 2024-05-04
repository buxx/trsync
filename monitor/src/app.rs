use std::{
    fmt::{Display, Write},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use anyhow::Result;
use crossbeam_channel::Receiver;
use eframe::{
    egui::{self, CentralPanel, Context as EguiContext, RichText, Ui},
    epaint::Color32,
};
use strum::IntoEnumIterator;
use strum_macros::EnumIter;
use trsync_core::{
    activity::ActivityState,
    error::{Decision, ErrorExchanger, OperatorError, RunnerError, StateError},
    job::JobIdentifier,
    sync::SyncExchanger,
    user::{MonitorWindowPanel, UserRequest},
};

use crate::event::Event;

const PIXELS_PER_POINT: f32 = 1.25;

#[derive(EnumIter, Eq, PartialEq)]
pub enum Panel {
    Root,
    StartupSynchronizations,
    Errors,
}

impl Display for Panel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Panel::Root => f.write_str("Moniteur"),
            Panel::StartupSynchronizations => f.write_str("Synchronizations"),
            Panel::Errors => f.write_str("Erreurs"),
        }
    }
}

impl From<MonitorWindowPanel> for Panel {
    fn from(value: MonitorWindowPanel) -> Self {
        match value {
            MonitorWindowPanel::Root => Self::Root,
            MonitorWindowPanel::StartupConfirmations => Self::StartupSynchronizations,
        }
    }
}

enum BlinkingChar {
    On,
    Off,
}
impl BlinkingChar {
    fn next(&self) -> BlinkingChar {
        match self {
            BlinkingChar::On => BlinkingChar::Off,
            BlinkingChar::Off => BlinkingChar::On,
        }
    }
}

impl Display for BlinkingChar {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BlinkingChar::On => f.write_char('⏺'),
            BlinkingChar::Off => f.write_char('○'),
        }
    }
}

pub struct App {
    activity_state: Arc<Mutex<ActivityState>>,
    user_request_receiver: Receiver<UserRequest>,
    sync_exchanger: Arc<Mutex<SyncExchanger>>,
    error_exchanger: Arc<Mutex<ErrorExchanger>>,
    current_panel: Panel,
    current_sync_space: Option<JobIdentifier>,
    current_error_space: Option<JobIdentifier>,
    blinking_char: BlinkingChar,
    last_blinking: Instant,
}

impl eframe::App for App {
    fn update(&mut self, ctx: &EguiContext, frame: &mut eframe::Frame) {
        let mut events = vec![];
        // Needed to display changes (like activity)
        ctx.request_repaint_after(Duration::from_millis(250));
        // Zoom a little the interface
        ctx.set_pixels_per_point(PIXELS_PER_POINT);

        self.update_synchronizations_combo_box_default_value();
        self.update_error_combo_box_default_value();
        self.update_blinking_char();
        self.update_error_space_seen();

        CentralPanel::default().show(ctx, |ui| {
            events.extend(self.header(ui));
            ui.separator();
            events.extend(self.body(ui));
        });

        // Exit window if user request something from systray
        if !self.user_request_receiver.is_empty() {
            frame.close()
        }

        // events ...
    }
}

impl App {
    pub fn new(
        activity_state: Arc<Mutex<ActivityState>>,
        user_request_receiver: Receiver<UserRequest>,
        sync_exchanger: Arc<Mutex<SyncExchanger>>,
        error_exchanger: Arc<Mutex<ErrorExchanger>>,
        panel: MonitorWindowPanel,
    ) -> Self {
        Self {
            activity_state,
            user_request_receiver,
            sync_exchanger,
            error_exchanger,
            current_panel: panel.into(),
            current_sync_space: None,
            current_error_space: None,
            blinking_char: BlinkingChar::Off,
            last_blinking: Instant::now(),
        }
    }

    pub fn start(&mut self) -> Result<()> {
        Ok(())
    }

    fn update_synchronizations_combo_box_default_value(&mut self) {
        if self.current_sync_space.is_none() {
            if let Some(waiting_space) = self.waiting_spaces().first() {
                self.current_sync_space = Some(waiting_space.clone())
            }
        }
    }

    fn update_error_combo_box_default_value(&mut self) {
        if self.current_error_space.is_none() {
            if let Some(error_space) = self.error_spaces().first() {
                self.current_error_space = Some(error_space.clone())
            }
        }
    }

    fn header(&mut self, ui: &mut Ui) -> Vec<Event> {
        ui.horizontal_wrapped(|ui| {
            for panel in Panel::iter() {
                let text = match &panel {
                    Panel::StartupSynchronizations => {
                        if !self.waiting_spaces().is_empty() {
                            format!("{} {}", panel, self.blinking_char)
                        } else {
                            panel.to_string()
                        }
                    }
                    Panel::Errors => {
                        if !self.error_spaces().is_empty() {
                            format!("{} {}", panel, self.blinking_char)
                        } else {
                            panel.to_string()
                        }
                    }
                    _ => panel.to_string(),
                };
                ui.selectable_value(&mut self.current_panel, panel, text);
            }
        });

        vec![]
    }

    fn body(&mut self, ui: &mut Ui) -> Vec<Event> {
        match self.current_panel {
            Panel::Root => self.root_body(ui),
            Panel::StartupSynchronizations => self.synchronizations_body(ui),
            Panel::Errors => self.errors_body(ui),
        }

        vec![]
    }

    fn root_body(&self, ui: &mut Ui) {
        let activity_state = self.activity_state.lock().unwrap();

        ui.label("État par espaces");
        egui::Grid::new("instances_states")
            .num_columns(3)
            .spacing([40.0, 4.0])
            .striped(true)
            .show(ui, |ui| {
                for (job_identifier, activity) in activity_state.activities() {
                    ui.label(&job_identifier.instance_name);
                    ui.label(&job_identifier.workspace_name);
                    ui.label(activity.to_string());
                    ui.end_row();
                }
            });
    }

    fn waiting_spaces(&self) -> Vec<JobIdentifier> {
        let binding = self.sync_exchanger.lock().unwrap();
        let channels = binding.channels();
        channels
            .iter()
            .filter(|(_, channels)| channels.changes().lock().unwrap().is_some())
            .map(|(i, _)| i.clone())
            .collect()
    }

    fn error_spaces(&self) -> Vec<JobIdentifier> {
        let binding = self.error_exchanger.lock().unwrap();
        let channels = binding.channels();
        channels
            .iter()
            .filter(|(_, channels)| channels.error().lock().unwrap().is_some())
            .map(|(i, _)| i.clone())
            .collect()
    }

    fn update_blinking_char(&mut self) {
        if self.last_blinking.elapsed() > Duration::from_millis(750) {
            self.blinking_char = self.blinking_char.next();
            self.last_blinking = Instant::now();
        }
    }

    fn update_error_space_seen(&self) {
        if self.current_panel == Panel::Errors {
            let binding = self.error_exchanger.lock().unwrap();
            let channels = binding.channels();

            if let Some(error_space) = &self.current_error_space {
                if let Some(sync_channels) = channels.get(error_space) {
                    if !sync_channels.seen() {
                        sync_channels.set_seen()
                    }
                }
            }
        }
    }

    fn synchronizations_body(&mut self, ui: &mut Ui) {
        self.synchronizations_combo_box(ui);
        self.synchronizations_display(ui);
    }

    fn synchronizations_combo_box(&mut self, ui: &mut Ui) {
        let waiting_spaces = self.waiting_spaces();

        egui::ComboBox::from_label("Espaces en attente de confirmation")
            .selected_text(
                self.current_sync_space
                    .as_ref()
                    .map(|x| x.to_string())
                    .unwrap_or("".to_string()),
            )
            .show_ui(ui, |ui| {
                ui.style_mut().wrap = Some(false);
                ui.set_min_width(120.0);
                for waiting_space in waiting_spaces {
                    ui.selectable_value(
                        &mut self.current_sync_space,
                        Some(waiting_space.clone()),
                        waiting_space.to_string(),
                    );
                }
            });
    }

    fn synchronizations_display(&mut self, ui: &mut Ui) {
        let binding = self.sync_exchanger.lock().unwrap();
        let channels = binding.channels();
        let mut answered = false;

        if let Some(waiting_space) = &self.current_sync_space {
            if let Some(sync_channels) = channels.get(waiting_space) {
                let mut changes = sync_channels.changes().lock().unwrap();

                ui.label(
                    "La synchronisation de départ de cet espace inclura les changements suivants :",
                );

                egui::ScrollArea::both().show(ui, |ui| {
                    egui::Grid::new("instances_states")
                    .num_columns(2)
                    .spacing([40.0, 4.0])
                    .striped(true)
                    .show(ui, |ui| {
                        for change in changes.iter() {
                            for remote_change in &change.0 {
                                ui.label(remote_change.utf8_icon());
                                ui.label(format!("{}", remote_change.path().display()));
                                ui.end_row();
                            }
                            for local_change in &change.1 {
                                ui.label(local_change.utf8_icon());
                                ui.label(format!("{}", local_change.path().display()));
                                ui.end_row();
                            }
                        }
                    });

                    ui.horizontal_wrapped(|ui| {
                        if changes.is_some() && ui.button("Refuser").clicked() {
                            *changes = None;
                            answered = true;
                            if sync_channels.confirm_sync_sender().send(false).is_err() {
                                log::error!(
                                    "Unable to communicate with trsync to confirm startup sync for {}::{}",
                                    &waiting_space.instance_name,
                                    &waiting_space.workspace_name
                                );
                            };
                        };

                        if changes.is_some() && ui.button("Accepter").clicked() {
                            *changes = None;
                            answered = true;
                            if sync_channels.confirm_sync_sender().send(true).is_err() {
                                log::error!(
                                    "Unable to communicate with trsync to refuse startup sync for {}::{}",
                                    &waiting_space.instance_name,
                                    &waiting_space.workspace_name
                                );
                            };
                        };
                    });

                });
            }
        }

        if answered {
            self.current_sync_space = None;
        }
    }

    fn errors_body(&mut self, ui: &mut Ui) {
        self.errors_combo_box(ui);
        self.errors_display(ui);
    }

    fn errors_combo_box(&mut self, ui: &mut Ui) {
        let error_spaces = self.error_spaces();

        egui::ComboBox::from_label("Espaces en erreur")
            .selected_text(
                self.current_error_space
                    .as_ref()
                    .map(|x| x.to_string())
                    .unwrap_or("".to_string()),
            )
            .show_ui(ui, |ui| {
                ui.style_mut().wrap = Some(false);
                ui.set_min_width(120.0);
                for error_space in error_spaces {
                    ui.selectable_value(
                        &mut self.current_error_space,
                        Some(error_space.clone()),
                        error_space.to_string(),
                    );
                }
            });
    }

    fn errors_display(&mut self, ui: &mut Ui) {
        let binding = self.error_exchanger.lock().unwrap();
        let channels = binding.channels();

        if let Some(error_space) = &self.current_error_space {
            if let Some(sync_channels) = channels.get(error_space) {
                let mut answered = false;
                let mut error_guard = sync_channels.error().lock().unwrap();

                if let Some(error) = (*error_guard).as_ref() {
                    let message = match error {
                        RunnerError::OperatorError(OperatorError::StateError(
                            StateError::PathAlreadyExist(path, _),
                        )) => format!("Un contenu semble dupliqué : '{}'", path.display()),
                        _ => error.to_string(),
                    };

                    ui.label("Cet espace de travail à rencontré une erreur :");
                    ui.label(RichText::new(message).color(Color32::RED));
                    ui.horizontal_wrapped(|ui| {
                        if let RunnerError::OperatorError(OperatorError::StateError(
                            StateError::PathAlreadyExist(_, content_id),
                        )) = error
                        {
                            if ui
                                .button("Ignorer à l'avenir & Redémarrer la synchronisation")
                                .clicked()
                            {
                                answered = true;
                                if sync_channels
                                    .decision_sender()
                                    .send(Decision::IgnoreAndRestartSpaceSync(*content_id))
                                    .is_err()
                                {
                                    print!("Unable to send decision to {}", error_space)
                                }
                            }
                        }

                        if ui.button("Redémarrer la synchronisation").clicked() {
                            answered = true;
                            if sync_channels
                                .decision_sender()
                                .send(Decision::RestartSpaceSync)
                                .is_err()
                            {
                                print!("Unable to send decision to {}", error_space)
                            }
                        }
                    });
                }
                if answered {
                    self.current_error_space = None;
                    *error_guard = None;
                }
            }
        }
    }
}
