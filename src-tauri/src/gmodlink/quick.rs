//! Comandi semplici riconosciuti a parole chiave: partono subito e non costano token
//! ("seguimi", "fermati", "vieni qui", "salta"…). Solo frasi corte: il resto va al modello.

use serde_json::{json, Value};

pub struct Quick {
    pub ops: Vec<Value>,
    pub emotion: &'static str,
    pub reply: String,
}

/// Toglie punteggiatura, il nome di Alter Ego e le parole di cortesia.
fn normalize(text: &str, name: &str) -> String {
    let mut t = format!(" {} ", text.to_lowercase().replace(['!', '?', '.', ',', ';', ':', '"', '\'', '…'], " "));
    for w in [name.to_lowercase().as_str(), "alter ego", "alterego", "alter", "chihiro", "per favore", "perfavore", "please", "pls", "dai", "ora", "subito", "now"] {
        if !w.trim().is_empty() {
            t = t.replace(&format!(" {w} "), " ");
        }
    }
    t.split_whitespace().collect::<Vec<_>>().join(" ")
        .replace('à', "a").replace('ì', "i")
}

fn pick(opts: &[(&'static str, &'static str)], en: bool, seed: usize) -> String {
    let (it, e) = opts[seed % opts.len()];
    (if en { e } else { it }).to_string()
}

/// Comando rapido per `text` detto da `who` (None = va al modello). `in_game`: Alter Ego è nel gioco.
pub fn find(text: &str, who: &str, name: &str, in_game: bool, en: bool) -> Option<Quick> {
    let t = normalize(text, name);
    if t.is_empty() || t.split(' ').count() > 5 {
        return None;
    }
    let seed = text.len();
    let is = |list: &[&str]| list.contains(&t.as_str());
    if !in_game {
        if is(&["entra", "vieni a giocare", "entra in gioco", "entra nel gioco", "gioca con me", "join", "come play", "come and play", "play with me"]) {
            return Some(Quick {
                ops: vec![json!({ "op": "join" })],
                emotion: "happy",
                reply: pick(&[("Arrivo! Ehehe…", "Coming! Ehehe…"), ("Eccomi, sto entrando!", "Here I come!")], en, seed),
            });
        }
        return None;
    }
    let (ops, emotion, opts): (Vec<Value>, &str, &[(&str, &str)]) = if is(&["seguimi", "vieni con me", "seguitemi", "follow me", "follow"]) {
        (vec![json!({ "op": "follow", "who": who })], "happy", &[("Ti seguo!", "Following you!"), ("Arrivo, ti sto dietro!", "Right behind you!")])
    } else if is(&["fermati", "aspetta", "aspettami", "stai qui", "resta qui", "stai ferma", "ferma", "stay", "wait", "stay here", "wait here"]) {
        (vec![json!({ "op": "stay" })], "idle", &[("Ok, resto qui.", "Okay, I'll stay here."), ("Va bene, ti aspetto.", "Alright, I'll wait.")])
    } else if is(&["vieni qui", "vieni qua", "vieni", "come here", "come", "over here"]) {
        (vec![json!({ "op": "come", "who": who })], "happy", &[("Arrivo!", "Coming!"), ("Eccomi!", "On my way!")])
    } else if is(&["vai li", "vai la", "vai lì", "vai là", "go there", "go over there"]) {
        (vec![json!({ "op": "goto_aim", "who": who })], "idle", &[("Vado!", "Going!"), ("Ok, ci vado.", "Okay, heading there.")])
    } else if is(&["salta", "jump"]) {
        (vec![json!({ "op": "jump" })], "happy", &[("Hop!", "Hop!"), ("Ehehe, così?", "Ehehe, like this?")])
    } else if is(&["balla", "dance", "balliamo"]) {
        (vec![json!({ "op": "dance" })], "embarrassed", &[("Ehm… va bene, ma non ridere!", "Um… okay, but don't laugh!"), ("Ehehe… guarda!", "Ehehe… watch this!")])
    } else if is(&["saluta", "wave", "say hi"]) {
        (vec![json!({ "op": "gesture", "name": "wave" })], "happy", &[("Ciao ciao!", "Hi hi!"), ("Ciao!", "Hello!")])
    } else if is(&["lascia", "lascialo", "mollalo", "mollala", "lasciala", "drop", "drop it"]) {
        (vec![json!({ "op": "drop" })], "idle", &[("Fatto.", "Done."), ("Ok, lo lascio.", "Okay, letting go.")])
    } else if is(&["lancia", "lancialo", "lanciala", "throw", "throw it"]) {
        (vec![json!({ "op": "throw" })], "happy", &[("Via!", "There it goes!"), ("Hop, lanciato!", "Thrown!")])
    } else if is(&["basta", "stop", "smettila", "fermati tutto", "stop everything"]) {
        (vec![json!({ "op": "stop" })], "idle", &[("Ok, mi fermo.", "Okay, stopping."), ("Fermo tutto!", "Stopping everything!")])
    } else if is(&["esci", "vattene", "esci dal gioco", "leave", "go away", "ciao ciao esci"]) {
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
        let q = find("Alter Ego, seguimi!", "Makoto", "Alter Ego", true, false).unwrap();
        assert_eq!(q.ops[0], json!({ "op": "follow", "who": "Makoto" }));
        assert!(find("vieni qui per favore", "Makoto", "Alter Ego", true, false).is_some_and(|q| q.ops[0]["op"] == "come"));
        assert!(find("Vai lì", "M", "Alter Ego", true, true).is_some_and(|q| q.ops[0]["op"] == "goto_aim" && q.reply.is_ascii()));
        // frasi lunghe o richieste vere: al modello
        assert!(find("seguimi e quando vedi uno zombie sparagli", "M", "Alter Ego", true, false).is_none());
        assert!(find("costruisci una torre", "M", "Alter Ego", true, false).is_none());
        // fuori dal gioco solo "entra"
        assert!(find("seguimi", "M", "Alter Ego", false, false).is_none());
        assert!(find("vieni a giocare!", "M", "Alter Ego", false, false).is_some_and(|q| q.ops[0]["op"] == "join"));
    }
}
