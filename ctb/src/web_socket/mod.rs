#[cfg(not(target_arch = "wasm32"))]
mod native;
#[cfg(target_arch = "wasm32")]
mod web;

#[derive(PartialEq)]
pub enum ConnectionStatus {
    Connected,
    Error,
    Disconnected,
}

pub trait WebSocketInterface {
    fn connect(addr: impl Into<String>) -> Self;
    fn poll(&mut self) -> Result<Vec<Vec<u8>>, String>;
    fn send(&self, data: Vec<u8>);
    fn status(&self) -> ConnectionStatus;
}

#[cfg(not(target_arch = "wasm32"))]
pub use native::WebSocket;
#[cfg(target_arch = "wasm32")]
pub use web::WebSocket;
