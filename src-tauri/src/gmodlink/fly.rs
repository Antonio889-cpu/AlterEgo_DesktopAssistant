//! Cervello di moscerino: i riflessi e l'istinto di Alter Ego in Garry's Mod.
//!
//! È un pezzo del connettoma reale di *Drosophila melanogaster* (FlyWire, v783, CC-BY 4.0:
//! Dorkenwald et al. e Schlegel et al., Nature 2024): i ~9.500 neuroni e le ~690.000 connessioni
//! che si attivano con i sensi usati dal gioco. Ogni neurone è un "leaky integrate-and-fire" con i
//! parametri di Shiu et al. (Nature 2024); il segno delle sinapsi viene dal neurotrasmettitore.
//! Il file si costruisce con `tools/flybrain/build.py`.
//!
//! Il gioco manda le intensità dei sensi (dolore, qualcosa che arriva addosso, dolce, tocco…), qui
//! si simula il cervello in tempo reale e si rimandano le attivazioni dei neuroni discendenti
//! (fuga, retromarcia, grooming, proboscide…). L'addon le trasforma in movimenti. Zero token.

use std::collections::HashMap;

static DATA: &[u8] = include_bytes!("../../resources/flybrain.bin");

// parametri del modello (Shiu et al. 2024)
const V0: f32 = -52.0;
const VRST: f32 = -52.0;
const VTH: f32 = -45.0;
const TMBR: f32 = 20.0;
const TAU: f32 = 5.0;
const TRFC: f32 = 2.2;
const WSYN: f32 = 0.275;
const RPOI: f32 = 150.0;
const FPOI: f32 = 250.0;
/// Passo di integrazione (ms).
const DT: f32 = 0.5;
/// Ritardo sinaptico in passi (1,8 ms ≈ 4 × 0,5 ms).
const DELAY: usize = 4;

/// Sensi predefiniti (sempre attivi) e opzionali (scelti nelle impostazioni).
pub const DEFAULT_SENSES: &[&str] = &["pain", "loom_l", "loom_r", "sweet", "touch"];
/// Sensi opzionali raggruppati come nelle impostazioni.
pub const OPTIONAL: &[(&str, &[&str])] = &[
    ("smell", &["smell_food", "smell_danger"]),
    ("thermo", &["heat", "cold", "wet", "water"]),
    ("hearing", &["sound"]),
    ("light", &["light"]),
    ("wind", &["wind"]),
];

/// Frequenza (Hz) a cui un gruppo di uscita vale 1.
fn full_rate(name: &str) -> f32 {
    match name {
        "escape" | "groom" | "feed" => 80.0,
        "turn_l" | "turn_r" => 60.0,
        _ => 50.0,
    }
}

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> f32 {
        // xorshift64*
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        ((self.0.wrapping_mul(0x2545_F491_4F6C_DD1D) >> 40) as f32) / (1u64 << 24) as f32
    }
}

/// Neuroni e sinapsi nel file (senza caricarlo).
pub fn size() -> (usize, usize) {
    let mut p = 6;
    (rd_u32(DATA, &mut p).unwrap_or(0) as usize, rd_u32(DATA, &mut p).unwrap_or(0) as usize)
}

pub struct Brain {
    n: usize,
    indptr: Vec<u32>,
    post: Vec<u16>,
    w: Vec<f32>,
    inputs: HashMap<String, Vec<u16>>,
    outputs: Vec<(String, Vec<u16>)>,
    v: Vec<f32>,
    x: Vec<f32>,
    refr: Vec<f32>,
    /// Corrente in arrivo fra 1..DELAY passi (anello) + chi la riceve.
    ring: Vec<Vec<f32>>,
    ring_hit: Vec<Vec<u16>>,
    active: Vec<u16>,
    is_active: Vec<bool>,
    step: usize,
    rng: Rng,
    /// Spike per neurone dall'ultima lettura delle uscite.
    spikes: Vec<u16>,
    total_spikes: u64,
    /// Uscite smussate 0..1.
    smooth: HashMap<String, f32>,
}

fn rd_u32(b: &[u8], p: &mut usize) -> Option<u32> {
    let v = u32::from_le_bytes(b.get(*p..*p + 4)?.try_into().ok()?);
    *p += 4;
    Some(v)
}
fn rd_u16(b: &[u8], p: &mut usize) -> Option<u16> {
    let v = u16::from_le_bytes(b.get(*p..*p + 2)?.try_into().ok()?);
    *p += 2;
    Some(v)
}

impl Brain {
    pub fn load() -> Option<Self> {
        Self::from_bytes(DATA)
    }

