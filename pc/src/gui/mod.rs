// Editor Gatito (PC) dengan egui: tata letak, warna, dan perilaku mengikuti editor.html versi Electron.
mod backend;
mod modals;
mod vpad;
mod widgets;

use crate::app::Ctx;
use crate::state::Key as KeyObj;
use backend::*;
use eframe::egui;
use eframe::egui::*;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};
use widgets::*;

pub use modals::*;

pub fn sget(k: &KeyObj, n: &str) -> String {
    k.get(n).and_then(|v| v.as_str()).unwrap_or("").to_string()
}

#[derive(Clone, Debug)]
pub struct Used {
    pub vk: String,
    pub label: String,
    pub img: String,
    pub color: String,
    pub prof: String,
    pub page: i64,
    pub pi: i64,
    pub pg: i64,
    pub ki: i64,
}

#[derive(Clone, Default)]
pub struct St {
    pub cols: usize,
    pub rows: usize,
    pub pages: Vec<HashMap<usize, KeyObj>>,
    pub profiles: Vec<String>,
    pub cur: i64,
    pub used: Vec<Used>,
}

#[derive(Clone, Debug)]
pub enum Payload {
    Lib(String),
    Key(usize),
}

pub enum Act {
    DelPage(usize),
    DelProf(usize),
    Update,
}
pub struct Confirm {
    pub title: String,
    pub text: String,
    pub ok: String,
    pub color: Color32,
    pub act: Act,
}
pub enum NameKind {
    New,
    Ren(usize),
}
pub struct NameDlg {
    pub title: String,
    pub val: String,
    pub kind: NameKind,
    pub focus: bool,
}

#[derive(Clone, Copy, PartialEq)]
pub enum DdMode {
    Vk,
    List,
}
pub struct Dd {
    pub mode: DdMode,
    pub anchor: Rect,
    pub m: char,
}

pub enum TexEntry {
    Still(TextureHandle, [usize; 2]),
    Anim(Vec<(TextureHandle, f64)>, f64, [usize; 2]),
}

#[derive(Default)]
pub struct Textures {
    map: HashMap<String, Option<TexEntry>>,
    icons: HashMap<usize, TextureHandle>,
    icon_fail: HashMap<usize, Instant>,
    crt: HashMap<usize, TextureHandle>,
}

pub struct Up {
    pub btn: String,
    pub new: Option<Value>,
    pub busy: bool,
    pub disabled: bool,
}

pub struct App {
    pub be: Backend,
    pub dir: PathBuf,
    pub st: Option<St>,
    pub page: usize,
    pub sel: Option<usize>,
    last_cur: i64,
    pub apps: Vec<(String, String)>,
    tex: Textures,
    dd: Option<Dd>,
    toast: Option<(String, Instant)>,
    pub confirm: Option<Confirm>,
    pub name_dlg: Option<NameDlg>,
    pub audio: AudioM,
    pub obs: ObsM,
    pub pair: PairM,
    pub avoid: AvoidM,
    pub vk: VkM,
    pub version: String,
    pub up: Up,
    started: Instant,
    pub dd_filter: String,
    up_started: bool,
    pub vpad_ui: vpad::VpadState,
}

pub fn run(ctx: Arc<Ctx>, port: u16) {
    let (rgba, w, h) = crate::shell::icon_rgba(64);
    let opts = eframe::NativeOptions {
        viewport: ViewportBuilder::default()
            .with_title(crate::shell::MAIN_TITLE)
            .with_inner_size([1000.0, 760.0])
            .with_min_inner_size([860.0, 600.0])
            .with_icon(IconData { rgba, width: w, height: h }),
        ..Default::default()
    };
    let rt = ctx.rt.clone();
    let dir = ctx.dir.clone();
    let _ = eframe::run_native(
        crate::shell::MAIN_TITLE,
        opts,
        Box::new(move |cc| {
            setup(&cc.egui_ctx);
            egui_extras::install_image_loaders(&cc.egui_ctx);
            crate::shell::set_egui(cc.egui_ctx.clone());
            Ok(Box::new(App::new(rt, cc.egui_ctx.clone(), dir, port)))
        }),
    );
}

impl App {
    fn new(rt: tokio::runtime::Handle, ctx: Context, dir: PathBuf, port: u16) -> App {
        let be = Backend::new(rt, ctx, port);
        let app = App {
            be: be.clone(),
            dir,
            st: None,
            page: 0,
            sel: None,
            last_cur: -1,
            apps: vec![],
            tex: Textures::default(),
            dd: None,
            toast: None,
            confirm: None,
            name_dlg: None,
            audio: AudioM::default(),
            obs: ObsM::default(),
            pair: PairM::default(),
            avoid: AvoidM::default(),
            vk: VkM::new(),
            version: "Versi".into(),
            up: Up { btn: "Cek update".into(), new: None, busy: false, disabled: false },
            started: Instant::now(),
            dd_filter: String::new(),
            up_started: false,
            vpad_ui: vpad::VpadState::default(),
        };
        be.get("/apps", |a, v| {
            if let Some(l) = v.as_array() {
                a.apps = l.iter().map(|x| (x["name"].as_str().unwrap_or("").to_string(), x["path"].as_str().unwrap_or("").to_string())).collect();
            }
        });
        be.get("/appinfo", |a, v| {
            if let Some(s) = v.get("version").and_then(|x| x.as_str()) {
                a.version = format!("Versi {}", s);
            }
        });
        app
    }

    pub fn on_state(&mut self, s: Value) {
        let mut st = St {
            cols: s["cols"].as_u64().unwrap_or(5) as usize,
            rows: s["rows"].as_u64().unwrap_or(3) as usize,
            cur: s["cur"].as_i64().unwrap_or(0),
            ..Default::default()
        };
        for pg in s["pages"].as_array().cloned().unwrap_or_default() {
            let mut m = HashMap::new();
            if let Some(ks) = pg["keys"].as_object() {
                for (k, v) in ks {
                    if let (Ok(i), Some(o)) = (k.parse::<usize>(), v.as_object()) {
                        m.insert(i, o.clone());
                    }
                }
            }
            st.pages.push(m);
        }
        st.profiles = s["profiles"].as_array().map(|a| a.iter().map(|x| x.as_str().unwrap_or("").to_string()).collect()).unwrap_or_default();
        for u in s["usedVk"].as_array().cloned().unwrap_or_default() {
            st.used.push(Used {
                vk: u["vk"].as_str().unwrap_or("").to_string(),
                label: u["label"].as_str().unwrap_or("").to_string(),
                img: u["img"].as_str().unwrap_or("").to_string(),
                color: u["color"].as_str().unwrap_or("").to_string(),
                prof: u["prof"].as_str().unwrap_or("").to_string(),
                page: u["page"].as_i64().unwrap_or(0),
                pi: u["pi"].as_i64().unwrap_or(-1),
                pg: u["pg"].as_i64().unwrap_or(-1),
                ki: u["ki"].as_i64().unwrap_or(-1),
            });
        }
        if st.pages.is_empty() {
            st.pages.push(HashMap::new());
        }
        if self.page >= st.pages.len() {
            self.page = st.pages.len() - 1;
        }
        if st.cur != self.last_cur {
            self.last_cur = st.cur;
            self.page = 0;
            self.sel = None;
        }
        self.st = Some(st);
    }

