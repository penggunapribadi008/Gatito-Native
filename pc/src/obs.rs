// Pemantau OBS: terhubung ke obs-websocket v5 (bawaan OBS 28 ke atas) lalu membaca
// status Live dan Rekaman beserta lamanya. Hasilnya dikirim ke HP lewat server.rs.
use crate::util::take;
use base64::Engine as _;
use futures_util::stream::SplitSink;
use futures_util::{SinkExt, StreamExt};
use once_cell::sync::Lazy;
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::net::TcpStream;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::protocol::Message;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ObsCfg {
    pub on: u8,
    pub host: String,
    pub port: u16,
    pub pass: String,
}

fn def() -> ObsCfg {
    ObsCfg { on: 0, host: "127.0.0.1".into(), port: 4455, pass: "".into() }
}

static HOST_RX: Lazy<Regex> = Lazy::new(|| Regex::new(r"^[A-Za-z0-9.\-]{1,100}$").unwrap());
static SKIP_RX: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?i)virtualcam|replay").unwrap());
static LIVE_RX: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?i)rtmp|srt|rist|whip|ftl|stream").unwrap());

/// Rapikan pengaturan dari editor. Nilai yang tidak diisi memakai nilai lama (old).
pub fn clean(o: Option<&Value>, old: Option<&ObsCfg>) -> ObsCfg {
    let d = old.cloned().unwrap_or_else(def);
    let empty = Map::new();
    let o = o.and_then(|v| v.as_object()).unwrap_or(&empty);
    let host = match o.get("host").and_then(|h| h.as_str()) {
        Some(h) if HOST_RX.is_match(h.trim()) => h.trim().to_string(),
        _ => d.host.clone(),
    };
    let port = {
        let n = match o.get("port") {
            Some(Value::Number(n)) => n.as_f64(),
            Some(Value::String(s)) => s.trim().parse::<f64>().ok(),
            _ => None,
        };
        match n {
            Some(n) if n.fract() == 0.0 && (1.0..=65535.0).contains(&n) => n as u16,
            _ => d.port,
        }
    };
    let pass = match o.get("pass").and_then(|p| p.as_str()) {
        Some(p) => take(p, 200),
        None => d.pass.clone(),
    };
    let on = match o.get("on") {
        None => d.on,
        Some(Value::Number(n)) if n.as_f64() == Some(1.0) => 1,
        Some(Value::Bool(true)) => 1,
        Some(Value::String(s)) if s == "1" => 1,
        _ => 0,
    };
    ObsCfg { on, host, port, pass }
}

#[derive(Clone, Serialize, Default, Debug)]
pub struct OutInfo {
    pub name: String,
    pub kind: String,
    #[serde(rename = "type")]
    pub typ: String,
    pub active: bool,
    pub ms: u64,
}

/// link: 'off' (dimatikan) | 'connecting' (OBS belum bisa dihubungi) | 'auth' (password salah) | 'ok'
#[derive(Clone, Serialize, Debug)]
pub struct ObsState {
    pub link: String,
    pub msg: String,
    pub live: bool,
    pub recon: bool,
    #[serde(rename = "liveMs")]
    pub live_ms: u64,
    pub rec: bool,
    pub paused: bool,
    #[serde(rename = "recMs")]
    pub rec_ms: u64,
    pub outs: Vec<OutInfo>,
}

impl ObsState {
    fn new() -> Self {
        ObsState { link: "off".into(), msg: "".into(), live: false, recon: false, live_ms: 0, rec: false, paused: false, rec_ms: 0, outs: vec![] }
    }
    /// HP tidak butuh daftar output
    pub fn phone(&self) -> Value {
        let mut v = serde_json::to_value(self).unwrap_or(json!({}));
        if let Some(m) = v.as_object_mut() {
            m.remove("outs");
        }
        v
    }
}

#[derive(Default, Clone)]
struct MainSt {
    live: bool,
    recon: bool,
    live_ms: u64,
    rec: bool,
    paused: bool,
    rec_ms: u64,
}

#[derive(Clone)]
struct Out {
    name: String,
    kind: String,
    typ: &'static str, // 'live' | 'rec' | 'skip'
    active: bool,
    recon: bool,
    ms: u64,
}

