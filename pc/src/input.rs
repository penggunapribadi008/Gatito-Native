// Menjalankan aksi tombol: hotkey, tombol virtual (F13-F24), ketik teks, multimedia, buka aplikasi atau website.
// Versi Electron memakai PowerShell yang tetap hidup; di sini langsung memakai API Windows (keybd_event / SendInput).
// Semua perintah lewat satu thread supaya urutannya tetap sama dengan urutan tombol ditekan.
use crate::state::Key;
use once_cell::sync::Lazy;
use regex::Regex;
use std::sync::mpsc::{channel, Sender};
use std::sync::Mutex;

pub enum Cmd {
    VKey(u32, String),
    SendKeys(String),
    Text(String),
    Media(u8),
    Open(String),
}

static TX: Lazy<Mutex<Sender<Cmd>>> = Lazy::new(|| {
    let (tx, rx) = channel::<Cmd>();
    std::thread::Builder::new()
        .name("gatito-input".into())
        .spawn(move || {
            while let Ok(c) = rx.recv() {
                match c {
                    Cmd::VKey(v, m) => kb::vkey(v, &m),
                    Cmd::SendKeys(s) => kb::send_keys(&s),
                    Cmd::Text(t) => kb::type_text(&t),
                    Cmd::Media(v) => kb::media(v),
                    Cmd::Open(t) => crate::util::open_target(&t),
                }
            }
        })
        .ok();
    Mutex::new(tx)
});

pub fn submit(c: Cmd) {
    let _ = TX.lock().unwrap().send(c);
}

static F_RX: Lazy<Regex> = Lazy::new(|| Regex::new(r"^F(\d+)$").unwrap());
static K_RX: Lazy<Regex> = Lazy::new(|| Regex::new(r"^K(\d+)$").unwrap());
static SCHEME_RX: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?i)^[a-z][a-z0-9+.\-]*://").unwrap());

pub fn media_vk(name: &str) -> Option<u8> {
    Some(match name {
        "volup" => 175,
        "voldown" => 174,
        "mute" => 173,
        "playpause" => 179,
        "next" => 176,
        "prev" => 177,
        _ => return None,
    })
}

fn sv<'a>(k: &'a Key, n: &str) -> &'a str {
    k.get(n).and_then(|v| v.as_str()).unwrap_or("")
}

/// Volume tombol 0 sampai 200. Angka 0 harus tetap 0 (bukan jatuh ke 80)
pub fn vol_of(v: &str) -> f32 {
    let n: i64 = v.trim().parse::<f64>().map(|x| x as i64).unwrap_or(80);
    n.clamp(0, 200) as f32 / 100.0
}

/// Mengembalikan Some((vk, mod)) kalau tombol virtual valid. vk 124..135 = F13..F24
pub fn vkey_of(k: &Key) -> Option<(u32, String)> {
    let vk = sv(k, "vk");
    let mut n: u32 = 0;
    let mut modi = String::new();
    if let Some(c) = F_RX.captures(vk) {
        n = c[1].parse().unwrap_or(0);
    } else if let Some(c) = K_RX.captures(vk) {
        let kk: u32 = c[1].parse().unwrap_or(0);
        if (1..=36).contains(&kk) {
            let i = kk - 1;
            n = 13 + (i % 12);
            modi = ["cs", "ca", "sa"][(i / 12) as usize].to_string();
        }
    }
    if (13..=24).contains(&n) {
        let m = if modi.is_empty() { sv(k, "mod").to_string() } else { modi };
        Some((111 + n, m))
    } else {
        None
    }
}

pub fn run(ctx: &crate::app::Ctx, k: &Key, id: &str) {
    let a = sv(k, "action");
    match a {
        "hotkey" if !sv(k, "keys").is_empty() => submit(Cmd::SendKeys(sv(k, "keys").to_string())),
        "vkey" => {
            if let Some((v, m)) = vkey_of(k) {
                submit(Cmd::VKey(v, m));
            }
        }
        "open" | "web" if !sv(k, "target").is_empty() => {
            let mut t = sv(k, "target").to_string();
            if a == "web" && !SCHEME_RX.is_match(&t) {
                t = format!("https://{}", t);
            }
            submit(Cmd::Open(t));
        }
        "text" if !sv(k, "text").is_empty() => submit(Cmd::Text(sv(k, "text").to_string())),
        "media" => {
            if let Some(v) = media_vk(sv(k, "key")) {
                submit(Cmd::Media(v));
            }
        }
        "sound" if !sv(k, "file").is_empty() => ctx.engine.play(id.to_string(), sv(k, "file").to_string(), vol_of(sv(k, "vol"))),
        "stopsound" => ctx.engine.stop(),
        _ => {}
    }
}

