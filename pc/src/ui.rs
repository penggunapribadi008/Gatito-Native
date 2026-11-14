// Jendela: editor utama (WebView2 memuat http://127.0.0.1:PORT/editor), jendela Tombol virtual, dan ikon tray
use tao::dpi::LogicalSize;
use tao::event_loop::{EventLoopProxy, EventLoopWindowTarget};
use tao::window::{Icon, Window, WindowBuilder};
use wry::{WebView, WebViewBuilder};

#[derive(Debug)]
pub enum UserEvent {
    ShowMain,
    OpenVPad,
    CloseVPad,
    DragVpad,
    Quit,
    Tray(tray_icon::TrayIconEvent),
    Menu(tray_icon::menu::MenuEvent),
}

const ICON_PNG: &[u8] = include_bytes!("../assets/icon256.png");

pub fn icon_rgba(size: u32) -> (Vec<u8>, u32, u32) {
    let im = image::load_from_memory(ICON_PNG).expect("ikon").resize_exact(size, size, image::imageops::FilterType::Lanczos3).to_rgba8();
    let (w, h) = im.dimensions();
    (im.into_raw(), w, h)
}

pub fn window_icon() -> Option<Icon> {
    let (d, w, h) = icon_rgba(64);
    Icon::from_rgba(d, w, h).ok()
}

pub fn create_main(t: &EventLoopWindowTarget<UserEvent>, port: u16) -> (Window, WebView) {
    let win = WindowBuilder::new()
        .with_title("Gatito: Stream Tool")
        .with_inner_size(LogicalSize::new(1000.0, 760.0))
        .with_min_inner_size(LogicalSize::new(860.0, 600.0))
        .with_window_icon(window_icon())
        .build(t)
        .expect("jendela utama");
    let wv = WebViewBuilder::new()
        .with_url(format!("http://127.0.0.1:{}/editor", port))
        .with_background_color((30, 31, 34, 255))
        .build(&win)
        .expect("webview utama");
    (win, wv)
}

pub fn create_vpad(t: &EventLoopWindowTarget<UserEvent>, port: u16, proxy: EventLoopProxy<UserEvent>) -> (Window, WebView) {
    use tao::platform::windows::WindowBuilderExtWindows;
    let win = WindowBuilder::new()
        .with_title("Tombol virtual")
        .with_inner_size(LogicalSize::new(340.0, 240.0))
        .with_decorations(false)
        .with_always_on_top(true)
        .with_focusable(false)
        .with_resizable(false)
        .with_skip_taskbar(true)
        .with_window_icon(window_icon())
        .build(t)
        .expect("jendela tombol virtual");
    let wv = WebViewBuilder::new()
        .with_url(format!("http://127.0.0.1:{}/vpad", port))
        .with_background_color((30, 31, 34, 255))
        .with_ipc_handler(move |req| {
            if req.body() == "drag" {
                let _ = proxy.send_event(UserEvent::DragVpad);
            }
        })
        .build(&win)
        .expect("webview tombol virtual");
    (win, wv)
}
