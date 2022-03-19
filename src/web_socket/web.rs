use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

use super::{ConnectionStatus, WebSocketInterface};
use macroquad::prelude::*;
use parking_lot::Mutex;
use wasm_bindgen::{prelude::*, JsCast};

pub struct WebSocket {
    connected: Arc<AtomicBool>,
    error: Arc<Mutex<Option<String>>>,
    ws: web_sys::WebSocket,

    rx: flume::Receiver<Vec<u8>>,

    send_queue: (flume::Sender<Vec<u8>>, flume::Receiver<Vec<u8>>),
}

impl WebSocketInterface for WebSocket {
    fn connect(addr: impl Into<String>) -> Self {
        let addr = addr.into();
        let ws = web_sys::WebSocket::new(&addr).unwrap();
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

        let error = Arc::new(Mutex::new(None));

        let error_clone = error.clone();
        let onerror_callback = Closure::wrap(Box::new(move |e: web_sys::ErrorEvent| {
            info!("Socket error: {:?}", e);
            *error_clone.lock() = Some(e.message());
        }) as Box<dyn FnMut(web_sys::ErrorEvent)>);
        ws.set_onerror(Some(onerror_callback.as_ref().unchecked_ref()));
        onerror_callback.forget();

        let connected = Arc::new(AtomicBool::new(false));

        let cloned_connected = connected.clone();
        let onopen_callback = Closure::wrap(Box::new(move |_| {
            info!("Socket opened..");
            cloned_connected.store(true, Ordering::Relaxed);
        }) as Box<dyn FnMut(JsValue)>);
        ws.set_onopen(Some(onopen_callback.as_ref().unchecked_ref()));
        onopen_callback.forget();

        WebSocket {
            connected,
            error,
            ws,
            rx,
            send_queue: flume::unbounded(),
        }
    }

    fn poll(&mut self) -> Vec<Vec<u8>> {
        if self.status() == ConnectionStatus::Connected {
            for data in self.send_queue.1.drain() {
                self.ws.send_with_u8_array(&data).unwrap();
            }
        }

        self.rx.drain().collect::<Vec<_>>()
    }

    fn send(&self, data: Vec<u8>) {
        self.send_queue.0.send(data).unwrap();
    }

    fn status(&self) -> ConnectionStatus {
        if self.error.lock().is_some() {
            ConnectionStatus::Error
        } else if self.connected.load(Ordering::Relaxed) {
            ConnectionStatus::Connected
        } else {
            ConnectionStatus::Disconnected
        }
    }
}
