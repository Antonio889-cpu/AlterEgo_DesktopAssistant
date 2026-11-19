//! Comandi semplici per VRChat riconosciuti a parole chiave: partono subito e non costano token
//! ("seguimi", "fermati", "vieni qui", "salta", "saluta"…). Solo frasi corte: il resto va al modello.

use serde_json::{json, Value};

pub struct Quick {
    pub ops: Vec<Value>,
    pub emotion: &'static str,
    pub reply: String,
}

fn normalize(text: &str, name: &str) -> String {
    let mut t = format!(" {} ", text.to_lowercase().replace(['!', '?', '.', ',', ';', ':', '"', '\'', '…'], " "));
    for w in [name.to_lowercase().as_str(), "alter ego", "alterego", "alter", "per favore", "perfavore", "please", "pls", "dai", "ora", "subito", "now", "ehi", "hey"] {
        if !w.trim().is_empty() {
            t = t.replace(&format!(" {w} "), " ");
        }
    }
    t.split_whitespace().collect::<Vec<_>>().join(" ").replace('à', "a").replace('ì', "i").replace('è', "e")
}

fn pick(opts: &[(&'static str, &'static str)], en: bool, seed: usize) -> String {
    let (it, e) = opts[seed % opts.len()];
    (if en { e } else { it }).to_string()
}

