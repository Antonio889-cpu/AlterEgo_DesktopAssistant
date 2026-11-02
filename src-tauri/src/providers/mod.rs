//! Backend AI: un solo client HTTP, un adapter per ogni provider, streaming, fallback.
//!
//! Tutti e cinque i provider espongono un endpoint compatibile OpenAI
//! (`/chat/completions` con SSE e `/models`), quindi l'adapter è unico
//! e cambia solo URL base e autenticazione. Punto di innesto per codice esterno
//! (es. CyberAI): aggiungi qui un modulo e una voce in `PROVIDERS`.

mod openai_compat;
pub mod prompt;

use crate::{config::AiConfig, keys};
use serde::{Deserialize, Serialize};
use std::{
    sync::atomic::{AtomicU64, Ordering},
    time::Duration,
};
use tauri::ipc::Channel;

pub struct ProviderInfo {
    pub id: &'static str,
    pub label: &'static str,
    pub default_base: &'static str,
    pub needs_key: bool,
}

pub const PROVIDERS: &[ProviderInfo] = &[
    ProviderInfo { id: "gemini", label: "Google Gemini", default_base: "https://generativelanguage.googleapis.com/v1beta/openai", needs_key: true },
    ProviderInfo { id: "openrouter", label: "OpenRouter", default_base: "https://openrouter.ai/api/v1", needs_key: true },
    ProviderInfo { id: "groq", label: "Groq", default_base: "https://api.groq.com/openai/v1", needs_key: true },
    ProviderInfo { id: "deepseek", label: "DeepSeek", default_base: "https://api.deepseek.com/v1", needs_key: true },
    ProviderInfo { id: "ollama", label: "Ollama (locale)", default_base: "http://localhost:11434/v1", needs_key: false },
];

pub fn info(id: &str) -> Option<&'static ProviderInfo> {
    PROVIDERS.iter().find(|p| p.id == id)
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
    /// Immagine PNG in base64 (solo con modelli con visione).
    #[serde(default)]
    pub image: Option<String>,
}

/// Eventi inviati al frontend durante una richiesta.
#[derive(Serialize, Clone)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum StreamEvent {
    Start { provider: String, model: String },
    Delta { text: String },
    /// Il provider è caduto a metà: il testo ricevuto finora va scartato.
    Reset,
    Fallback { from: String, reason: String },
}

#[derive(Serialize, Debug)]
pub struct ChatResult {
    pub text: String,
    pub provider: String,
    pub model: String,
}

/// Errore di un tentativo. `retryable`: ha senso riprovare lo stesso provider.
#[derive(Debug)]
pub struct ProviderError {
    pub message: String,
    pub retryable: bool,
}

impl ProviderError {
    pub fn new(message: impl Into<String>, retryable: bool) -> Self {
        Self { message: message.into(), retryable }
    }
}

/// Parametri risolti per un provider (URL, chiave, modello).
pub struct Target {
    pub id: String,
    pub base: String,
    pub key: Option<String>,
    pub model: String,
    pub temperature: f64,
}

pub struct Ai {
    pub http: reqwest::Client,
    /// Ogni nuova richiesta incrementa il contatore: quelle vecchie si fermano.
    pub generation: AtomicU64,
}

impl Ai {
    pub fn new() -> Self {
        let http = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(10))
            .user_agent("AlterEgo/0.1")
            .build()
            .expect("client HTTP");
        Self { http, generation: AtomicU64::new(0) }
    }
}

fn resolve(cfg: &AiConfig, id: &str) -> Result<Target, String> {
    let meta = info(id).ok_or_else(|| format!("provider sconosciuto: {id}"))?;
    let pc = cfg.providers.get(id).cloned().unwrap_or_default();
    let key = if meta.needs_key {
        Some(keys::get(id).ok_or("chiave API mancante")?)
    } else {
        None
    };
    let base = if pc.base_url.trim().is_empty() { meta.default_base.to_string() } else { pc.base_url.trim().trim_end_matches('/').to_string() };
    Ok(Target { id: id.into(), base, key, model: pc.model.trim().to_string(), temperature: cfg.temperature })
}

