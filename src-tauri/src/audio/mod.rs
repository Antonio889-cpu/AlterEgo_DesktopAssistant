//! Motore audio: tutta l'uscita di Alter Ego (voce, blip, jingle) passa da qui.
//!
//! - Uscita sugli altoparlanti/cuffie scelti nelle impostazioni (o i predefiniti di Windows).
//! - Mic-Link: un secondo flusso scrive nel lato "riproduzione" di un cavo audio virtuale
//!   (VB-CABLE, "CABLE Input") il microfono scelto + la voce di Alter Ego. Le altre app
//!   usano come microfono il lato "registrazione" del cavo, rinominato "AlterEgo Link Audio".
//!
//! Gli stream cpal non sono `Send`: vivono in un thread dedicato che li ricrea quando
//! cambiano i dispositivi. I callback leggono da code condivise (mono, f32, al rate del dispositivo).

pub mod commands;
pub mod kokoro;
#[cfg(windows)]
pub mod rename;
pub mod synth;
pub mod tts;

use crate::config::AudioConfig;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use serde::Serialize;
use std::{
    collections::VecDeque,
    sync::{
        atomic::{AtomicU64, Ordering},
        mpsc, Arc, Mutex,
    },
    time::Duration,
};

/// Nome del microfono virtuale (lato registrazione del cavo, dopo la rinomina).
pub const LINK_NAME: &str = "AlterEgo Link Audio";
/// Latenza massima del microfono nel cavo: oltre si scarta l'audio più vecchio.
const MIC_MAX_SECONDS: f32 = 0.15;

/// Una coda di uscita: `voice` è sequenziale (le frasi si accodano), `fx` si mixa
/// "da adesso" (i blip si sovrappongono a ciò che sta suonando).
#[derive(Default)]
struct Sink {
    rate: u32,
    active: bool,
    voice: VecDeque<f32>,
    fx: VecDeque<f32>,
}

impl Sink {
    fn next(&mut self) -> f32 {
        self.voice.pop_front().unwrap_or(0.0) + self.fx.pop_front().unwrap_or(0.0)
    }
    fn clear(&mut self) {
        self.voice.clear();
        self.fx.clear();
    }
}

fn mix_into(buf: &mut VecDeque<f32>, samples: &[f32]) {
    let n = buf.len();
    for (i, s) in samples.iter().enumerate() {
        if i < n {
            buf[i] += s;
        } else {
            buf.push_back(*s);
        }
    }
}

#[derive(Default)]
struct Shared {
    speaker: Mutex<Sink>,
    link: Mutex<Sink>,
    /// Microfono già ricampionato al rate del cavo.
    mic: Mutex<VecDeque<f32>>,
    gains: Mutex<LinkGains>,
    status: Mutex<Status>,
}

#[derive(Default, Clone, Copy)]
struct LinkGains {
    mic: f32,
    voice: f32,
    blips: bool,
}

/// Stato visibile nelle impostazioni.
#[derive(Serialize, Clone, Default, Debug)]
pub struct Status {
    /// Dispositivo di uscita effettivamente in uso.
    pub output: String,
    pub output_error: Option<String>,
    pub link_active: bool,
    /// Lato riproduzione del cavo usato da Mic-Link.
    pub link_cable: Option<String>,
    pub link_mic: Option<String>,
    pub link_error: Option<String>,
}

#[derive(Clone, PartialEq, Debug)]
struct Setup {
    output: String,
    link: Option<(String, String)>, // (microfono, lato riproduzione del cavo)
}

#[derive(Clone, Copy, PartialEq)]
pub enum Kind {
    Voice,
    Fx,
}

pub struct AudioEngine {
    tx: Mutex<mpsc::Sender<Setup>>,
    shared: Arc<Shared>,
    last: Mutex<Option<Setup>>,
    /// Ogni nuova frase/stop incrementa: le sintesi vecchie non vengono riprodotte.
    pub voice_gen: AtomicU64,
}

