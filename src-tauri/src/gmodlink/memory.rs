//! Memoria delle partite di Garry's Mod: un archivio separato (`gmod_memory.json`), fuori dalla
//! memoria principale di Alter Ego, così le partite non pesano sulle conversazioni normali.
//!
//! - Durante la partita si raccolgono gli eventi in RAM (chi l'ha colpita, scherzi, morti, cose
//!   costruite, giri in macchina, frasi rivolte a lei).
//! - A fine partita diventano UNA riga di riassunto, scritta senza modello AI (zero token).
//! - Si consulta solo quando serve: parole come "ti ricordi", "l'altra volta" (controllo
//!   algoritmico) oppure, per i casi dubbi, Rizzo Flow se è il router scelto; più una riga
//!   breve sull'ultima partita con gli stessi giocatori alla prima risposta della sessione.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

const FILE: &str = "gmod_memory.json";
const KEEP: usize = 60;

#[derive(Serialize, Deserialize, Default, Debug, Clone)]
#[serde(default)]
pub struct Store {
    pub sessions: Vec<Session>,
    pub players: BTreeMap<String, PlayerNote>,
}

#[derive(Serialize, Deserialize, Default, Debug, Clone)]
#[serde(default)]
pub struct Session {
    pub start: i64,
    pub end: i64,
    pub map: String,
    pub players: Vec<String>,
    pub line: String,
}

#[derive(Serialize, Deserialize, Default, Debug, Clone)]
#[serde(default)]
pub struct PlayerNote {
    pub games: u32,
    pub last: i64,
    pub hit_you: u32,
    pub killed_you: u32,
    pub pranked: u32,
    pub threw_at_you: u32,
}

pub fn path(data_dir: &Path) -> PathBuf {
    data_dir.join(FILE)
}

pub fn load(data_dir: &Path) -> Store {
    std::fs::read_to_string(path(data_dir)).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
}

fn save(data_dir: &Path, st: &Store) {
    if let Ok(s) = serde_json::to_string(st) {
        let tmp = data_dir.join(format!("{FILE}.tmp"));
        if std::fs::write(&tmp, s).is_ok() {
            let _ = std::fs::rename(&tmp, path(data_dir));
        }
    }
}

pub fn forget(data_dir: &Path) {
    let _ = std::fs::remove_file(path(data_dir));
}

/// Registro della partita in corso.
#[derive(Default, Debug, Clone)]
pub struct Log {
    pub start: i64,
    pub map: String,
    pub players: BTreeSet<String>,
    hurt_by: BTreeMap<String, u32>,
    killed_by: BTreeMap<String, u32>,
    deaths: u32,
    pranked: BTreeMap<String, u32>,
    /// Chi le ha lanciato roba addosso (e quante volte l'ha presa al volo).
    threw: BTreeMap<String, u32>,
    caught: u32,
    spawned: BTreeMap<String, u32>,
    tasks: Vec<String>,
    said: Vec<String>,
    rides: u32,
    /// Il riassunto dell'ultima volta è già stato dato al modello.
    pub recalled: bool,
}

impl Log {
    pub fn new(start: i64, map: &str) -> Self {
        Self { start, map: map.to_string(), ..Default::default() }
    }

    pub fn event(&mut self, e: &Value) {
        let s = |k: &str| e[k].as_str().unwrap_or("").to_string();
        let bump = |m: &mut BTreeMap<String, u32>, k: String| *m.entry(k).or_insert(0) += 1;
        match e["kind"].as_str().unwrap_or("") {
            "hurt" => {
                let by = s("by");
                if self.players.contains(&by) {
                    bump(&mut self.hurt_by, by);
                }
            }
            "died" => {
                self.deaths += 1;
                let by = s("by");
                if self.players.contains(&by) {
                    bump(&mut self.killed_by, by);
                }
            }
            "prank" => bump(&mut self.pranked, s("who")),
            "thrown" => {
                let by = s("by");
                if self.players.contains(&by) {
                    bump(&mut self.threw, by);
                }
                if s("how") == "caught" {
                    self.caught += 1;
                }
            }
            "spawned" if s("who").is_empty() => {}
            "task" if s("status") == "running" => {
                let n = s("name");
                if !self.tasks.contains(&n) && self.tasks.len() < 5 {
                    self.tasks.push(n);
                }
            }
            "ride" => self.rides += 1,
            _ => {}
        }
    }

    pub fn spawned(&mut self, what: &str) {
        *self.spawned.entry(what.to_string()).or_insert(0) += 1;
    }

    /// Frase di un giocatore rivolta a lei (le prime, accorciate).
    pub fn said(&mut self, from: &str, text: &str) {
        if self.said.len() < 4 {
            self.said.push(format!("{from}: \"{}\"", text.chars().take(60).collect::<String>()));
        }
    }

    fn has_content(&self) -> bool {
        self.deaths > 0 || !self.hurt_by.is_empty() || !self.threw.is_empty() || !self.pranked.is_empty() || !self.spawned.is_empty() || !self.tasks.is_empty() || !self.said.is_empty() || self.rides > 0
    }

