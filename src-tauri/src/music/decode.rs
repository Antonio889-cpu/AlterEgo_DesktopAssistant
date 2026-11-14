//! Thread di decodifica: legge il file (m4a/AAC, mp3, wav) con symphonia, ricampiona al rate
//! delle uscite e riempie la coda musica del motore audio (circa 0,6 s di anticipo).

use crate::audio::{AudioEngine, Resampler};
use std::{
    collections::VecDeque,
    fs::File,
    path::Path,
    sync::mpsc::{Receiver, TryRecvError},
    time::Duration,
};
use symphonia::core::{
    audio::SampleBuffer,
    codecs::DecoderOptions,
    errors::Error as SymErr,
    formats::{FormatOptions, SeekMode, SeekTo},
    io::MediaSourceStream,
    meta::MetadataOptions,
    probe::Hint,
    units::Time,
};

pub enum Ctl {
    Seek(f64),
    Stop,
}

/// Come è finita la riproduzione.
pub enum End {
    /// Brano finito: si passa al successivo.
    Finished,
    /// Fermato da un comando.
    Stopped,
}

/// Rate usato per contare la posizione quando non c'è un'uscita audio.
pub const FALLBACK_RATE: u32 = 48_000;
const AHEAD_SECONDS: f32 = 0.6;

struct Stereo {
    l: Resampler,
    r: Resampler,
}

impl Stereo {
    fn new(from: u32, to: u32) -> Self {
        Self { l: Resampler::new(from, to), r: Resampler::new(from, to) }
    }
    fn run(&mut self, l: &[f32], r: &[f32]) -> Vec<(f32, f32)> {
        let (mut ol, mut or) = (VecDeque::new(), VecDeque::new());
        self.l.push(l, &mut ol);
        self.r.push(r, &mut or);
        ol.into_iter().zip(or).collect()
    }
}

/// Durata del file in secondi (se il contenitore la indica).
pub fn duration(path: &Path) -> Option<f64> {
    let (format, _) = open(path).ok()?;
    let t = format.default_track()?;
    let frames = t.codec_params.n_frames?;
    let rate = t.codec_params.sample_rate?;
    Some(frames as f64 / rate as f64)
}

fn open(path: &Path) -> Result<(Box<dyn symphonia::core::formats::FormatReader>, ()), String> {
    let file = File::open(path).map_err(|e| format!("file audio: {e}"))?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());
    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }
    let probed = symphonia::default::get_probe()
        .format(&hint, mss, &FormatOptions { enable_gapless: true, ..Default::default() }, &MetadataOptions::default())
        .map_err(|e| format!("formato audio non supportato: {e}"))?;
    Ok((probed.format, ()))
}

/// Decodifica un file breve (Line, effetti) in campioni mono. Al massimo `max_secs` secondi.
pub fn decode_mono(path: &Path, max_secs: f64) -> Result<(Vec<f32>, u32), String> {
    let (mut format, _) = open(path)?;
    let track = format.default_track().ok_or("nessuna traccia audio")?.clone();
    let rate = track.codec_params.sample_rate.unwrap_or(44_100);
    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &DecoderOptions::default())
        .map_err(|e| format!("codec non supportato: {e}"))?;
    let max = (max_secs * rate as f64) as usize;
    let mut out = Vec::new();
    while out.len() < max {
        let packet = match format.next_packet() {
            Ok(p) => p,
            Err(SymErr::ResetRequired) => {
                decoder.reset();
                continue;
            }
            Err(_) => break,
        };
        if packet.track_id() != track.id {
            continue;
        }
        let Ok(decoded) = decoder.decode(&packet) else { continue };
        let spec = *decoded.spec();
        let ch = spec.channels.count().max(1);
        let mut sbuf = SampleBuffer::<f32>::new(decoded.capacity() as u64, spec);
        sbuf.copy_interleaved_ref(decoded);
        out.extend(sbuf.samples().chunks(ch).map(|f| f.iter().sum::<f32>() / ch as f32));
    }
    out.truncate(max);
    Ok((out, rate))
}

