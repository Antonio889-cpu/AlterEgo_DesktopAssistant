//! Profili di combattimento dei duelli in Minecraft (minecraft_combat.json nei dati dell'app), tra una
//! sessione e l'altra. Dopo ogni round (evento `duel_round` del bot, vedi mc_bot/pvp.js):
//! - lo stile dell'avversario (distanza, strafe, critici, w-tap, scudo, arco…) si aggiorna con una media mobile;
//! - la tattica usata prende il suo premio (vittoria + differenza di danno): tasso di successo per tattica;
//! - se l'avversario ha vinto, Alter Ego copia la sua distanza e il suo strafe (`mimic`).
//! La tattica del round dopo: UCB1 (sfruttamento + esplorazione, tipo bandit) + counter sullo stile;
//! con Rizzo Flow una sola chiamata decide tattica, se copiare e se chiedere al giocatore.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

const FILE: &str = "minecraft_combat.json";

/// Tattiche (stessi nomi di mc_bot/pvp.js) con la descrizione per Rizzo Flow.
pub const TACTICS: &[(&str, &str)] = &[
    ("rush", "stay close and spam jump crits"),
    ("spacing", "keep max reach, hit as they step in, w-tap"),
    ("strafe", "circle strafe, hard to hit"),
    ("shield", "block with the shield, counter-hit"),
    ("kite", "keep away and shoot with the bow"),
    ("axe", "axe to disable the shield, then crits"),
];

#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq)]
#[serde(default)]
pub struct Stat {
    pub n: u32,
    pub reward: f64,
    pub wins: u32,
}

/// Stile dell'avversario (media mobile dei round).
#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq)]
#[serde(default)]
pub struct Style {
    pub pref_dist: f64,
    pub strafe: f64,
    pub cps: f64,
    pub crit: f64,
    pub wtap: f64,
    pub shield: f64,
    pub bow: f64,
    pub rod: f64,
    pub eat: f64,
    pub pearls: f64,
    pub flee: f64,
    pub aggro: f64,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq)]
#[serde(default)]
pub struct Mimic {
    pub dist: f64,
    pub strafe: f64,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq)]
#[serde(default)]
pub struct Opponent {
    pub rounds: u32,
    pub wins: u32,
    pub losses: u32,
    pub style: Style,
    pub tactics: BTreeMap<String, Stat>,
    pub mimic: Option<Mimic>,
    /// Regole preferite (testo detto dal giocatore).
    pub rules: String,
    pub last: i64,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq)]
#[serde(default)]
pub struct Store {
    pub opponents: BTreeMap<String, Opponent>,
}

/// Premio di un round (0..1), come `reward` in pvp.js.
pub fn reward(won: Option<bool>, dealt: f64, taken: f64) -> f64 {
    let diff = (dealt - taken) / (dealt + taken).max(4.0);
    ((if won == Some(true) { 0.65 } else { 0.1 }) + diff * 0.35).clamp(0.0, 1.0)
}

fn f(v: &Value, k: &str) -> f64 {
    v[k].as_f64().unwrap_or(0.0)
}

impl Style {
    /// Media mobile: i round nuovi pesano `a`.
    fn blend(&mut self, s: &Value, a: f64, first: bool) {
        let a = if first { 1.0 } else { a };
        let mix = |old: &mut f64, new: f64| *old = *old * (1.0 - a) + new * a;
        mix(&mut self.pref_dist, f(s, "prefDist"));
        mix(&mut self.strafe, f(s, "strafe"));
        mix(&mut self.cps, f(s, "cps"));
        mix(&mut self.crit, f(s, "crit"));
        mix(&mut self.wtap, f(s, "wtap"));
        mix(&mut self.shield, f(s, "shield"));
        mix(&mut self.bow, f(s, "bow"));
        mix(&mut self.rod, f(s, "rod"));
        mix(&mut self.eat, f(s, "eat"));
        mix(&mut self.pearls, f(s, "pearls"));
        mix(&mut self.flee, f(s, "flee"));
        mix(&mut self.aggro, f(s, "aggro"));
    }

    /// "close crits, strafes a lot" (vuoto se non c'è niente di notevole).
    pub fn line(&self) -> String {
        let mut v = Vec::new();
        if self.pref_dist > 0.0 && self.pref_dist < 2.6 {
            v.push("fights very close");
        } else if self.pref_dist > 3.3 {
            v.push("keeps distance");
        }
        if self.crit > 0.3 {
            v.push("jump crits");
        }
        if self.strafe > 0.6 {
            v.push("strafes a lot");
        }
        if self.wtap > 0.3 {
            v.push("w-taps");
        }
        if self.shield > 0.15 {
            v.push("uses the shield");
        }
        if self.bow > 0.15 {
            v.push("uses the bow");
        }
        if self.rod > 0.5 {
            v.push("uses the fishing rod");
        }
        if self.pearls > 0.5 {
            v.push("throws pearls");
        }
        if self.flee > 0.1 {
            v.push("runs when low");
        }
        v.join(", ")
    }
}

