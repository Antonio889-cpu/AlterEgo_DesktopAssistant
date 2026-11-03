//! Backend AI: un solo client HTTP, un adapter per ogni provider, streaming, fallback.
//!
//! Tutti e cinque i provider espongono un endpoint compatibile OpenAI
//! (`/chat/completions` con SSE e `/models`), quindi l'adapter è unico
//! e cambia solo URL base e autenticazione. Punto di innesto per codice esterno
//! (es. CyberAI): aggiungi qui un modulo e una voce in `PROVIDERS`.
//!
//! Ordine dei tentativi: provider (ordine di fallback) → chiavi del provider
//! (rotazione su 401/402/403/429) → retry con backoff su errori temporanei.

mod openai_compat;
pub mod prompt;

use crate::{config::AiConfig, keys};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicU64, Ordering},
        Mutex,
    },
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

#[derive(Deserialize, Serialize, Clone, Debug, Default)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
    /// Immagine PNG in base64 (solo con modelli con visione).
    #[serde(default)]
    pub image: Option<String>,
    /// Messaggio "assistant" che chiede di usare strumenti.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
    /// Messaggio "tool": risposta a una chiamata.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
}

impl ChatMessage {
    pub fn new(role: &str, content: &str) -> Self {
        Self { role: role.into(), content: content.into(), ..Default::default() }
    }
}

#[derive(Deserialize, Serialize, Clone, Debug, Default, PartialEq)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    /// Argomenti JSON come stringa (formato OpenAI).
    pub arguments: String,
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
    #[serde(skip)]
    pub tool_calls: Vec<ToolCall>,
}

/// Errore di un tentativo.
#[derive(Debug, Default)]
pub struct ProviderError {
    pub message: String,
    /// Ha senso riprovare lo stesso provider (rete, 5xx, 429).
    pub retryable: bool,
    /// Problema legato alla chiave (401/402/403/429): prova la chiave successiva.
    pub key_problem: bool,
    /// Il modello non accetta il parametro `tools`: riprova senza.
    pub tools_unsupported: bool,
}

impl ProviderError {
    pub fn new(message: impl Into<String>, retryable: bool) -> Self {
        Self { message: message.into(), retryable, ..Default::default() }
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

/// Classifica una risposta HTTP non riuscita (stesse regole della chat).
pub fn status_error_public(status: reqwest::StatusCode, body: &str) -> ProviderError {
    openai_compat::status_error(status, body, false)
}

/// L'URL punta a questo PC (127.0.0.1, localhost, ::1)?
pub fn is_loopback(url: &str) -> bool {
    reqwest::Url::parse(url.trim())
        .ok()
        .and_then(|u| u.host_str().map(|h| matches!(h, "127.0.0.1" | "localhost" | "::1" | "[::1]")))
        .unwrap_or(false)
}

/// Descrive un errore di rete con tutte le cause (reqwest da solo dice solo "error sending request").
pub fn describe_net_error(e: &reqwest::Error) -> String {
    let mut parts = vec![e.to_string()];
    let mut src = std::error::Error::source(e);
    while let Some(s) = src {
        let t = s.to_string();
        if !parts.iter().any(|p| p.contains(&t)) {
            parts.push(t);
        }
        src = s.source();
    }
    let mut out = parts.join(": ");
    if e.is_timeout() {
        out.push_str(" (tempo scaduto)");
    }
    out
}

pub struct Ai {
    pub http: reqwest::Client,
    /// Client per i servizi su questo PC (Ollama, Kokoro, Whisper locale): ignora il proxy
    /// di sistema, che su Windows altrimenti verrebbe usato anche per 127.0.0.1.
    pub local: reqwest::Client,
    /// Ogni nuova richiesta incrementa il contatore: quelle vecchie si fermano.
    pub generation: AtomicU64,
    /// Ultima chiave funzionante per provider: si riparte da lì.
    sticky: Mutex<HashMap<String, usize>>,
    /// Token usati per provider da quando l'app è aperta (cache compresa).
    usage: Mutex<HashMap<String, Usage>>,
    /// Tipo di richieste per il registro dei costi ("chat", "riassunto").
    pub kind: &'static str,
    /// File del registro dei costi (impostato all'avvio).
    pub cost_log: Mutex<Option<std::path::PathBuf>>,
    /// Provider che hanno rifiutato `stream_options`: non lo si manda più.
    pub no_usage: Mutex<std::collections::HashSet<String>>,
}

/// Contatori dei token (dai campi `usage` delle risposte).
#[derive(Serialize, Clone, Default, Debug, PartialEq)]
pub struct Usage {
    pub requests: u64,
    pub input: u64,
    /// Token di input letti dalla cache del provider (più economici e veloci).
    pub cached: u64,
    pub output: u64,
}

impl Ai {
    pub fn new() -> Self {
        Self::with_kind("chat")
    }

    pub fn with_kind(kind: &'static str) -> Self {
        let http = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(10))
            .user_agent("AlterEgo/1.0")
            .build()
            .expect("client HTTP");
        let local = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(10))
            .user_agent("AlterEgo/1.0")
            .no_proxy()
            .build()
            .expect("client HTTP locale");
        Self {
            http,
            local,
            generation: AtomicU64::new(0),
            sticky: Mutex::new(HashMap::new()),
            usage: Mutex::new(HashMap::new()),
            kind,
            cost_log: Mutex::new(None),
            no_usage: Mutex::new(Default::default()),
        }
    }

