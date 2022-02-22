use super::{gameplay::Gameplay, Screen};
use crate::{
    promise::Promise,
    ui::{
        menubutton::{MenuButton, MenuButtonMessage, Popout},
        menubuttonlist::{MenuButtonList, MenuButtonListMessage},
        Message, MessageData, UiElement,
    },
    GameData,
};
use async_trait::async_trait;
use kira::{
    instance::{InstanceSettings, StopInstanceSettings},
    sound::handle::SoundHandle,
};
use macroquad::prelude::*;
use std::sync::Arc;

struct MapListing {
    title: String,
    difficulties: Vec<String>,
}

pub struct SelectScreen {
    maps: Vec<MapListing>,
    prev_selected_map: usize,
    selected_map: usize,
    selected_difficulty: usize,

    rx: flume::Receiver<Message>,
    tx: flume::Sender<Message>,
    map_list: MenuButtonList,
    difficulty_list: Option<MenuButtonList>,

    start: MenuButton,
    loading_promise: Option<Promise<(SoundHandle, Texture2D)>>,
}

impl SelectScreen {
    pub fn new(_data: Arc<GameData>) -> Self {
        let (tx, rx) = flume::unbounded();
        let maps = vec![
            MapListing {
                title: "Kizuato".to_string(),
                difficulties: vec!["Platter".to_string(), "Ascendance's Rain".to_string()],
            },
            MapListing {
                title: "Padoru".to_string(),
                difficulties: vec!["Salad".to_string(), "Platter".to_string()],
            },
            MapListing {
                title: "Troublemaker".to_string(),
                difficulties: vec![
                    "Cup".to_string(),
                    "Equim's Rain".to_string(),
                    "Kagari's Himedose".to_string(),
                    "MBomb's Light Rain".to_string(),
                    "Platter".to_string(),
                    "tocean's Salad".to_string(),
                ],
            },
        ];
        let map_list = MenuButtonList::new(
            "button_list".to_string(),
            Popout::Right,
            Rect::new(-400. / 4., 0., 400., 400.),
            maps.iter()
                .map(|map| map.title.as_str())
                .collect::<Vec<_>>()
                .as_slice(),
            tx.clone(),
        );
        tx.send(Message {
            sender: map_list.id.clone(),
            data: MessageData::MenuButtonList(MenuButtonListMessage::Click(0)),
        })
        .unwrap();

        SelectScreen {
            prev_selected_map: usize::MAX,
            selected_map: usize::MAX,
            selected_difficulty: 0,

            maps,
            rx,
            tx: tx.clone(),
            map_list,
            difficulty_list: None,
            start: MenuButton::new(
                "start".to_string(),
                "Start".to_string(),
                Popout::None,
                Rect::new(
                    screen_width() / 2. - 400. / 2.,
                    screen_height() - 100.,
                    400.,
                    100.,
                ),
                tx,
            ),
            loading_promise: None,
        }
    }
}

