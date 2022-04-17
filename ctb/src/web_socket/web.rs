// TODO Investigate whether we need all the atomics and mutexes here.
use super::{ConnectionStatus, WebSocketInterface};
use crate::web_socket::SharedGameData;
use macroquad::prelude::*;
use parking_lot::Mutex;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use wasm_bindgen::{prelude::*, JsCast};

pub struct WebSocket {
    connected: Arc<AtomicBool>,
    error: Arc<Mutex<Option<String>>>,
    ws: Option<web_sys::WebSocket>,

    rx: flume::Receiver<Vec<u8>>,

    send_queue: (flume::Sender<Vec<u8>>, flume::Receiver<Vec<u8>>),
}

impl WebSocketInterface for WebSocket {
    fn new(_data: SharedGameData) -> Self {
        // Broken channel
        let (_, rx) = flume::unbounded();
        WebSocket {
            connected: Arc::new(AtomicBool::new(false)),
            error: Arc::new(Mutex::new(None)),
            ws: None,
            rx,
            send_queue: flume::unbounded(),
        }
    }

    fn reset(&mut self) {
        // Broken channel
        (_, self.rx) = flume::unbounded();
        self.connected.store(false, Ordering::Relaxed);
        self.ws = None;
        *self.error.lock() = None;
    }

    fn connect(&mut self, addr: &str) {
        let ws = web_sys::WebSocket::new(addr).unwrap();
        ws.set_binary_type(web_sys::BinaryType::Blob);

        let (tx, rx) = flume::unbounded();

        let onmessage_callback = Closure::wrap(Box::new(move |e: web_sys::MessageEvent| {
            let blob = e.data().dyn_into::<web_sys::Blob>().expect("Not blob data");
            let fr = web_sys::FileReader::new().unwrap();
            let fr_c = fr.clone();

            let tx = tx.clone();
            let onloadend_cb = Closure::wrap(Box::new(move |_e: web_sys::ProgressEvent| {
                let array = js_sys::Uint8Array::new(&fr_c.result().unwrap());
                let vec = array.to_vec();
                tx.send(vec).unwrap();
            })
                as Box<dyn FnMut(web_sys::ProgressEvent)>);
            fr.set_onloadend(Some(onloadend_cb.as_ref().unchecked_ref()));
            fr.read_as_array_buffer(&blob).expect("Blob not readable");
            onloadend_cb.forget();
        }) as Box<dyn FnMut(web_sys::MessageEvent)>);
        ws.set_onmessage(Some(onmessage_callback.as_ref().unchecked_ref()));
        onmessage_callback.forget();

        let error = Arc::clone(&self.error);
        let onerror_callback = Closure::wrap(Box::new(move |e: web_sys::ErrorEvent| {
            info!("Socket error: {:?}", e);
            *error.lock() = Some(e.message());
        }) as Box<dyn FnMut(web_sys::ErrorEvent)>);
        ws.set_onerror(Some(onerror_callback.as_ref().unchecked_ref()));
        onerror_callback.forget();

        let connected = Arc::clone(&self.connected);
        let onopen_callback = Closure::wrap(Box::new(move |_| {
            info!("Socket opened..");
            connected.store(true, Ordering::Relaxed);
        }) as Box<dyn FnMut(JsValue)>);
        ws.set_onopen(Some(onopen_callback.as_ref().unchecked_ref()));
        onopen_callback.forget();

        self.ws = Some(ws);
        self.rx = rx;
    }

    fn poll(&mut self) -> Result<Vec<Vec<u8>>, String> {
        if let Some(e) = self.error.lock().take() {
            self.connected.store(false, Ordering::Relaxed);
            return Err(format!("Web Socket Error: {}", e));
        }

        if self.status() == ConnectionStatus::Connected {
            for data in self.send_queue.1.drain() {
                self.ws.as_mut().unwrap().send_with_u8_array(&data).unwrap();
            }
        }

        Ok(self.rx.drain().collect::<Vec<_>>())
    }

    fn send(&self, data: Vec<u8>) {
        self.send_queue.0.send(data).unwrap();
    }

    fn status(&self) -> ConnectionStatus {
        if self.connected.load(Ordering::Relaxed) {
            ConnectionStatus::Connected
        } else if self.ws.is_some() {
            ConnectionStatus::Connecting
        } else {
            ConnectionStatus::Disconnected
        }
    }
}
