//! Quello che Alter Ego impara guardando i giocatori in Minecraft (minecraft_players.json nei dati dell'app).
//! Il bot (mc_bot/observe.js) manda ogni 30 s un riassunto di quello che si vede da un altro client: cosa
//! hanno in mano e addosso, come si muovono, blocchi piazzati e rotti vicino a loro, crafting dedotto,
//! fornaci accese, voli con l'elytra. Qui si unisce al profilo del giocatore (con un po' di oblio, così le
//! abitudini cambiano col tempo) e diventa abitudini: armatura e attrezzi che usa, cosa raccoglie e costruisce,
//! stile di volo, dove sta la base. Le abitudini tornano al bot (`habits`) e finiscono nel grafo della memoria.
//!
//! Dubbi: quando il bot non è sicuro (rompere blocchi altrui, regole del duello, dove costruire…) chiede
//! all'app; se la risposta è già nel profilo la usa, altrimenti chiede in chat (poche domande, mai di fila)
//! e ricorda la risposta per non chiedere di nuovo.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

const FILE: &str = "minecraft_players.json";
/// A ogni riassunto i conteggi vecchi perdono un po' di peso.
const DECAY: f64 = 0.985;

#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq)]
#[serde(default)]
pub struct Fly {
    pub n: f64,
    pub t: f64,
    pub pitch: f64,
    pub speed: f64,
    pub rockets: f64,
    pub takeoff_jump: f64,
    pub takeoff_drop: f64,
    pub land_soft: f64,
    pub land_hard: f64,
    pub alt: f64,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq)]
#[serde(default)]
pub struct Answer {
    pub value: Value,
    /// Testo detto dal giocatore.
    pub text: String,
    pub at: i64,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq)]
#[serde(default)]
pub struct Profile {
    pub secs: f64,
    pub dist: f64,
    pub sprint: f64,
    pub sneak: f64,
    pub air: f64,
    pub jumps: f64,
    pub held: BTreeMap<String, f64>,
    pub armor: BTreeMap<String, String>,
    pub placed: BTreeMap<String, f64>,
    pub broken: BTreeMap<String, f64>,
    pub crafted: BTreeMap<String, f64>,
    pub smelted: f64,
    /// Celle da 16 blocchi ("x,z") → secondi passati lì.
    pub places: BTreeMap<String, f64>,
    pub fly: Fly,
    /// Risposte alle domande di Alter Ego (chiave → risposta).
    pub answers: BTreeMap<String, Answer>,
    /// Ultima volta che ha chiesto (chiave → secondi).
    pub asked: BTreeMap<String, i64>,
    pub updated: i64,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq)]
#[serde(default)]
pub struct Store {
    pub players: BTreeMap<String, Profile>,
    /// Ultima domanda fatta in chat (per non fare domande di fila).
    pub last_ask: i64,
}

fn merge_map(dst: &mut BTreeMap<String, f64>, src: &Value) {
    for v in dst.values_mut() {
        *v *= DECAY;
    }
    if let Some(o) = src.as_object() {
        for (k, v) in o {
            *dst.entry(k.clone()).or_insert(0.0) += v.as_f64().unwrap_or(0.0);
        }
    }
    dst.retain(|_, v| *v > 0.05);
    // al massimo 40 voci (le più pesanti)
    if dst.len() > 40 {
        let mut all: Vec<(String, f64)> = dst.iter().map(|(k, v)| (k.clone(), *v)).collect();
        all.sort_by(|a, b| b.1.total_cmp(&a.1));
        *dst = all.into_iter().take(40).collect();
    }
}

fn top(m: &BTreeMap<String, f64>, n: usize, filter: impl Fn(&str) -> bool) -> Vec<String> {
    let mut v: Vec<(&String, &f64)> = m.iter().filter(|(k, _)| filter(k)).collect();
    v.sort_by(|a, b| b.1.total_cmp(a.1));
    v.into_iter().take(n).map(|(k, _)| k.clone()).collect()
}

/// Materiale di un pezzo ("iron_chestplate" → "iron").
fn material(item: &str) -> Option<&str> {
    let (m, _) = item.rsplit_once('_')?;
    ["leather", "chainmail", "iron", "golden", "diamond", "netherite", "wooden", "stone"].contains(&m).then_some(m)
}