impl AudioEngine {
    pub fn new() -> Self {
        let shared = Arc::new(Shared::default());
        let (tx, rx) = mpsc::channel::<Setup>();
        let sh = shared.clone();
        std::thread::Builder::new()
            .name("audio".into())
            .spawn(move || run(rx, sh))
            .expect("thread audio");
        Self { tx: Mutex::new(tx), shared, last: Mutex::new(None), voice_gen: AtomicU64::new(0) }
    }

    /// Applica dispositivi e Mic-Link dalla config (ricrea gli stream solo se cambiano).
    pub fn configure(&self, a: &AudioConfig) {
        let ml = &a.mic_link;
        *self.shared.gains.lock().unwrap() =
            LinkGains { mic: ml.mic_volume as f32, voice: ml.voice_volume as f32, blips: ml.include_blips };
        let setup = Setup {
            output: a.output_device.clone(),
            link: ml.enabled.then(|| (ml.mic_device.clone(), ml.cable_device.clone())),
        };
        let mut last = self.last.lock().unwrap();
        if last.as_ref() != Some(&setup) {
            *last = Some(setup.clone());
            let _ = self.tx.lock().unwrap().send(setup);
        }
    }

    /// Riproduce campioni mono. Restituisce la durata in secondi.
    pub fn play(&self, samples: &[f32], rate: u32, kind: Kind) -> f32 {
        if samples.is_empty() || rate == 0 {
            return 0.0;
        }
        let gains = *self.shared.gains.lock().unwrap();
        for (sink, gain, is_link) in [(&self.shared.speaker, 1.0, false), (&self.shared.link, gains.voice, true)] {
            let mut s = sink.lock().unwrap();
            if !s.active || (is_link && kind == Kind::Fx && !gains.blips) {
                continue;
            }
            let mut data = crate::stt::resample(samples, rate, s.rate);
            if gain != 1.0 {
                data.iter_mut().for_each(|x| *x *= gain);
            }
            match kind {
                Kind::Voice => s.voice.extend(data),
                Kind::Fx => mix_into(&mut s.fx, &data),
            }
        }
        samples.len() as f32 / rate as f32
    }

    /// Secondi di voce ancora da riprodurre sugli altoparlanti.
    pub fn voice_remaining(&self) -> f32 {
        let s = self.shared.speaker.lock().unwrap();
        if s.rate == 0 { 0.0 } else { s.voice.len() as f32 / s.rate as f32 }
    }

    /// Ferma la voce (e i suoni) in corso.
    pub fn stop(&self) {
        self.voice_gen.fetch_add(1, Ordering::SeqCst);
        self.shared.speaker.lock().unwrap().clear();
        self.shared.link.lock().unwrap().clear();
    }

    pub fn status(&self) -> Status {
        self.shared.status.lock().unwrap().clone()
    }
}

// ---------- dispositivi ----------

#[derive(Serialize, Clone, Default, Debug)]
pub struct Devices {
    pub outputs: Vec<String>,
    pub inputs: Vec<String>,
    pub default_output: Option<String>,
    pub default_input: Option<String>,
    /// Lato riproduzione del cavo virtuale trovato (es. "CABLE Input (VB-Audio Virtual Cable)").
    pub cable_render: Option<String>,
    /// Lato registrazione del cavo (quello che le altre app usano come microfono).
    pub cable_capture: Option<String>,
    /// Il lato registrazione si chiama già "AlterEgo Link Audio".
    pub link_named: bool,
}

pub fn is_cable_render(name: &str) -> bool {
    let n = name.to_lowercase();
    n.contains("cable input") || (n.contains("alterego link") && !n.contains(&LINK_NAME.to_lowercase()))
}

pub fn is_cable_capture(name: &str) -> bool {
    let n = name.to_lowercase();
    n.contains("cable output") || n.contains(&LINK_NAME.to_lowercase())
}

