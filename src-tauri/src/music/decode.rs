//! Thread di decodifica: legge il file (m4a/AAC, mp3, wav) con symphonia, ricampiona al rate
//! delle uscite e riempie la coda musica del motore audio (circa 0,6 s di anticipo).

use crate::audio::{AudioEngine, Resampler};
use super::ytdlp::Download;
use std::{
    collections::VecDeque,
    fs::File,
    io::{Read, Seek, SeekFrom},
    path::Path,
    sync::{
        mpsc::{Receiver, TryRecvError},
        Arc,
    },
    time::{Duration, Instant},
};
use symphonia::core::{
    audio::SampleBuffer,
    codecs::DecoderOptions,
    errors::Error as SymErr,
    formats::{FormatOptions, SeekMode, SeekTo},
    io::{MediaSource, MediaSourceStream},
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

/// File che yt-dlp sta ancora scrivendo: la lettura aspetta i byte che mancano.
/// Non "seekable": il demuxer MP4 legge l'indice all'inizio e poi va avanti in ordine.
struct Growing {
    f: File,
    dl: Arc<Download>,
    alive: Arc<dyn Fn() -> bool + Send + Sync>,
}

impl Read for Growing {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let mut stall = Instant::now();
        loop {
            let n = self.f.read(buf)?;
            if n > 0 || buf.is_empty() {
                return Ok(n);
            }
            if self.dl.settled() {
                // fine del download: l'ultimo pezzo può essere arrivato dopo la lettura precedente
                return self.f.read(buf);
            }
            if !(self.alive)() {
                return Err(std::io::Error::new(std::io::ErrorKind::Interrupted, "brano cambiato"));
            }
            if stall.elapsed() > Duration::from_secs(90) {
                return Err(std::io::Error::new(std::io::ErrorKind::TimedOut, crate::i18n::tr("download bloccato", "download stalled")));
            }
            std::thread::sleep(Duration::from_millis(40));
            if self.f.metadata().map(|m| m.len()).unwrap_or(0) == 0 {
                stall = Instant::now();
            }
        }
    }
}

impl Seek for Growing {
    fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
        self.f.seek(pos)
    }
}

impl MediaSource for Growing {
    fn is_seekable(&self) -> bool {
        false
    }
    fn byte_len(&self) -> Option<u64> {
        None
    }
}

/// Apre il brano mentre si scarica (streaming). None se il formato non lo permette
/// (es. MP4 con l'indice in fondo): allora si aspetta la fine del download.
fn open_stream(dl: &Arc<Download>, alive: &Arc<dyn Fn() -> bool + Send + Sync>) -> Option<Box<dyn symphonia::core::formats::FormatReader>> {
    let f = File::open(&dl.part).or_else(|_| File::open(&dl.file)).ok()?;
    let src = Growing { f, dl: dl.clone(), alive: alive.clone() };
    let mss = MediaSourceStream::new(Box::new(src), Default::default());
    let mut hint = Hint::new();
    if let Some(ext) = dl.file.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }
    let probed = symphonia::default::get_probe().format(&hint, mss, &FormatOptions { enable_gapless: true, ..Default::default() }, &MetadataOptions::default()).ok()?;
    Some(probed.format)
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

/// Suona un file completo (musiche dei contenuti aggiuntivi). Solo sugli altoparlanti: non è
/// la voce di Alter Ego, quindi non va nel microfono virtuale di Mic-Link.
pub fn play_file(engine: &AudioEngine, path: &Path, start: f64, rx: &Receiver<Ctl>, on_seek: &dyn Fn(f64)) -> Result<End, String> {
    play_to(engine, &Download::finished(path.to_path_buf()), start, rx, on_seek, Arc::new(|| true), false)
}

/// Suona il brano da `start` secondi finché finisce o arriva `Ctl::Stop`. Se il download è ancora
/// in corso parte subito (streaming); i salti in avanti si fanno appena il file è completo.
/// `on_seek(t)`: chiamata quando la posizione salta (per azzerare il conteggio).
/// `alive()`: false quando il brano è stato sostituito (sblocca la lettura in attesa di dati).
pub fn play(
    engine: &AudioEngine,
    dl: &Arc<Download>,
    start: f64,
    rx: &Receiver<Ctl>,
    on_seek: &dyn Fn(f64),
    alive: Arc<dyn Fn() -> bool + Send + Sync>,
) -> Result<End, String> {
    play_to(engine, dl, start, rx, on_seek, alive, true)
}

