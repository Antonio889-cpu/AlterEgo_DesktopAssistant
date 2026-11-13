//! Garry's Mod (Shinri Trial): Alter Ego riconosce dall'audio del PC i suoni degli eventi
//! (corpo scoperto, indagine, processo, mani lavate…) e reagisce. Non tocca il gioco.
//!
//! - I suoni di riferimento si leggono dall'addon "Sounds @ ST" (workshop 2560008589) che
//!   l'utente ha già: il percorso si ricava dall'eseguibile di GMod in esecuzione.
//! - Si ascolta l'uscita audio predefinita in loopback (WASAPI) solo mentre GMod è aperto.
//! - Riconoscimento a impronta: coppie di picchi spettrali con lo stesso sfasamento
//!   temporale (come Shazam), quindi indipendente dal volume e robusto a musica e voci.

use crate::{update_config, AppState};
use rustfft::{num_complex::Complex, Fft, FftPlanner};
use serde::Serialize;
use std::{
    collections::{HashMap, VecDeque},
    io::{Read, Seek, SeekFrom},
    path::{Path, PathBuf},
    sync::{atomic::AtomicBool, atomic::Ordering, mpsc, Arc},
    time::{Duration, Instant},
};
use tauri::{AppHandle, Emitter, Manager};

/// ID workshop dell'addon con i suoni del gamemode.
const SOUNDS_ADDON: &str = "2560008589";

/// (evento, prefisso dei file nel GMA): ogni file che inizia col prefisso è un riferimento.
/// Esclusi perché non si distinguono in modo affidabile (rumore d'acqua, clip < 1 s, toni bassi
/// coperti dalla musica): lavaggio mani, prova pronta, ingresso indagine, salute/fame basse.
const CLIPS: &[(&str, &str)] = &[
    ("body", "sound/body_announcement/"),
    ("body", "sound/find_corpse"),
    ("check_body", "sound/dro/other/dro_sounds_check_body"),
    ("investigation", "sound/dro/voice/en/autogames/st_monokuma_broadcast_investigation"),
    ("game_start", "sound/dro/voice/en/autogames/st_monokuma_broadcast_gamestart"),
    ("trial", "sound/dro/other/dro_sounds_class_trial_start"),
    ("trial", "sound/dro/voice/en/autogames/st_monokuma_classtrial_start"),
    ("trial", "sound/dro/voice/en/class_trial/st_monokuma_classtrial_start"),
    ("voting", "sound/dro/voice/en/autogames/st_monokuma_classtrial_voting"),
    ("voting", "sound/dro/other/dro_sounds_vote_start"),
    ("verdict_true", "sound/dro/voice/en/autogames/st_monokuma_classtrial_verdict_true"),
    ("verdict_false", "sound/dro/voice/en/autogames/st_monokuma_classtrial_verdict_false"),
    ("end_peaceful", "sound/dro/voice/en/autogames/st_monokuma_endgame_peaceful"),
    ("end_failure", "sound/dro/voice/en/autogames/st_monokuma_endgame_failure"),
    ("blackout", "sound/dro/voice/en/autogames/st_monokuma_event_blackout"),
    ("disguise", "sound/dro/voice/en/autogames/st_monokuma_event_disguise"),
    ("teleport", "sound/dro/voice/en/autogames/st_monokuma_event_teleport"),
    ("killer", "sound/dro/player_status/killer_sense"),
    ("hurt", "sound/dro/player_status/after_hit"),
    ("hurt", "sound/dro/player_status/knocked"),
    ("hurt", "sound/dro/player_status/stunned"),
    ("death", "sound/dro/player_status/death"),
    ("low_sleep", "sound/hud/low_sleep"),
];

// ---------- impronta ----------

