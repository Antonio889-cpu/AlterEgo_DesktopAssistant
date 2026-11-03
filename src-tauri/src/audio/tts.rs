//! Sintesi vocale. Tutti i motori restituiscono audio PCM che viene riprodotto dal motore
//! audio (quindi sul dispositivo scelto e, con Mic-Link, anche nel microfono virtuale).
//!
//! - "windows": voci di sistema (WinRT `Windows.Media.SpeechSynthesis`, offline);
//! - "kokoro":  Kokoro-82M locale (server HTTP compatibile OpenAI, vedi kokoro.rs);
//! - "openai":  qualsiasi endpoint `/audio/speech` compatibile OpenAI.

use crate::config::AudioConfig;
use serde::Serialize;
use std::time::Duration;

pub struct Pcm {
    pub samples: Vec<f32>,
    pub rate: u32,
}

/// Legge un WAV PCM 8/16/24/32 bit o float 32. Tollera le lunghezze "in streaming"
/// (0 o 0xFFFFFFFF) che alcuni server mettono nell'intestazione.
pub fn decode_wav(b: &[u8]) -> Result<Pcm, String> {
    let bad = || "audio non valido (serve WAV)".to_string();
    if b.len() < 12 || &b[0..4] != b"RIFF" || &b[8..12] != b"WAVE" {
        return Err(bad());
    }
    let u16_at = |i: usize| u16::from_le_bytes([b[i], b[i + 1]]);
    let u32_at = |i: usize| u32::from_le_bytes([b[i], b[i + 1], b[i + 2], b[i + 3]]);
    let mut i = 12;
    let mut fmt: Option<(u16, usize, u32, usize)> = None; // formato, canali, rate, bit
    while i + 8 <= b.len() {
        let id = &b[i..i + 4];
        let size = u32_at(i + 4) as usize;
        let body = i + 8;
        if id == b"fmt " {
            if body + 16 > b.len() {
                return Err(bad());
            }
            let mut format = u16_at(body);
            if format == 0xFFFE && body + 26 <= b.len() {
                format = u16_at(body + 24); // WAVE_FORMAT_EXTENSIBLE: sottoformato
            }
            fmt = Some((format, u16_at(body + 2).max(1) as usize, u32_at(body + 4), u16_at(body + 14) as usize));
        } else if id == b"data" {
            let (format, ch, rate, bits) = fmt.ok_or_else(bad)?;
            let end = if size == 0 || size == u32::MAX as usize { b.len() } else { (body + size).min(b.len()) };
            let data = &b[body..end];
            let bytes = bits / 8;
            if bytes == 0 || rate == 0 {
                return Err(bad());
            }
            let sample = |s: &[u8]| -> f32 {
                match (format, bits) {
                    (3, 32) => f32::from_le_bytes([s[0], s[1], s[2], s[3]]),
                    (_, 8) => (s[0] as f32 - 128.0) / 128.0,
                    (_, 16) => i16::from_le_bytes([s[0], s[1]]) as f32 / 32768.0,
                    (_, 24) => (i32::from_le_bytes([0, s[0], s[1], s[2]]) >> 8) as f32 / 8_388_608.0,
                    (_, 32) => i32::from_le_bytes([s[0], s[1], s[2], s[3]]) as f32 / 2_147_483_648.0,
                    _ => 0.0,
                }
            };
            if !matches!(format, 1 | 3) || ![8, 16, 24, 32].contains(&bits) {
                return Err(format!("formato WAV non supportato ({format}, {bits} bit)"));
            }
            let samples = data
                .chunks_exact(bytes * ch)
                .map(|frame| frame.chunks_exact(bytes).map(sample).sum::<f32>() / ch as f32)
                .collect();
            return Ok(Pcm { samples, rate });
        }
        i = body + size + (size & 1);
    }
    Err(bad())
}

