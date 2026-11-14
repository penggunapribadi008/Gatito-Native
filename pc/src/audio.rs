// Mesin suara Gatito (pengganti audio.html di versi Electron). Cara kerjanya sama seperti Soundpad:
//  1. Suara tombol + mic aslimu dicampur, lalu dikirim ke "kabel virtual" (CABLE Input).
//     Discord/Roblox memakai ujung satunya (CABLE Output) sebagai mikrofon, jadi teman ikut mendengar.
//  2. Suara tombol yang sama juga diputar ke headset/speaker pilihanmu supaya kamu tetap mendengar.
use crate::state::{AudioCfg, DevSel};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Device, FromSample, SampleFormat, SizedSample, Stream, StreamConfig};
use once_cell::sync::Lazy;
use regex::Regex;
use serde_json::{json, Value};
use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::mpsc::{channel, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

static VIRT_RX: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?i)vb-audio|vb-output|cable[\s\w-]*(input|output)|voicemeeter").unwrap());
static CABLE1: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?i)^CABLE Input").unwrap());
static CABLE2: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?i)cable[\s\w-]*input").unwrap());
static STRIP_RX: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?i)^(Default|Communications) - ").unwrap());

fn is_virt(l: &str) -> bool {
    VIRT_RX.is_match(l)
}
fn strip(l: &str) -> String {
    STRIP_RX.replace(l, "").to_string()
}
fn find(list: &[String], p: &DevSel) -> Option<String> {
    if p.id.is_empty() && p.label.is_empty() {
        return None;
    }
    if !p.id.is_empty() {
        if let Some(d) = list.iter().find(|d| **d == p.id) {
            return Some(d.clone());
        }
    }
    if !p.label.is_empty() {
        return list.iter().find(|d| strip(d) == strip(&p.label)).cloned();
    }
    None
}
fn auto_cable(outs: &[String]) -> Option<String> {
    outs.iter()
        .find(|d| CABLE1.is_match(d))
        .or_else(|| outs.iter().find(|d| CABLE2.is_match(d)))
        .or_else(|| outs.iter().find(|d| is_virt(d)))
        .cloned()
}

// ---------------------------------------------------------------- decode
pub struct Decoded {
    pub samples: Vec<f32>, // interleaved
    pub ch: usize,
    pub rate: u32,
}

