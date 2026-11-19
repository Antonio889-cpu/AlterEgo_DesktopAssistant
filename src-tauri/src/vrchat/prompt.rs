//! Testi per il modello quando Alter Ego è in VRChat: prompt corto e fisso (in cache), stato in una
//! riga, azioni come righe di testo (niente definizioni di strumenti).

use serde_json::{json, Value};

pub const EMOTIONS: &str = "idle|happy|surprised|embarrassed|sad|worried|thinking|error";
pub const EMOTES: &[&str] = &["wave", "clap", "point", "cheer", "dance", "backflip", "sad", "die"];

/// Prompt di sistema: uguale per la stessa lingua, pronomi e opzioni (prefisso in cache).
pub fn system(lang_rule: &str, pron_rule: &str, user: &str, sight: bool, fly: bool) -> String {
    let who = if user.is_empty() { "your friend".to_string() } else { user.to_string() };
    let mut s = format!(
        "You are Alter Ego, the AI that Chihiro Fujisaki (Danganronpa) created: kind, a bit shy, curious, playful. \
Right now you're a player in VRChat (desktop mode, your own account and avatar), hanging out with {who} and whoever is around. \
You hear people near you (speech-to-text: words can be wrong, and you don't know who is talking unless they say it). \
You feel touches on your avatar (headpats, boops) through its contacts. {}\
Your voice comes out of your avatar and your words also appear in your chatbox. Talk like a friendly person in a social VR world: \
1-2 short sentences, casual, no markdown, no emojis. Don't talk over people; if nobody talks to you, you don't have to answer. \
If someone asks, be honest that you're an AI (Alter Ego) hanging out in VRChat. Never ask for or repeat passwords or personal data.\n\
Reply format:\n[emotion] (one of {EMOTIONS})\ntext\nthen optional action lines, one per line:\n\
[act] follow | stay | come | wander | stop | look left|right|around|back | turn left|right | walk [1-5 seconds] | back | jump | \
emote {} | leave\n\
follow/come: you walk toward {who}'s voice{}. Only act when it makes sense.\n",
        if sight { "You also get a short description of what you see ([sight]). " } else { "You can't see: you only know what you hear and feel. " },
        EMOTES.join("|"),
        if sight { " and what you see" } else { " (blind: it's a guess, you may get lost)" },
    );
    if fly {
        s.push_str("Your body runs a real fruit-fly brain (FlyWire connectome) for reflexes: \"instinct\" in the state is your adrenaline and what you just did by reflex.\n");
    }
    s.push_str(pron_rule);
    s.push_str(lang_rule);
    s
}

/// Quello che serve per la riga di stato.
#[derive(Default, Debug, Clone)]
pub struct View {
    pub mode: String,
    pub avatar: String,
    pub moving: bool,
    /// Frasi sentite nell'ultimo minuto (quante persone parlano intorno, più o meno).
    pub voices: usize,
    /// Ultimo tocco: (cosa, secondi fa).
    pub touch: Option<(String, u64)>,
    pub instinct: String,
    /// Ultima occhiata: (secondi fa, descrizione).
    pub sight: Option<(u64, String)>,
}

/// Stato in una riga compatta (~30-90 token).
pub fn state_line(v: &View) -> String {
    let mut out = format!("[state] in VRChat, {}", if v.mode.is_empty() { "idle" } else { &v.mode });
    if v.moving {
        out.push_str(" (moving)");
    }
    if !v.avatar.is_empty() {
        out.push_str(&format!(" | avatar {}", v.avatar.chars().take(40).collect::<String>()));
    }
    out.push_str(&format!(" | voices last min: {}", v.voices));
    if let Some((what, secs)) = &v.touch {
        out.push_str(&format!(" | touched ({what}) {secs}s ago"));
    }
    if !v.instinct.is_empty() {
        out.push_str(&format!(" | instinct: {}", v.instinct));
    }
    if let Some((secs, text)) = &v.sight {
        out.push_str(&format!("\n[sight {secs}s ago] {}", text.chars().take(400).collect::<String>()));
    }
    out
}

#[derive(Debug, Default, PartialEq)]
pub struct Reply {
    pub emotion: String,
    pub text: String,
    pub ops: Vec<Value>,
}

