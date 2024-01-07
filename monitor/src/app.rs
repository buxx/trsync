use std::{
    fmt::Display,
    sync::{Arc, Mutex},
    time::Duration,
};

use anyhow::Result;
use crossbeam_channel::Receiver;
use eframe::egui::{self, CentralPanel, Context as EguiContext, Ui};
use strum::IntoEnumIterator;
use strum_macros::EnumIter;
use trsync_core::{activity::ActivityState, sync::SyncExchanger, user::UserRequest};

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

pub struct App {
    activity_state: Arc<Mutex<ActivityState>>,
    user_request_receiver: Receiver<UserRequest>,
    sync_exchanger: Arc<Mutex<SyncExchanger>>,
    current_panel: Panel,
}

impl eframe::App for App {
    fn update(&mut self, ctx: &EguiContext, frame: &mut eframe::Frame) {
        let mut events = vec![];
        // Needed to display changes (like activity)
        ctx.request_repaint_after(Duration::from_millis(250));
        // Zoom a little the interface
        ctx.set_pixels_per_point(PIXELS_PER_POINT);

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
    ) -> Self {
        Self {
            activity_state,
            user_request_receiver,
            sync_exchanger,
            current_panel: Panel::Root,
        }
    }

    pub fn start(&mut self) -> Result<()> {
        Ok(())
    }

    fn header(&mut self, ui: &mut Ui) -> Vec<Event> {
        ui.horizontal_wrapped(|ui| {
            for panel in Panel::iter() {
                let text = panel.to_string();
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
                for (job_identifier, job_count) in activity_state.jobs() {
                    ui.label(&job_identifier.instance_name);
                    ui.label(&job_identifier.workspace_name);
                    ui.label(if job_count > &0 {
                        "Synchronization"
                    } else {
                        "En veille"
                    });
                    ui.end_row();
                }
            });
    }

    fn synchronizations_body(&self, ui: &mut Ui) {
        for (job_identifier, sync_channels) in self.sync_exchanger.lock().unwrap().channels() {
            let mut changes = sync_channels.changes().lock().unwrap();

            for change in changes.iter() {
                ui.label(format!(
                    "{}::{} : {:?}",
                    &job_identifier.instance_name, &job_identifier.workspace_name, change
                ));
            }

            if changes.is_some() && ui.button("Ok").clicked() {
                *changes = None;
                if sync_channels.confirm_sync_sender().send(true).is_err() {
                    log::error!(
                        "Unable to communicate with trsync to confirm startup sync for {}::{}",
                        &job_identifier.instance_name,
                        &job_identifier.workspace_name
                    );
                }
            }
        }
    }

    fn errors_body(&self, _ui: &mut Ui) {}
}
