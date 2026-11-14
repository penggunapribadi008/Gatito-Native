// Keadaan bersama seluruh aplikasi (pengganti variabel global di server.js)
use crate::apps::AppEntry;
use crate::audio::Engine;
use crate::obs::Obs;
use crate::state::State;
use crate::update::UpState;
use crate::winaudio::Vb;
use rand::Rng;
use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::sync::broadcast;

pub struct Ctx {
    pub file: PathBuf,
    pub dir: PathBuf,
    pub st: Mutex<State>,
    pub last_bak: Mutex<Option<Instant>>,
    pub port: u16,
    pub wanted: u16,
    pub code: Mutex<String>,
    pub bad: Mutex<u32>,
    pub tx: broadcast::Sender<String>,
    pub obs: Obs,
    pub engine: Engine,
    pub vb: Vb,
    pub apps: Mutex<Vec<AppEntry>>,
    pub up: Mutex<UpState>,
    pub rt: tokio::runtime::Handle,
}

pub fn new_code() -> String {
    rand::thread_rng().gen_range(100000..999999).to_string()
}

impl Ctx {
    pub fn save(&self, st: &State) {
        let mut lb = self.last_bak.lock().unwrap();
        if self.file.exists() && lb.map(|t| t.elapsed() > Duration::from_secs(60)).unwrap_or(true) {
            let mut bak = self.file.clone().into_os_string();
            bak.push(".bak");
            let _ = std::fs::copy(&self.file, PathBuf::from(bak));
            *lb = Some(Instant::now());
        }
        let _ = std::fs::write(&self.file, st.to_json().to_string());
    }

    pub fn pub_state(&self, st: &State) -> Value {
        json!({
            "cols": st.cols, "rows": st.rows,
            "pages": st.pages(),
            "profiles": st.profiles.iter().map(|p| p.name.clone()).collect::<Vec<_>>(),
            "cur": st.cur, "usedVk": st.used_vk(), "ips": crate::util::ips()
        })
    }

    pub fn state_msg(&self) -> String {
        let st = self.st.lock().unwrap();
        json!({ "t": "state", "state": self.pub_state(&st) }).to_string()
    }

    pub fn broadcast_state(&self) {
        let _ = self.tx.send(self.state_msg());
    }

    pub fn send_ui(&self, c: crate::shell::UiCmd) {
        crate::shell::cmd(c);
    }

    /// Mic otomatis: saat Gatito dibuka Virtual Mic jadi mic default Windows
    pub async fn auto_mic_on(self: &Arc<Self>) {
        if !cfg!(windows) || self.st.lock().unwrap().audio.auto == 0 {
            return;
        }
        let r = self.vb.use_virtual().await;
        let real = r.as_ref().and_then(|j| j.get("real")).and_then(|x| x.as_str()).unwrap_or("").to_string();
        if real.is_empty() {
            return;
        }
        let mut cfg_to_apply = None;
        {
            let mut st = self.st.lock().unwrap();
            let mut ch = false;
            if st.real_mic.as_deref() != Some(real.as_str()) {
                st.real_mic = Some(real.clone());
                ch = true;
            }
            // Kunci mic asli supaya mesin suara tetap memakainya walau default Windows sudah Virtual Mic
            if st.audio.mic.id.is_empty() && st.audio.mic.label.is_empty() {
                st.audio.mic = crate::state::DevSel { id: "".into(), label: real };
                ch = true;
                cfg_to_apply = Some(st.audio.clone());
            }
            if ch {
                self.save(&st);
            }
        }
        if let Some(c) = cfg_to_apply {
            self.engine.apply(c, true);
        }
    }

    /// Saat Gatito ditutup: kembalikan mic asli
    pub async fn auto_mic_off(self: &Arc<Self>) {
        if !cfg!(windows) || self.st.lock().unwrap().audio.auto == 0 {
            return;
        }
        let (mut real, ) = {
            let st = self.st.lock().unwrap();
            (if !st.audio.mic.label.is_empty() { st.audio.mic.label.clone() } else { st.real_mic.clone().unwrap_or_default() },)
        };
        if real.is_empty() {
            if let Some(inf) = self.engine.info(2500).await {
                let m = inf.get("mic").and_then(|x| x.as_str()).unwrap_or("");
                if !m.starts_with("Mati") && !m.starts_with("Belum") && !m.starts_with("Gagal") && !m.starts_with("Tidak") {
                    real = m.to_string();
                }
            }
        }
        if !real.is_empty() {
            let _ = self.vb.restore_if_virtual(&real).await;
        }
    }
}

/// Mulai ulang aplikasi (dipakai setelah port diganti)
pub fn relaunch() {
    if let Ok(exe) = std::env::current_exe() {
        let _ = std::process::Command::new(exe).env("GATITO_RELAUNCH", "1").spawn();
    }
    std::process::exit(0);
}