fn decode_file(path: &str) -> Result<Decoded, String> {
    use symphonia::core::audio::SampleBuffer;
    use symphonia::core::codecs::{DecoderOptions, CODEC_TYPE_NULL};
    use symphonia::core::errors::Error;
    use symphonia::core::formats::FormatOptions;
    use symphonia::core::io::MediaSourceStream;
    use symphonia::core::meta::MetadataOptions;
    use symphonia::core::probe::Hint;

    let file = std::fs::File::open(path).map_err(|_| "file tidak ditemukan".to_string())?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());
    let mut hint = Hint::new();
    if let Some(ext) = std::path::Path::new(path).extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }
    let probed = symphonia::default::get_probe()
        .format(&hint, mss, &FormatOptions::default(), &MetadataOptions::default())
        .map_err(|e| format!("format audio tidak dikenali ({e})"))?;
    let mut format = probed.format;
    let track = format
        .tracks()
        .iter()
        .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
        .ok_or_else(|| "tidak ada trek audio".to_string())?;
    let track_id = track.id;
    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &DecoderOptions::default())
        .map_err(|e| format!("codec tidak didukung ({e})"))?;
    let mut out: Vec<f32> = vec![];
    let (mut rate, mut ch) = (0u32, 0usize);
    loop {
        let packet = match format.next_packet() {
            Ok(p) => p,
            Err(Error::IoError(e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
            Err(Error::ResetRequired) => break,
            Err(_) => break,
        };
        if packet.track_id() != track_id {
            continue;
        }
        match decoder.decode(&packet) {
            Ok(buf) => {
                let spec = *buf.spec();
                let mut sb = SampleBuffer::<f32>::new(buf.capacity() as u64, spec);
                sb.copy_interleaved_ref(buf);
                out.extend_from_slice(sb.samples());
                rate = spec.rate;
                ch = spec.channels.count();
            }
            Err(Error::DecodeError(_)) => continue,
            Err(_) => break,
        }
    }
    if out.is_empty() || rate == 0 || ch == 0 {
        return Err("file audio kosong atau rusak".into());
    }
    Ok(Decoded { samples: out, ch, rate })
}

const MAXB: usize = 256 * 1024 * 1024;
struct CacheEnt {
    d: Arc<Decoded>,
    t: Instant,
    bytes: usize,
}
static CACHE: Lazy<Mutex<HashMap<String, CacheEnt>>> = Lazy::new(|| Mutex::new(HashMap::new()));

fn load(file: &str) -> Result<Arc<Decoded>, String> {
    let mt = std::fs::metadata(file).and_then(|m| m.modified()).ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_millis()).unwrap_or(0);
    let key = format!("{}|{}", file, mt);
    if let Some(e) = CACHE.lock().unwrap().get_mut(&key) {
        e.t = Instant::now();
        return Ok(e.d.clone());
    }
    let d = Arc::new(decode_file(file)?);
    let mut c = CACHE.lock().unwrap();
    let bytes = d.samples.len() * 4;
    c.insert(key.clone(), CacheEnt { d: d.clone(), t: Instant::now(), bytes });
    let mut tot: usize = c.values().map(|e| e.bytes).sum();
    let mut order: Vec<(String, Instant)> = c.iter().map(|(k, e)| (k.clone(), e.t)).collect();
    order.sort_by_key(|(_, t)| *t);
    for (k, _) in order {
        if tot > MAXB && k != key {
            if let Some(e) = c.remove(&k) {
                tot -= e.bytes;
            }
        }
    }
    Ok(d)
}

/// Ubah ke format bus keluaran: laju sampel dan jumlah kanal perangkat (resample linear)
fn to_bus(d: &Decoded, rate: u32, ch: usize) -> Vec<f32> {
    let sch = d.ch;
    let frames_in = d.samples.len() / sch;
    let ratio = d.rate as f64 / rate as f64;
    let frames_out = ((frames_in as f64) / ratio).floor() as usize;
    let mut out = vec![0f32; frames_out * ch];
    for i in 0..frames_out {
        let pos = i as f64 * ratio;
        let i0 = pos.floor() as usize;
        let fr = (pos - i0 as f64) as f32;
        let i1 = (i0 + 1).min(frames_in - 1);
        let i0 = i0.min(frames_in - 1);
        let sample = |f: usize, c: usize| d.samples[f * sch + c];
        let get = |c: usize| {
            let a = sample(i0, c);
            let b = sample(i1, c);
            a + (b - a) * fr
        };
        for c in 0..ch {
            let v = if sch == 1 {
                get(0)
            } else if ch == 1 {
                (0..sch).map(get).sum::<f32>() / sch as f32
            } else if c < sch {
                get(c)
            } else {
                0.0
            };
            out[i * ch + c] = v;
        }
    }
    out
}

// ---------------------------------------------------------------- bus
struct Voice {
    id: String,
    data: Arc<Vec<f32>>,
    pos: usize, // dalam sampel (bukan frame)
    gain: f32,
    fade_total: usize,
    fade_left: usize, // 0 = tidak sedang menghilang
}

struct MicRing {
    q: Mutex<VecDeque<f32>>,
    rate: u32,
}

struct Bus {
    rate: u32,
    ch: usize,
    voices: Vec<Voice>,
    send: f32, // volume kirim (vVol atau mVol, 0..2)
    env: f32,  // envelope limiter
    mic: Option<Arc<MicRing>>,
    mic_gain: f32,
    mic_frac: f64,
    peak_out: Arc<AtomicU32>,
    peak_mic: Arc<AtomicU32>,
}

const THR: f32 = 0.708; // -3 dB

