#![feature(array_windows)]
#![allow(clippy::eq_op)]

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
pub mod leaderboard;
pub mod promise;
pub mod score;
pub mod screen;
pub mod ui;
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
