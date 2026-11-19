//! Comportamento del corpo in VRChat: seguire, venire, girare per conto suo, guardarsi intorno.
//! Ogni azione è una sequenza di passi (ingresso, valore, durata); i passi scadono sempre da soli
//! (e il cane da guardia di input.rs li azzera comunque).
//! Senza vista segue "a orecchio": si gira verso il lato da cui arriva la voce e cammina finché la
//! voce non diventa forte (vicina). Con la vista usa la persona più vicina vista nello screenshot.

use std::collections::VecDeque;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Mode {
    Idle,
    Stay,
    Follow,
    /// Viene verso la voce finché è vicina o finché scade.
    Come(Instant),
    Wander,
}

impl Mode {
    pub fn name(&self) -> &'static str {
        match self {
            Mode::Idle => "idle",
            Mode::Stay => "stay",
            Mode::Follow => "follow",
            Mode::Come(_) => "come",
            Mode::Wander => "wander",
        }
    }
}

/// Un passo: ingresso, valore, durata, pausa dopo.
#[derive(Debug, Clone, PartialEq)]
pub struct Step {
    pub input: &'static str,
    pub value: f32,
    pub ms: u64,
    pub pause_ms: u64,
}

fn step(input: &'static str, value: f32, ms: u64, pause_ms: u64) -> Step {
    Step { input, value, ms, pause_ms }
}

/// Quello che il corpo sa adesso.
#[derive(Default, Debug, Clone)]
pub struct Cues {
    /// Ultima voce sentita: (id crescente, bilanciamento -1..1, volume).
    pub voice: Option<(u64, f32, f32)>,
    /// Persona più vicina vista: (id crescente, direzione -1..1, distanza "near"/"mid"/"far").
    pub sight: Option<(u64, f32, String)>,
    pub explore: bool,
    pub glances: bool,
}

/// Volume oltre cui una voce è "vicina" (RMS dopo il VAD).
pub const NEAR_LEVEL: f32 = 0.08;

pub struct Body {
    pub mode: Mode,
    plan: VecDeque<Step>,
    busy_until: Option<Instant>,
    next_idle: Option<Instant>,
    seen_voice: u64,
    seen_sight: u64,
    rng: u64,
}

impl Default for Body {
    fn default() -> Self {
        Self { mode: Mode::Idle, plan: VecDeque::new(), busy_until: None, next_idle: None, seen_voice: 0, seen_sight: 0, rng: 0x2545_F491_4F6C_DD1D }
    }
}

impl Body {
    fn rand(&mut self) -> f32 {
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 7;
        self.rng ^= self.rng << 17;
        (self.rng >> 40) as f32 / (1u64 << 24) as f32
    }

    pub fn seed(&mut self, s: u64) {
        self.rng = s | 1;
    }

    /// Cambia modo (svuota quello che stava facendo).
    pub fn set_mode(&mut self, m: Mode, now: Instant) {
        self.mode = m;
        self.plan.clear();
        self.busy_until = None;
        self.next_idle = Some(now + Duration::from_secs(4));
    }

    #[cfg(test)]
    pub fn busy(&self) -> bool {
        !self.plan.is_empty() || self.busy_until.is_some()
    }

    /// Azione singola chiesta dal modello o da un comando rapido (si accoda).
    pub fn act(&mut self, op: &str, dir: &str, secs: f32) {
        let side = if dir == "left" { -1.0 } else { 1.0 };
        let steps: Vec<Step> = match op {
            "look" => match dir {
                "around" => vec![step("LookHorizontal", -0.45, 600, 500), step("LookHorizontal", 0.45, 1200, 500), step("LookHorizontal", -0.45, 600, 0)],
                "back" => vec![step("LookHorizontal", 1.0, 1100, 200)],
                _ => vec![step("LookHorizontal", 0.5 * side, 400, 300)],
            },
            "turn" => vec![step("LookHorizontal", 0.6 * side, 500, 200)],
            "walk" => vec![step("Vertical", 1.0, (secs.clamp(0.5, 5.0) * 1000.0) as u64, 200)],
            "back" => vec![step("Vertical", -1.0, (secs.clamp(0.3, 3.0) * 1000.0) as u64, 200)],
            "jump" => vec![step("Jump", 1.0, 150, 250)],
            _ => Vec::new(),
        };
        self.plan.extend(steps);
    }

