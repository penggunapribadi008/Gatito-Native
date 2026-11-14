// Server HTTP + WebSocket (port dari server.js): editor di PC, HP lewat WebSocket, pairing lewat QR
use crate::app::{new_code, Ctx};
use crate::input;
use crate::obs;
use crate::state::{clean_audio, Key, Page, Profile};
use crate::update;
use crate::util::{self, sha1_hex, take, VERSION};
use axum::body::Bytes;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{ConnectInfo, Path, Query, State};
use axum::http::{header, HeaderMap, Method, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::Router;
use futures_util::{SinkExt, StreamExt};
use once_cell::sync::Lazy;
use regex::Regex;
use serde_json::{json, Map, Value};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tower_http::cors::{Any, CorsLayer};

type C = State<Arc<Ctx>>;
type A = ConnectInfo<SocketAddr>;

const PHONE: &str = include_str!("../assets/phone.html");
pub const OBS_LOGO: &[u8] = include_bytes!("../assets/obs-logo.png");

// Port yang biasanya sudah dipakai program lain (ditolak saat user mengganti port)
const AVOID: [(u16, &str); 14] = [
    (3306, "MySQL"), (3389, "Remote Desktop Windows"), (4200, "Angular"), (4455, "OBS WebSocket"), (5000, "Flask / AirPlay"),
    (5173, "Vite"), (5432, "PostgreSQL"), (5500, "Live Server VS Code"), (5900, "VNC"), (6379, "Redis"),
    (8000, "server web lokal"), (8080, "server web / proxy"), (8888, "Jupyter"), (27017, "MongoDB"),
];

static IMG_RX: Lazy<Regex> = Lazy::new(|| Regex::new(r"^[a-f0-9]{40}\.(png|gif|jpg|webp)$").unwrap());
static IMG_FIELD_RX: Lazy<Regex> = Lazy::new(|| Regex::new(r"^/img/[a-f0-9]{40}\.(png|gif|jpg|webp)$").unwrap());

fn local(a: &SocketAddr) -> bool {
    a.ip().is_loopback()
}
fn nf() -> Response {
    StatusCode::NOT_FOUND.into_response()
}
fn js(v: Value) -> Response {
    ([(header::CONTENT_TYPE, "application/json")], v.to_string()).into_response()
}
fn body(b: &Bytes) -> Value {
    serde_json::from_slice(b).unwrap_or_else(|_| json!({}))
}
fn same_origin(h: &HeaderMap, port: u16) -> bool {
    match h.get(header::ORIGIN).and_then(|o| o.to_str().ok()) {
        None => true,
        Some(o) => o == format!("http://127.0.0.1:{}", port) || o == format!("http://localhost:{}", port),
    }
}
fn forbidden() -> Response {
    (StatusCode::FORBIDDEN, "forbidden").into_response()
}

pub fn router(ctx: Arc<Ctx>) -> Router {
    Router::new()
        .route("/hello", get(hello))
        .route("/img/:name", get(img))
        .route("/", get(phone))
        .route("/appinfo", get(appinfo))
        .route("/obs-logo.png", get(obs_logo))
        .route("/update/check", post(up_check))
        .route("/update/status", get(up_status))
        .route("/update/install", post(up_install))
        .route("/update/open", post(up_open))
        .route("/pairinfo", get(pairinfo))
        .route("/portinfo", get(portinfo))
        .route("/port-set", post(port_set))
        .route("/restart", post(restart))
        .route("/apps", get(apps))
        .route("/icon", get(icon))
        .route("/vpad/open", post(vpad_open))
        .route("/vpad/close", post(vpad_close))
        .route("/audio/settings", get(audio_get).post(audio_set))
        .route("/audio/info", get(audio_info))
        .route("/audio/level", get(audio_level))
        .route("/audio/refresh", post(audio_refresh))
        .route("/audio/test", post(audio_test))
        .route("/audio/install/status", get(vb_status))
        .route("/audio/install", post(vb_install))
        .route("/audio/restoremic", post(vb_restore))
        .route("/obs/settings", get(obs_get).post(obs_set))
        .route("/obs/status", get(obs_status))
        .route("/qr.svg", get(qr))
        .route("/pick/audio", post(pick_audio))
        .route("/pick/image", post(pick_image))
        .route("/pair", post(pair))
        .route("/ws", get(ws_route))
        .route("/__show", get(show))
        .layer(CorsLayer::new().allow_origin(Any).allow_headers([header::CONTENT_TYPE]).allow_methods([Method::GET, Method::POST, Method::OPTIONS]))
        .fallback(|| async { nf() })
        .with_state(ctx)
}

async fn hello() -> Response {
    js(json!({ "app": "streamdeck-hp", "name": util::hostname_str() }))
}

async fn img(State(ctx): C, Path(name): Path<String>) -> Response {
    let m = match IMG_RX.captures(&name) {
        Some(m) => m,
        None => return nf(),
    };
    let ext = &m[1];
    match std::fs::read(ctx.dir.join("images").join(&name)) {
        Ok(b) => (
            [(header::CONTENT_TYPE, format!("image/{}", if ext == "jpg" { "jpeg" } else { ext })), (header::CACHE_CONTROL, "max-age=31536000".to_string())],
            b,
        )
            .into_response(),
        Err(_) => nf(),
    }
}

async fn phone() -> Response {
    let t = PHONE.replace("__VERSION__", VERSION).replace("__REPO__", util::repo());
    ([(header::CONTENT_TYPE, "text/html; charset=utf-8")], t).into_response()
}

async fn appinfo(ConnectInfo(a): A) -> Response {
    if !local(&a) {
        return nf();
    }
    js(json!({ "version": VERSION }))
}

async fn obs_logo(ConnectInfo(a): A) -> Response {
    if !local(&a) {
        return nf();
    }
    ([(header::CONTENT_TYPE, "image/png")], OBS_LOGO).into_response()
}

async fn show(State(ctx): C, ConnectInfo(a): A) -> Response {
    if !local(&a) {
        return nf();
    }
    ctx.send_ui(crate::shell::UiCmd::ShowMain);
    js(json!({ "ok": 1 }))
}

// ---- update ----
async fn up_check(ConnectInfo(a): A) -> Response {
    if !local(&a) {
        return nf();
    }
    js(update::check_update().await.json)
}
async fn up_status(State(ctx): C, ConnectInfo(a): A) -> Response {
    if !local(&a) {
        return nf();
    }
    js(serde_json::to_value(&*ctx.up.lock().unwrap()).unwrap())
}
async fn up_install(State(ctx): C, ConnectInfo(a): A, h: HeaderMap) -> Response {
    if !local(&a) {
        return nf();
    }
    if !same_origin(&h, ctx.port) {
        return forbidden();
    }
    {
        let u = ctx.up.lock().unwrap();
        if u.state == "down" || u.state == "ready" {
            return js(serde_json::to_value(&*u).unwrap());
        }
    }
    let r = update::check_update().await;
    let st = r.json.get("state").and_then(|s| s.as_str()).unwrap_or("");
    match r.asset {
        Some(asset) if st == "new" => {
            {
                let mut u = ctx.up.lock().unwrap();
                u.state = "down".into();
                u.pct = 0;
            }
            let c2 = ctx.clone();
            tokio::spawn(async move { update::fetch_update(c2, asset).await });
        }
        _ => ctx.up.lock().unwrap().state = "error".into(),
    }
    js(serde_json::to_value(&*ctx.up.lock().unwrap()).unwrap())
}
async fn up_open(ConnectInfo(a): A, State(ctx): C, h: HeaderMap, b: Bytes) -> Response {
    if !local(&a) {
        return nf();
    }
    if !same_origin(&h, ctx.port) {
        return forbidden();
    }
    if let Some(u) = body(&b).get("url").and_then(|u| u.as_str()) {
        if u.starts_with("https://github.com/") {
            util::open_target(u);
        }
    }
    js(json!({ "ok": 1 }))
}

// ---- pairing / port ----
async fn pairinfo(State(ctx): C, ConnectInfo(a): A) -> Response {
    if !local(&a) {
        return nf();
    }
    js(json!({ "code": *ctx.code.lock().unwrap(), "ips": util::ips(), "port": ctx.port, "wanted": ctx.wanted }))
}
async fn portinfo(State(ctx): C, ConnectInfo(a): A) -> Response {
    if !local(&a) {
        return nf();
    }
    js(json!({ "port": ctx.port, "avoid": AVOID.iter().map(|(p, n)| json!({ "port": p, "name": n })).collect::<Vec<_>>() }))
}
async fn port_set(State(ctx): C, ConnectInfo(a): A, h: HeaderMap, b: Bytes) -> Response {
    if !local(&a) {
        return nf();
    }
    if !same_origin(&h, ctx.port) {
        return forbidden();
    }
    let j = body(&b);
    let n = match j.get("port") {
        Some(Value::Number(n)) => n.as_f64(),
        Some(Value::String(s)) => s.trim().parse::<f64>().ok(),
        _ => None,
    };
    let n = match n {
        Some(n) if n.fract() == 0.0 && (1024.0..=65535.0).contains(&n) => n as u16,
        _ => return js(json!({ "error": "Port harus angka bulat antara 1024 dan 65535." })),
    };
    if n != ctx.port {
        if let Some((_, name)) = AVOID.iter().find(|(p, _)| *p == n) {
            return js(json!({ "error": format!("localhost:{} telah digunakan oleh {}. Pilih port lain (tombol Hindari menampilkan daftarnya).", n, name) }));
        }
        if !util::port_free(n) {
            return js(json!({ "error": format!("localhost:{} telah digunakan oleh program lain. Pilih port lain.", n) }));
        }
    }
    if std::fs::write(ctx.dir.join("port.json"), json!({ "port": n }).to_string()).is_err() {
        return js(json!({ "error": "Gagal menyimpan port." }));
    }
    js(json!({ "ok": true, "restart": n != ctx.port }))
}
async fn restart(State(ctx): C, ConnectInfo(a): A, h: HeaderMap) -> Response {
    if !local(&a) {
        return nf();
    }
    if !same_origin(&h, ctx.port) {
        return forbidden();
    }
    std::thread::spawn(|| {
        std::thread::sleep(Duration::from_millis(300));
        crate::app::relaunch();
    });
    js(json!({ "ok": true }))
}

// ---- aplikasi dan ikon ----
async fn apps(State(ctx): C, ConnectInfo(a): A) -> Response {
    if !local(&a) {
        return nf();
    }
    let list = tokio::task::spawn_blocking(crate::apps::list_apps).await.unwrap_or_default();
    crate::apps::extract_icons(&list, &ctx.dir.join("icons"));
    *ctx.apps.lock().unwrap() = list.clone();
    js(serde_json::to_value(list).unwrap())
}
async fn icon(State(ctx): C, ConnectInfo(a): A, Query(q): Query<HashMap<String, String>>) -> Response {
    if !local(&a) {
        return nf();
    }
    let i: usize = q.get("i").and_then(|i| i.parse().ok()).unwrap_or(usize::MAX);
    let app = ctx.apps.lock().unwrap().get(i).cloned();
    match app {
        Some(ap) => match std::fs::read(ctx.dir.join("icons").join(format!("{}.png", sha1_hex(ap.path.as_bytes())))) {
            Ok(b) => ([(header::CONTENT_TYPE, "image/png")], b).into_response(),
            Err(_) => nf(),
        },
        None => nf(),
    }
}
async fn vpad_open(State(ctx): C, ConnectInfo(a): A) -> Response {
    if !local(&a) {
        return nf();
    }
    ctx.send_ui(crate::shell::UiCmd::OpenVPad);
    js(json!({ "ok": 1 }))
}
async fn vpad_close(State(ctx): C, ConnectInfo(a): A) -> Response {
    if !local(&a) {
        return nf();
    }
    ctx.send_ui(crate::shell::UiCmd::CloseVPad);
    js(json!({ "ok": 1 }))
}

// ---- audio ----
async fn audio_get(State(ctx): C, ConnectInfo(a): A) -> Response {
    if !local(&a) {
        return nf();
    }
    js(serde_json::to_value(&ctx.st.lock().unwrap().audio).unwrap())
}
async fn audio_set(State(ctx): C, ConnectInfo(a): A, b: Bytes) -> Response {
    if !local(&a) {
        return nf();
    }
    let j = body(&b);
    let (was_auto, cfg) = {
        let mut st = ctx.st.lock().unwrap();
        let was = st.audio.auto;
        st.audio = clean_audio(Some(&j));
        ctx.save(&st);
        (was, st.audio.clone())
    };
    ctx.engine.apply(cfg.clone(), false);
    if was_auto == 0 && cfg.auto == 1 {
        let c2 = ctx.clone();
        tokio::spawn(async move { c2.auto_mic_on().await });
    }
    js(serde_json::to_value(&cfg).unwrap())
}
async fn audio_info(State(ctx): C, ConnectInfo(a): A) -> Response {
    if !local(&a) {
        return nf();
    }
    let cfg = serde_json::to_value(&ctx.st.lock().unwrap().audio).unwrap();
    let info = ctx.engine.info(8000).await;
    let mut m = Map::new();
    m.insert("cfg".into(), cfg);
    m.insert("ready".into(), json!(info.is_some()));
    if let Some(Value::Object(i)) = info {
        for (k, v) in i {
            m.insert(k, v);
        }
    }
    js(Value::Object(m))
}
async fn audio_level(State(ctx): C, ConnectInfo(a): A) -> Response {
    if !local(&a) {
        return nf();
    }
    let (mic, out) = ctx.engine.level();
    js(json!({ "mic": mic, "out": out }))
}
async fn audio_refresh(State(ctx): C, ConnectInfo(a): A) -> Response {
    if !local(&a) {
        return nf();
    }
    let cfg = ctx.st.lock().unwrap().audio.clone();
    ctx.engine.apply(cfg, true);
    js(json!({ "ok": 1 }))
}
async fn audio_test(State(ctx): C, ConnectInfo(a): A) -> Response {
    if !local(&a) {
        return nf();
    }
    ctx.engine.test();
    js(json!({ "ok": 1 }))
}
async fn vb_status(State(ctx): C, ConnectInfo(a): A) -> Response {
    if !local(&a) {
        return nf();
    }
    js(serde_json::to_value(ctx.vb.status()).unwrap())
}
async fn vb_install(State(ctx): C, ConnectInfo(a): A, b: Bytes) -> Response {
    if !local(&a) {
        return nf();
    }
    let install = body(&b).get("install").map(|v| v.as_bool().unwrap_or(false) || v.as_i64() == Some(1)).unwrap_or(false);
    if ctx.vb.status().running {
        return js(serde_json::to_value(ctx.vb.status()).unwrap());
    }
    let c2 = ctx.clone();
    tokio::spawn(async move {
        let inf = c2.engine.info(5000).await;
        let m = inf.as_ref().and_then(|i| i.get("mic")).and_then(|m| m.as_str()).unwrap_or("");
        let eng = if !m.is_empty() && !["Mati", "Belum", "Gagal", "Tidak"].iter().any(|p| m.starts_with(p)) { m.to_string() } else { String::new() };
        let mic = c2.st.lock().unwrap().audio.mic.label.clone();
        let res = c2.vb.run(crate::winaudio::RunOpt { install, mic, engine_mic: eng }).await;
        // Simpan mic asli supaya Gatito tetap memakainya walau default Windows sudah jadi Virtual Mic
        {
            let mut st = c2.st.lock().unwrap();
            if res.ok && !res.mic.is_empty() && res.mic != "Microphone" && st.audio.mic.id.is_empty() && st.audio.mic.label.is_empty() {
                st.audio.mic = crate::state::DevSel { id: "".into(), label: res.mic.clone() };
                c2.save(&st);
            }
        }
        let cfg = c2.st.lock().unwrap().audio.clone();
        c2.engine.apply(cfg, true);
    });
    js(serde_json::to_value(ctx.vb.status()).unwrap())
}
async fn vb_restore(State(ctx): C, ConnectInfo(a): A) -> Response {
    if !local(&a) {
        return nf();
    }
    let (label, cfg) = {
        let st = ctx.st.lock().unwrap();
        (st.audio.mic.label.clone(), st.audio.clone())
    };
    let mut real = label;
    if real.is_empty() {
        if let Some(inf) = ctx.engine.info(5000).await {
            let m = inf.get("mic").and_then(|m| m.as_str()).unwrap_or("");
            if !m.is_empty() && !["Mati", "Belum", "Gagal", "Tidak"].iter().any(|p| m.starts_with(p)) {
                real = m.to_string();
            }
        }
    }
    let r = ctx.vb.restore_mic(&real).await;
    ctx.engine.apply(cfg, true);
    js(r)
}

// ---- OBS ----
fn obs_pub(ctx: &Ctx) -> Value {
    let st = ctx.st.lock().unwrap();
    json!({ "on": st.obs.on, "host": st.obs.host, "port": st.obs.port, "hasPass": !st.obs.pass.is_empty() })
}
async fn obs_get(State(ctx): C, ConnectInfo(a): A) -> Response {
    if !local(&a) {
        return nf();
    }
    js(obs_pub(&ctx))
}
async fn obs_status(State(ctx): C, ConnectInfo(a): A) -> Response {
    if !local(&a) {
        return nf();
    }
    js(serde_json::to_value(ctx.obs.state()).unwrap())
}
async fn obs_set(State(ctx): C, ConnectInfo(a): A, h: HeaderMap, b: Bytes) -> Response {
    if !local(&a) {
        return nf();
    }
    if !same_origin(&h, ctx.port) {
        return forbidden();
    }
    let j = body(&b);
    let cfg = {
        let mut st = ctx.st.lock().unwrap();
        st.obs = obs::clean(Some(&j), Some(&st.obs));
        ctx.save(&st);
        st.obs.clone()
    };
    ctx.obs.configure(&cfg);
    js(obs_pub(&ctx))
}

// ---- QR, pilih berkas, pairing ----
async fn qr(State(ctx): C, ConnectInfo(a): A) -> Response {
    if !local(&a) {
        return nf();
    }
    let ips = util::ips();
    let url = format!("http://{}:{}/?code={}&h={}", ips.first().cloned().unwrap_or_else(|| "127.0.0.1".into()), ctx.port, ctx.code.lock().unwrap(), ips.join(","));
    match qrcode::QrCode::new(url.as_bytes()) {
        Ok(c) => {
            let svg = c.render::<qrcode::render::svg::Color>().min_dimensions(200, 200).dark_color(qrcode::render::svg::Color("#000000")).light_color(qrcode::render::svg::Color("#ffffff")).build();
            ([(header::CONTENT_TYPE, "image/svg+xml")], svg).into_response()
        }
        Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, "").into_response(),
    }
}
async fn pick_audio(ConnectInfo(a): A) -> Response {
    if !local(&a) {
        return nf();
    }
    let f = tokio::task::spawn_blocking(|| {
        rfd::FileDialog::new().add_filter("Audio", &["mp3", "wav", "ogg", "m4a", "aac", "flac"]).pick_file().map(|p| p.to_string_lossy().to_string())
    })
    .await
    .ok()
    .flatten();
    js(json!({ "file": f }))
}
async fn pick_image(State(ctx): C, ConnectInfo(a): A) -> Response {
    if !local(&a) {
        return nf();
    }
    let picked = tokio::task::spawn_blocking(|| -> Option<(Vec<u8>, &'static str)> {
        let f = rfd::FileDialog::new().add_filter("Gambar dan GIF", &["png", "jpg", "jpeg", "gif", "bmp"]).pick_file()?;
        let is_gif = f.extension().and_then(|e| e.to_str()).map(|e| e.eq_ignore_ascii_case("gif")).unwrap_or(false);
        if is_gif {
            let d = std::fs::read(&f).ok()?;
            if d.len() > 6_000_000 {
                rfd::MessageDialog::new().set_level(rfd::MessageLevel::Error).set_title("GIF terlalu besar").set_description("Ukuran GIF maksimal 6 MB.").show();
                return None;
            }
            return Some((d, "gif"));
        }
        // potong persegi di tengah lalu kecilkan jadi 144x144
        let im = image::open(&f).ok()?;
        let (w, h) = (im.width(), im.height());
        let m = w.min(h);
        if m == 0 {
            return None;
        }
        let im = im.crop_imm((w - m) / 2, (h - m) / 2, m, m).resize_exact(144, 144, image::imageops::FilterType::CatmullRom);
        let mut out = std::io::Cursor::new(Vec::new());
        im.write_to(&mut out, image::ImageFormat::Png).ok()?;
        Some((out.into_inner(), "png"))
    })
    .await
    .ok()
    .flatten();
    match picked {
        None => js(json!({ "img": null })),
        Some((data, ext)) => {
            let name = format!("{}.{}", sha1_hex(&data), ext);
            let dir = ctx.dir.join("images");
            let _ = std::fs::create_dir_all(&dir);
            let _ = std::fs::write(dir.join(&name), &data);
            js(json!({ "img": format!("/img/{}", name) }))
        }
    }
}
async fn pair(State(ctx): C, b: Bytes) -> Response {
    let j = body(&b);
    let given = match j.get("code") {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Number(n)) => n.to_string(),
        _ => String::new(),
    };
    let ok = !given.is_empty() && given == *ctx.code.lock().unwrap();
    if !ok {
        let mut bad = ctx.bad.lock().unwrap();
        *bad += 1;
        if *bad >= 5 {
            *ctx.code.lock().unwrap() = new_code();
            *bad = 0;
        }
        return (StatusCode::FORBIDDEN, "kode salah").into_response();
    }
    let rnd: [u8; 16] = rand::random();
    let token = hex::encode(rnd);
    let name = j.get("name").and_then(|n| n.as_str()).filter(|n| !n.is_empty()).map(|n| take(n, 40)).unwrap_or_else(|| "HP".into());
    {
        let mut st = ctx.st.lock().unwrap();
        st.devices.push(crate::state::Device { name, token: token.clone() });
        ctx.save(&st);
    }
    *ctx.code.lock().unwrap() = new_code();
    *ctx.bad.lock().unwrap() = 0;
    js(json!({ "token": token, "pc": util::hostname_str() }))
}

