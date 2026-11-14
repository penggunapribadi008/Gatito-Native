// Pemasangan VB-Cable otomatis (Windows): unduh dari vb-audio.com, pasang, jadikan mic default,
// ganti nama, dan kembalikan speaker default seperti semula.
// Bagian yang butuh hak administrator (pasang + ganti nama) dijalankan dalam SATU proses dengan satu kali izin UAC.
// Skripnya dicek hash SHA-256 sebelum dijalankan supaya tidak bisa diganti orang lain di tengah jalan.
// Skrip PowerShell di folder scripts/ dihasilkan langsung dari vbsetup.js versi Electron (isinya sama persis).
use base64::Engine as _;
use once_cell::sync::Lazy;
use regex::Regex;
use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Duration;

const LIB: &str = include_str!("../scripts/lib.ps1");
const ELEV_BODY: &str = include_str!("../scripts/elev_body.ps1");
const SNAP: &str = include_str!("../scripts/snap.ps1");
const USE_VIRTUAL: &str = include_str!("../scripts/use_virtual.ps1");
const RESTORE_IF_VIRT: &str = include_str!("../scripts/restore_if_virt.ps1");
const RESTORE: &str = include_str!("../scripts/restore.ps1");
const DEFAULTS_1: &str = include_str!("../scripts/defaults_1.ps1");
const DEFAULTS_4: &str = include_str!("../scripts/defaults_4.ps1");

static VIRT_RX: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?i)vb-audio|vb-output|cable (in|out)").unwrap());
static LABEL_PRE: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?i)^(Default|Communications) - ").unwrap());
static LABEL_USB: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?i)\s*\([0-9a-f]{4}:[0-9a-f]{4}\)\s*$").unwrap());

fn is_virt_name(n: &str) -> bool {
    VIRT_RX.is_match(n)
}

/// Label dari Chromium kadang berakhiran kode USB "(301a:7c19)" atau diawali "Default - ": buang supaya cocok dengan nama di Windows
pub fn clean_label(l: &str) -> String {
    let a = LABEL_PRE.replace(l, "");
    LABEL_USB.replace(&a, "").trim().to_string()
}

fn q(s: &str) -> String {
    format!("'{}'", s.replace('\'', "''"))
}

#[derive(Clone, Serialize, Default, Debug)]
pub struct Status {
    pub running: bool,
    pub msg: String,
    pub err: String,
    pub notes: Vec<String>,
    pub done: bool,
}

pub struct Vb {
    s: Mutex<Status>,
}

pub struct RunOpt {
    pub install: bool,
    pub mic: String,
    pub engine_mic: String,
}

pub struct RunOut {
    pub ok: bool,
    pub mic: String,
}

struct PsOut {
    code: i32,
    out: String,
}

fn enc(script: &str) -> String {
    let u: Vec<u8> = script.encode_utf16().flat_map(|c| c.to_le_bytes()).collect();
    base64::engine::general_purpose::STANDARD.encode(u)
}

async fn run_ps(script: &str, ms: u64) -> PsOut {
    if !cfg!(windows) {
        return PsOut { code: -1, out: String::new() };
    }
    let b64 = enc(&format!("[Console]::OutputEncoding=[Text.Encoding]::UTF8\n{}", script));
    let mut c = tokio::process::Command::new("powershell");
    c.args(["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-EncodedCommand", &b64]);
    c.stdin(std::process::Stdio::null()).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped());
    c.kill_on_drop(true);
    #[cfg(windows)]
    {
        c.creation_flags(0x0800_0000);
    }
    let child = match c.spawn() {
        Ok(ch) => ch,
        Err(_) => return PsOut { code: -1, out: String::new() },
    };
    match tokio::time::timeout(Duration::from_millis(ms), child.wait_with_output()).await {
        Ok(Ok(o)) => PsOut { code: o.status.code().unwrap_or(-1), out: String::from_utf8_lossy(&o.stdout).to_string() },
        _ => PsOut { code: -1, out: String::new() },
    }
}

fn last_json(s: &str) -> Option<Value> {
    let l = s.trim().lines().filter(|x| x.trim().starts_with('{')).last()?;
    serde_json::from_str(l.trim()).ok()
}

fn str_of(v: &Option<Value>, k: &str) -> String {
    v.as_ref().and_then(|j| j.get(k)).and_then(|x| x.as_str()).unwrap_or("").to_string()
}

