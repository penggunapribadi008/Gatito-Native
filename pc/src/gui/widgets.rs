// Komponen tampilan: warna, font, ikon, tombol, slider, kolom isian, dialog. Meniru CSS editor.html.
use eframe::egui::epaint::{Mesh, Vertex};
use eframe::egui::*;

pub const BG: Color32 = Color32::from_rgb(0x1e, 0x1f, 0x22);
pub const SIDE: Color32 = Color32::from_rgb(0x26, 0x27, 0x29);
pub const PANEL: Color32 = Color32::from_rgb(0x23, 0x24, 0x28);
pub const BORDER: Color32 = Color32::from_rgb(0x34, 0x36, 0x3d);
pub const HB: Color32 = Color32::from_rgb(0x2d, 0x2e, 0x31);
pub const HB_HOVER: Color32 = Color32::from_rgb(0x38, 0x3a, 0x3f);
pub const TEXT: Color32 = Color32::from_rgb(0xe8, 0xe9, 0xed);
pub const MUTED: Color32 = Color32::from_rgb(0x8b, 0x90, 0xa0);
pub const LIGHT: Color32 = Color32::from_rgb(0xc9, 0xcc, 0xd6);
pub const ACCENT: Color32 = Color32::from_rgb(0x4f, 0x6d, 0xf5);
pub const INPUT_BG: Color32 = Color32::from_rgb(0x14, 0x15, 0x18);
pub const INPUT_BORDER: Color32 = Color32::from_rgb(0x3c, 0x3f, 0x48);
pub const B_BG: Color32 = Color32::from_rgb(0x34, 0x36, 0x3d);
pub const DANGER_BG: Color32 = Color32::from_rgb(0x3a, 0x2a, 0x2e);
pub const DANGER_FG: Color32 = Color32::from_rgb(0xff, 0x7a, 0x8a);
pub const DANGER: Color32 = Color32::from_rgb(0xd6, 0x45, 0x5d);
pub const SEL: Color32 = Color32::from_rgb(0x7d, 0xa2, 0xff);
pub const GREEN: Color32 = Color32::from_rgb(0x6f, 0xd0, 0x8c);
pub const DOT_OK: Color32 = Color32::from_rgb(0x46, 0xc2, 0x7d);
pub const ORANGE: Color32 = Color32::from_rgb(0xe0, 0xa2, 0x3a);
pub const RED: Color32 = Color32::from_rgb(0xe5, 0x53, 0x4b);
pub const LINK: Color32 = Color32::from_rgb(0x8f, 0xa2, 0xff);

pub fn bold(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("bold".into()))
}
pub fn reg(size: f32) -> FontId {
    FontId::new(size, FontFamily::Proportional)
}

pub fn parse_hex(s: &str) -> Color32 {
    let h = s.trim().trim_start_matches('#');
    if h.len() == 6 {
        if let Ok(v) = u32::from_str_radix(h, 16) {
            return Color32::from_rgb((v >> 16) as u8, (v >> 8) as u8, v as u8);
        }
    }
    ACCENT
}
pub fn to_hex(c: [u8; 3]) -> String {
    format!("#{:02x}{:02x}{:02x}", c[0], c[1], c[2])
}