    /// Client giusto per l'URL: senza proxy per i servizi locali.
    pub fn client_for(&self, url: &str) -> &reqwest::Client {
        if is_loopback(url) { &self.local } else { &self.http }
    }

    /// Registra il blocco `usage` di una risposta. DeepSeek usa `prompt_cache_hit_tokens` /
    /// `prompt_cache_miss_tokens`, OpenAI e compatibili `prompt_tokens_details.cached_tokens`.
    pub fn record_usage(&self, provider: &str, model: &str, u: &Value, ms: u64) {
        let n = |p: &str| u.pointer(p).and_then(Value::as_u64);
        let hit = n("/prompt_cache_hit_tokens").or_else(|| n("/prompt_tokens_details/cached_tokens")).unwrap_or(0);
        let input = n("/prompt_tokens")
            .or_else(|| n("/prompt_cache_miss_tokens").map(|m| m + hit))
            .unwrap_or(hit);
        let mut all = self.usage.lock().unwrap();
        let e = all.entry(provider.to_string()).or_default();
        e.requests += 1;
        e.input += input;
        e.cached += hit;
        e.output += n("/completion_tokens").unwrap_or(0);
        drop(all);
        self.log_cost(crate::costs::Entry {
            ts: crate::costs::now(),
            kind: self.kind.into(),
            provider: provider.into(),
            model: model.into(),
            input: input.saturating_sub(hit),
            cached: hit,
            output: n("/completion_tokens").unwrap_or(0),
            ms,
            // OpenRouter riporta il costo effettivo (in dollari)
            reported_cost: u.get("cost").and_then(Value::as_f64),
            ..Default::default()
        });
    }

    pub fn log_cost(&self, e: crate::costs::Entry) {
        if let Some(p) = self.cost_log.lock().unwrap().as_ref() {
            crate::costs::append(p, &e);
        }
    }

    pub fn usage(&self) -> HashMap<String, Usage> {
        self.usage.lock().unwrap().clone()
    }

    /// Chiavi del provider in ordine di prova (partendo dall'ultima che ha funzionato).
    pub fn key_order(&self, provider: &str, ids: &[String]) -> Vec<(usize, String)> {
        let start = self.sticky.lock().unwrap().get(provider).copied().unwrap_or(0);
        let n = ids.len();
        (0..n)
            .map(|i| (start + i) % n)
            .filter_map(|i| keys::get_slot(provider, &ids[i]).map(|k| (i, k)))
            .collect()
    }

    pub fn mark_good(&self, provider: &str, index: usize) {
        self.sticky.lock().unwrap().insert(provider.to_string(), index);
    }
}

fn base_url(cfg: &AiConfig, id: &str) -> Result<String, String> {
    let meta = info(id).ok_or_else(|| format!("provider sconosciuto: {id}"))?;
    let pc = cfg.providers.get(id).cloned().unwrap_or_default();
    Ok(if pc.base_url.trim().is_empty() { meta.default_base.to_string() } else { pc.base_url.trim().trim_end_matches('/').to_string() })
}

