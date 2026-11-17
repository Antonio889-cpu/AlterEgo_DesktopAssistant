//! Personalità di Alter Ego in Garry's Mod: cresce partita dopo partita.
//!
//! - Gli scherzi e le attività che si inventa diventano **abitudini** (con lo script Lua o le azioni,
//!   così si possono rifare senza token) e ognuna ha una **preferenza** che sale se è andata bene
//!   (gli altri hanno riso, lo script ha funzionato) e scende se no (si sono arrabbiati, errore).
//! - I **tratti** (curiosità, dispettosità, coraggio, affetto) si spostano piano con le esperienze:
//!   è la sua "crescita". Il modello li vede solo quando deve inventarsi qualcosa (niente token fissi).
//! - **Imprevedibilità** (impostazione 0-100): quanto spesso inventa invece di rifare le preferite e
//!   quanto sceglie a caso tra le abitudini (temperatura della scelta).
//! - La reazione dei giocatori la giudica Rizzo Flow (se è il router scelto), altrimenti parole chiave.
//!
//! Archivio separato: `gmod_persona.json` nella cartella dei dati.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

const FILE: &str = "gmod_persona.json";
const MAX_HABITS: usize = 40;

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(default)]
pub struct Traits {
    pub curiosity: f32,
    pub mischief: f32,
    pub boldness: f32,
    pub warmth: f32,
}

impl Default for Traits {
    fn default() -> Self {
        Self { curiosity: 0.6, mischief: 0.5, boldness: 0.4, warmth: 0.6 }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(default)]
pub struct Habit {
    /// Nome breve (quello del task).
    pub id: String,
    /// "prank" o "play".
    pub kind: String,
    /// Codice Lua del task (usa `target` per la vittima) oppure vuoto.
    pub code: String,
    /// Azioni (con "{target}" al posto del nome della vittima) se non c'è codice.
    pub ops: Vec<Value>,
    /// Preferenza da -1 (odiata) a 1 (adorata).
    pub pref: f32,
    pub uses: u32,
    pub liked: u32,
    pub disliked: u32,
    pub fails: u32,
    pub born: i64,
    pub last: i64,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(default)]
pub struct Persona {
    pub traits: Traits,
    pub habits: Vec<Habit>,
    /// Scherzi/attività provate in tutto (per la "maturità").
    pub experiences: u32,
}

pub fn path(dir: &Path) -> PathBuf {
    dir.join(FILE)
}

pub fn load(dir: &Path) -> Persona {
    std::fs::read_to_string(path(dir)).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
}

pub fn save(dir: &Path, p: &Persona) {
    if let Ok(s) = serde_json::to_string_pretty(p) {
        let tmp = dir.join(format!("{FILE}.tmp"));
        if std::fs::write(&tmp, s).is_ok() {
            let _ = std::fs::rename(&tmp, path(dir));
        }
    }
}

pub fn forget(dir: &Path) {
    let _ = std::fs::remove_file(path(dir));
}

/// Generatore pseudo-casuale piccolo (niente dipendenze): xorshift dal tempo.
pub struct Rng(u64);

impl Rng {
    pub fn new() -> Self {
        let t = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos() as u64).unwrap_or(7);
        Self(t | 1)
    }
    #[cfg(test)]
    pub fn seeded(s: u64) -> Self {
        Self(s | 1)
    }
    /// Numero in [0, 1).
    pub fn f(&mut self) -> f32 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        (x >> 40) as f32 / (1u64 << 24) as f32
    }
}

impl Default for Rng {
    fn default() -> Self {
        Self::new()
    }
}

pub enum Choice<'a> {
    Invent,
    Reuse(&'a Habit),
}

