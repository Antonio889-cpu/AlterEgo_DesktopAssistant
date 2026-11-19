//! Comandi semplici in Minecraft riconosciuti a parole chiave: partono subito e non costano token
//! ("seguimi", "fermati", "vieni qui", "proteggimi"…). Solo frasi corte: il resto va al modello.
//! (Stessa idea di gmodlink/quick.rs.)

use serde_json::{json, Value};

pub struct Quick {
    pub ops: Vec<Value>,
    pub emotion: &'static str,
    pub reply: String,
}

/// Toglie punteggiatura, accenti, il nome di Alter Ego e le parole di cortesia.
fn normalize(text: &str, name: &str) -> String {
    let mut t = format!(" {} ", text.to_lowercase().replace(['!', '?', '.', ',', ';', ':', '"', '\'', '…'], " "));
    for w in [name.to_lowercase().as_str(), "alter ego", "alterego", "alter", "chihiro", "per favore", "perfavore", "please", "pls", "dai", "ora", "subito", "now", "hey", "ehi"] {
        if !w.trim().is_empty() {
            t = t.replace(&format!(" {w} "), " ");
        }
    }
    t.split_whitespace().collect::<Vec<_>>().join(" ").replace(['à', 'á'], "a").replace(['ì', 'í'], "i").replace(['è', 'é'], "e")
}

fn pick(opts: &[(&'static str, &'static str)], en: bool, seed: usize) -> String {
    let (it, e) = opts[seed % opts.len()];
    (if en { e } else { it }).to_string()
}

/// Comando rapido per `text` detto da `who` (None = va al modello).
pub fn find(text: &str, who: &str, name: &str, en: bool) -> Option<Quick> {
    let t = normalize(text, name);
    if t.is_empty() || t.split(' ').count() > 5 {
        return None;
    }
    let seed = text.len();
    let is = |list: &[&str]| list.contains(&t.as_str());
    let (ops, emotion, opts): (Vec<Value>, &str, &[(&str, &str)]) = if is(&["seguimi", "vieni con me", "seguitemi", "follow me", "follow", "come with me"]) {
        (vec![json!({ "op": "follow", "who": who })], "happy", &[("ti seguo!", "following you!"), ("arrivo, ti sto dietro", "right behind you")])
    } else if is(&["fermati", "aspetta", "aspettami", "stai qui", "resta qui", "stai ferma", "stai fermo", "ferma", "stay", "wait", "stay here", "wait here"]) {
        (vec![json!({ "op": "stay" })], "idle", &[("ok resto qui", "ok, staying here"), ("va bene, ti aspetto", "alright, i'll wait")])
    } else if is(&["vieni qui", "vieni qua", "vieni", "come here", "come", "over here", "da me"]) {
        (vec![json!({ "op": "come", "who": who })], "happy", &[("arrivo!", "coming!"), ("eccomi!", "on my way!")])
    } else if is(&["proteggimi", "difendimi", "guardami le spalle", "protect me", "guard me", "cover me"]) {
        (vec![json!({ "op": "guard", "who": who })], "happy", &[("ti copro io!", "i got your back!"), ("ci penso io ai mob", "i'll handle the mobs")])
    } else if is(&["basta", "stop", "smettila", "fermati tutto", "lascia stare", "stop everything", "cancel"]) {
        (vec![json!({ "op": "stop" })], "idle", &[("ok mi fermo", "ok, stopping"), ("fermo tutto!", "stopping everything!")])
    } else if is(&["salta", "jump"]) {
        (vec![json!({ "op": "jump" })], "happy", &[("hop!", "hop!"), ("ehehe cosi?", "hehe like this?")])
    } else if is(&["mangia", "mangia qualcosa", "eat", "eat something"]) {
        (vec![json!({ "op": "eat" })], "happy", &[("gnam", "nom"), ("ok, spuntino", "ok, snack time")])
    } else if is(&["dormi", "vai a dormire", "a letto", "sleep", "go to sleep", "go to bed"]) {
        (vec![json!({ "op": "sleep" })], "happy", &[("buonanotte!", "good night!"), ("a nanna", "bedtime")])
    } else if is(&["raccogli legna", "prendi legna", "taglia alberi", "taglia legna", "get wood", "chop wood", "collect wood", "gather wood"]) {
        (vec![json!({ "op": "collect", "block": "wood", "n": 8 })], "happy", &[("vado a fare legna!", "getting some wood!"), ("ok, taglio qualche albero", "ok, chopping trees")])
    } else if is(&["raccogli pietra", "prendi pietra", "scava pietra", "get stone", "mine stone", "collect stone"]) {
        (vec![json!({ "op": "collect", "block": "stone", "n": 8 })], "happy", &[("vado a scavare!", "going mining!"), ("pietra in arrivo", "stone incoming")])
    } else if is(&["esplora", "vai a esplorare", "explore", "go explore"]) {
        (vec![json!({ "op": "explore", "dist": 50 })], "happy", &[("vado a dare un'occhiata!", "gonna look around!"), ("esploro un po'", "exploring a bit")])
    } else if is(&["esci", "vattene", "esci dal mondo", "disconnettiti", "leave", "go away", "disconnect"]) {
        (vec![json!({ "op": "leave" })], "sad", &[("ok... torno nel computer. a dopo!", "okay... back to my computer. see you!"), ("esco! chiamami quando vuoi", "leaving! call me anytime")])
    } else {
        return None;
    };
    Some(Quick { ops, emotion, reply: pick(opts, en, seed) })
}

