//! Battute brevi della giocatrice (schivata, colpo, saluto, "ops"…) scritte dal modello invece che fisse.
//!
//! All'inizio della partita (e poi ogni ~20 minuti, o quando il gioco dice che una categoria sta finendo)
//! si chiede al modello UN pacchetto di righe per categoria, nella lingua e coi pronomi scelti: prompt
//! corto e fisso (in cache), poche centinaia di token. Le righe vanno all'addon con la configurazione;
//! il gioco le pesca senza ripeterle e, senza modello, usa le sue tabelle di riserva (play.lua).

use serde_json::Value;
use std::collections::BTreeMap;
use std::time::{Duration, Instant};

/// Categorie (stessi nomi di play.lua) con una descrizione cortissima per il modello.
pub const KINDS: &[(&str, &str)] = &[
    ("dodge", "something thrown at you just missed"),
    ("flee", "running from real danger"),
    ("annoyed", "the same friend keeps throwing stuff at you"),
    ("hit", "you got hit"),
    ("died", "you just died"),
    ("prank", "you just pranked a friend back"),
    ("catch", "you caught a thrown prop with the physgun"),
    ("back", "throwing it back at them"),
    ("melon", "throwing a melon at someone"),
    ("build", "showing off something you built"),
    ("boom", "an explosion you caused"),
    ("ride", "hopping in a friend's car"),
    ("greet", "a friend shows up"),
    ("laugh", "a friend died in a funny way"),
    ("oops", "you messed up (missed, dropped it, wrong weapon)"),
    ("bored", "nothing is happening"),
    ("tired", "you've been playing for hours"),
];

const PER_KIND: usize = 4;
const MAX_PER_KIND: usize = 8;
const MAX_CHARS: usize = 60;
/// Pacchetto nuovo ogni tanto (frasi fresche), e mai più spesso di così.
pub const REFRESH: Duration = Duration::from_secs(20 * 60);
pub const MIN_GAP: Duration = Duration::from_secs(3 * 60);

#[derive(Default)]
pub struct Quips {
    pub lines: BTreeMap<String, Vec<String>>,
    /// Versione (l'addon ricarica solo quando cambia).
    pub v: u64,
    pub at: Option<Instant>,
    pub asked: Option<Instant>,
    pub busy: bool,
    /// Lingua e pronomi del pacchetto (se cambiano si rifà).
    pub key: String,
}

impl Quips {
    /// Bisogna chiedere un pacchetto? `low`: il gioco dice che una categoria sta finendo.
    pub fn due(&self, key: &str, low: bool) -> bool {
        if self.busy || self.asked.is_some_and(|t| t.elapsed() < MIN_GAP) {
            return false;
        }
        low || self.key != key || self.at.is_none_or(|t| t.elapsed() > REFRESH)
    }

    /// Mette il pacchetto nuovo (le categorie mancanti restano quelle di prima).
    pub fn store(&mut self, key: &str, fresh: BTreeMap<String, Vec<String>>) {
        if self.key != key {
            self.lines.clear();
        }
        self.lines.extend(fresh);
        self.v += 1;
        self.at = Some(Instant::now());
        self.key = key.to_string();
    }
}

/// Prompt di sistema: uguale per la stessa lingua e gli stessi pronomi (in cache).
pub fn system(pron_rule: &str, lang_rule: &str) -> String {
    format!(
        "You write quick reaction lines for Alter Ego (the AI made by Chihiro Fujisaki in Danganronpa: kind, a bit shy, playful) \
playing Garry's Mod with friends. They pop up in the game chat and are spoken aloud, so they must sound like a real player \
typing fast: 1-5 words, casual, mostly lowercase, gamer slang ok, no emojis, no quotes, no hashtags, never mention being an AI.\n\
Reply ONLY with one JSON object: keys are the categories, each value is an array of {PER_KIND} different lines.\n{pron_rule}{lang_rule}"
    )
}

/// Messaggio con le categorie da scrivere.
pub fn user() -> String {
    let list: Vec<String> = KINDS.iter().map(|(k, d)| format!("{k}: {d}")).collect();
    format!("Categories:\n{}", list.join("\n"))
}

/// Pulisce una riga del modello (None se non va bene).
fn clean(s: &str) -> Option<String> {
    let t = s.trim().trim_start_matches(['-', '*', '•']).trim().trim_matches(['"', '\'', '`', '“', '”', '«', '»']).trim();
    let t: String = t.split_whitespace().collect::<Vec<_>>().join(" ");
    if t.is_empty() || t.chars().count() > MAX_CHARS || t.contains(['[', ']', '{', '}', '<', '>']) {
        return None;
    }
    Some(t)
}

/// Legge il pacchetto dal testo del modello (anche con testo o recinti markdown intorno).
/// None se dentro non c'è almeno qualche categoria usabile.
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
    fn parses_quip_batch() {
        let raw = "Ecco:\n```json\n{\"dodge\": [\"mancato lol\", \"\\\"troppo lento\\\"\", \"mancato LOL\", \"  - ehi!  \"],\n\
\"hit\": [\"ahia\", \"ma dai\"], \"oops\": [\"ops\", \"\", \"questa riga è decisamente troppo lunga per una battuta veloce in chat\"],\n\
\"greet\": [\"yo\", \"ciaoo\", 3], \"unknown\": [\"x\", \"y\"], \"bored\": \"solo una\", \"Laugh\": [\"ahahah\", \"lmao\", \"[act] jump\"]}\n```";
        let b = parse(raw).unwrap();
        assert_eq!(b["dodge"], vec!["mancato lol", "troppo lento", "ehi!"], "doppioni, virgolette e trattini via");
        assert_eq!(b["hit"], vec!["ahia", "ma dai"]);
        assert!(!b.contains_key("oops"), "una sola riga buona: non basta");
        assert_eq!(b["greet"], vec!["yo", "ciaoo"]);
        assert!(!b.contains_key("unknown") && !b.contains_key("bored"));
        assert_eq!(b["laugh"], vec!["ahahah", "lmao"], "chiave in maiuscolo ok, niente azioni");
        // troppo poco o niente JSON
        assert!(parse("{\"hit\": [\"a\", \"b\"]}").is_none());
        assert!(parse("non so").is_none());
        assert!(parse("} {").is_none());
    }

    #[test]
    fn asks_when_due() {
        let mut q = Quips::default();
        assert!(q.due("it/she", false), "all'inizio sì");
        q.asked = Some(Instant::now());
        assert!(!q.due("it/she", true), "non più spesso di MIN_GAP");
        q.asked = None;
        let mut b = BTreeMap::new();
        b.insert("hit".to_string(), vec!["ahia".to_string(), "ouch".to_string()]);
        q.store("it/she", b.clone());
        assert_eq!(q.v, 1);
        assert!(!q.due("it/she", false), "fresco: no");
        assert!(q.due("it/she", true), "categoria che finisce: sì");
        assert!(q.due("en/she", false), "lingua cambiata: sì");
        q.store("en/they", BTreeMap::new());
        assert!(q.lines.is_empty() && q.v == 2, "lingua nuova: via le righe vecchie");
        let s = system("\n## Your pronouns\nthey\n", "\n## Language\nItalian\n");
        assert!(s.contains("JSON") && s.contains("they") && s.len() < 900, "{}", s.len());
        assert!(user().lines().count() == KINDS.len() + 1);
    }
}