    pub fn toast(&mut self, t: &str) {
        self.toast = Some((t.to_string(), Instant::now()));
    }

    pub fn ask_confirm(&mut self, title: &str, text: &str, ok: &str, color: Color32, act: Act) {
        self.confirm = Some(Confirm { title: title.into(), text: text.into(), ok: ok.into(), color, act });
    }

    fn mkdef(&self, id: &str) -> Value {
        let mut d = match id {
            "hotkey" => json!({ "label": "Hotkey", "color": "#4f6df5", "action": "hotkey", "keys": "" }),
            "vkey" => json!({ "label": "Tombol Virtual", "color": "#3a8fb7", "action": "vkey", "vk": "F13" }),
            "open" => json!({ "label": "Aplikasi", "color": "#8a6d3b", "action": "open", "target": "" }),
            "web" => json!({ "label": "Website", "color": "#3a8fb7", "action": "web", "target": "" }),
            "text" => json!({ "label": "Teks", "color": "#7b5bb5", "action": "text", "text": "" }),
            "media" => json!({ "label": "Media", "color": "#3f8f6b", "action": "media", "key": "playpause" }),
            "sound" => json!({ "label": "Audio", "color": "#d6455d", "action": "sound", "file": "", "vol": "100" }),
            "stopsound" => json!({ "label": "Stop Audio", "color": "#7a3b46", "action": "stopsound" }),
            "next" => json!({ "label": "Berikutnya", "color": "#5c6370", "action": "nav", "to": "next" }),
            _ => json!({ "label": "Sebelumnya", "color": "#5c6370", "action": "nav", "to": "prev" }),
        };
        if id == "vkey" {
            if let Some(st) = &self.st {
                if let Some(v) = all_vk().into_iter().find(|x| !st.used.iter().any(|u| u.pi == st.cur && &u.vk == x)) {
                    d["vk"] = json!(v);
                }
            }
        }
        d
    }

    fn set_key(&self, k: &KeyObj) {
        if let Some(s) = self.sel {
            self.be.send(msg_set(self.page, s, &Value::Object(k.clone())));
        }
    }

    fn move_key(&mut self, k: usize, to: usize) {
        let st = match &self.st {
            Some(s) => s,
            None => return,
        };
        if to >= st.pages.len() {
            return;
        }
        if st.pages[to].len() >= st.cols * st.rows {
            return self.toast(&format!("Halaman {} sudah penuh", to + 1));
        }
        self.be.send(json!({ "t": "move", "p": self.page, "k": k, "to": to }));
        self.sel = None;
        self.toast(&format!("Tombol dipindah ke halaman {}", to + 1));
    }

    // ---------------------------------------------------------------- gambar
    fn img_bytes(&self, img: &str) -> Option<Vec<u8>> {
        if let Some(n) = img.strip_prefix("/img/") {
            if n.contains('/') || n.contains('\\') {
                return None;
            }
            return std::fs::read(self.dir.join("images").join(n)).ok();
        }
        if img.starts_with("data:image/") {
            use base64::Engine as _;
            let i = img.find(',')?;
            return base64::engine::general_purpose::STANDARD.decode(&img[i + 1..]).ok();
        }
        None
    }

    fn key_image(&mut self, ctx: &Context, img: &str, crt: bool) -> Option<(TextureId, [usize; 2])> {
        let ck = format!("{}|{}", img, crt as u8);
        if !self.tex.map.contains_key(&ck) {
            let e = self.img_bytes(img).and_then(|b| decode_tex(ctx, &ck, &b, crt));
            self.tex.map.insert(ck.clone(), e);
        }
        match self.tex.map.get(&ck)? {
            Some(TexEntry::Still(t, s)) => Some((t.id(), *s)),
            Some(TexEntry::Anim(fr, total, s)) => {
                let now = ctx.input(|i| i.time) % total.max(0.02);
                let mut acc = 0.0;
                let mut pick = &fr[0].0;
                for (t, d) in fr {
                    acc += d;
                    if now < acc {
                        pick = t;
                        break;
                    }
                }
                ctx.request_repaint_after(Duration::from_millis(40));
                Some((pick.id(), *s))
            }
            None => None,
        }
    }

    fn crt_tex(&mut self, ctx: &Context, n: usize) -> TextureId {
        self.tex.crt.entry(n).or_insert_with(|| ctx.load_texture(format!("crt{n}"), crt_image(n), TextureOptions::LINEAR)).id()
    }

    pub fn app_icon(&mut self, ctx: &Context, i: usize) -> Option<TextureId> {
        if let Some(t) = self.tex.icons.get(&i) {
            return Some(t.id());
        }
        if self.tex.icon_fail.get(&i).map(|t| t.elapsed() < Duration::from_secs(2)).unwrap_or(false) {
            return None;
        }
        let path = self.apps.get(i).map(|a| a.1.clone())?;
        let f = self.dir.join("icons").join(format!("{}.png", crate::util::sha1_hex(path.as_bytes())));
        if let Ok(b) = std::fs::read(&f) {
            if let Ok(im) = image::load_from_memory(&b) {
                let im = im.to_rgba8();
                let (w, h) = im.dimensions();
                let t = ctx.load_texture(format!("ico{i}"), ColorImage::from_rgba_unmultiplied([w as usize, h as usize], im.as_raw()), TextureOptions::LINEAR);
                let id = t.id();
                self.tex.icons.insert(i, t);
                return Some(id);
            }
        }
        self.tex.icon_fail.insert(i, Instant::now());
        None
    }

