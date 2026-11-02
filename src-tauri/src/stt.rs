//! Speech-to-text: push-to-talk → registrazione dal microfono (cpal) → WAV 16 kHz →
//! trascrizione Whisper (Groq, oppure un server Whisper locale compatibile OpenAI).

use crate::{config::SttConfig, keys};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use serde::Serialize;
use std::{
    io::Cursor,
    sync::{mpsc, Arc, Mutex},
    thread::JoinHandle,
    time::Duration,
};

const TARGET_RATE: u32 = 16_000;
const MAX_SECONDS: u64 = 60;
const MIN_SECONDS: f32 = 0.35;

#[derive(Serialize, Clone)]
#[serde(tag = "state", rename_all = "lowercase")]
pub enum SttEvent {
    Recording,
    Transcribing,
    Done { text: String },
    /// Frase della clip riconosciuta ed eseguita (senza modello).
    Clip { ok: bool, message: String },
    Empty,
    Error { message: String },
}

type Captured = Result<(Vec<f32>, u32), String>;

#[derive(Default)]
pub struct Recorder {
    stop: Mutex<Option<mpsc::Sender<()>>>,
    worker: Mutex<Option<JoinHandle<Captured>>>,
}

impl Recorder {
    pub fn is_recording(&self) -> bool {
        self.worker.lock().unwrap().is_some()
    }

    /// Avvia la registrazione in un thread dedicato (lo stream cpal non è Send).
    pub fn start(&self) -> Result<(), String> {
        let mut worker = self.worker.lock().unwrap();
        if worker.is_some() {
            return Ok(());
        }
        let (tx, rx) = mpsc::channel::<()>();
        let (ready_tx, ready_rx) = mpsc::channel::<Result<(), String>>();
        *worker = Some(std::thread::spawn(move || capture(rx, ready_tx)));
        *self.stop.lock().unwrap() = Some(tx);
        drop(worker);
        match ready_rx.recv_timeout(Duration::from_secs(5)) {
            Ok(Ok(())) => Ok(()),
            Ok(Err(e)) => {
                let _ = self.finish();
                Err(e)
            }
            Err(_) => {
                let _ = self.finish();
                Err("il microfono non risponde".into())
            }
        }
    }

    /// Ferma la registrazione e restituisce il WAV (None se troppo corta).
    pub fn finish(&self) -> Result<Option<Vec<u8>>, String> {
        if let Some(tx) = self.stop.lock().unwrap().take() {
            let _ = tx.send(());
        }
        let Some(handle) = self.worker.lock().unwrap().take() else {
            return Ok(None);
        };
        let (samples, rate) = handle.join().map_err(|_| "registrazione interrotta".to_string())??;
        if (samples.len() as f32) < rate as f32 * MIN_SECONDS {
            return Ok(None);
        }
        Ok(Some(to_wav(&resample(&samples, rate, TARGET_RATE))?))
    }
}

fn capture(stop: mpsc::Receiver<()>, ready: mpsc::Sender<Result<(), String>>) -> Captured {
    let fail = |msg: String| {
        let _ = ready.send(Err(msg.clone()));
        Err(msg)
    };
    let host = cpal::default_host();
    let Some(device) = host.default_input_device() else {
        return fail("nessun microfono trovato".into());
    };
    let config = match device.default_input_config() {
        Ok(c) => c,
        Err(e) => return fail(format!("microfono non utilizzabile: {e}")),
    };
    let rate = config.sample_rate().0;
    let channels = config.channels() as usize;
    let buf = Arc::new(Mutex::new(Vec::<f32>::with_capacity(rate as usize * 10)));
    let max = rate as usize * MAX_SECONDS as usize;

    // media dei canali → mono
    fn push<T: Copy>(buf: &Mutex<Vec<f32>>, data: &[T], channels: usize, max: usize, conv: impl Fn(T) -> f32) {
        let mut b = buf.lock().unwrap();
        for frame in data.chunks(channels) {
            if b.len() >= max {
                break;
            }
            b.push(frame.iter().map(|s| conv(*s)).sum::<f32>() / channels as f32);
        }
    }
    let err_fn = |e| eprintln!("errore microfono: {e}");
    let b = buf.clone();
    let stream = match config.sample_format() {
        cpal::SampleFormat::F32 => device.build_input_stream(
            &config.into(), move |d: &[f32], _: &_| push(&b, d, channels, max, |s| s), err_fn, None),
        cpal::SampleFormat::I16 => device.build_input_stream(
            &config.into(), move |d: &[i16], _: &_| push(&b, d, channels, max, |s| s as f32 / 32768.0), err_fn, None),
        cpal::SampleFormat::U16 => device.build_input_stream(
            &config.into(), move |d: &[u16], _: &_| push(&b, d, channels, max, |s| (s as f32 - 32768.0) / 32768.0), err_fn, None),
        other => return fail(format!("formato audio non supportato: {other:?}")),
    };
    let stream = match stream {
        Ok(s) => s,
        Err(e) => return fail(format!("impossibile aprire il microfono: {e}")),
    };
    if let Err(e) = stream.play() {
        return fail(format!("impossibile avviare il microfono: {e}"));
    }
    let _ = ready.send(Ok(()));
    let _ = stop.recv_timeout(Duration::from_secs(MAX_SECONDS));
    drop(stream);
    let samples = std::mem::take(&mut *buf.lock().unwrap());
    Ok((samples, rate))
}

