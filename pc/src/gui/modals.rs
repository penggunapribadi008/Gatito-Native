// Dialog: Suara dan mic virtual, Pantau OBS, Sambungkan HP (QR + port), Port yang dihindari,
// Tombol virtual, nama profil, dan konfirmasi. Isi dan teksnya mengikuti editor.html.
use super::widgets::*;
use super::*;

fn s_of(v: &Value, k: &str) -> String {
    v.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string()
}
fn b_of(v: &Value, k: &str) -> bool {
    match v.get(k) {
        Some(Value::Bool(b)) => *b,
        Some(Value::Number(n)) => n.as_f64().map(|x| x != 0.0).unwrap_or(false),
        _ => false,
    }
}
fn hms(ms: i64) -> String {
    let s = ms / 1000;
    format!("{:02}:{:02}:{:02}", s / 3600, (s / 60) % 60, s % 60)
}
fn tidy(l: &str) -> String {
    use once_cell::sync::Lazy;
    use regex::Regex;
    static A: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?i)\s*\((?:VB-Audio Virtual Cable|VB-Output)\)").unwrap());
    static B: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?i)\s*\([0-9a-f]{4}:[0-9a-f]{4}\)").unwrap());
    B.replace_all(&A.replace_all(l, ""), "").trim().to_string()
}
fn real_mic(s: &str) -> String {
    if !s.is_empty() && !["Mati", "Belum", "Gagal", "Tidak"].iter().any(|p| s.starts_with(p)) {
        s.to_string()
    } else {
        String::new()
    }
}

// ================================================================ keadaan tiap dialog
#[derive(Default)]
pub struct AudioM {
    pub open: bool,
    pub cfg: Option<Value>,
    pub info: Option<Value>,
    pub inst: Option<Value>,
    pub lmic: f32,
    pub lout: f32,
    pub last_level: Option<Instant>,
    pub inst_poll: bool,
    pub last_inst: Option<Instant>,
    pub sl: HashMap<String, f32>,
}

#[derive(Default)]
pub struct ObsM {
    pub open: bool,
    pub on: bool,
    pub host: String,
    pub port: String,
    pub pass: String,
    pub hint: bool,
    pub status: Value,
    pub last: Option<Instant>,
}

#[derive(Default)]
pub struct PairM {
    pub open: bool,
    pub code: String,
    pub ips: Vec<String>,
    pub port_txt: String,
    pub wanted: u64,
    pub port: u64,
    pub dirty: bool,
    pub busy: bool,
    pub pending: bool,
    pub msg: (String, String),
    pub restart: bool,
    pub last: Option<Instant>,
    pub qr: Option<(usize, Vec<bool>)>,
}

#[derive(Default)]
pub struct AvoidM {
    pub open: bool,
    pub list: Vec<(u64, String)>,
}

pub struct VkM {
    pub open: bool,
    pub m: String,
    pub busy: bool,
    pub sent: Option<(String, Instant)>,
    pub done: Option<(String, Instant)>,
}
impl VkM {
    pub fn new() -> VkM {
        VkM { open: false, m: "F".into(), busy: false, sent: None, done: None }
    }
}

const VHINT: &str = "Tekan sebuah tombol, lalu dalam 5 detik klik kotak hotkey di OBS (atau aplikasi lain). Tombol dikirim setelah hitungan selesai.";

impl App {
    // ================================================================ Suara
    pub fn open_audio(&mut self) {
        self.audio.open = true;
        self.audio.info = None;
        self.be.get("/audio/install/status", |a, s| {
            if s.get("running").and_then(|r| r.as_bool()).unwrap_or(false) {
                a.audio.inst = Some(s);
                a.audio.inst_poll = true;
            } else if a.audio.inst.is_none() && !s_of(&s, "msg").is_empty() {
                a.audio.inst = Some(s);
            }
        });
        self.a_load();
    }
    pub fn close_audio(&mut self) {
        self.audio.open = false;
        self.audio.inst_poll = false;
    }
    fn a_load(&mut self) {
        self.be.get("/audio/info", |a, v| {
            if v.is_null() {
                a.audio.info = Some(json!({ "ready": false }));
            } else {
                a.audio.cfg = v.get("cfg").cloned();
                a.audio.info = Some(v);
            }
        });
    }
    fn a_save(&mut self, key: &str, val: Value) {
        if let Some(Value::Object(c)) = &mut self.audio.cfg {
            c.insert(key.to_string(), val);
        }
        if let Some(c) = self.audio.cfg.clone() {
            self.be.post("/audio/settings", Some(c), |a, _| a.a_load());
        }
    }
    fn a_install(&mut self, install: bool) {
        self.audio.inst = Some(json!({ "running": true, "msg": "Memulai…", "notes": [] }));
        self.be.post("/audio/install", Some(json!({ "install": install })), |a, _| a.audio.inst_poll = true);
    }

    fn inst_lines(ui: &mut Ui, inst: &Option<Value>) {
        if let Some(s) = inst {
            let msg = s_of(s, "msg");
            if msg.is_empty() {
                return;
            }
            let c = if !s_of(s, "err").is_empty() { RED } else if b_of(s, "running") { ORANGE } else { DOT_OK };
            dot_text(ui, c, &msg);
            for n in s.get("notes").and_then(|n| n.as_array()).cloned().unwrap_or_default() {
                dot_text(ui, ORANGE, n.as_str().unwrap_or(""));
            }
        }
    }