// ---- WebSocket ----
async fn ws_route(State(ctx): C, ConnectInfo(a): A, Query(q): Query<HashMap<String, String>>, ws: WebSocketUpgrade) -> Response {
    let loc = local(&a);
    let ok = loc || q.get("token").map(|t| ctx.st.lock().unwrap().devices.iter().any(|d| &d.token == t)).unwrap_or(false);
    if !ok {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    ws.on_upgrade(move |s| ws_conn(s, ctx, loc))
}

async fn ws_conn(sock: WebSocket, ctx: Arc<Ctx>, local: bool) {
    let (mut sink, mut stream) = sock.split();
    let mut rx = ctx.tx.subscribe();
    if sink.send(Message::Text(ctx.state_msg())).await.is_err() {
        return;
    }
    let o = json!({ "t": "obs", "obs": ctx.obs.state().phone() }).to_string();
    if sink.send(Message::Text(o)).await.is_err() {
        return;
    }
    loop {
        tokio::select! {
            m = stream.next() => match m {
                Some(Ok(Message::Text(t))) => handle_msg(&ctx, local, &t),
                Some(Ok(Message::Close(_))) | Some(Err(_)) | None => break,
                _ => {}
            },
            b = rx.recv() => match b {
                Ok(s) => { if sink.send(Message::Text(s)).await.is_err() { break; } }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {}
                Err(_) => break,
            }
        }
    }
}

fn clean_key(o: &Value) -> Key {
    const FIELDS: [&str; 14] = ["label", "color", "action", "keys", "target", "appName", "text", "key", "to", "vk", "mod", "file", "vol", "crt"];
    let mut r = Map::new();
    if let Some(m) = o.as_object() {
        for f in FIELDS {
            if let Some(Value::String(s)) = m.get(f) {
                r.insert(f.into(), json!(take(s, 400)));
            }
        }
        if let Some(Value::String(img)) = m.get("img") {
            if IMG_FIELD_RX.is_match(img) || (img.starts_with("data:image/") && img.len() < 400000) {
                r.insert("img".into(), json!(img));
            }
        }
    }
    r
}

fn idx(v: Option<&Value>) -> Option<i64> {
    match v? {
        Value::Number(n) => n.as_i64(),
        Value::String(s) => s.trim().parse().ok(),
        _ => None,
    }
}

fn handle_msg(ctx: &Arc<Ctx>, local: bool, raw: &str) {
    let m: Value = match serde_json::from_str(raw) {
        Ok(v) => v,
        Err(_) => return,
    };
    let t = m.get("t").and_then(|t| t.as_str()).unwrap_or("");
    let p = idx(m.get("p"));

    if t == "press" {
        let key = {
            let st = ctx.st.lock().unwrap();
            p.and_then(|p| st.pages().get(p as usize)).and_then(|pg| idx(m.get("k")).and_then(|k| pg.keys.get(&k.to_string()))).cloned()
        };
        if let Some(k) = key {
            input::run(ctx, &k, &format!("{}-{}", p.unwrap_or(0), idx(m.get("k")).unwrap_or(0)));
        }
        return;
    }
    if t == "vsend" {
        let vk = m.get("vk").map(|v| v.as_str().map(|s| s.to_string()).unwrap_or_else(|| v.to_string())).unwrap_or_default();
        let mut k = Key::new();
        k.insert("action".into(), json!("vkey"));
        k.insert("vk".into(), json!(vk));
        input::run(ctx, &k, "v");
        return;
    }
    if t == "prof" {
        let mut st = ctx.st.lock().unwrap();
        if let Some(i) = idx(m.get("i")) {
            if i >= 0 && (i as usize) < st.profiles.len() {
                st.cur = i as usize;
                ctx.save(&st);
                drop(st);
                ctx.broadcast_state();
            }
        }
        return;
    }
    if !local {
        return;
    }
    if t == "test" {
        let delay = m.get("delay").and_then(|d| d.as_f64()).unwrap_or(0.0).clamp(0.0, 10000.0) as u64;
        let key = clean_key(m.get("key").unwrap_or(&json!({})));
        let c2 = ctx.clone();
        ctx.rt.spawn(async move {
            tokio::time::sleep(Duration::from_millis(delay)).await;
            input::run(&c2, &key, "test");
        });
        return;
    }

    let changed = {
        let mut st = ctx.st.lock().unwrap();
        let max = (st.cols * st.rows) as i64;
        let np = st.pages().len();
        let pi = p.filter(|p| *p >= 0 && (*p as usize) < np).map(|p| p as usize);
        match t {
            "set" => match (pi, idx(m.get("k"))) {
                (Some(pi), Some(k)) if k >= 0 && k < max => {
                    st.pages_mut()[pi].keys.insert(k.to_string(), clean_key(m.get("key").unwrap_or(&json!({}))));
                    true
                }
                _ => false,
            },
            "del" => match pi {
                Some(pi) => {
                    if let Some(k) = idx(m.get("k")) {
                        st.pages_mut()[pi].keys.remove(&k.to_string());
                    }
                    true
                }
                None => false,
            },
            "move" => {
                let to = match m.get("to").and_then(|t| t.as_i64()) {
                    Some(t) => t,
                    None => return,
                };
                let k = idx(m.get("k")).map(|k| k.to_string());
                match (pi, k) {
                    (Some(pi), Some(k)) if to != pi as i64 && to >= 0 && (to as usize) < np && st.pages()[pi].keys.contains_key(&k) => {
                        // Pindahkan tombol ke slot kosong pertama di halaman tujuan
                        let to = to as usize;
                        let slot = (0..max).find(|i| !st.pages()[to].keys.contains_key(&i.to_string()));
                        match slot {
                            Some(j) => {
                                let key = st.pages_mut()[pi].keys.remove(&k).unwrap();
                                st.pages_mut()[to].keys.insert(j.to_string(), key);
                                true
                            }
                            None => return,
                        }
                    }
                    _ => false,
                }
            }
            "swap" => match (pi, idx(m.get("a")), idx(m.get("b"))) {
                (Some(pi), Some(a), Some(b)) if a >= 0 && b >= 0 && a < max && b < max => {
                    let pg = &mut st.pages_mut()[pi];
                    let (ka, kb) = (pg.keys.remove(&a.to_string()), pg.keys.remove(&b.to_string()));
                    if let Some(x) = kb {
                        pg.keys.insert(a.to_string(), x);
                    }
                    if let Some(x) = ka {
                        pg.keys.insert(b.to_string(), x);
                    }
                    true
                }
                _ => false,
            },
            "addPage" if np < 10 => {
                st.pages_mut().push(Page::default());
                true
            }
            "delPage" => match pi {
                Some(pi) if np > 1 => {
                    st.pages_mut().remove(pi);
                    true
                }
                _ => false,
            },
            "addProf" if st.profiles.len() < 12 => {
                let name = m.get("name").and_then(|n| n.as_str()).filter(|n| !n.is_empty()).map(|n| take(n, 40)).unwrap_or_else(|| "Profil".into());
                st.profiles.push(Profile { name, pages: vec![Page::default()] });
                st.cur = st.profiles.len() - 1;
                true
            }
            "renProf" => match idx(m.get("i")) {
                Some(i) if i >= 0 && (i as usize) < st.profiles.len() => {
                    let nm = take(m.get("name").and_then(|n| n.as_str()).unwrap_or(""), 40);
                    if !nm.is_empty() {
                        st.profiles[i as usize].name = nm;
                    }
                    true
                }
                _ => false,
            },
            "delProf" => match idx(m.get("i")) {
                Some(i) if st.profiles.len() > 1 && i >= 0 && (i as usize) < st.profiles.len() => {
                    st.profiles.remove(i as usize);
                    st.cur = st.cur.min(st.profiles.len() - 1);
                    true
                }
                _ => false,
            },
            _ => false,
        }
    };
    if changed {
        {
            let st = ctx.st.lock().unwrap();
            ctx.save(&st);
        }
        ctx.broadcast_state();
    }
}