    /// Riassunto in una riga (inglese: è per il modello).
    pub fn summary(&self, end: i64) -> String {
        let date = chrono_date(self.start);
        let mins = ((end - self.start).max(0) / 60).max(1);
        let who: Vec<&String> = self.players.iter().collect();
        let mut parts = vec![format!(
            "{date} on {} with {} ({mins} min)",
            if self.map.is_empty() { "a map" } else { &self.map },
            who.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", ")
        )];
        for (p, n) in &self.hurt_by {
            parts.push(format!("{p} hit you {n}x"));
        }
        for (p, n) in &self.threw {
            parts.push(format!("{p} threw stuff at you {n}x"));
        }
        if self.caught > 0 {
            parts.push(format!("you caught {}x with the physgun", self.caught));
        }
        if self.deaths > 0 {
            let by: Vec<String> = self.killed_by.iter().map(|(p, n)| format!("{p} {n}x")).collect();
            parts.push(if by.is_empty() { format!("you died {}x", self.deaths) } else { format!("you died {}x (killed by {})", self.deaths, by.join(", ")) });
        }
        for (p, n) in &self.pranked {
            parts.push(format!("you pranked {p} {n}x"));
        }
        if !self.spawned.is_empty() {
            let mut v: Vec<(&String, &u32)> = self.spawned.iter().collect();
            v.sort_by(|a, b| b.1.cmp(a.1));
            parts.push(format!("spawned {}", v.iter().take(4).map(|(k, _)| k.as_str()).collect::<Vec<_>>().join(", ")));
        }
        if self.rides > 0 {
            parts.push(format!("rode in a vehicle {}x", self.rides));
        }
        if !self.tasks.is_empty() {
            parts.push(format!("scripts: {}", self.tasks.join(", ")));
        }
        if !self.said.is_empty() {
            parts.push(format!("talk: {}", self.said.join(" ")));
        }
        let line = parts.join("; ");
        line.chars().take(420).collect()
    }

    /// Chiude la partita: riga nell'archivio e note per giocatore. false se non c'era niente da ricordare.
    pub fn finish(&self, data_dir: &Path, end: i64) -> bool {
        if !self.has_content() || end - self.start < 90 {
            return false;
        }
        let mut st = load(data_dir);
        for p in &self.players {
            let n = st.players.entry(p.clone()).or_default();
            n.games += 1;
            n.last = end;
            n.hit_you += self.hurt_by.get(p).copied().unwrap_or(0);
            n.killed_you += self.killed_by.get(p).copied().unwrap_or(0);
            n.pranked += self.pranked.get(p).copied().unwrap_or(0);
            n.threw_at_you += self.threw.get(p).copied().unwrap_or(0);
        }
        st.sessions.push(Session { start: self.start, end, map: self.map.clone(), players: self.players.iter().cloned().collect(), line: self.summary(end) });
        let n = st.sessions.len();
        if n > KEEP {
            st.sessions.drain(..n - KEEP);
        }
        save(data_dir, &st);
        true
    }
}

/// "2026-11-17" da un timestamp Unix (UTC, senza dipendenze).
fn chrono_date(ts: i64) -> String {
    let days = ts.div_euclid(86_400);
    // algoritmo di Howard Hinnant (civil_from_days)
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    format!("{:04}-{:02}-{:02}", if m <= 2 { y + 1 } else { y }, m, d)
}

// ---------- quando consultarla ----------

/// Parole che chiedono sicuramente un ricordo.
const STRONG: &[&str] = &[
    "ti ricordi", "ricordi quando", "ricordi la", "ricordi il", "te lo ricordi", "l'altra volta", "altra volta", "la scorsa", "lo scorso",
    "ieri", "l'ultima volta", "ultima partita", "remember", "last time", "last game", "yesterday", "the other day", "do you recall",
];
/// Parole che potrebbero chiederlo (con Rizzo Flow si chiede a lui, altrimenti si consulta).
const WEAK: &[&str] = &["volta", "prima", "sempre", "mai ", "di nuovo", "ancora", "again", "before", "ever", "always", "never", "time we"];

#[derive(Debug, PartialEq)]
pub enum Need {
    Yes,
    Maybe,
    No,
}

pub fn need(text: &str) -> Need {
    let t = format!(" {} ", text.to_lowercase());
    if STRONG.iter().any(|w| t.contains(w)) {
        Need::Yes
    } else if WEAK.iter().any(|w| t.contains(w)) {
        Need::Maybe
    } else {
        Need::No
    }
}

/// Rizzo Flow decide nei casi dubbi (~50 ms, zero token). None se non risponde.
pub async fn rizzo_wants(url: &str, text: &str) -> Option<bool> {
    let body = json!({
        "model": "rizzo-latest",
        "state": { "user_message": text },
        "questions": {
            "past_games": {
                "type": "noul",
                "instructions": "In a video game chat, is the player referring to earlier game sessions, past events or shared memories with the AI companion (things that happened before today's match)?"
            }
        }
    });
    let endpoint = format!("{}/v1/systemone", url.trim().trim_end_matches('/'));
    let resp = reqwest::Client::new().post(endpoint).timeout(std::time::Duration::from_millis(1500)).json(&body).send().await.ok()?;
    let v: Value = resp.json().await.ok()?;
    v.pointer("/answers/past_games/noul").and_then(Value::as_f64).map(|p| p >= 0.5)
}