pub fn devices() -> Devices {
    let host = cpal::default_host();
    let outputs: Vec<String> =
        host.output_devices().map(|d| d.filter_map(|x| x.name().ok()).collect()).unwrap_or_default();
    let inputs: Vec<String> =
        host.input_devices().map(|d| d.filter_map(|x| x.name().ok()).collect()).unwrap_or_default();
    let cable_render = outputs.iter().find(|n| is_cable_render(n)).cloned();
    let cable_capture = inputs.iter().find(|n| is_cable_capture(n)).cloned();
    let link_named = cable_capture.as_deref().is_some_and(|n| n.contains(LINK_NAME));
    Devices {
        default_output: host.default_output_device().and_then(|d| d.name().ok()),
        default_input: host.default_input_device().and_then(|d| d.name().ok()),
        outputs,
        inputs,
        cable_render,
        cable_capture,
        link_named,
    }
}

pub fn find_output(name: &str) -> Option<cpal::Device> {
    let host = cpal::default_host();
    if name.is_empty() {
        return host.default_output_device();
    }
    host.output_devices().ok()?.find(|d| d.name().is_ok_and(|n| n == name))
}

pub fn find_input(name: &str) -> Option<cpal::Device> {
    let host = cpal::default_host();
    if name.is_empty() {
        return host.default_input_device();
    }
    host.input_devices().ok()?.find(|d| d.name().is_ok_and(|n| n == name))
}

// ---------- thread degli stream ----------

fn run(rx: mpsc::Receiver<Setup>, sh: Arc<Shared>) {
    let mut streams: Vec<cpal::Stream> = Vec::new();
    while let Ok(mut setup) = rx.recv() {
        // più cambi ravvicinati (cursore sulle impostazioni): applica l'ultimo
        while let Ok(s) = rx.recv_timeout(Duration::from_millis(150)) {
            setup = s;
        }
        streams.clear(); // chiude i vecchi
        for s in [&sh.speaker, &sh.link] {
            let mut s = s.lock().unwrap();
            s.active = false;
            s.clear();
        }
        sh.mic.lock().unwrap().clear();
        let mut status = Status::default();

        // altoparlanti
        match open_output(&setup.output, sh.clone(), false) {
            Ok((name, st)) => {
                status.output = name;
                streams.push(st);
            }
            Err(e) => status.output_error = Some(e),
        }

        // Mic-Link
        if let Some((mic, cable)) = &setup.link {
            let cable_name = if cable.is_empty() { devices().cable_render } else { Some(cable.clone()) };
            match cable_name {
                None => status.link_error = Some("cavo virtuale non trovato: installa VB-CABLE (vedi Impostazioni → Voce)".into()),
                Some(cable_name) => match open_output(&cable_name, sh.clone(), true) {
                    Err(e) => status.link_error = Some(e),
                    Ok((cable_name, out)) => {
                        let rate = sh.link.lock().unwrap().rate;
                        match open_mic(mic, rate, sh.clone()) {
                            Ok((mic_name, inp)) if is_cable_capture(&mic_name) => {
                                drop(inp);
                                status.link_error = Some("il microfono scelto è il cavo stesso: scegli il microfono vero".into());
                            }
                            Ok((mic_name, inp)) => {
                                streams.push(out);
                                streams.push(inp);
                                status.link_active = true;
                                status.link_cable = Some(cable_name);
                                status.link_mic = Some(mic_name);
                            }
                            Err(e) => {
                                // senza microfono passa comunque la voce di Alter Ego
                                streams.push(out);
                                status.link_active = true;
                                status.link_cable = Some(cable_name);
                                status.link_error = Some(format!("microfono: {e}"));
                            }
                        }
                    }
                },
            }
            if !status.link_active {
                sh.link.lock().unwrap().active = false;
            }
        }
        *sh.status.lock().unwrap() = status;
    }
}

