//! API nativa di Ollama (`/api/chat`, NDJSON). Si usa solo quando servono opzioni che l'endpoint
//! compatibile OpenAI ignora: finestra di contesto (`num_ctx`), `keep_alive`, livelli su GPU.

use super::{openai_compat::net_error, Ai, ChatMessage, ProviderError, StreamEvent, Target, ToolCall};
use futures_util::StreamExt;
use serde_json::{json, Value};
use std::{sync::atomic::Ordering, time::Duration};
use tauri::ipc::Channel;

/// "http://localhost:11434/v1" → "http://localhost:11434"
pub fn root(base: &str) -> String {
    let b = base.trim().trim_end_matches('/');
    b.strip_suffix("/v1").unwrap_or(b).to_string()
}

/// Messaggi nel formato di Ollama: immagini in `images`, argomenti degli strumenti come oggetti,
/// risposte degli strumenti con il nome dello strumento.
pub fn to_native(system: &str, messages: &[ChatMessage]) -> Vec<Value> {
    let mut out = vec![json!({ "role": "system", "content": system })];
    for (i, m) in messages.iter().enumerate() {
        let mut v = json!({ "role": m.role, "content": m.content });
        if let Some(img) = &m.image {
            v["images"] = json!([img]);
        }
        if let Some(calls) = &m.tool_calls {
            v["tool_calls"] = calls
                .iter()
                .map(|c| json!({ "function": { "name": c.name, "arguments": serde_json::from_str::<Value>(&c.arguments).unwrap_or(json!({})) } }))
                .collect();
        }
        if let Some(id) = &m.tool_call_id {
            let name = messages[..i]
                .iter()
                .rev()
                .filter_map(|p| p.tool_calls.as_ref())
                .flatten()
                .find(|c| &c.id == id)
                .map(|c| c.name.clone());
            if let Some(n) = name {
                v["tool_name"] = json!(n);
            }
        }
        out.push(v);
    }
    out
}

/// Contesto quando l'utente non lo sceglie: abbastanza per prompt, strumenti e risposta.
/// Solo due misure (16K/32K) perché cambiarlo costringe Ollama a ricaricare il modello.
fn auto_ctx(system: &str, messages: &[ChatMessage], tools: Option<&Value>) -> u32 {
    let chars = system.len() + messages.iter().map(|m| m.content.len()).sum::<usize>() + tools.map_or(0, |t| t.to_string().len());
    // ~3 caratteri per token, più 4K per la risposta
    if chars / 3 + 4096 <= 16384 { 16384 } else { 32768 }
}

pub fn body(t: &Target, system: &str, messages: &[ChatMessage], tools: Option<&Value>, think: bool) -> Value {
    let o = t.ollama.clone().unwrap_or_default();
    let mut options = json!({ "temperature": t.temperature });
    options["num_ctx"] = json!(if o.num_ctx > 0 { o.num_ctx } else { auto_ctx(system, messages, tools) });
    if o.num_gpu >= 0 {
        options["num_gpu"] = json!(o.num_gpu);
    }
    if t.max_tokens > 0 {
        options["num_predict"] = json!(t.max_tokens);
    }
    if t.top_p < 1.0 {
        options["top_p"] = json!(t.top_p);
    }
    if o.top_k > 0 {
        options["top_k"] = json!(o.top_k);
    }
    if o.repeat_penalty > 0.0 {
        options["repeat_penalty"] = json!(o.repeat_penalty);
    }
    let mut b = json!({ "model": t.model, "messages": to_native(system, messages), "stream": true, "options": options });
    if let Some(tools) = tools {
        b["tools"] = tools.clone();
    }
    if !o.keep_alive.is_empty() {
        // "-1" e numeri vanno come numero (secondi), "30m" come testo
        b["keep_alive"] = o.keep_alive.parse::<i64>().map(|n| json!(n)).unwrap_or(json!(o.keep_alive));
    }
    if think {
        b["think"] = json!(false);
    }
    b
}