/// Spezza il testo in frasi (unite fino a ~40 caratteri): la prima si sente subito,
/// le altre si sintetizzano mentre la precedente suona.
pub fn chunks(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut cur = String::new();
    let chars: Vec<char> = text.chars().collect();
    for (i, &c) in chars.iter().enumerate() {
        cur.push(c);
        let end_mark = matches!(c, '.' | '!' | '?' | '…' | '。' | '！' | '？' | '\n');
        let next_is_space = chars.get(i + 1).is_none_or(|n| n.is_whitespace());
        if (end_mark && next_is_space && cur.trim().chars().count() >= 40) || cur.chars().count() >= 280 {
            if !cur.trim().is_empty() {
                out.push(cur.trim().to_string());
            }
            cur.clear();
        }
    }
    if !cur.trim().is_empty() {
        match out.last_mut() {
            // un pezzetto finale troppo corto va con la frase precedente
            Some(last) if cur.trim().chars().count() < 12 => {
                last.push(' ');
                last.push_str(cur.trim());
            }
            _ => out.push(cur.trim().to_string()),
        }
    }
    out
}

#[derive(Serialize, Clone, Debug)]
pub struct Voice {
    pub id: String,
    pub name: String,
    pub lang: String,
}

/// Voci Kokoro-82M (VOICES.md del modello). Le italiane sono le prime.
pub const KOKORO_VOICES: &[(&str, &str, &str)] = &[
    ("if_sara", "Sara (italiano, F)", "it"),
    ("im_nicola", "Nicola (italiano, M)", "it"),
    ("af_heart", "Heart (inglese US, F)", "en"),
    ("af_bella", "Bella (inglese US, F)", "en"),
    ("af_nicole", "Nicole (inglese US, F)", "en"),
    ("af_sky", "Sky (inglese US, F)", "en"),
    ("am_michael", "Michael (inglese US, M)", "en"),
    ("am_fenrir", "Fenrir (inglese US, M)", "en"),
    ("bf_emma", "Emma (inglese UK, F)", "en"),
    ("bm_george", "George (inglese UK, M)", "en"),
    ("jf_alpha", "Alpha (giapponese, F)", "ja"),
    ("jf_tebukuro", "Tebukuro (giapponese, F)", "ja"),
    ("jm_kumo", "Kumo (giapponese, M)", "ja"),
    ("ef_dora", "Dora (spagnolo, F)", "es"),
    ("ff_siwis", "Siwis (francese, F)", "fr"),
    ("pf_dora", "Dora (portoghese BR, F)", "pt"),
];

pub fn kokoro_voices() -> Vec<Voice> {
    KOKORO_VOICES.iter().map(|(id, name, lang)| Voice { id: (*id).into(), name: (*name).into(), lang: (*lang).into() }).collect()
}

/// Sintetizza `text` col motore scelto.
pub async fn synth(http: &reqwest::Client, a: &AudioConfig, text: &str, lang: &str, tts_key: Option<String>) -> Result<Pcm, String> {
    match a.tts.as_str() {
        "windows" => {
            let (text, voice, lang, rate) = (text.to_string(), a.tts_voice.clone(), lang.to_string(), a.tts_rate);
            let wav = tauri::async_runtime::spawn_blocking(move || windows_synth(&text, &voice, &lang, rate))
                .await
                .map_err(|e| e.to_string())??;
            decode_wav(&wav)
        }
        "kokoro" => {
            let voice = if lang == "en" { &a.kokoro_voice_en } else { &a.kokoro_voice };
            let voice = if voice.trim().is_empty() { "if_sara" } else { voice.trim() };
            speech(http, &a.kokoro_url, "kokoro", voice, a.kokoro_speed, text, None)
                .await
                .map_err(|e| format!("Kokoro: {e}"))
        }
        "openai" => {
            let voice = if a.tts_voice.is_empty() { "alloy" } else { a.tts_voice.as_str() };
            speech(http, &a.tts_url, &a.tts_model, voice, a.tts_rate, text, tts_key).await
        }
        _ => Err("voce disattivata".into()),
    }
}