/// Ricampionamento lineare (sufficiente per la voce).
pub fn resample(input: &[f32], from: u32, to: u32) -> Vec<f32> {
    if from == to || input.is_empty() {
        return input.to_vec();
    }
    let ratio = from as f64 / to as f64;
    let n = (input.len() as f64 / ratio) as usize;
    (0..n)
        .map(|i| {
            let pos = i as f64 * ratio;
            let j = pos as usize;
            let frac = (pos - j as f64) as f32;
            let a = input[j];
            let b = *input.get(j + 1).unwrap_or(&a);
            a + (b - a) * frac
        })
        .collect()
}

pub fn to_wav(samples: &[f32]) -> Result<Vec<u8>, String> {
    let spec = hound::WavSpec { channels: 1, sample_rate: TARGET_RATE, bits_per_sample: 16, sample_format: hound::SampleFormat::Int };
    let mut cur = Cursor::new(Vec::new());
    {
        let mut w = hound::WavWriter::new(&mut cur, spec).map_err(|e| e.to_string())?;
        for s in samples {
            w.write_sample((s.clamp(-1.0, 1.0) * 32767.0) as i16).map_err(|e| e.to_string())?;
        }
        w.finalize().map_err(|e| e.to_string())?;
    }
    Ok(cur.into_inner())
}

/// Invia il WAV all'endpoint `/audio/transcriptions` (formato OpenAI).
/// `groq_keys`: chiavi Groq in ordine; se una fallisce (401/403/429) prova la successiva.
pub async fn transcribe(http: &reqwest::Client, cfg: &SttConfig, wav: Vec<u8>, groq_keys: Vec<String>) -> Result<String, String> {
    let (base, keys_list) = match cfg.provider.as_str() {
        "groq" => {
            if groq_keys.is_empty() {
                return Err("serve una chiave Groq (Impostazioni → AI → Groq)".into());
            }
            ("https://api.groq.com/openai/v1".to_string(), groq_keys.into_iter().map(Some).collect::<Vec<_>>())
        }
        "local" => (cfg.base_url.trim().trim_end_matches('/').to_string(), vec![None]),
        _ => return Err("riconoscimento vocale disattivato nelle impostazioni".into()),
    };
    let model = if cfg.model.trim().is_empty() { "whisper-large-v3-turbo" } else { cfg.model.trim() };
    let mut last = String::new();
    for key in keys_list {
        let part = reqwest::multipart::Part::bytes(wav.clone())
            .file_name("voce.wav")
            .mime_str("audio/wav")
            .map_err(|e| e.to_string())?;
        let mut form = reqwest::multipart::Form::new()
            .part("file", part)
            .text("model", model.to_string())
            .text("response_format", "json")
            .text("temperature", "0");
        if !cfg.language.trim().is_empty() {
            form = form.text("language", cfg.language.trim().to_string());
        }
        let mut req = http.post(format!("{base}/audio/transcriptions")).multipart(form).timeout(Duration::from_secs(60));
        if let Some(k) = &key {
            req = req.bearer_auth(k);
        }
        let resp = req.send().await.map_err(|e| {
            if e.is_connect() { "server di trascrizione non raggiungibile".to_string() } else { format!("errore di rete: {e}") }
        })?;
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        if status.is_success() {
            let v: serde_json::Value = serde_json::from_str(&body).map_err(|_| "risposta di trascrizione non valida".to_string())?;
            return Ok(v.get("text").and_then(|t| t.as_str()).unwrap_or("").trim().to_string());
        }
        let detail: String = body.chars().take(200).collect();
        last = keys::redact(&format!("trascrizione fallita (HTTP {}): {detail}", status.as_u16()), key.as_deref());
        if !matches!(status.as_u16(), 401 | 402 | 403 | 429) {
            break; // errore non legato alla chiave: inutile provare le altre
        }
    }
    Err(last)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resample_and_wav() {
        let input: Vec<f32> = (0..48_000).map(|i| (i as f32 / 10.0).sin()).collect();
        let out = resample(&input, 48_000, 16_000);
        assert_eq!(out.len(), 16_000);
        let wav = to_wav(&out).unwrap();
        assert_eq!(&wav[..4], b"RIFF");
        assert_eq!(wav.len(), 44 + 16_000 * 2);
    }

    /// Richiede tools/mock_openai.py in esecuzione.
    #[tokio::test]
    async fn transcribe_local_mock() {
        let cfg = SttConfig { provider: "local".into(), base_url: "http://127.0.0.1:18080/ok/v1/".into(), ..Default::default() };
        let wav = to_wav(&vec![0.0; 8000]).unwrap();
        let text = transcribe(&reqwest::Client::new(), &cfg, wav, vec![]).await.unwrap();
        assert_eq!(text, "Ciao Alter Ego");
        let off = SttConfig { provider: "off".into(), ..Default::default() };
        assert!(transcribe(&reqwest::Client::new(), &off, vec![], vec![]).await.is_err());
    }
}