pub async fn stream_chat(
    ai: &Ai,
    gen: u64,
    t: &Target,
    system: &str,
    messages: &[ChatMessage],
    tools: Option<&Value>,
    events: &Channel<StreamEvent>,
) -> Result<super::openai_compat::Reply, ProviderError> {
    let url = format!("{}/api/chat", root(&t.base));
    let think_key = format!("{}/{}", t.id, t.model);
    let mut think = t.thinking_off && !ai.no_think_params.lock().unwrap_or_else(std::sync::PoisonError::into_inner).contains(&think_key);
    // la prima volta Ollama carica il modello (con un contesto grande può volerci parecchio)
    let first_timeout = Duration::from_secs(t.timeout_secs.max(180));
    let idle_timeout = Duration::from_secs(t.timeout_secs.clamp(30, 180));
    let started = std::time::Instant::now();
    let resp = loop {
        let send = ai.client_for(&url).post(&url).json(&body(t, system, messages, tools, think)).send();
        let resp = tokio::time::timeout(first_timeout, send)
            .await
            .map_err(|_| ProviderError::new(crate::i18n::tr("Ollama non ha risposto in tempo (caricamento del modello?)", "Ollama did not answer in time (loading the model?)"), true))?
            .map_err(net_error)?;
        if resp.status().is_success() {
            break resp;
        }
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        let lower = text.to_lowercase();
        if think && lower.contains("think") {
            ai.no_think_params.lock().unwrap_or_else(std::sync::PoisonError::into_inner).insert(think_key.clone());
            think = false;
            continue;
        }
        let mut e = super::openai_compat::status_error(status, &text, tools.is_some());
        e.tools_unsupported = tools.is_some() && lower.contains("does not support tools");
        e.key_problem = false;
        if lower.contains("not found") && lower.contains("model") {
            e.message = crate::tr!("modello «{}» non installato in Ollama (ollama pull {})", "model “{}” not installed in Ollama (ollama pull {})", t.model, t.model);
        }
        return Err(e);
    };

    let mut stream = resp.bytes_stream();
    let mut buf = Vec::<u8>::new();
    let mut full = String::new();
    let mut calls = Vec::new();
    let mut usage = json!({});
    let mut first = true;
    'read: loop {
        if ai.generation.load(Ordering::SeqCst) != gen {
            return Err(ProviderError::new("richiesta annullata", false));
        }
        let chunk = match tokio::time::timeout(if first { first_timeout } else { idle_timeout }, stream.next()).await {
            Err(_) => return Err(ProviderError::new(crate::i18n::tr("lo streaming si è interrotto (timeout)", "the stream stopped (timeout)"), true)),
            Ok(None) => break,
            Ok(Some(Err(e))) => return Err(net_error(e)),
            Ok(Some(Ok(c))) => c,
        };
        first = false;
        buf.extend_from_slice(&chunk);
        while let Some(nl) = buf.iter().position(|b| *b == b'\n') {
            let line: Vec<u8> = buf.drain(..=nl).collect();
            let Ok(v) = serde_json::from_slice::<Value>(&line) else { continue };
            if let Some(err) = v.get("error").and_then(Value::as_str) {
                return Err(ProviderError::new(format!("Ollama: {err}"), false));
            }
            if let Some(text) = v.pointer("/message/content").and_then(Value::as_str) {
                if !text.is_empty() {
                    full.push_str(text);
                    let _ = events.send(StreamEvent::Delta { text: text.to_string() });
                }
            }
            if let Some(list) = v.pointer("/message/tool_calls").and_then(Value::as_array) {
                for c in list {
                    let name = c.pointer("/function/name").and_then(Value::as_str).unwrap_or("").to_string();
                    if name.is_empty() {
                        continue;
                    }
                    let args = c.pointer("/function/arguments").cloned().unwrap_or(json!({}));
                    let arguments = if let Some(s) = args.as_str() { s.to_string() } else { args.to_string() };
                    calls.push(ToolCall { id: format!("call_{}", calls.len()), name, arguments });
                }
            }
            if v.get("done").and_then(Value::as_bool) == Some(true) {
                usage = json!({ "prompt_tokens": v.get("prompt_eval_count"), "completion_tokens": v.get("eval_count") });
                break 'read;
            }
        }
    }
    ai.record_usage(&t.id, &t.model, &usage, started.elapsed().as_millis() as u64);
    if full.trim().is_empty() && calls.is_empty() {
        return Err(ProviderError::new(crate::i18n::tr("risposta vuota", "empty answer"), true));
    }
    Ok(super::openai_compat::Reply { text: full, tool_calls: calls })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_body() {
        let t = Target {
            ollama: Some(crate::config::OllamaOpts { num_ctx: 16384, keep_alive: "-1".into(), num_gpu: -1, top_k: 50, repeat_penalty: 1.1 }),
            max_tokens: 500,
            model: "qwen3".into(),
            ..Target::basic("ollama", "http://localhost:11434/v1".into(), None)
        };
        let msgs = vec![
            ChatMessage::new("user", "ciao"),
            ChatMessage { role: "assistant".into(), tool_calls: Some(vec![ToolCall { id: "c1".into(), name: "run_shell".into(), arguments: "{\"command\":\"ls\"}".into() }]), ..Default::default() },
            ChatMessage { role: "tool".into(), content: "ok".into(), tool_call_id: Some("c1".into()), ..Default::default() },
        ];
        let b = body(&t, "sys", &msgs, None, true);
        assert_eq!(b["options"]["num_ctx"], 16384);
        assert_eq!(b["options"]["num_predict"], 500);
        assert_eq!((b["options"]["top_k"].clone(), b["options"]["repeat_penalty"].clone()), (json!(50), json!(1.1)));
        assert!(b["options"].get("num_gpu").is_none());
        let auto = Target { model: "qwen3".into(), ..Target::basic("ollama", "http://localhost:11434/v1".into(), None) };
        assert_eq!(body(&auto, "sys", &msgs, None, false)["options"]["num_ctx"], 16384);
        assert_eq!(body(&auto, &"x".repeat(60_000), &msgs, None, false)["options"]["num_ctx"], 32768);
        assert_eq!(b["keep_alive"], -1);
        assert_eq!(b["think"], false);
        assert_eq!(b["messages"][2]["tool_calls"][0]["function"]["arguments"]["command"], "ls");
        assert_eq!(b["messages"][3]["tool_name"], "run_shell");
        assert_eq!(root("http://localhost:11434/v1/"), "http://localhost:11434");
    }
}
