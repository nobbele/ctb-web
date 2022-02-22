use self::{menubutton::MenuButtonMessage, menubuttonlist::MenuButtonListMessage};
use crate::GameData;
use macroquad::prelude::*;
use std::sync::Arc;

pub mod menubutton;
pub mod menubuttonlist;

pub struct Message {
    pub sender: String,
    pub data: MessageData,
}

pub enum MessageData {
    MenuButton(MenuButtonMessage),
    MenuButtonList(MenuButtonListMessage),
}

// Implementors assumed to call set_bounds in its new() method.
// Implementors assumed propogate draw_bounds to children.
pub trait UiElement {
    fn draw(&self, data: Arc<GameData>);
    fn draw_bounds(&self) {
        let bounds = self.bounds();
        draw_rectangle(
            bounds.x,
            bounds.y,
            bounds.w,
            bounds.h,
            Color::new(1.0, 0.0, 0.0, 0.5),
        );
    }

    fn set_bounds(&mut self, rect: Rect);
    fn bounds(&self) -> Rect;
    fn refresh_bounds(&mut self) {
        self.set_bounds(self.bounds());
    }

    fn update(&mut self, data: Arc<GameData>);
    fn handle_message(&mut self, _message: &Message) {}
}
