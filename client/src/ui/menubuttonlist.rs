use super::{
    menubutton::{MenuButton, MenuButtonMessage, Popout},
    Message, MessageData, UiElement,
};
use crate::screen::game::SharedGameData;
use macroquad::prelude::*;

pub enum MenuButtonListMessage {
    Click(usize),
    Selected(usize),
}

pub struct MenuButtonList {
    pub id: String,
    pub buttons: Vec<MenuButton>,
    pub selected: usize,
    pub sub_selected: usize,
    tx: flume::Sender<Message>,
    rect: Rect,
}

impl MenuButtonList {
    pub fn new(
        id: String,
        popout: Popout,
        rect: Rect,
        titles: Vec<Vec<String>>,
        tx: flume::Sender<Message>,
    ) -> Self {
        let mut list = MenuButtonList {
            id: id.clone(),
            buttons: titles
                .into_iter()
                .enumerate()
                .map(|(idx, title)| {
                    MenuButton::new(
                        format!("{}-{}", &id, idx),
                        title,
                        popout,
                        Rect::default(),
                        tx.clone(),
                        false,
                    )
                })
                .collect(),
            selected: 0,
            sub_selected: 0,
            tx,
            rect: Rect::default(),
        };
        list.set_bounds(rect);
        list
    }
}

impl UiElement for MenuButtonList {
    fn draw(&self, data: SharedGameData) {
        for button in &self.buttons {
            button.draw(data.clone());
        }
    }

    fn update(&mut self, data: SharedGameData) {
        for button in &mut self.buttons {
            button.update(data.clone());
        }
    }

    fn handle_message(&mut self, message: &Message) {
        if message.target == self.id {
            if let MessageData::MenuButtonList(MenuButtonListMessage::Click(idx)) = message.data {
                self.tx
                    .send(Message {
                        target: self.buttons[self.selected].id.clone(),
                        data: MessageData::MenuButton(MenuButtonMessage::Unselected),
                    })
                    .unwrap();
                self.tx
                    .send(Message {
                        target: self.buttons[idx].id.clone(),
                        data: MessageData::MenuButton(MenuButtonMessage::Selected),
                    })
                    .unwrap();
            }
        }
        if message.target.starts_with(&self.id) {
            if let MessageData::MenuButton(MenuButtonMessage::Selected) = message.data {
                let mut dirty = false;
                for (idx, button) in self.buttons.iter().enumerate() {
                    if message.target.starts_with(&button.id) {
                        if message.target == button.id {
                            self.tx
                                .send(Message {
                                    target: self.id.clone(),
                                    data: MessageData::MenuButtonList(
                                        MenuButtonListMessage::Selected(idx),
                                    ),
                                })
                                .unwrap();
                            self.selected = idx;
                            dirty = true;
                        }
                    } else {
                        button
                            .tx
                            .send(Message {
                                target: button.id.clone(),
                                data: MessageData::MenuButton(MenuButtonMessage::Unselected),
                            })
                            .unwrap();
                    }
                }
                if dirty {
                    self.refresh_bounds();
                }
            }
            for button in &mut self.buttons {
                button.handle_message(message);
            }
        }
    }

    fn set_bounds(&mut self, mut rect: Rect) {
        let mut y = 0.;
        for button in &mut self.buttons {
            button.set_bounds(Rect::new(rect.x, rect.y + y, rect.w, 100.));
            y += 100. + 5.;
        }
        rect.h = y;
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
    }
}