/// "Entra in Minecraft" detto all'app quando Alter Ego non è ancora nel mondo.
pub fn is_join(text: &str) -> bool {
    let t = normalize(text, "");
    let mc = t.contains("minecraft");
    let join = ["entra", "vieni a giocare", "gioca con me", "giochiamo", "join", "come play", "play with me", "let s play", "lets play"].iter().any(|w| t.contains(w));
    mc && join && t.split(' ').count() <= 8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_commands_only() {
        let q = find("Alter Ego, seguimi!", "Makoto", "AlterEgo", false).unwrap();
        assert_eq!(q.ops[0], json!({ "op": "follow", "who": "Makoto" }));
        assert!(find("vieni qui per favore", "Makoto", "AlterEgo", false).is_some_and(|q| q.ops[0]["op"] == "come"));
        assert!(find("Follow me!", "Steve", "AlterEgo", true).is_some_and(|q| q.ops[0]["op"] == "follow" && q.reply.is_ascii()));
        assert!(find("stop", "Steve", "AlterEgo", true).is_some_and(|q| q.ops[0]["op"] == "stop"));
        assert!(find("ehi AlterEgo fermati", "Steve", "AlterEgo", false).is_some_and(|q| q.ops[0]["op"] == "stay"));
        assert!(find("proteggimi!", "Steve", "AlterEgo", false).is_some_and(|q| q.ops[0]["op"] == "guard" && q.ops[0]["who"] == "Steve"));
        assert!(find("raccogli legna", "S", "AlterEgo", false).is_some_and(|q| q.ops[0]["block"] == "wood"));
        // frasi lunghe o richieste vere: al modello
        assert!(find("seguimi e quando vedi uno zombie uccidilo", "M", "AlterEgo", false).is_none());
        assert!(find("costruiamo una casa", "M", "AlterEgo", false).is_none());
        assert!(find("", "M", "AlterEgo", false).is_none());
    }

    #[test]
    fn join_phrases() {
        assert!(is_join("Alter Ego, vieni a giocare a Minecraft!"));
        assert!(is_join("let's play minecraft"));
        assert!(!is_join("vieni a giocare"), "senza la parola minecraft: no (potrebbe essere un altro gioco)");
        assert!(!is_join("che cos'è minecraft"));
    }
}
