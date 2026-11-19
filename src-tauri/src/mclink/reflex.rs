//! Dalle uscite del cervello di moscerino (gmodlink/fly.rs) ai riflessi del corpo in Minecraft.
//! Il bot manda i sensi (dolore, mob o frecce in arrivo da un lato, creeper che sibila, esplosioni, cibo,
//! colpi di un giocatore, caduta, acqua, luce), qui il cervello reale li simula e i neuroni discendenti
//! diventano movimenti fatti dal bot: scappare, scartare di lato, indietreggiare, saltare, sobbalzare, mangiare.
//! Zero token.

use std::collections::HashMap;

/// Riflesso: (uscita del cervello, soglia, nome per il bot, pausa minima in secondi).
const MAP: &[(&str, f32, &str, f64)] = &[
    ("escape", 0.55, "flee", 2.5),
    ("turn_l", 0.5, "strafe_l", 1.2),
    ("turn_r", 0.5, "strafe_r", 1.2),
    ("backward", 0.5, "back", 1.5),
    ("brace", 0.5, "jump", 1.5),
    ("groom", 0.5, "flinch", 3.0),
    ("feed", 0.5, "eat", 10.0),
    ("drink", 0.5, "swim_up", 2.0),
    ("listen", 0.5, "look_sound", 4.0),
    ("squint", 0.5, "look_away", 4.0),
];

/// Movimenti che si escludono a vicenda (le gambe sono due).
fn legs(k: &str) -> bool {
    matches!(k, "flee" | "strafe_l" | "strafe_r" | "back" | "swim_up")
}

/// Riflessi da fare adesso (al massimo due: uno di gambe e uno d'altro), più forte prima.
/// `last`: ultima volta (secondi) di ogni riflesso, aggiornato qui.
pub fn decide(out: &[(String, f32)], now: f64, last: &mut HashMap<&'static str, f64>) -> Vec<&'static str> {
    let get = |k: &str| out.iter().find(|(n, _)| n == k).map(|(_, v)| *v).unwrap_or(0.0);
    let mut cand: Vec<(f32, &'static str, f64)> = MAP.iter().filter(|(o, thr, _, _)| get(o) >= *thr).map(|(o, _, r, cd)| (get(o), *r, *cd)).collect();
    // scappare vale più di tutto il resto
    cand.sort_by(|a, b| (b.1 == "flee").cmp(&(a.1 == "flee")).then(b.0.total_cmp(&a.0)));
    let mut picked: Vec<&'static str> = Vec::new();
    for (_, r, cd) in cand {
        if last.get(r).is_some_and(|t| now - t < cd) {
            continue;
        }
        if legs(r) && picked.iter().any(|p| legs(p)) {
            continue;
        }
        if !legs(r) && picked.iter().any(|p| !legs(p)) {
            continue;
        }
        last.insert(r, now);
        picked.push(r);
    }
    picked
}

#[cfg(test)]
mod tests {
    use super::*;

    fn o(v: &[(&str, f32)]) -> Vec<(String, f32)> {
        v.iter().map(|(k, x)| (k.to_string(), *x)).collect()
    }

    #[test]
    fn reflexes_from_brain() {
        let mut last = HashMap::new();
        assert!(decide(&o(&[("escape", 0.2)]), 0.0, &mut last).is_empty(), "sotto soglia");
        // fuga + scarto: la fuga vince, sobbalzo insieme (non sono le gambe)
        let r = decide(&o(&[("turn_l", 0.9), ("escape", 0.6), ("groom", 0.7)]), 1.0, &mut last);
        assert_eq!(r, vec!["flee", "flinch"]);
        // subito dopo: in pausa, ma lo scarto adesso si può fare
        let r = decide(&o(&[("turn_l", 0.9), ("escape", 0.9), ("groom", 0.9)]), 1.5, &mut last);
        assert_eq!(r, vec!["strafe_l"]);
        // passata la pausa la fuga torna
        assert_eq!(decide(&o(&[("escape", 0.9)]), 4.0, &mut last), vec!["flee"]);
        // cibo: mangia, non più di una volta ogni 10 s
        assert_eq!(decide(&o(&[("feed", 0.8)]), 5.0, &mut last), vec!["eat"]);
        assert!(decide(&o(&[("feed", 0.8)]), 8.0, &mut last).is_empty());
        let mut l2 = HashMap::new();
        // due riflessi "non di gambe": solo il più forte
        assert_eq!(decide(&o(&[("brace", 0.6), ("listen", 0.9)]), 0.0, &mut l2), vec!["look_sound"]);
    }

    /// Con il connettoma vero: un mob che arriva da sinistra fa scappare, il dolce fa mangiare (dopo il filtro `gate`).
    #[test]
    fn real_brain_drives_reflexes() {
        use crate::gmodlink::fly;
        let mut b = fly::Brain::load().expect("flybrain.bin");
        let mut last = HashMap::new();
        let mut all: Vec<&str> = Vec::new();
        let senses: HashMap<String, f32> = [("loom_l".to_string(), 1.0), ("pain".to_string(), 0.6)].into();
        for i in 0..8 {
            b.run(100.0, &senses);
            let mut out = b.read(100.0);
            fly::gate(&mut out, &senses);
            all.extend(decide(&out, i as f64 * 0.1, &mut last));
        }
        assert!(all.contains(&"flee"), "{all:?}");
        assert!(!all.contains(&"eat"), "niente cibo: non mangia");
        for _ in 0..5 {
            b.run(100.0, &HashMap::new());
            b.read(100.0);
        }
        let sweet: HashMap<String, f32> = [("sweet".to_string(), 1.0)].into();
        let mut ate = false;
        for i in 0..8 {
            b.run(100.0, &sweet);
            let mut out = b.read(100.0);
            fly::gate(&mut out, &sweet);
            ate |= decide(&out, 100.0 + i as f64 * 0.1, &mut last).contains(&"eat");
        }
        assert!(ate);
    }
}
