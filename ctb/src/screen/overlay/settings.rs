use super::Overlay;
use crate::screen::game::{GameMessage, SharedGameData};
use async_trait::async_trait;
use egui_macroquad::egui;
use macroquad::prelude::*;

pub struct Settings {
    volume: f32,
}

impl Settings {
    pub fn new(initial_volume: f32) -> Self {
        Settings {
            volume: initial_volume,
        }
    }
}

#[async_trait(?Send)]
impl Overlay for Settings {
    async fn update(&mut self, data: SharedGameData) {
        let old_volume = self.volume;
        egui_macroquad::ui(|egui_ctx| {
            egui::Window::new("Settings")
                .collapsible(false)
                .frame(
                    egui::Frame::default()
                        .fill(egui::Color32::from_rgba_unmultiplied(64, 64, 64, 240)),
                )
                .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0., 0.))
                .show(egui_ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.label("Volume");
                        ui.add(
                            egui::Slider::new(&mut self.volume, 0.0..=1.0)
                                .clamp_to_range(true)
                                .show_value(false),
                        );
                    });
                });
        });
        if self.volume != old_volume {
            data.broadcast(GameMessage::SetVolume(self.volume));
        }
    }

    fn draw(&self, _data: SharedGameData) {
        egui_macroquad::draw();
    }
}