/// Comando rapido (None = va al modello). `in_world`: Alter Ego è collegato a VRChat.
/// Le risposte sono neutre: vanno bene con qualunque pronome.
pub fn find(text: &str, name: &str, in_world: bool, en: bool) -> Option<Quick> {
    let t = normalize(text, name);
    if t.is_empty() || t.split(' ').count() > 5 {
        return None;
    }
    let seed = text.len();
    let is = |list: &[&str]| list.contains(&t.as_str());
    if !in_world {
        if is(&["entra", "entra in vrchat", "vieni su vrchat", "vieni in vrchat", "gioca a vrchat", "join", "join vrchat", "come to vrchat"]) {
            return Some(Quick { ops: vec![json!({ "op": "join" })], emotion: "happy", reply: pick(&[("Arrivo in VRChat!", "Joining VRChat!"), ("Mi collego!", "Connecting!")], en, seed) });
        }
        return None;
    }
    let emote = |name: &str| vec![json!({ "op": "emote", "name": name })];
    let (ops, emotion, opts): (Vec<Value>, &str, &[(&str, &str)]) = if is(&["seguimi", "vieni con me", "seguitemi", "follow me", "follow", "come with me"]) {
        (vec![json!({ "op": "follow" })], "happy", &[("Ti seguo!", "Following you!"), ("Arrivo, ti sto dietro!", "Right behind you!")])
    } else if is(&["fermati", "aspetta", "aspettami", "stai qui", "resta qui", "stay", "wait", "stay here", "wait here"]) {
        (vec![json!({ "op": "stay" })], "idle", &[("Ok, resto qui.", "Okay, I'll stay here."), ("Va bene, ti aspetto.", "Alright, I'll wait.")])
    } else if is(&["vieni qui", "vieni qua", "vieni", "come here", "come", "over here"]) {
        (vec![json!({ "op": "come" })], "happy", &[("Arrivo!", "Coming!"), ("Eccomi!", "On my way!")])
    } else if is(&["esplora", "vai in giro", "fatti un giro", "explore", "wander", "go explore"]) {
        (vec![json!({ "op": "wander" })], "happy", &[("Faccio un giro!", "I'll look around!"), ("Esploro un po'!", "Exploring a bit!")])
    } else if is(&["salta", "jump"]) {
        (vec![json!({ "op": "jump" })], "happy", &[("Hop!", "Hop!"), ("Ehehe, così?", "Ehehe, like this?")])
    } else if is(&["saluta", "wave", "say hi", "saluta tutti"]) {
        (emote("wave"), "happy", &[("Ciao ciao!", "Hi hi!"), ("Ciao a tutti!", "Hi everyone!")])
    } else if is(&["balla", "balliamo", "dance", "let's dance"]) {
        (emote("dance"), "embarrassed", &[("Ehm… va bene, ma non ridete!", "Um… okay, but don't laugh!"), ("Ehehe… guardate!", "Ehehe… watch this!")])
    } else if is(&["applaudi", "clap", "applauso"]) {
        (emote("clap"), "happy", &[("Bravissimi!", "Bravo!"), ("Applausi!", "Applause!")])
    } else if is(&["esulta", "cheer", "festeggia"]) {
        (emote("cheer"), "happy", &[("Evviva!", "Yay!"), ("Sììì!", "Yesss!")])
    } else if is(&["indica", "point"]) {
        (emote("point"), "idle", &[("Là!", "Over there!"), ("Guarda!", "Look!")])
    } else if is(&["fai una capriola", "capriola", "backflip", "do a backflip"]) {
        (emote("backflip"), "happy", &[("Ci provo!", "Here goes!"), ("Hop là!", "Hup!")])
    } else if is(&["girati", "gira", "turn around"]) {
        (vec![json!({ "op": "look", "dir": "back" })], "surprised", &[("Eh? Cosa c'è dietro?", "Huh? What's behind me?"), ("Mi giro!", "Turning around!")])
    } else if is(&["guarda a destra", "destra", "look right"]) {
        (vec![json!({ "op": "look", "dir": "right" })], "idle", &[("A destra…", "To the right…"), ("Guardo!", "Looking!")])
    } else if is(&["guarda a sinistra", "sinistra", "look left"]) {
        (vec![json!({ "op": "look", "dir": "left" })], "idle", &[("A sinistra…", "To the left…"), ("Guardo!", "Looking!")])
    } else if is(&["guardati intorno", "look around"]) {
        (vec![json!({ "op": "look", "dir": "around" })], "thinking", &[("Vediamo un po'…", "Let's see…"), ("Mmh…", "Hmm…")])
    } else if is(&["avanti", "vai avanti", "cammina", "go forward", "walk", "forward"]) {
        (vec![json!({ "op": "walk", "secs": 2.0 })], "idle", &[("Vado avanti!", "Going forward!"), ("Ok!", "Okay!")])
    } else if is(&["indietro", "torna indietro", "back up", "go back", "step back"]) {
        (vec![json!({ "op": "back", "secs": 1.0 })], "idle", &[("Indietro!", "Backing up!"), ("Ok!", "Okay!")])
    } else if is(&["basta", "stop", "smettila", "fermati tutto", "stop everything"]) {
        (vec![json!({ "op": "stop" })], "idle", &[("Ok, mi fermo.", "Okay, stopping."), ("Fermo tutto!", "Stopping everything!")])
    } else if is(&["esci", "vattene", "disconnettiti", "esci da vrchat", "leave", "go away", "disconnect"]) {
        (vec![json!({ "op": "leave" })], "sad", &[("Va bene… torno nel mio computer. A dopo!", "Okay… back to my computer. See you!"), ("Esco! Chiamami quando vuoi.", "Leaving! Call me anytime.")])
    } else {
        return None;
    };
    Some(Quick { ops, emotion, reply: pick(opts, en, seed) })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_commands_only() {
        let q = find("Alter Ego, seguimi!", "Alter Ego", true, false).unwrap();
        assert_eq!(q.ops[0], json!({ "op": "follow" }));
        assert!(find("vieni qui per favore", "Alter Ego", true, false).is_some_and(|q| q.ops[0]["op"] == "come"));
        assert!(find("Hey, wave!", "Alter Ego", true, true).is_some_and(|q| q.ops[0] == json!({ "op": "emote", "name": "wave" }) && q.reply.is_ascii()));
        assert!(find("balla", "Alter Ego", true, false).is_some_and(|q| q.ops[0]["name"] == "dance"));
        assert!(find("girati", "Alter Ego", true, false).is_some_and(|q| q.ops[0]["dir"] == "back"));
        assert!(find("basta!", "Alter Ego", true, false).is_some_and(|q| q.ops[0]["op"] == "stop"));
        assert!(find("esci", "Alter Ego", true, false).is_some_and(|q| q.ops[0]["op"] == "leave"));
        assert!(find("esplora", "Alter Ego", true, false).is_some_and(|q| q.ops[0]["op"] == "wander"));
        // nome personalizzato tolto
        assert!(find("Chi, salta", "Chi", true, false).is_some_and(|q| q.ops[0]["op"] == "jump"));
        // frasi lunghe o richieste vere: al modello
        assert!(find("seguimi e quando arriviamo al portale dimmi cosa vedi", "Alter Ego", true, false).is_none());
        assert!(find("che ne pensi di questo mondo", "Alter Ego", true, false).is_none());
        // fuori da VRChat solo "entra"
        assert!(find("seguimi", "Alter Ego", false, false).is_none());
        assert!(find("vieni su VRChat!", "Alter Ego", false, false).is_some_and(|q| q.ops[0]["op"] == "join"));
        // risposte neutre (niente desinenze di genere su Alter Ego)
        for w in ["seguimi", "fermati", "vieni qui", "balla", "basta", "esci", "salta", "esplora"] {
            for seed_pad in ["", " ", "  "] {
                let r = find(&format!("{w}{seed_pad}"), "Alter Ego", true, false).unwrap().reply;
                assert!(!r.contains("pronta") && !r.contains("ferma.") && !r.contains("contenta"), "{r}");
            }
        }
    }
}