/// POST `{base}/audio/speech` (formato OpenAI) chiedendo un WAV.
async fn speech(
    http: &reqwest::Client,
    base: &str,
    model: &str,
    voice: &str,
    speed: f64,
    text: &str,
    key: Option<String>,
) -> Result<Pcm, String> {
    let mut req = http
        .post(format!("{}/audio/speech", base.trim().trim_end_matches('/')))
        .timeout(Duration::from_secs(90))
        .json(&serde_json::json!({
            "model": model,
            "input": text,
            "voice": voice,
            "response_format": "wav",
            "speed": speed,
        }));
    if let Some(k) = &key {
        req = req.bearer_auth(k);
    }
    let resp = req
        .send()
        .await
        .map_err(|e| format!("servizio voce non raggiungibile: {}", crate::providers::describe_net_error(&e)))?;
    let status = resp.status();
    let bytes = resp.bytes().await.map_err(|e| e.to_string())?;
    if !status.is_success() {
        let detail: String = String::from_utf8_lossy(&bytes).chars().take(200).collect();
        return Err(crate::keys::redact(&format!("voce non disponibile (HTTP {}): {detail}", status.as_u16()), key.as_deref()));
    }
    decode_wav(&bytes)
}

// ---------- voci di Windows (WinRT) ----------

#[cfg(windows)]
fn winrt_init() {
    use windows::Win32::System::WinRT::{RoInitialize, RO_INIT_MULTITHREADED};
    // già inizializzato = va bene lo stesso
    let _ = unsafe { RoInitialize(RO_INIT_MULTITHREADED) };
}

#[cfg(windows)]
pub fn windows_voices() -> Result<Vec<Voice>, String> {
    use windows::Media::SpeechSynthesis::SpeechSynthesizer;
    winrt_init();
    let all = SpeechSynthesizer::AllVoices().map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for i in 0..all.Size().map_err(|e| e.to_string())? {
        let v = all.GetAt(i).map_err(|e| e.to_string())?;
        let name = v.DisplayName().map_err(|e| e.to_string())?.to_string();
        let lang = v.Language().map_err(|e| e.to_string())?.to_string();
        out.push(Voice { id: name.clone(), name, lang });
    }
    Ok(out)
}

#[cfg(not(windows))]
pub fn windows_voices() -> Result<Vec<Voice>, String> {
    Err("voci di Windows disponibili solo su Windows".into())
}

#[cfg_attr(not(windows), allow(dead_code))]
/// Sceglie la voce: quella indicata se è della lingua giusta, altrimenti la prima della lingua.
pub fn pick_voice<'a>(voices: &'a [Voice], wanted: &str, lang: &str) -> Option<&'a Voice> {
    let lang_ok = |v: &Voice| v.lang.to_lowercase().starts_with(lang);
    voices
        .iter()
        .find(|v| !wanted.is_empty() && (v.name == wanted || wanted.starts_with(&v.name)) && lang_ok(v))
        .or_else(|| voices.iter().find(|v| lang_ok(v)))
        .or_else(|| voices.iter().find(|v| !wanted.is_empty() && v.name == wanted))
}

#[cfg(windows)]
fn windows_synth(text: &str, voice: &str, lang: &str, rate: f64) -> Result<Vec<u8>, String> {
    use windows::{core::HSTRING, Media::SpeechSynthesis::SpeechSynthesizer, Storage::Streams::DataReader};
    let err = |e: windows::core::Error| format!("voce di Windows: {e}");
    winrt_init();
    let synth = SpeechSynthesizer::new().map_err(err)?;
    let voices = windows_voices()?;
    if let Some(v) = pick_voice(&voices, voice, lang) {
        let all = SpeechSynthesizer::AllVoices().map_err(err)?;
        for i in 0..all.Size().map_err(err)? {
            let info = all.GetAt(i).map_err(err)?;
            if info.DisplayName().map_err(err)?.to_string() == v.name {
                synth.SetVoice(&info).map_err(err)?;
                break;
            }
        }
    }
    if let Ok(opt) = synth.Options() {
        let _ = opt.SetSpeakingRate(rate.clamp(0.5, 6.0));
    }
    let stream = synth.SynthesizeTextToStreamAsync(&HSTRING::from(text)).map_err(err)?.join().map_err(err)?;
    let size = stream.Size().map_err(err)? as u32;
    let reader = DataReader::CreateDataReader(&stream.GetInputStreamAt(0).map_err(err)?).map_err(err)?;
    reader.LoadAsync(size).map_err(err)?.join().map_err(err)?;
    let mut buf = vec![0u8; size as usize];
    reader.ReadBytes(&mut buf).map_err(err)?;
    Ok(buf)
}

