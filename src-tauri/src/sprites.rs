//! Sprite caricati a runtime da `%APPDATA%\AlterEgo\sprites\` (fuori dall'exe e dalla repo:
//! sono © Spike Chunsoft). Si possono importare da un foglio 3×3 (stessa elaborazione di
//! `tools/process_sprites.py`) oppure copiare a mano come `idle.png`, `happy.png`, ecc.

use base64::Engine;
use std::{collections::BTreeMap, path::{Path, PathBuf}};

/// Ordine dei pannelli nel foglio (riga per riga). Il 9° viene ignorato.
pub const NAMES: [&str; 8] = ["idle", "happy", "embarrassed", "worried", "thinking", "sad", "surprised", "error"];
/// Stati accettati se copiati a mano (inclusi quelli mancanti nel foglio).
const ALL: [&str; 13] = ["idle", "happy", "embarrassed", "worried", "thinking", "sad", "surprised", "error", "blink", "talk_open", "talk_closed", "listening", "sleep"];

const KEY_HARD: f32 = 35.0;
const KEY_SOFT: (f32, f32) = (18.0, 48.0);
const SCALE: usize = 3;

pub fn dir(data_dir: &Path) -> PathBuf {
    data_dir.join("sprites")
}

/// Sprite presenti, come data URI PNG (nome → "data:image/png;base64,...").
pub fn load(data_dir: &Path) -> BTreeMap<String, String> {
    let d = dir(data_dir);
    ALL.iter()
        .filter_map(|name| {
            let bytes = std::fs::read(d.join(format!("{name}.png"))).ok()?;
            Some((name.to_string(), format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(bytes))))
        })
        .collect()
}