/// Apre un'uscita che legge dalla coda `speaker` o `link`.
fn open_output(name: &str, sh: Arc<Shared>, link: bool) -> Result<(String, cpal::Stream), String> {
    let mut device = find_output(name);
    if device.is_none() && !link {
        // dispositivo scollegato: si torna al predefinito
        device = cpal::default_host().default_output_device();
    }
    let device = device.ok_or_else(|| format!("dispositivo di uscita non trovato: {name}"))?;
    let dev_name = device.name().unwrap_or_default();
    let config = device.default_output_config().map_err(|e| format!("{dev_name}: {e}"))?;
    let rate = config.sample_rate().0;
    let channels = config.channels() as usize;
    {
        let mut s = if link { sh.link.lock().unwrap() } else { sh.speaker.lock().unwrap() };
        s.rate = rate;
        s.clear();
    }
    let err_fn = |e| eprintln!("errore uscita audio: {e}");
    let sh2 = sh.clone();

    // campione successivo (mono) per questa uscita
    let next = move || -> f32 {
        if link {
            let g = *sh.gains.lock().unwrap();
            let m = sh.mic.lock().unwrap().pop_front().unwrap_or(0.0) * g.mic;
            (m + sh.link.lock().unwrap().next()).clamp(-1.0, 1.0)
        } else {
            sh.speaker.lock().unwrap().next().clamp(-1.0, 1.0)
        }
    };
    fn fill<T: Copy>(out: &mut [T], channels: usize, next: &mut impl FnMut() -> f32, conv: impl Fn(f32) -> T) {
        for frame in out.chunks_mut(channels) {
            let v = conv(next());
            frame.iter_mut().for_each(|s| *s = v);
        }
    }
    let mut n = next;
    let stream = match config.sample_format() {
        cpal::SampleFormat::F32 => device.build_output_stream(
            &config.into(), move |d: &mut [f32], _: &_| fill(d, channels, &mut n, |v| v), err_fn, None),
        cpal::SampleFormat::I16 => device.build_output_stream(
            &config.into(), move |d: &mut [i16], _: &_| fill(d, channels, &mut n, |v| (v * 32767.0) as i16), err_fn, None),
        cpal::SampleFormat::U16 => device.build_output_stream(
            &config.into(), move |d: &mut [u16], _: &_| fill(d, channels, &mut n, |v| ((v + 1.0) * 32767.5) as u16), err_fn, None),
        other => return Err(format!("{dev_name}: formato non supportato ({other:?})")),
    }
    .map_err(|e| format!("{dev_name}: {e}"))?;
    stream.play().map_err(|e| format!("{dev_name}: {e}"))?;
    // la coda si riempie solo quando lo stream gira davvero
    if link { sh2.link.lock().unwrap() } else { sh2.speaker.lock().unwrap() }.active = true;
    Ok((dev_name, stream))
}

/// Linear resampler con stato, per il microfono in tempo reale.
struct Resampler {
    step: f64,
    pos: f64,
    last: f32,
}

impl Resampler {
    fn new(from: u32, to: u32) -> Self {
        Self { step: from as f64 / to as f64, pos: 0.0, last: 0.0 }
    }
    /// Converte un blocco; `pos` è la posizione del prossimo campione d'uscita rispetto
    /// all'inizio del blocco (-1..0 = tra l'ultimo campione del blocco precedente e il primo).
    fn push(&mut self, input: &[f32], out: &mut VecDeque<f32>) {
        let n = input.len() as f64;
        let last = self.last;
        let get = |i: isize| if i < 0 { last } else { input[i as usize] };
        while self.pos <= n - 1.0 {
            let i = self.pos.floor();
            let frac = (self.pos - i) as f32;
            let a = get(i as isize);
            let b = if frac == 0.0 { a } else { get(i as isize + 1) };
            out.push_back(a + (b - a) * frac);
            self.pos += self.step;
        }
        if let Some(l) = input.last() {
            self.last = *l;
        }
        self.pos -= n;
    }
}

