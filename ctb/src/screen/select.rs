use super::{game::GameMessage, gameplay::Gameplay, get_charts, ChartInfo, GameData, Screen};
use crate::{
    azusa::{ClientPacket, ServerPacket},
    draw_text_centered,
    promise::Promise,
    ui::{
        menubutton::{MenuButton, MenuButtonMessage, Popout},
        menubuttonlist::{MenuButtonList, MenuButtonListMessage},
        Message, MessageData, UiElement,
    },
};
use async_trait::async_trait;
use kira::sound::handle::SoundHandle;
use macroquad::prelude::*;
use num_format::{Locale, ToFormattedString};
use std::sync::Arc;

pub struct SelectScreen {
    charts: Vec<ChartInfo>,
    prev_selected_chart: usize,
    selected_chart: usize,
    selected_difficulty: usize,

    scroll_vel: f32,

    rx: flume::Receiver<Message>,
    tx: flume::Sender<Message>,
    chart_list: MenuButtonList,
    global_lb: Option<MenuButtonList>,
    local_lb: Option<MenuButtonList>,
    scroll_target: Option<f32>,

    start: MenuButton,
    loading_promise: Option<Promise<(SoundHandle, Texture2D)>>,
}

impl SelectScreen {
    pub fn new(_data: Arc<GameData>) -> Self {
        let (tx, rx) = flume::unbounded();
        let charts = get_charts();
        let charts_raw = charts
            .iter()
            .map(|chart| {
                (
                    vec![chart.title.clone()],
                    Some(
                        chart
                            .difficulties
                            .iter()
                            .map(|diff| diff.name.clone())
                            .collect::<Vec<_>>(),
                    ),
                )
            })
            .collect::<Vec<_>>();
        let chart_list = MenuButtonList::new(
            "button_list".to_string(),
            Popout::Left,
            Rect::new(screen_width() - 400., 0., 400., 400.),
            charts_raw,
            tx.clone(),
        );
        tx.send(Message {
            target: chart_list.id.clone(),
            data: MessageData::MenuButtonList(MenuButtonListMessage::Click(0)),
        })
        .unwrap();

        SelectScreen {
            prev_selected_chart: usize::MAX,
            selected_chart: usize::MAX,
            selected_difficulty: 0,

            scroll_vel: 0.,

            charts,
            rx,
            tx: tx.clone(),
            chart_list,
            start: MenuButton::new(
                "start".to_string(),
                vec!["Start".to_string()],
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
            local_lb: None,
            global_lb: None,
            scroll_target: None,
        }
    }

    async fn start_map(&self, data: Arc<GameData>) {
        let chart = &self.charts[self.selected_chart];
        data.broadcast(GameMessage::change_screen(
            Gameplay::new(
                data.clone(),
                &chart.title,
                &chart.difficulties[self.selected_difficulty].name,
            )
            .await,
        ));
    }
}

#[async_trait(?Send)]
impl Screen for SelectScreen {
    async fn update(&mut self, data: Arc<GameData>) {
        if self.selected_chart != self.prev_selected_chart {
            let data_clone = data.clone();
            if let Some(loading_promise) = &self.loading_promise {
                data.exec.lock().cancel(loading_promise);
            }
            self.loading_promise = Some(data.exec.lock().spawn(move || async move {
                let sound = data_clone
                    .audio_cache
                    .get_sound(
                        &mut data_clone.audio.lock(),
                        &format!(
                            "resources/{}/audio.wav",
                            data_clone.state.lock().chart.title
                        ),
                    )
                    .await;
                let background = data_clone
                    .image_cache
                    .get_texture(&format!(
                        "resources/{}/bg.png",
                        data_clone.state.lock().chart.title
                    ))
                    .await;
                (sound, background)
            }));

            self.prev_selected_chart = self.selected_chart;
        }

        if let Some(loading_promise) = &self.loading_promise {
            if let Some((sound, background)) = data.exec.lock().try_get(loading_promise) {
                data.state.lock().background = Some(background);
                data.game_tx
                    .send(GameMessage::update_music_looped(sound))
                    .unwrap();

                self.loading_promise = None;
            }
        }

        let scroll_delta = mouse_wheel().1;
        if scroll_delta != 0. {
            self.scroll_vel += scroll_delta * 1.5;
        }
        if let Some(scroll_target) = self.scroll_target {
            let offset = screen_height() / 2. - (self.chart_list.bounds().y + scroll_target);
            // Check if target is within reasonable bounds.
            if offset.abs() < 10. {
                self.scroll_vel = 0.;
                self.scroll_target = None;
            } else {
                self.scroll_vel += offset / 400.;
            }
        }

        self.scroll_vel = self.scroll_vel.clamp(-18., 18.);
        if self.scroll_vel != 0. {
            let mut bounds = self.chart_list.bounds();
            bounds.y += self.scroll_vel;

            let pre_clamp = bounds.y;
            bounds.y = bounds
                .y
                .clamp(-(bounds.h - screen_height()).max(0.) - 100., 100.);
            if bounds.y != pre_clamp {
                // Check target is in the same direction as where it got clamped.
                // Meaning the target is in an unreachable spot such as the top or bottom of the screen.
                if let Some(scroll_target) = self.scroll_target {
                    if scroll_target.signum() != self.scroll_vel.signum() {
                        self.scroll_target = None;
                    }
                }
                self.scroll_vel = 0.;
            }

            self.chart_list.set_bounds(bounds);

            self.scroll_vel -= self.scroll_vel * get_frame_time() * 5.;
        }

        if is_key_pressed(KeyCode::Right) {
            self.tx
                .send(Message {
                    target: self.chart_list.id.clone(),
                    data: MessageData::MenuButtonList(MenuButtonListMessage::Click(
                        (self.chart_list.selected + 1) % self.chart_list.buttons.len(),
                    )),
                })
                .unwrap();
        } else if is_key_pressed(KeyCode::Left) {
            self.tx
                .send(Message {
                    target: self.chart_list.id.clone(),
                    data: MessageData::MenuButtonList(MenuButtonListMessage::Click(
                        (self.chart_list.selected + self.chart_list.buttons.len() - 1)
                            % self.chart_list.buttons.len(),
                    )),
                })
                .unwrap();
        }

        if is_key_pressed(KeyCode::Down) {
            self.tx
                .send(Message {
                    target: self.chart_list.id.clone(),
                    data: MessageData::MenuButtonList(MenuButtonListMessage::ClickSub(
                        (self.chart_list.sub_selected + 1)
                            % self.chart_list.buttons[self.chart_list.selected]
                                .1
                                .as_ref()
                                .unwrap()
                                .len(),
                    )),
                })
                .unwrap();
        }
        if is_key_pressed(KeyCode::Up) {
            let len = self.chart_list.buttons[self.chart_list.selected]
                .1
                .as_ref()
                .unwrap()
                .len();
            self.tx
                .send(Message {
                    target: self.chart_list.id.clone(),
                    data: MessageData::MenuButtonList(MenuButtonListMessage::ClickSub(
                        (self.chart_list.sub_selected + len - 1) % len,
                    )),
                })
                .unwrap();
        }

        //TODO Temporarily disabled since it messes with the chat overlay's send shortcut.
        /*if is_key_pressed(KeyCode::Enter) {
            self.start_map(data.clone()).await;
        }*/

        for message in self.rx.try_iter() {
            self.chart_list.handle_message(&message);
            self.start.handle_message(&message);
            if let Some(leaderboard) = &mut self.local_lb {
                leaderboard.handle_message(&message);
            }
            if let Some(leaderboard) = &mut self.global_lb {
                leaderboard.handle_message(&message);
            }
            if message.target == self.chart_list.id {
                if let MessageData::MenuButtonList(MenuButtonListMessage::Selected(idx)) =
                    message.data
                {
                    self.selected_chart = idx;
                    let chart = &self.charts[self.selected_chart];
                    data.state.lock().chart = chart.clone();
                }
                if let MessageData::MenuButtonList(MenuButtonListMessage::SelectedSub(idx)) =
                    message.data
                {
                    self.selected_difficulty = idx;
                    data.state.lock().difficulty_idx = idx;
                    let diff_id = data.state.lock().chart.difficulties[idx].id;
                    let entries = data.state.lock().leaderboard.get_local(diff_id).await;
                    let button_title = entries
                        .iter()
                        .map(|entry| {
                            (
                                vec![format!(
                                    "{} ({:.2}%)",
                                    entry.score.to_formatted_string(&Locale::en),
                                    entry.accuracy * 100.
                                )],
                                None,
                            )
                        })
                        .collect::<Vec<_>>();
                    self.local_lb = Some(MenuButtonList::new(
                        "leaderboard".to_owned(),
                        Popout::Towards,
                        Rect::new(5., 5., 400., 0.),
                        button_title,
                        self.tx.clone(),
                    ));

                    let sub_button = &self.chart_list.buttons[self.chart_list.selected]
                        .1
                        .as_ref()
                        .unwrap()[self.chart_list.sub_selected];
                    self.scroll_target = Some(
                        sub_button.bounds().y + sub_button.bounds().h / 2.
                            - self.chart_list.bounds().y,
                    );

                    self.global_lb = None;
                    data.send_server(ClientPacket::RequestLeaderboard(diff_id));
                }
            }
            if message.target == self.start.id {
                if let MessageData::MenuButton(MenuButtonMessage::Selected) = message.data {
                    self.start_map(data.clone()).await;
                }
            }
        }
        self.chart_list.update(data.clone());
        self.start.update(data.clone());
        if let Some(local) = &mut self.local_lb {
            local.update(data.clone());
        }
        if let Some(global) = &mut self.global_lb {
            global.update(data);
        }
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
        self.chart_list.draw(data.clone());
        self.start.draw(data.clone());
        if let Some(local) = &self.local_lb {
            local.draw(data.clone());
        }
        if let Some(global) = &self.global_lb {
            global.draw(data);
        }

        if self.loading_promise.is_some() {
            draw_text_centered(
                "Loading...",
                screen_width() / 2.,
                screen_height() / 2.,
                36,
                WHITE,
            );
        }
    }

    fn handle_packet(&mut self, data: Arc<GameData>, packet: &ServerPacket) {
        match packet {
            ServerPacket::Leaderboard { diff_id, scores } => {
                let current_diff_id = data.state.lock().difficulty().id;
                if *diff_id == current_diff_id {
                    let button_title = scores
                        .iter()
                        .map(|score| {
                            let accuracy = score.hit_count as f32
                                / (score.hit_count + score.miss_count) as f32;
                            (
                                vec![
                                    score.username.clone().unwrap(),
                                    format!(
                                        "{} ({:.2}%)",
                                        score.score.to_formatted_string(&Locale::en),
                                        accuracy * 100.
                                    ),
                                ],
                                None,
                            )
                        })
                        .collect::<Vec<_>>();
                    self.global_lb = Some(MenuButtonList::new(
                        "global_leaderboard".to_owned(),
                        Popout::Towards,
                        Rect::new(410., 5., 400., 0.),
                        button_title,
                        self.tx.clone(),
                    ));
                }
            }
            _ => {}
        }
    }
}
