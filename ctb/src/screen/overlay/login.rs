use super::Overlay;
use crate::screen::game::{GameMessage, SharedGameData};
use async_trait::async_trait;
use egui_macroquad::egui;

pub struct Login {
    username: String,
    password: String,
}

impl Login {
    pub fn new(_data: SharedGameData) -> Self {
        Login {
            username: String::new(),
            password: String::new(),
        }
    }
}

#[async_trait(?Send)]
impl Overlay for Login {
    async fn update(&mut self, data: SharedGameData) {
        egui_macroquad::ui(|egui_ctx| {
            egui::Window::new("Login")
                .collapsible(false)
                .frame(
                    egui::Frame::default()
                        .fill(egui::Color32::from_rgba_unmultiplied(64, 64, 64, 240)),
                )
                .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0., 0.))
                .show(egui_ctx, |ui| {
                    ui.label("Username");
                    ui.text_edit_singleline(&mut self.username);

                    ui.label("Password");
                    ui.add(egui::TextEdit::singleline(&mut self.password).password(true));

                    ui.add_space(1.);
                    if ui.button("Login").clicked() {
                        data.broadcast(GameMessage::Login {
                            username: self.username.clone(),
                            password: self.password.clone(),
                        });
                    }
                });
        });
    }

    fn draw(&self, _data: SharedGameData) {
        egui_macroquad::draw();
    }
}