fn fill(b: &mut Bus, out: &mut [f32]) {
    let ch = b.ch;
    let frames = out.len() / ch.max(1);
    let atk = 1.0 - (-1.0 / (0.002 * b.rate as f32)).exp();
    let rel = 1.0 - (-1.0 / (0.1 * b.rate as f32)).exp();
    let mut acc = vec![0f32; ch];
    let mut pk_out = 0f32;
    let mut pk_mic = 0f32;
    let ring = b.mic.clone();
    let mut q = ring.as_ref().map(|r| r.q.lock().unwrap());
    let ratio = ring.as_ref().map(|r| r.rate as f64 / b.rate as f64).unwrap_or(1.0);
    for f in 0..frames {
        for a in acc.iter_mut() {
            *a = 0.0;
        }
        for v in b.voices.iter_mut() {
            if v.pos + ch > v.data.len() {
                continue;
            }
            let mut g = v.gain;
            if v.fade_left > 0 {
                g *= v.fade_left as f32 / v.fade_total as f32;
                v.fade_left -= 1;
                if v.fade_left == 0 {
                    v.pos = v.data.len();
                }
            }
            for c in 0..ch {
                acc[c] += v.data[v.pos + c] * g;
            }
            v.pos += ch;
        }
        for a in acc.iter_mut() {
            *a *= b.send;
        }
        // mic asli ikut dicampur (tidak lewat volume kirim)
        if let Some(q) = q.as_mut() {
            let m = if q.len() >= 2 {
                let m = q[0] + (q[1] - q[0]) * b.mic_frac as f32;
                b.mic_frac += ratio;
                while b.mic_frac >= 1.0 && q.len() >= 2 {
                    q.pop_front();
                    b.mic_frac -= 1.0;
                }
                m
            } else {
                0.0
            };
            let m = m * b.mic_gain;
            pk_mic = pk_mic.max(m.abs());
            for a in acc.iter_mut() {
                *a += m;
            }
        }
        // limiter: mencegah suara pecah saat mic + suara tombol ditumpuk
        let peak = acc.iter().fold(0f32, |p, x| p.max(x.abs()));
        let target = if peak > THR { THR / peak } else { 1.0 };
        let coef = if target < b.env { atk } else { rel };
        b.env += (target - b.env) * coef;
        for c in 0..ch {
            let v = (acc[c] * b.env).clamp(-1.0, 1.0);
            pk_out = pk_out.max(v.abs());
            out[f * ch + c] = v;
        }
    }
    drop(q);
    b.voices.retain(|v| v.pos + ch <= v.data.len());
    b.peak_out.store(pk_out.to_bits(), Ordering::Relaxed);
    if b.mic.is_some() {
        b.peak_mic.store(pk_mic.to_bits(), Ordering::Relaxed);
    }
}

fn out_stream<T>(dev: &Device, sc: &StreamConfig, bus: Arc<Mutex<Bus>>) -> Result<Stream, cpal::BuildStreamError>
where
    T: SizedSample + FromSample<f32>,
{
    let mut tmp: Vec<f32> = Vec::new();
    dev.build_output_stream(
        sc,
        move |data: &mut [T], _| {
            tmp.resize(data.len(), 0.0);
            match bus.lock() {
                Ok(mut b) => fill(&mut b, &mut tmp),
                Err(_) => tmp.iter_mut().for_each(|x| *x = 0.0),
            }
            for (o, i) in data.iter_mut().zip(tmp.iter()) {
                *o = T::from_sample(*i);
            }
        },
        |e| eprintln!("audio keluaran: {e}"),
        None,
    )
}

fn in_stream<T>(dev: &Device, sc: &StreamConfig, ring: Arc<MicRing>) -> Result<Stream, cpal::BuildStreamError>
where
    T: SizedSample,
    f32: FromSample<T>,
{
    let ch = sc.channels as usize;
    let cap = (sc.sample_rate.0 as f32 * 0.15) as usize;
    dev.build_input_stream(
        sc,
        move |data: &[T], _| {
            let mut q = ring.q.lock().unwrap();
            for fr in data.chunks(ch.max(1)) {
                let m: f32 = fr.iter().map(|s| f32::from_sample(*s)).sum::<f32>() / fr.len() as f32;
                q.push_back(m);
            }
            while q.len() > cap {
                q.pop_front();
            }
        },
        |e| eprintln!("audio masukan: {e}"),
        None,
    )
}