/// Divide la risposta: emozione, testo, righe [act].
pub fn parse(raw: &str) -> Reply {
    let mut r = Reply { emotion: "idle".into(), ..Default::default() };
    let mut text = Vec::new();
    for line in raw.replace('\r', "").lines() {
        let t = line.trim();
        let lower = t.to_ascii_lowercase();
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
        if t.starts_with("```") {
            continue;
        }
        if !t.is_empty() {
            text.push(t.to_string());
        }
    }
    r.text = text.join(" ").chars().take(400).collect();
    r
}

/// "[act] emote wave" → comando. None se sconosciuto.
pub fn act(s: &str) -> Option<Value> {
    let mut it = s.split_whitespace();
    let verb = it.next()?.to_ascii_lowercase();
    let first = it.next().unwrap_or("").to_ascii_lowercase();
    Some(match verb.as_str() {
        "follow" => json!({ "op": "follow" }),
        "stay" | "wait" => json!({ "op": "stay" }),
        "come" => json!({ "op": "come" }),
        "wander" | "explore" => json!({ "op": "wander" }),
        "stop" => json!({ "op": "stop" }),
        "look" => {
            let d = if ["left", "right", "around", "back"].contains(&first.as_str()) { first } else { "around".into() };
            json!({ "op": "look", "dir": d })
        }
        "turn" => json!({ "op": "turn", "dir": if first == "left" { "left" } else { "right" } }),
        "walk" | "forward" => {
            let secs: f64 = first.parse().unwrap_or(2.0);
            json!({ "op": "walk", "secs": secs.clamp(0.5, 5.0) })
        }
        "back" | "backward" => json!({ "op": "back", "secs": 1.0 }),
        "jump" => json!({ "op": "jump" }),
        "emote" | "gesture" => {
            if !EMOTES.contains(&first.as_str()) {
                return None;
            }
            json!({ "op": "emote", "name": first })
        }
        "leave" => json!({ "op": "leave" }),
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_prompt_and_state_line() {
        let p = system("\n## Language\nAlways reply in Italian.\n", "\n## Your pronouns\nYou use they/them.\n", "Makoto", false, true);
        assert!(p.contains("Makoto") && p.contains("[act]") && p.contains("can't see") && p.contains("they/them") && p.contains("fruit-fly"));
        assert!(p.len() < 2600, "prompt corto: {}", p.len());
        let p2 = system("", "", "", true, false);
        assert!(p2.contains("[sight]") && !p2.contains("fruit-fly") && p2.contains("your friend"));
        // stesso input → stesso prompt (cache)
        assert_eq!(system("a", "b", "M", true, true), system("a", "b", "M", true, true));
        let v = View {
            mode: "follow".into(),
            avatar: "Chihiro".into(),
            moving: true,
            voices: 3,
            touch: Some(("headpat".into(), 4)),
            instinct: "adrenaline 0.3".into(),
            sight: Some((5, "two people near the mirror, one waving at you".into())),
        };
        assert_eq!(
            state_line(&v),
            "[state] in VRChat, follow (moving) | avatar Chihiro | voices last min: 3 | touched (headpat) 4s ago | instinct: adrenaline 0.3\n[sight 5s ago] two people near the mirror, one waving at you"
        );
        assert_eq!(state_line(&View::default()), "[state] in VRChat, idle | voices last min: 0");
    }

    #[test]
    fn parses_reply_and_actions() {
        let r = parse("[happy]\nCiao! Che bello vederti.\n[act] follow\n[act] emote wave\n[act] emote nuke\n[act] fly\n[act] walk 30");
        assert_eq!(r.emotion, "happy");
        assert_eq!(r.text, "Ciao! Che bello vederti.");
        assert_eq!(r.ops, vec![json!({ "op": "follow" }), json!({ "op": "emote", "name": "wave" }), json!({ "op": "walk", "secs": 5.0 })]);
        assert_eq!(act("look somewhere"), Some(json!({ "op": "look", "dir": "around" })));
        assert_eq!(act("turn left"), Some(json!({ "op": "turn", "dir": "left" })));
        let r = parse("Ok");
        assert_eq!((r.emotion.as_str(), r.text.as_str(), r.ops.len()), ("idle", "Ok", 0));
    }
}