/// Suona `path` da `start` secondi finché finisce o arriva `Ctl::Stop`.
/// `on_seek(t)`: chiamata quando la posizione salta (per azzerare il conteggio).
pub fn play(engine: &AudioEngine, path: &Path, start: f64, rx: &Receiver<Ctl>, on_seek: &dyn Fn(f64)) -> Result<End, String> {
    let (mut format, _) = open(path)?;
    let track = format.default_track().ok_or("nessuna traccia audio")?.clone();
    let track_id = track.id;
    let src_rate = track.codec_params.sample_rate.unwrap_or(44_100);
    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &DecoderOptions::default())
        .map_err(|e| format!("codec non supportato: {e}"))?;

    let seek = |format: &mut Box<dyn symphonia::core::formats::FormatReader>, t: f64| {
        let _ = format.seek(SeekMode::Coarse, SeekTo::Time { time: Time::from(t.max(0.0)), track_id: Some(track_id) });
    };
    engine.music_clear();
    if start > 0.5 {
        seek(&mut format, start);
    }
    on_seek(start);

    let mut rates = (0u32, None::<u32>);
    let mut sp = Stereo::new(src_rate, FALLBACK_RATE);
    let mut lk: Option<Stereo> = None;
    let mut eof = false;
    let mut lbuf = Vec::new();
    let mut rbuf = Vec::new();
    loop {
        match rx.try_recv() {
            Ok(Ctl::Stop) | Err(TryRecvError::Disconnected) => {
                engine.music_clear();
                return Ok(End::Stopped);
            }
            Ok(Ctl::Seek(t)) => {
                seek(&mut format, t);
                decoder.reset();
                engine.music_clear();
                on_seek(t);
                eof = false;
                rates = (0, None); // ricrea i ricampionatori (niente code vecchie)
            }
            Err(TryRecvError::Empty) => {}
        }
        if engine.music_is_paused() || engine.music_buffered() > AHEAD_SECONDS {
            std::thread::sleep(Duration::from_millis(20));
            continue;
        }
        if eof {
            if engine.music_buffered() < 0.02 {
                return Ok(End::Finished);
            }
            std::thread::sleep(Duration::from_millis(20));
            continue;
        }
        let now = engine.music_rates();
        if now != rates {
            rates = now;
            sp = Stereo::new(src_rate, if now.0 == 0 { FALLBACK_RATE } else { now.0 });
            lk = now.1.map(|r| Stereo::new(src_rate, r));
        }
        let packet = match format.next_packet() {
            Ok(p) => p,
            Err(SymErr::IoError(e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                eof = true;
                continue;
            }
            Err(SymErr::ResetRequired) => {
                decoder.reset();
                continue;
            }
            Err(e) => {
                eprintln!("musica: fine lettura ({e})");
                eof = true;
                continue;
            }
        };
        if packet.track_id() != track_id {
            continue;
        }
        let decoded = match decoder.decode(&packet) {
            Ok(d) => d,
            Err(SymErr::DecodeError(_)) | Err(SymErr::IoError(_)) => continue,
            Err(e) => return Err(format!("decodifica: {e}")),
        };
        let spec = *decoded.spec();
        let ch = spec.channels.count().max(1);
        let mut sbuf = SampleBuffer::<f32>::new(decoded.capacity() as u64, spec);
        sbuf.copy_interleaved_ref(decoded);
        lbuf.clear();
        rbuf.clear();
        for f in sbuf.samples().chunks(ch) {
            lbuf.push(f[0]);
            rbuf.push(if ch > 1 { f[1] } else { f[0] });
        }
        let out = sp.run(&lbuf, &rbuf);
        let link = lk.as_mut().map(|s| s.run(&lbuf, &rbuf));
        let n = out.len();
        if !engine.push_music(&out, link.as_deref()) {
            // nessuna uscita audio: si va a tempo reale lo stesso (posizione, spettro, fine brano)
            std::thread::sleep(Duration::from_secs_f64(n as f64 / FALLBACK_RATE as f64));
        }
    }
}
