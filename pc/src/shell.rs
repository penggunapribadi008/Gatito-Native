// Bagian sistem Windows di luar GUI: ikon tray, tampilkan/sembunyikan jendela utama, keluar dengan aman
use crate::app::Ctx;
use once_cell::sync::OnceCell;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

pub const MAIN_TITLE: &str = "Gatito: Stream Tool";
pub const VPAD_TITLE: &str = "Tombol virtual";

pub enum UiCmd {
    ShowMain,
    OpenVPad,
    CloseVPad,
    Quit,
}

static CTX: OnceCell<Arc<Ctx>> = OnceCell::new();
static EGUI: OnceCell<eframe::egui::Context> = OnceCell::new();
pub static VPAD: AtomicBool = AtomicBool::new(false);
static QUITTING: AtomicBool = AtomicBool::new(false);

pub fn init(ctx: Arc<Ctx>) {
    let _ = CTX.set(ctx);
}

pub fn set_egui(c: eframe::egui::Context) {
    let _ = EGUI.set(c);
}

fn repaint() {
    if let Some(c) = EGUI.get() {
        c.request_repaint();
    }
}

pub fn cmd(c: UiCmd) {
    match c {
        UiCmd::ShowMain => win::show_main(),
        UiCmd::OpenVPad => {
            VPAD.store(true, Ordering::SeqCst);
            repaint();
        }
        UiCmd::CloseVPad => {
            VPAD.store(false, Ordering::SeqCst);
            repaint();
        }
        UiCmd::Quit => quit(),
    }
}

/// Saat Gatito ditutup, kembalikan mic default Windows ke mic asli supaya Discord tidak kehilangan suara
pub fn quit() {
    if QUITTING.swap(true, Ordering::SeqCst) {
        return;
    }
    win::hide_all(); // jendela langsung hilang, pemulihan mic di bawah berjalan di belakang
    let c = match CTX.get() {
        Some(c) => c.clone(),
        None => std::process::exit(0),
    };
    std::thread::spawn(move || {
        let h = c.rt.clone();
        h.block_on(async {
            let _ = tokio::time::timeout(Duration::from_secs(6), c.auto_mic_off()).await;
        });
        std::process::exit(0);
    });
}

pub fn icon_rgba(size: u32) -> (Vec<u8>, u32, u32) {
    let png = include_bytes!("../assets/icon256.png");
    let im = image::load_from_memory(png).expect("ikon").resize_exact(size, size, image::imageops::FilterType::Lanczos3).to_rgba8();
    let (w, h) = im.dimensions();
    (im.into_raw(), w, h)
}

/// Ikon tray + menu (Buka Gatito, Keluar). Dibuat di thread utama sebelum GUI jalan; eventnya dibaca thread terpisah.
pub fn start_tray() -> Option<tray_icon::TrayIcon> {
    use tray_icon::menu::{Menu, MenuEvent, MenuItem};
    use tray_icon::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
    let menu = Menu::new();
    let open_item = MenuItem::new("Buka Gatito", true, None);
    let quit_item = MenuItem::new("Keluar", true, None);
    let _ = menu.append(&open_item);
    let _ = menu.append(&quit_item);
    let (open_id, quit_id) = (open_item.id().clone(), quit_item.id().clone());
    let (rgba, w, h) = icon_rgba(32);
    let tray = TrayIconBuilder::new()
        .with_tooltip(MAIN_TITLE)
        .with_icon(tray_icon::Icon::from_rgba(rgba, w, h).ok()?)
        .with_menu(Box::new(menu))
        .with_menu_on_left_click(false)
        .build()
        .ok()?;
    std::thread::spawn(move || {
        let te = TrayIconEvent::receiver();
        let me = MenuEvent::receiver();
        loop {
            crossbeam_channel::select! {
                recv(te) -> e => match e {
                    Ok(TrayIconEvent::DoubleClick { .. }) => win::show_main(),
                    Ok(TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. }) => win::show_main(),
                    Ok(_) => {}
                    Err(_) => break,
                },
                recv(me) -> e => match e {
                    Ok(m) => {
                        if m.id == open_id { win::show_main(); } else if m.id == quit_id { quit(); }
                    }
                    Err(_) => break,
                },
            }
        }
    });
    // Jendela yang diminimalkan disembunyikan ke tray (seperti versi Electron)
    std::thread::spawn(|| loop {
        std::thread::sleep(Duration::from_millis(300));
        win::hide_if_minimized();
    });
    Some(tray)
}

#[cfg(windows)]
pub mod win {
    use super::{MAIN_TITLE, VPAD_TITLE};
    use windows::core::{HSTRING, PCWSTR};
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::*;

    fn find(title: &str) -> Option<HWND> {
        let t = HSTRING::from(title);
        unsafe { FindWindowW(PCWSTR::null(), &t).ok() }
    }

    pub fn show_main() {
        if let Some(h) = find(MAIN_TITLE) {
            unsafe {
                let _ = ShowWindow(h, SW_SHOW);
                let _ = ShowWindow(h, SW_RESTORE);
                let _ = SetForegroundWindow(h);
            }
        }
    }

    pub fn hide_all() {
        for t in [MAIN_TITLE, VPAD_TITLE] {
            if let Some(h) = find(t) {
                unsafe {
                    let _ = ShowWindow(h, SW_HIDE);
                }
            }
        }
    }

    pub fn hide_if_minimized() {
        if let Some(h) = find(MAIN_TITLE) {
            unsafe {
                if IsIconic(h).as_bool() {
                    let _ = ShowWindow(h, SW_HIDE);
                }
            }
        }
    }

    /// Jendela tombol virtual tidak boleh merebut fokus (supaya tombol F13 dst. masuk ke OBS yang sedang aktif) dan tidak muncul di taskbar
    pub fn make_vpad_noactivate() -> bool {
        if let Some(h) = find(VPAD_TITLE) {
            unsafe {
                let old = GetWindowLongPtrW(h, GWL_EXSTYLE);
                SetWindowLongPtrW(h, GWL_EXSTYLE, old | WS_EX_NOACTIVATE.0 as isize | WS_EX_TOOLWINDOW.0 as isize);
            }
            return true;
        }
        false
    }
}

#[cfg(not(windows))]
pub mod win {
    pub fn show_main() {}
    pub fn hide_all() {}
    pub fn hide_if_minimized() {}
    pub fn make_vpad_noactivate() -> bool {
        true
    }
}