#[cfg(not(windows))]
fn windows_synth(_text: &str, _voice: &str, _lang: &str, _rate: f64) -> Result<Vec<u8>, String> {
    Err("voci di Windows disponibili solo su Windows".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wav_roundtrip_and_streaming_header() {
        let wav = crate::stt::to_wav(&[0.0, 0.5, -0.5, 0.25]).unwrap();
        let p = decode_wav(&wav).unwrap();
        assert_eq!(p.rate, 16_000);
        assert_eq!(p.samples.len(), 4);
        assert!((p.samples[1] - 0.5).abs() < 0.001);
        // lunghezza "sconosciuta" (streaming): si legge fino alla fine
        let mut w = wav.clone();
        w[40..44].copy_from_slice(&u32::MAX.to_le_bytes());
        assert_eq!(decode_wav(&w).unwrap().samples.len(), 4);
        assert!(decode_wav(b"ID3 mp3").is_err());
    }

    #[test]
    fn wav_stereo_float() {
        // float 32 stereo, 2 frame
        let mut b = Vec::new();
        b.extend_from_slice(b"RIFF\0\0\0\0WAVEfmt ");
        b.extend_from_slice(&16u32.to_le_bytes());
        b.extend_from_slice(&3u16.to_le_bytes());
        b.extend_from_slice(&2u16.to_le_bytes());
        b.extend_from_slice(&24_000u32.to_le_bytes());
        b.extend_from_slice(&(24_000u32 * 8).to_le_bytes());
        b.extend_from_slice(&8u16.to_le_bytes());
        b.extend_from_slice(&32u16.to_le_bytes());
        b.extend_from_slice(b"data");
        b.extend_from_slice(&16u32.to_le_bytes());
        for v in [1.0f32, 0.0, 0.5, 0.5] {
            b.extend_from_slice(&v.to_le_bytes());
        }
        let p = decode_wav(&b).unwrap();
        assert_eq!(p.rate, 24_000);
        assert_eq!(p.samples, vec![0.5, 0.5]);
    }

    #[test]
    fn sentence_chunks() {
        let t = "Ciao! Sono Alter Ego. Oggi ti aiuto volentieri con il tuo progetto in Rust. Va bene? Sì.";
        let c = chunks(t);
        assert!(c.len() >= 2, "{c:?}");
        assert_eq!(c.join(" "), t);
        assert!(c.iter().all(|x| x.chars().count() >= 12));
        assert_eq!(chunks("Ok."), vec!["Ok."]);
    }

    #[test]
    fn voice_choice() {
        let v = vec![
            Voice { id: "a".into(), name: "Microsoft Elsa".into(), lang: "it-IT".into() },
            Voice { id: "b".into(), name: "Microsoft Zira".into(), lang: "en-US".into() },
        ];
        assert_eq!(pick_voice(&v, "Microsoft Elsa", "it").unwrap().name, "Microsoft Elsa");
        assert_eq!(pick_voice(&v, "Microsoft Elsa", "en").unwrap().name, "Microsoft Zira");
        // nome della vecchia versione (Web Speech): "Microsoft Elsa - Italian (Italy)"
        assert_eq!(pick_voice(&v, "Microsoft Elsa - Italian (Italy)", "it").unwrap().name, "Microsoft Elsa");
    }

    /// Richiede tools/mock_openai.py in esecuzione.
    #[tokio::test]
    async fn speech_from_mock() {
        let mut a = AudioConfig { tts: "kokoro".into(), ..Default::default() };
        a.kokoro_url = "http://127.0.0.1:18080/ok/v1".into();
        let p = synth(&reqwest::Client::new(), &a, "Ciao", "it", None).await.unwrap();
        assert_eq!(p.rate, 24_000);
        assert!(!p.samples.is_empty());
        let last: serde_json::Value = serde_json::from_str(&std::fs::read_to_string("/tmp/mock_speech.json").unwrap()).unwrap();
        assert_eq!(last["voice"], "if_sara");
        assert_eq!(last["response_format"], "wav");
    }
}
