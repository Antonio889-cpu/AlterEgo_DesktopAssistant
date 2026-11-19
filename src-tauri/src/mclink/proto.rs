//! Protocollo con il bot Node (righe JSON su stdin/stdout, vedi mc_bot/bot.js):
//! comandi numerati verso il bot; stato, sensi, eventi e conferme dal bot.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;

/// Stato compatto mandato dal bot ogni secondo.
#[derive(Deserialize, Serialize, Default, Debug, Clone)]
#[serde(default)]
pub struct McState {
    pub hp: f64,
    pub food: f64,
    pub pos: Vec<i64>,
    pub dim: String,
    /// "day", "dusk", "night", "dawn".
    pub time: String,
    pub rain: bool,
    /// Comportamento continuo: "idle", "follow", "stay", "guard".
    pub mode: String,
    pub target: String,
    /// Azione lunga in corso ("collect", "craft"…).
    pub task: Option<String>,
    pub held: Option<String>,
    pub inv: String,
    pub players: Vec<Player>,
    pub near: Vec<Near>,
    pub light: Option<f64>,
    pub water: bool,
    /// Con chi gioca (nome nelle impostazioni o giocatore più vicino).
    pub owner: String,
    pub version: String,
}

#[derive(Deserialize, Serialize, Default, Debug, Clone)]
#[serde(default)]
pub struct Player {
    pub name: String,
    pub d: Option<f64>,
    pub dir: Option<String>,
    /// false = lontano, non lo vede.
    pub seen: Option<bool>,
}

#[derive(Deserialize, Serialize, Default, Debug, Clone)]
#[serde(default)]
pub struct Near {
    pub name: String,
    pub d: f64,
    pub dir: Option<String>,
    pub hostile: Option<bool>,
}

#[derive(Deserialize, Serialize, Default, Debug, Clone, PartialEq)]
#[serde(default)]
pub struct LanWorld {
    pub host: String,
    pub port: u16,
    pub motd: String,
}

/// Una riga dal bot.
#[derive(Debug, Clone)]
pub enum Incoming {
    Ready,
    State(Box<McState>),
    Sense(HashMap<String, f32>),
    Event(Value),
    Ack { id: u64, op: String, ok: bool, err: String, info: Value },
    Lan(Vec<LanWorld>),
}

/// Legge una riga del bot (None: riga non del protocollo, per esempio un log finito su stdout).
pub fn parse_line(line: &str) -> Option<Incoming> {
    let l = line.trim();
    if !l.starts_with('{') {
        return None;
    }
    let v: Value = serde_json::from_str(l).ok()?;
    Some(match v["t"].as_str()? {
        "ready" => Incoming::Ready,
        "state" => Incoming::State(Box::new(serde_json::from_value(v).ok()?)),
        "sense" => Incoming::Sense(
            v["s"]
                .as_object()
                .map(|o| o.iter().filter_map(|(k, x)| x.as_f64().map(|f| (k.clone(), (f as f32).clamp(0.0, 1.0)))).collect())
                .unwrap_or_default(),
        ),
        "ev" if v["kind"].is_string() => Incoming::Event(v),
        "ack" => Incoming::Ack {
            id: v["id"].as_u64()?,
            op: v["op"].as_str().unwrap_or("").to_string(),
            ok: v["ok"].as_bool().unwrap_or(false),
            err: v["err"].as_str().unwrap_or("").to_string(),
            info: v["info"].clone(),
        },
        "lan" => Incoming::Lan(v["worlds"].as_array().map(|a| a.iter().filter_map(|w| serde_json::from_value(w.clone()).ok()).collect()).unwrap_or_default()),
        _ => return None,
    })
}

/// Comando per il bot: `op` con il numero `id`, una riga terminata da "\n".
pub fn cmd_line(id: u64, op: &Value) -> String {
    let mut o = op.clone();
    if let Some(m) = o.as_object_mut() {
        m.insert("id".into(), json!(id));
    }
    let mut s = o.to_string();
    s.push('\n');
    s
}

/// Perché il server l'ha buttata fuori (per un messaggio chiaro nelle impostazioni).
pub fn kick_code(reason: &str) -> &'static str {
    let r = reason.to_lowercase();
    if r.contains("unverified_username") || r.contains("failed to verify") || r.contains("not authenticated") || r.contains("invalid session") || r.contains("authservers") {
        "online_mode"
    } else if r.contains("whitelist") || r.contains("white-list") || r.contains("not_whitelisted") {
        "whitelist"
    } else if r.contains("banned") {
        "banned"
    } else if r.contains("outdated") || r.contains("incompatible") || r.contains("unsupported") || r.contains("version") {
        "version"
    } else if r.contains("duplicate_login") || r.contains("another location") {
        "duplicate"
    } else if r.contains("server_full") || r.contains("server is full") {
        "full"
    } else if r.contains("econnrefused") || r.contains("connection refused") {
        "refused"
    } else if r.contains("no lan world") {
        "no_lan"
    } else {
        "other"
    }
}