    // ---------------------------------------------------------------- tombol di grid dan preview
    fn paint_key(&mut self, ui: &mut Ui, rect: Rect, key: Option<&KeyObj>, selected: bool, over: bool, pv: bool) {
        let ctx = ui.ctx().clone();
        let p = ui.painter().clone();
        let r = 14.0;
        let n = if pv { 64 } else { 78 };
        match key {
            None => {
                p.rect(rect, r, INPUT_BG, Stroke::new(2.0, if over { SEL } else { BORDER }));
            }
            Some(k) => {
                let color = parse_hex(&sget(k, "color"));
                let color = if sget(k, "color").is_empty() { ACCENT } else { color };
                let img = sget(k, "img");
                let crt = sget(k, "crt") == "1";
                p.rect_filled(rect, r, color);
                let mut has_img = false;
                if !img.is_empty() {
                    if let Some((tid, sz)) = self.key_image(&ctx, &img, crt) {
                        has_img = true;
                        let (w, h) = (sz[0] as f32, sz[1] as f32);
                        let uv = if w > h {
                            let u = h / w;
                            Rect::from_min_max(pos2((1.0 - u) / 2.0, 0.0), pos2((1.0 + u) / 2.0, 1.0))
                        } else {
                            let u = w / h;
                            Rect::from_min_max(pos2(0.0, (1.0 - u) / 2.0), pos2(1.0, (1.0 + u) / 2.0))
                        };
                        rounded_mesh(&p, tid, rect, r, uv, Color32::WHITE);
                    }
                }
                if has_img && crt {
                    let t = self.crt_tex(&ctx, n);
                    rounded_mesh(&p, t, rect, r, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
                }
                let label = sget(k, "label");
                if !label.is_empty() {
                    if has_img {
                        let fs = if pv { 10.0 } else { 9.0 };
                        let g = p.layout(label, bold(fs), Color32::WHITE, rect.width() - 6.0);
                        let h = g.size().y + 6.0;
                        let bar = Rect::from_min_max(pos2(rect.left(), rect.bottom() - h), rect.max);
                        // sudut bawah mengikuti lengkung tombol
                        p.rect_filled(bar, Rounding { nw: 0.0, ne: 0.0, sw: r, se: r }, Color32::from_black_alpha(140));
                        p.galley(pos2(rect.center().x - g.size().x / 2.0, bar.top() + 3.0), g, Color32::WHITE);
                    } else {
                        let g = p.layout(label, bold(12.0), Color32::WHITE, rect.width() - 8.0);
                        p.galley(rect.center() - g.size() / 2.0, g, Color32::WHITE);
                    }
                }
                if over {
                    p.rect_stroke(rect.shrink(1.0), r, Stroke::new(2.0, SEL));
                }
            }
        }
        if selected {
            p.rect_stroke(rect.expand(3.5), r + 2.0, Stroke::new(3.0, SEL));
        }
    }

    // ---------------------------------------------------------------- header
    fn draw_header(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 8.0;
                ui.label(RichText::new("Gatito: Stream Tool").font(bold(22.0)));
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 6.0;
                    let (names, cur) = self.st.as_ref().map(|s| (s.profiles.clone(), s.cur)).unwrap_or_default();
                    let opts: Vec<(String, String)> = names.iter().enumerate().map(|(i, n)| (i.to_string(), n.clone())).collect();
                    if let Some(v) = select(ui, "prof", 190.0, &cur.to_string(), &opts) {
                        self.be.send(json!({ "t": "prof", "i": v.parse::<i64>().unwrap_or(0) }));
                    }
                    if Btn::sb("+").w(30.0).font(reg(14.0)).show(ui).on_hover_text("Profil baru").clicked() {
                        self.name_dlg = Some(NameDlg { title: "Nama profil baru".into(), val: String::new(), kind: NameKind::New, focus: true });
                    }
                    if icon_btn(ui, 30.0, Icon::Pencil, HB, Some(HB_HOVER), false, true, 8.0).on_hover_text("Ubah nama profil").clicked() {
                        let n = names.get(cur as usize).cloned().unwrap_or_default();
                        self.name_dlg = Some(NameDlg { title: "Ubah nama profil".into(), val: n, kind: NameKind::Ren(cur as usize), focus: true });
                    }
                    if icon_btn(ui, 30.0, Icon::Trash, HB, None, true, true, 8.0).on_hover_text("Hapus profil").clicked() {
                        if names.len() < 2 {
                            self.toast("Minimal harus ada satu profil");
                        } else {
                            let n = names.get(cur as usize).cloned().unwrap_or_default();
                            self.ask_confirm(&format!("Hapus profil \"{}\"?", n), "Semua halaman dan tombol di profil ini ikut terhapus.", "Hapus", DANGER, Act::DelProf(cur as usize));
                        }
                    }
                });
            });
            ui.with_layout(Layout::right_to_left(Align::TOP), |ui| {
                ui.spacing_mut().item_spacing.x = 8.0;
                if icon_btn(ui, 40.0, Icon::Qr, HB, Some(HB_HOVER), false, true, 10.0).on_hover_text("Sambungkan HP (kode QR)").clicked() {
                    self.open_pair();
                }
                if icon_btn(ui, 40.0, Icon::Keyboard, HB, Some(HB_HOVER), false, true, 10.0).on_hover_text("Tombol virtual").clicked() {
                    self.vk = VkM::new();
                    self.vk.open = true;
                }
                // tombol OBS: logo
                let (rect, resp) = ui.allocate_exact_size(vec2(40.0, 40.0), Sense::click());
                ui.painter().rect_filled(rect, 10.0, if resp.hovered() { HB_HOVER } else { HB });
                let img = Image::from_bytes("bytes://obs-logo.png", crate::server::OBS_LOGO).fit_to_exact_size(vec2(26.0, 26.0));
                img.paint_at(ui, Rect::from_center_size(rect.center(), vec2(26.0, 26.0)));
                if resp.on_hover_text("OBS").on_hover_cursor(CursorIcon::PointingHand).clicked() {
                    self.open_obs();
                }
                if icon_btn(ui, 40.0, Icon::Mic, HB, Some(HB_HOVER), false, true, 10.0).on_hover_text("Suara").clicked() {
                    self.open_audio();
                }
            });
        });
    }

    // ---------------------------------------------------------------- grid + tab halaman
    fn draw_stage(&mut self, ui: &mut Ui) {
        let st = match self.st.clone() {
            Some(s) => s,
            None => return,
        };
        let avail = ui.available_rect_before_wrap();
        let (cols, rows) = (st.cols, st.rows);
        let gw = cols as f32 * 78.0 + (cols.saturating_sub(1)) as f32 * 10.0;
        let gh = rows as f32 * 78.0 + (rows.saturating_sub(1)) as f32 * 10.0;
        let total = gh + 14.0 + 30.0;
        let top = avail.top() + ((avail.height() - total) / 2.0).max(14.0);
        let left = avail.center().x - gw / 2.0;
        let keys = st.pages.get(self.page).cloned().unwrap_or_default();
        let mut drop_do: Option<Payload> = None;
        let mut drop_idx = 0usize;
        for i in 0..cols * rows {
            let (cx, cy) = (i % cols, i / cols);
            let rect = Rect::from_min_size(pos2(left + cx as f32 * 88.0, top + cy as f32 * 88.0), vec2(78.0, 78.0));
            let resp = ui.interact(rect, Id::new(("key", i)), Sense::click_and_drag());
            let k = keys.get(&i);
            if k.is_some() {
                if resp.drag_started() {
                    resp.dnd_set_drag_payload(Payload::Key(i));
                }
            }
            let over = resp.dnd_hover_payload::<Payload>().is_some();
            self.paint_key(ui, rect, k, self.sel == Some(i), over, false);
            if resp.clicked() {
                self.sel = Some(i);
            }
            if let Some(pl) = resp.dnd_release_payload::<Payload>() {
                drop_do = Some((*pl).clone());
                drop_idx = i;
            }
        }
        if let Some(pl) = drop_do {
            match pl {
                Payload::Lib(id) => {
                    let d = self.mkdef(&id);
                    self.be.send(json!({ "t": "set", "p": self.page, "k": drop_idx, "key": d }));
                }
                Payload::Key(a) => {
                    if a != drop_idx {
                        self.be.send(json!({ "t": "swap", "p": self.page, "a": a, "b": drop_idx }));
                    }
                }
            }
            self.sel = Some(drop_idx);
        }

        // tab halaman
        let ty = top + gh + 14.0;
        let n = st.pages.len();
        let mut widths: Vec<f32> = (0..n).map(|i| (ui.painter().layout_no_wrap((i + 1).to_string(), reg(14.0), TEXT).size().x + 16.0).max(36.0)).collect();
        widths.push(36.0);
        if n > 1 {
            widths.push(30.0);
        }
        let tw: f32 = widths.iter().sum::<f32>() + (widths.len() as f32 - 1.0) * 6.0;
        let mut x = avail.center().x - tw / 2.0;
        let mut move_to: Option<(usize, usize)> = None;
        for i in 0..n {
            let rect = Rect::from_min_size(pos2(x, ty), vec2(widths[i], 30.0));
            x += widths[i] + 6.0;
            let resp = ui.interact(rect, Id::new(("tab", i)), Sense::click());
            let hov_drop = i != self.page && resp.dnd_hover_payload::<Payload>().map(|p| matches!(*p, Payload::Key(_))).unwrap_or(false);
            let act = i == self.page;
            let (fill, fg) = if hov_drop { (ACCENT, Color32::WHITE) } else if act { (TEXT, BG) } else { (HB, LIGHT) };
            ui.painter().rect_filled(rect, 15.0, fill);
            ui.painter().text(rect.center(), Align2::CENTER_CENTER, (i + 1).to_string(), reg(14.0), fg);
            if resp.clicked() {
                self.page = i;
                self.sel = None;
            }
            if i != self.page {
                if let Some(pl) = resp.dnd_release_payload::<Payload>() {
                    if let Payload::Key(k) = *pl {
                        move_to = Some((k, i));
                    }
                }
            }
        }
        let rect = Rect::from_min_size(pos2(x, ty), vec2(36.0, 30.0));
        x += 42.0;
        let resp = ui.interact(rect, Id::new("tab+"), Sense::click());
        ui.painter().rect_filled(rect, 15.0, HB);
        ui.painter().text(rect.center(), Align2::CENTER_CENTER, "+", reg(14.0), LIGHT);
        if resp.on_hover_text("Tambah halaman").clicked() {
            self.be.send(json!({ "t": "addPage" }));
        }
        if n > 1 {
            let rect = Rect::from_min_size(pos2(x, ty), vec2(30.0, 30.0));
            let resp = ui.interact(rect, Id::new("tabdel"), Sense::click());
            let hov = resp.hovered();
            ui.painter().rect_filled(rect, 15.0, if hov { DANGER_BG } else { HB });
            paint_icon(ui.painter(), Rect::from_center_size(rect.center(), Vec2::splat(16.0)), Icon::Trash, if hov { DANGER_FG } else { LIGHT });
            if resp.on_hover_text("Hapus halaman ini").clicked() {
                self.ask_confirm(&format!("Hapus halaman {}?", self.page + 1), "Semua tombol di halaman ini ikut terhapus.", "Hapus", DANGER, Act::DelPage(self.page));
            }
        }
        if let Some((k, to)) = move_to {
            self.move_key(k, to);
        }
    }

    // ---------------------------------------------------------------- panel pengaturan tombol
    fn draw_panel(&mut self, ui: &mut Ui) {
        let cur: Option<KeyObj> = self.sel.and_then(|s| self.st.as_ref().and_then(|st| st.pages.get(self.page).and_then(|p| p.get(&s).cloned())));
        let mut k = match cur {
            Some(k) => k,
            None => {
                ui.label(RichText::new("Klik sebuah tombol untuk mengaturnya. Seret aksi dari daftar di kanan ke tombol. Seret tombol ke tombol lain untuk menukar posisi, atau ke nomor halaman untuk memindahkannya.").color(MUTED));
                return;
            }
        };
        let sel = self.sel.unwrap();
        let salt = format!("{}-{}", self.page, sel);
        let action = sget(&k, "action");
        let title = match action.as_str() {
            "hotkey" => "Hotkey",
            "vkey" => "Tombol virtual",
            "open" => "Buka aplikasi",
            "web" => "Buka website",
            "text" => "Ketik teks",
            "media" => "Multimedia",
            "sound" => "Putar audio",
            "stopsound" => "Hentikan audio",
            "nav" => "Navigasi",
            _ => "Tombol",
        };
        let mut changed = false;
        ui.horizontal(|ui| {
            ui.label(RichText::new(title).font(bold(14.0)));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if icon_btn(ui, 32.0, Icon::Trash, Color32::TRANSPARENT, None, true, true, 8.0).on_hover_text("Hapus tombol").clicked() {
                    self.be.send(json!({ "t": "del", "p": self.page, "k": sel }));
                }
            });
        });
        let sep = ui.cursor().top();
        ui.painter().hline(ui.max_rect().x_range(), sep, Stroke::new(1.0, BORDER));
        ui.add_space(8.0);
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = 16.0;
            // kolom preview
            ui.vertical(|ui| {
                ui.set_width(72.0);
                let (rect, _) = ui.allocate_exact_size(Vec2::splat(64.0), Sense::hover());
                let kc = k.clone();
                self.paint_key(ui, rect, Some(&kc), false, false, true);
                ui.add_space(2.0);
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 4.0;
                    if icon_btn(ui, 34.0, Icon::Upload, B_BG, None, false, true, 8.0).on_hover_text("Pilih gambar").clicked() {
                        let (p, s) = (self.page, sel);
                        let mut kk = k.clone();
                        self.be.post("/pick/image", None, move |a, v| {
                            if let Some(img) = v.get("img").and_then(|x| x.as_str()) {
                                kk.insert("img".into(), json!(img));
                                kk.insert("crt".into(), json!("1"));
                                a.be.send(msg_set(p, s, &Value::Object(kk)));
                            }
                        });
                    }
                    let has_img = !sget(&k, "img").is_empty();
                    if icon_btn(ui, 34.0, Icon::Trash, B_BG, None, false, has_img, 8.0).on_hover_text("Hapus gambar").clicked() {
                        k.remove("img");
                        changed = true;
                    }
                });
            });
            // formulir
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 6.0;
                ui.horizontal_top(|ui| {
                    ui.spacing_mut().item_spacing.x = 16.0;
                    labeled(ui, "Nama tombol", |ui| {
                        if field_text(ui, &salt, &mut k, "label", 200.0, "") {
                            changed = true;
                        }
                    });
                    labeled(ui, "Warna", |ui| {
                        let mut rgb = parse_hex(&sget(&k, "color")).to_array();
                        let mut c = [rgb[0], rgb[1], rgb[2]];
                        ui.scope(|ui| {
                            ui.spacing_mut().interact_size = vec2(60.0, 36.0);
                            if ui.color_edit_button_srgb(&mut c).changed() {
                                rgb = [c[0], c[1], c[2], 255];
                                k.insert("color".into(), json!(to_hex([rgb[0], rgb[1], rgb[2]])));
                                let did = Id::new(("colordirty", &salt));
                                ui.data_mut(|d| d.insert_temp(did, true));
                            }
                        });
                        let did = Id::new(("colordirty", &salt));
                        if ui.data(|d| d.get_temp::<bool>(did)).unwrap_or(false) && !ui.input(|i| i.pointer.any_down()) {
                            ui.data_mut(|d| d.insert_temp(did, false));
                            changed = true;
                        }
                    });
                    if !sget(&k, "img").is_empty() {
                        ui.add_space(0.0);
                        let mut on = sget(&k, "crt") == "1";
                        ui.vertical(|ui| {
                            ui.add_space(22.0);
                            if ui.checkbox(&mut on, RichText::new("Filter CRT").font(reg(12.0))).changed() {
                                k.insert("crt".into(), json!(if on { "1" } else { "0" }));
                                changed = true;
                            }
                        });
                    }
                });
                let id_dd = ui.id();
                let _ = id_dd;
                self.panel_fields(ui, &mut k, &salt, &action, &mut changed);
            });
        });
        if changed {
            self.set_key(&k);
        }
    }

    fn panel_fields(&mut self, ui: &mut Ui, k: &mut KeyObj, salt: &str, action: &str, changed: &mut bool) {
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = 16.0;
            match action {
                "hotkey" => {
                    labeled(ui, "Kombinasi tombol (klik lalu tekan)", |ui| {
                        let id = Id::new(("f", salt, "keys"));
                        let cur = sget(k, "keys");
                        let mut buf: String = ui.data_mut(|d| d.get_temp::<String>(id)).unwrap_or_else(|| cur.clone());
                        let focused = ui.memory(|m| m.has_focus(id));
                        if !focused {
                            buf = cur.clone();
                        } else if let Some(c) = capture_hotkey(ui) {
                            buf = c;
                            k.insert("keys".into(), json!(buf.clone()));
                            *changed = true;
                        }
                        let r = text_input(ui, id, &mut buf, 200.0, "", false, false);
                        ui.data_mut(|d| d.insert_temp(id, buf.clone()));
                        if r.lost_focus() && buf != cur {
                            k.insert("keys".into(), json!(buf));
                            *changed = true;
                        }
                    });
                }
                "vkey" => {
                    labeled(ui, "Tombol virtual", |ui| {
                        let mut v = sget(k, "vk");
                        let r = text_input(ui, Id::new(("vkin", salt)), &mut v, 130.0, "", false, true).on_hover_cursor(CursorIcon::PointingHand);
                        if r.clicked() {
                            let m = if v.starts_with('K') { 'K' } else { 'F' };
                            self.dd = Some(Dd { mode: DdMode::Vk, anchor: r.rect, m });
                        }
                    });
                }
                "web" => {
                    labeled(ui, "Alamat website", |ui| {
                        if field_text(ui, salt, k, "target", 260.0, "contoh: youtube.com") {
                            *changed = true;
                        }
                    });
                }
                "open" => {
                    labeled(ui, "Aplikasi (ketik untuk mencari)", |ui| {
                        let id = Id::new(("app", salt));
                        let name = sget(k, "appName");
                        let cur = if name.is_empty() { sget(k, "target") } else { name };
                        let mut buf: String = ui.data_mut(|d| d.get_temp::<String>(id)).unwrap_or_else(|| cur.clone());
                        let focused = ui.memory(|m| m.has_focus(id));
                        if !focused {
                            buf = cur.clone();
                        }
                        let r = text_input(ui, id, &mut buf, 260.0, "None", false, false);
                        ui.data_mut(|d| d.insert_temp(id, buf.clone()));
                        self.dd_filter = if r.has_focus() { buf.clone() } else { String::new() };
                        if r.gained_focus() || (r.has_focus() && self.dd.is_none()) {
                            self.dd = Some(Dd { mode: DdMode::List, anchor: r.rect, m: 'F' });
                        }
                        if let Some(dd) = &mut self.dd {
                            if dd.mode == DdMode::List {
                                dd.anchor = r.rect;
                            }
                        }
                                        if r.lost_focus() {
                            let over_pop = ui.input(|i| i.pointer.hover_pos()).map(|p| self.dd_rect(ui.ctx().screen_rect()).map(|r| r.contains(p)).unwrap_or(false)).unwrap_or(false);
                            if !over_pop {
                                self.dd = None;
                                if !self.apps.iter().any(|a| a.0 == buf) && buf != cur {
                                    k.insert("target".into(), json!(buf));
                                    k.insert("appName".into(), json!(""));
                                    *changed = true;
                                }
                            }
                        }
                    });
                }
                "text" => {
                    labeled(ui, "Teks yang diketik", |ui| {
                        if field_text(ui, salt, k, "text", 260.0, "") {
                            *changed = true;
                        }
                    });
                }
                "media" => {
                    labeled(ui, "Aksi", |ui| {
                        let opts: Vec<(String, String)> = [("playpause", "Play / Pause"), ("next", "Lagu berikutnya"), ("prev", "Lagu sebelumnya"), ("volup", "Volume +"), ("voldown", "Volume -"), ("mute", "Mute")]
                            .iter()
                            .map(|(a, b)| (a.to_string(), b.to_string()))
                            .collect();
                        if let Some(v) = select(ui, &format!("media{salt}"), 200.0, &sget(k, "key"), &opts) {
                            k.insert("key".into(), json!(v));
                            *changed = true;
                        }
                    });
                }
                "nav" => {
                    labeled(ui, "Pindah ke", |ui| {
                        let opts = vec![("next".to_string(), "Halaman berikutnya".to_string()), ("prev".to_string(), "Halaman sebelumnya".to_string())];
                        if let Some(v) = select(ui, &format!("nav{salt}"), 200.0, &sget(k, "to"), &opts) {
                            k.insert("to".into(), json!(v));
                            *changed = true;
                        }
                    });
                }
                "sound" => {
                    labeled(ui, "File audio", |ui| {
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 8.0;
                            let mut f = sget(k, "file");
                            let r = text_input(ui, Id::new(("snd", salt)), &mut f, 240.0, "", false, true);
                            r.on_hover_text(f.clone());
                            if Btn::b("Pilih file…").show(ui).clicked() {
                                let (p, s) = (self.page, self.sel.unwrap_or(0));
                                let mut kk = k.clone();
                                self.be.post("/pick/audio", None, move |a, v| {
                                    if let Some(file) = v.get("file").and_then(|x| x.as_str()) {
                                        kk.insert("file".into(), json!(file));
                                        a.be.send(msg_set(p, s, &Value::Object(kk)));
                                    }
                                });
                            }
                        });
                    });
                    labeled(ui, "Volume", |ui| {
                        ui.horizontal(|ui| {
                            let vol_s = sget(k, "vol");
                            let mut v: f32 = if vol_s.is_empty() { 80.0 } else { vol_s.parse().unwrap_or(80.0) };
                            let sid = Id::new(("vol", salt));
                            let live: Option<f32> = ui.data(|d| d.get_temp::<f32>(sid));
                            if let Some(l) = live {
                                v = l;
                            }
                            let (ch, rel) = slider(ui, &mut v, 0.0, 200.0, 140.0);
                            ui.add_sized([38.0, 20.0], Label::new(RichText::new(format!("{}%", v as i32)).font(reg(12.0)).color(LIGHT)));
                            if ch {
                                ui.data_mut(|d| d.insert_temp(sid, v));
                            }
                            if rel {
                                ui.data_mut(|d| d.remove::<f32>(sid));
                                k.insert("vol".into(), json!((v as i32).to_string()));
                                *changed = true;
                            }
                        });
                    });
                }
                _ => {}
            }
        });
    }

    // ---------------------------------------------------------------- dropdown (daftar aplikasi dan pilihan tombol virtual)
    fn dd_rect(&self, screen: Rect) -> Option<Rect> {
        let dd = self.dd.as_ref()?;
        let st = self.st.as_ref();
        let (w, h) = match dd.mode {
            DdMode::Vk => (300.0, 215.0),
            DdMode::List => {
                let n = self.dd_list().len().max(1).min(5) as f32;
                (dd.anchor.width().max(260.0), n * 34.0 + 4.0)
            }
        };
        let _ = st;
        let mut top = dd.anchor.bottom() + 4.0;
        if top + h > screen.bottom() - 4.0 {
            top = (dd.anchor.top() - h - 4.0).max(4.0);
        }
        let left = dd.anchor.left().min(screen.right() - w - 4.0).max(4.0);
        Some(Rect::from_min_size(pos2(left, top), vec2(w, h)))
    }

    fn dd_list(&self) -> Vec<(String, String, bool)> {
        let t = self.dd_filter.to_lowercase();
        let mut v = vec![("-1".to_string(), "None".to_string(), false)];
        for (i, a) in self.apps.iter().enumerate() {
            if t.is_empty() || a.0.to_lowercase().contains(&t) {
                v.push((i.to_string(), a.0.clone(), true));
            }
        }
        v
    }

    fn draw_dd(&mut self, ctx: &Context) {
        let screen = ctx.screen_rect();
        let rect = match self.dd_rect(screen) {
            Some(r) => r,
            None => return,
        };
        let mode = self.dd.as_ref().unwrap().mode;
        let mut pick: Option<String> = None;
        let mut new_m: Option<char> = None;
        let mut list_items: Vec<(String, String, bool)> = vec![];
        if mode == DdMode::List {
            list_items = self.dd_list();
        }
        let m = self.dd.as_ref().unwrap().m;
        let vk_items: Vec<String> = if mode == DdMode::Vk {
            let (cur, page, sel) = (self.st.as_ref().map(|s| s.cur).unwrap_or(0), self.page as i64, self.sel.map(|s| s as i64).unwrap_or(-1));
            let used: Vec<String> = self.st.as_ref().map(|s| s.used.iter().filter(|u| u.pi == cur && !(u.pg == page && u.ki == sel)).map(|u| u.vk.clone()).collect()).unwrap_or_default();
            all_vk().into_iter().filter(|v| !used.contains(v) && v.starts_with(m)).collect()
        } else {
            vec![]
        };
        let icons: Vec<Option<TextureId>> = list_items.iter().map(|(k, _, _)| k.parse::<usize>().ok().and_then(|i| self.app_icon(ctx, i))).collect();
        Area::new(Id::new("ddp")).order(Order::Tooltip).fixed_pos(rect.min).show(ctx, |ui| {
            let (r, _) = ui.allocate_exact_size(rect.size(), Sense::hover());
            ui.painter().rect(r, 8.0, HB, Stroke::new(1.0, INPUT_BORDER));
            let mut inner = ui.child_ui(r.shrink(1.0), Layout::top_down(Align::LEFT), None);
            if mode == DdMode::List {
                ScrollArea::vertical().max_height(r.height() - 2.0).auto_shrink([false, false]).show(&mut inner, |ui| {
                    ui.spacing_mut().item_spacing.y = 0.0;
                    if list_items.len() == 1 && self.apps.is_empty() {
                        ui.add_space(0.0);
                    }
                    for (i, (k, label, _)) in list_items.iter().enumerate() {
                        let (rr, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 34.0), Sense::click());
                        if resp.hovered() {
                            ui.painter().rect_filled(rr, 0.0, Color32::from_rgb(0x3d, 0x40, 0x48));
                        }
                        let mut x = rr.left() + 10.0;
                        if let Some(Some(t)) = icons.get(i) {
                            ui.painter().image(*t, Rect::from_min_size(pos2(x, rr.center().y - 10.0), Vec2::splat(20.0)), Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
                        }
                        x += 28.0;
                        ui.painter().with_clip_rect(rr).text(pos2(x, rr.center().y), Align2::LEFT_CENTER, label, reg(13.0), TEXT);
                        if resp.clicked() {
                            pick = Some(k.clone());
                        }
                    }
                });
            } else {
                inner.add_space(8.0);
                inner.horizontal(|ui| {
                    ui.add_space(10.0);
                    if let Some(v) = seg(ui, &[("F", "F"), ("K", "K")], &m.to_string(), 44.0, 26.0) {
                        new_m = v.chars().next();
                    }
                });
                inner.add_space(10.0);
                ScrollArea::vertical().max_height(150.0).auto_shrink([false, false]).show(&mut inner, |ui| {
                    ui.add_space(0.0);
                    if vk_items.is_empty() {
                        ui.horizontal(|ui| {
                            ui.add_space(10.0);
                            ui.label(RichText::new(format!("Semua tombol {} sudah terpakai", m)).color(MUTED));
                        });
                    }
                    for row in vk_items.chunks(4) {
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 8.0;
                            ui.add_space(10.0);
                            for v in row {
                                if Btn::b(v).w((r.width() - 20.0 - 24.0) / 4.0).h(36.0).font(bold(14.0)).hover(ACCENT).show(ui).clicked() {
                                    pick = Some(v.clone());
                                }
                            }
                        });
                        ui.add_space(0.0);
                    }
                });
            }
        });
        if let Some(m) = new_m {
            if let Some(dd) = &mut self.dd {
                dd.m = m;
            }
        }
        if let Some(p) = pick {
            self.on_pick(&p);
            self.dd = None;
        }
        // klik di luar daftar menutupnya (blur di versi web)
        if self.dd.is_some() && ctx.input(|i| i.pointer.any_pressed()) {
            if let Some(pos) = ctx.input(|i| i.pointer.interact_pos()) {
                let anchor = self.dd.as_ref().unwrap().anchor;
                if !rect.contains(pos) && !anchor.contains(pos) {
                    self.dd = None;
                }
            }
        }
    }

    fn on_pick(&mut self, key: &str) {
        let (sel, page) = (self.sel, self.page);
        let cur = sel.and_then(|s| self.st.as_ref().and_then(|st| st.pages.get(page).and_then(|p| p.get(&s).cloned())));
        let mut cur = match cur {
            Some(c) => c,
            None => return,
        };
        let a = sget(&cur, "action");
        if a == "vkey" {
            cur.insert("vk".into(), json!(key));
        } else if a == "open" {
            if key == "-1" {
                cur.insert("target".into(), json!(""));
                cur.insert("appName".into(), json!(""));
            } else if let Some((n, p)) = key.parse::<usize>().ok().and_then(|i| self.apps.get(i)) {
                cur.insert("target".into(), json!(p));
                cur.insert("appName".into(), json!(n));
            }
        }
        self.set_key(&cur);
    }

    // ---------------------------------------------------------------- daftar aksi di kanan
    fn draw_side(&mut self, ui: &mut Ui) {
        let groups: [(&str, [(&str, &str); 6]); 3] = [
            ("Sistem", [("hotkey", "Hotkey"), ("vkey", "Tombol virtual (F dan K)"), ("open", "Buka aplikasi"), ("web", "Buka website"), ("text", "Ketik teks"), ("media", "Multimedia")]),
            ("Soundboard", [("sound", "Putar audio"), ("stopsound", "Hentikan audio"), ("", ""), ("", ""), ("", ""), ("", "")]),
            ("Navigasi", [("next", "Halaman berikutnya"), ("prev", "Halaman sebelumnya"), ("", ""), ("", ""), ("", ""), ("", "")]),
        ];
        ui.spacing_mut().item_spacing.y = 0.0;
        for (g, items) in groups.iter() {
            let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 39.0), Sense::hover());
            ui.painter().rect_filled(r, 0.0, HB);
            ui.painter().text(pos2(r.left() + 14.0, r.center().y), Align2::LEFT_CENTER, g, bold(14.0), TEXT);
            for (id, name) in items.iter().filter(|(i, _)| !i.is_empty()) {
                let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 39.0), Sense::click_and_drag());
                if resp.hovered() {
                    ui.painter().rect_filled(r, 0.0, Color32::from_rgb(0x34, 0x36, 0x3b));
                    ui.ctx().set_cursor_icon(CursorIcon::Grab);
                }
                ui.painter().text(pos2(r.left() + 24.0, r.center().y), Align2::LEFT_CENTER, name, reg(14.0), LIGHT);
                if resp.drag_started() {
                    resp.dnd_set_drag_payload(Payload::Lib(id.to_string()));
                }
                if resp.double_clicked() {
                    if let Some(s) = self.sel {
                        let d = self.mkdef(id);
                        self.be.send(json!({ "t": "set", "p": self.page, "k": s, "key": d }));
                    }
                }
            }
        }
    }

    fn draw_ghost(&mut self, ctx: &Context) {
        if let Some(pl) = DragAndDrop::payload::<Payload>(ctx) {
            if let Some(pos) = ctx.input(|i| i.pointer.hover_pos()) {
                let text = match &*pl {
                    Payload::Lib(id) => self.mkdef(id)["label"].as_str().unwrap_or("").to_string(),
                    Payload::Key(i) => self.st.as_ref().and_then(|s| s.pages.get(self.page).and_then(|p| p.get(i)).map(|k| sget(k, "label"))).unwrap_or_default(),
                };
                Area::new(Id::new("ghost")).order(Order::Tooltip).fixed_pos(pos + vec2(14.0, 14.0)).interactable(false).show(ctx, |ui| {
                    Frame::none().fill(ACCENT).rounding(8.0).inner_margin(Margin::symmetric(10.0, 5.0)).show(ui, |ui| {
                        ui.label(RichText::new(text).font(bold(12.0)).color(Color32::WHITE));
                    });
                });
            }
        }
    }

    // ---------------------------------------------------------------- versi + update
    fn draw_version(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            ui.label(RichText::new(&self.version).font(reg(12.0)).color(MUTED));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let en = !self.up.disabled;
                if link(ui, &self.up.btn.clone(), en).clicked() {
                    if self.up.new.is_some() {
                        self.up_ask();
                    } else {
                        self.up_check(false);
                    }
                }
            });
        });
    }

    pub fn up_check(&mut self, auto: bool) {
        if self.up.busy {
            return;
        }
        self.up.disabled = true;
        if !auto {
            self.up.btn = "Memeriksa…".into();
        }
        self.be.post("/update/check", None, move |a, r| {
            a.up.disabled = false;
            let state = r.get("state").and_then(|s| s.as_str()).unwrap_or("error").to_string();
            if state == "new" {
                a.up.btn = format!("Update ke v{}", r.get("latest").and_then(|x| x.as_str()).unwrap_or(""));
                a.up.new = Some(r);
                a.up_ask();
                return;
            }
            a.up.new = None;
            a.up.btn = match state.as_str() {
                "latest" => "Sudah versi terbaru".into(),
                _ if auto => "Cek update".into(),
                "norelease" => "Belum ada rilis".into(),
                "norepo" => "Repo belum diatur".into(),
                _ => "Gagal memeriksa".into(),
            };
        });
    }

    pub fn up_ask(&mut self) {
        if self.up.busy {
            return;
        }
        if let Some(n) = &self.up.new {
            let v = n.get("latest").and_then(|x| x.as_str()).unwrap_or("");
            let text = format!("Versi {} sudah tersedia. Update sekarang? Aplikasi akan ditutup, dipasang otomatis, lalu terbuka lagi.", v);
            self.ask_confirm("Update tersedia", &text, "Update", ACCENT, Act::Update);
        }
    }

    pub fn up_go(&mut self) {
        let n = match self.up.new.clone() {
            Some(n) => n,
            None => return,
        };
        if !n.get("asset").and_then(|a| a.as_bool()).unwrap_or(false) {
            self.be.post("/update/open", Some(json!({ "url": n.get("url").cloned().unwrap_or(Value::Null) })), |_, _| {});
            return;
        }
        self.up.busy = true;
        self.up.disabled = true;
        self.up.btn = "Mengunduh 0%".into();
        self.be.post("/update/install", None, |a, r| a.up_state(if r.is_null() { json!({ "state": "error" }) } else { r }));
    }

    fn up_state(&mut self, r: Value) {
        match r.get("state").and_then(|s| s.as_str()).unwrap_or("error") {
            "down" => {
                self.up.btn = format!("Mengunduh {}%", r.get("pct").and_then(|p| p.as_i64()).unwrap_or(0));
                let be = self.be.clone();
                std::thread::spawn(move || {
                    std::thread::sleep(Duration::from_millis(400));
                    be.get("/update/status", |a, r| a.up_state(if r.is_null() { json!({ "state": "ready" }) } else { r }));
                });
            }
            "ready" => self.up.btn = "Memasang…".into(),
            _ => {
                self.up.busy = false;
                self.up.disabled = false;
                self.up.btn = "Gagal, coba lagi".into();
                self.up.new = None;
            }
        }
    }

    pub fn exec_confirm(&mut self, a: Act) {
        match a {
            Act::DelPage(p) => self.be.send(json!({ "t": "delPage", "p": p })),
            Act::DelProf(i) => self.be.send(json!({ "t": "delProf", "i": i })),
            Act::Update => self.up_go(),
        }
    }

    fn draw_toast(&mut self, ctx: &Context) {
        if let Some((t, at)) = &self.toast {
            let age = at.elapsed().as_secs_f32();
            if age > 2.4 {
                self.toast = None;
                return;
            }
            let a = if age < 2.2 { 1.0 } else { 1.0 - (age - 2.2) / 0.2 };
            let text = t.clone();
            Area::new(Id::new("toast")).order(Order::Tooltip).anchor(Align2::CENTER_BOTTOM, vec2(0.0, -210.0)).interactable(false).show(ctx, |ui| {
                Frame::none().fill(B_BG.gamma_multiply(a)).rounding(10.0).inner_margin(Margin::symmetric(14.0, 8.0)).show(ui, |ui| {
                    ui.label(RichText::new(text).font(reg(13.0)).color(TEXT.gamma_multiply(a)));
                });
            });
            ctx.request_repaint_after(Duration::from_millis(50));
        }
    }
}

