//! Comandi di movimento di VRChat (/input/*) con il "cane da guardia".
//!
//! La documentazione di VRChat avverte: un MoveForward lasciato a 1 cammina per sempre. Qui ogni
//! ingresso ha una scadenza (al massimo `MAX_HOLD`), `tick` rimette a zero quelli scaduti e
//! `zero_all` azzera tutto (stop, errori, uscita dall'app, panico del thread).

use super::osc::{Arg, Msg};
use std::collections::BTreeMap;
use std::time::{Duration, Instant};

/// Pulsanti (int 0/1).
pub const BUTTONS: &[&str] = &[
    "MoveForward", "MoveBackward", "MoveLeft", "MoveRight", "LookLeft", "LookRight", "Jump", "Run", "Voice",
    "ComfortLeft", "ComfortRight", "UseRight", "GrabRight", "DropRight",
];
/// Assi (float -1..1).
pub const AXES: &[&str] = &["Vertical", "Horizontal", "LookHorizontal", "UseAxisRight", "GrabAxisRight", "MoveHoldFB", "SpinHoldCwCcw", "SpinHoldUD", "SpinHoldLR"];

/// Nessun ingresso resta acceso più di così (anche se nessuno lo spegne).
pub const MAX_HOLD: Duration = Duration::from_secs(8);
/// Il microfono in push-to-talk può restare premuto di più (frasi lunghe), ma non per sempre.
pub const MAX_VOICE: Duration = Duration::from_secs(45);

pub fn is_axis(name: &str) -> bool {
    AXES.contains(&name)
}

fn known(name: &str) -> Option<&'static str> {
    BUTTONS.iter().chain(AXES.iter()).find(|n| **n == name).copied()
}

fn msg(name: &str, v: f32) -> Msg {
    let arg = if is_axis(name) { Arg::Float(v.clamp(-1.0, 1.0)) } else { Arg::Int(if v > 0.5 { 1 } else { 0 }) };
    Msg::new(&format!("/input/{name}"), vec![arg])
}

#[derive(Default)]
pub struct Inputs {
    /// Ingressi diversi da zero: valore e scadenza.
    on: BTreeMap<&'static str, (f32, Instant)>,
}

impl Inputs {
    /// Imposta un ingresso per `hold` (None = fino a nuovo ordine, ma mai oltre il limite).
    /// Restituisce il messaggio da mandare (None se il nome non esiste).
    pub fn set(&mut self, name: &str, value: f32, hold: Option<Duration>, now: Instant) -> Option<Msg> {
        let name = known(name)?;
        let cap = if name == "Voice" { MAX_VOICE } else { MAX_HOLD };
        if value == 0.0 {
            self.on.remove(name);
        } else {
            let d = hold.unwrap_or(cap).min(cap);
            self.on.insert(name, (value, now + d));
        }
        Some(msg(name, value))
    }

    /// Ingressi scaduti → messaggi a zero.
    pub fn tick(&mut self, now: Instant) -> Vec<Msg> {
        let expired: Vec<&'static str> = self.on.iter().filter(|(_, (_, t))| *t <= now).map(|(k, _)| *k).collect();
        expired
            .into_iter()
            .map(|k| {
                self.on.remove(k);
                msg(k, 0.0)
            })
            .collect()
    }

    /// Tutti gli ingressi conosciuti a zero (anche quelli che crediamo già spenti).
    pub fn zero_all(&mut self) -> Vec<Msg> {
        self.on.clear();
        BUTTONS.iter().chain(AXES.iter()).map(|k| msg(k, 0.0)).collect()
    }

    /// Valore attuale (0 se spento).
    #[cfg(test)]
    pub fn get(&self, name: &str) -> f32 {
        self.on.get(name).map(|x| x.0).unwrap_or(0.0)
    }

    /// Nomi accesi adesso (per lo stato).
    pub fn active(&self) -> Vec<&'static str> {
        self.on.keys().copied().collect()
    }

    #[cfg(test)]
    pub fn any_moving(&self) -> bool {
        self.on.keys().any(|k| *k != "Voice")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn watchdog_zeroes_everything() {
        let t0 = Instant::now();
        let mut i = Inputs::default();
        assert_eq!(i.set("MoveForward", 1.0, Some(Duration::from_millis(500)), t0), Some(msg("MoveForward", 1.0)));
        i.set("LookHorizontal", -0.6, Some(Duration::from_millis(300)), t0);
        // "per sempre" non esiste: al massimo MAX_HOLD
        i.set("Vertical", 1.0, None, t0);
        i.set("Voice", 1.0, Some(Duration::from_secs(600)), t0);
        assert!(i.set("Teleport", 1.0, None, t0).is_none(), "solo ingressi conosciuti");
        assert_eq!(i.active().len(), 4);
        // dopo 0,4 s: solo lo sguardo è scaduto
        let z = i.tick(t0 + Duration::from_millis(400));
        assert_eq!(z, vec![Msg::new("/input/LookHorizontal", vec![Arg::Float(0.0)])]);
        assert_eq!(i.tick(t0 + Duration::from_millis(600)), vec![Msg::new("/input/MoveForward", vec![Arg::Int(0)])]);
        assert_eq!(i.tick(t0 + MAX_HOLD), vec![Msg::new("/input/Vertical", vec![Arg::Float(0.0)])]);
        assert_eq!(i.get("Voice"), 1.0);
        assert_eq!(i.tick(t0 + MAX_VOICE).len(), 1);
        assert!(i.active().is_empty());
        // zero_all: ogni pulsante e ogni asse a zero, coi tipi giusti
        i.set("Run", 1.0, None, t0);
        let all = i.zero_all();
        assert_eq!(all.len(), BUTTONS.len() + AXES.len());
        for m in &all {
            let name = m.addr.strip_prefix("/input/").unwrap();
            match &m.args[..] {
                [Arg::Int(0)] => assert!(!is_axis(name), "{name}"),
                [Arg::Float(f)] => assert!(*f == 0.0 && is_axis(name), "{name}"),
                other => panic!("{name}: {other:?}"),
            }
        }
        assert!(all.iter().any(|m| m.addr == "/input/MoveForward") && all.iter().any(|m| m.addr == "/input/LookHorizontal"));
        assert!(i.active().is_empty() && !i.any_moving());
        // valori fuori scala tagliati
        assert_eq!(i.set("Horizontal", 3.0, None, t0).unwrap().args, vec![Arg::Float(1.0)]);
        assert_eq!(i.set("MoveLeft", 0.0, None, t0).unwrap().args, vec![Arg::Int(0)]);
    }
}
