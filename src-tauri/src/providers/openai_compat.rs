//! Adapter per endpoint compatibili OpenAI (Gemini, OpenRouter, Groq, DeepSeek, Ollama).

use super::{Ai, ChatMessage, ProviderError, StreamEvent, Target, ToolCall};
use futures_util::StreamExt;
use serde_json::{json, Value};
use std::{sync::atomic::Ordering, time::Duration};
use tauri::ipc::Channel;

pub struct Reply {
    pub text: String,
    pub tool_calls: Vec<ToolCall>,
}

fn request(http: &reqwest::Client, t: &Target, method: reqwest::Method, path: &str) -> reqwest::RequestBuilder {
    let mut req = http.request(method, format!("{}{}", t.base, path));
    if let Some(k) = &t.key {
        req = req.bearer_auth(k);
    }
    if t.id == "openrouter" {
        req = req.header("X-Title", "Alter Ego");
    }
    req
}

pub fn status_error(status: reqwest::StatusCode, body: &str, sent_tools: bool) -> ProviderError {
    let detail: String = serde_json::from_str::<Value>(body)
        .ok()
        .and_then(|v| {
            let v = if v.is_array() { v.get(0).cloned().unwrap_or(v) } else { v };
            v.pointer("/error/message")
                .or_else(|| v.get("error"))
                .or_else(|| v.get("message"))
                .map(|m| m.as_str().map(str::to_string).unwrap_or_else(|| m.to_string()))
        })
        .unwrap_or_else(|| body.chars().take(200).collect());
    let code = status.as_u16();
    let what = match code {
        401 | 403 => crate::i18n::tr("chiave non valida o senza permessi", "invalid key or no permission"),
        402 => crate::i18n::tr("credito esaurito", "credit exhausted"),
        404 => crate::i18n::tr("modello o endpoint non trovato", "model or endpoint not found"),
        429 => crate::i18n::tr("limite di richieste raggiunto", "rate limit reached"),
        500..=599 => crate::i18n::tr("errore del server", "server error"),
        _ => crate::i18n::tr("richiesta rifiutata", "request rejected"),
    };
    let lower = detail.to_lowercase();
    ProviderError {
        message: format!("{what} (HTTP {code}): {}", detail.trim()),
        retryable: code == 429 || status.is_server_error(),
        key_problem: matches!(code, 401 | 402 | 403 | 429) || lower.contains("quota") || lower.contains("api key"),
        tools_unsupported: sent_tools && matches!(code, 400 | 404 | 422 | 500) && lower.contains("tool"),
    }
}

pub fn net_error(e: reqwest::Error) -> ProviderError {
    let msg = if e.is_connect() {
        crate::i18n::tr("connessione impossibile", "cannot connect").to_string()
    } else if e.is_timeout() {
        "tempo scaduto".to_string()
    } else {
        crate::tr!("errore di rete: {}", "network error: {}", super::describe_net_error(&e))
    };
    ProviderError::new(msg, true)
}

fn to_wire(system: &str, messages: &[ChatMessage]) -> Vec<Value> {
    let mut out = vec![json!({ "role": "system", "content": system })];
    for m in messages {
        let content = match &m.image {
            Some(b64) => json!([
                { "type": "text", "text": m.content },
                { "type": "image_url", "image_url": { "url": format!("data:image/png;base64,{b64}") } }
            ]),
            None => json!(m.content),
        };
        let mut v = json!({ "role": m.role, "content": content });
        if let Some(calls) = &m.tool_calls {
            v["tool_calls"] = calls
                .iter()
                .map(|c| json!({ "id": c.id, "type": "function", "function": { "name": c.name, "arguments": c.arguments } }))
                .collect();
        }
        if let Some(id) = &m.tool_call_id {
            v["tool_call_id"] = json!(id);
        }
        out.push(v);
    }
    out
}

/// Parametri per chiedere una risposta senza ragionamento ("thinking"), per provider e modello.
/// None = il provider non ha un modo noto (il ragionamento viene comunque tolto dal testo).
pub fn thinking_off_params(id: &str, model: &str) -> Option<Value> {
    let m = model.to_lowercase();
    match id {
        // Gemini 2.5 Flash/Flash-Lite: "none" spegne il thinking (2.5 Pro non lo permette: si riprova senza)
        "gemini" if m.contains("gemma") => None,
        "gemini" if m.contains("gemini-3") => Some(json!({ "reasoning_effort": "low" })),
        "gemini" => Some(json!({ "reasoning_effort": "none" })),
        "openrouter" => Some(json!({ "reasoning": { "enabled": false, "exclude": true } })),
        "groq" if m.contains("qwen") => Some(json!({ "reasoning_effort": "none", "reasoning_format": "hidden" })),
        "groq" if m.contains("gpt-oss") => Some(json!({ "reasoning_effort": "low", "include_reasoning": false })),
        "groq" if m.contains("deepseek") || m.contains("r1") => Some(json!({ "reasoning_format": "hidden" })),
        "deepseek" => Some(json!({ "thinking": { "type": "disabled" } })),
        // Ollama ignora i campi sconosciuti; i modelli con thinking lo spengono con `think: false`
        "ollama" => Some(json!({ "think": false, "reasoning_effort": "none" })),
        _ => None,
    }
}

