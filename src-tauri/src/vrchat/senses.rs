//! Sensi dall'OSC: i parametri dei contatti e dei PhysBones dell'avatar diventano i sensi del
//! cervello di moscerino (tocco, dolore, qualcosa vicino a sinistra/destra, dolce), e le uscite del
//! cervello diventano riflessi del corpo (passo indietro, scatto della testa). Zero token.

use std::collections::HashMap;

/// Sensi del moscerino che hanno senso in VRChat.
pub const SENSES: &[&str] = &["touch", "pain", "loom_l", "loom_r", "sweet"];

/// Intensità dei sensi adesso (massimo tra i parametri collegati allo stesso senso).
pub fn from_params(params: &HashMap<String, f32>, map: &[(String, String)]) -> HashMap<String, f32> {
    let mut s: HashMap<String, f32> = HashMap::new();
    for (p, sense) in map {
        if !SENSES.contains(&sense.as_str()) {
            continue;
        }
        let Some(v) = params.get(p) else { continue };
        let mut v = v.abs().clamp(0.0, 1.0);
        // PhysBone tirato: fa male solo se tirato forte
        if p.to_lowercase().ends_with("_stretch") {
            v = ((v - 0.4) / 0.6).clamp(0.0, 1.0);
        }
        let e = s.entry(sense.clone()).or_insert(0.0);
        *e = e.max(v);
    }
    s.retain(|_, v| *v > 0.0);
    s
}

/// Nome umano del tocco dal nome del parametro.
pub fn touch_kind(param: &str) -> &'static str {
    let n = param.to_lowercase();
    if n.contains("pat") || n.contains("head") {
        "headpat"
    } else if n.contains("boop") || n.contains("nose") {
        "boop"
    } else if n.contains("hug") {
        "hug"
    } else if n.contains("grab") {
        "grab"
    } else if ["hit", "slap", "punch", "kick", "smack", "stretch"].iter().any(|k| n.contains(k)) {
        "hit"
    } else if n.contains("poke") || n.contains("tickle") {
        "poke"
    } else {
        "touch"
    }
}

/// Fronti di salita dei tocchi (un evento quando inizia, non a ogni pacchetto).
#[derive(Default)]
pub struct Edges {
    on: HashMap<String, bool>,
}

impl Edges {
    pub fn update(&mut self, params: &HashMap<String, f32>, map: &[(String, String)]) -> Vec<&'static str> {
        let mut out = Vec::new();
        for (p, sense) in map {
            if !["touch", "pain"].contains(&sense.as_str()) {
                continue;
            }
            let v = params.get(p).copied().unwrap_or(0.0).abs();
            let was = self.on.get(p).copied().unwrap_or(false);
            // isteresi: si accende sopra 0,5 e si spegne sotto 0,2
            let now = if was { v > 0.2 } else { v > 0.5 };
            if now && !was {
                out.push(touch_kind(p));
            }
            self.on.insert(p.clone(), now);
        }
        out
    }
}

/// Riflessi: uscite del cervello → ingressi di VRChat (nome, valore, millisecondi).
pub fn reflexes(out: &[(String, f32)]) -> Vec<(&'static str, f32, u64)> {
    let o = |k: &str| out.iter().find(|(n, _)| n == k).map(|(_, v)| *v).unwrap_or(0.0);
    let mut v = Vec::new();
    if o("escape") >= 0.6 {
        v.push(("MoveBackward", 1.0, 400));
    } else if o("backward") >= 0.5 {
        v.push(("MoveBackward", 1.0, 250));
    }
    let (l, r) = (o("turn_l"), o("turn_r"));
    if l.max(r) >= 0.5 {
        // si gira verso il lato del tocco/di quello che arriva (scatto breve della testa)
        v.push(("LookHorizontal", if l > r { -0.5 } else { 0.5 }, 220));
    }
    v
}

/// "happy=1, sad=2" → coppie (nome, valore). Righe sbagliate ignorate.
pub fn parse_map(s: &str) -> Vec<(String, i32)> {
    s.split([',', ';', '\n'])
        .filter_map(|p| {
            let (k, v) = p.split_once('=')?;
            let k = k.trim().to_lowercase();
            let v: i32 = v.trim().parse().ok()?;
            (!k.is_empty()).then_some((k, v))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn map() -> Vec<(String, String)> {
        [("HeadPat", "touch"), ("Contact_Boop", "touch"), ("Proximity_L", "loom_l"), ("Tail_Stretch", "pain"), ("Weird", "telepathy")]
            .iter()
            .map(|(a, b)| (a.to_string(), b.to_string()))
            .collect()
    }

    #[test]
    fn params_become_senses_and_touches() {
        let mut p: HashMap<String, f32> = HashMap::new();
        p.insert("HeadPat".into(), 1.0);
        p.insert("Contact_Boop".into(), 0.3);
        p.insert("Proximity_L".into(), 0.7);
        p.insert("Tail_Stretch".into(), 0.4);
        p.insert("Weird".into(), 1.0);
        let s = from_params(&p, &map());
        assert_eq!(s.get("touch"), Some(&1.0));
        assert_eq!(s.get("loom_l"), Some(&0.7));
        assert!(!s.contains_key("pain"), "tirato poco: niente dolore");
        assert!(!s.contains_key("telepathy"));
        p.insert("Tail_Stretch".into(), 1.0);
        assert_eq!(from_params(&p, &map()).get("pain"), Some(&1.0));
        // fronti di salita con isteresi
        let mut e = Edges::default();
        p.insert("HeadPat".into(), 0.0);
        p.insert("Tail_Stretch".into(), 0.0);
        assert!(e.update(&p, &map()).is_empty());
        p.insert("HeadPat".into(), 1.0);
        assert_eq!(e.update(&p, &map()), vec!["headpat"]);
        p.insert("HeadPat".into(), 0.4);
        assert!(e.update(&p, &map()).is_empty(), "ancora accarezzata: nessun evento nuovo");
        p.insert("HeadPat".into(), 0.0);
        e.update(&p, &map());
        p.insert("Contact_Boop".into(), 0.9);
        assert_eq!(e.update(&p, &map()), vec!["boop"]);
        assert_eq!(touch_kind("Tail_Stretch"), "hit");
        assert_eq!(touch_kind("Hug_Contact"), "hug");
    }

    #[test]
    fn reflexes_and_maps() {
        let r = reflexes(&[("escape".into(), 0.9), ("turn_r".into(), 0.7)]);
        assert_eq!(r, vec![("MoveBackward", 1.0, 400), ("LookHorizontal", 0.5, 220)]);
        assert!(reflexes(&[("groom".into(), 1.0), ("escape".into(), 0.2)]).is_empty());
        assert_eq!(parse_map("happy=1, sad = 2;bad;x=y;\nWave=3"), vec![("happy".into(), 1), ("sad".into(), 2), ("wave".into(), 3)]);
    }
}