    pub fn from_bytes(b: &[u8]) -> Option<Self> {
        if b.get(..6)? != b"AEFLY1" {
            return None;
        }
        let mut p = 6;
        let n = rd_u32(b, &mut p)? as usize;
        let e = rd_u32(b, &mut p)? as usize;
        let mut indptr = Vec::with_capacity(n + 1);
        for _ in 0..=n {
            indptr.push(rd_u32(b, &mut p)?);
        }
        let mut post = Vec::with_capacity(e);
        for _ in 0..e {
            post.push(rd_u16(b, &mut p)?);
        }
        let mut w = Vec::with_capacity(e);
        for _ in 0..e {
            w.push(rd_u16(b, &mut p)? as i16 as f32 * WSYN);
        }
        let groups = rd_u16(b, &mut p)?;
        let (mut inputs, mut outputs) = (HashMap::new(), Vec::new());
        for _ in 0..groups {
            let len = *b.get(p)? as usize;
            p += 1;
            let name = std::str::from_utf8(b.get(p..p + len)?).ok()?.to_string();
            p += len;
            let k = rd_u16(b, &mut p)?;
            let mut ix = Vec::with_capacity(k as usize);
            for _ in 0..k {
                let i = rd_u16(b, &mut p)?;
                if (i as usize) < n {
                    ix.push(i);
                }
            }
            if let Some(s) = name.strip_prefix("in:") {
                inputs.insert(s.to_string(), ix);
            } else if let Some(s) = name.strip_prefix("out:") {
                outputs.push((s.to_string(), ix));
            }
        }
        if indptr.last().copied()? as usize != e || post.iter().any(|&j| j as usize >= n) {
            return None;
        }
        Some(Self {
            n,
            indptr,
            post,
            w,
            inputs,
            outputs,
            v: vec![V0; n],
            x: vec![0.0; n],
            refr: vec![0.0; n],
            ring: vec![vec![0.0; n]; DELAY + 1],
            ring_hit: vec![Vec::new(); DELAY + 1],
            active: Vec::new(),
            is_active: vec![false; n],
            step: 0,
            rng: Rng(0x9E37_79B9_7F4A_7C15),
            spikes: vec![0; n],
            total_spikes: 0,
            smooth: HashMap::new(),
        })
    }

    pub fn neurons(&self) -> usize {
        self.n
    }
    pub fn synapses(&self) -> usize {
        self.post.len()
    }

    fn wake(&mut self, j: usize) {
        if !self.is_active[j] {
            self.is_active[j] = true;
            self.active.push(j as u16);
        }
    }

    /// Avanza di `ms` millisecondi con i sensi dati (nome → intensità 0..1).
    pub fn run(&mut self, ms: f32, senses: &HashMap<String, f32>) {
        let stim: Vec<(Vec<u16>, f32)> = senses
            .iter()
            .filter(|(_, r)| **r > 0.0)
            .filter_map(|(k, r)| self.inputs.get(k).map(|ix| (ix.clone(), RPOI * r.min(1.0) * DT / 1000.0)))
            .collect();
        let steps = (ms / DT).round() as usize;
        let dv = DT / TMBR;
        let dx = (-DT / TAU).exp();
        for _ in 0..steps {
            let slot = self.step % (DELAY + 1);
            // corrente arrivata adesso
            let hits = std::mem::take(&mut self.ring_hit[slot]);
            for &j in &hits {
                let j = j as usize;
                self.x[j] += self.ring[slot][j];
                self.ring[slot][j] = 0.0;
                self.wake(j);
            }
            self.ring_hit[slot] = hits;
            self.ring_hit[slot].clear();
            // ingressi di Poisson (neuroni sensoriali)
            for (ix, p) in &stim {
                for &j in ix {
                    if self.rng.next() < *p {
                        self.v[j as usize] += FPOI * WSYN;
                        if !self.is_active[j as usize] {
                            self.is_active[j as usize] = true;
                            self.active.push(j);
                        }
                    }
                }
            }
            // integrazione dei soli neuroni attivi
            let mut fired: Vec<u16> = Vec::new();
            let mut k = 0;
            while k < self.active.len() {
                let j = self.active[k] as usize;
                if self.refr[j] > 0.0 {
                    self.refr[j] -= DT;
                } else {
                    self.v[j] += dv * (self.x[j] - (self.v[j] - V0));
                }
                self.x[j] *= dx;
                if self.v[j] >= VTH && self.refr[j] <= 0.0 {
                    self.v[j] = VRST;
                    self.refr[j] = TRFC;
                    fired.push(j as u16);
                    self.spikes[j] = self.spikes[j].saturating_add(1);
                }
                // a riposo: esce dall'elenco
                if self.refr[j] <= 0.0 && self.x[j].abs() < 1e-3 && (self.v[j] - V0).abs() < 1e-2 {
                    self.v[j] = V0;
                    self.x[j] = 0.0;
                    self.is_active[j] = false;
                    self.active.swap_remove(k);
                    continue;
                }
                k += 1;
            }
            self.total_spikes += fired.len() as u64;
            let tgt = (self.step + DELAY) % (DELAY + 1);
            for &i in &fired {
                let (a, b) = (self.indptr[i as usize] as usize, self.indptr[i as usize + 1] as usize);
                for e in a..b {
                    let j = self.post[e] as usize;
                    if self.ring[tgt][j] == 0.0 {
                        self.ring_hit[tgt].push(j as u16);
                    }
                    self.ring[tgt][j] += self.w[e];
                    // somma esattamente zero: resta in elenco comunque (innocuo)
                }
            }
            self.step += 1;
        }
    }