/// Font Segoe UI dari Windows (sama dengan editor lama), warna dan ukuran dasar
pub fn setup(ctx: &Context) {
    let mut fonts = FontDefinitions::default();
    let base = fonts.families.get(&FontFamily::Proportional).cloned().unwrap_or_default();
    if let Ok(b) = std::fs::read(r"C:\Windows\Fonts\segoeui.ttf") {
        fonts.font_data.insert("segoe".into(), FontData::from_owned(b).into());
        fonts.families.get_mut(&FontFamily::Proportional).unwrap().insert(0, "segoe".into());
    }
    let mut bf = base.clone();
    if let Ok(b) = std::fs::read(r"C:\Windows\Fonts\segoeuib.ttf") {
        fonts.font_data.insert("segoeb".into(), FontData::from_owned(b).into());
        bf.insert(0, "segoeb".into());
    }
    fonts.families.insert(FontFamily::Name("bold".into()), bf);
    ctx.set_fonts(fonts);

    let mut v = Visuals::dark();
    v.panel_fill = BG;
    v.window_fill = SIDE;
    v.extreme_bg_color = INPUT_BG;
    v.widgets.noninteractive.fg_stroke.color = TEXT;
    v.widgets.inactive.fg_stroke.color = TEXT;
    v.widgets.hovered.fg_stroke.color = TEXT;
    v.widgets.active.fg_stroke.color = TEXT;
    for w in [&mut v.widgets.inactive, &mut v.widgets.hovered, &mut v.widgets.active] {
        w.rounding = Rounding::same(8.0);
        w.bg_stroke = Stroke::new(1.0, INPUT_BORDER);
        w.weak_bg_fill = INPUT_BG;
        w.bg_fill = INPUT_BG;
    }
    v.widgets.hovered.weak_bg_fill = Color32::from_rgb(0x1a, 0x1b, 0x1f);
    v.selection.stroke = Stroke::new(1.0, Color32::from_rgb(0x5b, 0x60, 0x70));
    v.selection.bg_fill = Color32::from_rgba_unmultiplied(0x4f, 0x6d, 0xf5, 120);
    v.window_rounding = Rounding::same(8.0);
    v.window_stroke = Stroke::new(1.0, INPUT_BORDER);
    v.popup_shadow = Shadow::NONE;
    ctx.set_visuals(v);
    ctx.style_mut(|s| {
        s.spacing.item_spacing = vec2(8.0, 6.0);
        s.spacing.interact_size.y = 32.0;
        s.spacing.button_padding = vec2(8.0, 6.0);
        s.spacing.scroll = style::ScrollStyle::floating();
        s.text_styles.insert(TextStyle::Body, reg(14.0));
        s.text_styles.insert(TextStyle::Button, reg(14.0));
        s.text_styles.insert(TextStyle::Small, reg(12.0));
    });
}

// ---------------------------------------------------------------- ikon (SVG 24x24 dari editor.html)
#[derive(Clone, Copy)]
pub enum Icon {
    Trash,
    Pencil,
    Mic,
    Keyboard,
    Qr,
    Upload,
}

pub fn paint_icon(p: &Painter, rect: Rect, icon: Icon, color: Color32) {
    let s = rect.width().min(rect.height()) / 24.0;
    let o = rect.center() - vec2(12.0 * s, 12.0 * s);
    let pt = |x: f32, y: f32| o + vec2(x * s, y * s);
    let st = Stroke::new(2.0 * s.max(0.75), color);
    let line = |a: Pos2, b: Pos2| p.line_segment([a, b], st);
    match icon {
        Icon::Trash => {
            line(pt(3.0, 6.0), pt(21.0, 6.0));
            p.add(Shape::line(vec![pt(8.0, 6.0), pt(8.0, 4.0), pt(16.0, 4.0), pt(16.0, 6.0)], st));
            p.add(Shape::line(vec![pt(6.0, 6.0), pt(7.0, 20.0), pt(17.0, 20.0), pt(18.0, 6.0)], st));
            line(pt(10.0, 11.0), pt(10.0, 17.0));
            line(pt(14.0, 11.0), pt(14.0, 17.0));
        }
        Icon::Pencil => {
            line(pt(12.0, 20.0), pt(21.0, 20.0));
            p.add(Shape::closed_line(vec![pt(16.5, 3.5), pt(19.5, 6.5), pt(7.0, 19.0), pt(3.0, 20.0), pt(4.0, 16.0)], st));
        }
        Icon::Mic => {
            p.rect_stroke(Rect::from_min_size(pt(9.0, 3.0), vec2(6.0 * s, 11.0 * s)), Rounding::same(3.0 * s), st);
            let arc: Vec<Pos2> = (0..=16).map(|i| {
                let t = std::f32::consts::PI * i as f32 / 16.0;
                pt(12.0 + 7.0 * t.cos(), 11.0 + 7.0 * t.sin())
            }).collect();
            p.add(Shape::line(arc, st));
            line(pt(12.0, 18.0), pt(12.0, 21.0));
            line(pt(9.0, 21.0), pt(15.0, 21.0));
        }
        Icon::Keyboard => {
            p.rect_stroke(Rect::from_min_size(pt(2.0, 6.0), vec2(20.0 * s, 12.0 * s)), Rounding::same(2.5 * s), st);
            for x in [6.0, 10.0, 14.0, 18.0] {
                p.circle_filled(pt(x, 10.0), 1.0 * s, color);
            }
            line(pt(7.0, 14.0), pt(17.0, 14.0));
        }
        Icon::Qr => {
            for (x, y) in [(3.0, 3.0), (14.0, 3.0), (3.0, 14.0)] {
                p.rect_stroke(Rect::from_min_size(pt(x, y), vec2(7.0 * s, 7.0 * s)), Rounding::same(1.2 * s), st);
            }
            p.add(Shape::closed_line(vec![pt(14.0, 14.0), pt(17.0, 14.0), pt(17.0, 17.0), pt(14.0, 17.0)], st));
            line(pt(20.0, 14.0), pt(20.0, 17.0));
            line(pt(17.0, 20.0), pt(21.0, 20.0));
        }
        Icon::Upload => {
            line(pt(12.0, 16.0), pt(12.0, 4.0));
            p.add(Shape::line(vec![pt(7.0, 9.0), pt(12.0, 4.0), pt(17.0, 9.0)], st));
            line(pt(4.0, 20.0), pt(20.0, 20.0));
        }
    }
}