// ---------------------------------------------------------------- engine
#[derive(Default, Clone)]
struct Info {
    cable_found: bool,
    cable: String,
    mon: String,
    mic: String,
    mic_id: String,
    errs: Vec<String>,
    warns: Vec<String>,
}

enum Cmd {
    Apply(AudioCfg, bool),
    Play(String, String, f32),
    Stop,
    Test,
    Info(Sender<Value>),
}

struct Shared {
    mic_peak: Arc<AtomicU32>,
    out_peak: Arc<AtomicU32>,
    lv: Mutex<(f32, f32)>,
}

#[derive(Clone)]
pub struct Engine {
    tx: Sender<Cmd>,
    sh: Arc<Shared>,
}

impl Engine {
    pub fn start() -> Engine {
        let (tx, rx) = channel();
        let sh = Arc::new(Shared { mic_peak: Arc::new(AtomicU32::new(0)), out_peak: Arc::new(AtomicU32::new(0)), lv: Mutex::new((0.0, 0.0)) });
        let sh2 = sh.clone();
        std::thread::Builder::new().name("gatito-audio".into()).spawn(move || engine_main(rx, sh2)).ok();
        Engine { tx, sh }
    }
    /// Terapkan pengaturan. force = bangun ulang perangkat walau pengaturannya sama
    pub fn apply(&self, cfg: AudioCfg, force: bool) {
        let _ = self.tx.send(Cmd::Apply(cfg, force));
    }
    pub fn play(&self, id: String, file: String, vol: f32) {
        let _ = self.tx.send(Cmd::Play(id, file, vol));
    }
    pub fn stop(&self) {
        let _ = self.tx.send(Cmd::Stop);
    }
    pub fn test(&self) {
        let _ = self.tx.send(Cmd::Test);
    }
    pub async fn info(&self, ms: u64) -> Option<Value> {
        let (rtx, rrx) = channel();
        self.tx.send(Cmd::Info(rtx)).ok()?;
        tokio::task::spawn_blocking(move || rrx.recv_timeout(Duration::from_millis(ms)).ok()).await.ok().flatten()
    }
    pub fn level(&self) -> (f32, f32) {
        *self.sh.lv.lock().unwrap()
    }
}

struct Live {
    cable: Option<Arc<Mutex<Bus>>>,
    mon: Option<Arc<Mutex<Bus>>>,
}

struct E {
    sh: Arc<Shared>,
    cfg: Option<AudioCfg>,
    last_sig: String,
    dev_sig: String,
    info: Info,
    outs: Vec<String>,
    inps: Vec<String>,
    def_out: String,
    def_in: String,
    streams: Vec<Stream>,
    live: Live,
    gens: Arc<Mutex<HashMap<String, u64>>>,
    built: bool,
    last_lv: Instant,
    last_poll: Instant,
}

fn engine_main(rx: Receiver<Cmd>, sh: Arc<Shared>) {
    let mut e = E {
        sh,
        cfg: None,
        last_sig: String::new(),
        dev_sig: String::new(),
        info: Info::default(),
        outs: vec![],
        inps: vec![],
        def_out: String::new(),
        def_in: String::new(),
        streams: vec![],
        live: Live { cable: None, mon: None },
        gens: Arc::new(Mutex::new(HashMap::new())),
        built: false,
        last_lv: Instant::now(),
        last_poll: Instant::now(),
    };
    loop {
        match rx.recv_timeout(Duration::from_millis(50)) {
            Ok(c) => e.handle(c),
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => break,
        }
        e.tick();
    }
}

fn bits(a: &AtomicU32) -> f32 {
    f32::from_bits(a.load(Ordering::Relaxed))
}

