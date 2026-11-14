// Daftar aplikasi dari Start Menu dan ikonnya (untuk aksi "Buka aplikasi")
use crate::util::{no_window, sha1_hex};
use once_cell::sync::Lazy;
use regex::Regex;
use serde::Serialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

#[derive(Clone, Serialize, Debug)]
pub struct AppEntry {
    pub name: String,
    pub path: String,
}

static SKIP: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?i)uninstall|readme|help|documentation|license|[o0]ptional").unwrap());

pub fn list_apps() -> Vec<AppEntry> {
    let roots: Vec<PathBuf> = vec![
        PathBuf::from(std::env::var("ProgramData").unwrap_or_else(|_| "C:\\ProgramData".into())).join("Microsoft").join("Windows").join("Start Menu").join("Programs"),
        PathBuf::from(std::env::var("APPDATA").unwrap_or_default()).join("Microsoft").join("Windows").join("Start Menu").join("Programs"),
    ];
    let mut out: HashMap<String, AppEntry> = HashMap::new();
    fn walk(d: &Path, depth: u32, out: &mut HashMap<String, AppEntry>) {
        let es = match std::fs::read_dir(d) {
            Ok(e) => e,
            Err(_) => return,
        };
        for e in es.flatten() {
            let p = e.path();
            let name = e.file_name().to_string_lossy().to_string();
            if p.is_dir() {
                if depth < 3 {
                    walk(&p, depth + 1, out);
                }
            } else if name.to_lowercase().ends_with(".lnk") {
                let n = name[..name.len() - 4].to_string();
                if !SKIP.is_match(&n) {
                    out.insert(n.to_lowercase(), AppEntry { name: n, path: p.to_string_lossy().to_string() });
                }
            }
        }
    }
    for r in &roots {
        walk(r, 0, &mut out);
    }
    let mut v: Vec<AppEntry> = out.into_values().collect();
    v.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()).then(a.name.cmp(&b.name)));
    v
}

static EXTRACTING: AtomicBool = AtomicBool::new(false);

/// Ikon aplikasi diambil lewat proses PowerShell terpisah supaya aplikasi utama tidak ikut crash
pub fn extract_icons(list: &[AppEntry], dir: &Path) {
    if !cfg!(windows) || EXTRACTING.swap(true, Ordering::SeqCst) {
        return;
    }
    let _ = std::fs::create_dir_all(dir);
    let todo: Vec<&AppEntry> = list.iter().filter(|a| !dir.join(format!("{}.png", sha1_hex(a.path.as_bytes()))).exists()).collect();
    if todo.is_empty() {
        EXTRACTING.store(false, Ordering::SeqCst);
        return;
    }
    let txt: Vec<String> = todo.iter().map(|a| format!("{}|{}", sha1_hex(a.path.as_bytes()), a.path)).collect();
    if std::fs::write(dir.join("list.txt"), txt.join("\n")).is_err() {
        EXTRACTING.store(false, Ordering::SeqCst);
        return;
    }
    let d = dir.to_string_lossy().replace('\'', "''");
    let script = format!(
        "$ErrorActionPreference='SilentlyContinue';Add-Type -AssemblyName System.Drawing;$sh=New-Object -ComObject WScript.Shell;Get-Content -Encoding UTF8 '{d}\\list.txt' | ForEach-Object {{ $p=$_.Split('|',2); $t=$sh.CreateShortcut($p[1]).TargetPath; if(-not $t -or -not (Test-Path $t)){{$t=$p[1]}}; $ic=[System.Drawing.Icon]::ExtractAssociatedIcon($t); if($ic){{ $ic.ToBitmap().Save('{d}\\'+$p[0]+'.png',[System.Drawing.Imaging.ImageFormat]::Png) }} }}"
    );
    let mut c = std::process::Command::new("powershell");
    c.args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-Command", &script]);
    no_window(&mut c);
    match c.spawn() {
        Ok(mut ch) => {
            std::thread::spawn(move || {
                let _ = ch.wait();
                EXTRACTING.store(false, Ordering::SeqCst);
            });
        }
        Err(_) => EXTRACTING.store(false, Ordering::SeqCst),
    }
}
