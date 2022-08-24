use super::Overlay;
use crate::screen::{game::SharedGameData, gameplay::Mod};
use egui_macroquad::egui;

pub struct Mods {
    rate: f32,
}

impl Mods {
    pub fn new(data: SharedGameData) -> Self {
        let mut mods = Mods { rate: 1.0 };

        for to_apply in data.mods.borrow().iter() {
            match to_apply {
                Mod::Rate(rate) => mods.rate = *rate,
            }
        }

        mods
    }
}

impl Overlay for Mods {
    fn update(&mut self, data: SharedGameData) {
        egui_macroquad::ui(|egui_ctx| {
            egui::Window::new("Mods")
                .collapsible(false)
                .frame(
                    egui::Frame::default()
                        .fill(egui::Color32::from_rgba_unmultiplied(64, 64, 64, 240)),
                )
                .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0., 0.))
                .show(egui_ctx, |ui| {
                    ui.add(
                        egui::Slider::new(&mut self.rate, 0.5..=2.0)
                            .clamp_to_range(true)
                            .text("Rate")
                            .show_value(true)
                            .suffix("x"),
                    );
                });
        });

        // Slow but I cba to add a dirty flag.
        let mods = &mut *data.mods.borrow_mut();
        mods.clear();
        if self.rate != 1.0 {
            mods.push(Mod::Rate(self.rate));
        }
    }

    fn draw(&self, _data: SharedGameData) {
        egui_macroquad::draw();
    }
}