// ---------------------------------------------------------------- tombol
pub struct Btn<'a> {
    pub text: &'a str,
    pub w: Option<f32>,
    pub h: f32,
    pub fill: Color32,
    pub hover: Option<Color32>,
    pub fg: Color32,
    pub hover_fg: Option<Color32>,
    pub font: FontId,
    pub round: f32,
    pub enabled: bool,
    pub pad: f32,
}

impl<'a> Btn<'a> {
    /// Tombol ".b": latar #34363d, radius 8, padding 9x12
    pub fn b(text: &'a str) -> Self {
        Btn { text, w: None, h: 36.0, fill: B_BG, hover: None, fg: TEXT, hover_fg: None, font: reg(14.0), round: 8.0, enabled: true, pad: 12.0 }
    }
    /// Tombol kecil ".sb": tinggi 30, font 12
    pub fn sb(text: &'a str) -> Self {
        Btn { text, w: None, h: 30.0, fill: HB, hover: Some(HB_HOVER), fg: TEXT, hover_fg: None, font: reg(12.0), round: 8.0, enabled: true, pad: 10.0 }
    }
    pub fn fill(mut self, c: Color32) -> Self {
        self.fill = c;
        self
    }
    pub fn w(mut self, w: f32) -> Self {
        self.w = Some(w);
        self
    }
    pub fn h(mut self, h: f32) -> Self {
        self.h = h;
        self
    }
    pub fn enabled(mut self, e: bool) -> Self {
        self.enabled = e;
        self
    }
    pub fn fg(mut self, c: Color32) -> Self {
        self.fg = c;
        self
    }
    pub fn font(mut self, f: FontId) -> Self {
        self.font = f;
        self
    }
    pub fn round(mut self, r: f32) -> Self {
        self.round = r;
        self
    }
    pub fn hover(mut self, c: Color32) -> Self {
        self.hover = Some(c);
        self
    }
    pub fn danger_hover(mut self) -> Self {
        self.hover = Some(DANGER_BG);
        self.hover_fg = Some(DANGER_FG);
        self
    }
    pub fn show(self, ui: &mut Ui) -> Response {
        let galley = ui.painter().layout_no_wrap(self.text.to_string(), self.font.clone(), self.fg);
        let w = self.w.unwrap_or(galley.size().x + self.pad * 2.0);
        let (rect, resp) = ui.allocate_exact_size(vec2(w, self.h), if self.enabled { Sense::click() } else { Sense::hover() });
        let hov = self.enabled && resp.hovered();
        let fill = if hov { self.hover.unwrap_or(self.fill) } else { self.fill };
        let fg = if hov { self.hover_fg.unwrap_or(self.fg) } else { self.fg };
        let alpha = if self.enabled { 1.0 } else { 0.4 };
        ui.painter().rect_filled(rect, self.round, fill.gamma_multiply(alpha));
        let g = ui.painter().layout_no_wrap(self.text.to_string(), self.font, fg.gamma_multiply(alpha));
        ui.painter().galley(rect.center() - g.size() / 2.0, g, fg);
        if hov {
            ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
        }
        resp
    }
}