pub fn remove_all(data_dir: &Path) -> Result<(), String> {
    let d = dir(data_dir);
    for name in ALL {
        let p = d.join(format!("{name}.png"));
        if p.exists() {
            std::fs::remove_file(p).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

struct Rgb {
    w: usize,
    h: usize,
    px: Vec<[f32; 3]>,
}

fn decode_png(bytes: &[u8]) -> Result<Rgb, String> {
    let mut dec = png::Decoder::new(std::io::Cursor::new(bytes));
    dec.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = dec.read_info().map_err(|_| crate::i18n::tr("il file non è un PNG valido", "the file is not a valid PNG").to_string())?;
    let mut buf = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buf).map_err(|e| e.to_string())?;
    let (w, h) = (info.width as usize, info.height as usize);
    let ch = match info.color_type {
        png::ColorType::Rgb => 3,
        png::ColorType::Rgba => 4,
        png::ColorType::Grayscale => 1,
        png::ColorType::GrayscaleAlpha => 2,
        _ => return Err(crate::i18n::tr("formato colore non supportato", "color format not supported").into()),
    };
    let px = (0..w * h)
        .map(|i| {
            let p = &buf[i * ch..i * ch + ch];
            if ch < 3 { [p[0] as f32; 3] } else { [p[0] as f32, p[1] as f32, p[2] as f32] }
        })
        .collect();
    Ok(Rgb { w, h, px })
}

/// Trova le linee chiare della cornice: restituisce gli intervalli interni dei pannelli.
fn panel_ranges(bright: &[f32], min_size: usize) -> Vec<(usize, usize)> {
    let mut lines = Vec::new();
    let mut i = 0;
    while i < bright.len() {
        if bright[i] > 0.5 {
            let s = i;
            while i < bright.len() && bright[i] > 0.5 {
                i += 1;
            }
            lines.push((s, i));
        } else {
            i += 1;
        }
    }
    let mut out = Vec::new();
    for pair in lines.windows(2) {
        let (a, b) = (pair[0].1, pair[1].0);
        if b > a + min_size {
            out.push((a + 4, b - 4)); // fuori dal bagliore della cornice
        }
    }
    out
}

fn detect_panels(img: &Rgb) -> Result<Vec<(usize, usize, usize, usize)>, String> {
    let is_bright = |p: &[f32; 3]| p[0] + p[1] + p[2] > 600.0;
    let cols: Vec<f32> = (0..img.w).map(|x| (0..img.h).filter(|&y| is_bright(&img.px[y * img.w + x])).count() as f32 / img.h as f32).collect();
    let rows: Vec<f32> = (0..img.h).map(|y| (0..img.w).filter(|&x| is_bright(&img.px[y * img.w + x])).count() as f32 / img.w as f32).collect();
    // le linee della cornice coprono solo la loro riga di pannelli: soglia relativa
    let norm = |v: Vec<f32>| {
        let max = v.iter().cloned().fold(0.0, f32::max).max(1e-6);
        v.into_iter().map(|x| x / max).collect::<Vec<_>>()
    };
    let xs = panel_ranges(&norm(cols), img.w / 10);
    let ys = panel_ranges(&norm(rows), img.h / 10);
    if xs.is_empty() || ys.is_empty() {
        return Err(crate::i18n::tr("non trovo i riquadri: serve un foglio con cornici chiare (come quello originale 3×3)", "can't find the panels: a sheet with light frames is needed (like the original 3×3)").into());
    }
    // stessa dimensione per tutti (il più piccolo), così l'ancoraggio è uguale
    let pw = xs.iter().map(|r| r.1 - r.0).min().unwrap();
    let ph = ys.iter().map(|r| r.1 - r.0).min().unwrap();
    Ok(ys.iter().flat_map(|y| xs.iter().map(move |x| (x.0, y.0, pw, ph))).collect())
}

fn process_panel(img: &Rgb, x0: usize, y0: usize, w: usize, h: usize) -> Vec<[f32; 4]> {
    // sfocatura verticale [1,2,3,2,1]: toglie le scanline "cotte"
    let k = [1.0, 2.0, 3.0, 2.0, 1.0];
    let mut rgb = vec![[0f32; 3]; w * h];
    for y in 0..h {
        for x in 0..w {
            let mut acc = [0f32; 3];
            for (i, kv) in k.iter().enumerate() {
                let yy = (y as isize + i as isize - 2).clamp(0, h as isize - 1) as usize;
                let p = img.px[(y0 + yy) * img.w + x0 + x];
                for c in 0..3 {
                    acc[c] += p[c] * kv / 9.0;
                }
            }
            rgb[y * w + x] = acc;
        }
    }
    let gr: Vec<f32> = rgb.iter().map(|p| p[1] - p[0]).collect();
    // flood fill dai bordi (alto, sinistra, destra) sui pixel verdi
    let cand: Vec<bool> = gr.iter().map(|&v| v > KEY_HARD).collect();
    let mut bg = vec![false; w * h];
    let mut stack: Vec<usize> = Vec::new();
    for x in 0..w {
        stack.push(x);
    }
    for y in 0..h {
        stack.push(y * w);
        stack.push(y * w + w - 1);
    }
    while let Some(i) = stack.pop() {
        if bg[i] || !cand[i] {
            continue;
        }
        bg[i] = true;
        let (x, y) = (i % w, i / w);
        if x > 0 { stack.push(i - 1); }
        if x + 1 < w { stack.push(i + 1); }
        if y > 0 { stack.push(i - w); }
        if y + 1 < h { stack.push(i + w); }
    }
    // banda di 2 px intorno allo sfondo: trasparenza graduale
    let mut near = bg.clone();
    for _ in 0..2 {
        let prev = near.clone();
        for i in 0..w * h {
            let (x, y) = (i % w, i / w);
            if !prev[i] && ((x > 0 && prev[i - 1]) || (x + 1 < w && prev[i + 1]) || (y > 0 && prev[i - w]) || (y + 1 < h && prev[i + w])) {
                near[i] = true;
            }
        }
    }
    (0..w * h)
        .map(|i| {
            let (x, y) = (i % w, i / w);
            let mut a = if bg[i] {
                0.0
            } else if near[i] {
                ((KEY_SOFT.1 - gr[i]) / (KEY_SOFT.1 - KEY_SOFT.0)).clamp(0.0, 1.0)
            } else {
                1.0
            };
            if y < 2 || x < 2 || x + 2 >= w {
                a = 0.0;
            }
            let mut p = rgb[i];
            if a > 0.0 && a < 1.0 {
                p[1] = p[1].min(p[0] + 10.0); // niente alone verde
            }
            [p[0].clamp(0.0, 255.0), p[1].clamp(0.0, 255.0), p[2].clamp(0.0, 255.0), a * 255.0]
        })
        .collect()
}

fn upscale(src: &[[f32; 4]], w: usize, h: usize, s: usize) -> Vec<u8> {
    let (nw, nh) = (w * s, h * s);
    let mut out = vec![0u8; nw * nh * 4];
    for y in 0..nh {
        let fy = ((y as f32 + 0.5) / s as f32 - 0.5).clamp(0.0, (h - 1) as f32);
        let (y0, ty) = (fy.floor() as usize, fy.fract());
        let y1 = (y0 + 1).min(h - 1);
        for x in 0..nw {
            let fx = ((x as f32 + 0.5) / s as f32 - 0.5).clamp(0.0, (w - 1) as f32);
            let (x0, tx) = (fx.floor() as usize, fx.fract());
            let x1 = (x0 + 1).min(w - 1);
            for c in 0..4 {
                let v = src[y0 * w + x0][c] * (1.0 - tx) * (1.0 - ty)
                    + src[y0 * w + x1][c] * tx * (1.0 - ty)
                    + src[y1 * w + x0][c] * (1.0 - tx) * ty
                    + src[y1 * w + x1][c] * tx * ty;
                out[(y * nw + x) * 4 + c] = v.round().clamp(0.0, 255.0) as u8;
            }
        }
    }
    out
}

/// Importa un foglio sprite PNG (3×3 con cornici chiare). Restituisce i nomi salvati.
pub fn import_sheet(data_dir: &Path, png_bytes: &[u8]) -> Result<Vec<String>, String> {
    let img = decode_png(png_bytes)?;
    let panels = detect_panels(&img)?;
    let processed: Vec<(&str, Vec<[f32; 4]>, usize, usize)> = panels
        .iter()
        .zip(NAMES)
        .map(|(&(x, y, w, h), name)| (name, process_panel(&img, x, y, w, h), w, h))
        .collect();
    let (pw, ph) = (processed[0].2, processed[0].3);
    // ritaglio comune: stesso ancoraggio per tutti
    let (mut minx, mut miny, mut maxx, mut maxy) = (pw, ph, 0, 0);
    for (_, px, _, _) in &processed {
        for (i, p) in px.iter().enumerate() {
            if p[3] > 8.0 {
                let (x, y) = (i % pw, i / pw);
                minx = minx.min(x);
                miny = miny.min(y);
                maxx = maxx.max(x + 1);
                maxy = maxy.max(y + 1);
            }
        }
    }
    if maxx <= minx || maxy <= miny {
        return Err(crate::i18n::tr("nessuna figura trovata nei riquadri", "no figure found in the panels").into());
    }
    let (cw, ch) = (maxx - minx, maxy - miny);
    let d = dir(data_dir);
    std::fs::create_dir_all(&d).map_err(|e| e.to_string())?;
    let mut saved = Vec::new();
    for (name, px, _, _) in processed {
        let crop: Vec<[f32; 4]> = (0..ch).flat_map(|y| (0..cw).map(move |x| (x, y))).map(|(x, y)| px[(miny + y) * pw + minx + x]).collect();
        let rgba = upscale(&crop, cw, ch, SCALE);
        let png = crate::actions::screen::encode_png((cw * SCALE) as u32, (ch * SCALE) as u32, &rgba)?;
        std::fs::write(d.join(format!("{name}.png")), png).map_err(|e| e.to_string())?;
        saved.push(name.to_string());
    }
    Ok(saved)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Usa il foglio originale se presente in locale (non è nella repo).
    #[test]
    fn import_real_sheet_if_present() {
        let sheet = Path::new(env!("CARGO_MANIFEST_DIR")).join("../assets/sprites/raw/alter_ego_sheet.png");
        let Ok(bytes) = std::fs::read(&sheet) else { return };
        let tmp = std::env::temp_dir().join(format!("ae_sprites_{}", crate::memory::now()));
        let saved = import_sheet(&tmp, &bytes).unwrap();
        assert_eq!(saved.len(), 8);
        let loaded = load(&tmp);
        assert_eq!(loaded.len(), 8);
        let img = decode_png(&std::fs::read(dir(&tmp).join("idle.png")).unwrap()).unwrap();
        assert!(img.w > 500 && img.h > 400, "{}x{}", img.w, img.h);
        // AE_KEEP_SPRITES=cartella: copia l'esito per controllarlo a occhio
        if let Ok(keep) = std::env::var("AE_KEEP_SPRITES") {
            let _ = std::fs::create_dir_all(&keep);
            for n in NAMES {
                let _ = std::fs::copy(dir(&tmp).join(format!("{n}.png")), Path::new(&keep).join(format!("{n}.png")));
            }
        }
        remove_all(&tmp).unwrap();
        assert!(load(&tmp).is_empty());
        let _ = std::fs::remove_dir_all(tmp);
    }

    #[test]
    fn rejects_garbage() {
        let tmp = std::env::temp_dir();
        assert!(import_sheet(&tmp, b"non sono un png").is_err());
        let blank = crate::actions::screen::encode_png(50, 50, &vec![0u8; 50 * 50 * 4]).unwrap();
        assert!(import_sheet(&tmp, &blank).is_err());
    }
}