impl E {
    fn tick(&mut self) {
        if self.last_lv.elapsed() >= Duration::from_millis(50) {
            self.last_lv = Instant::now();
            let mut lv = self.sh.lv.lock().unwrap();
            lv.0 = bits(&self.sh.mic_peak).max(lv.0 * 0.85);
            lv.1 = bits(&self.sh.out_peak).max(lv.1 * 0.85);
        }
        // pengganti event devicechange: bangun ulang kalau daftar perangkat berubah
        if self.last_poll.elapsed() >= Duration::from_secs(2) {
            self.last_poll = Instant::now();
            if self.cfg.is_some() {
                let (o, i, d1, d2) = list_devices();
                let sig = format!("{:?}|{:?}|{}|{}", o.iter().map(|x| &x.0).collect::<Vec<_>>(), i.iter().map(|x| &x.0).collect::<Vec<_>>(), d1, d2);
                if sig != self.dev_sig && !self.dev_sig.is_empty() {
                    self.build(true);
                }
            }
        }
    }

    fn handle(&mut self, c: Cmd) {
        match c {
            Cmd::Apply(cfg, force) => {
                self.cfg = Some(cfg);
                self.build(force);
            }
            Cmd::Play(id, file, vol) => {
                let f2 = file.clone();
                self.go(&id, vol, move || load(&f2));
            }
            Cmd::Stop => {
                let ids: Vec<String> = {
                    let mut g = self.gens.lock().unwrap();
                    g.values_mut().for_each(|v| *v += 1);
                    g.keys().cloned().collect()
                };
                for k in &ids {
                    self.stop_id(k);
                }
            }
            Cmd::Test => {
                self.go("test", 0.9, || {
                    // dua nada pendek: C5 lalu G5
                    let sr = 48000usize;
                    let n = (sr as f32 * 0.8) as usize;
                    let mut d = vec![0f32; n];
                    for (i, s) in d.iter_mut().enumerate() {
                        let t = i as f32 / sr as f32;
                        let tt = t % 0.4;
                        let f = if t < 0.4 { 523.25 } else { 783.99 };
                        *s = 0.5 * (2.0 * std::f32::consts::PI * f * tt).sin() * (tt / 0.01).min(1.0) * (-tt * 4.0).exp();
                    }
                    Ok(Arc::new(Decoded { samples: d, ch: 1, rate: sr as u32 }))
                });
            }
            Cmd::Info(r) => {
                let _ = r.send(self.info_json());
            }
        }
    }

    fn set_gains(&self) {
        if let Some(c) = &self.cfg {
            if let Some(b) = &self.live.cable {
                let mut b = b.lock().unwrap();
                b.send = c.v_vol as f32 / 100.0;
                b.mic_gain = c.mic_vol as f32 / 100.0;
            }
            if let Some(b) = &self.live.mon {
                b.lock().unwrap().send = c.m_vol as f32 / 100.0;
            }
        }
    }

    fn teardown(&mut self) {
        for v in self.gens.lock().unwrap().values_mut() {
            *v += 1;
        }
        self.streams.clear();
        self.live = Live { cable: None, mon: None };
        self.sh.mic_peak.store(0, Ordering::Relaxed);
        self.sh.out_peak.store(0, Ordering::Relaxed);
    }

    fn stop_id(&self, id: &str) {
        for b in [&self.live.cable, &self.live.mon].into_iter().flatten() {
            let mut b = b.lock().unwrap();
            let total = (b.rate as f32 * 0.02) as usize + 1; // fade 20 ms
            for v in b.voices.iter_mut().filter(|v| v.id == id && v.fade_left == 0) {
                v.fade_total = total;
                v.fade_left = total;
            }
        }
    }

