use super::{Message, MessageData, UiElement};
use crate::{draw_text_centered, screen::game::SharedGameData};
use macroquad::prelude::*;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Popout {
    None,
    Left,
    Right,
    Towards,
}

pub enum MenuButtonMessage {
    Selected,
    Unselected,
    Hovered,
    Unhovered,
}

pub struct MenuButton {
    pub id: String,
    title: Vec<String>,
    rect: Rect,
    visible_rect: Rect,
    pub tx: flume::Sender<Message>,
    hovered: bool,
    selected: bool,
    offset: f32,
    popout: Popout,
}
const SELECTED_COLOR: Color = Color::new(1.0, 1.0, 1.0, 1.0);
const HOVERED_COLOR: Color = Color::new(0.5, 0.5, 0.8, 1.0);
const IDLE_COLOR: Color = Color::new(0.5, 0.5, 0.5, 1.0);

impl MenuButton {
    pub fn new(
        id: String,
        title: Vec<String>,
        popout: Popout,
        rect: Rect,
        tx: flume::Sender<Message>,
    ) -> Self {
        let mut button = MenuButton {
            id,
            title,
            rect: Rect::default(),
            visible_rect: Rect::default(),
            tx,
            hovered: false,
            selected: false,
            offset: 0.,
            popout,
        };
        button.set_bounds(rect);
        button
    }
}

impl UiElement for MenuButton {
    fn draw(&self, data: SharedGameData) {
        draw_texture_ex(
            data.button,
            self.visible_rect.x,
            self.visible_rect.y,
            if self.selected {
                SELECTED_COLOR
            } else if self.hovered {
                HOVERED_COLOR
            } else {
                IDLE_COLOR
            },
            DrawTextureParams {
                dest_size: Some(vec2(self.visible_rect.w, self.visible_rect.h)),
                ..Default::default()
            },
        );
        for (idx, title) in self.title.iter().enumerate() {
            let title_length = measure_text(title, None, 36, 1.);
            draw_text_centered(
                title,
                self.visible_rect.x + self.visible_rect.w / 2.,
                self.visible_rect.y
                    + self.visible_rect.h / 2.
                    + title_length.height / 2.
                    + (title_length.height + 2.) * idx as f32,
                32,
                WHITE,
            );
        }
    }

    fn update(&mut self, _data: SharedGameData) {
        if self.visible_rect.contains(mouse_position().into()) {
            if !self.hovered {
                self.tx
                    .send(Message {
                        target: self.id.clone(),
                        data: MessageData::MenuButton(MenuButtonMessage::Hovered),
                    })
                    .unwrap();
            }
            if is_mouse_button_pressed(MouseButton::Left) {
                self.tx
                    .send(Message {
                        target: self.id.clone(),
                        data: MessageData::MenuButton(MenuButtonMessage::Selected),
                    })
                    .unwrap();
            }
        } else if self.hovered {
            self.tx
                .send(Message {
                    target: self.id.clone(),
                    data: MessageData::MenuButton(MenuButtonMessage::Unhovered),
                })
                .unwrap();
        }

        if self.selected {
            self.offset += 2. * get_frame_time();
        } else if self.hovered {
            if self.offset <= 0.8 {
                self.offset += 2. * get_frame_time();
                self.offset = self.offset.min(0.8);
            }
        } else {
            self.offset -= 2. * get_frame_time();
        }
        self.offset = self.offset.clamp(0., 1.0);
        let x_offset = self.offset * self.rect.w / 10.;
        let y_offset = self.offset * self.rect.h / 10.;

        match self.popout {
            Popout::None => {}
            Popout::Left => {
                self.visible_rect.x = self.rect.x - x_offset;
            }
            Popout::Right => {
                self.visible_rect.x = self.rect.x + x_offset;
            }
            Popout::Towards => {
                self.visible_rect.w = self.rect.w + x_offset;
                self.visible_rect.h = self.rect.h + y_offset;
                self.visible_rect.x = self.rect.x - x_offset / 2.;
                self.visible_rect.y = self.rect.y - y_offset / 2.;
            }
        }
    }

    fn handle_message(&mut self, message: &Message) {
        if message.target == self.id {
            match message.data {
                MessageData::MenuButton(MenuButtonMessage::Hovered) => self.hovered = true,
                MessageData::MenuButton(MenuButtonMessage::Unhovered) => self.hovered = false,
                MessageData::MenuButton(MenuButtonMessage::Selected) => self.selected = true,
                MessageData::MenuButton(MenuButtonMessage::Unselected) => self.selected = false,
                _ => (),
            }
        }
    }

    fn set_bounds(&mut self, rect: Rect) {
        self.rect = rect;
        self.visible_rect = rect;
    }

    fn bounds(&self) -> Rect {
        self.rect
    }
}
