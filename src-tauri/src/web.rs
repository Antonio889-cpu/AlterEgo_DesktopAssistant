//! Ricerca web con il provider scelto in Impostazioni → Permessi → Ricerca web, con fallback:
//! - DeepSeek Search/Fetch (stesse chiavi della chat);
//! - Google tramite Gemini ("Grounding with Google Search", chiavi Gemini);
//! - Tavily e Brave Search (chiave propria nel Credential Manager);
//! - DuckDuckGo (pagina HTML, gratis e senza chiave).
//! La lettura delle pagine può usare DeepSeek Fetch oppure un lettore locale (gratis).
//!
//! DeepSeek (ottobre 2026), con le stesse chiavi della chat DeepSeek (rotazione compresa):
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
        return Err(crate::i18n::tr("la ricerca web usa le API di DeepSeek: aggiungi una chiave DeepSeek in Impostazioni → AI e chiavi", "web search uses the DeepSeek API: add a DeepSeek key in Settings → AI & keys").into());
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

/// Provider di ricerca supportati, nell'ordine usato per il fallback.
pub const SEARCH_PROVIDERS: &[&str] = &["deepseek", "gemini", "tavily", "brave", "duckduckgo"];

fn web_cfg(app: &AppHandle) -> crate::config::WebConfig {
    app.state::<AppState>().cfg.lock().unwrap().web.clone()
}

/// Il provider ha ciò che serve (chiave) per essere provato.
fn available(app: &AppHandle, p: &str) -> bool {
    let st = app.state::<AppState>();
    let has = |id: &str| st.cfg.lock().unwrap().ai.providers.get(id).is_some_and(|pc| !pc.keys.is_empty());
    match p {
        "deepseek" => has("deepseek"),
        "gemini" => has("gemini"),
        "tavily" | "brave" => crate::keys::get_slot(p, "1").is_some(),
        "duckduckgo" => true,
        _ => false,
    }
}

pub async fn search(app: &AppHandle, query: &str, count: u64) -> Result<Vec<Hit>, String> {
    let q: String = query.trim().chars().take(128).collect();
    if q.is_empty() {
        return Err(crate::i18n::tr("ricerca vuota", "empty search").into());
    }
    let count = count.clamp(1, 10);
    let wc = web_cfg(app);
    let mut order = vec![wc.provider.clone()];
    if wc.fallback {
        order.extend(SEARCH_PROVIDERS.iter().map(|p| p.to_string()).filter(|p| *p != wc.provider && available(app, p)));
    }
    let mut errors = Vec::new();
    for p in order {
        let r = match p.as_str() {
            "deepseek" => call(app, "search", json!({ "query": q, "count": count })).await.map(|v| parse_search(&v)),
            "gemini" => gemini_search(app, &q, &wc.gemini_model).await,
            "tavily" => tavily_search(app, &q, count).await,
            "brave" => brave_search(app, &q, count).await,
            "duckduckgo" => ddg_search(app, &q, count).await,
            other => Err(format!("provider di ricerca sconosciuto: {other}")),
        };
        match r {
            Ok(h) if !h.is_empty() => return Ok(h),
            Ok(_) => errors.push(format!("{p}: nessun risultato")),
            Err(e) => errors.push(e),
        }
    }
    if errors.iter().all(|e| e.ends_with("nessun risultato")) {
        return Ok(Vec::new());
    }
    Err(errors.join(" · "))
}

fn log_call(app: &AppHandle, provider: &str, model: &str, t0: Instant, input: u64, output: u64) {
    app.state::<AppState>().ai.log_cost(costs::Entry {
        ts: costs::now(),
        kind: "ricerca web".into(),
        provider: provider.into(),
        model: model.into(),
        calls: 1,
        input,
        output,
        ms: t0.elapsed().as_millis() as u64,
        ..Default::default()
    });
}

