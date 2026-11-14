//! Screenshot dello schermo principale (GDI su Windows, grim/gnome-screenshot/scrot… su Linux),
//! ridotto alla griglia delle coordinate degli strumenti mouse, con righelli numerati e cursore
//! disegnati sopra (così il modello legge le coordinate invece di stimarle). PNG in base64.

#[cfg(windows)]
fn capture_rgba() -> Result<(u32, u32, Vec<u8>), String> {
    use windows_sys::Win32::Graphics::Gdi::*;
    use windows_sys::Win32::UI::WindowsAndMessaging::{GetSystemMetrics, SM_CXSCREEN, SM_CYSCREEN};
    unsafe {
        let w = GetSystemMetrics(SM_CXSCREEN);
        let h = GetSystemMetrics(SM_CYSCREEN);
        if w <= 0 || h <= 0 {
            return Err("schermo non trovato".into());
        }
        let screen = GetDC(std::ptr::null_mut());
        let mem = CreateCompatibleDC(screen);
        let bmp = CreateCompatibleBitmap(screen, w, h);
        let old = SelectObject(mem, bmp as _);
        let ok = BitBlt(mem, 0, 0, w, h, screen, 0, 0, SRCCOPY);
        let mut info: BITMAPINFO = std::mem::zeroed();
        info.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
        info.bmiHeader.biWidth = w;
        info.bmiHeader.biHeight = -h; // dall'alto in basso
        info.bmiHeader.biPlanes = 1;
        info.bmiHeader.biBitCount = 32;
        info.bmiHeader.biCompression = BI_RGB;
        let mut buf = vec![0u8; (w * h * 4) as usize];
        let lines = GetDIBits(mem, bmp, 0, h as u32, buf.as_mut_ptr() as _, &mut info, DIB_RGB_COLORS);
        SelectObject(mem, old);
        DeleteObject(bmp as _);
        DeleteDC(mem);
        ReleaseDC(std::ptr::null_mut(), screen);
        if ok == 0 || lines == 0 {
            return Err("cattura schermo fallita".into());
        }
        for px in buf.chunks_exact_mut(4) {
            px.swap(0, 2); // BGRA → RGBA
            px[3] = 255;
        }
        Ok((w as u32, h as u32, buf))
    }
}

/// Linux: il primo strumento di cattura disponibile (Wayland o X11), letto come PNG.
#[cfg(not(windows))]
fn capture_rgba() -> Result<(u32, u32, Vec<u8>), String> {
    use std::process::{Command, Stdio};
    let file = std::env::temp_dir().join(format!("alterego-shot-{}.png", std::process::id()));
    let f = file.to_string_lossy().to_string();
    let tools: [(&str, Vec<&str>); 6] = [
        ("grim", vec![f.as_str()]),
        ("gnome-screenshot", vec!["-f", f.as_str()]),
        ("spectacle", vec!["-b", "-n", "-f", "-o", f.as_str()]),
        ("scrot", vec!["-o", f.as_str()]),
        ("maim", vec![f.as_str()]),
        ("import", vec!["-window", "root", f.as_str()]),
    ];
    let _ = std::fs::remove_file(&file);
    let mut tried = false;
    for (tool, args) in tools {
        let r = Command::new(tool).args(&args).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).status();
        if r.is_ok() {
            tried = true;
        }
        if r.is_ok_and(|s| s.success()) && file.is_file() {
            break;
        }
    }
    let bytes = std::fs::read(&file).map_err(|_| {
        if tried {
            crate::i18n::tr("screenshot non riuscito (su Wayland serve grim o gnome-screenshot)", "screenshot failed (on Wayland you need grim or gnome-screenshot)").to_string()
        } else {
            crate::i18n::tr("per gli screenshot su Linux installa grim, gnome-screenshot, scrot o maim", "for screenshots on Linux install grim, gnome-screenshot, scrot or maim").to_string()
        }
    })?;
    let _ = std::fs::remove_file(&file);
    decode_png(&bytes)
}

#[cfg_attr(windows, allow(dead_code))]
fn decode_png(bytes: &[u8]) -> Result<(u32, u32, Vec<u8>), String> {
    let mut dec = png::Decoder::new(std::io::Cursor::new(bytes));
    dec.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = dec.read_info().map_err(|e| e.to_string())?;
    let mut buf = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buf).map_err(|e| e.to_string())?;
    buf.truncate(info.buffer_size());
    let rgba = match info.color_type {
        png::ColorType::Rgba => buf,
        png::ColorType::Rgb => buf.chunks_exact(3).flat_map(|p| [p[0], p[1], p[2], 255]).collect(),
        png::ColorType::GrayscaleAlpha => buf.chunks_exact(2).flat_map(|p| [p[0], p[0], p[0], 255]).collect(),
        png::ColorType::Grayscale => buf.iter().flat_map(|&g| [g, g, g, 255]).collect(),
        png::ColorType::Indexed => return Err("PNG indicizzato non atteso".into()),
    };
    Ok((info.width, info.height, rgba))
}