/// Blocchi naturali (raccolti) contro blocchi da costruzione.
fn natural(b: &str) -> bool {
    b.ends_with("_log") || b.ends_with("_ore") || ["stone", "dirt", "grass_block", "sand", "gravel", "deepslate", "clay", "netherrack", "andesite", "diorite", "granite", "sugar_cane", "pumpkin", "melon", "bamboo"].contains(&b)
}

impl Profile {
    /// Unisce un riassunto del bot (`s` di `observe` in observe.js).
    pub fn merge(&mut self, s: &Value, now: i64) {
        let n = |k: &str| s[k].as_f64().unwrap_or(0.0);
        self.secs = self.secs * DECAY + n("t");
        self.dist = self.dist * DECAY + n("dist");
        self.sprint = self.sprint * DECAY + n("sprint");
        self.sneak = self.sneak * DECAY + n("sneak");
        self.air = self.air * DECAY + n("air");
        self.jumps = self.jumps * DECAY + n("jumps");
        self.smelted = self.smelted * DECAY + n("smelted");
        merge_map(&mut self.held, &s["held"]);
        merge_map(&mut self.placed, &s["placed"]);
        merge_map(&mut self.broken, &s["broken"]);
        merge_map(&mut self.crafted, &s["crafted"]);
        merge_map(&mut self.places, &s["places"]);
        if let Some(a) = s["armor"].as_object() {
            for (k, v) in a {
                if let Some(v) = v.as_str() {
                    self.armor.insert(k.clone(), v.to_string());
                }
            }
        }
        let fl = &s["fly"];
        if fl.is_object() {
            let g = |k: &str| fl[k].as_f64().unwrap_or(0.0);
            let f = &mut self.fly;
            let w_old = f.t;
            let w_new = g("t");
            if w_old + w_new > 0.0 {
                f.pitch = (f.pitch * w_old + g("pitch") * w_new) / (w_old + w_new);
                f.speed = (f.speed * w_old + g("speed") * w_new) / (w_old + w_new);
            }
            f.t += w_new;
            f.n += g("n");
            f.rockets += g("rockets");
            f.alt += g("alt");
            f.takeoff_jump += fl["takeoff"]["jump"].as_f64().unwrap_or(0.0);
            f.takeoff_drop += fl["takeoff"]["drop"].as_f64().unwrap_or(0.0);
            f.land_soft += fl["land"]["soft"].as_f64().unwrap_or(0.0);
            f.land_hard += fl["land"]["hard"].as_f64().unwrap_or(0.0);
        }
        self.updated = now;
    }

    /// Abitudini per il bot: {armor, tools, gather, build, favorites, fly, base}.
    pub fn habits(&self) -> Value {
        // armatura: il materiale più frequente tra i pezzi indossati (l'elytra non conta)
        let mut mats: BTreeMap<&str, usize> = BTreeMap::new();
        for v in self.armor.values().filter(|v| v.as_str() != "elytra") {
            if let Some(m) = material(v) {
                *mats.entry(m).or_default() += 1;
            }
        }
        let armor = mats.iter().max_by_key(|(_, n)| **n).map(|(m, _)| m.to_string());
        let tools = top(&self.held, 4, |k| ["_pickaxe", "_axe", "_sword", "_shovel"].iter().any(|t| k.ends_with(t)));
        let gather = top(&self.broken, 3, natural);
        let build = top(&self.placed, 4, |b| !natural(b) || b == "dirt");
        let mut favorites: Vec<String> = build.clone();
        for t in top(&self.held, 3, |k| !k.ends_with("_sword")) {
            if !favorites.contains(&t) {
                favorites.push(t);
            }
        }
        let fly = (self.fly.n > 0.0).then(|| {
            json!({ "n": self.fly.n, "t": self.fly.t, "pitch": self.fly.pitch, "rockets": self.fly.rockets, "alt": if self.fly.n > 0.0 { self.fly.alt / self.fly.n } else { 0.0 },
                "takeoff": { "jump": self.fly.takeoff_jump, "drop": self.fly.takeoff_drop } })
        });
        let base = top(&self.places, 1, |_| true).first().and_then(|c| {
            let (x, z) = c.split_once(',')?;
            Some(json!([x.parse::<i64>().ok()? * 16 + 8, z.parse::<i64>().ok()? * 16 + 8]))
        });
        json!({ "armor": armor, "tools": tools, "gather": gather, "build": build, "favorites": favorites, "fly": fly, "base": base })
    }

