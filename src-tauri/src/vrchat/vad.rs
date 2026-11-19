//! Rilevamento della voce (VAD) sull'audio di VRChat: energia a finestre di 20 ms con soglia che
//! segue il rumore di fondo. Taglia le frasi (con un po' di audio prima e una pausa dopo) e dice
//! se qualcuno sta parlando adesso (per non parlargli sopra).
//! Lavora su audio stereo a 16 kHz: il bilanciamento sinistra/destra dice da che parte arriva la voce.

pub const RATE: u32 = 16_000;
const FRAME: usize = 320; // 20 ms
/// Finestre con voce di fila per dire "sta parlando".
const START_FRAMES: usize = 4;
/// Silenzio che chiude una frase.
const END_FRAMES: usize = 35; // 700 ms
/// Audio tenuto prima dell'inizio (le prime sillabe).
const PRE_FRAMES: usize = 12;
const MIN_SECS: f32 = 0.5;
const MAX_SECS: f32 = 14.0;
/// Soglia minima assoluta (RMS): sotto è silenzio anche se il fondo è bassissimo.
const MIN_RMS: f32 = 0.006;

#[derive(Debug, Clone)]
pub struct Segment {
    /// Mono 16 kHz.
    pub samples: Vec<f32>,
    /// -1 = tutta a sinistra, +1 = tutta a destra.
    pub balance: f32,
    /// Volume medio delle parti con voce (RMS): più alto = più vicino.
    pub level: f32,
}

impl Segment {
    #[cfg(test)]
    pub fn secs(&self) -> f32 {
        self.samples.len() as f32 / RATE as f32
    }
}

pub struct Vad {
    noise: f32,
    voiced_run: usize,
    silent_run: usize,
    in_speech: bool,
    pre: std::collections::VecDeque<(Vec<f32>, f32, f32)>,
    cur: Vec<f32>,
    sum_l: f32,
    sum_r: f32,
    sum_rms: f32,
    voiced: usize,
    pending: Vec<f32>,
    pending_lr: Vec<(f32, f32)>,
}

impl Default for Vad {
    fn default() -> Self {
        Self {
            noise: 0.003,
            voiced_run: 0,
            silent_run: 0,
            in_speech: false,
            pre: Default::default(),
            cur: Vec::new(),
            sum_l: 0.0,
            sum_r: 0.0,
            sum_rms: 0.0,
            voiced: 0,
            pending: Vec::new(),
            pending_lr: Vec::new(),
        }
    }
}

impl Vad {
    /// Qualcuno sta parlando adesso.
    pub fn talking(&self) -> bool {
        self.in_speech
    }

    /// Dimentica tutto (mentre parla Alter Ego: la sua voce non va trascritta).
    pub fn reset(&mut self) {
        let noise = self.noise;
        *self = Self { noise, ..Default::default() };
    }

    /// Soglia attuale.
    fn threshold(&self) -> f32 {
        (self.noise * 3.0).max(MIN_RMS)
    }

    /// Aggiunge audio stereo intrecciato (L, R) a 16 kHz. Restituisce le frasi finite.
    pub fn push_stereo(&mut self, lr: &[(f32, f32)]) -> Vec<Segment> {
        let mut out = Vec::new();
        for &(l, r) in lr {
            self.pending.push((l + r) * 0.5);
            self.pending_lr.push((l, r));
            if self.pending.len() == FRAME {
                let frame = std::mem::take(&mut self.pending);
                let side = std::mem::take(&mut self.pending_lr);
                let el = (side.iter().map(|x| x.0 * x.0).sum::<f32>() / FRAME as f32).sqrt();
                let er = (side.iter().map(|x| x.1 * x.1).sum::<f32>() / FRAME as f32).sqrt();
                if let Some(s) = self.frame(frame, el, er) {
                    out.push(s);
                }
            }
        }
        out
    }