/// Tombol berisi ikon (".hb.sq", ".sb.sq", "#trash", ".b.ib")
pub fn icon_btn(ui: &mut Ui, size: f32, icon: Icon, fill: Color32, hover: Option<Color32>, danger: bool, enabled: bool, round: f32) -> Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(size, size), if enabled { Sense::click() } else { Sense::hover() });
    let hov = enabled && resp.hovered();
    let (f, fg) = if hov && danger { (DANGER_BG, DANGER_FG) } else if hov { (hover.unwrap_or(fill), TEXT) } else { (fill, if danger { LIGHT } else { TEXT }) };
    let a = if enabled { 1.0 } else { 0.4 };
    ui.painter().rect_filled(rect, round, f.gamma_multiply(a));
    let ir = Rect::from_center_size(rect.center(), Vec2::splat(if size >= 40.0 { 22.0 } else if size >= 34.0 { 20.0 } else { 16.0 }));
    paint_icon(ui.painter(), ir, icon, fg.gamma_multiply(a));
    if hov {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    resp
}

// ---------------------------------------------------------------- slider (input[type=range])
/// Mengembalikan (berubah, dilepas). Nilai dibulatkan ke bilangan bulat.
pub fn slider(ui: &mut Ui, v: &mut f32, min: f32, max: f32, width: f32) -> (bool, bool) {
    let (rect, resp) = ui.allocate_exact_size(vec2(width, 22.0), Sense::click_and_drag());
    let mut changed = false;
    if resp.dragged() || resp.clicked() || resp.drag_started() {
        if let Some(pos) = resp.interact_pointer_pos() {
            let pf = ((pos.x - rect.left() - 7.0) / (rect.width() - 14.0)).clamp(0.0, 1.0);
            let nv = (min + pf * (max - min)).round();
            if (nv - *v).abs() > f32::EPSILON {
                *v = nv;
                changed = true;
            }
        }
    }
    let pf = ((*v - min) / (max - min).max(1.0)).clamp(0.0, 1.0);
    let cy = rect.center().y;
    let track = Rect::from_min_max(pos2(rect.left(), cy - 3.0), pos2(rect.right(), cy + 3.0));
    ui.painter().rect_filled(track, 3.0, INPUT_BORDER);
    let x = rect.left() + 7.0 + (rect.width() - 14.0) * pf;
    ui.painter().rect_filled(Rect::from_min_max(track.min, pos2(x, track.max.y)), 3.0, ACCENT);
    let thumb = if resp.hovered() { Color32::from_rgb(0xdf, 0xe5, 0xff) } else { Color32::WHITE };
    ui.painter().circle_filled(pos2(x, cy + 1.0), 7.0, Color32::from_black_alpha(90));
    ui.painter().circle_filled(pos2(x, cy), 7.0, thumb);
    (changed, resp.drag_stopped() || resp.clicked())
}

// ---------------------------------------------------------------- isian dan pilihan
fn input_scope<R>(ui: &mut Ui, f: impl FnOnce(&mut Ui) -> R) -> R {
    ui.scope(|ui| {
        ui.spacing_mut().interact_size.y = 32.0;
        f(ui)
    })
    .inner
}

