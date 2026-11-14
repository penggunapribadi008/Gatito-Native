// Fungsi kecil yang dipakai di banyak modul
use sha1::{Digest, Sha1};

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
/// "pemilik/repo" GitHub untuk cek update. Diisi CI lewat GITHUB_REPOSITORY saat kompilasi.
pub fn repo() -> &'static str {
    option_env!("GITHUB_REPOSITORY").unwrap_or("")
}

pub fn take(s: &str, n: usize) -> String {
    s.chars().take(n).collect()
}

pub fn sha1_hex(b: &[u8]) -> String {
    let mut h = Sha1::new();
    h.update(b);
    hex::encode(h.finalize())
}

pub fn hostname_str() -> String {
    hostname::get().ok().and_then(|h| h.into_string().ok()).unwrap_or_else(|| "PC".into())
}

/// IPv4 lokal non-loopback, alamat 192.168.x.x didahulukan
pub fn ips() -> Vec<String> {
    let mut v: Vec<String> = if_addrs::get_if_addrs()
        .map(|l| {
            l.into_iter()
                .filter(|i| !i.is_loopback())
                .filter_map(|i| match i.ip() {
                    std::net::IpAddr::V4(a) => Some(a.to_string()),
                    _ => None,
                })
                .collect()
        })
        .unwrap_or_default();
    v.sort_by_key(|a| if a.starts_with("192.168") { 0 } else { 1 });
    v
}

pub fn port_free(p: u16) -> bool {
    std::net::TcpListener::bind(("0.0.0.0", p)).is_ok()
}

/// Jalankan program tanpa memunculkan jendela konsol (Windows)
#[cfg(windows)]
pub fn no_window(cmd: &mut std::process::Command) -> &mut std::process::Command {
    use std::os::windows::process::CommandExt;
    cmd.creation_flags(0x0800_0000)
}
#[cfg(not(windows))]
pub fn no_window(cmd: &mut std::process::Command) -> &mut std::process::Command {
    cmd
}

/// Buka berkas, aplikasi, atau alamat web dengan program bawaan Windows
pub fn open_target(t: &str) {
    let mut c = std::process::Command::new(if cfg!(windows) { "explorer.exe" } else { "xdg-open" });
    c.arg(t);
    no_window(&mut c);
    let _ = c.spawn();
}

pub fn now_ms() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
}
