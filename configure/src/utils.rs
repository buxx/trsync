use eframe::egui::{Ui, Widget};

pub fn label_with_help<'a>(text: &'a str, help: &'a str) -> impl Widget + 'a {
    move |ui: &mut Ui| {
        ui.label(text).on_hover_ui(|ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label(help);
            });
        })
    }
}