pub fn all_vk() -> Vec<String> {
    (13..25).map(|i| format!("F{i}")).chain((1..37).map(|i| format!("K{i}"))).collect()
}

/// Kotak teks yang menyimpan nilainya dulu, lalu dikirim saat fokus lepas (event "change" di HTML)
pub fn field_text(ui: &mut Ui, salt: &str, k: &mut KeyObj, name: &str, w: f32, hint: &str) -> bool {
    let id = Id::new(("f", salt, name));
    let cur = sget(k, name);
    let mut buf: String = ui.data_mut(|d| d.get_temp::<String>(id)).unwrap_or_else(|| cur.clone());
    if !ui.memory(|m| m.has_focus(id)) {
        buf = cur.clone();
    }
    let r = text_input(ui, id, &mut buf, w, hint, false, false);
    ui.data_mut(|d| d.insert_temp(id, buf.clone()));
    if r.lost_focus() && buf != cur {
        k.insert(name.into(), json!(buf));
        return true;
    }
    false
}

/// Menangkap kombinasi tombol di kotak hotkey: Ctrl/Alt + tombol, atau tombol F
fn capture_hotkey(ui: &mut Ui) -> Option<String> {
    ui.input_mut(|i| {
        let mut out = None;
        let evs = std::mem::take(&mut i.events);
        let mut keep = Vec::with_capacity(evs.len());
        for e in evs {
            if let Event::Key { key, pressed: true, modifiers, .. } = &e {
                let name = format!("{:?}", key);
                let is_f = name.len() >= 2 && name.starts_with('F') && name[1..].chars().all(|c| c.is_ascii_digit());
                if modifiers.ctrl || modifiers.alt || is_f {
                    let sp = match key {
                        egui::Key::Enter => "{ENTER}",
                        egui::Key::Escape => "{ESC}",
                        egui::Key::Tab => "{TAB}",
                        egui::Key::Backspace => "{BACKSPACE}",
                        egui::Key::Delete => "{DEL}",
                        egui::Key::Home => "{HOME}",
                        egui::Key::End => "{END}",
                        egui::Key::ArrowUp => "{UP}",
                        egui::Key::ArrowDown => "{DOWN}",
                        egui::Key::ArrowLeft => "{LEFT}",
                        egui::Key::ArrowRight => "{RIGHT}",
                        egui::Key::PageUp => "{PGUP}",
                        egui::Key::PageDown => "{PGDN}",
                        _ => "",
                    };
                    let tail = if is_f {
                        format!("{{{}}}", name)
                    } else if !sp.is_empty() {
                        sp.to_string()
                    } else {
                        let s = key.symbol_or_name();
                        if s.chars().count() == 1 { s.to_lowercase() } else if *key == egui::Key::Space { " ".into() } else { String::new() }
                    };
                    out = Some(format!("{}{}{}{}", if modifiers.ctrl { "^" } else { "" }, if modifiers.shift { "+" } else { "" }, if modifiers.alt { "%" } else { "" }, tail));
                    continue;
                }
            }
            keep.push(e);
        }
        i.events = keep;
        out
    })
}

