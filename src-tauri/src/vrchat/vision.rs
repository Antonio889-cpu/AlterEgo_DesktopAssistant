//! Vista (facoltativa): uno screenshot piccolo della finestra di VRChat diventa una descrizione corta.
//! - locale: un modello di visione di Ollama descrive e basta; a decidere resta il provider predefinito;
//! - cloud: lo screenshot va al provider predefinito (se ha la visione attiva).
//! In tutti e due i casi al modello che decide arriva solo il testo (pochi token, niente immagini ripetute).

use serde_json::{json, Value};
use std::time::Duration;

/// Modelli piccoli consigliati (Ollama) se non ce n'è uno installato.
pub const RECOMMENDED: &[&str] = &["qwen2.5vl:3b", "gemma3:4b", "moondream:1.8b"];
/// Lato lungo dello screenshot.
pub const SHOT_MAX: u32 = 640;

pub const PROMPT: &str = "You are the eyes of a blind VRChat player. Describe this first-person screenshot in at most 50 words, plain text: \
people/avatars (how many; where: left, center or right; near, mid or far; what they're doing), the place, and any sign or menu that matters. \
Then write a last line exactly like: NEAREST: <left|center|right|none> <near|mid|far>";

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Sight {
    pub text: String,
    /// Persona più vicina: direzione (-1 sinistra, 0 centro, 1 destra) e distanza ("near", "mid", "far").
    pub nearest: Option<(f32, String)>,
}

/// Legge la descrizione del modello di visione.
pub fn parse(raw: &str) -> Sight {
    let mut lines = Vec::new();
    let mut nearest = None;
    for l in raw.replace('\r', "").lines() {
        let t = l.trim().trim_matches(['*', '`']).trim();
        if t.is_empty() {
            continue;
        }
        if let Some(rest) = t.to_ascii_lowercase().strip_prefix("nearest:") {
            let w: Vec<&str> = rest.split_whitespace().collect();
            let dir = match w.first().copied() {
                Some("left") => Some(-1.0),
                Some("center") | Some("centre") | Some("middle") => Some(0.0),
                Some("right") => Some(1.0),
                _ => None,
            };
            let dist = match w.get(1).copied() {
                Some(d @ ("near" | "mid" | "far")) => d.to_string(),
                _ => "mid".into(),
            };
            nearest = dir.map(|d| (d, dist));
            continue;
        }
        lines.push(t.to_string());
    }
    let text: String = lines.join(" ").chars().take(360).collect();
    Sight { text, nearest }
}

/// Screenshot della finestra di VRChat (tutto lo schermo se non si trova), PNG base64.
pub fn capture() -> Result<String, String> {
    crate::actions::screen::window_base64("VRChat", SHOT_MAX)
}

/// Descrizione con un modello di visione di Ollama (API nativa, una richiesta, niente streaming).
pub async fn describe_local(http: &reqwest::Client, base: &str, model: &str, image: &str) -> Result<String, String> {
    if model.trim().is_empty() {
        return Err(crate::i18n::tr("scegli un modello di visione locale", "choose a local vision model").into());
    }
    let body = json!({
        "model": model.trim(),
        "stream": false,
        "keep_alive": "10m",
        "messages": [{ "role": "user", "content": PROMPT, "images": [image] }],
        "options": { "temperature": 0.2, "num_predict": 160 },
    });
    let r = http.post(format!("{}/api/chat", base.trim_end_matches('/'))).json(&body).timeout(Duration::from_secs(60)).send().await.map_err(|e| format!("Ollama: {e}"))?;
    let st = r.status();
    let v: Value = r.json().await.unwrap_or(Value::Null);
    if !st.is_success() {
        return Err(format!("Ollama: {}", v.get("error").and_then(Value::as_str).unwrap_or(st.as_str())));
    }
    Ok(v.pointer("/message/content").and_then(Value::as_str).unwrap_or("").to_string())
}

/// Il modello Ollama vede le immagini? (capacità dichiarate, oppure il nome)
pub fn has_vision(name: &str, show: &Value) -> bool {
    if let Some(c) = show.get("capabilities").and_then(Value::as_array) {
        return c.iter().any(|x| x == "vision");
    }
    let n = name.to_lowercase();
    ["vl", "vision", "llava", "moondream", "gemma3", "gemma4", "minicpm-v", "bakllava", "llama4"].iter().any(|k| n.contains(k))
}

/// Modelli installati in Ollama che vedono le immagini.
pub async fn local_models(http: &reqwest::Client, base: &str) -> Vec<String> {
    let base = base.trim_end_matches('/');
    let Ok(r) = http.get(format!("{base}/api/tags")).timeout(Duration::from_secs(4)).send().await else { return Vec::new() };
    let tags: Value = r.json().await.unwrap_or(Value::Null);
    let names: Vec<String> = tags.get("models").and_then(Value::as_array).into_iter().flatten().filter_map(|m| m.get("name").and_then(Value::as_str).map(String::from)).collect();
    let mut out = Vec::new();
    for n in names {
        let show = match http.post(format!("{base}/api/show")).json(&json!({ "model": n })).timeout(Duration::from_secs(4)).send().await {
            Ok(r) => r.json::<Value>().await.unwrap_or(Value::Null),
            Err(_) => Value::Null,
        };
        if has_vision(&n, &show) {
            out.push(n);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_descriptions() {
        let s = parse("Two avatars stand near a big mirror; one on the right is waving.\n\n**NEAREST: right near**");
        assert_eq!(s.text, "Two avatars stand near a big mirror; one on the right is waving.");
        assert_eq!(s.nearest, Some((1.0, "near".into())));
        assert_eq!(parse("A dark room.\nNEAREST: none").nearest, None);
        assert_eq!(parse("x\nnearest: Center").nearest, Some((0.0, "mid".into())));
        assert_eq!(parse("No marker").nearest, None);
        assert!(parse(&"word ".repeat(200)).text.chars().count() <= 360);
        assert!(has_vision("x", &json!({ "capabilities": ["completion", "vision"] })));
        assert!(!has_vision("qwen2.5vl", &json!({ "capabilities": ["completion"] })), "le capacità dichiarate vincono");
        assert!(has_vision("qwen2.5vl:3b", &Value::Null));
        assert!(!has_vision("llama3.2:3b", &Value::Null));
        assert!(PROMPT.contains("NEAREST:"));
    }
}
