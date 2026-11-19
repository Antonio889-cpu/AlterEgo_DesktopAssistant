//! Quello che Alter Ego impara giocando a Minecraft (file JSON nei dati dell'app, come persona e memoria di gmodlink):
//! - abilità: sequenze di azioni riuscite, con un nome, da rifare con `[act] skill nome` (zero token);
//! - lezioni: azioni fallite e perché ("craft stone_pickaxe: mancano i materiali…").
//! Nel prompt vanno solo quelle pertinenti alla frase (parole chiave in comune): pochi token.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::{Path, PathBuf};

const FILE: &str = "minecraft_skills.json";
const MAX_SKILLS: usize = 40;
const MAX_LESSONS: usize = 40;
const MAX_OPS: usize = 12;

#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq)]
#[serde(default)]
pub struct Skill {
    pub name: String,
    /// Comandi per il bot (senza id).
    pub ops: Vec<Value>,
    pub keys: Vec<String>,
    /// Volte che ha funzionato.
    pub ok: u32,
    pub last: i64,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq)]
#[serde(default)]
pub struct Lesson {
    pub text: String,
    pub keys: Vec<String>,
    pub n: u32,
    pub last: i64,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq)]
#[serde(default)]
pub struct Store {
    pub skills: Vec<Skill>,
    pub lessons: Vec<Lesson>,
}

const STOP: &[&str] = &[
    "the", "and", "for", "you", "your", "with", "some", "can", "please", "make", "get", "let", "lets", "che", "per", "una", "uno", "con", "del", "della",
    "dei", "delle", "gli", "puoi", "fai", "fammi", "mi", "ci", "di", "da", "il", "lo", "la", "le", "un", "and", "alter", "ego", "alterego", "minecraft", "dai",
    "op", "n", "who", "item", "block", "shape", "size",
];

/// Parole chiave di un testo: minuscole, ≥ 3 lettere, senza parole vuote; i nomi con "_" anche a pezzi
/// (oak_planks → oak_planks, oak, planks). Plurali semplici ridotti (planks resta, logs → log).
pub fn keywords(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut push = |w: &str| {
        let w = w.trim_matches('_');
        if w.chars().count() < 3 || STOP.contains(&w) || w.chars().all(|c| c.is_ascii_digit()) {
            return;
        }
        let w = w.to_string();
        if !out.contains(&w) {
            out.push(w);
        }
    };
    for raw in text.to_lowercase().split(|c: char| !(c.is_alphanumeric() || c == '_')) {
        if raw.is_empty() {
            continue;
        }
        let parts: Vec<&str> = if raw.contains('_') { std::iter::once(raw).chain(raw.split('_')).collect() } else { vec![raw] };
        for p in parts {
            push(p);
            if let Some(s) = p.strip_suffix('s').filter(|s| s.len() >= 3 && !p.ends_with("ss")) {
                push(s);
            }
        }
    }
    out
}

/// Parole chiave dei comandi (oggetti, blocchi, forme).
fn op_keys(ops: &[Value]) -> Vec<String> {
    let text: Vec<String> = ops
        .iter()
        .flat_map(|o| ["block", "item", "shape", "what"].iter().filter_map(|k| o[*k].as_str().map(str::to_string)).collect::<Vec<_>>())
        .collect();
    keywords(&text.join(" "))
}

/// Comandi senza id e senza quelli che non servono a rifarli.
fn clean_ops(ops: &[Value]) -> Vec<Value> {
    ops.iter()
        .filter(|o| !matches!(o["op"].as_str(), Some("say" | "look" | "jump" | "stop" | "skill" | "reflex" | "leave" | "config" | "quips" | "skin") | None))
        .map(|o| {
            let mut o = o.clone();
            if let Some(m) = o.as_object_mut() {
                m.remove("id");
            }
            o
        })
        .take(MAX_OPS)
        .collect()
}

