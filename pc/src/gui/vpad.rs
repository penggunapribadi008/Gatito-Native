// Jendela kecil "Tombol virtual" yang tetap di atas OBS dan mengirim tombol langsung (mode mengambang)
use super::widgets::*;
use super::*;

pub struct VpadState {
    pub m: char,
    pub noact: bool,
}
impl Default for VpadState {
    fn default() -> Self {
        VpadState { m: 'F', noact: false }
    }
}

impl App {
    pub fn draw_vpad(&mut self, ctx: &Context) {
        if !crate::shell::VPAD.load(std::sync::atomic::Ordering::SeqCst) {
            self.vpad_ui.noact = false;
            return;
        }
        let vb = ViewportBuilder::default()
            .with_title(crate::shell::VPAD_TITLE)
            .with_inner_size([340.0, 240.0])
            .with_decorations(false)
            .with_always_on_top()
            .with_resizable(false);
        let mut close = false;
        ctx.show_viewport_immediate(ViewportId::from_hash_of("vpad"), vb, |ctx, _class| {
            if ctx.input(|i| i.viewport().close_requested()) {
                close = true;
            }
            CentralPanel::default().frame(Frame::none().fill(BG).stroke(Stroke::new(1.0, INPUT_BORDER))).show(ctx, |ui| {
                let full = ui.max_rect();
                // kepala: judul, saklar F/K, tombol tutup. Diseret untuk memindahkan jendela.
                let head = Rect::from_min_size(full.min, vec2(full.width(), 36.0));
                let drag = ui.interact(head, Id::new("vphead"), Sense::drag());
                if drag.drag_started() {
                    ctx.send_viewport_cmd(ViewportCommand::StartDrag);
                }
                ui.painter().rect_filled(head, 0.0, HB);
                ui.painter().text(pos2(head.left() + 12.0, head.center().y), Align2::LEFT_CENTER, "Tombol virtual", bold(13.0), TEXT);
                let sw = Rect::from_center_size(pos2(head.center().x, head.center().y), vec2(88.0, 28.0));
                let mut h = ui.child_ui(sw, Layout::left_to_right(Align::Center), None);
                if let Some(v) = seg(&mut h, &[("F", "F"), ("K", "K")], &self.vpad_ui.m.to_string(), 40.0, 24.0) {
                    self.vpad_ui.m = v.chars().next().unwrap_or('F');
                }
                let xr = Rect::from_center_size(pos2(head.right() - 21.0, head.center().y), vec2(30.0, 28.0));
                let xresp = ui.interact(xr, Id::new("vpx"), Sense::click());
                ui.painter().rect_filled(xr, 6.0, if xresp.hovered() { Color32::from_rgb(0xc9, 0x4c, 0x5a) } else { Color32::TRANSPARENT });
                ui.painter().text(xr.center(), Align2::CENTER_CENTER, "✕", reg(16.0), TEXT);
                if xresp.clicked() {
                    self.be.post_quiet("/vpad/close");
                }
                // isi
                let body = Rect::from_min_max(pos2(full.left() + 12.0, head.bottom() + 10.0), pos2(full.right() - 12.0, full.bottom() - 10.0));
                let mut b = ui.child_ui(body, Layout::top_down(Align::LEFT), None);
                b.label(RichText::new("Klik kotak hotkey di OBS, lalu tekan tombol di sini.").font(reg(13.0)).color(MUTED));
                b.add_space(10.0);
                let list: Vec<String> = if self.vpad_ui.m == 'F' { (13..25).map(|i| format!("F{i}")).collect() } else { (1..37).map(|i| format!("K{i}")).collect() };
                ScrollArea::vertical().max_height(136.0).auto_shrink([false, false]).show(&mut b, |ui| {
                    let w = (ui.available_width() - 24.0) / 4.0;
                    for row in list.chunks(4) {
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing = vec2(8.0, 8.0);
                            for v in row {
                                let hint = combo_hint(v);
                                let r = Btn::b(v).w(w).h(40.0).font(bold(13.0)).round(8.0).hover(ACCENT).show(ui).on_hover_text(hint);
                                if r.clicked() {
                                    self.be.send(json!({ "t": "vsend", "vk": v }));
                                }
                            }
                        });
                    }
                });
            });
            if !self.vpad_ui.noact {
                self.vpad_ui.noact = crate::shell::win::make_vpad_noactivate();
            }
        });
        if close {
            crate::shell::cmd(crate::shell::UiCmd::CloseVPad);
        }
    }
}

/// K1 sampai K36 bukan tombol keyboard sungguhan: diwujudkan sebagai F13-F24 + dua tombol pendamping
fn combo_hint(v: &str) -> String {
    let n: i32 = v[1..].parse().unwrap_or(0);
    if v.starts_with('F') {
        return v.to_string();
    }
    let i = n - 1;
    format!("{}+F{}", ["Ctrl+Shift", "Ctrl+Alt", "Shift+Alt"][(i / 12).clamp(0, 2) as usize], 13 + (i % 12))
}
