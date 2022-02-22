use std::sync::Arc;

use macroquad::prelude::*;

use crate::GameData;

use super::{
    menubutton::{MenuButton, MenuButtonMessage, Popout},
    Message, MessageData, UiElement,
};

pub enum MenuButtonListMessage {
    Click(usize),
    Selected(usize),
}

pub struct MenuButtonList {
    pub id: String,
    pub buttons: Vec<MenuButton>,
    pub selected: usize,
    tx: flume::Sender<Message>,
    rect: Rect,
}

impl MenuButtonList {
    pub fn new(
        id: String,
        popout: Popout,
        rect: Rect,
        titles: &[&str],
        tx: flume::Sender<Message>,
    ) -> Self {
        let mut list = MenuButtonList {
            id: id.clone(),
            buttons: titles
                .iter()
                .enumerate()
                .map(|(idx, &title)| {
                    MenuButton::new(
                        format!("{}-{}", &id, idx),
                        title.to_owned(),
                        popout,
                        Rect::default(),
                        tx.clone(),
                    )
                })
                .collect(),
            selected: 0,
            tx,
            rect: Rect::default(),
        };
        list.set_bounds(rect);
        list
    }
}

impl UiElement for MenuButtonList {
    fn draw(&self, data: Arc<GameData>) {
        for button in &self.buttons {
            button.draw(data.clone());
        }
    }

    fn update(&mut self, data: Arc<GameData>) {
        for button in &mut self.buttons {
            button.update(data.clone());
        }
    }

    fn handle_message(&mut self, message: &Message) {
        if message.sender == self.id {
            if let MessageData::MenuButtonList(MenuButtonListMessage::Click(idx)) = message.data {
                self.tx
                    .send(Message {
                        sender: self.buttons[self.selected].id.clone(),
                        data: MessageData::MenuButton(MenuButtonMessage::Unselected),
                    })
                    .unwrap();
                self.tx
                    .send(Message {
                        sender: self.buttons[idx].id.clone(),
                        data: MessageData::MenuButton(MenuButtonMessage::Selected),
                    })
                    .unwrap();
            }
        }
        if message.sender.starts_with(&self.id) {
            if let MessageData::MenuButton(MenuButtonMessage::Selected) = message.data {
                for (idx, button) in self.buttons.iter().enumerate() {
                    if button.id == message.sender {
                        self.selected = idx;
                        self.tx
                            .send(Message {
                                sender: self.id.clone(),
                                data: MessageData::MenuButtonList(MenuButtonListMessage::Selected(
                                    idx,
                                )),
                            })
                            .unwrap();
                    } else {
                        button
                            .tx
                            .send(Message {
                                sender: button.id.clone(),
                                data: MessageData::MenuButton(MenuButtonMessage::Unselected),
                            })
                            .unwrap();
                    }
                }
            }
            for button in &mut self.buttons {
                button.handle_message(message);
            }
        }
    }

    fn set_bounds(&mut self, rect: Rect) {
        //let button_height = rect.h / self.buttons.len() as f32;
        for (idx, button) in self.buttons.iter_mut().enumerate() {
            button.set_bounds(Rect::new(
                rect.x,
                rect.y + (100. + 5.) * idx as f32,
                rect.w,
                100.,
            ));
        }
        self.rect = rect;
    }

    fn bounds(&self) -> Rect {
        self.rect
    }

    fn draw_bounds(&self) {
        let bounds = self.bounds();
        draw_rectangle(
            bounds.x,
            bounds.y,
            bounds.w,
            bounds.h,
            Color::new(0.0, 0.0, 0.5, 0.5),
        );
        for button in &self.buttons {
            button.draw_bounds();
        }
    }
}