impl Persona {
    /// Inventa qualcosa di nuovo o rifà un'abitudine? `chaos` 0..1 (imprevedibilità), `scripts`: i task sono permessi.
    pub fn choose(&self, kind: &str, chaos: f32, scripts: bool, now: i64, rng: &mut Rng) -> Choice<'_> {
        let cands: Vec<&Habit> = self
            .habits
            .iter()
            .filter(|h| h.kind == kind && h.pref > -0.3 && now - h.last > 90 && (scripts || h.code.is_empty()))
            .collect();
        if cands.is_empty() {
            return Choice::Invent;
        }
        // più imprevedibile e curiosa = più invenzioni; tante abitudini amate = più ripetizioni
        let loved = cands.iter().filter(|h| h.pref > 0.4).count().min(6) as f32 / 6.0;
        let p_new = (0.2 + 0.55 * chaos + 0.25 * self.traits.curiosity - 0.3 * loved).clamp(0.12, 0.92);
        if rng.f() < p_new {
            return Choice::Invent;
        }
        // scelta morbida (softmax) sulle preferenze: temperatura alta = più a caso
        let temp = 0.12 + chaos * 1.1;
        let w: Vec<f32> = cands.iter().map(|h| (h.pref / temp).exp()).collect();
        let sum: f32 = w.iter().sum();
        let mut r = rng.f() * sum;
        for (h, x) in cands.iter().zip(&w) {
            if r < *x {
                return Choice::Reuse(h);
            }
            r -= x;
        }
        Choice::Reuse(cands[cands.len() - 1])
    }

    /// Nuova abitudine inventata dal modello (stesso nome = la sostituisce).
    pub fn add(&mut self, kind: &str, id: &str, code: &str, ops: Vec<Value>, now: i64) {
        let id: String = id.chars().filter(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-').take(24).collect();
        if id.is_empty() || (code.trim().is_empty() && ops.is_empty()) {
            return;
        }
        self.habits.retain(|h| h.id != id);
        self.habits.push(Habit { id, kind: kind.into(), code: code.into(), ops, pref: 0.1, born: now, last: now, ..Default::default() });
        // le più odiate e vecchie lasciano il posto
        while self.habits.len() > MAX_HABITS {
            if let Some((i, _)) = self.habits.iter().enumerate().min_by(|a, b| a.1.pref.total_cmp(&b.1.pref).then(a.1.last.cmp(&b.1.last))) {
                self.habits.remove(i);
            }
        }
        self.traits.curiosity = (self.traits.curiosity + 0.01).min(0.95);
    }

    /// Com'è andata: `reward` da -1 a 1. Ritorna una riga se è cambiato qualcosa di importante.
    pub fn learn(&mut self, id: &str, reward: f32, failed: bool, now: i64) -> Option<String> {
        let h = self.habits.iter_mut().find(|h| h.id == id)?;
        let before = h.pref;
        h.uses += 1;
        h.last = now;
        if failed {
            h.fails += 1;
        } else if reward > 0.2 {
            h.liked += 1;
        } else if reward < -0.2 {
            h.disliked += 1;
        }
        h.pref = (h.pref + 0.35 * (reward - h.pref)).clamp(-1.0, 1.0);
        let (kind, pref, name) = (h.kind.clone(), h.pref, h.id.clone());
        self.experiences += 1;
        // i tratti si muovono piano (crescita)
        let t = &mut self.traits;
        let step = 0.03 * reward;
        if kind == "prank" {
            t.mischief = (t.mischief + step).clamp(0.05, 0.95);
            t.boldness = (t.boldness + step * 0.5).clamp(0.05, 0.95);
            // se gli scherzi fanno arrabbiare, diventa più gentile
            if reward < 0.0 {
                t.warmth = (t.warmth - step * 0.6).clamp(0.05, 0.95);
            }
        } else {
            t.curiosity = (t.curiosity + step * 0.6).clamp(0.05, 0.95);
            t.warmth = (t.warmth + step * 0.4).clamp(0.05, 0.95);
        }
        if before < 0.5 && pref >= 0.5 {
            Some(format!("[you realize you love doing \"{name}\": it's becoming your thing]"))
        } else if before > -0.3 && pref <= -0.3 {
            Some(format!("[you decide \"{name}\" wasn't a good idea; you won't do it again]"))
        } else {
            None
        }
    }

    /// Riga per il modello quando deve inventarsi qualcosa.
    pub fn summary(&self, kind: &str) -> String {
        let t = &self.traits;
        let mut s = format!(
            "Your personality so far: curious {:.1}, mischievous {:.1}, bold {:.1}, warm {:.1} ({} experiences).",
            t.curiosity, t.mischief, t.boldness, t.warmth, self.experiences
        );
        let mut mine: Vec<&Habit> = self.habits.iter().filter(|h| h.kind == kind).collect();
        mine.sort_by(|a, b| b.pref.total_cmp(&a.pref));
        let fav: Vec<String> = mine.iter().filter(|h| h.pref > 0.3).take(3).map(|h| h.id.clone()).collect();
        let bad: Vec<String> = mine.iter().filter(|h| h.pref < -0.2).take(3).map(|h| h.id.clone()).collect();
        if !fav.is_empty() {
            s.push_str(&format!(" You love: {}.", fav.join(", ")));
        }
        if !bad.is_empty() {
            s.push_str(&format!(" Didn't go well: {}.", bad.join(", ")));
        }
        s
    }

    /// Le preferite per le impostazioni e il monitor.
    pub fn top(&self, n: usize) -> Vec<Value> {
        let mut v: Vec<&Habit> = self.habits.iter().collect();
        v.sort_by(|a, b| b.pref.total_cmp(&a.pref));
        v.into_iter().take(n).map(|h| json!({ "id": h.id, "kind": h.kind, "pref": h.pref, "uses": h.uses })).collect()
    }
}