    /// Riga in inglese per il prompt e il grafo (vuota se non c'è niente di notevole).
    pub fn line(&self, who: &str) -> String {
        let h = self.habits();
        let mut v: Vec<String> = Vec::new();
        if let Some(a) = h["armor"].as_str() {
            v.push(format!("wears {a} armor"));
        }
        if self.armor.values().any(|x| x == "elytra") || self.fly.n > 0.0 {
            let style = if self.fly.n > 0.0 {
                let take = if self.fly.takeoff_drop > self.fly.takeoff_jump { "takes off from heights" } else { "takes off with a jump" };
                let r = if self.fly.t > 0.0 && self.fly.rockets > 0.0 { format!(", a rocket every {:.0}s", (self.fly.t / self.fly.rockets).max(1.0)) } else { String::new() };
                format!(" ({take}{r})")
            } else {
                String::new()
            };
            v.push(format!("flies with an elytra{style}"));
        }
        let list = |k: &str| h[k].as_array().map(|a| a.iter().filter_map(Value::as_str).collect::<Vec<_>>().join(", ")).unwrap_or_default();
        let tools = list("tools");
        if !tools.is_empty() {
            v.push(format!("uses {tools}"));
        }
        let g = list("gather");
        if !g.is_empty() {
            v.push(format!("often gathers {g}"));
        }
        let b = list("build");
        if !b.is_empty() {
            v.push(format!("builds with {b}"));
        }
        let crafted = top(&self.crafted, 3, |_| true);
        if !crafted.is_empty() {
            v.push(format!("crafted {}", crafted.join(", ")));
        }
        if self.secs > 30.0 && self.sprint / self.secs > 0.4 {
            v.push("always sprints".into());
        }
        if self.secs > 30.0 && self.sneak / self.secs > 0.2 {
            v.push("sneaks a lot".into());
        }
        if let Some(base) = h["base"].as_array() {
            if self.secs > 120.0 {
                v.push(format!("spends most time near {} {}", base[0], base[1]));
            }
        }
        for (k, a) in &self.answers {
            if !a.text.is_empty() {
                v.push(format!("{k}: \"{}\"", a.text.chars().take(60).collect::<String>()));
            }
        }
        if v.is_empty() {
            return String::new();
        }
        format!("{who} {}", v.join("; "))
    }
}

// ---------- dubbi ----------

/// Domanda in chat per un dubbio del bot (`key`), nella lingua giusta. None = dubbio sconosciuto.
pub fn question(key: &str, info: &Value, en: bool) -> Option<String> {
    let list = |k: &str| info[k].as_array().map(|a| a.iter().filter_map(Value::as_str).take(3).collect::<Vec<_>>().join(", ")).unwrap_or_default();
    Some(match (key, en) {
        ("can_break", false) => format!("posso rompere i blocchi che ci sono in mezzo ({})? sì/no", list("blocks")),
        ("can_break", true) => format!("can i break the blocks in the way ({})? yes/no", list("blocks")),
        ("duel_rules", false) => "regole? fino a 3 cuori va bene, o preferisci al primo ko? (anche: senza arco, al meglio di 3…)".into(),
        ("duel_rules", true) => "rules? until 3 hearts ok, or first ko? (also: no bow, best of 3…)".into(),
        ("build_spot", false) => "dove la costruisco? qui davanti va bene, o vicino alla base?".into(),
        ("build_spot", true) => "where should i build it? right here ok, or near the base?".into(),
        ("base", false) => "dov'è la base? se ci sei vicino scrivi «qui»".into(),
        ("base", true) => "where's the base? if you're near it say \"here\"".into(),
        ("fight_style", false) => "nei duelli preferisci che usi lo scudo e l'arco, o solo spada?".into(),
        ("fight_style", true) => "in duels do you want me to use shield and bow, or sword only?".into(),
        _ => return None,
    })
}

