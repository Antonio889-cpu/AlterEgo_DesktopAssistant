//! Comandi audio per il frontend: dispositivi, suoni, voce, Kokoro, Mic-Link.

use super::{kokoro::Kokoro, synth, tts, AudioEngine, Kind};
use crate::AppState;
use std::{sync::atomic::Ordering, time::Duration};
use tauri::{AppHandle, Emitter, Manager, State};

#[tauri::command]
pub fn audio_devices() -> super::Devices {
    super::devices()
}

#[tauri::command]
pub fn audio_status(engine: State<AudioEngine>) -> super::Status {
    engine.status()
}

/// Blip del dialogo ("blip", con `seed` = carattere) o jingle ("chime").
#[tauri::command]
pub fn audio_sfx(app: AppHandle, kind: String, seed: u32) {
    let a = app.state::<AppState>().cfg.lock().unwrap().audio.clone();
    let samples = match kind.as_str() {
        "chime" if a.blips != "off" => synth::chime(a.blip_volume as f32),
        "blip" if a.blip_volume > 0.0 => synth::blip(&a.blips, seed, a.blip_volume as f32),
        _ => return,
    };
    app.state::<AudioEngine>().play(&samples, synth::RATE, Kind::Fx);
}

#[tauri::command]
pub fn audio_stop(app: AppHandle) {
    app.state::<AudioEngine>().stop();
    let _ = app.emit("voice", false);
}

/// Legge `text` con la voce scelta. Le frasi si sintetizzano una alla volta mentre la
/// precedente suona. Eventi "voice": true quando inizia, false quando finisce.
#[tauri::command]
pub async fn tts_say(app: AppHandle, text: String, lang: Option<String>) -> Result<(), String> {
    let engine = app.state::<AudioEngine>();
    engine.stop();
    let gen = engine.voice_gen.load(Ordering::SeqCst);
    let a = app.state::<AppState>().cfg.lock().unwrap().audio.clone();
    if a.tts == "off" || text.trim().is_empty() {
        return Ok(());
    }
    let lang = lang.unwrap_or_else(|| "it".into());
    let url = if a.tts == "kokoro" { &a.kokoro_url } else { &a.tts_url };
    let http = app.state::<AppState>().ai.client_for(url).clone();
    if a.tts == "kokoro" {
        app.state::<Kokoro>()
            .ensure_ready(&http, &a.kokoro_url, a.kokoro_autostart)
            .await
            .map_err(|e| format!("Kokoro non pronto: {e}"))?;
    }
    let key = if a.tts == "openai" { crate::keys::get_slot("tts", "1") } else { None };
    let text: String = text.chars().take(2000).collect();
    let mut started = false;
    for (i, chunk) in tts::chunks(&text).iter().enumerate() {
        let pcm = match tts::synth(&http, &a, chunk, &lang, key.clone()).await {
            Ok(p) => p,
            Err(e) if i == 0 => return Err(e),
            Err(e) => {
                eprintln!("voce: frase saltata: {e}");
                continue;
            }
        };
        if engine.voice_gen.load(Ordering::SeqCst) != gen {
            return Ok(()); // interrotta da una nuova frase o da stop
        }
        let samples: Vec<f32> = pcm.samples.iter().map(|s| s * a.tts_volume as f32).collect();
        engine.play(&samples, pcm.rate, Kind::Voice);
        if !started {
            started = true;
            let _ = app.emit("voice", true);
        }
    }
    // fine della riproduzione
    while started && engine.voice_gen.load(Ordering::SeqCst) == gen && engine.voice_remaining() > 0.0 {
        tokio::time::sleep(Duration::from_millis(60)).await;
    }
    if started && engine.voice_gen.load(Ordering::SeqCst) == gen {
        let _ = app.emit("voice", false);
    }
    Ok(())
}

/// Voci disponibili per un motore ("windows", "kokoro").
#[tauri::command]
pub async fn tts_voices(engine: String) -> Result<Vec<tts::Voice>, String> {
    match engine.as_str() {
        "windows" => tauri::async_runtime::spawn_blocking(tts::windows_voices).await.map_err(|e| e.to_string())?,
        "kokoro" => Ok(tts::kokoro_voices()),
        _ => Ok(Vec::new()),
    }
}

#[tauri::command]
pub async fn kokoro_status(app: AppHandle) -> Result<super::kokoro::KokoroStatus, String> {
    let url = app.state::<AppState>().cfg.lock().unwrap().audio.kokoro_url.clone();
    let http = app.state::<AppState>().ai.client_for(&url).clone();
    Ok(app.state::<Kokoro>().status(&http, &url).await)
}

#[tauri::command]
pub fn kokoro_install(app: AppHandle, cuda: bool) -> Result<(), String> {
    app.state::<Kokoro>().install(cuda)
}

#[tauri::command]
pub fn kokoro_start(app: AppHandle) -> Result<(), String> {
    let url = app.state::<AppState>().cfg.lock().unwrap().audio.kokoro_url.clone();
    app.state::<Kokoro>().start(&url)
}

#[tauri::command]
pub fn kokoro_stop(app: AppHandle) {
    app.state::<Kokoro>().stop();
}

/// Rinomina il microfono del cavo virtuale in "AlterEgo Link Audio" (chiede l'amministratore).
#[tauri::command]
pub async fn mic_link_rename(app: AppHandle) -> Result<String, String> {
    #[cfg(windows)]
    {
        tauri::async_runtime::spawn_blocking(super::rename::request).await.map_err(|e| e.to_string())??;
        // i nomi dei dispositivi sono cambiati: riapre gli stream
        let a = app.state::<AppState>().cfg.lock().unwrap().audio.clone();
        let engine = app.state::<AudioEngine>();
        engine.configure(&crate::config::AudioConfig { mic_link: Default::default(), ..a.clone() });
        engine.configure(&a);
        Ok(format!("Fatto: nelle altre app scegli «{}» come microfono.", super::LINK_NAME))
    }
    #[cfg(not(windows))]
    {
        let _ = app;
        Err("disponibile solo su Windows".into())
    }
}

/// Apre la pagina ufficiale di VB-CABLE (driver gratuito di VB-Audio).
#[tauri::command]
pub fn open_vbcable_page() -> Result<(), String> {
    const URL: &str = "https://vb-audio.com/Cable/";
    #[cfg(windows)]
    let r = std::process::Command::new("rundll32").args(["url.dll,FileProtocolHandler", URL]).spawn();
    #[cfg(not(windows))]
    let r = std::process::Command::new("xdg-open").arg(URL).spawn();
    r.map(|_| ()).map_err(|e| e.to_string())
}

/// Dopo un cambio di impostazioni audio: dispositivi, Mic-Link, server Kokoro.
pub fn apply(app: &AppHandle) {
    let a = app.state::<AppState>().cfg.lock().unwrap().audio.clone();
    app.state::<AudioEngine>().configure(&a);
    let k = app.state::<Kokoro>();
    if a.tts == "kokoro" && a.kokoro_autostart && k.installed() {
        if let Err(e) = k.start(&a.kokoro_url) {
            eprintln!("Kokoro: {e}");
        }
    }
}