/// Ricordi utili adesso: le partite con chi è presente (o le ultime), più le note sui giocatori.
pub fn recall(st: &Store, present: &[String], max: usize) -> String {
    let mut scored: Vec<(usize, &Session)> = st.sessions.iter().map(|s| (s.players.iter().filter(|p| present.contains(p)).count(), s)).collect();
    // prima chi c'è adesso, poi le più recenti
    scored.sort_by(|a, b| b.0.cmp(&a.0).then(b.1.end.cmp(&a.1.end)));
    let mut lines: Vec<String> = scored.iter().take(max).map(|(_, s)| format!("- {}", s.line)).collect();
    for p in present {
        if let Some(n) = st.players.get(p) {
            if n.games > 0 {
                lines.push(format!("- {p}: {} games together, hit you {}x, threw stuff at you {}x, killed you {}x, you pranked them {}x", n.games, n.hit_you, n.threw_at_you, n.killed_you, n.pranked));
            }
        }
    }
    if lines.is_empty() {
        return String::new();
    }
    format!("[memories of past games]\n{}", lines.join("\n"))
}

/// Una riga sull'ultima partita con questi giocatori (per la prima risposta della sessione).
pub fn brief(st: &Store, present: &[String]) -> String {
    st.sessions
        .iter()
        .rev()
        .find(|s| s.players.iter().any(|p| present.contains(p)))
        .map(|s| format!("[last game together] {}", s.line.chars().take(200).collect::<String>()))
        .unwrap_or_default()
}

/// Riassunto per le impostazioni: numero di partite ricordate.
pub fn count(data_dir: &Path) -> usize {
    load(data_dir).sessions.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_summary_and_recall() {
        let dir = std::env::temp_dir().join(format!("ae-gmem-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        forget(&dir);
        let start = 1_763_337_600; // 2025-11-17
        let mut log = Log::new(start, "gm_construct");
        log.players.insert("Makoto".into());
        log.players.insert("Kyoko".into());
        for e in [
            json!({ "kind": "hurt", "by": "Makoto", "dmg": 10 }),
            json!({ "kind": "hurt", "by": "Makoto", "dmg": 10 }),
            json!({ "kind": "hurt", "by": "zombie", "dmg": 10 }),
            json!({ "kind": "died", "by": "Makoto" }),
            json!({ "kind": "prank", "who": "Makoto", "kind2": "boom" }),
            json!({ "kind": "task", "name": "hunt", "status": "running" }),
        ] {
            log.event(&e);
        }
        log.spawned("crate");
        log.spawned("crate");
        log.spawned("jeep");
        log.event(&json!({ "kind": "ride" }));
        log.said("Makoto", "costruiamo una torre altissima");
        let line = log.summary(start + 35 * 60);
        assert!(line.starts_with("2025-11-17 on gm_construct with Kyoko, Makoto (35 min)"), "{line}");
        for w in ["Makoto hit you 2x", "you died 1x (killed by Makoto 1x)", "you pranked Makoto 1x", "spawned crate, jeep", "rode in a vehicle 1x", "scripts: hunt", "costruiamo"] {
            assert!(line.contains(w), "{w} in {line}");
        }
        assert!(!line.contains("zombie"));
        // partita troppo corta o vuota: niente
        assert!(!Log::new(start, "x").finish(&dir, start + 3600));
        assert!(!log.finish(&dir, start + 30));
        assert!(log.finish(&dir, start + 35 * 60));
        let st = load(&dir);
        assert_eq!(st.sessions.len(), 1);
        assert_eq!(st.players["Makoto"].games, 1);
        assert_eq!(st.players["Makoto"].killed_you, 1);
        // consultazione
        let r = recall(&st, &["Makoto".into()], 3);
        assert!(r.starts_with("[memories of past games]") && r.contains("gm_construct") && r.contains("Makoto: 1 games"), "{r}");
        assert!(brief(&st, &["Kyoko".into()]).starts_with("[last game together] 2025-11-17"));
        assert_eq!(brief(&st, &["Byakuya".into()]), "");
        forget(&dir);
        assert_eq!(count(&dir), 0);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn when_to_look() {
        assert_eq!(need("Ti ricordi quando hai fatto esplodere la jeep?"), Need::Yes);
        assert_eq!(need("remember last time?"), Need::Yes);
        assert_eq!(need("facciamolo di nuovo"), Need::Maybe);
        assert_eq!(need("spawna una cassa"), Need::No);
        assert_eq!(chrono_date(0), "1970-01-01");
        assert_eq!(chrono_date(1_709_164_800), "2024-02-29");
    }
}