const RATE: u32 = 11_025;
const N: usize = 1024;
const HOP: usize = 256;
/// Frame al secondo (~43).
const FPS: f32 = RATE as f32 / HOP as f32;
const FMIN: usize = 4;
const FMAX: usize = 400;
/// Vicinato del picco: ±frame, ±bin.
const TN: usize = 3;
const FNB: usize = 8;
/// Il picco deve superare la media del frame di questo (log naturale, ~×3).
const THR: f32 = 1.1;
const PEAKS_PER_FRAME: usize = 5;
const DT_MAX: u32 = 40;
const DF_MAX: i32 = 120;
const FAN: usize = 8;
/// Secondi di ogni clip usati come riferimento (i suoni lunghi si riconoscono dall'inizio).
const TMPL_SECS: f32 = 10.0;
/// Coppie allineate necessarie per dire "è lui" (meno per le clip corte, vedi `need`).
/// Sui 144 altri suoni del gioco il massimo misurato è 7: la soglia minima tiene 4 di margine.
const MIN_VOTES: u32 = 14;
const MIN_VOTES_SHORT: u32 = 11;
const COOLDOWN_SECS: f32 = 20.0;

/// Decimatore a media mobile (passa-basso grezzo + ricampionamento): stesso percorso per
/// riferimenti e audio dal vivo, così le impronte coincidono.
struct Decimator {
    step: f64,
    pos: f64,
    acc: f32,
    n: u32,
}

impl Decimator {
    fn new(from: u32) -> Self {
        Self { step: from as f64 / RATE as f64, pos: 0.0, acc: 0.0, n: 0 }
    }
    fn push(&mut self, input: &[f32], out: &mut Vec<f32>) {
        for &x in input {
            self.acc += x;
            self.n += 1;
            self.pos += 1.0;
            if self.pos >= self.step {
                self.pos -= self.step;
                out.push(self.acc / self.n as f32);
                self.acc = 0.0;
                self.n = 0;
            }
        }
    }
}

/// Dallo stream di campioni (a RATE) produce coppie (hash, frame dell'ancora).
struct Fingerprinter {
    fft: Arc<dyn Fft<f32>>,
    window: Vec<f32>,
    pending: Vec<f32>,
    /// Spettri recenti (log) per cercare i massimi locali con TN frame di anticipo.
    ring: VecDeque<Vec<f32>>,
    /// Frame del prossimo spettro in arrivo.
    t: u32,
    peaks: VecDeque<(u32, u16)>,
}

impl Fingerprinter {
    fn new() -> Self {
        let window = (0..N).map(|i| 0.5 - 0.5 * (2.0 * std::f32::consts::PI * i as f32 / N as f32).cos()).collect();
        Self { fft: FftPlanner::new().plan_fft_forward(N), window, pending: Vec::new(), ring: VecDeque::new(), t: 0, peaks: VecDeque::new() }
    }

    fn push(&mut self, samples: &[f32], emit: &mut impl FnMut(u32, u32)) {
        self.pending.extend_from_slice(samples);
        let mut start = 0;
        let mut buf = vec![Complex::new(0.0f32, 0.0); N];
        while self.pending.len() - start >= N {
            for (i, b) in buf.iter_mut().enumerate() {
                *b = Complex::new(self.pending[start + i] * self.window[i], 0.0);
            }
            self.fft.process(&mut buf);
            let spec: Vec<f32> = buf[..FMAX + FNB + 1].iter().map(|c| (c.norm() + 1e-6).ln()).collect();
            self.frame(spec, emit);
            start += HOP;
        }
        self.pending.drain(..start);
    }