    fn frame(&mut self, f: Vec<f32>, el: f32, er: f32) -> Option<Segment> {
        let rms = (f.iter().map(|x| x * x).sum::<f32>() / f.len() as f32).sqrt();
        let voiced = rms > self.threshold();
        // il fondo si adatta solo nel silenzio (sale piano, scende in fretta)
        if !voiced {
            let k = if rms < self.noise { 0.2 } else { 0.02 };
            self.noise = (self.noise * (1.0 - k) + rms * k).clamp(0.0005, 0.05);
        }
        if !self.in_speech {
            self.pre.push_back((f, el, er));
            while self.pre.len() > PRE_FRAMES {
                self.pre.pop_front();
            }
            self.voiced_run = if voiced { self.voiced_run + 1 } else { 0 };
            if self.voiced_run >= START_FRAMES {
                self.in_speech = true;
                self.silent_run = 0;
                for (pf, l, r) in std::mem::take(&mut self.pre) {
                    let prms = (pf.iter().map(|x| x * x).sum::<f32>() / pf.len() as f32).sqrt();
                    if prms > self.threshold() {
                        self.add_voiced(prms, l, r);
                    }
                    self.cur.extend(pf);
                }
            }
            return None;
        }
        self.cur.extend(&f);
        if voiced {
            self.silent_run = 0;
            self.add_voiced(rms, el, er);
        } else {
            self.silent_run += 1;
        }
        let secs = self.cur.len() as f32 / RATE as f32;
        if self.silent_run >= END_FRAMES || secs >= MAX_SECS {
            return self.close();
        }
        None
    }

    fn add_voiced(&mut self, rms: f32, l: f32, r: f32) {
        self.voiced += 1;
        self.sum_rms += rms;
        self.sum_l += l;
        self.sum_r += r;
    }

    fn close(&mut self) -> Option<Segment> {
        let samples = std::mem::take(&mut self.cur);
        let voiced = std::mem::take(&mut self.voiced);
        let (l, r, s) = (std::mem::take(&mut self.sum_l), std::mem::take(&mut self.sum_r), std::mem::take(&mut self.sum_rms));
        self.in_speech = false;
        self.voiced_run = 0;
        self.silent_run = 0;
        let voiced_secs = voiced as f32 * FRAME as f32 / RATE as f32;
        if voiced_secs < MIN_SECS * 0.6 || (samples.len() as f32) < MIN_SECS * RATE as f32 {
            return None; // colpo, click o rumore breve
        }
        let balance = if l + r > 0.0 { (r - l) / (l + r) } else { 0.0 };
        Some(Segment { samples, balance, level: s / voiced.max(1) as f32 })
    }
}

/// Converte un blocco intrecciato a `channels` canali in coppie (L, R) (mono: L = R).
pub fn to_lr(data: &[f32], channels: usize) -> Vec<(f32, f32)> {
    let c = channels.max(1);
    data.chunks(c).map(|f| if c >= 2 { (f[0], f[1]) } else { (f[0], f[0]) }).collect()
}

/// Ricampionamento lineare con stato per coppie stereo (blocchi in tempo reale).
pub struct StereoResampler {
    step: f64,
    pos: f64,
    last: (f32, f32),
}

