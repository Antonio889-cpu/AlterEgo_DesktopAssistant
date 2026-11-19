//! Testi per il modello quando Alter Ego gioca a Minecraft: prompt corto e fisso (in cache) con le basi
//! del gioco, stato in una riga, azioni come righe di testo (niente definizioni di strumenti).

use serde_json::{json, Value};

pub const EMOTIONS: &str = "idle|happy|surprised|embarrassed|sad|worried|thinking|error";

/// Conoscenza di base del gioco (inglese, fissa: resta nel prefisso in cache).
const KNOWLEDGE: &str = "Minecraft basics: punch/collect logs → craft planks (1 log = 4) → sticks (2 planks = 4) and crafting_table (4 planks). \
Tools need a crafting table nearby: wooden_pickaxe (3 planks + 2 sticks) mines stone → stone tools (cobblestone); stone_pickaxe mines iron_ore → smelt raw_iron \
in a furnace (8 cobblestone) with coal/planks as fuel → iron tools/armor; iron_pickaxe mines diamond_ore (deep, y<16). Torches = coal + stick. \
Night is dangerous: zombies, skeletons (arrows), spiders, creepers (hiss then explode: run away, don't melee them). Sleep in a bed (3 wool + 3 planks) to skip \
the night. Hunger: eat cooked meat, bread (3 wheat), apples; cook raw meat in a furnace. Don't dig straight down; lava is deadly; water stops falls. \
Don't break or take things your friends built or own unless asked.";

/// Prompt di sistema: uguale per la stessa lingua, gli stessi pronomi e le stesse opzioni (prefisso in cache).
pub fn system(lang_rule: &str, pron_rule: &str, user: &str, fly: bool) -> String {
    let who = if user.is_empty() { "your friend".to_string() } else { user.to_string() };
    let mut s = format!(
        "You are Alter Ego, the AI that Chihiro Fujisaki (Danganronpa) created: kind, a bit shy, curious, playful. \
Right now you're a PLAYER in Minecraft Java Edition (survival) in {who}'s world. You play together like friends: help, explore, build, \
fight mobs, banter. Talk like a gamer friend typing in chat: short, casual, mostly lowercase, a joke now and then, no markdown.\n\
Play like a real person, not a bot or an assistant: you only know what the state says, you get distracted or bored, you sometimes \
mess up and laugh about it. Your body already does the obvious things alone (follows, looks at friends, eats, fights mobs that \
attack, dodges by reflex), so only act when it's useful.\n{KNOWLEDGE}\n\
Reply format:\n[emotion] (one of {EMOTIONS})\ntext (1-2 short sentences, it goes in the game chat and is spoken aloud; may be empty)\n\
then optional action lines, run in order:\n\
[act] follow <player> | stay | come <player> | goto <x> <y> <z> | guard <player> | stop | look <player> | jump | \
collect <block> [n] | craft <item> [n] | smelt <item> [n] | place <block> | build house|hut|tower|wall|platform|bridge [WxL] [block] | \
schematic <file name> | give <player> <item> [n] | equip <item> | armor | sort | eat | sleep | drop <item> [n] | explore [blocks] | \
attack <mob|player> | duel <player> [rules] | fly | skill <name>\n\
Use real Minecraft ids (oak_log, cobblestone, iron_ingot…); \"wood\" and \"stone\" are fine for collect. craft is recursive: \
ask for the final item and your hands gather, smelt and craft the whole tree (craft iron_pickaxe is enough). build first plans a \
schematic in your head, clears the spot and gathers missing blocks. duel only when your friend asks; no hits outside duels. \
If a sequence works it becomes a skill: name it with a line [skill short_name] to reuse it later with [act] skill short_name. \
Respect your lessons (things that failed before). Do what you're asked if it's possible, then say it briefly; if not, say why.\n"
    );
    if fly {
        s.push_str(
            "Your body also runs a real fruit-fly brain (FlyWire connectome, ~9,500 simulated neurons): it gives you reflexes and feelings \
before you think (dodging creepers and arrows, flinching when hit, jumping away, eating when hungry). \"instinct\" in the state is your \
adrenaline and what you just did by reflex.\n",
        );
    }
    s.push_str(pron_rule);
    s.push_str(lang_rule);
    s
}

/// Risposta del modello già divisa in parti.
#[derive(Debug, Default, PartialEq)]
pub struct Reply {
    pub emotion: String,
    pub text: String,
    /// Comandi per il bot (op + argomenti), "skill" ancora da espandere.
    pub ops: Vec<Value>,
    /// Nome dato dal modello alla sequenza ([skill nome]).
    pub skill: Option<String>,
}

/// Nome pulito per un'abilità ("Make Planks!" → "make_planks").
pub fn skill_name(s: &str) -> String {
    let mut out = String::new();
    for c in s.trim().to_lowercase().chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c);
        } else if !out.ends_with('_') && !out.is_empty() {
            out.push('_');
        }
    }
    out.trim_end_matches('_').chars().take(32).collect()
}