    fn draw_audio(&mut self, ctx: &Context) {
        let sh = ctx.screen_rect().height();
        let inst = self.audio.inst.clone();
        let info = self.audio.info.clone();
        let cfg = self.audio.cfg.clone();
        let (lmic, lout) = (self.audio.lmic, self.audio.lout);
        let mut act: Vec<(String, Value)> = vec![];
        let mut do_install: Option<bool> = None;
        let mut do_restore = false;
        let (mut refresh, mut test, mut close) = (false, false, false);
        let mut sliders = self.audio.sl.clone();
        let (outside, _) = modal(ctx, "amodal", 560.0, false, |ui| {
            modal_title(ui, "Suara dan mic virtual");
            ui.add_space(6.0);
            ScrollArea::vertical().max_height((sh * 0.92 - 150.0).max(200.0)).auto_shrink([false, true]).show(ui, |ui| {
                Self::inst_lines(ui, &inst);
                let i = match &info {
                    Some(i) if b_of(i, "ready") => i.clone(),
                    Some(_) => {
                        dot_text(ui, ORANGE, "Mesin suara belum siap. Tunggu beberapa detik lalu klik Segarkan.");
                        return;
                    }
                    None => {
                        dot_text(ui, MUTED, "Memeriksa perangkat suara…");
                        return;
                    }
                };
                let c = match &cfg {
                    Some(c) => c.clone(),
                    None => return,
                };
                let virt_id = c["virt"]["id"].as_str().unwrap_or("").to_string();
                if !b_of(&i, "cableFound") {
                    if virt_id == "none" {
                        dot_text(ui, ORANGE, "Kirim ke teman dimatikan, jadi hanya kamu yang mendengar soundboard.");
                    } else {
                        dot_line(ui, RED, |ui| {
                            ui.add(Label::new(RichText::new("VB-Cable belum terpasang, teman belum bisa mendengar soundboard.").font(reg(13.0))).wrap());
                            if Btn::sb("Pasang otomatis").show(ui).clicked() {
                                do_install = Some(true);
                            }
                            ui.end_row();
                            ui.label(RichText::new("Gratis dari vb-audio.com. Saat Windows meminta izin, klik Ya dan Install.").font(reg(12.0)).color(MUTED));
                        });
                    }
                } else if b_of(&i, "defInVirt") {
                    dot_line(ui, DOT_OK, |ui| {
                        ui.label(RichText::new("Mic virtual aktif sebagai mic default").font(reg(13.0)));
                        if Btn::sb("Kembalikan ke mic asli").show(ui).clicked() {
                            do_restore = true;
                        }
                    });
                } else {
                    dot_line(ui, ORANGE, |ui| {
                        ui.label(RichText::new("Mic virtual aktif, belum jadi mic default.").font(reg(13.0)));
                        if Btn::sb("Atur otomatis").show(ui).clicked() {
                            do_install = Some(false);
                        }
                    });
                }
                let mic_s = s_of(&i, "mic");
                if b_of(&i, "cableFound") && !mic_s.is_empty() && real_mic(&mic_s).is_empty() {
                    dot_text(ui, ORANGE, &format!("Mic asli: {}", mic_s));
                }
                for w in i["warns"].as_array().cloned().unwrap_or_default() {
                    dot_text(ui, ORANGE, w.as_str().unwrap_or(""));
                }
                for w in i["errs"].as_array().cloned().unwrap_or_default() {
                    dot_text(ui, RED, w.as_str().unwrap_or(""));
                }
                if !s_of(&i, "permErr").is_empty() {
                    dot_text(ui, ORANGE, &format!("Nama perangkat tidak terbaca ({}). Cek izin mikrofon Windows untuk aplikasi desktop.", s_of(&i, "permErr")));
                }
                let outs: Vec<Value> = i["out"].as_array().cloned().unwrap_or_default();
                let inps: Vec<Value> = i["inp"].as_array().cloned().unwrap_or_default();
                let mic_cfg_id = c["mic"]["id"].as_str().unwrap_or("").to_string();
                let mon_id = { let m = c["mon"]["id"].as_str().unwrap_or(""); if m.is_empty() { "default".to_string() } else { m.to_string() } };
                let ic = if mic_cfg_id == "off" { "off".to_string() } else if !s_of(&i, "micId").is_empty() { s_of(&i, "micId") } else { mic_cfg_id };
                ui.add_space(6.0);
                let mut auto = c["auto"].as_i64().unwrap_or(1) != 0 && !matches!(c["auto"], Value::Bool(false));
                ui.horizontal_top(|ui| {
                    if ui.checkbox(&mut auto, "").changed() {
                        act.push(("auto".into(), json!(if auto { 1 } else { 0 })));
                    }
                    ui.vertical(|ui| {
                        ui.spacing_mut().item_spacing.y = 0.0;
                        ui.label(RichText::new("Ganti mic Windows otomatis saat Gatito dibuka dan ditutup").font(reg(12.5)));
                        ui.label(RichText::new("Di aplikasi lain, atur input mic ke Default agar ikut berpindah.").font(reg(12.5)).color(MUTED));
                    });
                });
                ui.add_space(12.0);
                let lab = |list: &Vec<Value>, id: &str| list.iter().find(|d| d["id"].as_str() == Some(id)).and_then(|d| d["label"].as_str()).unwrap_or("").to_string();
                let pickv = |v: &str, list: &Vec<Value>| -> Value {
                    json!({ "id": v, "label": if ["", "default", "off", "none"].contains(&v) { String::new() } else { lab(list, v) } })
                };
                let colw = (ui.available_width() - 16.0) / 2.0;
                ui.horizontal_top(|ui| {
                    ui.spacing_mut().item_spacing.x = 16.0;
                    labeled(ui, &format!("Mic virtual ({})", if b_of(&i, "cableFound") { tidy(&s_of(&i, "cable")) } else { "belum terpasang".into() }), |ui| {
                        let mut o: Vec<(String, String)> = vec![("".into(), "Otomatis (rekomendasi)".into())];
                        o.extend(outs.iter().map(|d| (s_of(d, "id"), s_of(d, "label"))));
                        o.push(("none".into(), "Jangan kirim ke teman".into()));
                        if let Some(v) = select(ui, "svirt", colw, &virt_id, &o) {
                            act.push(("virt".into(), pickv(&v, &outs)));
                        }
                    });
                    labeled(ui, "Speaker", |ui| {
                        let mut o: Vec<(String, String)> = vec![("default".into(), "Default Windows".into())];
                        o.extend(outs.iter().filter(|d| !b_of(d, "v")).map(|d| (s_of(d, "id"), s_of(d, "label"))));
                        o.push(("off".into(), "Mati (aku tidak mendengar)".into()));
                        if let Some(v) = select(ui, "smon", colw, &mon_id, &o) {
                            act.push(("mon".into(), pickv(&v, &outs)));
                        }
                    });
                });
                ui.add_space(12.0);
                ui.horizontal_top(|ui| {
                    ui.spacing_mut().item_spacing.x = 16.0;
                    let t = tidy(&real_mic(&mic_s));
                    labeled(ui, &format!("Mic ({})", if t.is_empty() { "-".to_string() } else { t }), |ui| {
                        let mut o: Vec<(String, String)> = vec![("".into(), "Otomatis (rekomendasi)".into())];
                        o.extend(inps.iter().filter(|d| !b_of(d, "v")).map(|d| (s_of(d, "id"), s_of(d, "label"))));
                        o.push(("off".into(), "Mati (hanya soundboard)".into()));
                        if let Some(v) = select(ui, "smic", colw, &ic, &o) {
                            act.push(("mic".into(), pickv(&v, &inps)));
                        }
                    });
                    let mut sld = |ui: &mut Ui, id: &str, label: &str, key: &str, max: f32| {
                        labeled(ui, label, |ui| {
                            ui.horizontal(|ui| {
                                let cur = c[key].as_f64().unwrap_or(100.0) as f32;
                                let mut v = *sliders.get(id).unwrap_or(&cur);
                                let (ch, rel) = slider(ui, &mut v, 0.0, max, colw - 50.0);
                                ui.add_sized([38.0, 20.0], Label::new(RichText::new(format!("{}%", v as i32)).font(reg(12.0)).color(LIGHT)));
                                if ch {
                                    sliders.insert(id.to_string(), v);
                                }
                                if rel {
                                    sliders.remove(id);
                                    act.push((key.to_string(), json!(v as i64)));
                                }
                            });
                        });
                    };
                    sld(ui, "cv", "Volume mic", "micVol", 200.0);
                });
                ui.add_space(12.0);
                ui.horizontal_top(|ui| {
                    ui.spacing_mut().item_spacing.x = 16.0;
                    let mut sld = |ui: &mut Ui, id: &str, label: &str, key: &str, max: f32| {
                        labeled(ui, label, |ui| {
                            ui.horizontal(|ui| {
                                let cur = c[key].as_f64().unwrap_or(100.0) as f32;
                                let mut v = *sliders.get(id).unwrap_or(&cur);
                                let (ch, rel) = slider(ui, &mut v, 0.0, max, colw - 50.0);
                                ui.add_sized([38.0, 20.0], Label::new(RichText::new(format!("{}%", v as i32)).font(reg(12.0)).color(LIGHT)));
                                if ch {
                                    sliders.insert(id.to_string(), v);
                                }
                                if rel {
                                    sliders.remove(id);
                                    act.push((key.to_string(), json!(v as i64)));
                                }
                            });
                        });
                    };
                    sld(ui, "vv", "Volume ke teman", "vVol", 200.0);
                    sld(ui, "mv", "Volume di speakerku", "mVol", 100.0);
                });
                ui.add_space(12.0);
                let meter = |ui: &mut Ui, v: f32| {
                    let (r, _) = ui.allocate_exact_size(vec2(colw, 8.0), Sense::hover());
                    ui.painter().rect_filled(r, 4.0, HB);
                    let f = (((20.0 * v.max(1e-6).log10()) + 60.0) / 60.0 * 100.0).clamp(0.0, 100.0) / 100.0;
                    ui.painter().rect_filled(Rect::from_min_size(r.min, vec2(r.width() * f, 8.0)), 4.0, DOT_OK);
                };
                ui.horizontal_top(|ui| {
                    ui.spacing_mut().item_spacing.x = 16.0;
                    labeled(ui, "Level mic", |ui| meter(ui, lmic));
                    labeled(ui, "Level mic virtual", |ui| meter(ui, lout));
                });
            });
            ui.add_space(12.0);
            rrow(ui, |ui| {
                ui.spacing_mut().item_spacing.x = 8.0;
                if Btn::b("Tutup").fill(ACCENT).show(ui).clicked() {
                    close = true;
                }
                if Btn::b("Tes suara").show(ui).clicked() {
                    test = true;
                }
                if Btn::b("Segarkan").show(ui).clicked() {
                    refresh = true;
                }
            });
        });
        self.audio.sl = sliders;
        for (k, v) in act {
            self.a_save(&k, v);
        }
        if let Some(i) = do_install {
            self.a_install(i);
        }
        if do_restore {
            self.be.post("/audio/restoremic", None, |a, r| {
                let ok = b_of(&r, "ok");
                let err = if s_of(&r, "err").is_empty() { "Gagal.".to_string() } else { s_of(&r, "err") };
                a.audio.inst = Some(json!({ "running": false, "msg": if ok { "Mic default Windows dikembalikan ke mic asli.".to_string() } else { err.clone() }, "err": if ok { String::new() } else { err }, "notes": [] }));
                a.a_load();
            });
        }
        if refresh {
            self.audio.info = None;
            self.be.post("/audio/refresh", None, |a, _| a.a_load());
        }
        if test {
            self.be.post_quiet("/audio/test");
        }
        if close || outside {
            self.close_audio();
        }
    }