impl StereoResampler {
    pub fn new(from: u32, to: u32) -> Self {
        Self { step: from as f64 / to.max(1) as f64, pos: 0.0, last: (0.0, 0.0) }
    }
    pub fn push(&mut self, input: &[(f32, f32)], out: &mut Vec<(f32, f32)>) {
        let n = input.len() as f64;
        let last = self.last;
        let get = |i: isize| if i < 0 { last } else { input[i as usize] };
        while self.pos <= n - 1.0 {
            let i = self.pos.floor();
            let fr = (self.pos - i) as f32;
            let a = get(i as isize);
            let b = if fr == 0.0 { a } else { get(i as isize + 1) };
            out.push((a.0 + (b.0 - a.0) * fr, a.1 + (b.1 - a.1) * fr));
            self.pos += self.step;
        }
        if let Some(l) = input.last() {
            self.last = *l;
        }
        self.pos -= n;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tone(secs: f32, amp: f32, pan: f32) -> Vec<(f32, f32)> {
        let n = (secs * RATE as f32) as usize;
        (0..n)
            .map(|i| {
                // voce finta: due toni con un po' di "sillabe"
                let t = i as f32 / RATE as f32;
                let env = 0.6 + 0.4 * (t * 6.0 * std::f32::consts::TAU).sin().abs();
                let s = amp * env * ((t * 220.0 * std::f32::consts::TAU).sin() * 0.7 + (t * 660.0 * std::f32::consts::TAU).sin() * 0.3);
                (s * (1.0 - pan).min(1.0), s * (1.0 + pan).min(1.0))
            })
            .collect()
    }

    fn noise(secs: f32, amp: f32) -> Vec<(f32, f32)> {
        let mut x = 0x1234_5678u32;
        (0..(secs * RATE as f32) as usize)
            .map(|_| {
                x ^= x << 13;
                x ^= x >> 17;
                x ^= x << 5;
                let v = (x as f32 / u32::MAX as f32 - 0.5) * 2.0 * amp;
                (v, v)
            })
            .collect()
    }

    #[test]
    fn segments_speech_between_silences() {
        let mut v = Vad::default();
        let mut segs = Vec::new();
        segs.extend(v.push_stereo(&noise(1.0, 0.002)));
        assert!(!v.talking());
        // prima frase, a destra
        segs.extend(v.push_stereo(&tone(1.2, 0.2, 0.8)));
        assert!(v.talking(), "sta parlando");
        segs.extend(v.push_stereo(&noise(1.0, 0.002)));
        assert!(!v.talking());
        // colpo breve: niente frase
        segs.extend(v.push_stereo(&tone(0.1, 0.5, 0.0)));
        segs.extend(v.push_stereo(&noise(1.0, 0.002)));
        // seconda frase, a sinistra e più piano (più lontana), con una pausa breve in mezzo
        segs.extend(v.push_stereo(&tone(0.8, 0.05, -0.8)));
        segs.extend(v.push_stereo(&noise(0.3, 0.002)));
        segs.extend(v.push_stereo(&tone(0.8, 0.05, -0.8)));
        segs.extend(v.push_stereo(&noise(1.0, 0.002)));
        assert_eq!(segs.len(), 2, "{:?}", segs.iter().map(|s| s.secs()).collect::<Vec<_>>());
        let (a, b) = (&segs[0], &segs[1]);
        assert!(a.secs() > 1.2 && a.secs() < 2.3, "{}", a.secs());
        assert!(a.balance > 0.5, "da destra: {}", a.balance);
        assert!(b.balance < -0.5, "da sinistra: {}", b.balance);
        assert!(b.secs() > 1.8, "la pausa breve non taglia la frase: {}", b.secs());
        assert!(a.level > b.level * 2.0, "la prima è più vicina");
        // frase lunghissima: tagliata a MAX_SECS
        let mut v = Vad::default();
        let long = v.push_stereo(&tone(MAX_SECS + 2.0, 0.2, 0.0));
        assert_eq!(long.len(), 1);
        assert!(long[0].secs() <= MAX_SECS + 0.05);
        // reset mentre parla: niente frase a metà
        let mut v = Vad::default();
        v.push_stereo(&tone(1.0, 0.2, 0.0));
        assert!(v.talking());
        v.reset();
        assert!(!v.talking());
        assert!(v.push_stereo(&noise(1.0, 0.002)).is_empty());
    }

    #[test]
    fn resampler_and_channels() {
        assert_eq!(to_lr(&[0.1, 0.2, 0.3, 0.4], 2), vec![(0.1, 0.2), (0.3, 0.4)]);
        assert_eq!(to_lr(&[0.1, 0.2], 1), vec![(0.1, 0.1), (0.2, 0.2)]);
        assert_eq!(to_lr(&[1.0, 2.0, 3.0, 4.0, 5.0, 6.0], 3), vec![(1.0, 2.0), (4.0, 5.0)]);
        let mut rs = StereoResampler::new(48_000, 16_000);
        let mut out = Vec::new();
        for _ in 0..10 {
            rs.push(&vec![(0.5, -0.5); 4800], &mut out);
        }
        assert!((out.len() as i64 - 16_000).abs() <= 2, "{}", out.len());
        assert!(out.iter().all(|x| (x.0 - 0.5).abs() < 1e-6 && (x.1 + 0.5).abs() < 1e-6));
    }
}