pub fn text_input(ui: &mut Ui, id: Id, buf: &mut String, w: f32, hint: &str, password: bool, readonly: bool) -> Response {
    if readonly {
        // Kotak hanya-baca (input readonly): digambar sendiri supaya tetap bisa diklik
        let (rect, resp) = ui.allocate_exact_size(vec2(w, 33.0), Sense::click());
        ui.painter().rect(rect, 8.0, INPUT_BG, Stroke::new(1.0, INPUT_BORDER));
        let clip = rect.shrink2(vec2(8.0, 0.0));
        let g = ui.painter().layout_no_wrap(buf.clone(), reg(14.0), TEXT);
        ui.painter().with_clip_rect(clip).galley(pos2(clip.left(), rect.center().y - g.size().y / 2.0), g, TEXT);
        return resp;
    }
    input_scope(ui, |ui| {
        let mut te = TextEdit::singleline(buf).id(id).desired_width(w).margin(Margin::symmetric(8.0, 6.0)).font(reg(14.0)).password(password);
        if !hint.is_empty() {
            te = te.hint_text(RichText::new(hint).color(MUTED));
        }
        ui.add_sized([w, 33.0], te)
    })
}

/// Kotak pilihan (select). Mengembalikan nilai yang dipilih kalau berubah.
pub fn select(ui: &mut Ui, id: &str, w: f32, current: &str, opts: &[(String, String)]) -> Option<String> {
    let mut chosen = None;
    let text = opts.iter().find(|(v, _)| v == current).map(|(_, t)| t.clone()).unwrap_or_default();
    input_scope(ui, |ui| {
        ComboBox::from_id_salt(id).width(w).selected_text(RichText::new(text).font(reg(14.0))).show_ui(ui, |ui| {
            ui.set_min_width(w - 16.0);
            for (v, t) in opts {
                if ui.selectable_label(v == current, t).clicked() {
                    chosen = Some(v.clone());
                }
            }
        });
    });
    chosen
}

/// Label kecil di atas isian ("label{display:grid;gap:4px;font-size:12px;color:#8b90a0}")
pub fn labeled<R>(ui: &mut Ui, label: &str, f: impl FnOnce(&mut Ui) -> R) -> R {
    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing.y = 4.0;
        ui.label(RichText::new(label).font(reg(12.0)).color(MUTED));
        f(ui)
    })
    .inner
}

pub fn dot_line(ui: &mut Ui, color: Color32, f: impl FnOnce(&mut Ui)) {
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = vec2(8.0, 2.0);
        let (r, _) = ui.allocate_exact_size(vec2(10.0, 14.0), Sense::hover());
        ui.painter().circle_filled(pos2(r.center().x, r.center().y + 1.0), 5.0, color);
        f(ui);
    });
}

pub fn dot_text(ui: &mut Ui, color: Color32, text: &str) {
    dot_line(ui, color, |ui| {
        ui.add(Label::new(RichText::new(text).font(reg(13.0))).wrap());
    });
}

pub fn link(ui: &mut Ui, text: &str, enabled: bool) -> Response {
    let color = if enabled { LINK } else { MUTED };
    let r = ui.add(Label::new(RichText::new(text).font(reg(12.0)).color(color)).sense(if enabled { Sense::click() } else { Sense::hover() }));
    if enabled && r.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
        let rect = r.rect;
        ui.painter().line_segment([pos2(rect.left(), rect.bottom() - 1.0), pos2(rect.right(), rect.bottom() - 1.0)], Stroke::new(1.0, color));
    }
    r
}

/// Segmen F / K / Terpakai (".seg")
pub fn seg(ui: &mut Ui, items: &[(&str, &str)], cur: &str, btn_w: f32, h: f32) -> Option<String> {
    let mut chosen = None;
    Frame::none().fill(BG).rounding(8.0).inner_margin(2.0).show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            for (k, label) in items {
                let on = *k == cur;
                let galley = ui.painter().layout_no_wrap(label.to_string(), bold(14.0), MUTED);
                let w = btn_w.max(galley.size().x + 20.0);
                let r = Btn::b(label).w(w).h(h).fill(if on { ACCENT } else { Color32::TRANSPARENT }).fg(if on { Color32::WHITE } else { MUTED }).font(bold(14.0)).round(6.0).show(ui);
                if r.clicked() {
                    chosen = Some(k.to_string());
                }
            }
        });
    });
    chosen
}