/// Google Search tramite Gemini: risposta già riassunta + fonti.
async fn gemini_search(app: &AppHandle, q: &str, model: &str) -> Result<Vec<Hit>, String> {
    let st = app.state::<AppState>();
    let ids = st.cfg.lock().unwrap().ai.providers.get("gemini").map(|p| p.key_ids()).unwrap_or_default();
    let keys = st.ai.key_order("gemini", &ids);
    if keys.is_empty() {
        return Err(crate::i18n::tr("Gemini: manca la chiave (AI e chiavi)", "Gemini: key missing (AI & keys)").into());
    }
    let model = if model.trim().is_empty() { "gemini-2.5-flash" } else { model.trim() };
    let url = format!("https://generativelanguage.googleapis.com/v1beta/models/{model}:generateContent");
    let body = json!({
        "contents": [{ "role": "user", "parts": [{ "text": format!("Search the web and report the relevant, up-to-date facts (with numbers and dates) for: {q}") }] }],
        "tools": [{ "google_search": {} }],
        "generationConfig": { "temperature": 0.2 }
    });
    let mut last = String::new();
    for (i, key) in keys {
        let t0 = Instant::now();
        let resp = st.ai.http.post(&url).header("x-goog-api-key", &key).timeout(Duration::from_secs(40)).json(&body).send().await
            .map_err(|e| format!("Gemini search: {}", providers::describe_net_error(&e)))?;
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        if status.is_success() {
            st.ai.mark_good("gemini", i);
            let v: Value = serde_json::from_str(&text).map_err(|_| "Gemini search: risposta non valida".to_string())?;
            let n = |p: &str| v.pointer(p).and_then(Value::as_u64).unwrap_or(0);
            log_call(app, "gemini", &format!("{model} search"), t0, n("/usageMetadata/promptTokenCount"), n("/usageMetadata/candidatesTokenCount"));
            return Ok(parse_gemini(&v, q));
        }
        let err = providers::status_error_public(status, &text);
        last = crate::keys::redact(&format!("Gemini search: {}", err.message), Some(&key));
        if !err.key_problem {
            break;
        }
    }
    Err(last)
}

pub fn parse_gemini(v: &Value, q: &str) -> Vec<Hit> {
    let answer: String = v
        .pointer("/candidates/0/content/parts")
        .and_then(Value::as_array)
        .map(|parts| parts.iter().filter_map(|p| p.get("text").and_then(Value::as_str)).collect::<Vec<_>>().join(""))
        .unwrap_or_default();
    let mut hits = Vec::new();
    let chunks = v.pointer("/candidates/0/groundingMetadata/groundingChunks").and_then(Value::as_array).cloned().unwrap_or_default();
    if !answer.trim().is_empty() {
        let first = chunks.first().and_then(|c| c.pointer("/web/uri")).and_then(Value::as_str).map(str::to_string);
        hits.push(Hit {
            title: "Google (Gemini)".into(),
            url: first.unwrap_or_else(|| format!("https://www.google.com/search?q={}", enc(q))),
            snippet: answer.trim().to_string(),
        });
    }
    for c in chunks {
        let (Some(u), t) = (c.pointer("/web/uri").and_then(Value::as_str), c.pointer("/web/title").and_then(Value::as_str)) else { continue };
        hits.push(Hit { title: t.unwrap_or("").to_string(), url: u.to_string(), snippet: String::new() });
    }
    hits
}

async fn tavily_search(app: &AppHandle, q: &str, count: u64) -> Result<Vec<Hit>, String> {
    let key = crate::keys::get_slot("tavily", "1").ok_or("Tavily: manca la chiave")?;
    let t0 = Instant::now();
    let resp = app.state::<AppState>().ai.http.post("https://api.tavily.com/search").bearer_auth(&key).timeout(Duration::from_secs(30))
        .json(&json!({ "query": q, "max_results": count, "search_depth": "basic" })).send().await
        .map_err(|e| format!("Tavily: {}", providers::describe_net_error(&e)))?;
    let status = resp.status();
    let text = resp.text().await.unwrap_or_default();
    if !status.is_success() {
        return Err(crate::keys::redact(&format!("Tavily: {}", providers::status_error_public(status, &text).message), Some(&key)));
    }
    log_call(app, "tavily", "search", t0, 0, 0);
    let v: Value = serde_json::from_str(&text).map_err(|_| "Tavily: risposta non valida".to_string())?;
    Ok(parse_search(&v))
}

