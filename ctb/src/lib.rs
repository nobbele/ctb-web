#![feature(array_windows)]
#![feature(once_cell)]
#![allow(clippy::eq_op)]
#![allow(clippy::new_without_default)]
#![allow(clippy::await_holding_refcell_ref)]
#![allow(clippy::or_fun_call)]

use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};

pub mod azusa;
pub mod cache;
pub mod chart;
pub mod chat;
pub mod config;
pub mod convert;
pub mod frozen;
pub mod leaderboard;
pub mod log;
pub mod math;
pub mod promise;
pub mod rulesets;
pub mod score;
pub mod screen;
pub mod ui;
pub mod web_request;
pub mod web_socket;

pub struct Delay {
    target: f32,
}

impl Future for Delay {
    type Output = ();

    fn poll(self: Pin<&mut Self>, _: &mut Context) -> Poll<Self::Output> {
        if macroquad::time::get_time() as f32 > self.target {
            Poll::Ready(())
        } else {
            Poll::Pending
        }
    }
}

pub fn delay(time: f32) -> Delay {
    Delay {
        target: macroquad::time::get_time() as f32 + time,
    }
}

pub fn draw_text_centered(
    text: &str,
    x: f32,
    y: f32,
    font_size: u16,
    color: macroquad::color::Color,
) {
    let measurements = macroquad::text::measure_text(text, None, font_size, 1.0);
    macroquad::text::draw_text(
        text,
        x - measurements.width / 2.0,
        y,
        font_size as f32,
        color,
    );
}

fn draw_circle_range(
    x: f32,
    y: f32,
    thickness: f32,
    radius: f32,
    range: f32,
    offset_angle: f32,
    color: macroquad::color::Color,
) {
    const DELTA: f32 = std::f32::consts::TAU / 128.;
    let full_angle = std::f32::consts::TAU * range;

    let adjusted_radius = radius - thickness / 2.;

    let mut angle = 0.;
    while angle <= full_angle {
        let first = offset_angle + std::f32::consts::TAU / 4. - angle;
        let second = first + DELTA;
        macroquad::shapes::draw_line(
            x + first.cos() * adjusted_radius,
            y + first.sin() * adjusted_radius,
            x + second.cos() * adjusted_radius,
            y + second.sin() * adjusted_radius,
            thickness,
            color,
        );
        angle += DELTA;
    }
}
