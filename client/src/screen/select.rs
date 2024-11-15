use std::{
    cell::Cell,
    collections::{BTreeMap, HashMap},
};

use super::{
    game::{GameMessage, SharedGameData},
    gameplay::Gameplay,
    get_charts, ChartInfo, Screen,
};
use crate::{
    azusa::{ClientPacket, ServerPacket},
    chart::Chart,
    convert::ConvertFrom,
    draw_circle_range, draw_text_centered,
    promise::Promise,
    score,
    ui::{
        expandablelist::{ExpandableList, ExpandableListMessage},
        menubutton::{MenuButton, MenuButtonMessage, Popout},
        menubuttonlist::MenuButtonList,
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

fn mean(data: &[f32]) -> f32 {
    let sum = data.iter().sum::<f32>();
    let count = data.len();

    match count {
        positive if positive > 0 => sum / count as f32,
        _ => 0.,
    }
}

fn std_deviation(data: &[f32]) -> f32 {
    let data_mean = mean(data);
    let count = data.len();
    if count > 0 {
        let variance = data
            .iter()
            .map(|value| {
                let diff = data_mean - (*value as f32);

                diff * diff
            })
            .sum::<f32>()
            / count as f32;

        variance.sqrt()
    } else {
        0.
    }
}

pub struct ChartCalcData {
    density: BTreeMap<R32, R32>,
    angles: BTreeMap<R32, R32>,
    angle_changes: BTreeMap<R32, R32>,
}

impl ChartCalcData {
    pub fn new(chart: &Chart) -> Self {
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
        chart_data
    }

    pub fn avg_density(&self) -> f32 {
        self.density.values().sum::<R32>().raw() / self.density.len() as f32
    }

    pub fn avg_angles(&self) -> f32 {
        self.angles.values().sum::<R32>().raw() / self.angles.len() as f32
    }

    pub fn avg_angle_changes(&self) -> f32 {
        self.angle_changes.values().sum::<R32>().raw() / self.angle_changes.len() as f32
    }

    pub fn star_rating(&self) -> f32 {
        let density_difficulty = self.avg_density() * 10.;
        let angle_difficulty = 1. - self.avg_angles();
        let angle_changes_difficulty = self.avg_angle_changes();

        let density_inconsistency =
            std_deviation(&self.density.values().map(|f| f.raw()).collect::<Vec<_>>());

        let mut anomalies = self
            .density
            .values()
            .map(|v| v.raw() / self.avg_density())
            .map(|v| v.max(1.))
            .collect::<Vec<_>>();
        anomalies.dedup();
        let spikiness = anomalies.iter().sum::<f32>() / anomalies.len() as f32;

        (1. + density_difficulty
            * angle_difficulty
            * angle_changes_difficulty.powi(2)
            * density_inconsistency.sqrt()
            * spikiness.sqrt()
            * 100.)
            .powf(1.5)
            - 1.
    }
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
    chart_list: ExpandableList,
    global_lb: Option<MenuButtonList>,
    local_lb: Option<MenuButtonList>,
    scroll_target: Option<f32>,

    start: MenuButton,
    pause: MenuButton,
    loading_promise: Option<Promise<(StaticSoundData, Texture2D)>>,
    started_map: Cell<bool>,
}

impl SelectScreen {
    pub async fn new(data: SharedGameData) -> Self {
        let (tx, rx) = flume::unbounded();
        let charts = get_charts(data.clone());

        let diff_futures = charts.iter().flat_map(|chart| {
            chart.difficulties.iter().map(|diff| async {
                let beatmap_data =
                    load_file(&format!("resources/{}/{}.osu", chart.title, diff.name))
                        .await
                        .unwrap();
                let beatmap_content = std::str::from_utf8(&beatmap_data).unwrap();
                let beatmap = osu_parser::load_content(
                    beatmap_content,
                    osu_parser::BeatmapParseOptions::default(),
                )
                .unwrap();
                let chart_impl = crate::chart::Chart::convert_from(&beatmap);
                (
                    format!("{}-{}", chart.title, diff.name),
                    ChartCalcData::new(&chart_impl),
                )
            })
        });
        let mut diffs = HashMap::new();
        for diff_future in diff_futures {
            let (key, value) = diff_future.await;
            diffs.insert(key, value);
        }

        let chart_list = ExpandableList::new(
            data,
            "button_list".to_string(),
            Popout::Left,
            Rect::new(screen_width() - 400., 0., 400., 400.),
            tx.clone(),
        );
        tx.send(Message {
            target: chart_list.id.clone(),
            data: MessageData::ExpandableList(ExpandableListMessage::Click(0)),
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
                tx.clone(),
                false,
            ),
            loading_promise: None,
            local_lb: None,
            global_lb: None,
            scroll_target: None,
            chart_data: None,
            started_map: Cell::new(false),
            pause: MenuButton::new(
                "pause".to_string(),
                vec!["Pause".to_string()],
                Popout::None,
                Rect::new(
                    screen_width() / 2. - 400. / 2.,
                    screen_height() / 2. - 100. / 2.,
                    400.,
                    100.,
                ),
                tx.clone(),
                false,
            ),
        }
    }

    fn start_map(&self, data: SharedGameData) {
        self.started_map.set(true);
        let chart = &self.charts[self.selected_chart];
        data.broadcast(GameMessage::load_screen({
            let data = data.clone();
            let chart_title = chart.title.clone();
            let diff_name = chart.difficulties[self.selected_difficulty].name.clone();
            async move { Gameplay::new(data, &chart_title, &diff_name).await }
        }));
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

            self.loading_promise = Some(data.promises().spawn(async move {
                let title = data_clone.state().chart.title.clone();

                let files = load_file(&format!("resources/{}/files.json", title))
                    .await
                    .unwrap();
                let files: Vec<String> = serde_json::from_slice(&files).unwrap();
                files.into_iter().for_each(|path| {
                    data_clone
                        .audio_cache
                        .whitelist(format!("resources/{}/{}", title, path))
                });

                let sound = data_clone
                    .audio_cache
                    .get_sound(
                        &format!("resources/{}/audio.wav", title),
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
            self.scroll_target = None;
            self.scroll_vel += scroll_delta * 1.5;
        }
        if let Some(scroll_target) = self.scroll_target {
            let offset = screen_height() / 2. - (self.chart_list.bounds().y + scroll_target);
            // Check if target is within reasonable bounds.
            if offset.abs() < 10. {
                self.scroll_target = None;
                self.scroll_vel = 0.;
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
                self.scroll_target = None;
                self.scroll_vel = 0.;
            }

            self.chart_list.set_bounds(bounds);

            self.scroll_vel -= self.scroll_vel * get_frame_time() * 5.;
        }

        if data.is_key_pressed(KeyCode::Right) {
            self.tx
                .send(Message {
                    target: self.chart_list.id.clone(),
                    data: MessageData::ExpandableList(ExpandableListMessage::Click(
                        (self.chart_list.selected + 1) % self.chart_list.buttons.len(),
                    )),
                })
                .unwrap();
        } else if data.is_key_pressed(KeyCode::Left) {
            self.tx
                .send(Message {
                    target: self.chart_list.id.clone(),
                    data: MessageData::ExpandableList(ExpandableListMessage::Click(
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
                    data: MessageData::ExpandableList(ExpandableListMessage::ClickSub(
                        (self.chart_list.sub_selected + 1)
                            % self.chart_list.buttons[self.chart_list.selected].1.len(),
                    )),
                })
                .unwrap();
        }
        if data.is_key_pressed(KeyCode::Up) {
            let len = self.chart_list.buttons[self.chart_list.selected].1.len();
            self.tx
                .send(Message {
                    target: self.chart_list.id.clone(),
                    data: MessageData::ExpandableList(ExpandableListMessage::ClickSub(
                        (self.chart_list.sub_selected + len - 1) % len,
                    )),
                })
                .unwrap();
        }

        if data.is_key_pressed(KeyCode::Enter) {
            self.start_map(data.clone());
        }

        for message in self.rx.try_iter() {
            if !self.started_map.get() {
                self.chart_list.handle_message(&message);
                self.start.handle_message(&message);
                self.pause.handle_message(&message);
                if let Some(leaderboard) = &mut self.local_lb {
                    leaderboard.handle_message(&message);
                }
                if let Some(leaderboard) = &mut self.global_lb {
                    leaderboard.handle_message(&message);
                }

                if message.target == self.chart_list.id {
                    if let MessageData::ExpandableList(ExpandableListMessage::Selected(idx)) =
                        message.data
                    {
                        self.selected_chart = idx;
                        let chart = &self.charts[self.selected_chart];
                        data.state.borrow_mut().chart = chart.clone();
                    }
                    if let MessageData::ExpandableList(ExpandableListMessage::SelectedSub(idx)) =
                        message.data
                    {
                        self.selected_difficulty = idx;
                        data.state.borrow_mut().difficulty_idx = idx;
                        let diff_id = data.state().chart.difficulties[idx].id;

                        let entries = data.state_mut().leaderboard.query_local(diff_id).await;
                        let button_title = entries
                            .iter()
                            .map(|entry| {
                                vec![format!(
                                    "{} ({:.2}%)",
                                    entry.score.to_formatted_string(&Locale::en),
                                    entry.accuracy * 100.
                                )]
                            })
                            .collect::<Vec<_>>();
                        self.local_lb = Some(MenuButtonList::new(
                            "leaderboard".to_owned(),
                            Popout::Towards,
                            Rect::new(5., 5., 400., 0.),
                            button_title,
                            self.tx.clone(),
                        ));

                        let sub_button = &self.chart_list.buttons[self.chart_list.selected].1
                            [self.chart_list.sub_selected];
                        self.scroll_target = Some(
                            sub_button.bounds().y + sub_button.bounds().h / 2.
                                - self.chart_list.bounds().y,
                        );

                        self.global_lb = None;
                        data.send_server(ClientPacket::RequestLeaderboard(diff_id));

                        let chart_info = &self.charts[self.selected_chart];
                        let beatmap_data = load_file(&format!(
                            "resources/{}/{}.osu",
                            chart_info.title,
                            chart_info.difficulties[self.selected_difficulty].name
                        ))
                        .await
                        .unwrap();
                        let beatmap_content = std::str::from_utf8(&beatmap_data).unwrap();
                        let beatmap = osu_parser::load_content(
                            beatmap_content,
                            osu_parser::BeatmapParseOptions::default(),
                        )
                        .unwrap();
                        self.chart_data = Some(ChartCalcData::new(
                            &crate::chart::Chart::convert_from(&beatmap),
                        ));
                    }
                }
                if message.target == self.start.id {
                    if let MessageData::MenuButton(MenuButtonMessage::Selected) = message.data {
                        self.start_map(data.clone());
                    }
                }

                if message.target == self.pause.id {
                    if let MessageData::MenuButton(MenuButtonMessage::Selected) = message.data {
                        if data.state_mut().music.state()
                            == kira::sound::static_sound::PlaybackState::Playing
                        {
                            data.broadcast(GameMessage::PauseMusic);
                        } else {
                            data.broadcast(GameMessage::ResumeMusic);
                        }
                    }
                }
            }
        }
        self.chart_list.update(data.clone());
        self.start.update(data.clone());
        self.pause.update(data.clone());
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
        self.pause.draw(data.clone());
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

            let spacing = 100.;
            draw_text(
                &format!("Density ({:.2})", chart_data.avg_density()),
                0.,
                screen_height() - 106.,
                16.,
                WHITE,
            );
            draw_visualization(
                0.,
                screen_height() - 100.,
                50.,
                100.,
                interpolate(max_density.0.raw(), &chart_data.density),
            );

            draw_text(
                &format!("Angles ({:.2})", chart_data.avg_angles()),
                spacing + 5.,
                screen_height() - 106.,
                16.,
                WHITE,
            );
            draw_visualization(
                spacing + 5.,
                screen_height() - 100.,
                50.,
                100.,
                interpolate(max_angle.0.raw(), &chart_data.angles),
            );

            draw_text(
                &format!("Angle Changes ({:.2})", chart_data.avg_angle_changes()),
                spacing * 2. + 10.,
                screen_height() - 106.,
                16.,
                WHITE,
            );
            draw_visualization(
                spacing * 2. + 10.,
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

        if self.started_map.get() {
            draw_rectangle(
                0.,
                0.,
                screen_width(),
                screen_height(),
                Color { a: 0.25, ..BLACK },
            );

            let value = 2. * get_time() as f32;
            let range = 0.75;
            let value_norm = (range * value.sin() + 1.) / 2.0;
            let angle = (value_norm * std::f32::consts::TAU + 3. * value) % std::f32::consts::TAU;
            draw_circle_range(
                screen_width() / 2.,
                screen_height() / 2.,
                8.,
                50.,
                value_norm,
                angle,
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
                            vec![
                                score.username.clone().unwrap(),
                                format!(
                                    "{} ({:.2}%)",
                                    score.score.to_formatted_string(&Locale::en),
                                    score::accuracy(&score.judgements) * 100.
                                ),
                            ]
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
