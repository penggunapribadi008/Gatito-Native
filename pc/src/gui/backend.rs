// Hubungan GUI ke server lokal lewat HTTP dan WebSocket, sama seperti editor.html dulu (fetch + WebSocket).
use super::App;
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio_tungstenite::tungstenite::protocol::Message;

pub type Job = Box<dyn FnOnce(&mut App) + Send>;

#[derive(Clone)]
pub struct Backend {
    base: String,
    rt: tokio::runtime::Handle,
    pub inbox: Arc<Mutex<Vec<Job>>>,
    egui: eframe::egui::Context,
    tx: tokio::sync::mpsc::UnboundedSender<String>,
    client: reqwest::Client,
}

impl Backend {
    pub fn new(rt: tokio::runtime::Handle, egui: eframe::egui::Context, port: u16) -> Backend {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<String>();
        let inbox: Arc<Mutex<Vec<Job>>> = Arc::new(Mutex::new(vec![]));
        let (ib, eg) = (inbox.clone(), egui.clone());
        rt.spawn(async move {
            let url = format!("ws://127.0.0.1:{}/ws", port);
            loop {
                if let Ok((ws, _)) = tokio_tungstenite::connect_async(url.as_str()).await {
                    let (mut sink, mut stream) = ws.split();
                    loop {
                        tokio::select! {
                            m = stream.next() => match m {
                                Some(Ok(Message::Text(t))) => {
                                    if let Ok(v) = serde_json::from_str::<Value>(&t) {
                                        if v.get("t").and_then(|x| x.as_str()) == Some("state") {
                                            if let Some(s) = v.get("state").cloned() {
                                                ib.lock().unwrap().push(Box::new(move |a: &mut App| a.on_state(s)));
                                                eg.request_repaint();
                                            }
                                        }
                                    }
                                }
                                Some(Ok(_)) => {}
                                _ => break,
                            },
                            o = rx.recv() => match o {
                                Some(s) => { if sink.send(Message::Text(s)).await.is_err() { break; } }
                                None => return,
                            }
                        }
                    }
                }
                tokio::time::sleep(Duration::from_millis(1500)).await;
            }
        });
        let client = reqwest::Client::builder().timeout(Duration::from_secs(30)).build().expect("http");
        Backend { base: format!("http://127.0.0.1:{}", port), rt, inbox, egui, tx, client }
    }

    /// Kirim pesan WebSocket, sama dengan send() di editor.html
    pub fn send(&self, m: Value) {
        let _ = self.tx.send(m.to_string());
    }

    pub fn get<F>(&self, path: &str, f: F)
    where
        F: FnOnce(&mut App, Value) + Send + 'static,
    {
        self.call("GET", path, None, f)
    }

    pub fn post<F>(&self, path: &str, body: Option<Value>, f: F)
    where
        F: FnOnce(&mut App, Value) + Send + 'static,
    {
        self.call("POST", path, body, f)
    }

    pub fn post_quiet(&self, path: &str) {
        self.call("POST", path, None, |_, _| {})
    }

    /// Permintaan HTTP di latar belakang. Hasil JSON (atau Null kalau gagal) diteruskan ke f di thread GUI.
    pub fn call<F>(&self, method: &str, path: &str, body: Option<Value>, f: F)
    where
        F: FnOnce(&mut App, Value) + Send + 'static,
    {
        let c = self.client.clone();
        let url = format!("{}{}", self.base, path);
        let (inbox, ctx) = (self.inbox.clone(), self.egui.clone());
        let post = method == "POST";
        self.rt.spawn(async move {
            let rb = if post {
                let r = c.post(url);
                match body {
                    Some(b) => r.header("Content-Type", "application/json").body(b.to_string()),
                    None => r,
                }
            } else {
                c.get(url)
            };
            let v: Value = match rb.send().await {
                Ok(r) => r.json().await.unwrap_or(Value::Null),
                Err(_) => Value::Null,
            };
            inbox.lock().unwrap().push(Box::new(move |a| f(a, v)));
            ctx.request_repaint();
        });
    }

    pub fn raw_get(&self, path: &str) -> String {
        format!("{}{}", self.base, path)
    }
}

pub fn msg_set(p: usize, k: usize, key: &Value) -> Value {
    json!({ "t": "set", "p": p, "k": k, "key": key })
}
