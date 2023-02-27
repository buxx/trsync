use eframe::egui::{self, Ui};

use crate::{event::Event, panel::Panel, state::State};

const PIXELS_PER_POINT: f32 = 1.25;

pub struct App {
    pub(crate) state: State,
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        ctx.set_pixels_per_point(PIXELS_PER_POINT);

        egui::CentralPanel::default().show(ctx, |ui| {
            let mut events = vec![];

            events.extend(self.header(ui));
            ui.separator();
            events.extend(self.body(ui));

            self.react(events);
        });
    }
}

impl App {
    pub fn new(state: State) -> Self {
        Self { state }
    }

    pub fn header(&mut self, ui: &mut Ui) -> Vec<Event> {
        ui.horizontal(|ui| {
            for available_panel in &self.state.available_panels {
                let text = available_panel.to_string();
                ui.selectable_value(&mut self.state.current_panel, available_panel.clone(), text);
            }
        });

        vec![]
    }

    pub fn body(&mut self, ui: &mut Ui) -> Vec<Event> {
        let mut events = vec![];

        events.extend(match &self.state.current_panel {
            Panel::Root => self.root(ui),
            Panel::Instance(_id) => todo!(),
        });

        events
    }

    pub fn react(&mut self, events: Vec<Event>) {
        for event in events {
            match event {
                Event::GlobalConfigurationUpdated => {
                    todo!()
                }
            }
        }
    }
}
