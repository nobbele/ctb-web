use super::Overlay;
use crate::screen::game::{GameMessage, SharedGameData};
use async_trait::async_trait;
use egui_macroquad::egui;
use macroquad::prelude::*;

pub struct Settings {
    master_volume: f32,
    hitsound_volume: f32,
    panning: (f32, f32),
}

impl Settings {
    pub fn new(data: SharedGameData) -> Self {
        Settings {
            master_volume: data.master_volume(),
            panning: data.panning(),
            hitsound_volume: data.hitsound_volume.get(),
        }
    }
}

#[async_trait(?Send)]
impl Overlay for Settings {
    async fn update(&mut self, data: SharedGameData) {
        egui_macroquad::ui(|egui_ctx| {
            egui::Window::new("Settings")
                .collapsible(false)
                .frame(
                    egui::Frame::default()
                        .fill(egui::Color32::from_rgba_unmultiplied(64, 64, 64, 240)),
                )
                .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0., 0.))
                .show(egui_ctx, |ui| {
                    ui.label("Volume");
                    let master_volume = ui.add(
                        egui::Slider::new(&mut self.master_volume, 0.0..=1.0)
                            .clamp_to_range(true)
                            .text("Master")
                            .show_value(false),
                    );
                    if master_volume.changed() {
                        data.broadcast(GameMessage::SetMasterVolume(self.master_volume));
                    }

                    let hitsound_volume = ui.add(
                        egui::Slider::new(&mut self.hitsound_volume, 0.0..=1.0)
                            .clamp_to_range(true)
                            .text("Hitsound")
                            .show_value(false),
                    );
                    if hitsound_volume.changed() {
                        data.broadcast(GameMessage::SetHitsoundVolume(self.hitsound_volume));
                    }

                    ui.label("Panning");
                    ui.vertical(|ui| {
                        let left_pan = ui.add(
                            egui::Slider::new(&mut self.panning.0, 0.0..=1.0)
                                .clamp_to_range(true)
                                .fixed_decimals(2)
                                .step_by(0.05)
                                .text("Left")
                                .show_value(true),
                        );
                        if left_pan.changed() {
                            self.panning.0 = self.panning.0.min(self.panning.1);
                        }
                        let right_pan = ui.add(
                            egui::Slider::new(&mut self.panning.1, 0.0..=1.0)
                                .clamp_to_range(true)
                                .fixed_decimals(2)
                                .step_by(0.05)
                                .text("Right")
                                .show_value(true),
                        );
                        if right_pan.changed() {
                            self.panning.1 = self.panning.1.max(self.panning.0);
                        }

                        if left_pan.changed() || right_pan.changed() {
                            data.set_panning(self.panning.0, self.panning.1);
                        }
                    });
                });
        });
    }

    fn draw(&self, _data: SharedGameData) {
        egui_macroquad::draw();
    }
}