/// Kode keluar: bit flag 1..32 untuk hasil kerja; 98 = hash skrip tidak cocok; 99 = UAC ditolak/gagal dijalankan.
/// Satu kali UAC: skrip ditulis ke berkas, hash-nya dicek di dalam proses administrator sebelum dijalankan
async fn run_elevated(install: bool, name: &str, work_dir: PathBuf) -> i32 {
    let text = format!("$install = {}\n$name = {}\n{}{}", if install { "$true" } else { "$false" }, q(name), LIB, ELEV_BODY);
    let _ = std::fs::create_dir_all(&work_dir);
    let rnd: [u8; 6] = rand::random();
    let file = work_dir.join(format!("elev-{}.ps1", hex::encode(rnd)));
    if std::fs::write(&file, text.as_bytes()).is_err() {
        return -1;
    }
    let hash = hex::encode(Sha256::digest(text.as_bytes()));
    let inner = format!(
        "$f={};$c=[IO.File]::ReadAllText($f,[Text.Encoding]::UTF8);$sha=[Security.Cryptography.SHA256]::Create();$h=(($sha.ComputeHash([Text.Encoding]::UTF8.GetBytes($c))|ForEach-Object{{$_.ToString('x2')}}) -join '');if($h -ne {}){{exit 98}};Invoke-Expression $c;exit $code",
        q(&file.to_string_lossy()),
        q(&hash)
    );
    let outer = format!(
        "$a = {}\ntry {{ $p = Start-Process -FilePath powershell.exe -Verb RunAs -ArgumentList $a -Wait -PassThru -WindowStyle Hidden; exit $p.ExitCode }} catch {{ exit 99 }}",
        q(&format!("-NoProfile -NonInteractive -ExecutionPolicy Bypass -EncodedCommand {}", enc(&inner)))
    );
    let code = run_ps(&outer, 8 * 60 * 1000).await.code;
    let _ = std::fs::remove_file(&file);
    code
}

impl Vb {
    pub fn new() -> Vb {
        Vb { s: Mutex::new(Status::default()) }
    }

    pub fn status(&self) -> Status {
        self.s.lock().unwrap().clone()
    }

    fn set<F: FnOnce(&mut Status)>(&self, f: F) {
        f(&mut self.s.lock().unwrap());
    }

