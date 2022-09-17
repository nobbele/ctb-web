use super::{
    menubutton::{MenuButton, MenuButtonMessage, Popout},
    Message, MessageData, UiElement,
};
use crate::screen::game::SharedGameData;
use macroquad::prelude::*;

pub enum ExpandableListMessage {
    Click(usize),
    ClickSub(usize),
    Selected(usize),
    SelectedSub(usize),
}

pub struct ExpandableList {
    pub id: String,
    pub buttons: Vec<(MenuButton, Vec<MenuButton>)>,
    pub selected: usize,
    pub sub_selected: usize,
    tx: flume::Sender<Message>,
    rect: Rect,
}

impl ExpandableList {
    pub fn new(
        id: String,
        popout: Popout,
        rect: Rect,
        titles: Vec<(Vec<String>, Vec<String>)>,
        tx: flume::Sender<Message>,
    ) -> Self {
        let mut list = ExpandableList {
            id: id.clone(),
            buttons: titles
                .into_iter()
                .enumerate()
                .map(|(idx, (title, diffs))| {
                    let id = format!("{}-{}", &id, idx);
                    (
                        MenuButton::new(
                            id.clone(),
                            title,
                            popout,
                            Rect::default(),
                            tx.clone(),
                            true,
                        ),
                        diffs
                            .into_iter()
                            .enumerate()
                            .map(|(idx, diff)| {
                                MenuButton::new(
                                    format!("{}-{}", id, idx),
                                    vec![diff],
                                    popout,
                                    Rect::default(),
                                    tx.clone(),
                                    true,
                                )
                            })
                            .collect(),
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

impl UiElement for ExpandableList {
    fn draw(&self, data: SharedGameData) {
        for (idx, button) in self.buttons.iter().enumerate() {
            button.0.draw(data.clone());
            if self.selected == idx {
                for sub_button in &button.1 {
                    sub_button.draw(data.clone());
                }
            }
        }
    }

    fn update(&mut self, data: SharedGameData) {
        for (idx, button) in self.buttons.iter_mut().enumerate() {
            button.0.update(data.clone());
            if self.selected == idx {
                for sub_button in &mut button.1 {
                    sub_button.update(data.clone());
                }
            }
        }
    }

    fn handle_message(&mut self, message: &Message) {
        if message.target == self.id {
            if let MessageData::ExpandableList(ExpandableListMessage::Click(idx)) = message.data {
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
            if let MessageData::ExpandableList(ExpandableListMessage::ClickSub(idx)) = message.data
            {
                self.tx
                    .send(Message {
                        target: self.buttons[self.selected].1[idx].id.clone(),
                        data: MessageData::MenuButton(MenuButtonMessage::Unselected),
                    })
                    .unwrap();
                self.tx
                    .send(Message {
                        target: self.buttons[self.selected].1[idx].id.clone(),
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
                                    data: MessageData::ExpandableList(
                                        ExpandableListMessage::Selected(idx),
                                    ),
                                })
                                .unwrap();
                            self.tx
                                .send(Message {
                                    target: button.1.first().unwrap().id.clone(),
                                    data: MessageData::MenuButton(MenuButtonMessage::Selected),
                                })
                                .unwrap();
                            self.selected = idx;
                            dirty = true;
                        } else {
                            for (idx, sub_button) in button.1.iter().enumerate() {
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
                                            data: MessageData::ExpandableList(
                                                ExpandableListMessage::SelectedSub(idx),
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
                    for (sub_idx, sub) in button.1.iter_mut().enumerate() {
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

    fn set_bounds(&mut self, mut rect: Rect) {
        let mut y = 0.;
        for (idx, button) in self.buttons.iter_mut().enumerate() {
            button
                .0
                .set_bounds(Rect::new(rect.x, rect.y + y, rect.w, 100.));
            y += 100. + 5.;
            if idx == self.selected {
                for sub_button in &mut button.1 {
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
