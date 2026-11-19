//! Battute brevi per Minecraft (colpita, creeper, trovato diamanti, notte…) scritte dal modello invece che fisse,
//! come gmodlink/quips.rs: un pacchetto ogni tanto, prompt corto e fisso (in cache), poche centinaia di token.
//! Il bot le pesca senza ripeterle; senza modello usa le sue righe di riserva (mc_bot/lib.js, FALLBACK).

use serde_json::Value;
use std::collections::BTreeMap;

/// Categorie (stessi nomi di FALLBACK in lib.js) con una descrizione cortissima per il modello.
pub const KINDS: &[(&str, &str)] = &[
    ("hurt", "you got hit"),
    ("flee", "running away from danger"),
    ("died", "you just died"),
    ("creeper", "a creeper is right next to you"),
    ("greet", "you just joined the world / a friend shows up"),
    ("found", "you spotted diamonds or something rare"),
    ("done", "you finished what you were asked"),
    ("night", "night is falling, mobs are coming"),
    ("hungry", "your hunger bar is low"),
    ("oops", "something didn't work (can't reach, missing stuff)"),
    ("bored", "nothing is happening"),
    ("eat", "you're eating"),
    ("dodge", "an arrow just missed you"),
];

const PER_KIND: usize = 4;
const MAX_PER_KIND: usize = 8;
const MAX_CHARS: usize = 60;

/// Prompt di sistema: uguale per la stessa lingua e gli stessi pronomi (in cache).
pub fn system(pron_rule: &str, lang_rule: &str) -> String {
    format!(
        "You write quick reaction lines for Alter Ego (the AI made by Chihiro Fujisaki in Danganronpa: kind, a bit shy, playful) \
playing Minecraft with friends. They go in the game chat and are spoken aloud, so they must sound like a real player typing fast: \
1-5 words, casual, mostly lowercase, gamer slang ok, no emojis, no quotes, no hashtags, never mention being an AI.\n\
Reply ONLY with one JSON object: keys are the categories, each value is an array of {PER_KIND} different lines.\n{pron_rule}{lang_rule}"
    )
}

pub fn user() -> String {
    let list: Vec<String> = KINDS.iter().map(|(k, d)| format!("{k}: {d}")).collect();
    format!("Categories:\n{}", list.join("\n"))
}

fn clean(s: &str) -> Option<String> {
    let t = s.trim().trim_start_matches(['-', '*', '•']).trim().trim_matches(['"', '\'', '`', '“', '”', '«', '»']).trim();
    let t: String = t.split_whitespace().collect::<Vec<_>>().join(" ");
    // niente comandi in chat
    if t.is_empty() || t.starts_with('/') || t.chars().count() > MAX_CHARS || t.contains(['[', ']', '{', '}', '<', '>']) {
        return None;
    }
    Some(t)
}

/// Legge il pacchetto dal testo del modello (anche con recinti markdown). None se troppo poco.
pub fn parse(raw: &str) -> Option<BTreeMap<String, Vec<String>>> {
    let (s, e) = (raw.find('{')?, raw.rfind('}')?);
    if e <= s {
        return None;
    }
    let v: Value = serde_json::from_str(&raw[s..=e]).ok()?;
    let mut out = BTreeMap::new();
    for (k, x) in v.as_object()? {
        let k = k.trim().to_lowercase();
        if !KINDS.iter().any(|(n, _)| *n == k) {
            continue;
        }
        let items: Vec<&str> = match x {
            Value::Array(a) => a.iter().filter_map(Value::as_str).collect(),
            Value::String(s) => vec![s.as_str()],
            _ => continue,
        };
        let mut lines: Vec<String> = Vec::new();
        for it in items {
            if let Some(l) = clean(it) {
                if !lines.iter().any(|x| x.to_lowercase() == l.to_lowercase()) && lines.len() < MAX_PER_KIND {
                    lines.push(l);
                }
            }
        }
        if lines.len() >= 2 {
            out.insert(k, lines);
        }
    }
    (out.len() >= 3).then_some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_minecraft_quips() {
        let raw = "```json\n{\"hurt\": [\"ahia\", \"ma dai\", \"/kill @a\"], \"creeper\": [\"creeper!!\", \"nope\"], \"night\": [\"notte...\", \"occhio\"], \"boh\": [\"a\", \"b\"], \"done\": [\"fatto\"]}\n```";
        let b = parse(raw).unwrap();
        assert_eq!(b["hurt"], vec!["ahia", "ma dai"], "niente comandi");
        assert!(b.contains_key("creeper") && b.contains_key("night") && !b.contains_key("boh") && !b.contains_key("done"));
        assert!(parse("{\"hurt\": [\"a\", \"b\"]}").is_none());
        assert!(parse("niente").is_none());
        let s = system("\n## Your pronouns\nthey\n", "\n## Language\nItalian\n");
        assert!(s.contains("Minecraft") && s.contains("JSON") && s.len() < 900);
        assert_eq!(user().lines().count(), KINDS.len() + 1);
    }
}