async fn brave_search(app: &AppHandle, q: &str, count: u64) -> Result<Vec<Hit>, String> {
    let key = crate::keys::get_slot("brave", "1").ok_or("Brave: manca la chiave")?;
    let t0 = Instant::now();
    let url = format!("https://api.search.brave.com/res/v1/web/search?q={}&count={count}", enc(q));
    let resp = app.state::<AppState>().ai.http.get(&url).header("X-Subscription-Token", &key).header("Accept", "application/json")
        .timeout(Duration::from_secs(30)).send().await
        .map_err(|e| format!("Brave: {}", providers::describe_net_error(&e)))?;
    let status = resp.status();
    let text = resp.text().await.unwrap_or_default();
    if !status.is_success() {
        return Err(crate::keys::redact(&format!("Brave: {}", providers::status_error_public(status, &text).message), Some(&key)));
    }
    log_call(app, "brave", "search", t0, 0, 0);
    let v: Value = serde_json::from_str(&text).map_err(|_| "Brave: risposta non valida".to_string())?;
    Ok(parse_search(v.get("web").unwrap_or(&v)))
}

/// DuckDuckGo, versione HTML (gratis, nessuna chiave).
async fn ddg_search(app: &AppHandle, q: &str, count: u64) -> Result<Vec<Hit>, String> {
    let t0 = Instant::now();
    let resp = app.state::<AppState>().ai.http
        .post("https://html.duckduckgo.com/html/")
        .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AlterEgo")
        .form(&[("q", q), ("kl", "it-it")])
        .timeout(Duration::from_secs(20))
        .send()
        .await
        .map_err(|e| format!("DuckDuckGo: {}", providers::describe_net_error(&e)))?;
    let html = resp.text().await.unwrap_or_default();
    log_call(app, "duckduckgo", "search", t0, 0, 0);
    let mut hits = parse_ddg(&html);
    hits.truncate(count as usize);
    Ok(hits)
}

pub fn parse_ddg(html: &str) -> Vec<Hit> {
    let mut hits = Vec::new();
    let mut pos = 0;
    while let Some(i) = html[pos..].find("class=\"result__a\"").map(|i| i + pos) {
        let tag_start = html[..i].rfind("<a").unwrap_or(i);
        let tag_end = html[i..].find('>').map(|e| i + e).unwrap_or(html.len());
        let href = html[tag_start..tag_end].split("href=\"").nth(1).and_then(|h| h.split('"').next()).unwrap_or("");
        let close = html[tag_end..].find("</a>").map(|e| tag_end + e).unwrap_or(html.len());
        let title = html_to_text(&html[(tag_end + 1).min(close)..close]);
        // link di reindirizzamento: //duckduckgo.com/l/?uddg=<url codificato>&...
        let url = match href.split("uddg=").nth(1) {
            Some(u) => dec(u.split('&').next().unwrap_or("")),
            None => href.replace("&amp;", "&"),
        };
        let next = html[close..].find("class=\"result__a\"").map(|n| close + n).unwrap_or(html.len());
        let snippet = html[close..next]
            .find("result__snippet")
            .and_then(|s| {
                let a = close + s;
                let st = html[a..].find('>')? + a + 1;
                // fine dell'elemento dell'estratto (dentro può avere <b>…</b>)
                let en = ["</a>", "</td>", "</div>", "</span>"].iter().filter_map(|t| html[st..].find(t)).min().map(|e| st + e)?;
                Some(html_to_text(&html[st..en]))
            })
            .unwrap_or_default();
        if url.starts_with("http") && !url.contains("duckduckgo.com/y.js") {
            hits.push(Hit { title, url, snippet });
        }
        pos = close;
    }
    hits
}

