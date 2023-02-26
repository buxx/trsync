#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release

use eframe::egui;
use state::State;

mod panel;
mod state;

fn main() -> Result<(), eframe::Error> {
    let options = eframe::NativeOptions {
        initial_window_size: Some(egui::vec2(800.0, 600.0)),
        ..Default::default()
    };
    eframe::run_native("TrSync", options, Box::new(|_cc| Box::new(App::default())))
}

struct App {
    state: State,
}

impl Default for App {
    fn default() -> Self {
        Self {
            state: State::default(),
        }
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ctx.set_pixels_per_point(1.5);

            ui.horizontal(|ui| {
                for available_panel in &self.state.available_panels {
                    let text = available_panel.to_string();
                    ui.selectable_value(
                        &mut self.state.current_panel,
                        available_panel.clone(),
                        text,
                    );
                }
            });
            ui.separator();

            match &self.state.current_panel {
                panel::Panel::Root => self.root(ui, ctx),
                panel::Panel::Instance(_id) => todo!(),
            }

            // ui.horizontal(|ui| {
            //     let name_label = ui.label("Your name: ");
            //     ui.text_edit_singleline(&mut self.name)
            //         .labelled_by(name_label.id);
            // });
            // ui.add(egui::Slider::new(&mut self.age, 0..=120).text("age"));
            // if ui.button("Click each year").clicked() {
            //     self.age += 1;
            // }
            // ui.label(format!("Hello '{}', age {}", self.name, self.age));
        });
    }
}
