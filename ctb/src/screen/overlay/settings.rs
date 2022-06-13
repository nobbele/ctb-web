use super::Overlay;
use crate::{
    config,
    screen::game::{GameMessage, SharedGameData},
};
use async_trait::async_trait;
use egui_macroquad::egui;
use macroquad::prelude::*;

pub struct Settings {
    main_volume: f32,
    hitsound_volume: f32,
    panning: (f32, f32),
    offset_ms: i32,

    max_stack: u32,
    playfield_size: u32,
}

impl Settings {
    pub fn new(data: SharedGameData) -> Self {
        Settings {
            main_volume: data.main_volume(),
            panning: data.panning(),
            hitsound_volume: data.hitsound_volume.get(),
            offset_ms: (data.offset.get() * 1000.) as i32,
            max_stack: data.max_stack.get(),
            playfield_size: (data.playfield_size.get() * 100.) as u32,
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
                    let main_volume = ui.add(
                        egui::Slider::new(&mut self.main_volume, 0.0..=1.0)
                            .clamp_to_range(true)
                            .text("Main")
                            .show_value(false),
                    );
                    if main_volume.changed() {
                        data.broadcast(GameMessage::SetMainVolume(self.main_volume));
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

                    ui.label("Offset");
                    let offset = ui.add(
                        egui::Slider::new(&mut self.offset_ms, -100..=100)
                            .show_value(true)
                            .suffix(" ms")
                            .show_value(true),
                    );
                    if offset.changed() {
                        data.broadcast(GameMessage::SetOffset(self.offset_ms as f32 / 1000.));
                    }

                    let playfield_size_slider = ui.add(
                        egui::Slider::new(&mut self.playfield_size, 10..=100)
                            .clamp_to_range(true)
                            .suffix("%")
                            .text("Playfield Width")
                            .show_value(true),
                    );
                    if playfield_size_slider.changed() {
                        let playfield_size = self.playfield_size as f32 / 100.;
                        data.playfield_size.set(playfield_size);
                        config::set_value("playfield_size", playfield_size);
                    }

                    let max_stack_slider = ui.add(
                        egui::Slider::new(&mut self.max_stack, 4..=32)
                            .clamp_to_range(true)
                            .text("Max Stack")
                            .show_value(true),
                    );
                    if max_stack_slider.changed() {
                        data.max_stack.set(self.max_stack);
                        config::set_value("max_stack", self.max_stack);
                    }
                });
        });
    }

    fn draw(&self, _data: SharedGameData) {
        egui_macroquad::draw();
    }
}