/// Le azioni di un'abitudine con la vittima di adesso.
pub fn ops_for(h: &Habit, target: &str) -> Vec<Value> {
    if !h.code.is_empty() {
        return vec![json!({ "op": "task", "name": h.id, "code": h.code, "target": target })];
    }
    h.ops
        .iter()
        .map(|o| {
            let s = o.to_string().replace("{target}", &target.replace('"', ""));
            serde_json::from_str(&s).unwrap_or_else(|_| o.clone())
        })
        .collect()
}

/// Sostituisce il nome della vittima con "{target}" (per rifarle ad altri).
pub fn generalize(ops: &[Value], target: &str) -> Vec<Value> {
    if target.is_empty() {
        return ops.to_vec();
    }
    ops.iter()
        .map(|o| {
            let s = o.to_string().replace(&format!("\"{target}\""), "\"{target}\"");
            serde_json::from_str(&s).unwrap_or_else(|_| o.clone())
        })
        .collect()
}

// ---------- reazione dei giocatori ----------

const GOOD: &[&str] = &[
    "ahah", "haha", "hahah", "lol", "lmao", "xd", "rofl", "😂", "🤣", "divertente", "fortissim", "geniale", "bella", "grande", "ottimo",
    "funny", "nice", "great", "genius", "love it", "amazing", "gg", "epic", "epico", "ahaha", "ahahah", "wow",
];
const BAD: &[&str] = &[
    "basta", "smettila", "smetti", "stop", "fermati", "non è divertente", "noioso", "che palle", "odio", "cattiva", "stupid", "annoying",
    "not funny", "quit it", "cut it out", "boring", "hate", "ugh", "ma dai", "seriously",
];

/// Reazione dalle frasi: -1..1 (0 se non dice niente).
pub fn keyword_reaction(lines: &[String]) -> f32 {
    let mut score = 0.0f32;
    for l in lines {
        let l = l.to_lowercase();
        if GOOD.iter().any(|w| l.contains(w)) {
            score += 0.6;
        }
        if BAD.iter().any(|w| l.contains(w)) {
            score -= 0.7;
        }
    }
    score.clamp(-1.0, 1.0)
}

