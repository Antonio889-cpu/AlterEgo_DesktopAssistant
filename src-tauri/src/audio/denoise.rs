//! Riduzione del rumore del microfono di Mic-Link, in stile Krisp: RNNoise (nnnoiseless) a 48 kHz
//! + gate sulla probabilità di voce. Si regola dal vivo (intensità e soglia) senza riaprire il microfono.

use nnnoiseless::DenoiseState;

pub const RATE: u32 = 48_000;
const FRAME: usize = DenoiseState::FRAME_SIZE; // 480 campioni = 10 ms

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Params {
    pub enabled: bool,
    /// 0 = segnale originale, 1 = solo segnale ripulito.
    pub strength: f32,
    /// Probabilità di voce (0–1) sotto cui il microfono si abbassa; 0 = gate spento.
    pub gate: f32,
}

pub struct Denoiser {
    state: Box<DenoiseState<'static>>,
    pending: Vec<f32>,
    frame_out: Vec<f32>,
    /// Guadagno del gate (morbido, per non fare "clic").
    gate_gain: f32,
    /// Frame da tenere aperto il gate dopo la fine della voce (code delle parole).
    hold: u32,
}

impl Default for Denoiser {
    fn default() -> Self {
        Self { state: DenoiseState::new(), pending: Vec::with_capacity(FRAME * 4), frame_out: vec![0.0; FRAME], gate_gain: 1.0, hold: 0 }
    }
}

impl Denoiser {
    /// Elabora campioni mono a 48 kHz (−1..1); aggiunge a `out` quelli pronti (ritardo ≤ 10 ms).
    pub fn process(&mut self, input: &[f32], p: Params, out: &mut Vec<f32>) {
        self.pending.extend_from_slice(input);
        let mut start = 0;
        while self.pending.len() - start >= FRAME {
            let dry = &self.pending[start..start + FRAME];
            let scaled: Vec<f32> = dry.iter().map(|s| s * 32767.0).collect();
            let vad = self.state.process_frame(&mut self.frame_out, &scaled);
            // gate: aperto con la voce, chiuso (non del tutto) sul rumore
            let target = if p.gate <= 0.0 {
                1.0
            } else if vad >= p.gate {
                self.hold = 20; // 200 ms
                1.0
            } else if self.hold > 0 {
                self.hold -= 1;
                1.0
            } else {
                1.0 - 0.92 * p.strength
            };
            let g0 = self.gate_gain;
            // apertura veloce, chiusura lenta
            let g1 = g0 + (target - g0) * if target > g0 { 0.6 } else { 0.12 };
            self.gate_gain = g1;
            let s = p.strength.clamp(0.0, 1.0);
            for (i, (d, w)) in dry.iter().zip(&self.frame_out).enumerate() {
                let g = g0 + (g1 - g0) * (i as f32 / FRAME as f32);
                let wet = w / 32767.0;
                out.push((d * (1.0 - s) + wet * s) * g);
            }
            start += FRAME;
        }
        self.pending.drain(..start);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reduces_steady_noise() {
        // rumore bianco pseudo-casuale costante: dopo l'adattamento esce molto più basso
        let mut seed = 1u32;
        let mut noise = || {
            seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            ((seed >> 9) as f32 / (1u32 << 23) as f32 - 0.5) * 0.1
        };
        let input: Vec<f32> = (0..RATE as usize * 2).map(|_| noise()).collect();
        let mut d = Denoiser::default();
        let mut out = Vec::new();
        for chunk in input.chunks(441) {
            d.process(chunk, Params { enabled: true, strength: 1.0, gate: 0.3 }, &mut out);
        }
        assert!(out.len() >= input.len() - FRAME);
        let rms = |v: &[f32]| (v.iter().map(|x| x * x).sum::<f32>() / v.len() as f32).sqrt();
        let tail = out.len() / 2;
        assert!(rms(&out[tail..]) < rms(&input[tail..]) * 0.6, "{} vs {}", rms(&out[tail..]), rms(&input[tail..]));
    }
}
