//! Spettro (barre del lettore) e "umore" della musica per cambiare lo sprite a tempo.

use rustfft::{num_complex::Complex, FftPlanner};

pub const FFT: usize = 2048;
pub const BANDS: usize = 24;

pub struct Analyzer {
    fft: std::sync::Arc<dyn rustfft::Fft<f32>>,
    window: Vec<f32>,
    smooth: [f32; BANDS],
    bass_avg: f32,
    since_beat: f32,
    since_sprite: f32,
    pick: usize,
}

/// Risultato di un'analisi (ogni ~40 ms).
pub struct Frame {
    /// Barre 0..=100.
    pub bands: Vec<u8>,
    /// Battito sui bassi.
    pub beat: bool,
    /// Nuovo sprite suggerito (al massimo ogni ~1,5 s, su un battito o un cambio di energia).
    pub sprite: Option<&'static str>,
}

impl Analyzer {
    pub fn new() -> Self {
        let fft = FftPlanner::new().plan_fft_forward(FFT);
        let window = (0..FFT).map(|i| 0.5 - 0.5 * (std::f32::consts::TAU * i as f32 / FFT as f32).cos()).collect();
        Self { fft, window, smooth: [0.0; BANDS], bass_avg: 0.1, since_beat: 0.0, since_sprite: 0.0, pick: 0 }
    }

    /// `samples`: ultimi campioni suonati (mono) a `rate`; `dt`: secondi dall'analisi precedente.
    pub fn run(&mut self, samples: &[f32], rate: u32, dt: f32) -> Frame {
        let mut buf: Vec<Complex<f32>> = vec![Complex::default(); FFT];
        let off = samples.len().saturating_sub(FFT);
        for (i, s) in samples[off..].iter().enumerate() {
            buf[i] = Complex::new(s * self.window[i], 0.0);
        }
        self.fft.process(&mut buf);
        let bin_hz = rate as f32 / FFT as f32;
        let (lo, hi) = (40.0f32, 14_000.0f32.min(rate as f32 / 2.0 - bin_hz));
        let mut bands = Vec::with_capacity(BANDS);
        for b in 0..BANDS {
            let f0 = lo * (hi / lo).powf(b as f32 / BANDS as f32);
            let f1 = lo * (hi / lo).powf((b + 1) as f32 / BANDS as f32);
            let (i0, i1) = ((f0 / bin_hz) as usize, ((f1 / bin_hz) as usize).max((f0 / bin_hz) as usize + 1));
            let peak = buf[i0.max(1)..i1.min(FFT / 2)].iter().map(|c| c.norm()).fold(0.0f32, f32::max);
            // ampiezza → dB (−60..0) → 0..1, con un leggero aumento sugli acuti
            let amp = peak * 4.0 / FFT as f32 * (1.0 + b as f32 / BANDS as f32);
            let db = 20.0 * amp.max(1e-6).log10();
            let v = ((db + 60.0) / 60.0).clamp(0.0, 1.0);
            // salita immediata, discesa morbida
            let s = &mut self.smooth[b];
            *s = if v > *s { v } else { *s * 0.82 + v * 0.18 };
            bands.push((*s * 100.0) as u8);
        }
        let mean = |r: std::ops::Range<usize>| r.clone().map(|i| self.smooth[i]).sum::<f32>() / r.len() as f32;
        let bass = mean(0..4);
        let energy = mean(0..BANDS);
        let treble = mean(16..BANDS);
        self.bass_avg = self.bass_avg * 0.96 + bass * 0.04;
        self.since_beat += dt;
        self.since_sprite += dt;
        let beat = bass > self.bass_avg * 1.18 && bass > 0.35 && self.since_beat > 0.28;
        if beat {
            self.since_beat = 0.0;
        }
        let mut sprite = None;
        if (beat && self.since_sprite > 1.6) || self.since_sprite > 6.0 {
            self.since_sprite = 0.0;
            self.pick += 1;
            let set: &[&'static str] = if energy < 0.12 {
                &["idle"] // silenzio/pausa
            } else if energy > 0.55 && treble > 0.45 {
                &["surprised", "happy", "surprised", "embarrassed"]
            } else if energy > 0.4 {
                &["happy", "embarrassed", "happy", "idle"]
            } else if energy > 0.22 {
                &["idle", "happy", "thinking", "embarrassed"]
            } else {
                &["thinking", "sad", "idle", "worried"]
            };
            sprite = Some(set[self.pick % set.len()]);
        }
        Frame { bands, beat, sprite }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tone_lights_the_right_band_and_kicks_beat() {
        let mut a = Analyzer::new();
        let rate = 48_000;
        let silence = vec![0.0f32; FFT];
        for _ in 0..20 {
            let f = a.run(&silence, rate, 0.04);
            assert!(f.bands.iter().all(|b| *b < 5) && !f.beat);
        }
        // grancassa a 60 Hz: le prime barre salgono, battito
        let kick: Vec<f32> = (0..FFT).map(|i| (std::f32::consts::TAU * 60.0 * i as f32 / rate as f32).sin() * 0.9).collect();
        let f = a.run(&kick, rate, 0.04);
        assert!(f.beat);
        assert!(f.bands[0..4].iter().any(|b| *b > 60), "{:?}", f.bands);
        assert!(f.bands[20..].iter().all(|b| *b < 40), "{:?}", f.bands);
        assert!(f.sprite.is_some() || a.since_sprite < 6.0);
    }
}