    // ================================================================ OBS
    pub fn open_obs(&mut self) {
        self.obs.open = true;
        self.obs.pass.clear();
        self.be.get("/obs/settings", |a, c| {
            a.obs.on = b_of(&c, "on");
            a.obs.host = s_of(&c, "host");
            a.obs.port = c.get("port").map(|p| p.to_string()).unwrap_or_default();
            a.obs.hint = b_of(&c, "hasPass");
        });
        self.o_stat();
    }
    pub fn close_obs(&mut self) {
        self.obs.open = false;
    }
    fn o_stat(&mut self) {
        self.obs.last = Some(Instant::now());
        self.be.get("/obs/status", |a, s| {
            if !s.is_null() {
                a.obs.status = s;
            }
        });
    }
    fn draw_obs(&mut self, ctx: &Context) {
        let status = self.obs.status.clone();
        let (mut on, mut host, mut port, mut pass) = (self.obs.on, self.obs.host.clone(), self.obs.port.clone(), self.obs.pass.clone());
        let hint = self.obs.hint;
        let (mut save, mut close) = (false, false);
        let (outside, _) = modal(ctx, "omodal", 420.0, false, |ui| {
            ui.horizontal(|ui| {
                Image::from_bytes("bytes://obs-logo.png", crate::server::OBS_LOGO).fit_to_exact_size(vec2(28.0, 28.0)).ui(ui);
                ui.label(RichText::new("Pantau OBS di HP").font(bold(18.0)));
            });
            ui.add_space(6.0);
            for (n, parts) in [
                (1, vec![("Di OBS buka ", false), ("Tools > WebSocket Server Settings", true)]),
                (2, vec![("Centang ", false), ("Enable WebSocket server", true), (" (OBS 28 ke atas)", false)]),
                (3, vec![("Isi password di bawah kalau OBS memakainya", false)]),
            ] {
                ui.horizontal_top(|ui| {
                    let (r, _) = ui.allocate_exact_size(vec2(20.0, 20.0), Sense::hover());
                    ui.painter().circle_filled(r.center(), 10.0, B_BG);
                    ui.painter().text(r.center(), Align2::CENTER_CENTER, n.to_string(), bold(11.0), TEXT);
                    ui.horizontal_wrapped(|ui| {
                        ui.spacing_mut().item_spacing.x = 0.0;
                        for (t, b) in &parts {
                            ui.label(RichText::new(*t).font(if *b { bold(13.0) } else { reg(13.0) }).color(LIGHT));
                        }
                    });
                });
            }
            ui.add_space(8.0);
            ui.checkbox(&mut on, "Aktifkan pemantauan OBS");
            ui.add_space(8.0);
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing.x = 8.0;
                labeled(ui, "Alamat OBS", |ui| {
                    text_input(ui, Id::new("ohost"), &mut host, 220.0, "127.0.0.1", false, false);
                });
                labeled(ui, "Port", |ui| {
                    text_input(ui, Id::new("oport"), &mut port, 96.0, "4455", false, false);
                    port.retain(|c| c.is_ascii_digit());
                });
            });
            ui.add_space(8.0);
            labeled(ui, "Password OBS", |ui| {
                text_input(ui, Id::new("opass"), &mut pass, ui.available_width(), if hint { "(tersimpan, ketik untuk mengganti)" } else { "" }, true, false);
            });
            ui.add_space(10.0);
            let link_s = s_of(&status, "link");
            if link_s == "off" || link_s.is_empty() {
                dot_text(ui, MUTED, "Pemantauan belum diaktifkan.");
            } else if link_s == "ok" {
                dot_text(ui, DOT_OK, "Terhubung ke OBS");
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing.x = 16.0;
                    let live = b_of(&status, "live");
                    let t = format!("Live: {}", if live { format!("{}{}", if b_of(&status, "recon") { "menyambung ulang " } else { "berjalan " }, hms(status["liveMs"].as_i64().unwrap_or(0))) } else { "belum mulai".into() });
                    dot_text(ui, if live { RED } else { MUTED }, &t);
                    let rec = b_of(&status, "rec");
                    let t = format!("Rekaman: {}", if rec { format!("{}{}", if b_of(&status, "paused") { "dijeda " } else { "berjalan " }, hms(status["recMs"].as_i64().unwrap_or(0))) } else { "belum mulai".into() });
                    dot_text(ui, if rec { RED } else { MUTED }, &t);
                });
            } else {
                dot_text(ui, if link_s == "auth" { RED } else { ORANGE }, &s_of(&status, "msg"));
            }
            if link_s == "ok" {
                if let Some(outs) = status["outs"].as_array() {
                    if !outs.is_empty() {
                        ui.label(RichText::new("Output yang terbaca di OBS:").font(reg(12.0)).color(MUTED));
                        ScrollArea::vertical().max_height(150.0).show(ui, |ui| {
                            for o in outs {
                                let l = match o["type"].as_str().unwrap_or("") { "live" => "Live", "rec" => "Rekam", _ => "diabaikan" };
                                let act = b_of(o, "active");
                                let t = format!("{} ({}, {}){}", s_of(o, "name"), s_of(o, "kind"), l, if act { format!(" {}", hms(o["ms"].as_i64().unwrap_or(0))) } else { String::new() });
                                dot_text(ui, if act { RED } else { MUTED }, &t);
                            }
                        });
                    }
                }
            }
            ui.add_space(14.0);
            rrow(ui, |ui| {
                ui.spacing_mut().item_spacing.x = 8.0;
                if Btn::b("Tutup").show(ui).clicked() {
                    close = true;
                }
                if Btn::b("Simpan").fill(ACCENT).show(ui).clicked() {
                    save = true;
                }
            });
        });
        self.obs.on = on;
        self.obs.host = host.clone();
        self.obs.port = port.clone();
        self.obs.pass = pass.clone();
        if save {
            let mut b = json!({ "on": if on { 1 } else { 0 }, "host": host.trim(), "port": port.parse::<i64>().unwrap_or(0) });
            if !pass.is_empty() {
                b["pass"] = json!(pass);
            }
            self.be.post("/obs/settings", Some(b), |a, c| {
                a.obs.pass.clear();
                a.obs.hint = b_of(&c, "hasPass");
                a.o_stat();
            });
        }
        if close || outside {
            self.close_obs();
        }
    }

    // ================================================================ Sambungkan HP
    pub fn open_pair(&mut self) {
        self.pair.open = true;
        self.pair.dirty = false;
        self.pair.busy = false;
        self.p_info();
    }
    pub fn close_pair(&mut self) {
        self.pair.open = false;
    }
    fn p_info(&mut self) {
        self.pair.last = Some(Instant::now());
        self.be.get("/pairinfo", |a, j| {
            if j.is_null() {
                return;
            }
            let code = s_of(&j, "code");
            let ips: Vec<String> = j["ips"].as_array().map(|x| x.iter().map(|i| i.as_str().unwrap_or("").to_string()).collect()).unwrap_or_default();
            let (port, wanted) = (j["port"].as_u64().unwrap_or(0), j["wanted"].as_u64().unwrap_or(0));
            let p = &mut a.pair;
            if code != p.code || ips != p.ips || port != p.port {
                let url = format!("http://{}:{}/?code={}&h={}", ips.first().cloned().unwrap_or_else(|| "127.0.0.1".into()), port, code, ips.join(","));
                p.qr = qrcode::QrCode::new(url.as_bytes()).ok().map(|q| (q.width(), q.to_colors().iter().map(|c| *c == qrcode::Color::Dark).collect()));
            }
            p.code = code;
            p.ips = ips;
            p.port = port;
            p.wanted = wanted;
            if !p.dirty {
                p.port_txt = wanted.to_string();
            }
            if !p.busy {
                if port != wanted {
                    p.msg = ("no".into(), format!("localhost:{} telah digunakan program lain, jadi Gatito sementara memakai {}.", wanted, port));
                } else if !p.pending {
                    p.msg = ("".into(), format!("Berjalan di port {}. Angka port bisa diubah.", port));
                }
            }
        });
    }
    fn p_save(&mut self) {
        let b = json!({ "port": self.pair.port_txt });
        self.be.post("/port-set", Some(b), |a, r| {
            let p = &mut a.pair;
            p.busy = true;
            if let Some(e) = r.get("error").and_then(|e| e.as_str()) {
                p.msg = ("no".into(), e.to_string());
                return;
            }
            p.dirty = false;
            if b_of(&r, "restart") {
                p.pending = true;
                p.msg = ("ok".into(), "Tersimpan. Mulai ulang Gatito agar port baru berlaku. HP perlu scan QR lagi.".into());
                p.restart = true;
            } else {
                p.pending = false;
                p.msg = ("ok".into(), "Port ini sudah dipakai sekarang.".into());
                p.restart = false;
            }
        });
    }
    fn draw_pair(&mut self, ctx: &Context) {
        let mut port_txt = self.pair.port_txt.clone();
        let (code, ips, qr, msg, restart) = (self.pair.code.clone(), self.pair.ips.clone(), self.pair.qr.clone(), self.pair.msg.clone(), self.pair.restart);
        let (mut save, mut avoid, mut rst, mut close, mut enter) = (false, false, false, false, false);
        let mut edited = false;
        let (outside, _) = modal(ctx, "pmodal", 380.0, false, |ui| {
            ui.vertical_centered(|ui| {
                ui.label(RichText::new("Sambungkan HP").font(bold(18.0)));
                ui.add_space(12.0);
                let (r, _) = ui.allocate_exact_size(Vec2::splat(230.0), Sense::hover());
                ui.painter().rect_filled(r, 10.0, Color32::WHITE);
                if let Some((n, cells)) = &qr {
                    let quiet = 2.0;
                    let m = (r.width() - 20.0) / (*n as f32 + quiet * 2.0);
                    let o = r.min + vec2(10.0, 10.0);
                    for y in 0..*n {
                        for x in 0..*n {
                            if cells[y * n + x] {
                                let p0 = o + vec2((x as f32 + quiet) * m, (y as f32 + quiet) * m);
                                ui.painter().rect_filled(Rect::from_min_size(p0, Vec2::splat(m + 0.4)), 0.0, Color32::BLACK);
                            }
                        }
                    }
                }
            });
            ui.add_space(8.0);
            for (n, parts) in [(1, vec![("Buka aplikasi Gatito: Stream Tool di HP", false)]), (2, vec![("Tekan ", false), ("Scan QR", true), (", arahkan kamera ke kode ini", false)])] {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 0.0;
                    ui.label(RichText::new(format!("{}. ", n)).color(LIGHT));
                    for (t, b) in parts {
                        ui.label(RichText::new(t).font(if b { bold(14.0) } else { reg(14.0) }).color(LIGHT));
                    }
                });
            }
            ui.add_space(8.0);
            Frame::none().fill(HB).rounding(8.0).inner_margin(Margin::symmetric(12.0, 9.0)).show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Alamat").color(MUTED));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let r = TextEdit::singleline(&mut port_txt).desired_width(60.0).frame(false).font(bold(14.0)).id(Id::new("pport")).show(ui);
                        if r.response.changed() {
                            port_txt.retain(|c| c.is_ascii_digit());
                            edited = true;
                        }
                        if r.response.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                            enter = true;
                        }
                        ui.label(RichText::new(":").font(bold(14.0)));
                        ui.label(RichText::new(if ips.is_empty() { "-".to_string() } else { ips.join(" / ") }).font(bold(14.0)));
                    });
                });
            });
            ui.add_space(6.0);
            Frame::none().fill(HB).rounding(8.0).inner_margin(Margin::symmetric(12.0, 9.0)).show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    ui.label(RichText::new("PIN").color(MUTED));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.label(RichText::new(if code.is_empty() { "-" } else { &code }).font(bold(14.0)));
                    });
                });
            });
            ui.add_space(10.0);
            rrow(ui, |ui| {
                ui.spacing_mut().item_spacing.x = 8.0;
                if restart && Btn::b("Mulai ulang").fill(ACCENT).show(ui).clicked() {
                    rst = true;
                }
                if Btn::b("Simpan port").show(ui).clicked() {
                    save = true;
                }
                if Btn::b("Hindari").show(ui).clicked() {
                    avoid = true;
                }
            });
            let c = match msg.0.as_str() { "ok" => GREEN, "no" => Color32::from_rgb(0xff, 0x8a, 0x80), _ => MUTED };
            ui.add_space(8.0);
            ui.add(Label::new(RichText::new(&msg.1).font(reg(12.5)).color(c)).wrap());
            ui.add_space(14.0);
            rrow(ui, |ui| {
                if Btn::b("Tutup").fill(ACCENT).show(ui).clicked() {
                    close = true;
                }
            });
        });
        if edited {
            self.pair.dirty = true;
            self.pair.busy = false;
        }
        self.pair.port_txt = port_txt;
        if save || enter {
            self.p_save();
        }
        if rst {
            self.be.post_quiet("/restart");
        }
        if avoid {
            self.be.get("/portinfo", |a, j| {
                a.avoid.list = j["avoid"].as_array().map(|x| x.iter().map(|o| (o["port"].as_u64().unwrap_or(0), s_of(o, "name"))).collect()).unwrap_or_default();
                a.avoid.open = true;
            });
        }
        if close || outside {
            self.close_pair();
        }
    }

    fn draw_avoid(&mut self, ctx: &Context) {
        let list = self.avoid.list.clone();
        let mut close = false;
        let (outside, _) = modal(ctx, "hmodal", 420.0, true, |ui| {
            ui.label(RichText::new("Port yang sebaiknya dihindari").font(bold(18.0)));
            ui.add_space(6.0);
            ui.add(Label::new(RichText::new("Port ini biasanya sudah dipakai program lain. Kalau dimasukkan, Gatito menolaknya.").font(reg(13.0)).color(MUTED)).wrap());
            ui.add_space(10.0);
            ScrollArea::vertical().max_height(260.0).show(ui, |ui| {
                for (p, n) in &list {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 12.0;
                        ui.add_sized([56.0, 20.0], Label::new(RichText::new(p.to_string()).font(bold(13.0))));
                        ui.label(RichText::new(n).font(reg(13.0)).color(MUTED));
                    });
                    let y = ui.cursor().top();
                    ui.painter().hline(ui.max_rect().x_range(), y, Stroke::new(1.0, INPUT_BORDER));
                    ui.add_space(6.0);
                }
            });
            ui.add_space(10.0);
            ui.add(Label::new(RichText::new("Port 3000 (bawaan) juga dipakai ChatLivestream Labs dan banyak tool developer. Kalau 3000 sedang terpakai saat Gatito dibuka, Gatito otomatis pindah ke port kosong berikutnya dan memberi tahu kamu. Supaya HP tidak perlu scan ulang, simpan port tetap yang jarang dipakai, misalnya 3456 atau 4600.").font(reg(12.0)).color(MUTED)).wrap());
            ui.add_space(14.0);
            rrow(ui, |ui| {
                if Btn::b("Tutup").fill(ACCENT).show(ui).clicked() {
                    close = true;
                }
            });
        });
        if close || outside {
            self.avoid.open = false;
        }
    }

    // ================================================================ Tombol virtual
    fn draw_vk(&mut self, ctx: &Context) {
        let m = self.vk.m.clone();
        let busy = self.vk.busy;
        let sent = self.vk.sent.clone();
        let done = self.vk.done.clone();
        let used: Vec<Used> = {
            let mut u = self.st.as_ref().map(|s| s.used.clone()).unwrap_or_default();
            let ord = |v: &str| -> i32 { let n: i32 = v[1.min(v.len())..].parse().unwrap_or(0); if v.starts_with('F') { n } else { 100 + n } };
            u.sort_by_key(|x| ord(&x.vk));
            u
        };
        let (mut new_m, mut send_vk, mut float, mut close) = (None::<String>, None::<String>, false, false);
        let mut tex: Vec<Option<TextureId>> = vec![];
        if m == "U" {
            for u in &used {
                tex.push(if u.img.is_empty() { None } else { self.key_image_pub(ctx, &u.img) });
            }
        }
        let (outside, _) = modal(ctx, "vmodal", 420.0, false, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new("Tombol virtual").font(bold(18.0)));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if let Some(v) = seg(ui, &[("F", "F"), ("K", "K"), ("U", "Terpakai")], &m, 44.0, 26.0) {
                        new_m = Some(v);
                    }
                });
            });
            ui.add_space(10.0);
            // keterangan
            if busy {
                if let Some((vk, at)) = &sent {
                    let n = 5 - (at.elapsed().as_secs() as i64).min(5);
                    ui.horizontal_wrapped(|ui| {
                        ui.spacing_mut().item_spacing.x = 0.0;
                        ui.label(RichText::new(vk).font(bold(13.0)).color(MUTED));
                        ui.label(RichText::new(" dikirim dalam ").font(reg(13.0)).color(MUTED));
                        ui.label(RichText::new(n.to_string()).font(bold(13.0)).color(MUTED));
                        ui.label(RichText::new(" detik. Klik kotak hotkey di OBS sekarang.").font(reg(13.0)).color(MUTED));
                    });
                }
            } else if let Some((vk, at)) = done.as_ref().filter(|(_, t)| t.elapsed() < Duration::from_millis(2500)).map(|(a, b)| (a.clone(), *b)) {
                let _ = at;
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing.x = 0.0;
                    ui.label(RichText::new(&vk).font(bold(13.0)).color(MUTED));
                    ui.label(RichText::new(" terkirim.").font(reg(13.0)).color(MUTED));
                });
            } else if m == "U" {
                ui.label(RichText::new("Tombol virtual yang sedang dipakai di semua profil.").font(reg(13.0)).color(MUTED));
            } else {
                ui.add(Label::new(RichText::new(VHINT).font(reg(13.0)).color(MUTED)).wrap());
            }
            ui.add_space(10.0);
            if m == "U" {
                ScrollArea::vertical().max_height(250.0).show(ui, |ui| {
                    if used.is_empty() {
                        ui.label(RichText::new("Belum ada tombol virtual yang dipakai.").color(MUTED));
                    }
                    for (i, u) in used.iter().enumerate() {
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 12.0;
                            ui.add_sized([42.0, 40.0], Label::new(RichText::new(&u.vk).font(bold(14.0))));
                            let (r, _) = ui.allocate_exact_size(Vec2::splat(40.0), Sense::hover());
                            if let Some(Some(t)) = tex.get(i) {
                                rounded_mesh(ui.painter(), *t, r, 9.0, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
                            } else {
                                let c = if u.color.is_empty() { ACCENT } else { parse_hex(&u.color) };
                                ui.painter().rect_filled(r, 9.0, c);
                            }
                            ui.painter().rect_stroke(r, 9.0, Stroke::new(1.0, BORDER));
                            ui.vertical(|ui| {
                                ui.spacing_mut().item_spacing.y = 0.0;
                                if u.label.is_empty() {
                                    ui.label(RichText::new("(tanpa nama)").color(MUTED));
                                } else {
                                    ui.label(&u.label);
                                }
                                ui.label(RichText::new(format!("{}, halaman {}", u.prof, u.page)).font(reg(12.0)).color(MUTED));
                            });
                        });
                        let y = ui.cursor().top();
                        ui.painter().hline(ui.max_rect().x_range(), y + 4.0, Stroke::new(1.0, HB));
                        ui.add_space(8.0);
                    }
                });
            } else {
                let list: Vec<String> = if m == "F" { (13..25).map(|i| format!("F{i}")).collect() } else { (1..37).map(|i| format!("K{i}")).collect() };
                ScrollArea::vertical().max_height(210.0).show(ui, |ui| {
                    let w = (ui.available_width() - 24.0) / 4.0;
                    for row in list.chunks(4) {
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing = vec2(8.0, 8.0);
                            for v in row {
                                if Btn::b(v).w(w).h(40.0).font(bold(14.0)).enabled(!busy).hover(ACCENT).show(ui).clicked() {
                                    send_vk = Some(v.clone());
                                }
                            }
                        });
                    }
                });
            }
            ui.add_space(14.0);
            ui.horizontal(|ui| {
                if Btn::b("Mode mengambang").show(ui).on_hover_text("Jendela kecil yang tetap di atas OBS dan mengirim tombol langsung").clicked() {
                    float = true;
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if Btn::b("Tutup").fill(ACCENT).show(ui).clicked() {
                        close = true;
                    }
                });
            });
        });
        if let Some(v) = new_m {
            self.vk.m = v;
        }
        if let Some(vk) = send_vk {
            self.vk.busy = true;
            self.vk.sent = Some((vk.clone(), Instant::now()));
            self.be.send(json!({ "t": "test", "key": { "action": "vkey", "vk": vk }, "delay": 5000 }));
        }
        if float {
            self.vk.open = false;
            self.be.post_quiet("/vpad/open");
        }
        if close || outside {
            self.vk.open = false;
        }
    }

    pub fn key_image_pub(&mut self, ctx: &Context, img: &str) -> Option<TextureId> {
        self.key_image(ctx, img, false).map(|(t, _)| t)
    }

    // ================================================================ nama profil dan konfirmasi
    fn draw_name(&mut self, ctx: &Context) {
        let mut d = match self.name_dlg.take() {
            Some(d) => d,
            None => return,
        };
        let (mut ok, mut cancel) = (false, false);
        let (outside, _) = modal(ctx, "nmodal", 340.0, true, |ui| {
            ui.label(RichText::new(&d.title).font(bold(18.0)));
            ui.add_space(8.0);
            let id = Id::new("nin");
            let r = text_input(ui, id, &mut d.val, ui.available_width(), "", false, false);
            if d.focus {
                r.request_focus();
                d.focus = false;
            }
            if r.has_focus() || r.lost_focus() {
                if ui.input(|i| i.key_pressed(Key::Enter)) {
                    ok = true;
                }
            }
            ui.add_space(14.0);
            rrow(ui, |ui| {
                ui.spacing_mut().item_spacing.x = 8.0;
                if Btn::b("Simpan").fill(ACCENT).show(ui).clicked() {
                    ok = true;
                }
                if Btn::b("Batal").show(ui).clicked() {
                    cancel = true;
                }
            });
        });
        let v = d.val.trim().to_string();
        if ok && !v.is_empty() {
            match d.kind {
                NameKind::New => self.be.send(json!({ "t": "addProf", "name": v })),
                NameKind::Ren(i) => self.be.send(json!({ "t": "renProf", "i": i, "name": v })),
            }
        } else if !(ok || cancel || outside) {
            self.name_dlg = Some(d);
        }
    }

    fn draw_confirm(&mut self, ctx: &Context) {
        let c = match self.confirm.take() {
            Some(c) => c,
            None => return,
        };
        let (mut ok, mut cancel) = (false, false);
        let (outside, _) = modal(ctx, "cmodal", 340.0, true, |ui| {
            ui.label(RichText::new(&c.title).font(bold(18.0)));
            if !c.text.is_empty() {
                ui.add_space(6.0);
                ui.add(Label::new(RichText::new(&c.text).font(reg(13.0)).color(MUTED)).wrap());
            }
            ui.add_space(16.0);
            rrow(ui, |ui| {
                ui.spacing_mut().item_spacing.x = 8.0;
                if Btn::b(&c.ok).fill(c.color).show(ui).clicked() {
                    ok = true;
                }
                if Btn::b("Batal").show(ui).clicked() {
                    cancel = true;
                }
            });
        });
        if ok {
            self.exec_confirm(c.act);
        } else if !(cancel || outside) {
            self.confirm = Some(c);
        }
    }

    pub fn draw_modals(&mut self, ctx: &Context) {
        if self.audio.open {
            self.draw_audio(ctx);
        }
        if self.obs.open {
            self.draw_obs(ctx);
        }
        if self.pair.open {
            self.draw_pair(ctx);
        }
        if self.vk.open {
            self.draw_vk(ctx);
        }
        if self.avoid.open {
            self.draw_avoid(ctx);
        }
        if self.name_dlg.is_some() {
            self.draw_name(ctx);
        }
        if self.confirm.is_some() {
            self.draw_confirm(ctx);
        }
    }

    /// Polling berkala dan pewaktu, setara setInterval di editor.html
    pub fn tick(&mut self, ctx: &Context) {
        let now = Instant::now();
        if self.audio.open {
            if self.audio.last_level.map(|t| t.elapsed() >= Duration::from_millis(120)).unwrap_or(true) {
                self.audio.last_level = Some(now);
                self.be.get("/audio/level", |a, l| {
                    if !l.is_null() {
                        a.audio.lmic = l["mic"].as_f64().unwrap_or(0.0) as f32;
                        a.audio.lout = l["out"].as_f64().unwrap_or(0.0) as f32;
                    }
                });
            }
            if self.audio.inst_poll && self.audio.last_inst.map(|t| t.elapsed() >= Duration::from_secs(1)).unwrap_or(true) {
                self.audio.last_inst = Some(now);
                self.be.get("/audio/install/status", |a, s| {
                    if s.is_null() {
                        return;
                    }
                    let running = b_of(&s, "running");
                    a.audio.inst = Some(s);
                    if !running {
                        a.audio.inst_poll = false;
                        a.a_load();
                    }
                });
            }
            ctx.request_repaint_after(Duration::from_millis(60));
        }
        if self.obs.open {
            if self.obs.last.map(|t| t.elapsed() >= Duration::from_secs(1)).unwrap_or(true) {
                self.o_stat();
            }
            ctx.request_repaint_after(Duration::from_millis(500));
        }
        if self.pair.open {
            if self.pair.last.map(|t| t.elapsed() >= Duration::from_secs(3)).unwrap_or(true) {
                self.p_info();
            }
            ctx.request_repaint_after(Duration::from_millis(500));
        }
        if self.vk.open {
            if self.vk.busy {
                if let Some((vk, at)) = self.vk.sent.clone() {
                    if at.elapsed() >= Duration::from_secs(5) {
                        self.vk.busy = false;
                        self.vk.done = Some((vk, Instant::now()));
                    }
                }
            }
            ctx.request_repaint_after(Duration::from_millis(250));
        }
    }
}