    /// Mulai memutar: dekode di thread lain supaya perintah berikutnya (misalnya Stop) tidak tertahan
    fn go<F>(&mut self, id: &str, vol: f32, getbuf: F)
    where
        F: FnOnce() -> Result<Arc<Decoded>, String> + Send + 'static,
    {
        let my = {
            let mut g = self.gens.lock().unwrap();
            let e = g.entry(id.to_string()).or_insert(0);
            *e += 1;
            *e
        };
        self.stop_id(id);
        let gens = self.gens.clone();
        let (cable, mon) = (self.live.cable.clone(), self.live.mon.clone());
        let id = id.to_string();
        std::thread::spawn(move || {
            let d = match getbuf() {
                Ok(d) => d,
                Err(e) => {
                    eprintln!("Gagal memutar: {e}");
                    return;
                }
            };
            if gens.lock().unwrap().get(&id).copied() != Some(my) {
                return; // tombol ditekan lagi atau dihentikan saat file dimuat
            }
            for b in [cable, mon].into_iter().flatten() {
                let (rate, ch) = {
                    let g = b.lock().unwrap();
                    (g.rate, g.ch)
                };
                let data = Arc::new(to_bus(&d, rate, ch));
                if gens.lock().unwrap().get(&id).copied() != Some(my) {
                    return;
                }
                b.lock().unwrap().voices.push(Voice { id: id.clone(), data, pos: 0, gain: vol, fade_total: 1, fade_left: 0 });
            }
        });
    }

    fn build(&mut self, force: bool) {
        let c = match &self.cfg {
            Some(c) => c.clone(),
            None => return,
        };
        let sig = format!("{:?}{:?}{:?}", (&c.virt.id, &c.virt.label), (&c.mon.id, &c.mon.label), (&c.mic.id, &c.mic.label));
        if !force && sig == self.last_sig && self.built {
            return self.set_gains();
        }
        self.last_sig = sig;
        self.teardown();
        self.built = true;
        self.info = Info::default();
        let host = cpal::default_host();
        let (outs, ins, d_out, d_in) = list_devices();
        self.dev_sig = format!("{:?}|{:?}|{}|{}", outs.iter().map(|x| &x.0).collect::<Vec<_>>(), ins.iter().map(|x| &x.0).collect::<Vec<_>>(), d_out, d_in);
        self.outs = outs.iter().map(|x| x.0.clone()).collect();
        self.inps = ins.iter().map(|x| x.0.clone()).collect();
        self.def_out = d_out.clone();
        self.def_in = d_in.clone();
        let out_names = self.outs.clone();
        let dev_by_name = |list: &Vec<(String, Device)>, n: &str| list.iter().find(|x| x.0 == n).map(|x| x.1.clone());

        // 1) Jalur ke teman: kabel virtual
        let mut v: Option<String> = None;
        if c.virt.id != "none" {
            v = find(&out_names, &c.virt).or_else(|| auto_cable(&out_names));
        }
        if let Some(name) = v.clone() {
            match dev_by_name(&outs, &name).ok_or("perangkat tidak ditemukan".to_string()).and_then(|d| self.open_out(&d)) {
                Ok(bus) => {
                    self.live.cable = Some(bus);
                    self.info.cable_found = true;
                    self.info.cable = name;
                }
                Err(e) => {
                    self.info.errs.push(format!("Gagal membuka \"{}\": {}", name, e));
                    v = None;
                }
            }
        }

        // 2) Jalur ke headset/speaker kamu
        if let Some(mon_dev) = self.pick_mon(&c, &out_names, &v, &d_out) {
            match mon_dev.and_then(|n| dev_by_name(&outs, &n)).or_else(|| host.default_output_device()) {
                Some(d) => match self.open_out(&d) {
                    Ok(bus) => self.live.mon = Some(bus),
                    Err(e) => self.info.errs.push(format!("Gagal membuka headset/speaker: {}", e)),
                },
                None => self.info.errs.push("Gagal membuka headset/speaker: tidak ada perangkat keluaran.".into()),
            }
        }
        // Cadangan terakhir: kalau semuanya gagal, tetap putar ke output default supaya tombol tidak bisu
        if self.live.cable.is_none() && self.live.mon.is_none() && c.mon.id != "off" {
            if let Some(d) = host.default_output_device() {
                if let Ok(bus) = self.open_out(&d) {
                    self.live.mon = Some(bus);
                }
            }
        }

        // 3) Mic asli ikut dicampur ke kabel (supaya suaramu tetap terdengar teman)
        if self.live.cable.is_some() {
            self.build_mic(&c, &ins, &d_in);
        } else {
            self.info.mic = "Tidak dipakai (kabel virtual tidak ada)".into();
        }
        self.set_gains();
    }