/// "sì/no" a parole chiave (None = non è una risposta chiara).
pub fn yes_no(text: &str) -> Option<bool> {
    let t = format!(" {} ", text.to_lowercase().replace(['!', '?', '.', ',', ';'], " "));
    let yes = [" si ", " sì ", " ok ", " okay ", " va bene ", " certo ", " vai ", " yes ", " yep ", " yeah ", " sure ", " go ", " fai pure ", " puoi ", " rompi ", " break "];
    let no = [" no ", " nope ", " non ", " don't ", " dont ", " lascia ", " leave it ", " stop ", " nah "];
    let y = yes.iter().any(|w| t.contains(w));
    let n = no.iter().any(|w| t.contains(w));
    match (y, n) {
        (true, false) => Some(true),
        (false, true) => Some(false),
        _ => None,
    }
}

/// La risposta in chat a un dubbio → valore per il bot (zero token). None = non sembra una risposta.
pub fn interpret(key: &str, text: &str) -> Option<Value> {
    let low = text.to_lowercase();
    match key {
        "can_break" => yes_no(text).map(Value::Bool),
        // le regole sono testo: le capisce pvp.js (parseRules); "ok/va bene" = quelle proposte
        "duel_rules" => {
            if ["cuor", "heart", "ko", "colpo", "hit", "meglio", "best", "round", "senza", "no bow", "no rod", "arco", "perl"].iter().any(|w| low.contains(w)) {
                Some(Value::String(text.chars().take(120).collect()))
            } else {
                yes_no(text).filter(|y| *y).map(|_| Value::String(String::new()))
            }
        }
        "build_spot" => {
            if ["qui", "here", "va bene", "ok", "sì", "si ", "yes"].iter().any(|w| low.contains(w)) {
                Some(json!("here"))
            } else if ["base", "casa", "home"].iter().any(|w| low.contains(w)) {
                Some(json!("base"))
            } else {
                None
            }
        }
        "base" => ["qui", "here", "questa", "this"].iter().any(|w| low.contains(w)).then(|| json!("here")),
        "fight_style" => {
            if ["solo spada", "sword only", "just sword", "only sword", "niente scudo", "no shield"].iter().any(|w| low.contains(w)) {
                Some(json!("sword"))
            } else if ["scudo", "shield", "arco", "bow", "tutto", "everything", "sì", "yes"].iter().any(|w| low.contains(w)) {
                Some(json!("all"))
            } else {
                None
            }
        }
        _ => None,
    }
}

impl Store {
    /// Si può chiedere adesso? (non già risposto, non chiesto negli ultimi 10 minuti, al massimo una domanda ogni 90 s)
    pub fn may_ask(&self, who: &str, key: &str, now: i64) -> bool {
        let p = self.players.get(who);
        let answered = p.is_some_and(|p| p.answers.contains_key(key));
        let recent = p.and_then(|p| p.asked.get(key)).is_some_and(|t| now - t < 600);
        !answered && !recent && now - self.last_ask >= 90
    }

    pub fn mark_asked(&mut self, who: &str, key: &str, now: i64) {
        self.players.entry(who.to_string()).or_default().asked.insert(key.to_string(), now);
        self.last_ask = now;
    }

    pub fn answer(&self, who: &str, key: &str) -> Option<&Answer> {
        self.players.get(who).and_then(|p| p.answers.get(key))
    }