/// Chiavi utilizzabili per un provider: `[None]` per Ollama, altrimenti una per slot.
fn keys_for(ai: &Ai, cfg: &AiConfig, id: &str) -> Result<Vec<(usize, Option<String>)>, String> {
    let meta = info(id).ok_or_else(|| format!("provider sconosciuto: {id}"))?;
    if !meta.needs_key {
        return Ok(vec![(0, None)]);
    }
    let ids = cfg.providers.get(id).map(|p| p.key_ids()).unwrap_or_default();
    let list: Vec<_> = ai.key_order(id, &ids).into_iter().map(|(i, k)| (i, Some(k))).collect();
    if list.is_empty() {
        Err(crate::i18n::tr("chiave API mancante", "API key missing").into())
    } else {
        Ok(list)
    }
}

/// Invia la conversazione al primo provider disponibile nell'ordine di fallback.
/// `tools`: definizioni degli strumenti (formato OpenAI) o None.
pub async fn chat(
    ai: &Ai,
    cfg: AiConfig,
    system: String,
    messages: Vec<ChatMessage>,
    tools: Option<Value>,
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
        return Err(crate::i18n::tr("nessun provider attivo: aprili in Impostazioni", "no active provider: enable one in Settings").into());
    }
    let has_image = messages.iter().any(|m| m.image.is_some());
    let mut failures = Vec::new();

    'providers: for id in enabled {
        let pc = cfg.providers.get(&id).cloned().unwrap_or_default();
        if has_image && !pc.vision {
            failures.push(crate::tr!("{id}: visione non abilitata", "{id}: vision not enabled"));
            continue;
        }
        let fail = |failures: &mut Vec<String>, msg: String| {
            let _ = events.send(StreamEvent::Fallback { from: id.clone(), reason: msg.clone() });
            failures.push(format!("{id}: {msg}"));
        };
        let (base, key_list) = match base_url(&cfg, &id).and_then(|b| keys_for(ai, &cfg, &id).map(|k| (b, k))) {
            Ok(v) => v,
            Err(e) => {
                fail(&mut failures, e);
                continue;
            }
        };
        let n_keys = key_list.len();
        let mut model = pc.model.trim().to_string();
        let mut use_tools = tools.clone();
        let mut last_msg = String::new();

        for (k_pos, (k_index, key)) in key_list.into_iter().enumerate() {
            let mut target = Target { id: id.clone(), base: base.clone(), key, model: model.clone(), temperature: cfg.temperature };
            if target.model.is_empty() {
                // Ollama senza modello scelto: usa il primo installato.
                match openai_compat::list_models(ai.client_for(&target.base), &target).await {
                    Ok(list) if !list.is_empty() => {
                        model = list[0].clone();
                        target.model = model.clone();
                    }
                    Ok(_) => {
                        fail(&mut failures, crate::i18n::tr("nessun modello disponibile", "no model available").into());
                        continue 'providers;
                    }
                    Err(e) => {
                        fail(&mut failures, e.message);
                        continue 'providers;
                    }
                }
            }
            let more_keys = k_pos + 1 < n_keys;
            let mut attempt = 0u32;
            loop {
                if ai.generation.load(Ordering::SeqCst) != gen {
                    return Err("richiesta annullata".into());
                }
                let _ = events.send(StreamEvent::Start { provider: id.clone(), model: target.model.clone() });
                match openai_compat::stream_chat(ai, gen, &target, &system, &messages, use_tools.as_ref(), &events).await {
                    Ok(reply) => {
                        ai.mark_good(&id, k_index);
                        return Ok(ChatResult { text: reply.text, provider: id, model: target.model, tool_calls: reply.tool_calls });
                    }
                    Err(e) => {
                        let _ = events.send(StreamEvent::Reset);
                        last_msg = keys::redact(&e.message, target.key.as_deref());
                        if e.tools_unsupported && use_tools.is_some() {
                            use_tools = None; // il modello non supporta gli strumenti: riprova senza
                            continue;
                        }
                        if e.key_problem && more_keys {
                            let _ = events.send(StreamEvent::Fallback {
                                from: format!("{id} (chiave {})", k_pos + 1),
                                reason: crate::tr!("{last_msg} → provo la chiave successiva", "{last_msg} → trying the next key"),
                            });
                            break; // chiave successiva
                        }
                        if e.retryable && attempt < 2 {
                            // backoff: 0.8 s, poi 2.4 s
                            tokio::time::sleep(Duration::from_millis(800 * 3u64.pow(attempt))).await;
                            attempt += 1;
                            continue;
                        }
                        fail(&mut failures, last_msg.clone());
                        continue 'providers;
                    }
                }
            }
        }
        fail(&mut failures, last_msg);
    }
    Err(failures.join(" · "))
}