/// Ridimensiona a `nw`×`nh` (anche con rapporti non interi): media dei pixel quando si riduce,
/// pixel più vicino quando si ingrandisce (testo nitido nello zoom).
pub fn resample(w: u32, h: u32, rgba: &[u8], nw: u32, nh: u32) -> Vec<u8> {
    let (nw, nh) = (nw.max(1), nh.max(1));
    let fx = w as f64 / nw as f64;
    let fy = h as f64 / nh as f64;
    let mut out = vec![0u8; (nw * nh * 4) as usize];
    let span = |o: u32, f: f64, max: u32| -> (u32, u32) {
        let a = ((o as f64 * f).floor() as u32).min(max - 1);
        let b = (((o + 1) as f64 * f).floor() as u32).clamp(a + 1, max);
        (a, b)
    };
    for y in 0..nh {
        let (y0, y1) = span(y, fy, h);
        for x in 0..nw {
            let (x0, x1) = span(x, fx, w);
            let mut acc = [0u32; 4];
            let mut n = 0u32;
            for sy in y0..y1 {
                for sx in x0..x1 {
                    let i = ((sy * w + sx) * 4) as usize;
                    for c in 0..4 {
                        acc[c] += rgba[i + c] as u32;
                    }
                    n += 1;
                }
            }
            let o = ((y * nw + x) * 4) as usize;
            for c in 0..4 {
                out[o + c] = (acc[c] / n.max(1)) as u8;
            }
        }
    }
    out
}

/// Riduzione a larghezza massima `max_w` (stesse proporzioni).
#[cfg(test)]
pub fn downscale(w: u32, h: u32, rgba: &[u8], max_w: u32) -> (u32, u32, Vec<u8>) {
    if w <= max_w {
        return (w, h, rgba.to_vec());
    }
    let nh = ((h as f64) * max_w as f64 / w as f64).round() as u32;
    (max_w, nh, resample(w, h, rgba, max_w, nh))
}

// ---------- righelli e cursore disegnati sull'immagine ----------

/// Cifre 3×5 (bit alti = colonna sinistra), per le etichette dei righelli.
const DIGITS: [[u8; 5]; 10] = [
    [7, 5, 5, 5, 7], [2, 6, 2, 2, 7], [7, 1, 7, 4, 7], [7, 1, 7, 1, 7], [5, 5, 7, 1, 1],
    [7, 4, 7, 1, 7], [7, 4, 7, 5, 7], [7, 1, 1, 1, 1], [7, 5, 7, 5, 7], [7, 5, 7, 1, 7],
];

struct Canvas<'a> {
    w: i64,
    h: i64,
    px: &'a mut [u8],
}

impl Canvas<'_> {
    fn blend(&mut self, x: i64, y: i64, c: [u8; 3], a: f32) {
        if x < 0 || y < 0 || x >= self.w || y >= self.h {
            return;
        }
        let i = ((y * self.w + x) * 4) as usize;
        for k in 0..3 {
            self.px[i + k] = (self.px[i + k] as f32 * (1.0 - a) + c[k] as f32 * a) as u8;
        }
    }

    fn rect(&mut self, x: i64, y: i64, w: i64, h: i64, c: [u8; 3], a: f32) {
        for yy in y..y + h {
            for xx in x..x + w {
                self.blend(xx, yy, c, a);
            }
        }
    }

    /// Numero con cifre 3×5 ingrandite `s` volte, su un riquadro scuro.
    fn number(&mut self, x: i64, y: i64, n: i64, s: i64) {
        let text = n.to_string();
        let tw = text.len() as i64 * 4 * s + s;
        self.rect(x - s, y - s, tw + s, 7 * s, [0, 0, 0], 0.75);
        for (k, ch) in text.bytes().enumerate() {
            let Some(d) = (ch as char).to_digit(10) else { continue };
            for (row, bits) in DIGITS[d as usize].iter().enumerate() {
                for col in 0..3 {
                    if bits & (4 >> col) != 0 {
                        self.rect(x + k as i64 * 4 * s + col * s, y + row as i64 * s, s, s, [255, 230, 0], 1.0);
                    }
                }
            }
        }
    }
}

/// Zona dello schermo mostrata da un'immagine, in coordinate della griglia (quelle degli strumenti mouse).
#[derive(Clone, Copy)]
pub struct View {
    pub gx: f64,
    pub gy: f64,
    /// pixel dell'immagine per unità di griglia
    pub ppu: f64,
}