    /// Riflesso: passa davanti a tutto.
    pub fn reflex(&mut self, input: &'static str, value: f32, ms: u64) {
        self.plan.push_front(step(input, value, ms, 150));
        self.busy_until = None;
    }

    /// Un giro: restituisce il prossimo passo da eseguire (al più uno alla volta).
    pub fn tick(&mut self, now: Instant, c: &Cues) -> Option<Step> {
        if let Mode::Come(until) = self.mode {
            if now >= until {
                self.mode = Mode::Idle;
            }
        }
        if self.busy_until.is_some_and(|t| now < t) {
            return None;
        }
        self.busy_until = None;
        if self.plan.is_empty() {
            self.plan_next(now, c);
        }
        let s = self.plan.pop_front()?;
        self.busy_until = Some(now + Duration::from_millis(s.ms + s.pause_ms));
        Some(s)
    }

    fn plan_next(&mut self, now: Instant, c: &Cues) {
        match self.mode {
            Mode::Follow | Mode::Come(_) => {
                // con la vista: la persona più vicina
                if let Some((id, dir, dist)) = &c.sight {
                    if *id != self.seen_sight {
                        self.seen_sight = *id;
                        if dir.abs() > 0.3 {
                            self.plan.push_back(step("LookHorizontal", 0.45 * dir.signum(), 300, 150));
                        }
                        match dist.as_str() {
                            "far" => self.plan.push_back(step("Vertical", 1.0, 1800, 200)),
                            "mid" => self.plan.push_back(step("Vertical", 1.0, 900, 200)),
                            _ => {
                                if matches!(self.mode, Mode::Come(_)) {
                                    self.mode = Mode::Idle;
                                }
                            }
                        }
                        return;
                    }
                }
                // a orecchio: si gira verso la voce, cammina se è lontana
                if let Some((id, bal, level)) = c.voice {
                    if id != self.seen_voice {
                        self.seen_voice = id;
                        if bal.abs() > 0.2 {
                            let ms = (200.0 + 500.0 * bal.abs()) as u64;
                            self.plan.push_back(step("LookHorizontal", 0.5 * bal.signum(), ms, 200));
                        }
                        if level < NEAR_LEVEL {
                            self.plan.push_back(step("Vertical", 1.0, if level < NEAR_LEVEL / 3.0 { 1600 } else { 800 }, 200));
                        } else if matches!(self.mode, Mode::Come(_)) {
                            self.mode = Mode::Idle; // è arrivata
                        }
                    }
                }
            }
            Mode::Wander if c.explore => {
                if self.next_idle.is_some_and(|t| now < t) {
                    return;
                }
                let turn = self.rand() * 2.0 - 1.0;
                let tms = (200.0 + 600.0 * turn.abs()) as u64;
                self.plan.push_back(step("LookHorizontal", 0.5 * turn.signum(), tms, 300));
                let walk = (1000.0 + self.rand() * 2000.0) as u64;
                self.plan.push_back(step("Vertical", 1.0, walk, 300));
                if self.rand() < 0.08 {
                    self.plan.push_back(step("Jump", 1.0, 150, 300));
                }
                self.next_idle = Some(now + Duration::from_millis((4000.0 + self.rand() * 5000.0) as u64));
            }
            Mode::Idle | Mode::Wander if c.glances => {
                // occhiate ogni tanto, come una persona ferma
                let Some(t) = self.next_idle else {
                    self.next_idle = Some(now + Duration::from_secs(10));
                    return;
                };
                if now < t {
                    return;
                }
                let side = if self.rand() < 0.5 { -1.0 } else { 1.0 };
                let ms = (180.0 + self.rand() * 250.0) as u64;
                self.plan.push_back(step("LookHorizontal", 0.35 * side, ms, 900));
                self.plan.push_back(step("LookHorizontal", -0.35 * side, ms, 0));
                self.next_idle = Some(now + Duration::from_millis((12_000.0 + self.rand() * 18_000.0) as u64));
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(b: &mut Body, t0: Instant, secs: f32, c: &Cues) -> Vec<Step> {
        let mut out = Vec::new();
        let mut t = 0.0;
        while t < secs {
            if let Some(s) = b.tick(t0 + Duration::from_secs_f32(t), c) {
                out.push(s);
            }
            t += 0.1;
        }
        out
    }

    #[test]
    fn follows_by_ear() {
        let t0 = Instant::now();
        let mut b = Body::default();
        b.set_mode(Mode::Follow, t0);
        // voce lontana da destra: si gira a destra e cammina
        let c = Cues { voice: Some((1, 0.7, 0.01)), ..Default::default() };
        let s = run(&mut b, t0, 4.0, &c);
        assert_eq!(s.len(), 2, "{s:?}");
        assert_eq!((s[0].input, s[0].value), ("LookHorizontal", 0.5));
        assert_eq!((s[1].input, s[1].value, s[1].ms), ("Vertical", 1.0, 1600));
        // stessa voce: niente di nuovo
        assert!(run(&mut b, t0 + Duration::from_secs(5), 3.0, &c).is_empty());
        // voce vicina al centro: resta lì
        let c = Cues { voice: Some((2, 0.05, 0.2)), ..Default::default() };
        assert!(run(&mut b, t0 + Duration::from_secs(10), 3.0, &c).is_empty());
        // "vieni qui": arriva vicino e torna ferma
        b.set_mode(Mode::Come(t0 + Duration::from_secs(60)), t0 + Duration::from_secs(20));
        let c = Cues { voice: Some((3, -0.5, 0.2)), ..Default::default() };
        let s = run(&mut b, t0 + Duration::from_secs(20), 2.0, &c);
        assert_eq!(s.len(), 1);
        assert!(s[0].value < 0.0, "si gira a sinistra");
        assert_eq!(b.mode, Mode::Idle);
        // "vieni qui" scaduto
        b.set_mode(Mode::Come(t0), t0);
        b.tick(t0 + Duration::from_secs(1), &Cues::default());
        assert_eq!(b.mode, Mode::Idle);
    }

    #[test]
    fn follows_by_sight_and_wanders() {
        let t0 = Instant::now();
        let mut b = Body::default();
        b.set_mode(Mode::Follow, t0);
        let c = Cues { sight: Some((1, -1.0, "far".into())), voice: Some((9, 1.0, 0.01)), ..Default::default() };
        let s = run(&mut b, t0, 3.0, &c);
        assert_eq!(s[0].value, -0.45, "la vista vince sull'udito");
        assert_eq!((s[1].input, s[1].ms), ("Vertical", 1800));
        // a zonzo: giri e passi, sempre con durata finita
        let mut b = Body::default();
        b.seed(42);
        b.set_mode(Mode::Wander, t0);
        let s = run(&mut b, t0, 40.0, &Cues { explore: true, ..Default::default() });
        assert!(s.iter().filter(|x| x.input == "Vertical").count() >= 3, "{s:?}");
        assert!(s.iter().all(|x| x.ms > 0 && x.ms <= 5000 && x.value.abs() <= 1.0));
        // fermo: niente movimenti, solo occhiate se richieste
        let mut b = Body::default();
        b.set_mode(Mode::Stay, t0);
        assert!(run(&mut b, t0, 60.0, &Cues { explore: true, glances: true, ..Default::default() }).is_empty());
        b.set_mode(Mode::Idle, t0);
        let s = run(&mut b, t0, 60.0, &Cues { glances: true, ..Default::default() });
        assert!(!s.is_empty() && s.iter().all(|x| x.input == "LookHorizontal"));
        // azioni e riflessi
        let mut b = Body::default();
        b.set_mode(Mode::Stay, t0);
        b.act("walk", "", 30.0);
        b.reflex("MoveBackward", 1.0, 400);
        let s = run(&mut b, t0, 8.0, &Cues::default());
        assert_eq!(s[0].input, "MoveBackward", "il riflesso passa davanti");
        assert_eq!((s[1].input, s[1].ms), ("Vertical", 5000), "massimo 5 s");
        b.act("look", "around", 0.0);
        assert!(b.busy());
        b.set_mode(Mode::Idle, t0);
        assert!(!b.busy(), "cambio modo: piano svuotato");
    }
}