pub async fn list_models(ai: &Ai, cfg: &AiConfig, id: &str) -> Result<Vec<String>, String> {
    let base = base_url(cfg, id)?;
    let mut last = String::from("nessuna chiave");
    for (_, key) in keys_for(ai, cfg, id)? {
        let t = Target { id: id.into(), base: base.clone(), key, model: String::new(), temperature: 0.0 };
        match openai_compat::list_models(ai.client_for(&t.base), &t).await {
            Ok(v) => return Ok(v),
            Err(e) => last = keys::redact(&e.message, t.key.as_deref()),
        }
    }
    Err(last)
}

/// Prova rapida: una domanda minima senza streaming verso la UI.
pub async fn test(ai: &Ai, cfg: &AiConfig, id: &str) -> Result<String, String> {
    let mut one = cfg.clone();
    one.order = vec![id.to_string()];
    if let Some(p) = one.providers.get_mut(id) {
        p.enabled = true;
    }
    let ch = Channel::new(|_| Ok(()));
    let msg = ChatMessage::new("user", "Rispondi solo con la parola: ok");
    chat(ai, one, "Sei un test di connessione.".into(), vec![msg], None, ch)
        .await
        .map(|r| crate::tr!("{} risponde ({}): {}", "{} answers ({}): {}", r.provider, r.model, r.text.trim()))
}

#[cfg(test)]
mod usage_tests {
    use super::*;

    #[test]
    fn local_urls_skip_proxy() {
        assert!(is_loopback("http://127.0.0.1:8880/v1"));
        assert!(is_loopback("http://localhost:11434/v1"));
        assert!(!is_loopback("https://api.deepseek.com/v1"));
        let ai = Ai::new();
        assert!(std::ptr::eq(ai.client_for("http://127.0.0.1:8880/v1"), &ai.local));
        assert!(std::ptr::eq(ai.client_for("https://api.groq.com"), &ai.http));
    }

    #[tokio::test]
    async fn net_error_has_cause() {
        let e = reqwest::Client::new().get("http://127.0.0.1:1/health").send().await.unwrap_err();
        let d = describe_net_error(&e);
        assert!(d.len() > e.to_string().len(), "{d}");
    }