/// Counter sullo stile (come `counters` in pvp.js).
pub fn counters(st: &Style, rounds: u32, have: &Have) -> BTreeMap<&'static str, f64> {
    let mut b: BTreeMap<&'static str, f64> = TACTICS.iter().map(|(k, _)| (*k, 0.0)).collect();
    if rounds > 0 {
        let mut add = |k: &'static str, v: f64| *b.get_mut(k).unwrap() += v;
        if st.crit > 0.3 {
            add("shield", 0.35);
            add("spacing", 0.2);
        }
        if (st.pref_dist > 0.0 && st.pref_dist < 2.6) || st.cps > 4.0 {
            add("spacing", 0.3);
            add("strafe", 0.15);
        }
        if st.bow > 0.15 {
            add("rush", 0.35);
            add("strafe", 0.15);
        }
        if st.shield > 0.15 {
            add("axe", 0.4);
            add("rush", -0.1);
        }
        if st.strafe > 0.6 {
            add("strafe", 0.15);
            add("spacing", 0.1);
        }
        if st.flee > 0.1 || st.aggro < 0.2 {
            add("rush", 0.25);
        }
    }
    if !have.shield {
        b.insert("shield", -9.0);
    }
    if !have.bow {
        b.insert("kite", -9.0);
    }
    if !have.axe {
        b.insert("axe", -9.0);
    }
    b
}

/// Cosa ha in inventario per le tattiche.
#[derive(Debug, Clone, Default)]
pub struct Have {
    pub shield: bool,
    pub bow: bool,
    pub axe: bool,
}

impl Have {
    pub fn from_inv(inv: &str) -> Self {
        Self { shield: inv.contains("shield"), bow: inv.contains("bow") && inv.contains("arrow"), axe: inv.contains("_axe") }
    }
}

/// UCB1 + counter (rng in 0..1: sotto `eps` prova una tattica a caso).
pub fn choose(o: &Opponent, have: &Have, rng: f64, eps: f64) -> String {
    let c = counters(&o.style, o.rounds, have);
    let avail: Vec<&str> = TACTICS.iter().map(|(k, _)| *k).filter(|k| c[k] > -5.0).collect();
    if rng < eps && !avail.is_empty() {
        let i = ((rng / eps.max(1e-9)) * avail.len() as f64) as usize % avail.len();
        return avail[i].to_string();
    }
    let total: u32 = avail.iter().map(|k| o.tactics.get(*k).map(|s| s.n).unwrap_or(0)).sum();
    let mut best = ("spacing", f64::MIN);
    for k in &avail {
        let s = o.tactics.get(*k).cloned().unwrap_or_default();
        let mean = if s.n > 0 { s.reward / s.n as f64 } else { 0.5 };
        let bonus = if s.n > 0 { ((2.0 * ((total + 1) as f64).ln()) / s.n as f64).sqrt() * 0.5 } else { 0.6 };
        let v = mean + bonus + c[k];
        if v > best.1 {
            best = (k, v);
        }
    }
    best.0.to_string()
}

impl Store {
    /// Un round finito: aggiorna stile, tattica, copia. Restituisce la riga per la memoria.
    pub fn on_round(&mut self, ev: &Value, now: i64) -> String {
        let who = ev["who"].as_str().unwrap_or("?").to_string();
        let o = self.opponents.entry(who.clone()).or_default();
        let won = ev["won"].as_bool();
        let (dealt, taken) = (f(ev, "dealt"), f(ev, "taken"));
        let first = o.rounds == 0;
        o.rounds += 1;
        o.last = now;
        match won {
            Some(true) => o.wins += 1,
            Some(false) => o.losses += 1,
            None => {}
        }
        if ev["style"].is_object() && f(&ev["style"], "secs") > 0.3 {
            o.style.blend(&ev["style"], 0.35, first);
        }
        let t = ev["tactic"].as_str().unwrap_or("spacing").to_string();
        let st = o.tactics.entry(t.clone()).or_default();
        st.n += 1;
        st.reward += reward(won, dealt, taken);
        if won == Some(true) {
            st.wins += 1;
        }
        // ha vinto lui: si copia quello che gli funziona
        if won == Some(false) && o.style.pref_dist > 0.0 {
            o.mimic = Some(Mimic { dist: o.style.pref_dist, strafe: o.style.strafe });
        }
        let res = match won {
            Some(true) => "won",
            Some(false) => "lost",
            None => "draw",
        };
        format!("duel round vs {who}: {res} with {t} (dealt {dealt:.0}, took {taken:.0}); total {}-{}", o.wins, o.losses)
    }