#[cfg(windows)]
mod kb {
    use std::thread::sleep;
    use std::time::Duration;
    use windows::Win32::UI::Input::KeyboardAndMouse::*;

    fn ev(vk: u8, scan: u8, flags: KEYBD_EVENT_FLAGS) {
        unsafe { keybd_event(vk, scan, flags, 0) };
    }
    const UP: KEYBD_EVENT_FLAGS = KEYEVENTF_KEYUP;
    const NONE: KEYBD_EVENT_FLAGS = KEYBD_EVENT_FLAGS(0);
    const EXT: KEYBD_EVENT_FLAGS = KEYEVENTF_EXTENDEDKEY;

    // Kode scan ikut dikirim (F13 sampai F23 = 0x64 sampai 0x6E, F24 = 0x76), supaya aplikasi selain OBS (Discord, game, browser) juga mengenali tombolnya
    fn sdsc(v: u32) -> u8 {
        if v == 135 {
            118
        } else {
            (100 + v as i32 - 124) as u8
        }
    }
    fn sdms(v: u8) -> u8 {
        match v {
            17 => 29,
            16 => 42,
            _ => 56,
        }
    }

    pub fn vkey(v: u32, m: &str) {
        let mut ks: Vec<u8> = vec![];
        if m.contains('c') {
            ks.push(17);
        }
        if m.contains('s') {
            ks.push(16);
        }
        if m.contains('a') {
            ks.push(18);
        }
        for x in &ks {
            ev(*x, sdms(*x), NONE);
        }
        let s = sdsc(v);
        ev(v as u8, s, NONE);
        sleep(Duration::from_millis(120));
        ev(v as u8, s, UP);
        for x in &ks {
            ev(*x, sdms(*x), UP);
        }
    }

    pub fn media(vk: u8) {
        ev(vk, 0, EXT);
        ev(vk, 0, EXT | UP);
    }

    #[derive(Clone, Copy, Default)]
    struct Mods {
        ctrl: bool,
        shift: bool,
        alt: bool,
    }

    fn scan_of(vk: u8) -> u8 {
        unsafe { MapVirtualKeyW(vk as u32, MAPVK_VK_TO_VSC) as u8 }
    }
    fn mods_down(m: Mods) {
        if m.ctrl {
            ev(17, scan_of(17), NONE);
        }
        if m.shift {
            ev(16, scan_of(16), NONE);
        }
        if m.alt {
            ev(18, scan_of(18), NONE);
        }
    }
    fn mods_up(m: Mods) {
        if m.alt {
            ev(18, scan_of(18), UP);
        }
        if m.shift {
            ev(16, scan_of(16), UP);
        }
        if m.ctrl {
            ev(17, scan_of(17), UP);
        }
    }
    fn tap(vk: u8, ext: bool) {
        let f = if ext { EXT } else { NONE };
        let s = scan_of(vk);
        ev(vk, s, f);
        ev(vk, s, f | UP);
    }

    fn named(name: &str) -> Option<(u8, bool)> {
        let n = name.to_uppercase();
        if let Some(num) = n.strip_prefix('F').and_then(|x| x.parse::<u8>().ok()) {
            if (1..=24).contains(&num) {
                return Some((0x6F + num, false));
            }
        }
        Some(match n.as_str() {
            "ENTER" => (0x0D, false),
            "ESC" | "ESCAPE" => (0x1B, false),
            "TAB" => (0x09, false),
            "BACKSPACE" | "BS" | "BKSP" => (0x08, false),
            "DELETE" | "DEL" => (0x2E, true),
            "INSERT" | "INS" => (0x2D, true),
            "HOME" => (0x24, true),
            "END" => (0x23, true),
            "UP" => (0x26, true),
            "DOWN" => (0x28, true),
            "LEFT" => (0x25, true),
            "RIGHT" => (0x27, true),
            "PGUP" => (0x21, true),
            "PGDN" => (0x22, true),
            "CAPSLOCK" => (0x14, false),
            "NUMLOCK" => (0x90, true),
            "SCROLLLOCK" => (0x91, false),
            "PRTSC" => (0x2C, true),
            "BREAK" => (0x03, false),
            "HELP" => (0x2F, false),
            "SPACE" => (0x20, false),
            _ => return None,
        })
    }

