//! Conversazione (memoria + prompt + provider), comandi memoria, push-to-talk.

use crate::{
    config, memory::{self, Memory}, providers, stt, update_config, AppState,
};
use std::sync::Mutex;
use tauri::{ipc::Channel, AppHandle, Emitter, Manager, State};

/// Stato della conversazione: memoria su disco oppure solo sessione.
pub struct Conversation {
    pub memory: Option<Memory>,
    pub memory_error: Option<String>,
    /// Usata quando la memoria persistente è spenta o non disponibile.
    pub session: Mutex<Vec<providers::ChatMessage>>,
    pub recorder: stt::Recorder,
    /// Client separato per i riassunti in background (non annulla la chat).
    pub ai_bg: providers::Ai,
    pub data_dir: std::path::PathBuf,
}

impl Conversation {
    pub fn new(data_dir: std::path::PathBuf) -> Self {
        let (memory, memory_error) = match Memory::open(&data_dir.join("memory.db")) {
            Ok(m) => (Some(m), None),
            Err(e) => (None, Some(e)),
        };
        Self { memory, memory_error, session: Mutex::new(Vec::new()), recorder: stt::Recorder::default(), ai_bg: providers::Ai::new(), data_dir }
    }

    fn mem(&self, enabled: bool) -> Option<&Memory> {
        if enabled { self.memory.as_ref() } else { None }
    }
}

fn msg(role: &str, content: &str) -> providers::ChatMessage {
    providers::ChatMessage { role: role.into(), content: content.into(), image: None }
}

#[tauri::command]
pub async fn chat(
    app: AppHandle,
    text: String,
    image: Option<String>,
    on_event: Channel<providers::StreamEvent>,
) -> Result<providers::ChatResult, String> {
    let state = app.state::<AppState>();
    let conv = app.state::<Conversation>();
    let cfg = state.cfg.lock().unwrap().clone();
    let text: String = text.trim().chars().take(4000).collect();
    if text.is_empty() {
        return Err("messaggio vuoto".into());
    }
    let mem = conv.mem(cfg.memory_enabled);

    // contesto: ultimi N messaggi + riassunto + fatti
    let (mut history, user_id, summary, facts) = match mem {
        Some(m) => {
            let id = m.add_message("user", &text)?;
            let recent = m.recent(cfg.ai.max_history)?.into_iter().map(|x| msg(&x.role, &x.content)).collect::<Vec<_>>();
            let summary = m.latest_summary()?.map(|s| s.1);
            let facts = m.facts()?.into_iter().map(|f| f.content).collect::<Vec<_>>();
            (recent, Some(id), summary, facts)
        }
        None => {
            let mut s = conv.session.lock().unwrap();
            s.push(msg("user", &text));
            let start = s.len().saturating_sub(cfg.ai.max_history);
            (s[start..].to_vec(), None, None, Vec::new())
        }
    };
    if let (Some(img), Some(last)) = (image, history.last_mut()) {
        last.image = Some(img);
    }

    let persona = providers::prompt::load_persona(&conv.data_dir);
    let system = providers::prompt::system(
        &cfg,
        &persona,
        &providers::prompt::MemoryContext { summary: summary.as_deref(), facts: &facts },
    );

    match providers::chat(&state.ai, cfg.ai.clone(), system, history, on_event).await {
        Ok(res) => {
            match mem {
                Some(m) => {
                    m.add_message("assistant", &res.text)?;
                    let app2 = app.clone();
                    let ai_cfg = cfg.ai.clone();
                    tauri::async_runtime::spawn(async move {
                        let conv = app2.state::<Conversation>();
                        if let Some(m) = conv.memory.as_ref() {
                            if let Err(e) = memory::summarize::maybe_run(&conv.ai_bg, ai_cfg, m).await {
                                eprintln!("riassunto non riuscito: {e}");
                            }
                        }
                    });
                }
                None => conv.session.lock().unwrap().push(msg("assistant", &res.text)),
            }
            Ok(res)
        }
        Err(e) => {
            // domanda senza risposta: non resta nel contesto
            match (mem, user_id) {
                (Some(m), Some(id)) => { let _ = m.delete_message(id); }
                _ => { conv.session.lock().unwrap().pop(); }
            }
            Err(e)
        }
    }
}

// ---------- impostazioni personalità / lingua / voce ----------

#[tauri::command]
pub fn set_persona_options(app: AppHandle, language: String, personality: config::Personality, memory_enabled: bool) {
    update_config(&app, |c| {
        c.language = language;
        c.personality = personality;
        c.memory_enabled = memory_enabled;
        c.normalize_voice();
    });
}

#[tauri::command]
pub fn set_stt(app: AppHandle, stt: config::SttConfig) {
    update_config(&app, |c| {
        c.stt = stt;
        c.normalize_voice();
    });
}

// ---------- memoria ----------