fn enc(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => (b as char).to_string(),
            b' ' => "+".into(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

fn dec(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'%' if i + 2 < b.len() => {
                match u8::from_str_radix(std::str::from_utf8(&b[i + 1..i + 3]).unwrap_or("zz"), 16) {
                    Ok(v) => {
                        out.push(v);
                        i += 3;
                        continue;
                    }
                    Err(_) => out.push(b'%'),
                }
            }
            b'+' => out.push(b' '),
            c => out.push(c),
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// HTML → testo semplice (titoli e paragrafi a capo, niente script/stili, entità comuni).
pub fn html_to_text(html: &str) -> String {
    let mut s = html.to_string();
    for tag in ["script", "style", "noscript", "svg", "head", "nav", "footer", "form"] {
        loop {
            let lower = s.to_lowercase();
            let Some(a) = lower.find(&format!("<{tag}")) else { break };
            let close = format!("</{tag}>");
            let b = lower[a..].find(&close).map(|e| a + e + close.len()).unwrap_or(s.len());
            s.replace_range(a..b, " ");
        }
    }
    let mut out = String::with_capacity(s.len() / 2);
    let mut in_tag = false;
    let mut tag = String::new();
    for ch in s.chars() {
        match ch {
            '<' => {
                in_tag = true;
                tag.clear();
            }
            '>' if in_tag => {
                in_tag = false;
                let t = tag.trim_start_matches('/').split_whitespace().next().unwrap_or("").to_lowercase();
                if ["p", "br", "div", "li", "tr", "h1", "h2", "h3", "h4", "h5", "h6", "section", "article"].contains(&t.as_str()) {
                    out.push('\n');
                }
            }
            c if in_tag => tag.push(c),
            c => out.push(c),
        }
    }
    let out = out
        .replace("&nbsp;", " ")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&#x27;", "'")
        .replace("&apos;", "'");
    let lines: Vec<String> = out.lines().map(|l| l.split_whitespace().collect::<Vec<_>>().join(" ")).filter(|l| !l.is_empty()).collect();
    lines.join("\n")
}

/// Legge la pagina direttamente (gratis): HTML → testo.
async fn local_fetch(app: &AppHandle, url: &str) -> Result<Page, String> {
    let resp = app.state::<AppState>().ai.http
        .get(url)
        .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AlterEgo")
        .header("Accept-Language", "it,en;q=0.8")
        .timeout(Duration::from_secs(25))
        .send()
        .await
        .map_err(|e| crate::tr!("pagina non raggiungibile: {}", "page not reachable: {}", providers::describe_net_error(&e)))?;
    if !resp.status().is_success() {
        return Err(format!("HTTP {}", resp.status().as_u16()));
    }
    let ctype = resp.headers().get("content-type").and_then(|v| v.to_str().ok()).unwrap_or("").to_lowercase();
    if !(ctype.is_empty() || ctype.contains("html") || ctype.contains("text") || ctype.contains("json") || ctype.contains("xml")) {
        return Err(crate::tr!("non è una pagina di testo ({ctype})", "not a text page ({ctype})"));
    }
    let bytes = resp.bytes().await.map_err(|e| e.to_string())?;
    let raw = String::from_utf8_lossy(&bytes[..bytes.len().min(3_000_000)]).to_string();
    let lower = raw.to_lowercase();
    let title = lower
        .find("<title")
        .and_then(|a| {
            let st = raw[a..].find('>')? + a + 1;
            let en = lower[st..].find("</title>")? + st;
            Some(html_to_text(&raw[st..en]))
        })
        .unwrap_or_default();
    let content = if ctype.contains("html") || lower.contains("<html") { html_to_text(&raw) } else { raw };
    Ok(Page { title, content })
}

pub async fn fetch(app: &AppHandle, url: &str) -> Result<Page, String> {
    let u = reqwest::Url::parse(url.trim()).map_err(|_| "URL non valido".to_string())?;
    if !matches!(u.scheme(), "http" | "https") || !u.username().is_empty() || u.password().is_some() {
        return Err(crate::i18n::tr("si possono leggere solo pagine http/https senza credenziali", "only http/https pages without credentials can be read").into());
    }
    let wc = web_cfg(app);
    // "auto": DeepSeek Fetch se la ricerca usa DeepSeek e c'è la chiave, altrimenti il lettore locale
    let use_deepseek = match wc.fetch.as_str() {
        "deepseek" => true,
        "local" => false,
        _ => wc.provider == "deepseek" && available(app, "deepseek"),
    };
    let p = if use_deepseek {
        match call(app, "fetch", json!({ "url": u.as_str() })).await {
            Ok(v) => parse_fetch(&v),
            Err(e) if wc.fallback => local_fetch(app, u.as_str()).await.map_err(|l| format!("{e} · {l}"))?,
            Err(e) => return Err(e),
        }
    } else {
        local_fetch(app, u.as_str()).await?
    };
    if p.content.is_empty() {
        return Err(crate::i18n::tr("la pagina non ha contenuto leggibile", "the page has no readable content").into());
    }
    Ok(p)
}

/// Chiave di Tavily o Brave (vuota = cancella).
#[tauri::command]
pub fn set_web_key(provider: String, key: String) -> Result<(), String> {
    if !["tavily", "brave"].contains(&provider.as_str()) {
        return Err("provider senza chiave propria".into());
    }
    if key.trim().is_empty() { crate::keys::delete_slot(&provider, "1") } else { crate::keys::set_slot(&provider, "1", key.trim()) }
}

#[tauri::command]
pub fn web_keys() -> std::collections::HashMap<String, bool> {
    ["tavily", "brave"].iter().map(|p| (p.to_string(), crate::keys::get_slot(p, "1").is_some())).collect()
}

#[tauri::command]
pub fn set_web_config(app: AppHandle, web: crate::config::WebConfig) {
    crate::update_config(&app, |c| {
        c.web = web;
        c.normalize_voice();
    });
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

    #[test]
    fn other_providers() {
        let html = r#"<div><a rel="nofollow" class="result__a" href="//duckduckgo.com/l/?uddg=https%3A%2F%2Fexample.com%2Fa%3Fb%3D1&amp;rut=x">Esempio &amp; co</a>
            <a class="result__snippet" href="x">Un <b>estratto</b> di prova</a></div>
            <div><a class="result__a" href="https://diretto.it/">Diretto</a></div>"#;
        let h = parse_ddg(html);
        assert_eq!(h[0], Hit { title: "Esempio & co".into(), url: "https://example.com/a?b=1".into(), snippet: "Un estratto di prova".into() });
        assert_eq!(h[1].url, "https://diretto.it/");
        let g = json!({"candidates": [{"content": {"parts": [{"text": "Roma: 18°C"}]},
            "groundingMetadata": {"groundingChunks": [{"web": {"uri": "https://meteo.it", "title": "meteo.it"}}]}}]});
        let h = parse_gemini(&g, "meteo roma");
        assert_eq!((h[0].snippet.as_str(), h[0].url.as_str(), h[1].title.as_str()), ("Roma: 18°C", "https://meteo.it", "meteo.it"));
        let t = html_to_text("<html><head><title>x</title></head><body><script>var a=1</script><h1>Titolo</h1><p>Uno&nbsp;&amp; due</p></body></html>");
        assert_eq!(t, "Titolo\nUno & due");
        assert_eq!(dec("a%20b+c%2Fd"), "a b c/d");
        assert_eq!(enc("a b/è"), "a+b%2F%C3%A8");
    }
}