    /// Attivazioni delle uscite (0..1) da quando sono state lette l'ultima volta, su `ms` millisecondi.
    pub fn read(&mut self, ms: f32) -> Vec<(String, f32)> {
        let mut out = Vec::new();
        for (name, ix) in &self.outputs {
            let n = ix.len().max(1) as f32;
            let s: u32 = ix.iter().map(|&j| self.spikes[j as usize] as u32).sum();
            let hz = s as f32 / n / (ms / 1000.0).max(0.001);
            let now = (hz / full_rate(name)).clamp(0.0, 1.0);
            let e = self.smooth.entry(name.clone()).or_insert(0.0);
            // sale subito, scende in ~0,3 s
            *e = if now > *e { now } else { *e * 0.6 + now * 0.4 };
            out.push((name.clone(), (*e * 100.0).round() / 100.0));
        }
        self.spikes.iter_mut().for_each(|s| *s = 0);
        out
    }

    /// Spike totali (per l'attività mostrata sopra la testa).
    pub fn take_total(&mut self) -> u64 {
        std::mem::take(&mut self.total_spikes)
    }

    #[cfg(test)]
    pub fn active_count(&self) -> usize {
        self.active.len()
    }
}

/// Alter Ego non è una mosca: un'uscita del cervello conta solo se c'è davvero la cosa che la giustifica
/// (un oggetto in arrivo accende anche i neuroni della proboscide, ma senza cibo vicino non vuol dire niente).
pub fn gate(out: &mut [(String, f32)], senses: &HashMap<String, f32>) {
    let has = |ks: &[&str]| ks.iter().any(|k| senses.get(*k).copied().unwrap_or(0.0) > 0.05);
    for (k, v) in out.iter_mut() {
        let need: &[&str] = match k.as_str() {
            "feed" => &["sweet", "smell_food"],
            "drink" => &["water", "wet"],
            "squint" => &["light"],
            "listen" => &["sound", "smell_danger"],
            "brace" => &["wind", "touch", "pain"],
            "escape" | "backward" | "turn_l" | "turn_r" => &["loom_l", "loom_r", "pain", "heat", "smell_danger", "touch"],
            _ => continue,
        };
        if !has(need) {
            *v = 0.0;
        }
    }
}

/// Umore che nasce dai riflessi (per il modello): stress che sale con dolore e fughe e scende piano.
#[derive(Default, Debug, Clone)]
pub struct Mood {
    pub stress: f32,
    pub hunger_fed: f32,
    /// Riflessi recenti (nome, istante).
    pub recent: Vec<(String, std::time::Instant)>,
}

impl Mood {
    pub fn update(&mut self, dt: f32, senses: &HashMap<String, f32>, out: &[(String, f32)]) {
        let get = |k: &str| out.iter().find(|(n, _)| n == k).map(|(_, v)| *v).unwrap_or(0.0);
        let pain = senses.get("pain").copied().unwrap_or(0.0);
        let drive = pain * 0.6 + get("escape") * 0.5 + get("backward") * 0.3 + get("groom") * 0.1;
        // sale in fretta, scende con costante di ~40 s
        self.stress = (self.stress + drive * dt * 0.8).min(1.0);
        self.stress *= (-dt / 40.0).exp();
        if get("feed") > 0.5 {
            self.hunger_fed = 1.0;
        }
        self.hunger_fed *= (-dt / 60.0).exp();
        for (k, thr) in [("escape", 0.5), ("backward", 0.5), ("groom", 0.5), ("feed", 0.5), ("listen", 0.5), ("squint", 0.5), ("brace", 0.5), ("drink", 0.5)] {
            if get(k) >= thr && !self.recent.iter().any(|(n, t)| n == k && t.elapsed().as_secs() < 5) {
                self.recent.push((k.to_string(), std::time::Instant::now()));
            }
        }
        self.recent.retain(|(_, t)| t.elapsed().as_secs() < 20);
    }