    fn frame(&mut self, spec: Vec<f32>, emit: &mut impl FnMut(u32, u32)) {
        self.ring.push_back(spec);
        self.t += 1;
        if self.ring.len() < 2 * TN + 1 {
            return;
        }
        let tc = self.t - 1 - TN as u32;
        let c = &self.ring[TN];
        let mean = c[FMIN..FMAX].iter().sum::<f32>() / (FMAX - FMIN) as f32;
        let mut found: Vec<(f32, u16)> = Vec::new();
        for f in FMIN..FMAX {
            let v = c[f];
            if v < mean + THR {
                continue;
            }
            let lo = f.saturating_sub(FNB);
            let is_max = self.ring.iter().all(|s| s[lo..=f + FNB].iter().all(|&w| w <= v));
            if is_max {
                found.push((v, f as u16));
            }
        }
        self.ring.pop_front();
        found.sort_by(|a, b| b.0.total_cmp(&a.0));
        found.truncate(PEAKS_PER_FRAME);
        while self.peaks.front().is_some_and(|p| p.0 + DT_MAX < tc) {
            self.peaks.pop_front();
        }
        for &(_, f2) in &found {
            let mut n = 0;
            for &(t1, f1) in self.peaks.iter().rev() {
                if (f2 as i32 - f1 as i32).abs() > DF_MAX {
                    continue;
                }
                emit(hash(f1, f2, tc - t1), t1);
                n += 1;
                if n == FAN {
                    break;
                }
            }
        }
        self.peaks.extend(found.iter().map(|&(_, f)| (tc, f)));
    }
}

fn hash(f1: u16, f2: u16, dt: u32) -> u32 {
    ((f1 as u32) << 15) | ((f2 as u32) << 6) | dt
}

