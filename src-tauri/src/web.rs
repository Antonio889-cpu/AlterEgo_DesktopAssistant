//! Ricerca web con le API di DeepSeek (Search e Fetch, ottobre 2026), con le stesse chiavi
//! della chat DeepSeek (rotazione compresa):
//!   POST {base}/v1/tools/search  {"query": "...", "count": 1..50}  → risultati (titolo, url, estratto)
//!   POST {base}/v1/tools/fetch   {"url": "https://..."}            → pagina in Markdown
//! La lettura della risposta è tollerante sui nomi dei campi. Ogni chiamata va nel registro dei costi.

use crate::{costs, providers, AppState};
use serde::Serialize;
use serde_json::{json, Value};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Manager};

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Hit {
    pub title: String,
    pub url: String,
    pub snippet: String,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Page {
    pub title: String,
    pub content: String,
}

/// `https://api.deepseek.com/v1` (o senza /v1) → `https://api.deepseek.com`
pub fn api_root(base: &str) -> String {
    let b = base.trim().trim_end_matches('/');
    b.strip_suffix("/v1").unwrap_or(b).to_string()
}

fn text_of(v: &Value, keys: &[&str]) -> String {
    keys.iter().find_map(|k| v.get(*k).and_then(Value::as_str)).unwrap_or("").trim().to_string()
}

pub fn parse_search(v: &Value) -> Vec<Hit> {
    let list = v
        .get("results")
        .or_else(|| v.pointer("/data/results"))
        .or_else(|| v.get("data"))
        .or_else(|| v.get("items"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    list.iter()
        .map(|r| Hit {
            title: text_of(r, &["title", "name"]),
            url: text_of(r, &["url", "link"]),
            snippet: text_of(r, &["snippet", "summary", "description", "content", "text"]),
        })
        .filter(|h| !h.url.is_empty())
        .collect()
}

pub fn parse_fetch(v: &Value) -> Page {
    let d = v.get("data").filter(|d| d.is_object()).unwrap_or(v);
    Page {
        title: text_of(d, &["title"]),
        content: text_of(d, &["markdown", "content", "text", "body"]),
    }
}

/// Chiamata POST con rotazione delle chiavi DeepSeek. `what` = "search" o "fetch".
async fn call(app: &AppHandle, what: &str, body: Value) -> Result<Value, String> {
    let state = app.state::<AppState>();
    let (base, ids) = {
        let cfg = state.cfg.lock().unwrap();
        let pc = cfg.ai.providers.get("deepseek").cloned().unwrap_or_default();
        let base = if pc.base_url.trim().is_empty() { "https://api.deepseek.com".to_string() } else { pc.base_url.clone() };
        (api_root(&base), pc.key_ids())
    };
    let keys = state.ai.key_order("deepseek", &ids);
    if keys.is_empty() {
        return Err("la ricerca web usa le API di DeepSeek: aggiungi una chiave DeepSeek in Impostazioni → AI e chiavi".into());
    }
    let url = format!("{base}/v1/tools/{what}");
    let mut last = String::new();
    for (i, key) in keys {
        let t0 = Instant::now();
        let resp = state
            .ai
            .client_for(&url)
            .post(&url)
            .bearer_auth(&key)
            .timeout(Duration::from_secs(40))
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("DeepSeek {what}: {}", providers::describe_net_error(&e)))?;
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        if status.is_success() {
            state.ai.mark_good("deepseek", i);
            let v: Value = serde_json::from_str(&text).map_err(|_| format!("DeepSeek {what}: risposta non valida"))?;
            // si paga solo una chiamata riuscita
            state.ai.log_cost(costs::Entry {
                ts: costs::now(),
                kind: if what == "search" { "ricerca web" } else { "pagina web" }.into(),
                provider: "deepseek".into(),
                model: what.into(),
                calls: 1,
                ms: t0.elapsed().as_millis() as u64,
                ..Default::default()
            });
            return Ok(v);
        }
        let err = crate::providers::status_error_public(status, &text);
        last = crate::keys::redact(&format!("DeepSeek {what}: {}", err.message), Some(&key));
        if !err.key_problem {
            break;
        }
    }
    Err(last)
}

pub async fn search(app: &AppHandle, query: &str, count: u64) -> Result<Vec<Hit>, String> {
    let q: String = query.trim().chars().take(128).collect();
    if q.is_empty() {
        return Err("ricerca vuota".into());
    }
    let v = call(app, "search", json!({ "query": q, "count": count.clamp(1, 10) })).await?;
    Ok(parse_search(&v))
}

pub async fn fetch(app: &AppHandle, url: &str) -> Result<Page, String> {
    let u = reqwest::Url::parse(url.trim()).map_err(|_| "URL non valido".to_string())?;
    if !matches!(u.scheme(), "http" | "https") || !u.username().is_empty() || u.password().is_some() {
        return Err("si possono leggere solo pagine http/https senza credenziali".into());
    }
    let v = call(app, "fetch", json!({ "url": u.as_str() })).await?;
    let p = parse_fetch(&v);
    if p.content.is_empty() {
        return Err("la pagina non ha contenuto leggibile".into());
    }
    Ok(p)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_variants() {
        let v = json!({"results": [{"title": "A", "url": "https://a", "snippet": "x"}, {"title": "senza url"}]});
        assert_eq!(parse_search(&v), vec![Hit { title: "A".into(), url: "https://a".into(), snippet: "x".into() }]);
        let v = json!({"data": [{"name": "B", "link": "https://b", "summary": "y"}]});
        assert_eq!(parse_search(&v)[0].url, "https://b");
        assert_eq!(parse_fetch(&json!({"title": "T", "markdown": "# ciao"})).content, "# ciao");
        assert_eq!(parse_fetch(&json!({"data": {"title": "T", "content": "testo"}})).content, "testo");
        assert_eq!(api_root("https://api.deepseek.com/v1/"), "https://api.deepseek.com");
        assert_eq!(api_root("https://api.deepseek.com"), "https://api.deepseek.com");
    }
}
