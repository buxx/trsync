use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

use anyhow::Result;
use crossbeam_channel::Receiver;
use eframe::{
    egui::{self, CentralPanel, Context as EguiContext, Layout, Ui},
    emath::Align,
};
use trsync_core::{
    activity::{Activity, ActivityState},
    user::UserRequest,
};

use crate::event::Event;

const PIXELS_PER_POINT: f32 = 1.25;

pub struct App {
    activity_state: Arc<Mutex<ActivityState>>,
    user_request_receiver: Receiver<UserRequest>,
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
    ) -> Self {
        Self {
            activity_state,
            user_request_receiver,
        }
    }

    pub fn start(&mut self) -> Result<()> {
        Ok(())
    }

    fn header(&mut self, ui: &mut Ui) -> Vec<Event> {
        let activity_state = self.activity_state.lock().unwrap();

        ui.horizontal_wrapped(|ui| {
            ui.with_layout(Layout::right_to_left(Align::TOP), |ui| {
                ui.label(format!(
                    "Activité : {}",
                    match activity_state.activity() {
                        Activity::Idle => "Aucune",
                        Activity::Working => "Synchronisation",
                    }
                ));
            });
        });

        vec![]
    }

    fn body(&mut self, ui: &mut Ui) -> Vec<Event> {
        let activity_state = self.activity_state.lock().unwrap();

        ui.label("État par instances");
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

        vec![]
    }
}