    /// Riga corta per il prompt e il grafo ("duels with Makoto: 3-2, he fights very close, jump crits; best tactic: shield").
    pub fn line(&self, who: &str) -> String {
        let Some(o) = self.opponents.get(who) else { return String::new() };
        let best = o.tactics.iter().filter(|(_, s)| s.n > 0).max_by(|a, b| (a.1.reward / a.1.n as f64).total_cmp(&(b.1.reward / b.1.n as f64))).map(|(k, _)| k.as_str());
        let mut s = format!("duels with {who}: you {}-{}", o.wins, o.losses);
        let st = o.style.line();
        if !st.is_empty() {
            s.push_str(&format!(", they {st}"));
        }
        if let Some(b) = best {
            s.push_str(&format!("; best tactic so far: {b}"));
        }
        if !o.rules.is_empty() {
            s.push_str(&format!("; rules they like: {}", o.rules));
        }
        s
    }
}

/// Opzioni di tattica per Rizzo Flow con i risultati finora ("reward 0.62 in 4 rounds").
pub fn options(o: &Opponent, have: &Have) -> Vec<(String, String)> {
    let c = counters(&o.style, o.rounds, have);
    TACTICS
        .iter()
        .filter(|(k, _)| c[k] > -5.0)
        .map(|(k, d)| {
            let s = o.tactics.get(*k).cloned().unwrap_or_default();
            let r = if s.n > 0 { format!(" (avg reward {:.2} over {} rounds)", s.reward / s.n as f64, s.n) } else { " (never tried)".into() };
            (k.to_string(), format!("{d}{r}"))
        })
        .collect()
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

    fn round(who: &str, tactic: &str, won: Option<bool>, dealt: f64, taken: f64, crit: f64, dist: f64) -> Value {
        json!({ "who": who, "tactic": tactic, "won": won, "dealt": dealt, "taken": taken, "style": { "secs": 10, "prefDist": dist, "crit": crit, "strafe": 0.8, "cps": 2 } })
    }

    #[test]
    fn learns_between_rounds_and_sessions() {
        let mut st = Store::default();
        let have = Have { shield: true, bow: false, axe: false };
        // primo incontro: niente dati, sceglie con l'esplorazione (tutte uguali + counter vuoti)
        let first = choose(&Opponent::default(), &have, 0.9, 0.1);
        assert!(TACTICS.iter().any(|(k, _)| *k == first) && first != "kite" && first != "axe");
        // perde con rush contro uno che fa critici da vicino
        let line = st.on_round(&round("Makoto", "rush", Some(false), 6.0, 14.0, 0.8, 2.2), 100);
        assert!(line.contains("lost with rush") && line.contains("0-1"), "{line}");
        let o = &st.opponents["Makoto"];
        assert!(o.style.crit > 0.7 && o.style.pref_dist < 2.6);
        assert_eq!(o.mimic, Some(Mimic { dist: 2.2, strafe: 0.8 }), "copia chi vince");
        // counter: scudo e distanza contro i critici da vicino
        let c = counters(&o.style, o.rounds, &have);
        assert!(c["shield"] > 0.3 && c["spacing"] > 0.3 && c["kite"] < -5.0);
        let next = choose(o, &have, 0.9, 0.1);
        assert!(next == "shield" || next == "spacing", "{next}");
        // lo scudo funziona: vince due volte → resta la scelta migliore
        st.on_round(&round("Makoto", "shield", Some(true), 14.0, 4.0, 0.8, 2.2), 101);
        st.on_round(&round("Makoto", "shield", Some(true), 12.0, 6.0, 0.8, 2.2), 102);
        st.on_round(&round("Makoto", "spacing", Some(false), 4.0, 14.0, 0.8, 2.2), 103);
        let o = &st.opponents["Makoto"];
        assert_eq!((o.wins, o.losses, o.rounds), (2, 2, 4));
        assert_eq!(choose(o, &have, 0.9, 0.1), "shield");
        assert!(st.line("Makoto").contains("best tactic so far: shield") && st.line("Makoto").contains("jump crits"), "{}", st.line("Makoto"));
        assert_eq!(st.line("Nessuno"), "");
        // opzioni per Rizzo con i risultati
        let opts = options(o, &have);
        assert!(opts.iter().any(|(k, d)| k == "shield" && d.contains("over 2 rounds")));
        assert!(!opts.iter().any(|(k, _)| k == "kite"), "niente arco: niente kite");
        // tra sessioni: file
        let dir = std::env::temp_dir().join(format!("ae-mc-pvp-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        save(&dir, &st);
        assert_eq!(load(&dir), st);
        forget(&dir);
        assert_eq!(load(&dir), Store::default());
        let _ = std::fs::remove_dir_all(&dir);
        // esplorazione: a volte prova altro
        assert!(TACTICS.iter().any(|(k, _)| *k == choose(o, &have, 0.01, 0.1)));
        assert!(reward(Some(true), 10.0, 2.0) > reward(Some(true), 2.0, 10.0) && reward(Some(true), 2.0, 10.0) > reward(Some(false), 2.0, 10.0));
        assert!(Have::from_inv("iron_sword, shield, bow, arrow x12").bow && !Have::from_inv("bow").bow);
    }
}