/// Come `play`; `link` = false non manda mai la musica al cavo di Mic-Link.
fn play_to(
    engine: &AudioEngine,
    dl: &Arc<Download>,
    start: f64,
    rx: &Receiver<Ctl>,
    on_seek: &dyn Fn(f64),
    alive: Arc<dyn Fn() -> bool + Send + Sync>,
    link: bool,
) -> Result<End, String> {
    let mut streaming = false;
    let mut format = if dl.is_done() {
        open(&dl.file)?.0
    } else if let Some(f) = (start <= 0.5).then(|| open_stream(dl, &alive)).flatten() {
        streaming = true;
        f
    } else {
        // niente streaming: si aspetta il file completo (si può comunque fermare)
        engine.music_clear();
        loop {
            if dl.is_done() {
                break open(&dl.file)?.0;
            }
            if dl.is_failed() {
                return Err(crate::i18n::tr("download non riuscito", "download failed").into());
            }
            match rx.recv_timeout(Duration::from_millis(100)) {
                Ok(Ctl::Stop) | Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => return Ok(End::Stopped),
                _ => {}
            }
        }
    };
    // salto chiesto durante lo streaming: si fa quando il file è completo
    let mut pending_seek: Option<f64> = None;
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
            Ok(Ctl::Seek(t)) if streaming && !dl.is_done() => pending_seek = Some(t),
            Ok(Ctl::Seek(t)) => {
                if streaming {
                    // file completo: si riapre normale (con salti)
                    format = open(&dl.file)?.0;
                    streaming = false;
                }
                seek(&mut format, t);
                decoder.reset();
                engine.music_clear();
                on_seek(t);
                eof = false;
                rates = (0, None); // ricrea i ricampionatori (niente code vecchie)
            }
            Err(TryRecvError::Empty) => {
                if let Some(t) = pending_seek.filter(|_| dl.is_done()) {
                    pending_seek = None;
                    format = open(&dl.file)?.0;
                    streaming = false;
                    seek(&mut format, t);
                    decoder.reset();
                    engine.music_clear();
                    on_seek(t);
                    eof = false;
                    rates = (0, None);
                }
            }
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
        let mut now = engine.music_rates();
        if !link {
            now.1 = None;
        }
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
            Err(e) => return Err(crate::tr!("decodifica: {e}", "decoding: {e}")),
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::Ordering;

    /// Streaming da un finto yt-dlp che scrive un m4a frammentato un po' alla volta.
    /// Serve ffmpeg (si salta se manca).
    #[tokio::test]
    async fn streams_while_downloading() {
        let dir = std::env::temp_dir().join(format!("ae-stream-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let src = dir.join("src.m4a");
        let ok = std::process::Command::new("ffmpeg")
            .args(["-loglevel", "error", "-f", "lavfi", "-i", "sine=frequency=440:duration=12", "-c:a", "aac", "-movflags", "+frag_keyframe+empty_moov+default_base_moof"])
            .arg(&src)
            .status()
            .is_ok_and(|s| s.success());
        if !ok {
            eprintln!("ffmpeg mancante: test saltato");
            return;
        }
        // finto yt-dlp: stampa i metadati, poi scrive il .part a pezzi e lo rinomina
        let fake = dir.join("yt-dlp");
        std::fs::write(
            &fake,
            format!(
                "#!/usr/bin/env python3\nimport sys,time,os,json\nout=sys.argv[sys.argv.index('-o')+1].replace('%(id)s','abc').replace('%(ext)s','m4a')\n\
print(json.dumps({{'id':'abc','title':'Test','ext':'m4a','duration':12}}),flush=True)\n\
data=open({:?},'rb').read()\nwith open(out+'.part','wb') as f:\n  for i in range(0,len(data),8192):\n    f.write(data[i:i+8192]); f.flush(); time.sleep(0.02)\nos.replace(out+'.part',out)\n",
                src.to_string_lossy()
            ),
        )
        .unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        let tools = super::super::ytdlp::Tools { ytdlp: fake, deno: None };
        let cache = dir.join("cache");
        let t0 = Instant::now();
        let (info, dl) = super::super::ytdlp::fetch(&tools, "prova", &cache).await.unwrap();
        assert_eq!(info.id, "abc");
        assert!(!dl.is_done(), "deve tornare prima della fine del download");
        let started = t0.elapsed();
        // legge tutto in streaming mentre il file cresce
        let dl2 = dl.clone();
        let frames = tokio::task::spawn_blocking(move || {
            let alive: Arc<dyn Fn() -> bool + Send + Sync> = Arc::new(|| true);
            let mut format = open_stream(&dl2, &alive).expect("formato apribile in streaming");
            let track = format.default_track().unwrap().clone();
            let mut dec = symphonia::default::get_codecs().make(&track.codec_params, &DecoderOptions::default()).unwrap();
            let mut n = 0usize;
            while let Ok(p) = format.next_packet() {
                if let Ok(d) = dec.decode(&p) {
                    n += d.frames();
                }
            }
            (n, track.codec_params.sample_rate.unwrap_or(44_100))
        })
        .await
        .unwrap();
        let secs = frames.0 as f64 / frames.1 as f64;
        assert!((secs - 12.0).abs() < 0.3, "letti {secs:.2} s");
        // il download finisce, la ricerca resta in cache
        for _ in 0..100 {
            if dl.done.load(Ordering::SeqCst) {
                break;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        assert!(dl.is_done() && dl.file.is_file());
        let (_, hit) = super::super::ytdlp::cached("  Prova ", &cache).expect("ricerca in cache");
        assert!(hit.is_done());
        assert!(started < t0.elapsed());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