// ---------------------------------------------------------------- dialog
/// Dialog dengan latar gelap. Mengembalikan (diklik_di_luar, hasil).
pub fn modal<R>(ctx: &Context, id: &str, width: f32, top: bool, add: impl FnOnce(&mut Ui) -> R) -> (bool, R) {
    let order = if top { Order::Tooltip } else { Order::Foreground };
    let outside = Area::new(Id::new((id, "scrim"))).order(order).fixed_pos(Pos2::ZERO).show(ctx, |ui| {
        let r = ctx.screen_rect();
        ui.painter().rect_filled(r, 0.0, Color32::from_black_alpha(153));
        ui.allocate_rect(r, Sense::click()).clicked()
    });
    let res = Area::new(Id::new((id, "box"))).order(order).anchor(Align2::CENTER_CENTER, Vec2::ZERO).show(ctx, |ui| {
        Frame::none()
            .fill(SIDE)
            .rounding(16.0)
            .inner_margin(24.0)
            .show(ui, |ui| {
                ui.set_width(width - 48.0);
                add(ui)
            })
            .inner
    });
    (outside.inner, res.inner)
}

pub fn modal_title(ui: &mut Ui, text: &str) {
    ui.label(RichText::new(text).font(bold(18.0)));
}

// ---------------------------------------------------------------- gambar bersudut bulat (CSS overflow:hidden + border-radius)
pub fn rounded_mesh(p: &Painter, tex: TextureId, rect: Rect, r: f32, uv: Rect, tint: Color32) {
    let mut pts: Vec<Pos2> = vec![];
    let n = 8;
    let corners = [
        (pos2(rect.left() + r, rect.top() + r), std::f32::consts::PI),
        (pos2(rect.right() - r, rect.top() + r), 1.5 * std::f32::consts::PI),
        (pos2(rect.right() - r, rect.bottom() - r), 0.0),
        (pos2(rect.left() + r, rect.bottom() - r), 0.5 * std::f32::consts::PI),
    ];
    for (c, a0) in corners {
        for i in 0..=n {
            let a = a0 + 0.5 * std::f32::consts::PI * i as f32 / n as f32;
            pts.push(c + vec2(a.cos(), a.sin()) * r);
        }
    }
    let uvof = |q: Pos2| {
        let t = (q - rect.min) / rect.size();
        pos2(uv.min.x + t.x * uv.width(), uv.min.y + t.y * uv.height())
    };
    let mut m = Mesh::with_texture(tex);
    m.vertices.push(Vertex { pos: rect.center(), uv: uvof(rect.center()), color: tint });
    for q in &pts {
        m.vertices.push(Vertex { pos: *q, uv: uvof(*q), color: tint });
    }
    let k = pts.len() as u32;
    for i in 0..k {
        m.indices.extend_from_slice(&[0, 1 + i, 1 + (i + 1) % k]);
    }
    p.add(Shape::mesh(m));
}

/// Tekstur overlay filter CRT (garis pindai + vignette), n x n piksel
pub fn crt_image(n: usize) -> ColorImage {
    let mut px = vec![Color32::TRANSPARENT; n * n];
    let c = (n as f32 - 1.0) / 2.0;
    for y in 0..n {
        for x in 0..n {
            let scan = if (n - 1 - y) % 3 == 0 { 0.35 } else { 0.0 };
            let d = (((x as f32 - c) / c).powi(2) + ((y as f32 - c) / c).powi(2)).sqrt() / std::f32::consts::SQRT_2;
            let t = ((d - 0.55) / 0.45).clamp(0.0, 1.0);
            let vig = 0.55 * t;
            let a = 1.0 - (1.0 - scan) * (1.0 - vig);
            px[y * n + x] = Color32::from_rgba_premultiplied(0, 0, 0, (a * 255.0) as u8);
        }
    }
    ColorImage { size: [n, n], pixels: px }
}