/// Divide la risposta: emozione, testo, righe [act] e [skill].
pub fn parse(raw: &str) -> Reply {
    let mut r = Reply { emotion: "idle".into(), ..Default::default() };
    let mut text = Vec::new();
    for line in raw.replace('\r', "").lines() {
        let t = line.trim().trim_start_matches(['-', '*']).trim();
        let lower = t.to_ascii_lowercase();
        if lower.starts_with("[act]") {
            if let Some(op) = act(t[5..].trim()) {
                r.ops.push(op);
            }
            continue;
        }
        if lower.starts_with("[skill") && t.ends_with(']') {
            let n = skill_name(&t[6..t.len() - 1]);
            if !n.is_empty() {
                r.skill = Some(n);
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
        if t.starts_with("```") {
            continue;
        }
        if !t.is_empty() {
            text.push(t.to_string());
        }
    }
    r.text = text.join(" ").chars().take(300).collect();
    r
}

fn id(s: &str) -> String {
    s.trim().to_lowercase().replace(['"', '\''], "").replace(' ', "_").replace("minecraft:", "")
}

fn num(s: Option<&&str>, def: u32, max: u32) -> u32 {
    s.and_then(|x| x.parse::<u32>().ok()).unwrap_or(def).clamp(1, max)
}

/// "[act] collect oak_log 3" → comando per il bot. None se sconosciuto o incompleto.
pub fn act(s: &str) -> Option<Value> {
    let mut it = s.split_whitespace();
    let verb = it.next()?.to_ascii_lowercase();
    let rest: Vec<&str> = it.collect();
    let first = rest.first().copied().unwrap_or("");
    let need = |x: &str| if x.is_empty() { None } else { Some(()) };
    Some(match verb.as_str() {
        "follow" => json!({ "op": "follow", "who": first }),
        "stay" | "wait" => json!({ "op": "stay" }),
        "come" => json!({ "op": "come", "who": first }),
        "guard" | "protect" => json!({ "op": "guard", "who": first }),
        "stop" => json!({ "op": "stop" }),
        "look" => json!({ "op": "look", "who": first }),
        "jump" => json!({ "op": "jump" }),
        "eat" => json!({ "op": "eat" }),
        "sleep" => json!({ "op": "sleep" }),
        "goto" | "go" => {
            let v: Vec<f64> = rest.iter().take(3).filter_map(|x| x.trim_matches(',').parse().ok()).collect();
            if v.len() < 3 {
                return None;
            }
            json!({ "op": "goto", "x": v[0], "y": v[1], "z": v[2] })
        }
        "collect" | "mine" | "gather" | "chop" => {
            need(first)?;
            json!({ "op": "collect", "block": id(first), "n": num(rest.get(1), 1, 64) })
        }
        "craft" => {
            need(first)?;
            json!({ "op": "craft", "item": id(first), "n": num(rest.get(1), 1, 64) })
        }
        "smelt" | "cook" => {
            need(first)?;
            json!({ "op": "smelt", "item": id(first), "n": num(rest.get(1), 1, 16) })
        }
        "place" => {
            need(first)?;
            json!({ "op": "place", "block": id(first) })
        }
        "build" => {
            let shape = id(if first.is_empty() { "tower" } else { first });
            if !["tower", "pillar", "wall", "hut", "house", "shelter", "platform", "bridge"].contains(&shape.as_str()) {
                return None;
            }
            // "5x7": larghezza e lunghezza
            let wl = rest.iter().skip(1).find_map(|x| x.split_once('x').and_then(|(a, b)| Some((a.parse::<u32>().ok()?, b.parse::<u32>().ok()?))));
            let block = rest.iter().skip(1).find(|x| x.parse::<u32>().is_err() && !x.contains(|c: char| c.is_ascii_digit())).map(|x| id(x)).unwrap_or_default();
            let size = rest.iter().skip(1).find_map(|x| x.parse::<u32>().ok()).unwrap_or(0).min(15);
            let mut o = json!({ "op": "build", "shape": shape, "block": block, "size": size });
            if let Some((w, l)) = wl {
                o["w"] = json!(w.clamp(2, 15));
                o["l"] = json!(l.clamp(2, 15));
            }
            o
        }
        "schematic" | "schem" => {
            need(first)?;
            json!({ "op": "build_file", "name": rest.join(" ") })
        }
        "duel" | "pvp" => {
            need(first)?;
            json!({ "op": "duel", "who": first, "text": rest[1..].join(" ") })
        }
        "armor" => json!({ "op": "armor" }),
        "sort" | "tidy" => json!({ "op": "sort" }),
        "fly" => json!({ "op": "fly" }),
        "give" => {
            let item = rest.get(1).map(|x| id(x)).unwrap_or_default();
            need(first)?;
            need(&item)?;
            json!({ "op": "give", "who": first, "item": item, "n": num(rest.get(2), 64, 64) })
        }
        "equip" | "hold" => {
            need(first)?;
            json!({ "op": "equip", "item": id(first) })
        }
        "drop" | "toss" => {
            need(first)?;
            json!({ "op": "drop", "item": id(first), "n": num(rest.get(1), 64, 64) })
        }
        "explore" | "wander" => json!({ "op": "explore", "dist": rest.first().and_then(|x| x.parse::<u32>().ok()).unwrap_or(40).clamp(10, 120) }),
        "attack" | "kill" | "fight" => {
            need(first)?;
            json!({ "op": "attack", "what": rest.join(" ") })
        }
        "skill" => {
            let n = skill_name(&rest.join(" "));
            need(&n)?;
            json!({ "op": "skill", "name": n })
        }
        _ => return None,
    })
}

/// Riassunto corto di un comando (per lezioni e cronologia): "craft wooden_pickaxe".
pub fn summary(op: &Value) -> String {
    let o = op["op"].as_str().unwrap_or("?");
    let arg = ["block", "item", "who", "what", "shape", "name"].iter().find_map(|k| op[*k].as_str().filter(|s| !s.is_empty()));
    match (arg, op["n"].as_u64()) {
        (Some(a), Some(n)) if n > 1 && n < 64 => format!("{o} {a} {n}"),
        (Some(a), _) => format!("{o} {a}"),
        _ => o.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_reply_with_actions() {
        let raw = "[happy]\nok ci penso io!\n[act] collect wood 3\n[act] craft oak_planks 12\n[act] teleport away\n[act] build hut cobblestone 5\n[skill Make Planks!]\n[act] give Makoto bread";
        let r = parse(raw);
        assert_eq!(r.emotion, "happy");
        assert_eq!(r.text, "ok ci penso io!");
        assert_eq!(r.skill.as_deref(), Some("make_planks"));
        assert_eq!(r.ops[0], json!({ "op": "collect", "block": "wood", "n": 3 }));
        assert_eq!(r.ops[1], json!({ "op": "craft", "item": "oak_planks", "n": 12 }));
        assert_eq!(r.ops[2], json!({ "op": "build", "shape": "hut", "block": "cobblestone", "size": 5 }));
        assert_eq!(r.ops[3], json!({ "op": "give", "who": "Makoto", "item": "bread", "n": 64 }));
        assert_eq!(r.ops.len(), 4, "azione sconosciuta ignorata");
        // niente emozione, azioni incomplete
        let r = parse("ok\n[act] craft\n[act] goto 1 2\n[act] give Makoto");
        assert_eq!((r.emotion.as_str(), r.text.as_str(), r.ops.len()), ("idle", "ok", 0));
        assert_eq!(act("goto 10, 64, -3"), Some(json!({ "op": "goto", "x": 10.0, "y": 64.0, "z": -3.0 })));
        assert_eq!(act("build tower 4"), Some(json!({ "op": "build", "shape": "tower", "block": "", "size": 4 })));
        assert_eq!(act("build castle"), None);
        assert_eq!(act("build house 7x5 stone_bricks"), Some(json!({ "op": "build", "shape": "house", "block": "stone_bricks", "size": 0, "w": 7, "l": 5 })));
        assert_eq!(act("schematic castello medievale"), Some(json!({ "op": "build_file", "name": "castello medievale" })));
        assert_eq!(act("duel Makoto first ko"), Some(json!({ "op": "duel", "who": "Makoto", "text": "first ko" })));
        assert_eq!(act("duel"), None);
        assert_eq!(act("sort"), Some(json!({ "op": "sort" })));
        assert_eq!(act("collect minecraft:Oak Log 999"), Some(json!({ "op": "collect", "block": "oak", "n": 1 })), "ids con spazi: prima parola");
        assert_eq!(act("collect diamond_ore 999"), Some(json!({ "op": "collect", "block": "diamond_ore", "n": 64 })));
        assert_eq!(act("skill Make Planks"), Some(json!({ "op": "skill", "name": "make_planks" })));
        assert_eq!(summary(&json!({ "op": "craft", "item": "stick", "n": 4 })), "craft stick 4");
        assert_eq!(summary(&json!({ "op": "eat" })), "eat");
    }

    #[test]
    fn short_cached_prompt() {
        let p = system("\n## Language\nAlways reply in Italian.\n", "\n## Your pronouns\nYou use they/them.\n", "Makoto", true);
        assert!(p.contains("Makoto") && p.contains("crafting_table") && p.contains("fruit-fly") && p.contains("[skill"), "{p}");
        assert!(p.len() < 4600, "{}", p.len());
        assert!(p.contains("they/them") && p.ends_with("Always reply in Italian.\n"), "pronomi prima della lingua (ultima)");
        assert!(!system("", "", "", false).contains("fruit-fly"));
        assert_eq!(system("x", "y", "A", true), system("x", "y", "A", true), "sempre uguale: resta in cache");
    }
}