/// Passo dei righelli (in unità di griglia) perché le tacche stiano ad almeno ~45 pixel.
fn tick_step(ppu: f64) -> i64 {
    [5, 10, 20, 25, 50, 100, 200, 250, 500].into_iter().find(|s| *s as f64 * ppu >= 45.0).unwrap_or(1000)
}

/// Linee leggere della griglia, numeri sui bordi in alto e a sinistra, cursore in rosso.
pub fn annotate(w: u32, h: u32, rgba: &mut [u8], view: View, cursor: Option<(f64, f64)>, grid: bool) {
    let mut c = Canvas { w: w as i64, h: h as i64, px: rgba };
    if grid {
        let step = tick_step(view.ppu);
        let s = if w >= 900 { 2 } else { 1 };
        let first = |g0: f64| ((g0 / step as f64).ceil() as i64) * step;
        // verticali + etichette in alto
        let mut g = first(view.gx);
        loop {
            let x = ((g as f64 - view.gx) * view.ppu).round() as i64;
            if x >= c.w {
                break;
            }
            for y in 0..c.h {
                c.blend(x, y, [255, 0, 200], if y % 6 < 3 { 0.35 } else { 0.0 });
            }
            c.number(x + 2 * s, s + 1, g, s);
            g += step;
        }
        // orizzontali + etichette a sinistra
        let mut g = first(view.gy);
        loop {
            let y = ((g as f64 - view.gy) * view.ppu).round() as i64;
            if y >= c.h {
                break;
            }
            for x in 0..c.w {
                c.blend(x, y, [255, 0, 200], if x % 6 < 3 { 0.35 } else { 0.0 });
            }
            if y > 8 * s {
                c.number(s + 1, y + 2 * s, g, s);
            }
            g += step;
        }
    }
    if let Some((gx, gy)) = cursor {
        let x = ((gx - view.gx) * view.ppu).round() as i64;
        let y = ((gy - view.gy) * view.ppu).round() as i64;
        for d in -14i64..=14 {
            if d.abs() > 3 {
                c.blend(x + d, y, [255, 0, 0], 1.0);
                c.blend(x, y + d, [255, 0, 0], 1.0);
            }
        }
        for i in 0..64 {
            let t = i as f64 / 64.0 * std::f64::consts::TAU;
            c.blend(x + (9.0 * t.cos()).round() as i64, y + (9.0 * t.sin()).round() as i64, [255, 0, 0], 1.0);
        }
    }
}

pub fn encode_png(w: u32, h: u32, rgba: &[u8]) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    {
        let mut enc = png::Encoder::new(&mut out, w, h);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        let mut writer = enc.write_header().map_err(|e| e.to_string())?;
        writer.write_image_data(rgba).map_err(|e| e.to_string())?;
    }
    Ok(out)
}

fn b64(w: u32, h: u32, rgba: &[u8]) -> Result<String, String> {
    use base64::Engine;
    Ok(base64::engine::general_purpose::STANDARD.encode(encode_png(w, h, rgba)?))
}

/// Posizione del cursore in coordinate di griglia.
fn cursor_grid() -> (f64, f64) {
    let (cx, cy) = super::input::pointer::cursor();
    let (_, _, f) = super::input::grid();
    (cx as f64 / f, cy as f64 / f)
}

/// Screenshot piccolo (lato lungo `max` px) senza righelli, per chi deve solo "capire" cosa c'è.
pub fn small_base64(max: u32) -> Result<String, String> {
    let (w, h, rgba) = capture_rgba()?;
    let (nw, nh) = if w >= h { (max.min(w), ((h as f64) * max.min(w) as f64 / w as f64).round() as u32) } else { (((w as f64) * max.min(h) as f64 / h as f64).round() as u32, max.min(h)) };
    b64(nw, nh, &resample(w, h, &rgba, nw.max(1), nh.max(1)))
}

/// Screenshot di tutto lo schermo principale alla dimensione della griglia (righelli e cursore se `grid`).
pub fn screenshot_base64(grid: bool) -> Result<String, String> {
    let (w, h, rgba) = capture_rgba()?;
    let (gw, gh, _) = super::input::grid();
    let mut img = resample(w, h, &rgba, gw as u32, gh as u32);
    annotate(gw as u32, gh as u32, &mut img, View { gx: 0.0, gy: 0.0, ppu: 1.0 }, grid.then(cursor_grid), grid);
    b64(gw as u32, gh as u32, &img)
}