    /// "adrenaline 0.6, dodged, got hit" (vuoto se calma).
    pub fn line(&self) -> String {
        let mut parts = Vec::new();
        if self.stress >= 0.1 {
            parts.push(format!("adrenaline {:.1}", self.stress));
        }

        let word = |k: &str| match k {
            "escape" => "dodged something",
            "backward" => "stepped back",
            "groom" => "flinched",
            "feed" => "noticed some food",
            "listen" => "heard a bang",
            "squint" => "dazzled by a light",
            "brace" => "got flung",
            "drink" => "splashed in water",
            _ => "",
        };
        let mut seen: Vec<&str> = Vec::new();
        for (k, _) in self.recent.iter().rev() {
            let w = word(k);
            if !w.is_empty() && !seen.contains(&w) {
                seen.push(w);
            }
        }
        parts.extend(seen.into_iter().take(3).map(String::from));
        parts.join(", ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run_for(b: &mut Brain, ms: f32, s: &[(&str, f32)]) -> HashMap<String, f32> {
        let senses: HashMap<String, f32> = s.iter().map(|(k, v)| (k.to_string(), *v)).collect();
        let mut best: HashMap<String, f32> = HashMap::new();
        let mut t = 0.0;
        while t < ms {
            b.run(100.0, &senses);
            for (k, v) in b.read(100.0) {
                let e = best.entry(k).or_insert(0.0);
                *e = e.max(v);
            }
            t += 100.0;
        }
        best
    }

    #[test]
    fn real_connectome_reflexes() {
        let mut b = Brain::load().expect("flybrain.bin");
        assert!(b.neurons() > 5000 && b.synapses() > 100_000, "{} {}", b.neurons(), b.synapses());
        for k in DEFAULT_SENSES.iter().chain(OPTIONAL.iter().flat_map(|(_, s)| s.iter())) {
            assert!(b.inputs.contains_key(*k), "manca il senso {k}");
        }
        // a riposo il cervello tace
        let quiet = run_for(&mut b, 300.0, &[]);
        assert!(quiet.values().all(|v| *v == 0.0), "{quiet:?}");
        assert_eq!(b.active_count(), 0);
        // qualcosa che arriva da sinistra: Giant Fiber e fuga
        let t = std::time::Instant::now();
        let loom = run_for(&mut b, 600.0, &[("loom_l", 1.0)]);
        let el = t.elapsed();
        assert!(loom["escape"] >= 0.6, "{loom:?}");
        assert!(loom["feed"] < 0.2, "{loom:?}");
        // dolce: proboscide, niente fuga
        let _ = run_for(&mut b, 500.0, &[]);
        let sweet = run_for(&mut b, 600.0, &[("sweet", 1.0)]);
        assert!(sweet["feed"] >= 0.6 && sweet["escape"] < 0.1, "{sweet:?}");
        // setole della testa (dolore): grooming
        let _ = run_for(&mut b, 500.0, &[]);
        let pain = run_for(&mut b, 600.0, &[("pain", 1.0)]);
        assert!(pain["groom"] >= 0.5, "{pain:?}");
        // deve stare ben dentro il tempo reale
        if !cfg!(debug_assertions) {
            assert!(el.as_millis() < 600, "troppo lento: {el:?} per 600 ms");
        }
        eprintln!("600 ms di cervello in {el:?}");
    }

    #[test]
    fn approaching_thing_is_not_food() {
        // qualcosa in arrivo accende anche la proboscide: senza cibo vicino non conta
        let mut b = Brain::load().expect("flybrain.bin");
        let senses: HashMap<String, f32> = [("loom_l".to_string(), 0.3)].into();
        let mut feed = 0.0f32;
        for _ in 0..10 {
            b.run(100.0, &senses);
            let mut out = b.read(100.0);
            gate(&mut out, &senses);
            feed = feed.max(out.iter().find(|(k, _)| k == "feed").map(|x| x.1).unwrap_or(0.0));
        }
        assert_eq!(feed, 0.0);
        let mut m = Mood::default();
        let mut out = vec![("feed".to_string(), 1.0), ("escape".to_string(), 0.8)];
        gate(&mut out, &senses);
        m.update(0.1, &senses, &out);
        assert!(!m.line().contains("food"), "{}", m.line());
    }

    #[test]
    fn mood_from_reflexes() {
        let mut m = Mood::default();
        let senses: HashMap<String, f32> = [("pain".to_string(), 1.0)].into();
        let out = vec![("escape".to_string(), 0.9), ("groom".to_string(), 0.7)];
        for _ in 0..20 {
            m.update(0.1, &senses, &out);
        }
        assert!(m.stress > 0.5, "{}", m.stress);
        let l = m.line();
        assert!(l.starts_with("adrenaline") && l.contains("dodged") && l.contains("flinched"), "{l}");
        m.recent.clear();
        m.stress = 0.0;
        assert_eq!(m.line(), "");
    }
}