fn decode_tex(ctx: &Context, name: &str, bytes: &[u8], crt: bool) -> Option<TexEntry> {
    fn filt(px: &mut [u8]) {
        // filter: contrast(1.2) saturate(1.35), untuk tombol bergambar dengan filter CRT
        for c in px.chunks_exact_mut(4) {
            let mut v = [c[0] as f32 / 255.0, c[1] as f32 / 255.0, c[2] as f32 / 255.0];
            for x in v.iter_mut() {
                *x = ((*x - 0.5) * 1.2 + 0.5).clamp(0.0, 1.0);
            }
            let g = 0.2126 * v[0] + 0.7152 * v[1] + 0.0722 * v[2];
            for (i, x) in v.iter().enumerate() {
                c[i] = ((g + (x - g) * 1.35).clamp(0.0, 1.0) * 255.0) as u8;
            }
        }
    }
    if bytes.starts_with(b"GIF8") {
        use image::AnimationDecoder;
        let dec = image::codecs::gif::GifDecoder::new(std::io::Cursor::new(bytes)).ok()?;
        let frames = dec.into_frames().collect_frames().ok()?;
        let mut out = vec![];
        let mut total = 0.0;
        let mut size = [1, 1];
        for (i, f) in frames.into_iter().enumerate() {
            let (n, d) = f.delay().numer_denom_ms();
            let secs = (n as f64 / d.max(1) as f64 / 1000.0).max(0.02);
            let mut buf = f.into_buffer();
            let (w, h) = buf.dimensions();
            size = [w as usize, h as usize];
            if crt {
                filt(&mut buf);
            }
            let t = ctx.load_texture(format!("{name}#{i}"), ColorImage::from_rgba_unmultiplied(size, buf.as_raw()), TextureOptions::LINEAR);
            total += secs;
            out.push((t, secs));
        }
        if out.is_empty() {
            return None;
        }
        if out.len() == 1 {
            let (t, _) = out.remove(0);
            return Some(TexEntry::Still(t, size));
        }
        return Some(TexEntry::Anim(out, total, size));
    }
    let mut im = image::load_from_memory(bytes).ok()?.to_rgba8();
    let (w, h) = im.dimensions();
    if crt {
        filt(&mut im);
    }
    let size = [w as usize, h as usize];
    Some(TexEntry::Still(ctx.load_texture(name.to_string(), ColorImage::from_rgba_unmultiplied(size, im.as_raw()), TextureOptions::LINEAR), size))
}