/// Ingrandimento di una zona (coordinate di griglia): pixel veri dello schermo, righelli con le
/// coordinate della griglia, così il modello legge direttamente dove cliccare.
/// Restituisce l'immagine e la zona effettiva.
pub fn zoom_base64(gx: i32, gy: i32, gw: i32, gh: i32, grid: bool) -> Result<(String, (i32, i32, i32, i32)), String> {
    let (w, h, rgba) = capture_rgba()?;
    let (tw, th, _) = super::input::grid();
    // la cattura può avere una risoluzione diversa da quella dichiarata (scala di Wayland)
    let f = w as f64 / tw as f64;
    let gw = gw.clamp(20, tw);
    let gh = gh.clamp(12, th);
    let gx = gx.clamp(0, tw - gw);
    let gy = gy.clamp(0, th - gh);
    let (sx, sy) = ((gx as f64 * f) as u32, (gy as f64 * f) as u32);
    let sw = ((gw as f64 * f) as u32).min(w - sx).max(1);
    let sh = ((gh as f64 * f) as u32).min(h - sy).max(1);
    let mut crop = Vec::with_capacity((sw * sh * 4) as usize);
    for y in sy..sy + sh {
        let i = ((y * w + sx) * 4) as usize;
        crop.extend_from_slice(&rgba[i..i + (sw * 4) as usize]);
    }
    // al massimo 4× i pixel veri, e non più larga degli screenshot normali
    let scale = (super::input::shot_width() as f64 / sw as f64).min(4.0);
    let (nw, nh) = (((sw as f64 * scale).round() as u32).max(1), ((sh as f64 * scale).round() as u32).max(1));
    let mut img = resample(sw, sh, &crop, nw, nh);
    let view = View { gx: gx as f64, gy: gy as f64, ppu: nw as f64 / gw as f64 };
    let cur = cursor_grid();
    let inside = cur.0 >= gx as f64 && cur.1 >= gy as f64 && cur.0 < (gx + gw) as f64 && cur.1 < (gy + gh) as f64;
    annotate(nw, nh, &mut img, view, (grid && inside).then_some(cur), grid);
    Ok((b64(nw, nh, &img)?, (gx, gy, gw, gh)))
}

#[cfg(test)]
mod tests {
    #[test]
    fn downscale_and_png() {
        let rgba = vec![200u8; 2560 * 10 * 4];
        let (w, h, out) = super::downscale(2560, 10, &rgba, 1280);
        assert_eq!((w, h, out.len()), (1280, 5, 1280 * 5 * 4));
        // rapporto non intero: 1920 → 1280
        let (w3, h3, o3) = super::downscale(1920, 1080, &vec![9u8; 1920 * 1080 * 4], 1280);
        assert_eq!((w3, h3, o3.len()), (1280, 720, 1280 * 720 * 4));
        assert_eq!(o3[0], 9);
        let png = super::encode_png(w, h, &out).unwrap();
        assert_eq!(&png[1..4], b"PNG");
        let (w2, h2, back) = super::decode_png(&png).unwrap();
        assert_eq!((w2, h2), (1280, 5));
        assert_eq!(&back[..4], &out[..4]);
    }

    /// Salva screenshot e zoom annotati per controllarli a occhio: AE_SHOT_DIR=cartella (serve uno schermo).
    #[test]
    fn annotated_shots_to_disk() {
        let Ok(dir) = std::env::var("AE_SHOT_DIR") else { return };
        use base64::Engine;
        let dec = |b: &str| base64::engine::general_purpose::STANDARD.decode(b).unwrap();
        std::fs::write(format!("{dir}/full.png"), dec(&super::screenshot_base64(true).unwrap())).unwrap();
        let (z, area) = super::zoom_base64(100, 80, 240, 135, true).unwrap();
        std::fs::write(format!("{dir}/zoom.png"), dec(&z)).unwrap();
        assert_eq!(area, (100, 80, 240, 135));
    }

    #[test]
    fn rulers_and_cursor() {
        let (w, h) = (400u32, 300u32);
        let mut img = vec![255u8; (w * h * 4) as usize];
        super::annotate(w, h, &mut img, super::View { gx: 100.0, gy: 50.0, ppu: 2.0 }, Some((150.0, 100.0)), true);
        // centro del cursore: (150-100)*2 = 100, (100-50)*2 = 100 → la croce passa a 5 px dal centro
        let i = ((100 * w + 110) * 4) as usize;
        assert_eq!(&img[i..i + 3], &[255, 0, 0]);
        // passo 25 unità (50 px): linea verticale della griglia a x = 0 (g = 100)
        assert!(img[((2 * w) * 4) as usize + 1] < 255);
        assert_eq!(super::tick_step(1.0), 50);
        assert_eq!(super::tick_step(4.0), 20);
    }
}