fn open_mic(name: &str, link_rate: u32, sh: Arc<Shared>) -> Result<(String, cpal::Stream), String> {
    let device = find_input(name).ok_or_else(|| format!("microfono non trovato: {name}"))?;
    let dev_name = device.name().unwrap_or_default();
    let config = device.default_input_config().map_err(|e| format!("{dev_name}: {e}"))?;
    let rate = config.sample_rate().0;
    let channels = config.channels() as usize;
    let max = (link_rate as f32 * MIC_MAX_SECONDS) as usize;
    let mut rs = Resampler::new(rate, link_rate);
    let mut mono = Vec::<f32>::new();
    let mut on_data = move |frames: &mut dyn Iterator<Item = f32>| {
        mono.clear();
        let mut acc = 0.0;
        for (i, s) in frames.enumerate() {
            acc += s;
            if (i + 1) % channels == 0 {
                mono.push(acc / channels as f32);
                acc = 0.0;
            }
        }
        let mut q = sh.mic.lock().unwrap();
        rs.push(&mono, &mut q);
        let len = q.len();
        if len > max {
            q.drain(..len - max);
        }
    };
    let err_fn = |e| eprintln!("errore microfono (Mic-Link): {e}");
    let stream = match config.sample_format() {
        cpal::SampleFormat::F32 => device.build_input_stream(
            &config.into(), move |d: &[f32], _: &_| on_data(&mut d.iter().copied()), err_fn, None),
        cpal::SampleFormat::I16 => device.build_input_stream(
            &config.into(), move |d: &[i16], _: &_| on_data(&mut d.iter().map(|s| *s as f32 / 32768.0)), err_fn, None),
        cpal::SampleFormat::U16 => device.build_input_stream(
            &config.into(), move |d: &[u16], _: &_| on_data(&mut d.iter().map(|s| (*s as f32 - 32768.0) / 32768.0)), err_fn, None),
        other => return Err(format!("{dev_name}: formato non supportato ({other:?})")),
    }
    .map_err(|e| format!("{dev_name}: {e}"))?;
    stream.play().map_err(|e| format!("{dev_name}: {e}"))?;
    Ok((dev_name, stream))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mix_overlaps_from_now() {
        let mut q: VecDeque<f32> = [1.0, 1.0].into_iter().collect();
        mix_into(&mut q, &[0.5, 0.5, 0.5]);
        assert_eq!(q.iter().copied().collect::<Vec<_>>(), vec![1.5, 1.5, 0.5]);
    }

    #[test]
    fn streaming_resampler_matches_rate() {
        let mut rs = Resampler::new(44_100, 48_000);
        let mut out = VecDeque::new();
        let block: Vec<f32> = (0..441).map(|i| (i as f32 / 10.0).sin()).collect();
        for _ in 0..100 {
            rs.push(&block, &mut out);
        }
        // 1 secondo a 44,1 kHz → ~48000 campioni
        assert!((out.len() as i64 - 48_000).abs() < 10, "{}", out.len());
        // nessun salto tra un blocco e l'altro
        let v: Vec<f32> = out.into_iter().collect();
        assert!(v.windows(2).all(|w| (w[1] - w[0]).abs() < 0.2));
    }

    #[test]
    fn cable_names() {
        assert!(is_cable_render("CABLE Input (VB-Audio Virtual Cable)"));
        assert!(is_cable_capture("CABLE Output (VB-Audio Virtual Cable)"));
        assert!(is_cable_capture("AlterEgo Link Audio (VB-Audio Virtual Cable)"));
        assert!(!is_cable_render("AlterEgo Link Audio (VB-Audio Virtual Cable)"));
        assert!(!is_cable_render("Altoparlanti (Realtek)"));
    }

    #[test]
    fn link_sink_gets_voice_only_when_active() {
        let e = AudioEngine::new();
        {
            let mut s = e.shared.speaker.lock().unwrap();
            s.rate = 48_000;
            s.active = true;
        }
        e.shared.link.lock().unwrap().rate = 48_000;
        let d = e.play(&[0.5; 24_000], 24_000, Kind::Voice);
        assert!((d - 1.0).abs() < 1e-6);
        assert!((e.voice_remaining() - 1.0).abs() < 0.01);
        assert!(e.shared.link.lock().unwrap().voice.is_empty());
        e.stop();
        assert_eq!(e.voice_remaining(), 0.0);
    }
}