#[derive(Default)]
struct Work {
    m: MainSt,
    x: Vec<Out>, // semua output OBS (termasuk dari plugin seperti Aitum Vertical)
}

struct Inner {
    s: Mutex<ObsState>,
    last: Mutex<String>,
    task: Mutex<Option<tokio::task::JoinHandle<()>>>,
    cb: Arc<dyn Fn(ObsState) + Send + Sync>,
    rt: tokio::runtime::Handle,
}

#[derive(Clone)]
pub struct Obs {
    inner: Arc<Inner>,
}

pub fn create(rt: tokio::runtime::Handle, cb: Arc<dyn Fn(ObsState) + Send + Sync>) -> Obs {
    Obs {
        inner: Arc::new(Inner { s: Mutex::new(ObsState::new()), last: Mutex::new(String::new()), task: Mutex::new(None), cb, rt }),
    }
}

impl Obs {
    pub fn state(&self) -> ObsState {
        self.inner.s.lock().unwrap().clone()
    }
    pub fn configure(&self, c: &ObsCfg) {
        let inner = self.inner.clone();
        if let Some(h) = inner.task.lock().unwrap().take() {
            h.abort();
        }
        {
            let mut s = inner.s.lock().unwrap();
            reset_fields(&mut s);
            if c.on == 0 {
                s.link = "off".into();
                s.msg = "".into();
            }
        }
        if c.on == 0 {
            emit(&inner, false);
            return;
        }
        let cfg = c.clone();
        let i2 = inner.clone();
        let h = inner.rt.spawn(async move { session(i2, cfg).await });
        *inner.task.lock().unwrap() = Some(h);
    }
}

fn reset_fields(s: &mut ObsState) {
    s.live = false;
    s.recon = false;
    s.live_ms = 0;
    s.rec = false;
    s.paused = false;
    s.rec_ms = 0;
    s.outs.clear();
}

fn emit(inner: &Inner, force: bool) {
    let snap = inner.s.lock().unwrap().clone();
    let j = serde_json::to_string(&snap).unwrap_or_default();
    {
        let mut last = inner.last.lock().unwrap();
        if !force && *last == j {
            return;
        }
        *last = j;
    }
    (inner.cb)(snap);
}

/// Jenis output: 'live' (siaran ke layanan), 'rec' (simpan ke file), 'skip' (virtual cam, replay buffer: tidak dihitung)
fn type_of(o: &Value) -> &'static str {
    let kind = o.get("outputKind").and_then(|v| v.as_str()).unwrap_or("");
    let name = o.get("outputName").and_then(|v| v.as_str()).unwrap_or("");
    let kn = format!("{} {}", kind, name);
    if SKIP_RX.is_match(&kn) {
        return "skip";
    }
    if let Some(f) = o.get("outputFlags").and_then(|f| f.as_object()) {
        if let Some(sv) = f.get("OBS_OUTPUT_SERVICE") {
            let truthy = match sv {
                Value::Bool(b) => *b,
                Value::Number(n) => n.as_f64().map(|x| x != 0.0).unwrap_or(false),
                Value::Null => false,
                _ => true,
            };
            return if truthy { "live" } else { "rec" };
        }
    }
    if LIVE_RX.is_match(&kn) {
        "live"
    } else {
        "rec"
    }
}

/// Gabungkan output utama dan output plugin: ada yang aktif berarti aktif, durasi diambil yang terlama
fn merge(w: &Work, s: &mut ObsState) {
    let m = &w.m;
    let (mut live, mut live_ms, mut recon, mut rec, mut rec_ms) = (m.live, m.live_ms, m.recon, m.rec, m.rec_ms);
    for o in &w.x {
        if !o.active || o.typ == "skip" {
            continue;
        }
        if o.typ == "live" {
            live = true;
            live_ms = live_ms.max(o.ms);
            recon = recon || o.recon;
        } else {
            rec = true;
            rec_ms = rec_ms.max(o.ms);
        }
    }
    s.live = live;
    s.live_ms = if live { live_ms } else { 0 };
    s.recon = live && recon;
    s.rec = rec;
    s.rec_ms = if rec { rec_ms } else { 0 };
    s.paused = rec && m.paused;
    s.outs = w
        .x
        .iter()
        .map(|o| OutInfo { name: o.name.clone(), kind: o.kind.clone(), typ: o.typ.to_string(), active: o.active, ms: if o.active { o.ms } else { 0 } })
        .collect();
}

