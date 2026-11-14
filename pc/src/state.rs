// Penyimpanan profil, halaman, tombol, dan pengaturan (deck2.json), sama dengan versi Electron
use crate::obs::{self, ObsCfg};
use crate::util::take;
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

pub type Key = Map<String, Value>;

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct Page {
    #[serde(default)]
    pub keys: HashMap<String, Key>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Profile {
    pub name: String,
    #[serde(default)]
    pub pages: Vec<Page>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Device {
    pub name: String,
    pub token: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct DevSel {
    pub id: String,
    pub label: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AudioCfg {
    pub virt: DevSel,
    pub mon: DevSel,
    pub mic: DevSel,
    #[serde(rename = "vVol")]
    pub v_vol: u32,
    #[serde(rename = "mVol")]
    pub m_vol: u32,
    #[serde(rename = "micVol")]
    pub mic_vol: u32,
    pub auto: u8,
}

impl AudioCfg {
    pub fn default_cfg() -> AudioCfg {
        AudioCfg {
            virt: DevSel { id: "".into(), label: "".into() },
            mon: DevSel { id: "default".into(), label: "".into() },
            mic: DevSel { id: "".into(), label: "".into() },
            v_vol: 100,
            m_vol: 100,
            mic_vol: 100,
            auto: 1,
        }
    }
}

/// id: '' = otomatis, 'default' = default Windows, 'off' = mati, 'none' = jangan kirim ke teman
pub fn clean_audio(o: Option<&Value>) -> AudioCfg {
    let d = AudioCfg::default_cfg();
    let empty = Map::new();
    let o = o.and_then(|v| v.as_object()).unwrap_or(&empty);
    let dv = |x: Option<&Value>, def: &DevSel| -> DevSel {
        match x {
            Some(Value::Object(m)) => DevSel {
                id: m.get("id").and_then(|v| v.as_str()).map(|s| take(s, 300)).unwrap_or_else(|| def.id.clone()),
                label: m.get("label").and_then(|v| v.as_str()).map(|s| take(s, 300)).unwrap_or_default(),
            },
            _ => def.clone(),
        }
    };
    let vl = |n: Option<&Value>, def: u32, max: u32| -> u32 {
        let f = match n {
            Some(Value::Number(x)) => x.as_f64(),
            Some(Value::String(s)) if !s.is_empty() => s.trim().parse::<f64>().ok(),
            Some(Value::Bool(b)) => Some(if *b { 1.0 } else { 0.0 }),
            _ => None,
        };
        match f {
            Some(v) if v.is_finite() => v.round().clamp(0.0, max as f64) as u32,
            _ => def,
        }
    };
    let auto = match o.get("auto") {
        Some(Value::Number(n)) if n.as_f64() == Some(0.0) => 0,
        Some(Value::Bool(false)) => 0,
        Some(Value::String(s)) if s == "0" => 0,
        _ => 1,
    };
    AudioCfg {
        virt: dv(o.get("virt"), &d.virt),
        mon: dv(o.get("mon"), &d.mon),
        mic: dv(o.get("mic"), &d.mic),
        v_vol: vl(o.get("vVol"), 100, 200),
        m_vol: vl(o.get("mVol"), 100, 100),
        mic_vol: vl(o.get("micVol"), 100, 200),
        auto,
    }
}

#[derive(Clone, Debug)]
pub struct State {
    pub cols: u32,
    pub rows: u32,
    pub devices: Vec<Device>,
    pub profiles: Vec<Profile>,
    pub cur: usize,
    pub audio: AudioCfg,
    pub obs: ObsCfg,
    pub real_mic: Option<String>,
}

impl State {
    pub fn default_state() -> State {
        State {
            cols: 5,
            rows: 3,
            devices: vec![],
            profiles: vec![Profile { name: "Default Profile".into(), pages: vec![Page::default()] }],
            cur: 0,
            audio: AudioCfg::default_cfg(),
            obs: obs::clean(None, None),
            real_mic: None,
        }
    }

    fn from_value(mut v: Value) -> Option<State> {
        let obj = v.as_object_mut()?;
        // Format lama: tanpa profil, hanya "pages"
        if !obj.contains_key("profiles") {
            let pages = obj.remove("pages").unwrap_or_else(|| json!([{ "keys": {} }]));
            obj.insert("profiles".into(), json!([{ "name": "Default Profile", "pages": pages }]));
            obj.insert("cur".into(), json!(0));
        }
        let mut profiles: Vec<Profile> = serde_json::from_value(obj.get("profiles")?.clone()).ok()?;
        if profiles.is_empty() {
            profiles.push(Profile { name: "Default Profile".into(), pages: vec![] });
        }
        for p in profiles.iter_mut() {
            if p.pages.is_empty() {
                p.pages.push(Page::default());
            }
        }
        let cur = obj.get("cur").and_then(|c| c.as_u64()).unwrap_or(0) as usize;
        let devices: Vec<Device> = obj.get("devices").and_then(|d| serde_json::from_value(d.clone()).ok()).unwrap_or_default();
        Some(State {
            cols: obj.get("cols").and_then(|c| c.as_u64()).unwrap_or(5).max(1) as u32,
            rows: obj.get("rows").and_then(|c| c.as_u64()).unwrap_or(3).max(1) as u32,
            devices,
            cur: cur.min(profiles.len() - 1),
            profiles,
            audio: clean_audio(obj.get("audio")),
            obs: obs::clean(obj.get("obs"), None),
            real_mic: obj.get("realMic").and_then(|r| r.as_str()).map(|s| s.to_string()),
        })
    }

    pub fn load(file: &Path) -> State {
        let try_read = |p: &Path| -> Option<State> {
            let txt = fs::read_to_string(p).ok()?;
            let v: Value = serde_json::from_str(txt.trim_start_matches('\u{feff}')).ok()?;
            State::from_value(v)
        };
        let bak = file.with_file_name(format!("{}.bak", file.file_name().and_then(|n| n.to_str()).unwrap_or("deck2.json")));
        try_read(file).or_else(|| try_read(&bak)).unwrap_or_else(State::default_state)
    }

    pub fn to_json(&self) -> Value {
        let mut m = Map::new();
        m.insert("cols".into(), json!(self.cols));
        m.insert("rows".into(), json!(self.rows));
        m.insert("devices".into(), serde_json::to_value(&self.devices).unwrap_or(json!([])));
        m.insert("profiles".into(), serde_json::to_value(&self.profiles).unwrap_or(json!([])));
        m.insert("cur".into(), json!(self.cur));
        m.insert("audio".into(), serde_json::to_value(&self.audio).unwrap_or(json!({})));
        m.insert("obs".into(), serde_json::to_value(&self.obs).unwrap_or(json!({})));
        if let Some(r) = &self.real_mic {
            m.insert("realMic".into(), json!(r));
        }
        Value::Object(m)
    }

    pub fn pages(&self) -> &Vec<Page> {
        &self.profiles[self.cur].pages
    }
    pub fn pages_mut(&mut self) -> &mut Vec<Page> {
        let c = self.cur;
        &mut self.profiles[c].pages
    }

    /// Tombol virtual yang dipakai di semua profil
    pub fn used_vk(&self) -> Vec<Value> {
        let mut out = vec![];
        for (pi, pr) in self.profiles.iter().enumerate() {
            for (pgi, pg) in pr.pages.iter().enumerate() {
                let mut es: Vec<(&String, &Key)> = pg.keys.iter().collect();
                es.sort_by_key(|(k, _)| k.parse::<i64>().unwrap_or(0));
                for (ki, k) in es {
                    if k.get("action").and_then(|a| a.as_str()) == Some("vkey") {
                        let s = |n: &str| k.get(n).and_then(|v| v.as_str()).unwrap_or("").to_string();
                        out.push(json!({
                            "vk": k.get("vk").cloned().unwrap_or(Value::Null),
                            "label": s("label"), "img": s("img"), "color": s("color"),
                            "prof": pr.name, "page": pgi + 1, "pi": pi, "pg": pgi, "ki": ki.parse::<i64>().unwrap_or(0)
                        }));
                    }
                }
            }
        }
        out
    }
}

/// Pulihkan profil dan tombol dari folder versi lama kalau folder baru masih kosong atau hanya berisi contoh bawaan
pub fn migrate(base: &Path, dir: &Path) {
    let file = dir.join("deck2.json");
    struct Info {
        n: usize,
        sample: bool,
        t: std::time::SystemTime,
    }
    let info = |f: &Path| -> Option<Info> {
        let txt = fs::read_to_string(f).ok()?;
        let j: Value = serde_json::from_str(&txt).ok()?;
        let pages: Vec<Value> = if let Some(ps) = j.get("profiles").and_then(|p| p.as_array()) {
            ps.iter().flat_map(|p| p.get("pages").and_then(|x| x.as_array()).cloned().unwrap_or_default()).collect()
        } else {
            j.get("pages").and_then(|p| p.as_array()).cloned().unwrap_or_default()
        };
        let n: usize = pages.iter().map(|pg| pg.get("keys").and_then(|k| k.as_object()).map(|o| o.len()).unwrap_or(0)).sum();
        let t = fs::metadata(f).and_then(|m| m.modified()).unwrap_or(std::time::UNIX_EPOCH);
        Some(Info { n, sample: n == 14 && txt.contains("\"Mulai Stream\""), t })
    };
    if let Some(cur) = info(&file) {
        if cur.n > 0 && !cur.sample {
            return;
        }
    }
    let mut best: Option<(std::time::SystemTime, std::path::PathBuf, &str)> = None;
    for nm in ["StreamDeck HP", "streamdeck-hp", "Gatito Stream Tool", "gatito-stream-tool", "Gatito"] {
        let d = base.join(nm);
        if d == dir {
            continue;
        }
        for fnm in ["deck2.json", "deck2.json.bak"] {
            if let Some(i) = info(&d.join(fnm)) {
                if i.n > 0 && !i.sample && best.as_ref().map(|b| i.t > b.0).unwrap_or(true) {
                    best = Some((i.t, d.clone(), fnm));
                }
            }
        }
    }
    if let Some((_, d, fnm)) = best {
        let _ = fs::create_dir_all(dir);
        let _ = fs::copy(d.join(fnm), &file);
        let im = d.join("images");
        if im.exists() {
            let _ = copy_dir(&im, &dir.join("images"));
        }
    }
}

fn copy_dir(from: &Path, to: &Path) -> std::io::Result<()> {
    fs::create_dir_all(to)?;
    for e in fs::read_dir(from)? {
        let e = e?;
        let p = e.path();
        let t = to.join(e.file_name());
        if p.is_dir() {
            copy_dir(&p, &t)?;
        } else {
            fs::copy(&p, &t)?;
        }
    }
    Ok(())
}
