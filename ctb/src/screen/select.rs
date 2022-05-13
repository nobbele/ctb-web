use std::collections::BTreeMap;

use super::{
    game::{GameMessage, SharedGameData},
    gameplay::Gameplay,
    get_charts, ChartInfo, Screen,
};
use crate::{
    azusa::{ClientPacket, ServerPacket},
    convert::ConvertFrom,
    draw_text_centered,
    promise::Promise,
    score,
    ui::{
        menubutton::{MenuButton, MenuButtonMessage, Popout},
        menubuttonlist::{MenuButtonList, MenuButtonListMessage},
        Message, MessageData, UiElement,
    },
};
use async_trait::async_trait;
use kira::sound::static_sound::StaticSoundData;
use macroquad::prelude::*;
use noisy_float::prelude::R32;
use num_format::{Locale, ToFormattedString};

/// f takes progress [0-1] as input and result as output [0-1]
fn draw_visualization(x: f32, y: f32, width: f32, height: f32, f: impl Fn(f32) -> f32) {
    for i in 0..(height as u32) {
        let progress = i as f32 / height as f32;
        let value = f(progress);
        draw_rectangle(
            x,
            y + i as f32,
            width,
            1.,
            Color {
                r: value,
                g: value,
                b: value,
                a: 1.0,
            },
        )
    }
}

fn interpolate(max: f32, tree: &BTreeMap<R32, R32>) -> impl Fn(f32) -> f32 + '_ {
    move |progress| {
        let time = progress * max;
        let pre = if let Some(v) = tree.iter().take_while(|(t, _)| t.raw() < time).last() {
            (v.0.raw(), v.1.raw())
        } else {
            (0.0, 0.0)
        };
        let post = if let Some(v) = tree.iter().find(|(t, _)| t.raw() >= time) {
            (v.0.raw(), v.1.raw())
        } else {
            (0.0, 0.0)
        };
        crate::math::remap(pre.0, post.0, pre.1, post.1, time)
    }
}

pub struct ChartCalcData {
    density: BTreeMap<R32, R32>,
    angles: BTreeMap<R32, R32>,
    angle_changes: BTreeMap<R32, R32>,
}

pub struct SelectScreen {
    charts: Vec<ChartInfo>,
    prev_selected_chart: usize,
    selected_chart: usize,
    selected_difficulty: usize,

    chart_data: Option<ChartCalcData>,

    scroll_vel: f32,

    rx: flume::Receiver<Message>,
    tx: flume::Sender<Message>,
    chart_list: MenuButtonList,
    global_lb: Option<MenuButtonList>,
    local_lb: Option<MenuButtonList>,
    scroll_target: Option<f32>,

    start: MenuButton,
    loading_promise: Option<Promise<(StaticSoundData, Texture2D)>>,
}