    /// opt: install = unduh dan pasang VB-Cable, mic = label mic asli yang sudah dipilih, engine_mic = label dari mesin audio
    pub async fn run(&self, opt: RunOpt) -> RunOut {
        if self.status().running {
            return RunOut { ok: false, mic: String::new() };
        }
        *self.s.lock().unwrap() = Status { running: true, msg: "Memeriksa pengaturan suara Windows…".into(), ..Default::default() };
        let mut out = RunOut { ok: false, mic: String::new() };
        let res: Result<(), String> = async {
            if !cfg!(windows) {
                return Err("Fitur ini hanya untuk Windows.".to_string());
            }
            let sn = last_json(&run_ps(&format!("{}{}", LIB, SNAP), 60000).await.out);
            let cname = str_of(&sn, "cname");
            let real_mic = if !opt.mic.is_empty() {
                opt.mic.clone()
            } else if !cname.is_empty() && !is_virt_name(&cname) {
                cname.clone()
            } else if !opt.engine_mic.is_empty() {
                opt.engine_mic.clone()
            } else {
                "Microphone".to_string()
            };
            let name: String = format!("{} (VB-Output)", real_mic).chars().filter(|c| (*c as u32) >= 0x20).take(120).collect();

            self.set(|s| {
                s.msg = if opt.install {
                    "Menunggu izin Windows (klik Ya), lalu VB-Cable diunduh dan dipasang. Kalau muncul jendela \"Windows Security\", klik Install.".into()
                } else {
                    "Menunggu izin Windows (klik Ya)…".into()
                }
            });
            let work = PathBuf::from(std::env::var("ProgramData").unwrap_or_else(|_| std::env::temp_dir().to_string_lossy().to_string())).join("GatitoVB");
            let code = run_elevated(opt.install, &name, work).await;

            let mut bad: Vec<&str> = vec![];
            if code == 99 {
                bad.push("Izin administrator dibatalkan atau ditolak. Klik tombolnya lagi lalu pilih Ya.");
            } else if code == 98 {
                bad.push("Pemeriksaan keamanan skrip gagal, proses dihentikan.");
            } else if code < 0 {
                bad.push("PowerShell tidak bisa dijalankan.");
            } else {
                if code & 4 != 0 {
                    bad.push("VB-Cable gagal diunduh. Cek koneksi internet lalu coba lagi.");
                }
                if code & 8 != 0 {
                    bad.push("Installer VB-Cable tidak ditemukan di paket unduhan.");
                }
                if code & 16 != 0 {
                    bad.push("VB-Cable belum muncul sebagai perangkat. Restart PC, lalu buka Gatito dan klik \"Atur otomatis\".");
                }
                if code & 32 != 0 {
                    bad.push("Terjadi kesalahan saat pemasangan.");
                }
            }
            let found = code > 0 && code < 64 && (code & 1) == 1;
            if found && (code & 2) == 0 {
                self.set(|s| s.notes.push("Nama mic virtual tidak bisa diganti (tidak masalah, tetap berfungsi).".into()));
            }

            self.set(|s| s.msg = "Mengatur mic default dan menjaga speaker tetap di perangkatmu…".into());
            let tpl = if opt.install { DEFAULTS_4 } else { DEFAULTS_1 };
            let script = tpl.replace("@@RID@@", &str_of(&sn, "rid").replace('\'', "''")).replace("@@RNAME@@", &str_of(&sn, "rname").replace('\'', "''"));
            let df = last_json(&run_ps(&format!("{}{}", LIB, script), 90000).await.out);
            let df_err = str_of(&df, "err");
            let df_cap: Option<i64> = df.as_ref().and_then(|j| j.get("cap")).and_then(|c| c.as_i64());
            let def_ok = df_err.is_empty() && df_cap.is_some() && (!found || df_cap == Some(0));
            let rname = str_of(&sn, "rname");
            if !df_err.is_empty() {
                self.set(|s| s.notes.push(format!("Default suara Windows tidak bisa diubah otomatis ({}). Atur manual di pengaturan suara Windows.", df_err)));
            } else if df_cap.is_none() {
                self.set(|s| s.notes.push("Pengaturan default suara Windows tidak memberi hasil. Atur manual di pengaturan suara Windows.".into()));
            } else {
                if found && df_cap != Some(0) {
                    self.set(|s| s.notes.push("Mic default belum berhasil dipindah ke Virtual Mic. Atur manual di pengaturan suara Windows.".into()));
                }
                if !rname.is_empty() && is_virt_name(&rname) {
                    self.set(|s| s.notes.push("Speaker default Windows sebelumnya sudah berupa CABLE Input, jadi tidak dikembalikan. Pilih speakermu di menu Speaker.".into()));
                }
            }

            if !bad.is_empty() {
                let e = bad.join(" ");
                self.set(|s| {
                    s.err = e.clone();
                    s.msg = e;
                });
            } else {
                out.ok = true;
                let m = if def_ok {
                    format!("Selesai. Mic default Windows sekarang Virtual Mic, speakermu tidak diubah.{}", if opt.install { " Kalau belum muncul di Discord, restart PC." } else { "" })
                } else {
                    "VB-Cable sudah siap, tetapi default suara Windows belum berhasil diatur otomatis (lihat catatan).".to_string()
                };
                self.set(|s| s.msg = m);
            }
            out.mic = real_mic;
            Ok(())
        }
        .await;
        if let Err(e) = res {
            self.set(|s| {
                s.err = e.clone();
                s.msg = e;
            });
        }
        self.set(|s| {
            s.running = false;
            s.done = true;
        });
        out
    }

    /// Saat Gatito dibuka: catat mic asli (kalau default masih mic asli), lalu jadikan Virtual Mic mic default
    pub async fn use_virtual(&self) -> Option<Value> {
        if !cfg!(windows) {
            return None;
        }
        last_json(&run_ps(&format!("{}{}", LIB, USE_VIRTUAL), 60000).await.out)
    }

    /// Saat Gatito ditutup: kembalikan mic asli, tapi hanya kalau default sekarang masih Virtual Mic
    pub async fn restore_if_virtual(&self, label: &str) -> Option<Value> {
        let l = clean_label(label);
        if !cfg!(windows) || l.is_empty() {
            return None;
        }
        let script = RESTORE_IF_VIRT.replace("@@LABEL@@", &l.replace('\'', "''"));
        last_json(&run_ps(&format!("{}{}", LIB, script), 60000).await.out)
    }

    pub async fn restore_mic(&self, label: &str) -> Value {
        let l = clean_label(label);
        if !cfg!(windows) {
            return serde_json::json!({ "ok": false, "err": "Hanya untuk Windows." });
        }
        if l.is_empty() {
            return serde_json::json!({ "ok": false, "err": "Mic asli belum dipilih. Pilih dulu di menu Microphone Device." });
        }
        let script = RESTORE.replace("@@LABEL@@", &l.replace('\'', "''"));
        let r = last_json(&run_ps(&format!("{}{}", LIB, script), 60000).await.out);
        let ok = r.as_ref().and_then(|j| j.get("ok")).and_then(|o| o.as_i64()) == Some(1);
        if ok {
            serde_json::json!({ "ok": true })
        } else if str_of(&r, "err") == "notfound" {
            serde_json::json!({ "ok": false, "err": "Mic asli tidak ditemukan." })
        } else {
            serde_json::json!({ "ok": false, "err": "Gagal mengubah default mic." })
        }
    }
}