impl Store {
    /// Una sequenza riuscita diventa (o rinforza) un'abilità. Restituisce il nome usato, None se non vale la pena.
    pub fn add_skill(&mut self, name: Option<&str>, ops: &[Value], request: &str, now: i64) -> Option<String> {
        let ops = clean_ops(ops);
        // una sola azione semplice non è un'abilità (seguire, venire…), a meno che il modello non l'abbia chiamata
        let worth = ops.len() >= 2 || ops.iter().any(|o| matches!(o["op"].as_str(), Some("collect" | "craft" | "smelt" | "build")));
        if ops.is_empty() || (!worth && name.is_none()) {
            return None;
        }
        let mut keys = keywords(request);
        for k in op_keys(&ops) {
            if !keys.contains(&k) {
                keys.push(k);
            }
        }
        keys.truncate(16);
        let name = match name.map(super::prompt::skill_name).filter(|n| !n.is_empty()) {
            Some(n) => n,
            None => {
                let base: Vec<String> = ops.iter().take(3).map(|o| super::prompt::skill_name(&super::prompt::summary(o))).collect();
                base.join("_").chars().take(32).collect::<String>().trim_end_matches('_').to_string()
            }
        };
        if let Some(s) = self.skills.iter_mut().find(|s| s.name == name || s.ops == ops) {
            s.ok += 1;
            s.last = now;
            s.ops = ops;
            for k in keys {
                if !s.keys.contains(&k) && s.keys.len() < 20 {
                    s.keys.push(k);
                }
            }
            return Some(s.name.clone());
        }
        self.skills.push(Skill { name: name.clone(), ops, keys, ok: 1, last: now });
        if self.skills.len() > MAX_SKILLS {
            // via la meno usata (e più vecchia)
            if let Some(i) = self.skills.iter().enumerate().min_by_key(|(_, s)| (s.ok, s.last)).map(|(i, _)| i) {
                self.skills.remove(i);
            }
        }
        Some(name)
    }

    /// Un'azione fallita diventa una lezione ("craft iron_pickaxe: missing materials").
    pub fn add_lesson(&mut self, op: &Value, err: &str, request: &str, now: i64) {
        let what = super::prompt::summary(op);
        let err: String = err.split_whitespace().collect::<Vec<_>>().join(" ").chars().take(120).collect();
        // fermata a mano o interrotta: non è una lezione
        if err.is_empty() || ["stopped", "fermata", "goal was changed", "not connected"].iter().any(|w| err.to_lowercase().contains(w)) {
            return;
        }
        let text = format!("{what}: {err}");
        if let Some(l) = self.lessons.iter_mut().find(|l| l.text == text) {
            l.n += 1;
            l.last = now;
            return;
        }
        let mut keys = keywords(&format!("{what} {request}"));
        keys.truncate(16);
        self.lessons.push(Lesson { text, keys, n: 1, last: now });
        if self.lessons.len() > MAX_LESSONS {
            if let Some(i) = self.lessons.iter().enumerate().min_by_key(|(_, l)| (l.n, l.last)).map(|(i, _)| i) {
                self.lessons.remove(i);
            }
        }
    }

    pub fn get(&self, name: &str) -> Option<&Skill> {
        let n = super::prompt::skill_name(name);
        self.skills.iter().find(|s| s.name == n)
    }

    /// Abilità e lezioni pertinenti a `text` (al massimo `n` per tipo), dalla più pertinente.
    pub fn relevant(&self, text: &str, n: usize) -> (Vec<&Skill>, Vec<&Lesson>) {
        let words = keywords(text);
        let score = |keys: &[String]| keys.iter().filter(|k| words.contains(k)).count();
        let mut s: Vec<(usize, &Skill)> = self.skills.iter().map(|x| (score(&x.keys) + usize::from(words.contains(&x.name)) * 3, x)).filter(|x| x.0 > 0).collect();
        s.sort_by(|a, b| b.0.cmp(&a.0).then(b.1.ok.cmp(&a.1.ok)));
        let mut l: Vec<(usize, &Lesson)> = self.lessons.iter().map(|x| (score(&x.keys), x)).filter(|x| x.0 > 0).collect();
        l.sort_by(|a, b| b.0.cmp(&a.0).then(b.1.last.cmp(&a.1.last)));
        (s.into_iter().take(n).map(|x| x.1).collect(), l.into_iter().take(n).map(|x| x.1).collect())
    }

    /// Righe per il prompt (vuoto se niente di pertinente): poche decine di token.
    pub fn prompt_block(&self, text: &str) -> String {
        let (s, l) = self.relevant(text, 3);
        let mut out = String::new();
        if !s.is_empty() {
            let list: Vec<String> = s.iter().map(|k| format!("{} = {}", k.name, k.ops.iter().map(super::prompt::summary).collect::<Vec<_>>().join(", "))).collect();
            out.push_str(&format!("[your skills, use with [act] skill <name>] {}", list.join(" | ")));
        }
        if !l.is_empty() {
            if !out.is_empty() {
                out.push('\n');
            }
            out.push_str(&format!("[lessons learned] {}", l.iter().map(|x| x.text.as_str()).collect::<Vec<_>>().join(" | ")));
        }
        out
    }
}