    #[test]
    fn usage_deepseek_and_openai() {
        let ai = Ai::new();
        let log = std::env::temp_dir().join(format!("ae-usage-{}.jsonl", std::process::id()));
        let _ = std::fs::remove_file(&log);
        *ai.cost_log.lock().unwrap() = Some(log.clone());
        ai.record_usage("deepseek", "deepseek-flash", &serde_json::json!({"prompt_tokens": 1000, "prompt_cache_hit_tokens": 900, "prompt_cache_miss_tokens": 100, "completion_tokens": 50}), 1200);
        ai.record_usage("openrouter", "x", &serde_json::json!({"prompt_tokens": 10, "prompt_tokens_details": {"cached_tokens": 4}, "completion_tokens": 2, "cost": 0.5}), 10);
        let rows = crate::costs::read(&log, 10);
        assert_eq!(rows[1].input, 100);
        assert_eq!(rows[1].cached, 900);
        assert_eq!(rows[1].ms, 1200);
        assert_eq!(rows[0].reported_cost, Some(0.5));
        let _ = std::fs::remove_file(&log);
        let u = ai.usage();
        assert_eq!(u["deepseek"], Usage { requests: 1, input: 1000, cached: 900, output: 50 });
        assert_eq!(u["openrouter"].cached, 4);
    }
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
            c.providers.insert(id.to_string(), ProviderCfg { enabled: true, model: "m".into(), base_url: base.to_string(), ..Default::default() });
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
        vec![ChatMessage::new("user", text)]
    }

    #[tokio::test]
    async fn streaming_ok() {
        let ai = Ai::new();
        let (ch, log) = channel();
        let r = chat(&ai, cfg(&[("ollama", "http://127.0.0.1:18080/ok/v1")]), "sys".into(), user("ciao"), None, ch).await.unwrap();
        assert_eq!(r.text, "[happy]\nCiao! Sono il mock (m).");
        assert!(log.lock().unwrap().iter().filter(|e| e.contains("delta")).count() >= 4);
    }

    #[tokio::test]
    async fn fallback_on_500_and_missing_key() {
        let ai = Ai::new();
        let (ch, log) = channel();
        // gemini senza chiave → salta; ollama 500 → (retry) → fallback; deepseek senza chiave → salta
        let c = cfg(&[("gemini", ""), ("ollama", "http://127.0.0.1:18080/fail500/v1")]);
        let err = chat(&ai, c, "s".into(), user("x"), None, ch).await.unwrap_err();
        assert!(err.contains("gemini: chiave API mancante"), "{err}");
        assert!(err.contains("ollama: errore del server (HTTP 500): boom"), "{err}");
        assert!(log.lock().unwrap().iter().any(|e| e.contains("fallback")));
    }

    #[tokio::test]
    async fn retry_after_429() {
        let ai = Ai::new();
        let (ch, _) = channel();
        let r = chat(&ai, cfg(&[("ollama", "http://127.0.0.1:18080/flaky/v1")]), "s".into(), user("x"), None, ch).await.unwrap();
        assert!(r.text.contains("mock"));
    }

    #[tokio::test]
    async fn connection_refused_is_reported() {
        let ai = Ai::new();
        let (ch, _) = channel();
        let err = chat(&ai, cfg(&[("ollama", "http://127.0.0.1:1/v1")]), "s".into(), user("x"), None, ch).await.unwrap_err();
        assert!(err.contains("connessione impossibile"), "{err}");
    }

    #[tokio::test]
    async fn models_sorted_and_auto_model() {
        let ai = Ai::new();
        let mut c = cfg(&[("ollama", "http://127.0.0.1:18080/ok/v1")]);
        assert_eq!(list_models(&ai, &c, "ollama").await.unwrap(), vec!["mock-a", "mock-b"]);
        c.providers.get_mut("ollama").unwrap().model.clear();
        let (ch, _) = channel();
        let r = chat(&ai, c, "s".into(), user("x"), None, ch).await.unwrap();
        assert_eq!(r.model, "mock-a");
    }

    #[tokio::test]
    async fn rotates_api_keys() {
        use crate::config::KeySlot;
        crate::keys::TEST_KEYS.with(|m| {
            let mut m = m.borrow_mut();
            m.insert("api-key:gemini:a".into(), "bad-quota".into());
            m.insert("api-key:gemini:b".into(), "bad".into());
            m.insert("api-key:gemini:c".into(), "good".into());
        });
        let ai = Ai::new();
        let mut c = cfg(&[("gemini", "http://127.0.0.1:18080/keycheck/v1")]);
        c.providers.get_mut("gemini").unwrap().keys = ["a", "b", "c"].map(|id| KeySlot { id: id.into(), label: id.into() }).to_vec();
        let (ch, log) = channel();
        let r = chat(&ai, c.clone(), "s".into(), user("x"), None, ch).await.unwrap();
        assert!(r.text.contains("mock"));
        assert_eq!(log.lock().unwrap().iter().filter(|e| e.contains("chiave successiva")).count(), 2);
        // la volta dopo parte direttamente dalla chiave buona
        let (ch, log) = channel();
        chat(&ai, c, "s".into(), user("x"), None, ch).await.unwrap();
        assert_eq!(log.lock().unwrap().iter().filter(|e| e.contains("chiave successiva")).count(), 0);
    }

    #[test]
    fn redact_hides_key() {
        assert_eq!(keys::redact("invalid key Bearer sk-123456", Some("sk-123456")), "invalid key Bearer ••••");
    }
}