type Tx = SplitSink<WebSocketStream<MaybeTlsStream<TcpStream>>, Message>;

async fn ask(tx: &mut Tx, typ: &str, id: &str, data: Option<Value>) {
    let mut d = json!({ "requestType": typ, "requestId": id });
    if let Some(rd) = data {
        d["requestData"] = rd;
    }
    let _ = tx.send(Message::Text(json!({ "op": 6, "d": d }).to_string())).await;
}

async fn ask_out(tx: &mut Tx, name: &str) {
    ask(tx, "GetOutputStatus", &format!("o|{}", name), Some(json!({ "outputName": name }))).await;
}

async fn session(inner: Arc<Inner>, cfg: ObsCfg) {
    loop {
        let mut w = Work::default();
        {
            let mut s = inner.s.lock().unwrap();
            reset_fields(&mut s);
            s.link = "connecting".into();
            s.msg = "Menghubungkan ke OBS…".into();
        }
        emit(&inner, false);

        let url = format!("ws://{}:{}", cfg.host, cfg.port);
        let req = match url.into_client_request() {
            Ok(mut r) => {
                r.headers_mut().insert("Sec-WebSocket-Protocol", "obswebsocket.json".parse().unwrap());
                r
            }
            Err(_) => {
                inner.s.lock().unwrap().msg = "Alamat OBS tidak valid.".into();
                emit(&inner, false);
                tokio::time::sleep(Duration::from_millis(5000)).await;
                continue;
            }
        };

        let mut close_code: u16 = 1006;
        match tokio::time::timeout(Duration::from_secs(5), tokio_tungstenite::connect_async(req)).await {
            Ok(Ok((ws, _))) => {
                let (mut tx, mut rx) = ws.split();
                let mut ident = false;
                let mut tick_n: u64 = 0;
                let mut poll = tokio::time::interval(Duration::from_secs(1));
                poll.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
                loop {
                    tokio::select! {
                        _ = poll.tick(), if ident => {
                            ask(&mut tx, "GetStreamStatus", "s", None).await;
                            ask(&mut tx, "GetRecordStatus", "r", None).await;
                            // daftar output dicek ulang tiap 3 detik: plugin bisa membuat output baru saat mulai live
                            if tick_n % 3 == 0 {
                                ask(&mut tx, "GetOutputList", "l", None).await;
                            }
                            tick_n += 1;
                            let names: Vec<String> = w.x.iter().filter(|o| o.typ != "skip").map(|o| o.name.clone()).collect();
                            for n in names {
                                ask_out(&mut tx, &n).await;
                            }
                        }
                        msg = rx.next() => {
                            let m = match msg {
                                Some(Ok(Message::Text(t))) => match serde_json::from_str::<Value>(&t) { Ok(v) => v, Err(_) => continue },
                                Some(Ok(Message::Close(f))) => { close_code = f.map(|f| u16::from(f.code)).unwrap_or(1005); break; }
                                Some(Ok(_)) => continue,
                                Some(Err(_)) | None => { close_code = 1006; break; }
                            };
                            match m.get("op").and_then(|o| o.as_i64()) {
                                Some(0) => {
                                    // Hello -> Identify (dengan jawaban tantangan kalau OBS memakai password)
                                    let mut id = json!({ "rpcVersion": 1 });
                                    let a = m.pointer("/d/authentication");
                                    if let Some(a) = a {
                                        if let (Some(ch), Some(salt)) = (a.get("challenge").and_then(|c| c.as_str()), a.get("salt").and_then(|c| c.as_str())) {
                                            if !ch.is_empty() && !salt.is_empty() {
                                                let b64 = base64::engine::general_purpose::STANDARD;
                                                let secret = b64.encode(Sha256::digest(format!("{}{}", cfg.pass, salt).as_bytes()));
                                                id["authentication"] = json!(b64.encode(Sha256::digest(format!("{}{}", secret, ch).as_bytes())));
                                            }
                                        }
                                    }
                                    let _ = tx.send(Message::Text(json!({ "op": 1, "d": id }).to_string())).await;
                                }
                                Some(2) => {
                                    // Identified
                                    ident = true;
                                    {
                                        let mut s = inner.s.lock().unwrap();
                                        s.link = "ok".into();
                                        s.msg = "".into();
                                    }
                                    poll.reset_immediately();
                                    emit(&inner, true);
                                }
                                Some(5) => {
                                    // Event: segarkan langsung saat Live atau Rekaman mulai/berhenti
                                    let t = m.pointer("/d/eventType").and_then(|t| t.as_str()).unwrap_or("");
                                    if t == "StreamStateChanged" || t == "RecordStateChanged" {
                                        poll.reset_immediately();
                                    }
                                }
                                Some(7) => {
                                    let d = match m.get("d") { Some(d) => d, None => continue };
                                    let ok = d.pointer("/requestStatus/result").and_then(|r| r.as_bool()).unwrap_or(false);
                                    let r = match d.get("responseData") { Some(r) if ok => r, _ => continue };
                                    let ms = r.get("outputDuration").and_then(|x| x.as_f64()).unwrap_or(0.0).round().max(0.0) as u64;
                                    let active = r.get("outputActive").and_then(|x| x.as_bool()).unwrap_or(false);
                                    let reconn = r.get("outputReconnecting").and_then(|x| x.as_bool()).unwrap_or(false);
                                    let paused = r.get("outputPaused").and_then(|x| x.as_bool()).unwrap_or(false);
                                    let rt = d.get("requestType").and_then(|x| x.as_str()).unwrap_or("");
                                    let rid = d.get("requestId").and_then(|x| x.as_str()).unwrap_or("");
                                    if rt == "GetStreamStatus" {
                                        w.m.live = active;
                                        w.m.recon = active && reconn;
                                        w.m.live_ms = if active { ms } else { 0 };
                                    } else if rt == "GetRecordStatus" {
                                        w.m.rec = active;
                                        w.m.paused = active && paused;
                                        w.m.rec_ms = if active { ms } else { 0 };
                                    } else if rt == "GetOutputList" && r.get("outputs").map(|o| o.is_array()).unwrap_or(false) {
                                        let mut names: Vec<String> = vec![];
                                        let mut to_ask: Vec<String> = vec![];
                                        for o in r["outputs"].as_array().unwrap() {
                                            let name = match o.get("outputName").and_then(|n| n.as_str()) { Some(n) => n.to_string(), None => continue };
                                            names.push(name.clone());
                                            let kind = o.get("outputKind").and_then(|k| k.as_str()).unwrap_or("").to_string();
                                            let typ = type_of(o);
                                            if let Some(old) = w.x.iter_mut().find(|e| e.name == name) {
                                                old.kind = kind;
                                                old.typ = typ;
                                            } else {
                                                w.x.push(Out { name: name.clone(), kind, typ, active: false, recon: false, ms: 0 });
                                                if typ != "skip" {
                                                    to_ask.push(name);
                                                }
                                            }
                                        }
                                        w.x.retain(|e| names.contains(&e.name));
                                        for n in to_ask {
                                            ask_out(&mut tx, &n).await;
                                        }
                                    } else if rt == "GetOutputStatus" && rid.starts_with("o|") {
                                        let n = &rid[2..];
                                        if let Some(o) = w.x.iter_mut().find(|e| e.name == n) {
                                            o.active = active;
                                            o.recon = active && reconn;
                                            o.ms = if active { ms } else { 0 };
                                        }
                                    }
                                    {
                                        let mut s = inner.s.lock().unwrap();
                                        merge(&w, &mut s);
                                    }
                                    emit(&inner, false);
                                }
                                _ => {}
                            }
                        }
                    }
                }
            }
            _ => {}
        }

        // koneksi putus
        {
            let mut s = inner.s.lock().unwrap();
            reset_fields(&mut s);
            if close_code == 4009 {
                s.link = "auth".into();
                s.msg = "OBS meminta password, atau password yang diisi salah.".into();
            } else {
                s.link = "connecting".into();
                s.msg = "OBS belum terbuka, atau WebSocket Server di OBS belum diaktifkan. Mencoba lagi…".into();
            }
        }
        emit(&inner, false);
        tokio::time::sleep(Duration::from_millis(if close_code == 4009 { 10000 } else { 4000 })).await;
    }
}