/// Invia la conversazione al primo provider disponibile nell'ordine di fallback.
pub async fn chat(
    ai: &Ai,
    cfg: AiConfig,
    system: String,
    messages: Vec<ChatMessage>,
    events: Channel<StreamEvent>,
) -> Result<ChatResult, String> {
    let gen = ai.generation.fetch_add(1, Ordering::SeqCst) + 1;
    let enabled: Vec<String> = cfg
        .order
        .iter()
        .filter(|id| cfg.providers.get(id.as_str()).map(|p| p.enabled).unwrap_or(false))
        .cloned()
        .collect();
    if enabled.is_empty() {
        return Err("nessun provider attivo: aprili in Impostazioni".into());
    }
    let has_image = messages.iter().any(|m| m.image.is_some());
    let mut failures = Vec::new();

    for id in enabled {
        let pc = cfg.providers.get(&id).cloned().unwrap_or_default();
        if has_image && !pc.vision {
            failures.push(format!("{id}: visione non abilitata"));
            continue;
        }
        let mut target = match resolve(&cfg, &id) {
            Ok(t) => t,
            Err(e) => {
                let _ = events.send(StreamEvent::Fallback { from: id.clone(), reason: e.clone() });
                failures.push(format!("{id}: {e}"));
                continue;
            }
        };
        if target.model.is_empty() {
            // Ollama senza modello scelto: usa il primo installato.
            match openai_compat::list_models(&ai.http, &target).await {
                Ok(list) if !list.is_empty() => target.model = list[0].clone(),
                Ok(_) => {
                    failures.push(format!("{id}: nessun modello disponibile"));
                    continue;
                }
                Err(e) => {
                    failures.push(format!("{id}: {}", e.message));
                    continue;
                }
            }
        }

        let mut last_err = None;
        for attempt in 0..3u32 {
            if ai.generation.load(Ordering::SeqCst) != gen {
                return Err("richiesta annullata".into());
            }
            let _ = events.send(StreamEvent::Start { provider: id.clone(), model: target.model.clone() });
            match openai_compat::stream_chat(ai, gen, &target, &system, &messages, &events).await {
                Ok(text) => {
                    return Ok(ChatResult { text, provider: id, model: target.model });
                }
                Err(e) => {
                    let _ = events.send(StreamEvent::Reset);
                    let retry = e.retryable && attempt < 2;
                    last_err = Some(e);
                    if !retry {
                        break;
                    }
                    // backoff: 0.8 s, poi 2.4 s
                    tokio::time::sleep(Duration::from_millis(800 * 3u64.pow(attempt))).await;
                }
            }
        }
        let msg = last_err.map(|e| e.message).unwrap_or_default();
        let msg = keys::redact(&msg, target.key.as_deref());
        let _ = events.send(StreamEvent::Fallback { from: id.clone(), reason: msg.clone() });
        failures.push(format!("{id}: {msg}"));
    }
    Err(failures.join(" · "))
}

pub async fn list_models(ai: &Ai, cfg: &AiConfig, id: &str) -> Result<Vec<String>, String> {
    let target = resolve(cfg, id)?;
    openai_compat::list_models(&ai.http, &target)
        .await
        .map_err(|e| keys::redact(&e.message, target.key.as_deref()))
}

/// Prova rapida: una domanda minima senza streaming verso la UI.
pub async fn test(ai: &Ai, cfg: &AiConfig, id: &str) -> Result<String, String> {
    let mut one = cfg.clone();
    one.order = vec![id.to_string()];
    if let Some(p) = one.providers.get_mut(id) {
        p.enabled = true;
    }
    let ch = Channel::new(|_| Ok(()));
    let msg = ChatMessage { role: "user".into(), content: "Rispondi solo con la parola: ok".into(), image: None };
    chat(ai, one, "Sei un test di connessione.".into(), vec![msg], ch)
        .await
        .map(|r| format!("{} risponde ({}): {}", r.provider, r.model, r.text.trim()))
}