    /// Ketik satu karakter: pakai tombol keyboard kalau ada, kalau tidak kirim sebagai Unicode
    fn char_key(c: char, m: Mods, rep: u32) {
        let mut mm = m;
        let r = unsafe { VkKeyScanW(c as u16) };
        if r == -1 {
            for _ in 0..rep {
                unicode(c);
            }
            return;
        }
        let vk = (r & 0xFF) as u8;
        let st = ((r >> 8) & 0xFF) as u8;
        if st & 1 != 0 {
            mm.shift = true;
        }
        if st & 2 != 0 {
            mm.ctrl = true;
        }
        if st & 4 != 0 {
            mm.alt = true;
        }
        for _ in 0..rep {
            mods_down(mm);
            tap(vk, false);
            mods_up(mm);
        }
    }

    fn unicode(c: char) {
        let mut buf = [0u16; 2];
        for u in c.encode_utf16(&mut buf).iter().copied() {
            let down = INPUT {
                r#type: INPUT_KEYBOARD,
                Anonymous: INPUT_0 { ki: KEYBDINPUT { wVk: VIRTUAL_KEY(0), wScan: u, dwFlags: KEYEVENTF_UNICODE, time: 0, dwExtraInfo: 0 } },
            };
            let up = INPUT {
                r#type: INPUT_KEYBOARD,
                Anonymous: INPUT_0 { ki: KEYBDINPUT { wVk: VIRTUAL_KEY(0), wScan: u, dwFlags: KEYEVENTF_UNICODE | KEYEVENTF_KEYUP, time: 0, dwExtraInfo: 0 } },
            };
            unsafe { SendInput(&[down, up], std::mem::size_of::<INPUT>() as i32) };
        }
    }

    /// Ketik teks apa adanya. Baris baru jadi Enter.
    pub fn type_text(t: &str) {
        let cs: Vec<char> = t.chars().collect();
        let mut i = 0;
        while i < cs.len() {
            match cs[i] {
                '\r' => {
                    if i + 1 < cs.len() && cs[i + 1] == '\n' {
                        i += 1;
                    }
                    tap(0x0D, false);
                }
                '\n' => tap(0x0D, false),
                c => unicode(c),
            }
            i += 1;
        }
    }

    /// Penafsir sintaks SendKeys: ^ ctrl, + shift, % alt, ~ enter, ( ) kelompok, {NAMA} tombol khusus, {NAMA n} diulang n kali
    pub fn send_keys(s: &str) {
        let cs: Vec<char> = s.chars().collect();
        let mut i = 0;
        seq(&cs, &mut i, false);
    }

    fn seq(cs: &[char], i: &mut usize, group: bool) {
        while *i < cs.len() {
            let mut m = Mods::default();
            while *i < cs.len() && matches!(cs[*i], '^' | '+' | '%') {
                match cs[*i] {
                    '^' => m.ctrl = true,
                    '+' => m.shift = true,
                    _ => m.alt = true,
                }
                *i += 1;
            }
            if *i >= cs.len() {
                break;
            }
            let c = cs[*i];
            if c == ')' && group {
                *i += 1;
                return;
            }
            if c == '(' {
                *i += 1;
                mods_down(m);
                seq(cs, i, true);
                mods_up(m);
            } else if c == '~' {
                *i += 1;
                mods_down(m);
                tap(0x0D, false);
                mods_up(m);
            } else if c == '{' {
                // {NAMA}, {NAMA n}, atau karakter khusus seperti {+} {^} {%} {~} {(} {)} {[} {]} {{} {}}
                let mut j = *i + 1;
                if j < cs.len() && cs[j] == '}' {
                    // "{}}" = tanda } literal
                    j += 1;
                }
                while j < cs.len() && cs[j] != '}' {
                    j += 1;
                }
                let body: String = cs[(*i + 1)..j.min(cs.len())].iter().collect();
                *i = (j + 1).min(cs.len());
                let mut parts = body.splitn(2, ' ');
                let name = parts.next().unwrap_or("").to_string();
                let rep: u32 = parts.next().and_then(|n| n.trim().parse().ok()).unwrap_or(1).clamp(1, 200);
                if let Some((vk, ext)) = named(&name) {
                    for _ in 0..rep {
                        mods_down(m);
                        tap(vk, ext);
                        mods_up(m);
                    }
                } else if name.chars().count() == 1 || name == "}" {
                    char_key(name.chars().next().unwrap_or('}'), m, rep);
                }
            } else {
                *i += 1;
                char_key(c, m, 1);
            }
            if group {
                // modifier hanya berlaku untuk satu butir; di dalam kelompok sudah dipasang di luar
            }
        }
    }
}

#[cfg(not(windows))]
mod kb {
    pub fn vkey(_v: u32, _m: &str) {}
    pub fn media(_v: u8) {}
    pub fn type_text(_t: &str) {}
    pub fn send_keys(_s: &str) {}
}