#[async_trait(?Send)]
impl Screen for SelectScreen {
    async fn update(&mut self, data: Arc<GameData>) {
        if self.selected_map != self.prev_selected_map {
            let data_clone = data.clone();
            let map_title = self.maps[self.selected_map].title.clone();
            if let Some(loading_promise) = &self.loading_promise {
                data.exec.lock().cancel(loading_promise);
            }
            self.loading_promise = Some(data.exec.lock().spawn(move || async move {
                let sound = data_clone
                    .audio_cache
                    .get_sound(
                        &mut data_clone.audio.lock(),
                        &format!("resources/{}/audio.wav", map_title),
                    )
                    .await;
                let background = data_clone
                    .image_cache
                    .get_texture(&format!("resources/{}/bg.png", map_title))
                    .await;
                (sound, background)
            }));

            self.prev_selected_map = self.selected_map;
        }

        if let Some(loading_promise) = &self.loading_promise {
            if let Some((mut sound, background)) = data.exec.lock().try_get(loading_promise) {
                data.state
                    .lock()
                    .music
                    .stop(StopInstanceSettings::new())
                    .unwrap();
                data.state.lock().background = Some(background);
                data.state.lock().music =
                    sound.play(InstanceSettings::default().volume(0.5)).unwrap();

                let difficulty_list = MenuButtonList::new(
                    "difficulty_list".to_string(),
                    Popout::Left,
                    Rect::new(screen_width() - 400. + 400. / 4., 0., 400., 400.),
                    &self.maps[self.selected_map]
                        .difficulties
                        .iter()
                        .map(|s| s.as_str())
                        .collect::<Vec<_>>(),
                    self.tx.clone(),
                );
                self.tx
                    .send(Message {
                        sender: difficulty_list.id.clone(),
                        data: MessageData::MenuButtonList(MenuButtonListMessage::Click(0)),
                    })
                    .unwrap();
                self.difficulty_list = Some(difficulty_list);

                self.loading_promise = None;
            }
        }

        if let Some(difficulty_list) = &self.difficulty_list {
            if is_key_pressed(KeyCode::Down) {
                self.tx
                    .send(Message {
                        sender: difficulty_list.id.clone(),
                        data: MessageData::MenuButtonList(MenuButtonListMessage::Click(
                            (difficulty_list.selected + 1) % difficulty_list.buttons.len(),
                        )),
                    })
                    .unwrap();
            } else if is_key_pressed(KeyCode::Up) {
                self.tx
                    .send(Message {
                        sender: difficulty_list.id.clone(),
                        data: MessageData::MenuButtonList(MenuButtonListMessage::Click(
                            (difficulty_list.selected + difficulty_list.buttons.len() - 1)
                                % difficulty_list.buttons.len(),
                        )),
                    })
                    .unwrap();
            }
        }

        if is_key_pressed(KeyCode::Right) {
            self.tx
                .send(Message {
                    sender: self.map_list.id.clone(),
                    data: MessageData::MenuButtonList(MenuButtonListMessage::Click(
                        (self.map_list.selected + 1) % self.map_list.buttons.len(),
                    )),
                })
                .unwrap();
        } else if is_key_pressed(KeyCode::Left) {
            self.tx
                .send(Message {
                    sender: self.map_list.id.clone(),
                    data: MessageData::MenuButtonList(MenuButtonListMessage::Click(
                        (self.map_list.selected + self.map_list.buttons.len() - 1)
                            % self.map_list.buttons.len(),
                    )),
                })
                .unwrap();
        }

        if is_key_pressed(KeyCode::Enter) {
            self.tx
                .send(Message {
                    sender: self.start.id.clone(),
                    data: MessageData::MenuButton(MenuButtonMessage::Selected),
                })
                .unwrap();
        }

        for message in self.rx.drain() {
            self.map_list.handle_message(&message);
            if let Some(difficulty_list) = &mut self.difficulty_list {
                difficulty_list.handle_message(&message);
            }
            self.start.handle_message(&message);
            if message.sender == self.map_list.id {
                if let MessageData::MenuButtonList(MenuButtonListMessage::Selected(idx)) =
                    message.data
                {
                    self.selected_map = idx;
                }
            }
            if let Some(ref mut difficulty_list) = self.difficulty_list {
                if message.sender == difficulty_list.id {
                    if let MessageData::MenuButtonList(MenuButtonListMessage::Selected(idx)) =
                        message.data
                    {
                        self.selected_difficulty = idx;
                    }
                }
            }
            if message.sender == self.start.id {
                if let MessageData::MenuButton(MenuButtonMessage::Selected) = message.data {
                    let map = &self.maps[self.selected_map];
                    data.state.lock().queued_screen = Some(Box::new(
                        Gameplay::new(
                            data.clone(),
                            &map.title,
                            &map.difficulties[self.selected_difficulty],
                        )
                        .await,
                    ));
                }
            }
        }
        self.map_list.update(data.clone());
        if let Some(difficulty_list) = &mut self.difficulty_list {
            difficulty_list.update(data.clone());
        }
        self.start.update(data.clone());
    }

    fn draw(&self, data: Arc<GameData>) {
        if let Some(background) = data.state.lock().background {
            draw_texture_ex(
                background,
                0.,
                0.,
                Color::new(1., 1., 1., 0.6),
                DrawTextureParams {
                    dest_size: Some(vec2(screen_width(), screen_height())),
                    ..Default::default()
                },
            );
        }
        self.map_list.draw(data.clone());
        if let Some(ref list) = self.difficulty_list {
            list.draw(data.clone());
        }
        self.start.draw(data);

        if self.loading_promise.is_some() {
            let loading_dim = measure_text("Loading...", None, 36, 1.);
            draw_text(
                "Loading...",
                screen_width() / 2. - loading_dim.width / 2.,
                screen_height() / 2. - loading_dim.height / 2.,
                36.,
                WHITE,
            );
        }
    }
}