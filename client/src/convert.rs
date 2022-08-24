use crate::{
    chart::{Additions, Chart, Event, EventData, Fruit, HitSoundKind},
    rulesets::catch::catcher_speed,
};
use macroquad::prelude::Color;
use osu_types::SpecificHitObject;

/// Converts from bits used in osu to an `Additions´ struct.
///
/// 0000\[clap]\[finish]\[whistle]0
pub fn from_hit_sound_bits(bits: u8) -> Additions {
    Additions {
        whistle: bits & (1 << 1) > 0,
        finish: bits & (1 << 2) > 0,
        clap: bits & (1 << 3) > 0,
    }
}

/// Converts an osu [`osu_types::HitObject`] into a catch [`Fruit`]
pub fn from_hitobject(
    hitobject: &osu_types::HitObject,
    small: bool,
    color: Color,
    plate_reset: bool,
    fall_multiplier: f32,
) -> Fruit {
    Fruit {
        position: hitobject.position.0 as f32,
        time: hitobject.time as f32 / 1000.,
        hyper: None,
        small,
        additions: from_hit_sound_bits(match &hitobject.specific {
            SpecificHitObject::Slider { edge_sounds, .. } => *edge_sounds.first().unwrap(),
            _ => hitobject.hit_sound,
        }),
        color,
        plate_reset,
        fall_multiplier,
    }
}

/// Trait that defines types [`Chart`] can be converted from. This is very similar to the [`From`] trait.
pub trait ConvertFrom<T> {
    fn convert_from(foreign: &T) -> Self;
}