impl eframe::App for App {
    fn update(&mut self, ctx: &Context, _f: &mut eframe::Frame) {
        let jobs = std::mem::take(&mut *self.be.inbox.lock().unwrap());
        for j in jobs {
            j(self);
        }
        if ctx.input(|i| i.viewport().close_requested()) {
            ctx.send_viewport_cmd(ViewportCommand::CancelClose);
            crate::shell::quit();
        }
        if !self.up_started && self.started.elapsed() > Duration::from_millis(2500) {
            self.up_started = true;
            self.up_check(true);
        }

        // Esc: tutup dialog teratas (konfirmasi dulu, lalu jendela lain)
        let esc = ctx.input(|i| i.key_pressed(egui::Key::Escape));
        if esc {
            if self.confirm.is_some() {
                self.confirm = None;
            } else if self.name_dlg.is_some() {
                self.name_dlg = None;
            } else if self.avoid.open {
                self.avoid.open = false;
            } else if self.audio.open {
                self.close_audio();
            } else if self.pair.open {
                self.close_pair();
            } else if self.vk.open {
                self.vk.open = false;
            } else if self.obs.open {
                self.close_obs();
            }
        }

        SidePanel::right("side").exact_width(280.0).resizable(false).frame(Frame::none().fill(SIDE)).show(ctx, |ui| {
            let r = ui.max_rect();
            ui.painter().vline(r.left(), r.y_range(), Stroke::new(1.0, BORDER));
            TopBottomPanel::bottom("ver").exact_height(40.0).frame(Frame::none().fill(SIDE).inner_margin(Margin::symmetric(14.0, 10.0))).show_inside(ui, |ui| {
                ui.painter().hline(ui.max_rect().x_range().expand(14.0), ui.max_rect().top() - 10.0, Stroke::new(1.0, BORDER));
                self.draw_version(ui);
            });
            ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| self.draw_side(ui));
        });
        TopBottomPanel::bottom("panel").exact_height(190.0).frame(Frame::none().fill(PANEL).inner_margin(Margin::symmetric(24.0, 8.0))).show(ctx, |ui| {
            ui.painter().hline(ui.max_rect().x_range().expand(24.0), ui.max_rect().top() - 8.0, Stroke::new(1.0, BORDER));
            ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| self.draw_panel(ui));
        });
        TopBottomPanel::top("top").frame(Frame::none().fill(BG).inner_margin(Margin { left: 24.0, right: 24.0, top: 16.0, bottom: 4.0 })).show(ctx, |ui| self.draw_header(ui));
        CentralPanel::default().frame(Frame::none().fill(BG)).show(ctx, |ui| self.draw_stage(ui));

        self.draw_dd(ctx);
        self.draw_modals(ctx);
        self.draw_ghost(ctx);
        self.draw_toast(ctx);
        self.draw_vpad(ctx);
        self.tick(ctx);
    }
}