pub fn path(dir: &Path) -> PathBuf {
    dir.join(FILE)
}

pub fn load(dir: &Path) -> Store {
    std::fs::read_to_string(path(dir)).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
}

pub fn save(dir: &Path, st: &Store) {
    if let Ok(s) = serde_json::to_string_pretty(st) {
        let tmp = dir.join(format!("{FILE}.tmp"));
        if std::fs::write(&tmp, s).is_ok() {
            let _ = std::fs::rename(&tmp, path(dir));
        }
    }
}

pub fn forget(dir: &Path) {
    let _ = std::fs::remove_file(path(dir));
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn keyword_selection() {
        let k = keywords("Fammi delle oak_planks, per favore!");
        for w in ["oak_planks", "oak", "planks", "plank"] {
            assert!(k.contains(&w.to_string()), "{w}: {k:?}");
        }
        assert!(!k.contains(&"fammi".to_string()) && !k.contains(&"per".to_string()), "{k:?}");
        assert!(keywords("make some logs").contains(&"log".to_string()));
        assert!(keywords("a 12 op").is_empty());
    }

    #[test]
    fn learns_skills_and_lessons() {
        let dir = std::env::temp_dir().join(format!("ae-mc-skills-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut st = Store::default();
        let ops = vec![
            json!({ "op": "collect", "block": "wood", "n": 3, "id": 5 }),
            json!({ "op": "craft", "item": "oak_planks", "n": 12, "id": 6 }),
            json!({ "op": "say", "text": "fatto" }),
        ];
        assert_eq!(st.add_skill(Some("Make Planks"), &ops, "fammi delle assi di legno", 10).as_deref(), Some("make_planks"));
        assert_eq!(st.skills[0].ops.len(), 2, "say via, id via");
        assert!(st.skills[0].ops[0].get("id").is_none());
        // stessa sequenza: rinforzo, non doppione
        assert_eq!(st.add_skill(None, &ops, "assi", 20).as_deref(), Some("make_planks"));
        assert_eq!((st.skills.len(), st.skills[0].ok), (1, 2));
        // una sola azione semplice senza nome: niente abilità
        assert!(st.add_skill(None, &[json!({ "op": "follow", "who": "M" })], "seguimi", 1).is_none());
        // nome automatico
        let n = st.add_skill(None, &[json!({ "op": "craft", "item": "stick", "n": 4 })], "bastoni", 30).unwrap();
        assert_eq!(n, "craft_stick_4");
        st.add_lesson(&json!({ "op": "craft", "item": "stone_pickaxe" }), "missing materials for stone_pickaxe (cobblestone, stick)", "make a stone pickaxe", 40);
        st.add_lesson(&json!({ "op": "craft", "item": "stone_pickaxe" }), "missing materials for stone_pickaxe (cobblestone, stick)", "", 41);
        st.add_lesson(&json!({ "op": "collect", "block": "wood" }), "stopped", "", 42);
        assert_eq!((st.lessons.len(), st.lessons[0].n), (1, 2), "doppione contato, interruzione ignorata");
        // nel prompt solo le cose pertinenti
        let b = st.prompt_block("mi fai delle planks?");
        assert!(b.contains("make_planks = collect wood 3, craft oak_planks 12") && !b.contains("stick") && !b.contains("lessons"), "{b}");
        let b = st.prompt_block("craft a stone pickaxe");
        assert!(b.contains("[lessons learned] craft stone_pickaxe: missing materials"), "{b}");
        assert_eq!(st.prompt_block("ciao come stai"), "");
        assert_eq!(st.get("Make Planks").map(|s| s.ops.len()), Some(2));
        // file
        save(&dir, &st);
        assert_eq!(load(&dir), st);
        forget(&dir);
        assert_eq!(load(&dir), Store::default());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn capped_store() {
        let mut st = Store::default();
        for i in 0..(MAX_SKILLS + 5) {
            st.add_skill(Some(&format!("s{i}")), &[json!({ "op": "craft", "item": format!("x{i}") })], "", i as i64);
        }
        assert_eq!(st.skills.len(), MAX_SKILLS);
        for i in 0..(MAX_LESSONS + 5) {
            st.add_lesson(&json!({ "op": "craft", "item": "a" }), &format!("err {i}"), "", i as i64);
        }
        assert_eq!(st.lessons.len(), MAX_LESSONS);
    }
}
