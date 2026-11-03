//! Suoni sintetizzati (nessun file audio): blip del dialogo e jingle.
//! - "dr":   blip acuto e corto, ispirato al suono dei dialoghi di Danganronpa (ricreato da zero)
//! - "soft": blip originale più morbido

pub const RATE: u32 = 48_000;

/// Inviluppo: attacco esponenziale rapido, poi decadimento esponenziale fino a ~0.
fn env(t: f32, attack: f32, end: f32, peak: f32) -> f32 {
    let floor = 0.0001f32;
    if t < attack {
        floor * (peak / floor).powf(t / attack)
    } else {
        peak * (floor / peak).powf(((t - attack) / (end - attack)).min(1.0))
    }
}

/// Un blip. `seed`: codice del carattere (varia leggermente il tono).
pub fn blip(style: &str, seed: u32, volume: f32) -> Vec<f32> {
    let r = RATE as f32;
    match style {
        "dr" => {
            let f = 880.0 + (seed % 5) as f32 * 40.0;
            let n = (r * 0.05) as usize;
            (0..n)
                .map(|i| {
                    let t = i as f32 / r;
                    let sq = if (t * f).fract() < 0.5 { 1.0 } else { -1.0 };
                    sq * env(t, 0.004, 0.045, 0.18 * volume)
                })
                .collect()
        }
        "soft" => {
            let n = (r * 0.08) as usize;
            let mut phase = 0.0f32;
            (0..n)
                .map(|i| {
                    let t = i as f32 / r;
                    // glissando 520 → 380 Hz
                    let f = 520.0 * (380.0f32 / 520.0).powf((t / 0.06).min(1.0));
                    phase = (phase + f / r).fract();
                    let tri = 1.0 - 4.0 * (phase - 0.5).abs();
                    tri * env(t, 0.006, 0.07, 0.35 * volume)
                })
                .collect()
        }
        _ => Vec::new(),
    }
}

/// Breve jingle a due note (clip salvata, promemoria).
pub fn chime(volume: f32) -> Vec<f32> {
    let r = RATE as f32;
    let v = 0.25 * volume.max(0.2);
    let total = (r * 0.29) as usize;
    let mut out = vec![0.0f32; total];
    for (k, f) in [660.0f32, 990.0].iter().enumerate() {
        let start = (r * 0.09 * k as f32) as usize;
        for i in 0..(r * 0.2) as usize {
            let t = i as f32 / r;
            if let Some(o) = out.get_mut(start + i) {
                *o += (std::f32::consts::TAU * f * t).sin() * env(t, 0.01, 0.18, v);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sounds_are_short_and_bounded() {
        for s in ["dr", "soft"] {
            let b = blip(s, 7, 1.0);
            assert!(!b.is_empty() && b.len() < RATE as usize / 5);
            assert!(b.iter().all(|x| x.abs() <= 0.36));
            // finisce in silenzio (niente click)
            assert!(b.last().unwrap().abs() < 0.01);
        }
        assert!(blip("off", 1, 1.0).is_empty());
        let c = chime(1.0);
        assert!(c.iter().all(|x| x.abs() <= 0.5));
    }
}
