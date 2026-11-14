// Gatito: Stream Tool (PC), ditulis ulang dengan Rust. Editor (egui) + server untuk HP.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod app;
mod apps;
mod audio;
mod gui;
mod input;
mod obs;
mod server;
mod shell;
mod state;
mod update;
mod util;
mod winaudio;

use app::Ctx;
use std::sync::{Arc, Mutex};
use std::time::Duration;

fn read_port(dir: &std::path::Path) -> u16 {
    std::fs::read_to_string(dir.join("port.json"))
        .ok()
        .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
        .and_then(|j| j.get("port").and_then(|p| p.as_u64()))
        .filter(|p| (1024..=65535).contains(p))
        .map(|p| p as u16)
        .unwrap_or(3000)
}

fn main() {
    if std::env::var("GATITO_RELAUNCH").is_ok() {
        std::thread::sleep(Duration::from_millis(1500));
    }
    let base = dirs::config_dir().unwrap_or_else(std::env::temp_dir);
    let dir = base.join("Gatito Stream Tool");
    let _ = std::fs::create_dir_all(&dir);

    // Hanya satu Gatito yang boleh jalan. Yang kedua cukup menampilkan jendela yang pertama.
    let inst = single_instance::SingleInstance::new("com.streamdeckhp.gatito-stream-tool").ok();
    if let Some(i) = &inst {
        if !i.is_single() {
            use std::io::Write;
            if let Ok(mut s) = std::net::TcpStream::connect(("127.0.0.1", read_port(&dir))) {
                let _ = s.write_all(b"GET /__show HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n");
            }
            return;
        }
    }

    // Port server lokal: bawaan 3000, bisa diubah dari jendela Sambungkan HP (disimpan di port.json)
    let wanted = read_port(&dir);
    let mut port = wanted;
    if !util::port_free(port) {
        let old = port;
        let mut p = port + 1;
        while p < port + 50 && !util::port_free(p) {
            p += 1;
        }
        port = p;
        std::thread::spawn(move || {
            rfd::MessageDialog::new()
                .set_level(rfd::MessageLevel::Warning)
                .set_title("Gatito: Stream Tool")
                .set_description(format!(
                    "localhost:{old} telah digunakan program lain.\n\nGatito sementara memakai localhost:{p}. HP yang sudah tersambung perlu scan QR lagi. Untuk menetapkan port lain, buka tombol QR (Sambungkan HP) lalu ubah angka port-nya."
                ))
                .show();
        });
    }

    state::migrate(&base, &dir);
    let file = dir.join("deck2.json");

    let rt = tokio::runtime::Builder::new_multi_thread().enable_all().build().expect("tokio");
    let handle = rt.handle().clone();

    let (tx, _) = tokio::sync::broadcast::channel::<String>(64);
    let st = state::State::load(&file);
    let obs_cfg = st.obs.clone();
    let audio_cfg = st.audio.clone();
    let tx2 = tx.clone();
    let obs = obs::create(
        handle.clone(),
        Arc::new(move |s: obs::ObsState| {
            let _ = tx2.send(serde_json::json!({ "t": "obs", "obs": s.phone() }).to_string());
        }),
    );
    let engine = audio::Engine::start();
    let ctx = Arc::new(Ctx {
        file,
        dir: dir.clone(),
        st: Mutex::new(st),
        last_bak: Mutex::new(None),
        port,
        wanted,
        code: Mutex::new(app::new_code()),
        bad: Mutex::new(0),
        tx,
        obs: obs.clone(),
        engine: engine.clone(),
        vb: winaudio::Vb::new(),
        apps: Mutex::new(vec![]),
        up: Mutex::new(update::UpState::idle()),
        rt: handle.clone(),
    });
    shell::init(ctx.clone());

    engine.apply(audio_cfg, true);
    obs.configure(&obs_cfg);

    // Server HTTP + WebSocket
    {
        let c = ctx.clone();
        let listener = std::net::TcpListener::bind(("0.0.0.0", port)).expect("port");
        listener.set_nonblocking(true).ok();
        handle.spawn(async move {
            let l = tokio::net::TcpListener::from_std(listener).expect("listener");
            let app = server::router(c).into_make_service_with_connect_info::<std::net::SocketAddr>();
            let _ = axum::serve(l, app).await;
        });
    }

    // Setelah mesin suara siap, jadikan Virtual Mic sebagai mic default Windows (kalau opsinya aktif)
    {
        let c = ctx.clone();
        handle.spawn(async move {
            tokio::time::sleep(Duration::from_secs(3)).await;
            c.auto_mic_on().await;
        });
    }

    let _tray = shell::start_tray();
    let _keep = inst;
    gui::run(ctx, port);
    drop(rt);
}