/// Indice dei riferimenti + voti dal vivo.
pub struct Matcher {
    index: HashMap<u32, Vec<(u16, u32)>>,
    clips: Vec<(&'static str, String)>,
    /// Per clip: voti necessari e durata del riferimento (frame).
    need: Vec<u32>,
    lens: Vec<u32>,
    /// Clip riconosciuta che sta ancora suonando: (evento, fino al frame). Le frasi di Monokuma
    /// si ripetono tra clip diverse, quindi intanto gli altri eventi non contano.
    busy: Option<(&'static str, i64)>,
    live: Fingerprinter,
    dec: Option<(u32, Decimator)>,
    votes: HashMap<(u16, i64), (u32, u32)>,
    cool: HashMap<&'static str, u32>,
    scratch: Vec<f32>,
}

impl Matcher {
    fn new() -> Self {
        Self { index: HashMap::new(), clips: Vec::new(), need: Vec::new(), lens: Vec::new(), busy: None, live: Fingerprinter::new(), dec: None, votes: HashMap::new(), cool: HashMap::new(), scratch: Vec::new() }
    }

    /// Aggiunge un riferimento (mono, `rate` Hz).
    fn add(&mut self, event: &'static str, name: String, mono: &[f32], rate: u32) {
        let id = self.clips.len() as u16;
        self.clips.push((event, name));
        let mut s = Vec::new();
        Decimator::new(rate).push(&mono[..mono.len().min((rate as f32 * TMPL_SECS) as usize)], &mut s);
        s.extend(std::iter::repeat(0.0).take(N + TN * HOP)); // svuota la coda dei picchi
        let index = &mut self.index;
        let mut hashes = 0u32;
        Fingerprinter::new().push(&s, &mut |h, t| {
            index.entry(h).or_default().push((id, t));
            hashes += 1;
        });
        // circa un decimo delle coppie del riferimento, tra 11 e 14
        self.need.push((hashes / 10).clamp(MIN_VOTES_SHORT, MIN_VOTES));
        self.lens.push((s.len() / HOP) as u32);
    }

    /// Audio dal vivo (mono, `rate` Hz): restituisce gli eventi riconosciuti.
    pub fn feed(&mut self, mono: &[f32], rate: u32) -> Vec<&'static str> {
        if self.dec.as_ref().map(|d| d.0) != Some(rate) {
            self.dec = Some((rate, Decimator::new(rate)));
        }
        let mut s = std::mem::take(&mut self.scratch);
        s.clear();
        self.dec.as_mut().unwrap().1.push(mono, &mut s);
        let mut hits: Vec<(u16, i64, u32)> = Vec::new();
        let (index, votes) = (&self.index, &mut self.votes);
        self.live.push(&s, &mut |h, t| {
            for &(clip, tt) in index.get(&h).map(|v| v.as_slice()).unwrap_or(&[]) {
                let off = t as i64 - tt as i64;
                let e = votes.entry((clip, off)).or_insert((0, t));
                e.0 += 1;
                e.1 = t;
                hits.push((clip, off, t));
            }
        });
        self.scratch = s;
        let now = self.live.t;
        let mut out = Vec::new();
        for (clip, off, _) in hits {
            let score: u32 = (off - 1..=off + 1).filter_map(|o| self.votes.get(&(clip, o))).map(|v| v.0).sum();
            let ev = self.clips[clip as usize].0;
            let cooling = self.cool.get(ev).is_some_and(|&t| now < t + (COOLDOWN_SECS * FPS) as u32);
            let masked = self.busy.is_some_and(|(b, until)| b != ev && (now as i64) < until);
            if score >= self.need[clip as usize] && !cooling && !masked {
                self.cool.insert(ev, now);
                self.busy = Some((ev, off + self.lens[clip as usize] as i64));
                out.push(ev);
            }
        }
        // i voti vecchi (> 4 s) non contano più
        let keep = (4.0 * FPS) as u32;
        if now % 64 == 0 {
            self.votes.retain(|_, v| v.1 + keep >= now);
        }
        out
    }
}

// ---------- lettura dei suoni dal GMA ----------

fn cstr(r: &mut impl Read) -> std::io::Result<String> {
    let mut v = Vec::new();
    let mut b = [0u8];
    loop {
        r.read_exact(&mut b)?;
        if b[0] == 0 {
            return Ok(String::from_utf8_lossy(&v).into_owned());
        }
        v.push(b[0]);
    }
}

/// File del GMA che iniziano con uno dei prefissi: (nome, byte).
fn read_gma(path: &Path, wanted: impl Fn(&str) -> bool) -> Result<Vec<(String, Vec<u8>)>, String> {
    let file = std::fs::File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut r = std::io::BufReader::new(file);
    let bad = |e: std::io::Error| format!("GMA non valido: {e}");
    let mut head = [0u8; 4 + 1 + 16];
    r.read_exact(&mut head).map_err(bad)?;
    if &head[..4] != b"GMAD" {
        return Err("non è un file GMA".into());
    }
    while !cstr(&mut r).map_err(bad)?.is_empty() {} // contenuti richiesti
    for _ in 0..3 {
        cstr(&mut r).map_err(bad)?; // nome, descrizione, autore
    }
    let mut u32b = [0u8; 4];
    r.read_exact(&mut u32b).map_err(bad)?; // versione addon
    let mut entries = Vec::new();
    loop {
        r.read_exact(&mut u32b).map_err(bad)?;
        if u32::from_le_bytes(u32b) == 0 {
            break;
        }
        let name = cstr(&mut r).map_err(bad)?;
        let mut rest = [0u8; 12];
        r.read_exact(&mut rest).map_err(bad)?;
        entries.push((name, u64::from_le_bytes(rest[..8].try_into().unwrap())));
    }
    let mut pos = r.stream_position().map_err(bad)?;
    let mut out = Vec::new();
    for (name, size) in entries {
        if wanted(&name) {
            r.seek(SeekFrom::Start(pos)).map_err(bad)?;
            let mut b = vec![0u8; size as usize];
            r.read_exact(&mut b).map_err(bad)?;
            out.push((name, b));
        }
        pos += size;
    }
    Ok(out)
}

/// Decodifica (mp3/ogg/wav) in mono; si ferma dopo `max_secs`.
fn decode(bytes: Vec<u8>, ext: &str, max_secs: f32) -> Result<(Vec<f32>, u32), String> {
    use symphonia::core::{audio::SampleBuffer, codecs::DecoderOptions, formats::FormatOptions, io::MediaSourceStream, meta::MetadataOptions, probe::Hint};
    let mss = MediaSourceStream::new(Box::new(std::io::Cursor::new(bytes)), Default::default());
    let mut hint = Hint::new();
    hint.with_extension(ext);
    let probed = symphonia::default::get_probe()
        .format(&hint, mss, &FormatOptions::default(), &MetadataOptions::default())
        .map_err(|e| format!("formato: {e}"))?;
    let mut format = probed.format;
    let track = format.default_track().ok_or("nessuna traccia")?.clone();
    let rate = track.codec_params.sample_rate.unwrap_or(44_100);
    let mut dec = symphonia::default::get_codecs().make(&track.codec_params, &DecoderOptions::default()).map_err(|e| format!("codec: {e}"))?;
    let max = (rate as f32 * max_secs) as usize;
    let mut out = Vec::new();
    while out.len() < max {
        let Ok(packet) = format.next_packet() else { break };
        if packet.track_id() != track.id {
            continue;
        }
        let Ok(buf) = dec.decode(&packet) else { continue };
        let ch = buf.spec().channels.count().max(1);
        let mut sb = SampleBuffer::<f32>::new(buf.capacity() as u64, *buf.spec());
        sb.copy_interleaved_ref(buf);
        out.extend(sb.samples().chunks(ch).map(|f| f.iter().sum::<f32>() / ch as f32));
    }
    Ok((out, rate))
}

/// Costruisce il matcher dai suoni dell'addon. Restituisce anche i file non decodificati.
pub fn load(gma: &Path) -> Result<(Matcher, Vec<String>), String> {
    let files = read_gma(gma, |n| CLIPS.iter().any(|(_, p)| n.starts_with(p)))?;
    let mut m = Matcher::new();
    let mut failed = Vec::new();
    for (name, bytes) in files {
        let ev = CLIPS.iter().find(|(_, p)| name.starts_with(p)).unwrap().0;
        let ext = name.rsplit('.').next().unwrap_or("").to_string();
        match decode(bytes, &ext, TMPL_SECS + 1.0) {
            Ok((mono, rate)) if !mono.is_empty() => m.add(ev, name, &mono, rate),
            Ok(_) => failed.push(name),
            Err(e) => failed.push(format!("{name} ({e})")),
        }
    }
    if m.clips.is_empty() {
        return Err("nessun suono di riferimento trovato nell'addon".into());
    }
    Ok((m, failed))
}

/// Dall'eseguibile di GMod: <steamapps>/workshop/content/4000/<addon>/*.gma
fn sounds_gma(exe: &Path) -> Option<PathBuf> {
    let steamapps = exe.ancestors().find(|p| p.file_name().is_some_and(|n| n.eq_ignore_ascii_case("steamapps")))?;
    let dir = steamapps.join("workshop/content/4000").join(SOUNDS_ADDON);
    std::fs::read_dir(dir).ok()?.flatten().map(|e| e.path()).find(|p| p.extension().is_some_and(|e| e == "gma"))
}

// ---------- ascolto ----------

/// Loopback dell'uscita predefinita: invia blocchi mono al canale.
fn open_loopback(tx: mpsc::SyncSender<Vec<f32>>, broken: Arc<AtomicBool>) -> Result<(cpal::Stream, u32, String), String> {
    use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
    let device = cpal::default_host().default_output_device().ok_or("nessuna uscita audio")?;
    let name = device.name().unwrap_or_default();
    let config = device.default_output_config().map_err(|e| format!("{name}: {e}"))?;
    let ch = config.channels() as usize;
    let rate = config.sample_rate().0;
    let err = move |e| {
        eprintln!("loopback GMod: {e}");
        broken.store(true, Ordering::Relaxed);
    };
    let stream = match config.sample_format() {
        cpal::SampleFormat::F32 => device.build_input_stream(
            &config.into(),
            move |d: &[f32], _: &_| {
                let _ = tx.try_send(d.chunks(ch).map(|f| f.iter().sum::<f32>() / ch as f32).collect());
            },
            err,
            None,
        ),
        cpal::SampleFormat::I16 => device.build_input_stream(
            &config.into(),
            move |d: &[i16], _: &_| {
                let _ = tx.try_send(d.chunks(ch).map(|f| f.iter().map(|&s| s as f32 / 32768.0).sum::<f32>() / ch as f32).collect());
            },
            err,
            None,
        ),
        f => return Err(format!("{name}: formato {f:?} non supportato")),
    }
    .map_err(|e| format!("{name}: {e}"))?;
    stream.play().map_err(|e| format!("{name}: {e}"))?;
    Ok((stream, rate, name))
}

#[derive(Serialize, Clone)]
struct GmodEvent {
    event: &'static str,
}

/// Thread unico: controlla ogni 3 s se la funzione è attiva e GMod è aperto; intanto ascolta.
pub fn spawn(app: AppHandle) {
    std::thread::spawn(move || {
        let mut matcher: Option<(PathBuf, Matcher)> = None;
        let mut failed_gma: Option<PathBuf> = None;
        let mut live: Option<(cpal::Stream, mpsc::Receiver<Vec<f32>>, u32, Arc<AtomicBool>)> = None;
        let mut next = Instant::now();
        loop {
            if Instant::now() >= next {
                next = Instant::now() + Duration::from_secs(3);
                let on = app.state::<AppState>().cfg.lock().unwrap().gmod;
                let gma = if on { crate::apps::exe_path(&["gmod", "hl2"]).filter(|p| p.to_string_lossy().to_lowercase().contains("garrysmod")).and_then(|p| sounds_gma(&p)) } else { None };
                if live.as_ref().is_some_and(|l| l.3.load(Ordering::Relaxed)) {
                    live = None; // dispositivo cambiato o scollegato: si riapre
                }
                match gma {
                    None => {
                        if live.take().is_some() {
                            crate::applog::info("gmod", "ascolto fermato");
                        }
                    }
                    Some(p) => {
                        if matcher.as_ref().map(|m| &m.0) != Some(&p) && failed_gma.as_ref() != Some(&p) {
                            let t0 = Instant::now();
                            match load(&p) {
                                Ok((m, bad)) => {
                                    crate::applog::info("gmod", &format!("{} suoni di riferimento caricati in {:.1}s", m.clips.len(), t0.elapsed().as_secs_f32()));
                                    if !bad.is_empty() {
                                        crate::applog::warn("gmod", &format!("non decodificati: {}", bad.join(", ")));
                                    }
                                    matcher = Some((p, m));
                                }
                                Err(e) => {
                                    crate::applog::warn("gmod", &e);
                                    failed_gma = Some(p);
                                }
                            }
                        }
                        if live.is_none() && matcher.is_some() {
                            let (tx, rx) = mpsc::sync_channel(64);
                            let broken = Arc::new(AtomicBool::new(false));
                            match open_loopback(tx, broken.clone()) {
                                Ok((s, rate, name)) => {
                                    crate::applog::info("gmod", &format!("ascolto l'audio di GMod su {name}"));
                                    live = Some((s, rx, rate, broken));
                                }
                                Err(e) => crate::applog::warn("gmod", &format!("loopback: {e}")),
                            }
                        }
                    }
                }
            }
            let Some((_, rx, rate, _)) = &live else {
                std::thread::sleep(Duration::from_millis(500));
                continue;
            };
            let Ok(block) = rx.recv_timeout(Duration::from_millis(200)) else { continue };
            let Some((_, m)) = matcher.as_mut() else { continue };
            for ev in m.feed(&block, *rate) {
                crate::applog::info("gmod", &format!("evento: {ev}"));
                let _ = app.emit("gmod", GmodEvent { event: ev });
            }
        }
    });
}

#[tauri::command]
pub fn gmod_settings(app: AppHandle, enabled: bool) {
    update_config(&app, |c| c.gmod = enabled);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Rumore deterministico.
    fn noise(seed: &mut u32, n: usize, amp: f32) -> Vec<f32> {
        (0..n)
            .map(|_| {
                *seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                (*seed >> 8) as f32 / (1u32 << 24) as f32 * 2.0 * amp - amp
            })
            .collect()
    }

    /// Melodia sintetica: toni casuali da 120 ms.
    fn melody(seed: &mut u32, secs: f32, rate: u32) -> Vec<f32> {
        let mut out = Vec::new();
        while out.len() < (secs * rate as f32) as usize {
            *seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            let f = 200.0 + (*seed >> 16) as f32 % 3000.0;
            let n0 = out.len();
            out.extend((0..(0.12 * rate as f32) as usize).map(|i| (2.0 * std::f32::consts::PI * f * (n0 + i) as f32 / rate as f32).sin() * 0.5));
        }
        out
    }

    fn to_48k(mono: &[f32], rate: u32) -> Vec<f32> {
        let mut out = VecDeque::new();
        crate::audio::Resampler::new(rate, 48_000).push(mono, &mut out);
        out.into()
    }

    /// Prova sui suoni veri: ogni riferimento, sopra musica della lobby e rumore, deve dare il
    /// suo evento (e solo quello); tutti gli altri suoni del gioco non devono dare niente.
    /// `GMOD_SOUNDS_GMA=<...>/2560008589/gmpublisher.gma cargo test real_sounds -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn real_sounds() {
        let gma = PathBuf::from(std::env::var("GMOD_SOUNDS_GMA").expect("GMOD_SOUNDS_GMA"));
        let t0 = Instant::now();
        let (mut m, bad) = load(&gma).unwrap();
        println!("{} riferimenti in {:.1}s, non decodificati: {bad:?}", m.clips.len(), t0.elapsed().as_secs_f32());
        let all = read_gma(&gma, |n| n.starts_with("sound/")).unwrap();
        let dec48 = |b: &Vec<u8>, n: &str, secs: f32| decode(b.clone(), n.rsplit('.').next().unwrap(), secs).ok().map(|(s, r)| to_48k(&s, r));
        let mut bg: Vec<f32> = Vec::new();
        for (n, b) in all.iter().filter(|(n, _)| n.starts_with("sound/lobby/")) {
            bg.extend(dec48(b, n, 30.0).unwrap_or_default());
        }
        assert!(bg.len() > 48_000 * 60, "serve musica di sottofondo");
        let gain = std::env::var("GMOD_GAIN").ok().and_then(|g| g.parse().ok()).unwrap_or(0.5f32);
        let mut seed = 3;
        let mut bgpos = 0usize;
        let mut run = |m: &mut Matcher, clip: &[f32]| -> Vec<(&'static str, f32)> {
            m.votes.clear();
            m.cool.clear();
            m.busy = None;
            m.live = Fingerprinter::new(); // niente coda della prova precedente
            let lead = 48_000 * 2;
            let len = lead + clip.len().min(48_000 * 9) + 48_000;
            let n = noise(&mut seed, len, 0.02);
            let mut got = Vec::new();
            let mut i = 0;
            while i < len {
                let c: Vec<f32> = (i..(i + 480).min(len))
                    .map(|k| bg[(bgpos + k) % bg.len()] * 0.3 + n[k] + if k >= lead { clip.get(k - lead).copied().unwrap_or(0.0) * gain } else { 0.0 })
                    .collect();
                for ev in m.feed(&c, 48_000) {
                    got.push((ev, (i as f32 - lead as f32) / 48_000.0));
                }
                i += 480;
            }
            bgpos += len;
            got
        };
        let mut fails = Vec::new();
        let mut lat = Vec::new();
        let names: Vec<(&'static str, String)> = m.clips.clone();
        for (ev, name) in &names {
            let b = &all.iter().find(|(n, _)| n == name).unwrap().1;
            let got = run(&mut m, &dec48(b, name, 12.0).unwrap());
            match got.first() {
                Some((e, t)) if e == ev && got.iter().all(|g| g.0 == *ev) => lat.push(*t),
                _ => {
                    let id = names.iter().position(|c| &c.1 == name).unwrap() as u16;
                    let best = m.votes.iter().filter(|((c, _), _)| *c == id).map(|(_, v)| v.0).max().unwrap_or(0);
                    let total = m.index.values().flatten().filter(|(c, _)| *c == id).count();
                    fails.push(format!("{name} [{ev}] -> {got:?} (voti max {best}, servono {}, coppie {total}, {:.1}s)", m.need[id as usize], m.lens[id as usize] as f32 / FPS))
                }
            }
        }
        let mut false_hits = Vec::new();
        let negatives: Vec<_> = all
            .iter()
            .filter(|(n, _)| !CLIPS.iter().any(|(_, p)| n.starts_with(p)) && !n.starts_with("sound/lobby/") && !n.starts_with("sound/dro/voice/"))
            .collect();
        for (n, b) in &negatives {
            if let Some(s) = dec48(b, n, 12.0) {
                let got = run(&mut m, &s);
                let worst = m.votes.keys().map(|&(c, o)| (o - 1..=o + 1).filter_map(|o| m.votes.get(&(c, o))).map(|v| v.0).sum::<u32>()).max().unwrap_or(0);
                if worst >= 5 {
                    println!("negativo {n}: voti max {worst}");
                }
                if !got.is_empty() {
                    false_hits.push(format!("{n} -> {got:?}"));
                }
            }
        }
        lat.sort_by(f32::total_cmp);
        println!("riconosciuti {}/{}; latenza mediana {:.2}s, max {:.2}s", lat.len(), names.len(), lat.get(lat.len() / 2).unwrap_or(&0.0), lat.last().unwrap_or(&0.0));
        println!("mancati/confusi: {fails:#?}");
        println!("falsi allarmi su {} suoni: {false_hits:#?}", negatives.len());
        assert!(fails.is_empty() && false_hits.is_empty());
    }

    /// Ascolta 3 s dell'uscita predefinita: `cargo test loopback_works -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn loopback_works() {
        let (tx, rx) = mpsc::sync_channel(256);
        let (_s, rate, name) = open_loopback(tx, Arc::new(AtomicBool::new(false))).unwrap();
        std::thread::sleep(Duration::from_secs(3));
        let all: Vec<f32> = rx.try_iter().flatten().collect();
        let rms = (all.iter().map(|x| x * x).sum::<f32>() / all.len().max(1) as f32).sqrt();
        println!("{name}: {rate} Hz, {} campioni, rms {rms:.4}", all.len());
    }

    #[test]
    fn recognizes_the_right_clip_in_noise() {
        let mut seed = 7;
        let a = melody(&mut seed, 3.0, 44_100);
        let b = melody(&mut seed, 3.0, 44_100);
        let mut m = Matcher::new();
        m.add("body", "a".into(), &a, 44_100);
        m.add("wash", "b".into(), &b, 44_100);
        // dal vivo a 48 kHz: rumore, poi `a` a basso volume col rumore sopra
        let mut r = crate::audio::Resampler::new(44_100, 48_000);
        let mut up = VecDeque::new();
        r.push(&a, &mut up);
        let mut live = noise(&mut seed, 48_000, 0.05);
        let n = noise(&mut seed, up.len(), 0.05);
        live.extend(up.iter().zip(n).map(|(x, y)| x * 0.2 + y));
        let mut got = Vec::new();
        for c in live.chunks(480) {
            got.extend(m.feed(c, 48_000));
        }
        assert_eq!(got, ["body"]);
        // solo rumore: niente
        let mut m2 = Matcher::new();
        m2.add("body", "a".into(), &a, 44_100);
        assert!(noise(&mut seed, 48_000 * 5, 0.3).chunks(480).all(|c| m2.feed(c, 48_000).is_empty()));
    }
}