#[cfg(test)]
mod tests {
    //! Richiedono il mock server su 127.0.0.1:18080 (vedi README, sezione test).
    use super::*;
    use crate::config::{AiConfig, ProviderCfg};
    use std::sync::{Arc, Mutex};

    fn cfg(order: &[(&str, &str)]) -> AiConfig {
        let mut c = AiConfig::default();
        c.order = order.iter().map(|(id, _)| id.to_string()).collect();
        for (id, base) in order {
            c.providers.insert(id.to_string(), ProviderCfg { enabled: true, model: "m".into(), base_url: base.to_string(), vision: false });
        }
        c.normalize();
        for (id, p) in c.providers.iter_mut() {
            if !order.iter().any(|(o, _)| o == id) {
                p.enabled = false;
            }
        }
        c
    }

    fn channel() -> (Channel<StreamEvent>, Arc<Mutex<Vec<String>>>) {
        let log = Arc::new(Mutex::new(Vec::new()));
        let l = log.clone();
        let ch = Channel::new(move |body| {
            if let tauri::ipc::InvokeResponseBody::Json(s) = body {
                l.lock().unwrap().push(s);
            }
            Ok(())
        });
        (ch, log)
    }

    fn user(text: &str) -> Vec<ChatMessage> {
        vec![ChatMessage { role: "user".into(), content: text.into(), image: None }]
    }

    #[tokio::test]
    async fn streaming_ok() {
        let ai = Ai::new();
        let (ch, log) = channel();
        let r = chat(&ai, cfg(&[("ollama", "http://127.0.0.1:18080/ok/v1")]), "sys".into(), user("ciao"), ch).await.unwrap();
        assert_eq!(r.text, "[happy]\nCiao! Sono il mock (m).");
        assert!(log.lock().unwrap().iter().filter(|e| e.contains("delta")).count() >= 4);
    }

    #[tokio::test]
    async fn fallback_on_500_and_missing_key() {
        let ai = Ai::new();
        let (ch, log) = channel();
        // gemini senza chiave → salta; ollama 500 → (retry) → fallback; deepseek senza chiave → salta
        let c = cfg(&[("gemini", ""), ("ollama", "http://127.0.0.1:18080/fail500/v1")]);
        let err = chat(&ai, c, "s".into(), user("x"), ch).await.unwrap_err();
        assert!(err.contains("gemini: chiave API mancante"), "{err}");
        assert!(err.contains("ollama: errore del server (HTTP 500): boom"), "{err}");
        assert!(log.lock().unwrap().iter().any(|e| e.contains("fallback")));
    }

    #[tokio::test]
    async fn retry_after_429() {
        let ai = Ai::new();
        let (ch, _) = channel();
        let r = chat(&ai, cfg(&[("ollama", "http://127.0.0.1:18080/flaky/v1")]), "s".into(), user("x"), ch).await.unwrap();
        assert!(r.text.contains("mock"));
    }

    #[tokio::test]
    async fn connection_refused_is_reported() {
        let ai = Ai::new();
        let (ch, _) = channel();
        let err = chat(&ai, cfg(&[("ollama", "http://127.0.0.1:1/v1")]), "s".into(), user("x"), ch).await.unwrap_err();
        assert!(err.contains("connessione impossibile"), "{err}");
    }

    #[tokio::test]
    async fn models_sorted_and_auto_model() {
        let ai = Ai::new();
        let mut c = cfg(&[("ollama", "http://127.0.0.1:18080/ok/v1")]);
        assert_eq!(list_models(&ai, &c, "ollama").await.unwrap(), vec!["mock-a", "mock-b"]);
        c.providers.get_mut("ollama").unwrap().model.clear();
        let (ch, _) = channel();
        let r = chat(&ai, c, "s".into(), user("x"), ch).await.unwrap();
        assert_eq!(r.model, "mock-a");
    }

    #[test]
    fn redact_hides_key() {
        assert_eq!(keys::redact("invalid key Bearer sk-123456", Some("sk-123456")), "invalid key Bearer ••••");
    }
}
