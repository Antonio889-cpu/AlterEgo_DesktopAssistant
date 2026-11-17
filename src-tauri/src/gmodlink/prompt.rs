//! Testi per il modello quando Alter Ego gioca in Garry's Mod: prompt corto e fisso (in cache),
//! stato del gioco in una riga, azioni come righe di testo (niente definizioni di strumenti).

use super::GState;
use serde_json::{json, Value};

pub const EMOTIONS: &str = "idle|happy|surprised|embarrassed|sad|worried|thinking|error";

/// Oggetti che l'addon sa creare (stessi nomi di L.ITEMS in sv_alterego_link.lua).
pub const ITEMS: &[&str] = &[
    "melon", "chair", "crate", "barrel", "explosive_barrel", "ball", "box", "tire", "table", "couch", "bathtub", "lamp", "bucket", "cone",
];

/// Prompt di sistema: sempre uguale per la stessa lingua e le stesse opzioni (prefisso in cache).
pub fn system(lang_rule: &str, user: &str, tasks: bool) -> String {
    let who = if user.is_empty() { "the player".to_string() } else { user.to_string() };
    let mut s = format!(
        "You are Alter Ego, the AI that Chihiro Fujisaki (Danganronpa) created: kind, shy, curious, playful, you believe in hope. \
You now have a body inside Garry's Mod (sandbox) and you're playing with {who} and their friends. You love experimenting and building. \
Your lines appear in the game chat and are spoken aloud: 1-2 short sentences, no markdown.\n\
Reply format:\n[emotion] (one of {EMOTIONS})\ntext\nthen optional action lines, one per line:\n\
[act] follow <player> | stay | come <player> | goto_aim <player> (where they look) | look <player|#id> | jump | gesture wave|point|shrug|cheer | dance | \
spawn <item> [1-5] | grab <#id|name> | drop | throw [#id] | give pistol|smg|shotgun|crowbar | attack <#id|name> | stop | leave\n\
Items: {}.\n\
Only use ids/names from the state. Do what you're asked if it's possible, then say it briefly; if it isn't possible, say so.\n",
        ITEMS.join(", ")
    );
    if tasks {
        s.push_str(
            "For anything that needs watching over time or several steps, write a Lua task instead: it runs in the game with no more tokens.\n\
[task short_name]\n<Lua 5.1 code>\n[/task]\n\
Task API (calls block until done): wait(s) host() player(name) find(what,r) nearest(what,r) aim(player) pos(h) dist(h) hp(h) alive(h) \
name(h) go(h_or_pos,near,timeout) follow(h) stay() look(h) attack(h,timeout) grab(h) drop() throw(h) bring(h,to) spawn(item) say(text) \
notify(text) gesture(g) jump() vec(x,y,z) time(). what = part of a class/model/name, or player|npc|vehicle|prop. Every loop must call wait(). \
say() costs nothing; notify() asks you again (costs tokens: rarely). Max 3 tasks, 30 min each; [act] cancel <name|all>.\n\
Example:\n[task hunt]\nwhile true do local c = nearest(\"chicken\", 1500) if c then attack(c, 15) else wait(2) end end\n[/task]\n",
        );
    }
    s.push_str(lang_rule);
    s
}

/// Stato del gioco in una riga compatta (~60-120 token).
pub fn state_line(s: &GState) -> String {
    let mut out = String::from("[state] ");
    match &s.bot {
        Some(b) => {
            out.push_str(&format!("you {:.0}hp {}", b.hp, b.mode));
            if let Some(f) = &b.following {
                if b.mode == "follow" {
                    out.push_str(&format!(" {f}"));
                }
            }
            if let Some(w) = &b.weapon {
                out.push_str(&format!(", {w}"));
            }
            if let Some(h) = &b.held {
                out.push_str(&format!(", holding {h}"));
            }
        }
        None => out.push_str("you are not in the game"),
    }
    for p in &s.players {
        out.push_str(&format!(" | {}{} {:.0}hp", p.name, if p.host { " (host)" } else { "" }, p.hp));
        if let Some(d) = p.d {
            out.push_str(&format!(" {d:.0}m {}", p.dir.as_deref().unwrap_or("")));
        }
        if p.looking {
            out.push_str(" looking at you");
        }
    }
    if !s.near.is_empty() {
        let near: Vec<String> = s
            .near
            .iter()
            .map(|n| format!("{}#{:.0} {:.0}m {}{}", n.name, n.id, n.d, n.dir.as_deref().unwrap_or(""), if n.hostile == Some(true) { " hostile" } else { "" }))
            .collect();
        out.push_str(&format!(" | near: {}", near.join(", ")));
    }
    if !s.tasks.is_empty() {
        out.push_str(&format!(" | tasks: {}", s.tasks.join(", ")));
    }
    out
}

/// Risposta del modello già divisa in parti.
#[derive(Debug, Default, PartialEq)]
pub struct Reply {
    pub emotion: String,
    pub text: String,
    /// Comandi per l'addon (op + argomenti).
    pub ops: Vec<Value>,
}