impl SelectScreen {
    pub fn new(_data: SharedGameData) -> Self {
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
            chart_data: None,
        }
    }

    async fn start_map(&self, data: SharedGameData) {
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
    async fn update(&mut self, data: SharedGameData) {
        if self.selected_chart != self.prev_selected_chart {
            let data_clone = data.clone();
            if let Some(loading_promise) = &self.loading_promise {
                data.promises().cancel(loading_promise);
            }

            self.loading_promise = Some(data.promises().spawn(move || async move {
                let sound = data_clone
                    .audio_cache
                    .get_sound(
                        &format!("resources/{}/audio.wav", data_clone.state().chart.title),
                        data_clone.main_track.id(),
                    )
                    .await
                    .unwrap();
                let background = data_clone
                    .image_cache
                    .get_texture(&format!(
                        "resources/{}/bg.png",
                        data_clone.state().chart.title
                    ))
                    .await;
                (sound, background)
            }));

            self.prev_selected_chart = self.selected_chart;
        }

        if let Some(loading_promise) = &self.loading_promise {
            if let Some((sound, background)) = data.promises().try_get(loading_promise) {
                data.background.set(Some(background));
                data.broadcast(GameMessage::update_music_looped(sound));

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

        if data.is_key_pressed(KeyCode::Right) {
            self.tx
                .send(Message {
                    target: self.chart_list.id.clone(),
                    data: MessageData::MenuButtonList(MenuButtonListMessage::Click(
                        (self.chart_list.selected + 1) % self.chart_list.buttons.len(),
                    )),
                })
                .unwrap();
        } else if data.is_key_pressed(KeyCode::Left) {
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

        if data.is_key_pressed(KeyCode::Down) {
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
        if data.is_key_pressed(KeyCode::Up) {
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

        if data.is_key_pressed(KeyCode::Enter) {
            self.start_map(data.clone()).await;
        }

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
                    data.state.borrow_mut().chart = chart.clone();
                }
                if let MessageData::MenuButtonList(MenuButtonListMessage::SelectedSub(idx)) =
                    message.data
                {
                    self.selected_difficulty = idx;
                    data.state.borrow_mut().difficulty_idx = idx;
                    let diff_id = data.state().chart.difficulties[idx].id;

                    let entries = data.state_mut().leaderboard.query_local(diff_id).await;
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

                    let chart_info = &self.charts[self.selected_chart];
                    let beatmap_data = load_file(&format!(
                        "resources/{}/{}.osu",
                        chart_info.title, chart_info.difficulties[self.selected_difficulty].name
                    ))
                    .await
                    .unwrap();
                    let beatmap_content = std::str::from_utf8(&beatmap_data).unwrap();
                    let beatmap = osu_parser::load_content(
                        beatmap_content,
                        osu_parser::BeatmapParseOptions::default(),
                    )
                    .unwrap();
                    let chart = crate::chart::Chart::convert_from(&beatmap);

                    let mut chart_data = ChartCalcData {
                        density: BTreeMap::new(),
                        angles: BTreeMap::new(),
                        angle_changes: BTreeMap::new(),
                    };
                    for [a, b] in chart.fruits.array_windows::<2>() {
                        assert_ne!(a.time, b.time);
                        let time_to_hit = b.time - a.time;
                        // https://www.desmos.com/calculator/yt3tru6suf
                        // \frac{1}{1+e^{\frac{v}{100}\left(x-m\right)}}
                        fn density(diff_ms: f32) -> f32 {
                            const V: f32 = 3.0 / 100.;
                            const M: f32 = 166.0;
                            1. / (1. + (diff_ms * V - M * V).exp())
                        }

                        let angle = a.angle_to(b, chart.fall_time).to_degrees();

                        chart_data
                            .density
                            .insert(R32::new(b.time), R32::new(density(time_to_hit * 1000.)));
                        chart_data
                            .angles
                            .insert(R32::new(b.time), R32::new(angle.abs() / 90.));
                    }
                    for [a, b, c] in chart.fruits.array_windows::<3>() {
                        let angle_a = a.angle_to(b, chart.fall_time).to_degrees();
                        let angle_b = b.angle_to(c, chart.fall_time).to_degrees();

                        let angle_change = angle_a.max(angle_b) - angle_a.min(angle_b);

                        chart_data
                            .angle_changes
                            .insert(R32::new(b.time), R32::new(angle_change / 180.));
                    }
                    self.chart_data = Some(chart_data);
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

    fn draw(&self, data: SharedGameData) {
        draw_texture_ex(
            data.background(),
            0.,
            0.,
            Color::new(1., 1., 1., 0.6),
            DrawTextureParams {
                dest_size: Some(vec2(screen_width(), screen_height())),
                ..Default::default()
            },
        );
        self.chart_list.draw(data.clone());
        self.start.draw(data.clone());
        if let Some(local) = &self.local_lb {
            local.draw(data.clone());
        }
        if let Some(global) = &self.global_lb {
            global.draw(data);
        }

        if let Some(chart_data) = &self.chart_data {
            let max_density = chart_data.density.iter().last().unwrap();
            let max_angle = chart_data.angles.iter().last().unwrap();
            let max_angle_change = chart_data.angles.iter().last().unwrap();

            draw_text("Density", 0., screen_height() - 106., 16., WHITE);
            draw_visualization(
                0.,
                screen_height() - 100.,
                50.,
                100.,
                interpolate(max_density.0.raw(), &chart_data.density),
            );

            draw_text("Angles", 55., screen_height() - 106., 16., WHITE);
            draw_visualization(
                55.,
                screen_height() - 100.,
                50.,
                100.,
                interpolate(max_angle.0.raw(), &chart_data.angles),
            );

            draw_text("Angles Changes", 110., screen_height() - 106., 16., WHITE);
            draw_visualization(
                110.,
                screen_height() - 100.,
                50.,
                100.,
                interpolate(max_angle_change.0.raw(), &chart_data.angle_changes),
            );
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

    fn handle_packet(&mut self, data: SharedGameData, packet: &ServerPacket) {
        #[allow(clippy::single_match)]
        match packet {
            ServerPacket::Leaderboard { diff_id, scores } => {
                let current_diff_id = data.state().difficulty().id;
                if *diff_id == current_diff_id {
                    let button_title = scores
                        .iter()
                        .map(|score| {
                            (
                                vec![
                                    score.username.clone().unwrap(),
                                    format!(
                                        "{} ({:.2}%)",
                                        score.score.to_formatted_string(&Locale::en),
                                        score::accuracy(&score.judgements) * 100.
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
