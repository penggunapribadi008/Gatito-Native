// Cek rilis terbaru di GitHub, unduh installer, periksa ukuran dan hash-nya, lalu pasang diam-diam.
use crate::app::Ctx;
use crate::util::{no_window, VERSION};
use futures_util::StreamExt;
use once_cell::sync::Lazy;
use regex::Regex;
use serde::Serialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::io::Write;
use std::sync::Arc;
use std::time::Duration;

#[derive(Clone, Serialize, Debug)]
pub struct UpState {
    pub state: String,
    pub pct: u32,
}

impl UpState {
    pub fn idle() -> UpState {
        UpState { state: "idle".into(), pct: 0 }
    }
}

#[derive(Clone, Debug)]
pub struct Asset {
    pub url: String,
    pub name: String,
    pub size: u64,
    pub sha: String,
}

pub struct Check {
    pub json: Value,
    pub asset: Option<Asset>,
}

fn ver_num(v: &str) -> Vec<u64> {
    v.trim_start_matches(|c: char| c == 'v' || c == 'V')
        .split(|c: char| c == '.' || c == '-' || c == '+')
        .take(3)
        .map(|n| n.parse::<u64>().unwrap_or(0))
        .collect()
}
fn is_newer(a: &str, b: &str) -> bool {
    let (x, y) = (ver_num(a), ver_num(b));
    for i in 0..3 {
        let (p, q) = (x.get(i).copied().unwrap_or(0), y.get(i).copied().unwrap_or(0));
        if p != q {
            return p > q;
        }
    }
    false
}

static REPO_RX: Lazy<Regex> = Lazy::new(|| Regex::new(r"^[\w.\-]+/[\w.\-]+$").unwrap());
static EXE_RX: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?i)^gatito-stream-tool-v[\d.]+\.exe$").unwrap());
static ANY_EXE: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?i)\.exe$").unwrap());
static SHA_RX: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?i)^sha256:([a-f0-9]{64})$").unwrap());

