//! Battute brevi in VRChat (carezza, "boop", colpo, saluto, persa…): un pacchetto scritto dal modello
//! all'ingresso (lingua e pronomi giusti, prompt corto in cache), con righe di riserva neutre.
//! Si pescano senza ripetersi e senza token.

use serde_json::Value;
use std::collections::BTreeMap;

pub const KINDS: &[(&str, &str)] = &[
    ("headpat", "someone gives you a headpat"),
    ("boop", "someone boops your nose"),
    ("hug", "someone hugs you"),
    ("hit", "someone hits or pulls you (playfully)"),
    ("poke", "someone pokes or tickles you"),
    ("greet", "you just arrived in a world"),
    ("lost", "you tried to follow by ear but lost them"),
    ("bored", "nobody is talking for a while"),
];

/// Righe di riserva (italiano, inglese): neutre, vanno con ogni pronome.
fn fallback(kind: &str, en: bool) -> &'static [&'static str] {
    match (kind, en) {
        ("headpat", false) => &["Ehehe… grazie!", "Mi piace!", "Ancora?"],
        ("headpat", true) => &["Ehehe… thanks!", "I like that!", "Again?"],
        ("boop", false) => &["Boop!", "Ehi, il naso!", "Ahah, preso!"],
        ("boop", true) => &["Boop!", "Hey, my nose!", "Haha, got me!"],
        ("hug", false) => &["Che abbraccio!", "Aww…"],
        ("hug", true) => &["What a hug!", "Aww…"],
        ("hit", false) => &["Ahia!", "Ehi! Piano!", "Ma dai!"],
        ("hit", true) => &["Ouch!", "Hey! Easy!", "Come on!"],
        ("poke", false) => &["Ehi!", "Mi fai il solletico!"],
        ("poke", true) => &["Hey!", "That tickles!"],
        ("greet", false) => &["Ciao a tutti!", "Eccomi!"],
        ("greet", true) => &["Hi everyone!", "I'm here!"],
        ("lost", false) => &["Dove sei? Parla, ti seguo!", "Non ti sento più… dimmi qualcosa!"],
        ("lost", true) => &["Where did you go? Say something!", "I lost you… talk to me!"],
        (_, false) => &["Mmh…", "Che silenzio…"],
        (_, true) => &["Hmm…", "So quiet…"],
    }
}

#[derive(Default)]
pub struct Quips {
    pub lines: BTreeMap<String, Vec<String>>,
    used: BTreeMap<String, usize>,
    pub busy: bool,
    /// Lingua e pronomi del pacchetto.
    pub key: String,
}

impl Quips {
    /// Una riga per `kind` (generata se c'è, altrimenti di riserva), a giro senza ripetersi.
    pub fn pick(&mut self, kind: &str, en: bool) -> String {
        let n = self.used.entry(kind.to_string()).or_insert(0);
        *n += 1;
        if let Some(l) = self.lines.get(kind).filter(|l| !l.is_empty()) {
            return l[*n % l.len()].clone();
        }
        let f = fallback(kind, en);
        f[*n % f.len()].to_string()
    }

    pub fn store(&mut self, key: &str, fresh: BTreeMap<String, Vec<String>>) {
        self.lines = fresh;
        self.key = key.to_string();
    }
}

pub fn system(pron_rule: &str, lang_rule: &str) -> String {
    format!(
        "You write quick reaction lines for Alter Ego (the AI made by Chihiro Fujisaki in Danganronpa: kind, a bit shy, playful) \
hanging out in VRChat. They are spoken aloud and shown in the chatbox: 1-5 words, casual, no emojis, no quotes.\n\
Reply ONLY with one JSON object: keys are the categories, each value is an array of 4 different lines.\n{pron_rule}{lang_rule}"
    )
}

pub fn user() -> String {
    let list: Vec<String> = KINDS.iter().map(|(k, d)| format!("{k}: {d}")).collect();
    format!("Categories:\n{}", list.join("\n"))
}

/// Legge il pacchetto (anche con testo intorno). None se non c'è niente di usabile.
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
        let lines: Vec<String> = x
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .map(|l| l.trim().trim_matches(['"', '\'', '`']).trim().to_string())
            .filter(|l| !l.is_empty() && l.chars().count() <= 60 && !l.contains(['[', ']', '{', '}', '<', '>']))
            .take(8)
            .collect();
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
    fn quips_pack_and_fallback() {
        let raw = "ok ```{\"headpat\": [\"ehehe\", \"grazie!\"], \"boop\": [\"boop!\", \"[act] jump\", \"ehi\"], \"greet\": [\"ciao!\", \"yo\"], \"x\": [\"a\", \"b\"]}```";
        let b = parse(raw).unwrap();
        assert_eq!(b["boop"], vec!["boop!", "ehi"]);
        assert!(!b.contains_key("x"));
        assert!(parse("{\"headpat\": [\"a\", \"b\"]}").is_none());
        let mut q = Quips::default();
        let a = q.pick("headpat", false);
        let b2 = q.pick("headpat", false);
        assert_ne!(a, b2, "non si ripete subito");
        for _ in 0..3 {
            assert!(!q.pick("lost", false).contains("persa"), "riserva neutra");
        }
        q.store("it/they", b);
        assert!(["ehehe", "grazie!"].contains(&q.pick("headpat", false).as_str()));
        assert!(!q.pick("bored", true).contains("silenzio"));
        assert!(system("P", "L").contains("JSON") && user().lines().count() == KINDS.len() + 1);
    }
}