impl ConvertFrom<osu_parser::Beatmap> for Chart {
    fn convert_from(beatmap: &osu_parser::Beatmap) -> Self {
        // List of "osu!pixels per seconds" at timing points.
        let opx_per_secs = beatmap
            .timing_points
            .iter()
            .scan(0.0, |bps, tp| {
                Some((
                    tp.time,
                    if tp.uninherited {
                        *bps = 1000.0 / tp.beat_length;
                        let px_per_beat = beatmap.info.difficulty.slider_multiplier * 100.0;
                        px_per_beat * *bps
                    } else {
                        let velocity = -100.0 / (tp.beat_length);
                        let px_per_beat =
                            beatmap.info.difficulty.slider_multiplier * velocity * 100.0;
                        px_per_beat * *bps
                    },
                ))
            })
            .collect::<Vec<_>>();
        // List of "beats per second" at timing points.
        let bps = beatmap
            .timing_points
            .iter()
            .filter_map(|tp| {
                if tp.uninherited {
                    Some((tp.time, 1000.0 / tp.beat_length))
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();

        // Index into the beatmap color table. Incremented once per new combo.
        let mut color_idx = 0;

        // Checks whether the current hitobject we are iterating over is the first one. Used to prevent incrementing color index at the start.
        let mut is_first = true;

        let mut fruits = Vec::with_capacity(beatmap.hit_objects.len());
        for hitobject in &beatmap.hit_objects {
            if !is_first && hitobject.new_combo {
                color_idx += 1;
                color_idx %= beatmap.colors.len();
            }
            let color = beatmap.colors[color_idx];

            // Get the "osu!pixels per second" from the last timing point that is before the current hitobject, aka the current timing point.
            let opx_per_sec = opx_per_secs
                .iter()
                .take_while(|&p| p.0 <= hitobject.time as i32)
                .last()
                .unwrap()
                .1;

            let fruit = from_hitobject(
                hitobject,
                false,
                Color {
                    r: color.r as f32 / u8::MAX as f32,
                    g: color.g as f32 / u8::MAX as f32,
                    b: color.b as f32 / u8::MAX as f32,
                    a: 1.0,
                },
                !is_first && hitobject.new_combo,
                opx_per_sec / 432.5,
            );
            fruits.push(fruit);

            // If it's a slider we need to create the drop and the slider tail fruit.
            if let osu_types::SpecificHitObject::Slider {
                curve_type,
                curve_points,
                length,
                ..
            } = &hitobject.specific
            {
                // Get the "beats per second" from the last timing point that is before the current hitobject, aka the current timing point.
                let bps = bps
                    .iter()
                    .take_while(|&p| p.0 <= hitobject.time as i32)
                    .last()
                    .unwrap()
                    .1;

                // Beatmap file lacks the initial curve point so we need to add that manually.
                let mut curve_points = curve_points.clone();
                curve_points.insert(
                    0,
                    mint::Point2 {
                        x: hitobject.position.0 as i16,
                        y: hitobject.position.1 as i16,
                    },
                );

                let spline =
                    osu_utils::Spline::from_control(*curve_type, &curve_points, Some(*length));

                // Length of the current slider in seconds.
                let slide_length_secs = length / opx_per_sec;

                let secs_per_beat = 1.0 / bps;
                let secs_per_drop = secs_per_beat / beatmap.info.difficulty.slider_tick_rate;

                // Place a drop every `secs_per_drop` seconds.
                let drops = (slide_length_secs / secs_per_drop).floor() as u32;
                for i in 1..drops {
                    let sec = secs_per_drop * i as f32;
                    let opx = opx_per_sec * sec;
                    let position = spline.point_at_length(opx).x;
                    fruits.push(Fruit {
                        position,
                        time: fruit.time + sec,
                        hyper: None,
                        small: true,
                        plate_reset: false,
                        ..fruit
                    })
                }

                // Create slider tail fruit.
                fruits.push(Fruit {
                    position: spline.point_at_length(*length).x,
                    time: fruit.time + slide_length_secs,
                    hyper: None,
                    small: false,
                    plate_reset: false,
                    ..fruit
                })
            }

            is_first = false;
        }

        // Calculate hyper speeds for each fruit.
        for idx in 0..fruits.len().saturating_sub(1) {
            let [fruit, next_fruit]: &mut [Fruit; 2] =
                (&mut fruits[idx..idx + 2]).try_into().unwrap();
            // If you can't get to the center of the next fruit in time, we need to give the player some extra speed.
            // TODO use same implementation as osu!catch.
            let dist = (next_fruit.position - fruit.position).abs();
            let time = next_fruit.time - fruit.time;
            let required_time = dist / catcher_speed(true, 1.);
            if required_time > time {
                fruit.hyper = Some(required_time / time);
            };
        }

        let mut sections = Vec::new();

        let mut current_bps = 180. / 60.;
        let mut current_sample_set = osu_types::SampleSet::Normal;

        for tp in &beatmap.timing_points {
            if tp.uninherited {
                let beats_per_second = 1000.0 / tp.beat_length;
                if beats_per_second != current_bps {
                    current_bps = beats_per_second;
                    sections.push(Event {
                        time: tp.time as f32 / 1000.,
                        data: EventData::Timing {
                            bpm: beats_per_second * 60.,
                        },
                    });
                }
            }

            let sample_set = tp.sample_set.unwrap_or(osu_types::SampleSet::Normal);
            if current_sample_set != sample_set {
                current_sample_set = sample_set;

                let hs_kind = match sample_set {
                    osu_types::SampleSet::Normal => HitSoundKind::Normal,
                    osu_types::SampleSet::Soft => HitSoundKind::Soft,
                    osu_types::SampleSet::Drum => HitSoundKind::Drum,
                };
                sections.push(Event {
                    time: tp.time as f32 / 1000.,
                    data: EventData::Hitsound {
                        kind: if tp.sample_index >= 2 {
                            HitSoundKind::Custom(format!(
                                "{}{}",
                                match sample_set {
                                    osu_types::SampleSet::Normal => "normal",
                                    osu_types::SampleSet::Soft => "soft",
                                    osu_types::SampleSet::Drum => "drum",
                                },
                                tp.sample_index
                            ))
                        } else {
                            hs_kind
                        },
                        volume: tp.volume as f32 / 100.,
                    },
                })
            }
        }

        Chart {
            fruits,
            events: sections,
            fall_time: osu_utils::ar_to_ms(beatmap.info.difficulty.ar) / 1000.,
            fruit_radius: osu_utils::cs_to_px(beatmap.info.difficulty.cs),
            // Taken from osu!lazer source code.
            catcher_width: {
                let scale = 1. - 0.7 * (beatmap.info.difficulty.cs - 5.) / 5.;
                106.75 * scale * 0.8
            },
        }
    }
}