/// Accumula i frammenti di `tool_calls` che arrivano in streaming.
#[derive(Default)]
struct ToolAcc(Vec<ToolCall>);

impl ToolAcc {
    fn push(&mut self, deltas: &[Value]) {
        for (pos, d) in deltas.iter().enumerate() {
            let idx = d.get("index").and_then(Value::as_u64).map(|i| i as usize).unwrap_or(pos);
            while self.0.len() <= idx {
                self.0.push(ToolCall::default());
            }
            let c = &mut self.0[idx];
            if let Some(id) = d.get("id").and_then(Value::as_str) {
                c.id = id.to_string();
            }
            if let Some(n) = d.pointer("/function/name").and_then(Value::as_str) {
                c.name.push_str(n);
            }
            if let Some(a) = d.pointer("/function/arguments") {
                match a.as_str() {
                    Some(s) => c.arguments.push_str(s),
                    None => c.arguments.push_str(&a.to_string()),
                }
            }
        }
    }

    fn finish(self) -> Vec<ToolCall> {
        self.0
            .into_iter()
            .filter(|c| !c.name.is_empty())
            .enumerate()
            .map(|(i, mut c)| {
                if c.id.is_empty() {
                    c.id = format!("call_{i}");
                }
                if c.arguments.trim().is_empty() {
                    c.arguments = "{}".into();
                }
                c
            })
            .collect()
    }
}

/// Chiamata in streaming (SSE). Restituisce testo e/o richieste di strumenti.
pub async fn stream_chat(
    ai: &Ai,
    gen: u64,
    t: &Target,
    system: &str,
    messages: &[ChatMessage],
    tools: Option<&Value>,
    events: &Channel<StreamEvent>,
) -> Result<Reply, ProviderError> {
    let mut body = json!({
        "model": t.model,
        "messages": to_wire(system, messages),
        "temperature": t.temperature,
        "stream": true,
    });
    if let Some(tools) = tools {
        body["tools"] = tools.clone();
    }
    if t.max_tokens > 0 {
        body["max_tokens"] = json!(t.max_tokens);
    }
    if t.top_p < 1.0 {
        body["top_p"] = json!(t.top_p);
    }
    // ragionamento spento: parametri del provider (se li rifiuta si riprova senza e lo si ricorda)
    let think_key = format!("{}/{}", t.id, t.model);
    let mut think_params = if t.thinking_off && !ai.no_think_params.lock().unwrap().contains(&think_key) { thinking_off_params(&t.id, &t.model) } else { None };
    if let (Some(p), Some(o)) = (&think_params, body.as_object_mut()) {
        for (k, v) in p.as_object().into_iter().flatten() {
            o.insert(k.clone(), v.clone());
        }
    }
    // uso dei token in fondo allo stream (per il pannello Costi; DeepSeek riporta anche la cache).
    // Ollama è gratis; un provider che rifiuta il parametro viene ricordato e non lo riceve più.
    let want_usage = t.id != "ollama" && !ai.no_usage.lock().unwrap().contains(&t.id);
    if want_usage {
        body["stream_options"] = json!({ "include_usage": true });
        if t.id == "openrouter" {
            body["usage"] = json!({ "include": true }); // costo effettivo in dollari
        }
    }
    let started = std::time::Instant::now();
    // Ollama può impiegare molto a caricare il modello la prima volta.
    let first_timeout = Duration::from_secs(if t.id == "ollama" { t.timeout_secs.max(120) } else { t.timeout_secs });
    let idle_timeout = Duration::from_secs(t.timeout_secs.clamp(30, 120));

    let mut retried = false;
    let resp = loop {
        let send = request(ai.client_for(&t.base), t, reqwest::Method::POST, "/chat/completions").json(&body).send();
        let resp = tokio::time::timeout(first_timeout, send)
            .await
            .map_err(|_| ProviderError::new(crate::i18n::tr("tempo scaduto in attesa della risposta", "timed out waiting for the answer"), true))?
            .map_err(net_error)?;
        if resp.status().is_success() {
            break resp;
        }
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        let lower = text.to_lowercase();
        let think_rejected = think_params.is_some()
            && matches!(status.as_u16(), 400 | 404 | 422)
            && ["reason", "think", "unrecognized", "unknown", "not supported", "extra", "invalid"].iter().any(|w| lower.contains(w));
        if think_rejected {
            ai.no_think_params.lock().unwrap().insert(think_key.clone());
            if let (Some(o), Some(p)) = (body.as_object_mut(), think_params.take()) {
                for k in p.as_object().map(|m| m.keys().cloned().collect::<Vec<_>>()).unwrap_or_default() {
                    o.remove(&k);
                }
            }
            continue;
        }
        if want_usage && !retried && status.as_u16() == 400 && text.to_lowercase().contains("stream_options") {
            ai.no_usage.lock().unwrap().insert(t.id.clone());
            if let Some(o) = body.as_object_mut() {
                o.remove("stream_options");
                o.remove("usage");
            }
            retried = true;
            continue;
        }
        return Err(status_error(status, &text, tools.is_some()));
    };

    let mut stream = resp.bytes_stream();
    let mut buf = String::new();
    let mut full = String::new();
    let mut acc = ToolAcc::default();
    let mut first = true;
    let mut usage: Option<Value> = None;
    'read: loop {
        if ai.generation.load(Ordering::SeqCst) != gen {
            return Err(ProviderError::new("richiesta annullata", false));
        }
        let wait = if first { first_timeout } else { idle_timeout };
        let chunk = match tokio::time::timeout(wait, stream.next()).await {
            Err(_) => return Err(ProviderError::new(crate::i18n::tr("lo streaming si è interrotto (timeout)", "the stream stopped (timeout)"), true)),
            Ok(None) => break,
            Ok(Some(Err(e))) => return Err(net_error(e)),
            Ok(Some(Ok(c))) => c,
        };
        first = false;
        buf.push_str(&String::from_utf8_lossy(&chunk));
        while let Some(nl) = buf.find('\n') {
            let line = buf[..nl].trim().to_string();
            buf.drain(..=nl);
            let Some(data) = line.strip_prefix("data:") else { continue };
            let data = data.trim();
            if data == "[DONE]" {
                break 'read;
            }
            let Ok(v) = serde_json::from_str::<Value>(data) else { continue };
            if let Some(err) = v.get("error") {
                let msg = err.get("message").and_then(Value::as_str).unwrap_or(crate::i18n::tr("errore dal provider", "provider error")).to_string();
                let lower = msg.to_lowercase();
                return Err(ProviderError {
                    key_problem: lower.contains("quota") || lower.contains("rate limit"),
                    retryable: lower.contains("rate limit") || lower.contains("overloaded"),
                    message: msg,
                    ..Default::default()
                });
            }
            if let Some(u) = v.get("usage").filter(|u| u.is_object()) {
                usage = Some(u.clone());
            }
            if let Some(text) = v.pointer("/choices/0/delta/content").and_then(Value::as_str) {
                if !text.is_empty() {
                    full.push_str(text);
                    let _ = events.send(StreamEvent::Delta { text: text.to_string() });
                }
            }
            if let Some(calls) = v.pointer("/choices/0/delta/tool_calls").and_then(Value::as_array) {
                acc.push(calls);
            }
        }
    }
    let tool_calls = acc.finish();
    // anche senza `usage` la richiesta va nel registro (token sconosciuti = 0)
    ai.record_usage(&t.id, &t.model, usage.as_ref().unwrap_or(&json!({})), started.elapsed().as_millis() as u64);
    if full.trim().is_empty() && tool_calls.is_empty() {
        return Err(ProviderError::new(crate::i18n::tr("risposta vuota", "empty answer"), true));
    }
    Ok(Reply { text: full, tool_calls })
}