    /// Mengembalikan None = tidak ada jalur ke speaker; Some(None) = pakai default; Some(Some(nama)) = perangkat tertentu
    fn pick_mon(&mut self, c: &AudioCfg, outs: &[String], v: &Option<String>, d_out: &str) -> Option<Option<String>> {
        if c.mon.id == "off" {
            return None;
        }
        let want_specific = !c.mon.id.is_empty() && c.mon.id != "default";
        let d = if want_specific { find(outs, &c.mon) } else { None };
        if want_specific && d.is_none() {
            self.info.warns.push("Perangkat headset yang tersimpan tidak ditemukan, sementara memakai Default Windows.".into());
        }
        let label = match &d {
            Some(n) => n.clone(),
            None => {
                if d_out.is_empty() {
                    "Default Windows".to_string()
                } else {
                    strip(d_out)
                }
            }
        };
        if is_virt(&label) || v.as_ref().map(|vv| strip(&label) == strip(vv)).unwrap_or(false) {
            self.info.warns.push(format!(
                "Output yang kamu dengar sekarang adalah \"{}\" (kabel virtual), makanya kamu tidak mendengar apa-apa. Pilih headset atau speaker aslimu di menu \"Aku mendengar lewat\", atau ganti Default output di pengaturan suara Windows.",
                strip(&label)
            ));
            return None;
        }
        self.info.mon = label;
        Some(d)
    }

    fn open_out(&mut self, dev: &Device) -> Result<Arc<Mutex<Bus>>, String> {
        let cfg = dev.default_output_config().map_err(|e| e.to_string())?;
        let fmt = cfg.sample_format();
        let sc: StreamConfig = cfg.clone().into();
        let bus = Arc::new(Mutex::new(Bus {
            rate: sc.sample_rate.0,
            ch: (sc.channels as usize).max(1),
            voices: vec![],
            send: 1.0,
            env: 1.0,
            mic: None,
            mic_gain: 1.0,
            mic_frac: 0.0,
            peak_out: self.sh.out_peak.clone(),
            peak_mic: self.sh.mic_peak.clone(),
        }));
        let st = match fmt {
            SampleFormat::F32 => out_stream::<f32>(dev, &sc, bus.clone()),
            SampleFormat::I16 => out_stream::<i16>(dev, &sc, bus.clone()),
            SampleFormat::U16 => out_stream::<u16>(dev, &sc, bus.clone()),
            SampleFormat::I32 => out_stream::<i32>(dev, &sc, bus.clone()),
            other => return Err(format!("format sampel {other:?} tidak didukung")),
        }
        .map_err(|e| e.to_string())?;
        st.play().map_err(|e| e.to_string())?;
        // Stream disimpan di thread mesin ini (Stream tidak boleh pindah thread)
        self.streams.push(st);
        Ok(bus)
    }

