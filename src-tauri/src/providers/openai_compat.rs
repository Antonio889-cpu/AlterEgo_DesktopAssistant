//! Adapter per endpoint compatibili OpenAI (Gemini, OpenRouter, Groq, DeepSeek, Ollama).

use super::{Ai, ChatMessage, ProviderError, StreamEvent, Target};
use futures_util::StreamExt;
use serde_json::{json, Value};
use std::{sync::atomic::Ordering, time::Duration};
use tauri::ipc::Channel;

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

fn status_error(status: reqwest::StatusCode, body: &str) -> ProviderError {
    let detail: String = serde_json::from_str::<Value>(body)
        .ok()
        .and_then(|v| {
            v.pointer("/error/message")
                .or_else(|| v.get("error"))
                .or_else(|| v.get("message"))
                .map(|m| m.as_str().map(str::to_string).unwrap_or_else(|| m.to_string()))
        })
        .unwrap_or_else(|| body.chars().take(200).collect());
    let what = match status.as_u16() {
        401 | 403 => "chiave non valida o senza permessi",
        404 => "modello o endpoint non trovato",
        429 => "limite di richieste raggiunto",
        500..=599 => "errore del server",
        _ => "richiesta rifiutata",
    };
    let retryable = status.as_u16() == 429 || status.is_server_error();
    ProviderError::new(format!("{what} (HTTP {}): {}", status.as_u16(), detail.trim()), retryable)
}

fn net_error(e: reqwest::Error) -> ProviderError {
    let msg = if e.is_connect() {
        "connessione impossibile".to_string()
    } else if e.is_timeout() {
        "tempo scaduto".to_string()
    } else {
        format!("errore di rete: {e}")
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
        out.push(json!({ "role": m.role, "content": content }));
    }
    out
}

/// Chiamata in streaming (SSE). Restituisce il testo completo.
pub async fn stream_chat(
    ai: &Ai,
    gen: u64,
    t: &Target,
    system: &str,
    messages: &[ChatMessage],
    events: &Channel<StreamEvent>,
) -> Result<String, ProviderError> {
    let body = json!({
        "model": t.model,
        "messages": to_wire(system, messages),
        "temperature": t.temperature,
        "stream": true,
    });
    // Ollama può impiegare molto a caricare il modello la prima volta.
    let first_timeout = Duration::from_secs(if t.id == "ollama" { 120 } else { 45 });
    let idle_timeout = Duration::from_secs(30);

    let send = request(&ai.http, t, reqwest::Method::POST, "/chat/completions").json(&body).send();
    let resp = tokio::time::timeout(first_timeout, send)
        .await
        .map_err(|_| ProviderError::new("tempo scaduto in attesa della risposta", true))?
        .map_err(net_error)?;
    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(status_error(status, &text));
    }

    let mut stream = resp.bytes_stream();
    let mut buf = String::new();
    let mut full = String::new();
    let mut first = true;
    loop {
        if ai.generation.load(Ordering::SeqCst) != gen {
            return Err(ProviderError::new("richiesta annullata", false));
        }
        let wait = if first { first_timeout } else { idle_timeout };
        let chunk = match tokio::time::timeout(wait, stream.next()).await {
            Err(_) => return Err(ProviderError::new("lo streaming si è interrotto (timeout)", true)),
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
                return finish(full);
            }
            let Ok(v) = serde_json::from_str::<Value>(data) else { continue };
            if let Some(err) = v.get("error") {
                let msg = err.get("message").and_then(Value::as_str).unwrap_or("errore dal provider");
                return Err(ProviderError::new(msg.to_string(), false));
            }
            if let Some(text) = v.pointer("/choices/0/delta/content").and_then(Value::as_str) {
                if !text.is_empty() {
                    full.push_str(text);
                    let _ = events.send(StreamEvent::Delta { text: text.to_string() });
                }
            }
        }
    }
    finish(full)
}

fn finish(full: String) -> Result<String, ProviderError> {
    if full.trim().is_empty() {
        Err(ProviderError::new("risposta vuota", true))
    } else {
        Ok(full)
    }
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
        return Err(status_error(status, &text));
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