pub async fn list_models(http: &reqwest::Client, t: &Target) -> Result<Vec<String>, ProviderError> {
    let resp = tokio::time::timeout(
        Duration::from_secs(20),
        request(http, t, reqwest::Method::GET, "/models").send(),
    )
    .await
    .map_err(|_| ProviderError::new("tempo scaduto", true))?
    .map_err(net_error)?;
    let status = resp.status();
    let text = resp.text().await.map_err(net_error)?;
    if !status.is_success() {
        return Err(status_error(status, &text, false));
    }
    let v: Value = serde_json::from_str(&text).map_err(|_| ProviderError::new("risposta /models non valida", false))?;
    let mut ids: Vec<String> = v
        .get("data")
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(|m| m.get("id").and_then(Value::as_str)).map(|s| s.trim_start_matches("models/").to_string()).collect())
        .unwrap_or_default();
    ids.sort();
    Ok(ids)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_call_fragments() {
        let mut acc = ToolAcc::default();
        acc.push(&[json!({"index":0,"id":"c1","function":{"name":"set_","arguments":"{\"min"}})]);
        acc.push(&[json!({"index":0,"function":{"name":"reminder","arguments":"utes\":5}"}})]);
        let calls = acc.finish();
        assert_eq!(calls, vec![ToolCall { id: "c1".into(), name: "set_reminder".into(), arguments: "{\"minutes\":5}".into() }]);
    }

    #[test]
    fn errors_classified() {
        let e = status_error(reqwest::StatusCode::TOO_MANY_REQUESTS, r#"{"error":{"message":"slow"}}"#, false);
        assert!(e.key_problem && e.retryable);
        let e = status_error(reqwest::StatusCode::BAD_REQUEST, r#"{"error":{"message":"tools not supported"}}"#, true);
        assert!(e.tools_unsupported && !e.key_problem);
        let e = status_error(reqwest::StatusCode::UNAUTHORIZED, "{}", false);
        assert!(e.key_problem && !e.retryable);
    }
}