    fn build_mic(&mut self, c: &AudioCfg, ins: &Vec<(String, Device)>, d_in: &str) {
        if c.mic.id == "off" {
            self.info.mic = "Mati".into();
            return;
        }
        let real: Vec<String> = ins.iter().map(|x| x.0.clone()).filter(|n| !is_virt(n)).collect();
        let explicit = !c.mic.id.is_empty() || !c.mic.label.is_empty();
        let mut d = if explicit { find(&real, &c.mic) } else { None };
        if explicit && d.is_none() {
            self.info.warns.push("Mic yang tersimpan tidak ditemukan, sementara dipilih otomatis.".into());
        }
        if let Some(n) = &d {
            self.info.mic_id = n.clone();
        }
        if d.is_none() {
            // Default Windows boleh berupa Virtual Mic (VB-Cable). Mic itu tidak boleh dipakai di sini (akan berdengung), jadi ambil mic asli.
            d = if !d_in.is_empty() && !is_virt(d_in) { Some(d_in.to_string()) } else { real.first().cloned() };
        }
        let name = match d {
            Some(n) => n,
            None => {
                self.info.mic = "Belum dipilih".into();
                self.info.errs.push("Tidak ada mikrofon asli yang terdeteksi.".into());
                return;
            }
        };
        let dev = match ins.iter().find(|x| x.0 == name) {
            Some(x) => x.1.clone(),
            None => {
                self.info.mic = "Gagal".into();
                return;
            }
        };
        let r = (|| -> Result<(Stream, Arc<MicRing>), String> {
            let cfg = dev.default_input_config().map_err(|e| e.to_string())?;
            let fmt = cfg.sample_format();
            let sc: StreamConfig = cfg.into();
            let ring = Arc::new(MicRing { q: Mutex::new(VecDeque::new()), rate: sc.sample_rate.0 });
            let st = match fmt {
                SampleFormat::F32 => in_stream::<f32>(&dev, &sc, ring.clone()),
                SampleFormat::I16 => in_stream::<i16>(&dev, &sc, ring.clone()),
                SampleFormat::U16 => in_stream::<u16>(&dev, &sc, ring.clone()),
                SampleFormat::I32 => in_stream::<i32>(&dev, &sc, ring.clone()),
                other => return Err(format!("format sampel {other:?} tidak didukung")),
            }
            .map_err(|e| e.to_string())?;
            st.play().map_err(|e| e.to_string())?;
            Ok((st, ring))
        })();
        match r {
            Ok((st, ring)) => {
                self.streams.push(st);
                if let Some(b) = &self.live.cable {
                    b.lock().unwrap().mic = Some(ring);
                }
                self.info.mic = strip(&name);
            }
            Err(e) => {
                self.info.mic = "Gagal".into();
                self.info.errs.push(format!(
                    "Mic tidak bisa dibuka ({}). Cek Pengaturan Windows > Privasi > Mikrofon, aktifkan \"Izinkan aplikasi desktop mengakses mikrofon\".",
                    e
                ));
            }
        }
    }

    fn info_json(&self) -> Value {
        let m = |n: &String| json!({ "id": n, "label": if n.is_empty() { "(tanpa nama)".to_string() } else { n.clone() }, "v": is_virt(n) });
        let errs: Vec<&String> = self.info.errs.iter().rev().take(5).collect::<Vec<_>>().into_iter().rev().collect();
        json!({
            "cableFound": self.info.cable_found,
            "cable": self.info.cable,
            "mon": self.info.mon,
            "mic": self.info.mic,
            "micId": self.info.mic_id,
            "defInVirt": !self.def_in.is_empty() && is_virt(&self.def_in),
            "errs": errs,
            "warns": self.info.warns,
            "permErr": "",
            "defOut": strip(&self.def_out),
            "defIn": strip(&self.def_in),
            "out": self.outs.iter().map(m).collect::<Vec<_>>(),
            "inp": self.inps.iter().map(m).collect::<Vec<_>>(),
        })
    }
}

/// (perangkat keluaran, perangkat masukan, nama default keluaran, nama default masukan)
fn list_devices() -> (Vec<(String, Device)>, Vec<(String, Device)>, String, String) {
    let host = cpal::default_host();
    let outs: Vec<(String, Device)> = host.output_devices().map(|it| it.filter_map(|d| d.name().ok().map(|n| (n, d))).collect()).unwrap_or_default();
    let ins: Vec<(String, Device)> = host.input_devices().map(|it| it.filter_map(|d| d.name().ok().map(|n| (n, d))).collect()).unwrap_or_default();
    let d_out = host.default_output_device().and_then(|d| d.name().ok()).unwrap_or_default();
    let d_in = host.default_input_device().and_then(|d| d.name().ok()).unwrap_or_default();
    (outs, ins, d_out, d_in)
}