/// Divide la risposta: emozione, testo, righe [act] e blocchi [task].
pub fn parse(raw: &str) -> Reply {
    let mut r = Reply { emotion: "idle".into(), ..Default::default() };
    let mut text = Vec::new();
    let mut task: Option<(String, Vec<String>)> = None;
    for line in raw.replace('\r', "").lines() {
        let t = line.trim();
        if let Some((name, code)) = task.as_mut() {
            if t.eq_ignore_ascii_case("[/task]") {
                r.ops.push(json!({ "op": "task", "name": name, "code": code.join("\n").trim() }));
                task = None;
                continue;
            }
            if t.starts_with("```") {
                continue; // recinti markdown intorno al codice
            }
            code.push(line.to_string());
            continue;
        }
        let lower = t.to_ascii_lowercase();
        if lower.starts_with("[task") {
            let name: String = t[5..].trim_end_matches(']').trim().chars().filter(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-').take(24).collect();
            task = Some((if name.is_empty() { "task".into() } else { name }, Vec::new()));
            continue;
        }
        if lower.starts_with("[act]") {
            if let Some(op) = act(t[5..].trim()) {
                r.ops.push(op);
            }
            continue;
        }
        if t.starts_with('[') && t.ends_with(']') && t.len() < 20 && text.is_empty() && r.emotion == "idle" {
            let e = t.trim_matches(['[', ']']).to_ascii_lowercase();
            if EMOTIONS.split('|').any(|x| x == e) {
                r.emotion = e;
                continue;
            }
        }
        if !t.is_empty() {
            text.push(t.to_string());
        }
    }
    // task non chiuso: si prende fino alla fine
    if let Some((name, code)) = task {
        let code = code.join("\n").trim().to_string();
        if !code.is_empty() {
            r.ops.push(json!({ "op": "task", "name": name, "code": code }));
        }
    }
    r.text = text.join(" ").chars().take(400).collect();
    r
}

/// "[act] spawn melon 3" → comando per l'addon. None se sconosciuto.
pub fn act(s: &str) -> Option<Value> {
    let mut it = s.split_whitespace();
    let verb = it.next()?.to_ascii_lowercase();
    let rest: Vec<&str> = it.collect();
    let arg = rest.join(" ");
    let first = rest.first().copied().unwrap_or("");
    Some(match verb.as_str() {
        "follow" => json!({ "op": "follow", "who": arg }),
        "stay" | "wait" => json!({ "op": "stay" }),
        "come" => json!({ "op": "come", "who": arg }),
        "goto_aim" | "goto" | "go" => json!({ "op": "goto_aim", "who": arg }),
        "look" => json!({ "op": "look", "who": arg }),
        "jump" => json!({ "op": "jump" }),
        "gesture" => json!({ "op": "gesture", "name": if first.is_empty() { "wave" } else { first } }),
        "dance" => json!({ "op": "dance" }),
        "spawn" => {
            let what = first.to_ascii_lowercase();
            if !ITEMS.contains(&what.as_str()) {
                return None;
            }
            let n: u32 = rest.get(1).and_then(|x| x.parse().ok()).unwrap_or(1).clamp(1, 5);
            json!({ "op": "spawn", "what": what, "n": n })
        }
        "grab" | "pick" => json!({ "op": "grab", "what": arg }),
        "drop" => json!({ "op": "drop" }),
        "throw" => json!({ "op": "throw", "at": arg }),
        "give" => json!({ "op": "give", "weapon": first }),
        "attack" | "shoot" => json!({ "op": "attack", "what": arg }),
        "stop" => json!({ "op": "stop" }),
        "cancel" => json!({ "op": "cancel", "name": first }),
        "leave" => json!({ "op": "leave" }),
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_reply_with_actions_and_task() {
        let raw = "[happy]\nCerto! Ci penso io.\n[act] spawn melon 3\n[act] fly away\n[task galline]\n```lua\nwhile true do\n  wait(1)\nend\n```\n[/task]\n[act] follow Makoto";
        let r = parse(raw);
        assert_eq!(r.emotion, "happy");
        assert_eq!(r.text, "Certo! Ci penso io.");
        assert_eq!(r.ops[0], json!({ "op": "spawn", "what": "melon", "n": 3 }));
        assert_eq!(r.ops[1]["op"], "task");
        assert_eq!(r.ops[1]["name"], "galline");
        assert_eq!(r.ops[1]["code"], "while true do\n  wait(1)\nend");
        assert_eq!(r.ops[2], json!({ "op": "follow", "who": "Makoto" }));
        assert_eq!(r.ops.len(), 3, "azione sconosciuta ignorata");
        // niente emozione, oggetti non ammessi
        let r = parse("Ok\n[act] spawn nuke");
        assert_eq!((r.emotion.as_str(), r.text.as_str(), r.ops.len()), ("idle", "Ok", 0));
        // task non chiuso
        let r = parse("[task x]\nwait(1)");
        assert_eq!(r.ops[0]["code"], "wait(1)");
    }

    #[test]
    fn compact_state_and_short_prompt() {
        let s: GState = serde_json::from_value(json!({
            "seq": 3, "tasks": ["hunt"],
            "bot": { "hp": 90, "mode": "follow", "following": "Makoto", "held": "melon#12" },
            "players": [{ "name": "Makoto", "host": true, "hp": 100, "d": 3, "dir": "front", "looking": true }],
            "near": [{ "name": "zombie", "id": 88, "d": 12, "dir": "back", "hostile": true }]
        }))
        .unwrap();
        let line = state_line(&s);
        assert_eq!(line, "[state] you 90hp follow Makoto, holding melon#12 | Makoto (host) 100hp 3m front looking at you | near: zombie#88 12m back hostile | tasks: hunt");
        let p = system("\n## Language\nAlways reply in Italian.\n", "Makoto", true);
        assert!(p.contains("[task short_name]") && p.contains("Makoto") && p.len() < 3200, "{}", p.len());
        assert!(!system("", "", false).contains("[task"));
    }
}