fn memory_or_err<'a>(conv: &'a Conversation) -> Result<&'a Memory, String> {
    conv.memory.as_ref().ok_or_else(|| conv.memory_error.clone().unwrap_or_else(|| "memoria non disponibile".into()))
}

#[tauri::command]
pub fn memory_overview(conv: State<Conversation>) -> Result<memory::Overview, String> {
    memory_or_err(&conv)?.overview()
}

#[tauri::command]
pub fn memory_add_fact(conv: State<Conversation>, content: String) -> Result<(), String> {
    memory_or_err(&conv)?.add_fact(&content)
}

#[tauri::command]
pub fn memory_delete_fact(conv: State<Conversation>, id: i64) -> Result<(), String> {
    memory_or_err(&conv)?.delete_fact(id)
}

#[tauri::command]
pub fn memory_forget_all(conv: State<Conversation>) -> Result<(), String> {
    conv.session.lock().unwrap().clear();
    memory_or_err(&conv)?.forget_all()
}

/// Esporta la memoria in JSON nella cartella Documenti. Restituisce il percorso.
#[tauri::command]
pub fn memory_export(app: AppHandle, conv: State<Conversation>) -> Result<String, String> {
    let data = memory_or_err(&conv)?.export()?;
    let dir = app.path().document_dir().unwrap_or_else(|_| conv.data_dir.clone());
    let path = dir.join(format!("AlterEgo-memoria-{}.json", memory::now()));
    std::fs::write(&path, serde_json::to_vec_pretty(&data).map_err(|e| e.to_string())?)
        .map_err(|e| format!("esportazione fallita: {e}"))?;
    Ok(path.display().to_string())
}

/// Apre la cartella dei dati (config, memoria, persona) in Esplora file.
#[tauri::command]
pub fn open_data_dir(conv: State<Conversation>) -> Result<(), String> {
    let dir = &conv.data_dir;
    let _ = std::fs::create_dir_all(dir);
    #[cfg(windows)]
    let r = std::process::Command::new("explorer").arg(dir).spawn();
    #[cfg(not(windows))]
    let r = std::process::Command::new("xdg-open").arg(dir).spawn();
    r.map(|_| ()).map_err(|e| e.to_string())
}

// ---------- push-to-talk ----------

fn emit_stt(app: &AppHandle, ev: stt::SttEvent) {
    let _ = app.emit("stt", ev);
}

/// Chiamata dalla hotkey: premuto = registra, rilasciato = trascrivi.
pub fn ptt(app: &AppHandle, pressed: bool) {
    let conv = app.state::<Conversation>();
    let provider = app.state::<AppState>().cfg.lock().unwrap().stt.provider.clone();
    if pressed {
        if conv.recorder.is_recording() {
            return;
        }
        if provider == "off" {
            emit_stt(app, stt::SttEvent::Error { message: "riconoscimento vocale disattivato nelle impostazioni".into() });
            return;
        }
        // l'apertura del microfono può richiedere un attimo: non bloccare il thread delle hotkey
        let app = app.clone();
        std::thread::spawn(move || {
            let conv = app.state::<Conversation>();
            match conv.recorder.start() {
                Ok(()) => emit_stt(&app, stt::SttEvent::Recording),
                Err(e) => emit_stt(&app, stt::SttEvent::Error { message: e }),
            }
        });
        return;
    }
    if !conv.recorder.is_recording() {
        return;
    }
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let conv = app.state::<Conversation>();
        let wav = match conv.recorder.finish() {
            Ok(Some(w)) => w,
            Ok(None) => return emit_stt(&app, stt::SttEvent::Empty),
            Err(e) => return emit_stt(&app, stt::SttEvent::Error { message: e }),
        };
        emit_stt(&app, stt::SttEvent::Transcribing);
        let cfg = app.state::<AppState>().cfg.lock().unwrap().stt.clone();
        let http = app.state::<AppState>().ai.http.clone();
        match stt::transcribe(&http, &cfg, wav).await {
            Ok(t) if t.is_empty() => emit_stt(&app, stt::SttEvent::Empty),
            Ok(text) => emit_stt(&app, stt::SttEvent::Done { text }),
            Err(message) => emit_stt(&app, stt::SttEvent::Error { message }),
        }
    });
}

/// Prova microfono dalle impostazioni: registra 4 secondi e trascrive.
#[tauri::command]
pub async fn stt_test(app: AppHandle) -> Result<String, String> {
    let conv = app.state::<Conversation>();
    conv.recorder.start()?;
    tokio::time::sleep(std::time::Duration::from_secs(4)).await;
    let wav = conv.recorder.finish()?.ok_or("nessun audio registrato")?;
    let cfg = app.state::<AppState>().cfg.lock().unwrap().stt.clone();
    let http = app.state::<AppState>().ai.http.clone();
    let text = stt::transcribe(&http, &cfg, wav).await?;
    Ok(if text.is_empty() { "(silenzio: nessuna parola riconosciuta)".into() } else { text })
}
