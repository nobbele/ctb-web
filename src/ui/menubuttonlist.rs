use super::{
    menubutton::{MenuButton, MenuButtonMessage, Popout},
    Message, MessageData, UiElement,
};
use crate::GameData;
use macroquad::prelude::*;
use std::sync::Arc;

pub enum MenuButtonListMessage {
    Click(usize),
    ClickSub(usize),
    Selected(usize),
    SelectedSub(usize),
}

pub struct MenuButtonList {
    pub id: String,
    pub buttons: Vec<(MenuButton, Option<Vec<MenuButton>>)>,
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
        titles: &[(&str, Option<&[&str]>)],
        tx: flume::Sender<Message>,
    ) -> Self {
        let mut list = MenuButtonList {
            id: id.clone(),
            buttons: titles
                .iter()
                .enumerate()
                .map(|(idx, &(title, diffs))| {
                    let id = format!("{}-{}", &id, idx);
                    (
                        MenuButton::new(
                            id.clone(),
                            title.to_owned(),
                            popout,
                            Rect::default(),
                            tx.clone(),
                        ),
                        diffs.map(|diffs| {
                            diffs
                                .iter()
                                .enumerate()
                                .map(|(idx, &diff)| {
                                    MenuButton::new(
                                        format!("{}-{}", id, idx),
                                        diff.to_owned(),
                                        popout,
                                        Rect::default(),
                                        tx.clone(),
                                    )
                                })
                                .collect()
                        }),
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
    fn draw(&self, data: Arc<GameData>) {
        for (idx, button) in self.buttons.iter().enumerate() {
            button.0.draw(data.clone());
            if self.selected == idx {
                if let Some(sub) = &button.1 {
                    for sub_button in sub {
                        sub_button.draw(data.clone());
                    }
                }
            }
        }
    }

    fn update(&mut self, data: Arc<GameData>) {
        for (idx, button) in self.buttons.iter_mut().enumerate() {
            button.0.update(data.clone());
            if self.selected == idx {
                if let Some(sub) = &mut button.1 {
                    for sub_button in sub {
                        sub_button.update(data.clone());
                    }
                }
            }
        }
    }

    fn handle_message(&mut self, message: &Message) {
        if message.target == self.id {
            if let MessageData::MenuButtonList(MenuButtonListMessage::Click(idx)) = message.data {
                self.tx
                    .send(Message {
                        target: self.buttons[self.selected].0.id.clone(),
                        data: MessageData::MenuButton(MenuButtonMessage::Unselected),
                    })
                    .unwrap();
                self.tx
                    .send(Message {
                        target: self.buttons[idx].0.id.clone(),
                        data: MessageData::MenuButton(MenuButtonMessage::Selected),
                    })
                    .unwrap();
            }
        }
        if message.target == self.id {
            if let MessageData::MenuButtonList(MenuButtonListMessage::ClickSub(idx)) = message.data
            {
                self.tx
                    .send(Message {
                        target: self.buttons[self.selected].1.as_ref().unwrap()[idx]
                            .id
                            .clone(),
                        data: MessageData::MenuButton(MenuButtonMessage::Unselected),
                    })
                    .unwrap();
                self.tx
                    .send(Message {
                        target: self.buttons[self.selected].1.as_ref().unwrap()[idx]
                            .id
                            .clone(),
                        data: MessageData::MenuButton(MenuButtonMessage::Selected),
                    })
                    .unwrap();
            }
        }
        if message.target.starts_with(&self.id) {
            if let MessageData::MenuButton(MenuButtonMessage::Selected) = message.data {
                let mut dirty = false;
                for (idx, button) in self.buttons.iter().enumerate() {
                    if message.target.starts_with(&button.0.id) {
                        if message.target == button.0.id {
                            self.tx
                                .send(Message {
                                    target: self.id.clone(),
                                    data: MessageData::MenuButtonList(
                                        MenuButtonListMessage::Selected(idx),
                                    ),
                                })
                                .unwrap();
                            if let Some(sub) = &button.1 {
                                self.tx
                                    .send(Message {
                                        target: sub.first().unwrap().id.clone(),
                                        data: MessageData::MenuButton(MenuButtonMessage::Selected),
                                    })
                                    .unwrap();
                            }
                            self.selected = idx;
                            dirty = true;
                        } else if let Some(sub) = &button.1 {
                            for (idx, sub_button) in sub.iter().enumerate() {
                                if message.target != sub_button.id {
                                    sub_button
                                        .tx
                                        .send(Message {
                                            target: sub_button.id.clone(),
                                            data: MessageData::MenuButton(
                                                MenuButtonMessage::Unselected,
                                            ),
                                        })
                                        .unwrap();
                                } else {
                                    self.tx
                                        .send(Message {
                                            target: self.id.clone(),
                                            data: MessageData::MenuButtonList(
                                                MenuButtonListMessage::SelectedSub(idx),
                                            ),
                                        })
                                        .unwrap();
                                }
                            }
                        }
                    } else {
                        button
                            .0
                            .tx
                            .send(Message {
                                target: button.0.id.clone(),
                                data: MessageData::MenuButton(MenuButtonMessage::Unselected),
                            })
                            .unwrap();
                    }
                }
                if dirty {
                    self.refresh_bounds();
                }
            }
            for (idx, button) in self.buttons.iter_mut().enumerate() {
                button.0.handle_message(message);
                if idx == self.selected {
                    if let Some(sub) = &mut button.1 {
                        for (sub_idx, sub) in sub.iter_mut().enumerate() {
                            if message.target == sub.id {
                                sub.handle_message(message);
                                if let MessageData::MenuButton(MenuButtonMessage::Selected) =
                                    message.data
                                {
                                    self.sub_selected = sub_idx;
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    fn set_bounds(&mut self, mut rect: Rect) {
        let mut y = 0.;
        for (idx, button) in self.buttons.iter_mut().enumerate() {
            button
                .0
                .set_bounds(Rect::new(rect.x, rect.y + y, rect.w, 100.));
            y += 100. + 5.;
            if idx == self.selected {
                if let Some(sub) = &mut button.1 {
                    for sub_button in sub {
                        sub_button.set_bounds(Rect::new(
                            rect.x + rect.w / 4.,
                            rect.y + y,
                            rect.w / 1.5,
                            120.,
                        ));
                        y += 120. + 5.;
                    }
                }
            }
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
        // TODO fix.
        for button in &self.buttons {
            button.0.draw_bounds();
        }
    }
}