    pub fn store_answer(&mut self, who: &str, key: &str, value: Value, text: &str, now: i64) {
        self.players.entry(who.to_string()).or_default().answers.insert(key.to_string(), Answer { value, text: text.chars().take(120).collect(), at: now });
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

    #[test]
    fn profile_from_observations() {
        let mut p = Profile::default();
        let s = json!({
            "t": 30, "dist": 120, "sprint": 20, "sneak": 1, "air": 3, "jumps": 6,
            "held": { "iron_pickaxe": 12, "iron_sword": 8, "cobblestone": 4 },
            "armor": { "helmet": "iron_helmet", "chestplate": "elytra", "leggings": "iron_leggings", "boots": "diamond_boots" },
            "placed": { "stone_bricks": 30, "glass": 5 }, "broken": { "oak_log": 9, "stone": 20, "short_grass": 1 }, "crafted": { "iron_pickaxe": 1 },
            "places": { "12,-3": 25, "13,-3": 5 },
            "fly": { "n": 2, "t": 40, "pitch": -0.3, "speed": 25, "rockets": 8, "takeoff": { "jump": 0, "drop": 2 }, "land": { "soft": 2 }, "alt": 60 }
        });
        for _ in 0..5 {
            p.merge(&s, 10);
        }
        let h = p.habits();
        assert_eq!(h["armor"], "iron", "due pezzi di ferro, l'elytra non conta");
        assert_eq!(h["gather"][0], "stone");
        assert!(h["gather"].as_array().unwrap().iter().any(|x| x == "oak_log"));
        assert_eq!(h["build"][0], "stone_bricks");
        assert!(h["tools"].as_array().unwrap().iter().any(|x| x == "iron_pickaxe"));
        assert!(h["favorites"].as_array().unwrap().iter().any(|x| x == "stone_bricks"));
        assert_eq!(h["base"], json!([12 * 16 + 8, -3 * 16 + 8]));
        assert!((h["fly"]["pitch"].as_f64().unwrap() + 0.3).abs() < 1e-9 && h["fly"]["takeoff"]["drop"].as_f64().unwrap() > 0.0);
        let line = p.line("Makoto");
        for w in ["Makoto wears iron armor", "flies with an elytra (takes off from heights, a rocket every 5s)", "often gathers stone", "builds with stone_bricks", "always sprints", "crafted iron_pickaxe"] {
            assert!(line.contains(w), "{w}: {line}");
        }
        // oblio: le cose vecchie perdono peso
        let before = p.broken["stone"];
        p.merge(&json!({ "t": 30, "broken": {} }), 20);
        assert!(p.broken["stone"] < before);
        assert_eq!(Profile::default().line("X"), "");
    }

    #[test]
    fn doubts_asked_once_and_remembered() {
        let mut st = Store::default();
        assert!(st.may_ask("Makoto", "can_break", 1000));
        st.mark_asked("Makoto", "can_break", 1000);
        assert!(!st.may_ask("Makoto", "can_break", 1100), "appena chiesto");
        assert!(!st.may_ask("Makoto", "duel_rules", 1050), "niente domande di fila");
        assert!(st.may_ask("Makoto", "duel_rules", 1100));
        st.store_answer("Makoto", "can_break", json!(true), "sì rompi pure", 1200);
        assert!(!st.may_ask("Makoto", "can_break", 9999), "già risposto: non richiede");
        assert_eq!(st.answer("Makoto", "can_break").map(|a| a.value.clone()), Some(json!(true)));
        // risposte a parole chiave
        assert_eq!(interpret("can_break", "sì rompi pure"), Some(json!(true)));
        assert_eq!(interpret("can_break", "no lascia stare"), Some(json!(false)));
        assert_eq!(interpret("can_break", "boh vediamo"), None);
        assert_eq!(interpret("duel_rules", "al primo ko senza arco"), Some(json!("al primo ko senza arco")));
        assert_eq!(interpret("duel_rules", "ok va bene"), Some(json!("")));
        assert_eq!(interpret("build_spot", "qui va bene"), Some(json!("here")));
        assert_eq!(interpret("build_spot", "vicino alla base"), Some(json!("base")));
        assert_eq!(interpret("fight_style", "solo spada dai"), Some(json!("sword")));
        assert!(question("can_break", &json!({ "blocks": ["stone_bricks"] }), false).unwrap().contains("stone_bricks"));
        assert!(question("duel_rules", &json!({}), true).unwrap().starts_with("rules?"));
        assert!(question("boh", &json!({}), true).is_none());
        // le risposte finiscono nella riga del profilo
        assert!(st.players["Makoto"].line("Makoto").contains("can_break: \"sì rompi pure\""));
        let dir = std::env::temp_dir().join(format!("ae-mc-obs-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        save(&dir, &st);
        assert_eq!(load(&dir), st);
        forget(&dir);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