pub async fn check_update() -> Check {
    let repo = crate::util::repo();
    if !REPO_RX.is_match(repo) {
        return Check { json: json!({ "state": "norepo" }), asset: None };
    }
    let err = || Check { json: json!({ "state": "error" }), asset: None };
    let client = match reqwest::Client::builder().timeout(Duration::from_secs(8)).user_agent("gatito-stream-tool").build() {
        Ok(c) => c,
        Err(_) => return err(),
    };
    let rs = match client.get(format!("https://api.github.com/repos/{}/releases/latest", repo)).header("Accept", "application/vnd.github+json").send().await {
        Ok(r) => r,
        Err(_) => return err(),
    };
    if rs.status().as_u16() == 404 {
        return Check { json: json!({ "state": "norelease" }), asset: None };
    }
    let status = rs.status().as_u16();
    let j: Value = match rs.json().await {
        Ok(j) => j,
        Err(_) => return err(),
    };
    let tag = match j.get("tag_name").and_then(|t| t.as_str()) {
        Some(t) if status == 200 => t.to_string(),
        _ => return err(),
    };
    let nw = is_newer(&tag, VERSION);
    let empty = vec![];
    let assets = j.get("assets").and_then(|a| a.as_array()).unwrap_or(&empty);
    let name_of = |x: &Value| x.get("name").and_then(|n| n.as_str()).unwrap_or("").to_string();
    let a = assets.iter().find(|x| EXE_RX.is_match(&name_of(x))).or_else(|| assets.iter().find(|x| ANY_EXE.is_match(&name_of(x))));
    let dl = a.and_then(|a| a.get("browser_download_url")).and_then(|u| u.as_str()).unwrap_or("");
    let ok = a.is_some() && dl.starts_with("https://github.com/");
    let sha = a.and_then(|a| a.get("digest")).and_then(|d| d.as_str()).and_then(|d| SHA_RX.captures(d)).map(|c| c[1].to_lowercase()).unwrap_or_default();
    let asset = if nw && ok {
        let a = a.unwrap();
        Some(Asset {
            url: dl.to_string(),
            name: name_of(a).chars().map(|c| if c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_' { c } else { '_' }).collect(),
            size: a.get("size").and_then(|s| s.as_u64()).unwrap_or(0),
            sha,
        })
    } else {
        None
    };
    Check {
        json: json!({
            "state": if nw { "new" } else { "latest" },
            "latest": tag.trim_start_matches(|c: char| c == 'v' || c == 'V'),
            "url": j.get("html_url").cloned().unwrap_or(Value::Null),
            "asset": nw && ok
        }),
        asset,
    }
}

async fn download(url: &str, dest: &std::path::Path, size: u64, ctx: &Arc<Ctx>) -> Result<(String, u64), String> {
    let client = reqwest::Client::builder().timeout(Duration::from_secs(600)).user_agent("gatito-stream-tool").build().map_err(|e| e.to_string())?;
    let rs = client.get(url).send().await.map_err(|e| e.to_string())?;
    if !rs.status().is_success() {
        return Err(format!("http {}", rs.status().as_u16()));
    }
    let total = rs.content_length().unwrap_or(size);
    let mut f = std::fs::File::create(dest).map_err(|e| e.to_string())?;
    let mut h = Sha256::new();
    let mut got: u64 = 0;
    let mut last: i64 = -1;
    let mut stream = rs.bytes_stream();
    while let Some(c) = stream.next().await {
        let c = c.map_err(|e| e.to_string())?;
        f.write_all(&c).map_err(|e| e.to_string())?;
        h.update(&c);
        got += c.len() as u64;
        if total > 0 {
            let p = (got * 100 / total).min(99) as i64;
            if p != last {
                last = p;
                ctx.up.lock().unwrap().pct = p as u32;
            }
        }
    }
    Ok((hex::encode(h.finalize()), got))
}

/// Unduh dan pasang update otomatis: berkas diunduh ke folder sementara, diperiksa ukuran dan hash-nya, lalu dipasang diam-diam
pub async fn fetch_update(ctx: Arc<Ctx>, a: Asset) {
    let dir = std::env::temp_dir().join("gatito-update");
    let dest = dir.join(&a.name);
    let res: Result<(), String> = async {
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let (sha, got) = download(&a.url, &dest, a.size, &ctx).await?;
        if (a.size > 0 && got != a.size) || (!a.sha.is_empty() && sha != a.sha) {
            return Err("berkas tidak cocok".into());
        }
        {
            let mut u = ctx.up.lock().unwrap();
            u.pct = 100;
            u.state = "ready".into();
        }
        if !apply_update(&ctx, &dest) {
            return Err("tidak bisa dipasang".into());
        }
        Ok(())
    }
    .await;
    if res.is_err() {
        ctx.up.lock().unwrap().state = "error".into();
        let _ = std::fs::remove_dir_all(&dir);
    }
}

/// Pasang update: tunggu aplikasi tertutup, jalankan installer diam-diam, lalu buka lagi aplikasinya
fn apply_update(_ctx: &Arc<Ctx>, file: &std::path::Path) -> bool {
    if cfg!(debug_assertions) || !cfg!(windows) {
        return false;
    }
    let exe = match std::env::current_exe() {
        Ok(e) => e,
        Err(_) => return false,
    };
    let e = |s: &str| s.replace('\'', "''");
    let cmd = format!(
        "Wait-Process -Id {} -ErrorAction SilentlyContinue; Start-Process -FilePath '{}' -ArgumentList '/S' -Wait; Start-Process -FilePath '{}'",
        std::process::id(),
        e(&file.to_string_lossy()),
        e(&exe.to_string_lossy())
    );
    let mut c = std::process::Command::new("powershell");
    c.args(["-NoProfile", "-WindowStyle", "Hidden", "-Command", &cmd]);
    no_window(&mut c);
    if c.spawn().is_err() {
        return false;
    }
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(800));
        crate::shell::cmd(crate::shell::UiCmd::Quit);
    });
    true
}
