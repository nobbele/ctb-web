use crate::{
    config::{set_value, KeyBinds},
    ui::{
        menubutton::Popout,
        menubuttonlist::{MenuButtonList, MenuButtonListMessage},
        Message, MessageData, UiElement,
    },
};
use async_trait::async_trait;
use macroquad::prelude::*;
use std::sync::Arc;

use super::{select::SelectScreen, GameData, Screen};

pub struct SetupScreen {
    binding_types: MenuButtonList,
    rx: flume::Receiver<Message>,
}

impl SetupScreen {
    pub fn new() -> Self {
        let (tx, rx) = flume::unbounded();
        SetupScreen {
            binding_types: MenuButtonList::new(
                "binding_types".to_string(),
                Popout::Towards,
                Rect::new(
                    screen_width() / 2. - 400. / 2.,
                    screen_height() / 2. - 105. * 3. / 2.,
                    400.,
                    400.,
                ),
                &[
                    "Left-handed (A D RShift)",
                    "Right-handed (Left Right LShift)",
                    "Custom (TODO)",
                ],
                tx,
            ),
            rx,
        }
    }
}

#[async_trait(?Send)]
impl Screen for SetupScreen {
    fn draw(&self, data: Arc<GameData>) {
        self.binding_types.draw(data);
    }

    async fn update(&mut self, data: Arc<GameData>) {
        self.binding_types.update(data.clone());
        for message in self.rx.drain() {
            self.binding_types.handle_message(&message);
            if message.sender == self.binding_types.id {
                if let MessageData::MenuButtonList(MenuButtonListMessage::Selected(idx)) =
                    message.data
                {
                    let key_binds = match idx {
                        0 => KeyBinds {
                            right: KeyCode::D,
                            left: KeyCode::A,
                            dash: KeyCode::RightShift,
                        },
                        1 => KeyBinds {
                            right: KeyCode::Right,
                            left: KeyCode::Left,
                            dash: KeyCode::LeftShift,
                        },
                        _ => todo!(),
                    };
                    set_value("first_time", false);
                    set_value("binds", key_binds);
                    data.state.lock().binds = key_binds;
                    data.state
                        .lock()
                        .queued_screen
                        .replace(Box::new(SelectScreen::new(data.clone())));
                }
            }
        }
    }
}

impl Default for SetupScreen {
    fn default() -> Self {
        SetupScreen::new()
    }
}
