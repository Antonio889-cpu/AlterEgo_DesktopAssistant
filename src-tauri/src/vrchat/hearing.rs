//! Udito: cattura l'audio che VRChat suona (uscita del sistema in "loopback"), ne taglia le frasi
//! con il VAD e le passa alla trascrizione. L'audio di VRChat è spaziale: chi è vicino si sente più
//! forte, e il bilanciamento sinistra/destra dice da che parte parla.
//! - Windows: WASAPI loopback (cpal apre il dispositivo di uscita come ingresso);
//! - Linux: la sorgente "Monitor of …" di PulseAudio/PipeWire, se cpal la vede; altrimenti niente.
//! Mentre parla Alter Ego l'audio si scarta (`mute`): la sua voce non va trascritta.

use super::vad::{self, Segment, StereoResampler, Vad};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc, Arc, Mutex,
};
use std::time::Duration;

#[derive(Default)]
pub struct Ears {
    /// Qualcuno sta parlando adesso (turni: non parlargli sopra).
    pub talking: AtomicBool,
    pub stop: AtomicBool,
    /// Dispositivo in ascolto o errore (per lo stato).
    pub device: Mutex<String>,
    pub error: Mutex<Option<String>>,
}

/// Dispositivo da cui ascoltare.
fn open(name: &str) -> Result<(cpal::Device, cpal::SupportedStreamConfig), String> {
    #[cfg(windows)]
    {
        let dev = crate::audio::find_output(name).or_else(|| cpal::default_host().default_output_device()).ok_or_else(|| crate::i18n::tr("nessuna uscita audio", "no audio output").to_string())?;
        let cfg = dev.default_output_config().map_err(|e| e.to_string())?;
        Ok((dev, cfg))
    }
    #[cfg(not(windows))]
    {
        let host = cpal::default_host();
        let want = name.to_lowercase();
        let dev = host
            .input_devices()
            .map_err(|e| e.to_string())?
            .find(|d| d.name().is_ok_and(|n| {
                let n = n.to_lowercase();
                n.contains("monitor") && (want.is_empty() || n.contains(&want))
            }))
            .ok_or_else(|| crate::i18n::tr("ascolto dell'audio del PC non disponibile su Linux (nessuna sorgente «Monitor»)", "listening to the PC audio is not available on Linux (no \"Monitor\" source)").to_string())?;
        let cfg = dev.default_input_config().map_err(|e| e.to_string())?;
        Ok((dev, cfg))
    }
}

/// Avvia l'ascolto in un thread (lo stream cpal non è Send). Si ferma con `ears.stop`.
pub fn start(device: String, ears: Arc<Ears>, mute: impl Fn() -> bool + Send + 'static, on_segment: impl Fn(Segment) + Send + 'static) {
    ears.stop.store(false, Ordering::SeqCst);
    let _ = std::thread::Builder::new().name("vrchat-udito".into()).spawn(move || {
        let (dev, cfg) = match open(&device) {
            Ok(x) => x,
            Err(e) => {
                crate::applog::warn("vrchat", &format!("udito: {e}"));
                *ears.error.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = Some(e);
                return;
            }
        };
        let rate = cfg.sample_rate().0;
        let channels = cfg.channels() as usize;
        let (tx, rx) = mpsc::channel::<Vec<f32>>();
        let err_fn = |e| eprintln!("udito VRChat: {e}");
        let sc: cpal::StreamConfig = cfg.clone().into();
        let stream = match cfg.sample_format() {
            cpal::SampleFormat::F32 => {
                let tx = tx.clone();
                dev.build_input_stream(&sc, move |d: &[f32], _: &_| { let _ = tx.send(d.to_vec()); }, err_fn, None)
            }
            cpal::SampleFormat::I16 => {
                let tx = tx.clone();
                dev.build_input_stream(&sc, move |d: &[i16], _: &_| { let _ = tx.send(d.iter().map(|s| *s as f32 / 32768.0).collect()); }, err_fn, None)
            }
            cpal::SampleFormat::U16 => {
                let tx = tx.clone();
                dev.build_input_stream(&sc, move |d: &[u16], _: &_| { let _ = tx.send(d.iter().map(|s| (*s as f32 - 32768.0) / 32768.0).collect()); }, err_fn, None)
            }
            other => {
                *ears.error.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = Some(format!("formato audio non supportato: {other:?}"));
                return;
            }
        };
        let stream = match stream.map_err(|e| e.to_string()).and_then(|s| s.play().map(|_| s).map_err(|e| e.to_string())) {
            Ok(s) => s,
            Err(e) => {
                crate::applog::warn("vrchat", &format!("udito: {e}"));
                *ears.error.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = Some(e);
                return;
            }
        };
        let name = dev.name().unwrap_or_default();
        crate::applog::info("vrchat", &format!("udito: ascolto {name} ({rate} Hz, {channels} canali)"));
        *ears.device.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = name;
        *ears.error.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = None;
        let mut rs = StereoResampler::new(rate, vad::RATE);
        let mut v = Vad::default();
        let mut buf = Vec::new();
        while !ears.stop.load(Ordering::SeqCst) {
            let Ok(first) = rx.recv_timeout(Duration::from_millis(200)) else { continue };
            let mut data = first;
            while let Ok(more) = rx.try_recv() {
                data.extend(more);
            }
            if mute() {
                v.reset();
                ears.talking.store(false, Ordering::Relaxed);
                continue;
            }
            buf.clear();
            rs.push(&vad::to_lr(&data, channels), &mut buf);
            for seg in v.push_stereo(&buf) {
                on_segment(seg);
            }
            ears.talking.store(v.talking(), Ordering::Relaxed);
        }
        drop(stream);
        ears.talking.store(false, Ordering::Relaxed);
        crate::applog::info("vrchat", "udito: fermo");
    });
}

/// Le frasi che Whisper "sente" nel rumore o nella musica: si scartano.
pub fn hallucination(text: &str) -> bool {
    let t = text.trim().trim_matches(|c: char| c.is_ascii_punctuation() || c.is_whitespace()).to_lowercase();
    if t.chars().filter(|c| c.is_alphanumeric()).count() < 2 {
        return true;
    }
    const BAD: &[&str] = &[
        "thank you", "thanks for watching", "thank you for watching", "grazie", "grazie a tutti", "grazie per la visione", "sottotitoli",
        "sottotitoli creati dalla comunità amara.org", "subtitles by the amara.org community", "you", "bye", "ciao", "music", "musica",
        "please subscribe", "iscriviti al canale", "...",
    ];
    BAD.contains(&t.as_str()) || t.contains("amara.org")
}

/// WAV pronto per la trascrizione (mono 16 kHz).
pub fn wav(seg: &Segment) -> Result<Vec<u8>, String> {
    crate::stt::to_wav(&seg.samples)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filters_whisper_noise() {
        assert!(hallucination("Thank you."));
        assert!(hallucination("  Grazie a tutti! "));
        assert!(hallucination("Sottotitoli creati dalla comunità Amara.org"));
        assert!(hallucination("."));
        assert!(!hallucination("Ciao Alter Ego, vieni qui"));
        assert!(!hallucination("Thank you so much for the help"));
        let seg = Segment { samples: vec![0.0; 16_000], balance: 0.0, level: 0.1 };
        assert_eq!(&wav(&seg).unwrap()[..4], b"RIFF");
    }
}
