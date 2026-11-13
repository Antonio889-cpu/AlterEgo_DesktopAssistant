//! VoiceStudio (debpalash, AGPL-3.0) come voce di Alter Ego: server locale compatibile OpenAI
//! (`POST /v1/audio/speech` su 127.0.0.1:3900, senza chiave in loopback), quindi la voce passa dal
//! TTS "openai" già esistente. Qui: installazione ufficiale (con conferma) e avvio insieme ad Alter Ego.
//! Fonte: github.com/debpalash/VoiceStudio (docs/agentic-voice.md, docs/api-auth.md, docs/install/windows.md).

use std::{path::PathBuf, process::Command};
use tauri::{AppHandle, Manager};

pub const URL: &str = "http://127.0.0.1:3900/v1";
const INSTALL_WIN: &str = "irm https://voicestudio.sh/install.ps1 | iex";
const INSTALL_UNIX: &str = "curl -fsSL https://voicestudio.sh/install | sh";

fn dirs() -> Vec<PathBuf> {
    let local = PathBuf::from(std::env::var_os("LOCALAPPDATA").unwrap_or_default());
    let pf = PathBuf::from(std::env::var_os("ProgramFiles").unwrap_or_default());
    vec![local.join("Programs"), pf.join("VoiceStudio")]
}

pub fn find() -> Option<PathBuf> {
    crate::hermes::which(&["voicestudio", "VoiceStudio"], &dirs())
}

pub fn running() -> bool {
    crate::alternet::url::reachable("http://127.0.0.1:3900")
}

/// Avvia VoiceStudio se non risponde già (i comandi della sua app avviano il backend in background).
pub fn start() -> Result<(), String> {
    if running() {
        return Ok(());
    }
    let exe = find().ok_or_else(|| crate::i18n::tr("VoiceStudio non è installato", "VoiceStudio is not installed").to_string())?;
    Command::new(exe).spawn().map(|_| ()).map_err(|e| crate::tr!("avvio di VoiceStudio non riuscito: {e}", "could not start VoiceStudio: {e}"))
}

/// Avvio insieme ad Alter Ego: solo se la voce in uso è VoiceStudio.
pub fn autostart(app: &AppHandle) {
    let a = app.state::<crate::AppState>().cfg.lock().unwrap().audio.clone();
    if a.tts == "openai" && a.tts_url.contains(":3900") && a.voicestudio_autostart && find().is_some() {
        std::thread::spawn(|| {
            if let Err(e) = start() {
                crate::applog::warn("voicestudio", &e);
            }
        });
    }
}

#[derive(serde::Serialize)]
pub struct Status {
    installed: bool,
    running: bool,
}

#[tauri::command]
pub async fn voicestudio_status() -> Status {
    tauri::async_runtime::spawn_blocking(|| Status { installed: find().is_some(), running: running() })
        .await
        .unwrap_or(Status { installed: false, running: false })
}

/// Installa (se manca, con conferma), avvia e imposta VoiceStudio come voce.
#[tauri::command]
pub async fn voicestudio_setup(app: AppHandle) -> Result<String, String> {
    if find().is_none() {
        let q = crate::i18n::tr(
            "Installare VoiceStudio?\nUso lo script ufficiale (voicestudio.sh): scarica l'installer dalla pagina delle release di GitHub e lo avvia. I modelli di voce (alcuni GB) li scarica VoiceStudio quando lo chiedi.",
            "Install VoiceStudio?\nI'll use the official script (voicestudio.sh): it downloads the installer from the GitHub releases and runs it. VoiceStudio downloads the voice models (a few GB) when you ask.",
        );
        if !crate::actions::confirm_always(&app, q.into()).await {
            return Err("l'utente ha negato il permesso".into());
        }
        let log = std::env::temp_dir().join("alterego-voicestudio-install.log");
        let mut cmd;
        if cfg!(windows) {
            cmd = Command::new("powershell.exe");
            cmd.args(["-NoProfile", "-ExecutionPolicy", "ByPass", "-Command", INSTALL_WIN]);
        } else {
            cmd = Command::new("sh");
            cmd.args(["-c", INSTALL_UNIX]);
        }
        crate::rizzo::step(&app, &log, crate::i18n::tr("Installo VoiceStudio", "Installing VoiceStudio"), cmd).await?;
        crate::actions::log(&app, "voicestudio", "installato", "ok");
    }
    tauri::async_runtime::spawn_blocking(start).await.map_err(|e| e.to_string())??;
    crate::update_config(&app, |c| {
        c.audio.tts = "openai".into();
        c.audio.tts_url = URL.into();
        if c.audio.tts_model.trim().is_empty() {
            c.audio.tts_model = "tts-1".into();
        }
        c.audio.voicestudio_autostart = true;
    });
    Ok(crate::i18n::tr(
        "VoiceStudio impostato come voce. Al primo uso scegli in VoiceStudio un motore e una voce (o clona quella che preferisci), poi scrivi qui il nome della voce.",
        "VoiceStudio set as the voice. On first use pick an engine and a voice in VoiceStudio (or clone the one you like), then type the voice name here.",
    )
    .into())
}
