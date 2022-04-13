#[cfg(not(target_arch = "wasm32"))]
mod native;
#[cfg(target_arch = "wasm32")]
mod web;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ConnectionStatus {
    Connected,
    Connecting,
    Disconnected,
}

pub trait WebSocketInterface {
    fn new(data: SharedGameData) -> Self;
    fn reset(&mut self);
    fn connect(&mut self, addr: &str);
    fn poll(&mut self) -> Result<Vec<Vec<u8>>, String>;
    fn send(&self, data: Vec<u8>);
    fn status(&self) -> ConnectionStatus;
}

use std::{
    time::{Duration, Instant},
};

#[cfg(not(target_arch = "wasm32"))]
use native::WebSocket as WebSocketImpl;
#[cfg(target_arch = "wasm32")]
use web::WebSocket as WebSocketImpl;

use crate::{
    log_to,
    screen::{game::SharedGameData},
};

pub struct WebSocket {
    data: SharedGameData,

    addresses: Vec<String>,
    address_index: usize,

    inner: WebSocketImpl,

    next_attempt: Instant,
    connection_timeout: Option<Instant>,
}

impl WebSocket {
    pub fn new(data: SharedGameData, addresses: Vec<impl Into<String>>) -> Self {
        WebSocket {
            data: data.clone(),

            addresses: addresses.into_iter().map(Into::into).collect(),
            address_index: 0,
            inner: WebSocketImpl::new(data),

            next_attempt: Instant::now(),
            connection_timeout: None,
        }
    }

    pub fn poll(&mut self) -> Result<Vec<Vec<u8>>, String> {
        match self.inner.status() {
            ConnectionStatus::Connected => self.address_index = 0,
            ConnectionStatus::Connecting => {
                if Instant::now() >= self.connection_timeout.unwrap() {
                    log_to!(self.data.network, "Connection timed out");

                    // Easy way to reset the connection.
                    self.inner.reset();
                }
            }
            ConnectionStatus::Disconnected => {
                if Instant::now() >= self.next_attempt {
                    let addr: &str = &self.addresses[self.address_index];
                    log_to!(self.data.network, "Attempting to connect to `{}`", addr);

                    self.inner.connect(addr);
                    self.address_index += 1;
                    self.connection_timeout = Some(Instant::now() + Duration::from_secs(1));

                    if self.address_index >= self.addresses.len() {
                        self.next_attempt = Instant::now() + Duration::from_secs(5);
                        self.address_index = 0;
                    }
                }
            }
        }
        self.inner.poll()
    }

    pub fn send(&self, data: Vec<u8>) {
        self.inner.send(data)
    }

    pub fn status(&self) -> ConnectionStatus {
        self.inner.status()
    }
}