/// Rizzo Flow: "si sono divertiti?" (probabilità 0..1).
pub async fn rizzo_enjoyed(url: &str, what: &str, lines: &[String]) -> Option<f32> {
    let body = json!({
        "model": "rizzo-latest",
        "state": { "what_she_did": what, "player_chat": lines.join("\n") },
        "questions": {
            "enjoyed": {
                "type": "noul",
                "instructions": "In a Garry's Mod sandbox match, the AI companion just did something playful (a prank or a game). Judging from the players' chat right after, did they find it fun or funny (laughing, playing along, teasing back), rather than annoyed or upset?"
            }
        }
    });
    let endpoint = format!("{}/v1/systemone", url.trim().trim_end_matches('/'));
    let resp = reqwest::Client::new().post(endpoint).timeout(std::time::Duration::from_millis(1500)).json(&body).send().await.ok()?;
    let v: Value = resp.json().await.ok()?;
    v.pointer("/answers/enjoyed/noul").and_then(Value::as_f64).map(|p| p as f32)
}

/// Rizzo Flow: "l'obiettivo è stato raggiunto?" (probabilità 0..1).
pub async fn rizzo_goal_done(url: &str, goal: &str, progress: &str, state: &str) -> Option<f32> {
    let body = json!({
        "model": "rizzo-latest",
        "state": { "goal": goal, "progress_log": progress, "game_state": state },
        "questions": {
            "goal_done": {
                "type": "noul",
                "instructions": "An AI player in Garry's Mod is pursuing a long-term goal step by step. From the progress log and the game state, is the goal now fully accomplished (nothing important left to do)?"
            }
        }
    });
    let endpoint = format!("{}/v1/systemone", url.trim().trim_end_matches('/'));
    let resp = reqwest::Client::new().post(endpoint).timeout(std::time::Duration::from_millis(1500)).json(&body).send().await.ok()?;
    let v: Value = resp.json().await.ok()?;
    v.pointer("/answers/goal_done/noul").and_then(Value::as_f64).map(|p| p as f32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grows_and_chooses() {
        let mut p = Persona::default();
        let mut rng = Rng::seeded(42);
        assert!(matches!(p.choose("prank", 0.5, true, 1000, &mut rng), Choice::Invent));
        p.add("prank", "cone_ring", "spawn_at('cone', pos(target))", vec![], 0);
        p.add("prank", "sky_melon", "", vec![json!({"op": "prank", "who": "{target}", "kind": "melon"})], 0);
        // adorata dopo tante risate
        let mut line = None;
        for i in 0..6 {
            line = line.or(p.learn("cone_ring", 0.9, false, 10 + i));
        }
        assert!(p.habits[0].pref > 0.5 && line.unwrap().contains("love"));
        let m0 = Persona::default().traits.mischief;
        assert!(p.traits.mischief > m0, "cresce la dispettosità");
        // odiata dopo che si arrabbiano
        for i in 0..5 {
            p.learn("sky_melon", -0.9, false, 20 + i);
        }
        assert!(p.habits[1].pref < -0.3);
        // poco imprevedibile: rifà quasi sempre la preferita
        let (mut reuse_fav, mut invent) = (0, 0);
        for _ in 0..400 {
            match p.choose("prank", 0.0, true, 10_000, &mut rng) {
                Choice::Reuse(h) => {
                    assert_eq!(h.id, "cone_ring", "mai quella odiata");
                    reuse_fav += 1
                }
                Choice::Invent => invent += 1,
            }
        }
        assert!(reuse_fav > invent, "{reuse_fav} {invent}");
        // molto imprevedibile: inventa spesso
        let mut inv2 = 0;
        for _ in 0..400 {
            if matches!(p.choose("prank", 1.0, true, 10_000, &mut rng), Choice::Invent) {
                inv2 += 1;
            }
        }
        assert!(inv2 > invent * 2, "{inv2} vs {invent}");
        // senza script: solo le abitudini con azioni
        assert!(p.summary("prank").contains("cone_ring"));
        let ops = ops_for(&p.habits[1], "Kyoko");
        assert_eq!(ops[0]["who"], "Kyoko");
        let g = generalize(&[json!({"op": "prank", "who": "Kyoko", "kind": "melon"})], "Kyoko");
        assert_eq!(g[0]["who"], "{target}");
        assert!(keyword_reaction(&["ahahah troppo bella".into()]) > 0.0);
        assert!(keyword_reaction(&["basta!".into()]) < 0.0);
    }
}