/// Stato in una riga compatta per il modello (~50-120 token).
pub fn state_line(s: &McState) -> String {
    let mut out = format!("[state] you {:.0}hp food {:.0}", s.hp, s.food);
    if !s.mode.is_empty() {
        out.push_str(&format!(", {}", s.mode));
        if (s.mode == "follow" || s.mode == "guard") && !s.target.is_empty() {
            out.push_str(&format!(" {}", s.target));
        }
    }
    if let Some(t) = &s.task {
        out.push_str(&format!(", doing {t}"));
    }
    if s.pos.len() == 3 {
        out.push_str(&format!(", {} at {} {} {}", if s.dim.is_empty() { "overworld" } else { &s.dim }, s.pos[0], s.pos[1], s.pos[2]));
    }
    if !s.time.is_empty() {
        out.push_str(&format!(", {}", s.time));
    }
    if s.rain {
        out.push_str(" raining");
    }
    if s.light.is_some_and(|l| l <= 4.0) {
        out.push_str(", dark here");
    }
    if s.water {
        out.push_str(", in water");
    }
    if let Some(h) = &s.held {
        out.push_str(&format!(", holding {h}"));
    }
    out.push_str(&format!(" | inv: {}", if s.inv.is_empty() { "empty" } else { &s.inv }));
    for p in &s.players {
        out.push_str(&format!(" | {}", p.name));
        if p.name == s.owner {
            out.push_str(" (your friend)");
        }
        match p.d {
            Some(d) if p.seen != Some(false) => out.push_str(&format!(" {d:.0}m {}", p.dir.as_deref().unwrap_or(""))),
            _ => out.push_str(" far away"),
        }
    }
    if !s.near.is_empty() {
        let near: Vec<String> =
            s.near.iter().map(|n| format!("{} {:.0}m {}{}", n.name, n.d, n.dir.as_deref().unwrap_or(""), if n.hostile == Some(true) { " hostile" } else { "" })).collect();
        out.push_str(&format!(" | near: {}", near.join(", ")));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_bot_lines() {
        assert!(matches!(parse_line(r#"{"t":"ready","v":1}"#), Some(Incoming::Ready)));
        let st = parse_line(r#"{"t":"state","hp":18,"food":15,"pos":[12,64,-30],"mode":"follow","target":"Makoto","players":[{"name":"Makoto","d":3,"dir":"front"}],"extra":1}"#);
        match st {
            Some(Incoming::State(s)) => assert_eq!((s.hp, s.players[0].name.as_str(), s.pos.len()), (18.0, "Makoto", 3)),
            other => panic!("{other:?}"),
        }
        match parse_line(r#"{"t":"sense","s":{"pain":0.5,"loom_l":3,"x":"no"}}"#) {
            Some(Incoming::Sense(s)) => assert_eq!((s["pain"], s["loom_l"], s.len()), (0.5, 1.0, 2), "fuori scala: tagliato a 1"),
            other => panic!("{other:?}"),
        }
        match parse_line(r#"{"t":"ack","id":42,"op":"collect","ok":false,"err":"no oak_log nearby"}"#) {
            Some(Incoming::Ack { id, ok, err, op, .. }) => assert_eq!((id, ok, err.as_str(), op.as_str()), (42, false, "no oak_log nearby", "collect")),
            other => panic!("{other:?}"),
        }
        assert!(matches!(parse_line(r#"{"t":"ev","kind":"chat","from":"A","text":"ciao"}"#), Some(Incoming::Event(v)) if v["text"] == "ciao"));
        match parse_line(r#"{"t":"lan","worlds":[{"host":"192.168.1.5","port":51234,"motd":"Makoto - Mondo"}]}"#) {
            Some(Incoming::Lan(w)) => assert_eq!(w[0], LanWorld { host: "192.168.1.5".into(), port: 51234, motd: "Makoto - Mondo".into() }),
            other => panic!("{other:?}"),
        }
        // log o righe rotte: ignorate
        for l in ["connessione a 127.0.0.1", "{rotto", r#"{"t":"ev"}"#, r#"{"t":"boh"}"#, r#"{"t":"ack"}"#, ""] {
            assert!(parse_line(l).is_none(), "{l}");
        }
    }

    #[test]
    fn numbered_commands() {
        let l = cmd_line(7, &json!({ "op": "follow", "who": "Makoto" }));
        assert!(l.ends_with('\n') && l.matches('\n').count() == 1);
        let v: Value = serde_json::from_str(l.trim()).unwrap();
        assert_eq!(v, json!({ "op": "follow", "who": "Makoto", "id": 7 }));
        // testo con a capo: resta una riga sola
        assert_eq!(cmd_line(1, &json!({ "op": "say", "text": "a\nb" })).matches('\n').count(), 1);
    }

    #[test]
    fn kick_reasons() {
        assert_eq!(kick_code("multiplayer.disconnect.unverified_username"), "online_mode");
        assert_eq!(kick_code("Failed to verify username!"), "online_mode");
        assert_eq!(kick_code("You are not whitelisted on this server!"), "whitelist");
        assert_eq!(kick_code("Outdated client! Please use 1.21.4"), "version");
        assert_eq!(kick_code("connect ECONNREFUSED 127.0.0.1:25565"), "refused");
        assert_eq!(kick_code("You logged in from another location"), "duplicate");
        assert_eq!(kick_code("boh"), "other");
    }

    #[test]
    fn compact_state_line() {
        let s: McState = serde_json::from_value(json!({
            "hp": 18, "food": 15, "pos": [12, 64, -30], "dim": "overworld", "time": "night", "mode": "follow", "target": "Makoto",
            "task": "collect", "held": "stone_pickaxe", "inv": "oak_log x12, cobblestone x30", "light": 2, "owner": "Makoto",
            "players": [{ "name": "Makoto", "d": 3, "dir": "front" }, { "name": "Hina", "seen": false }],
            "near": [{ "name": "zombie", "d": 8, "dir": "left", "hostile": true }, { "name": "cow", "d": 5, "dir": "back" }]
        }))
        .unwrap();
        assert_eq!(
            state_line(&s),
            "[state] you 18hp food 15, follow Makoto, doing collect, overworld at 12 64 -30, night, dark here, holding stone_pickaxe | inv: oak_log x12, cobblestone x30 | Makoto (your friend) 3m front | Hina far away | near: zombie 8m left hostile, cow 5m back"
        );
        assert!(state_line(&McState::default()).contains("inv: empty"));
    }
}
