//! Screenshot dello schermo principale (GDI su Windows, grim/gnome-screenshot/scrot… su Linux), ridotto a max 1280 px di larghezza, PNG in base64.

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

/// Riduzione semplice (media dei pixel) per stare sotto `max_w`.
pub fn downscale(w: u32, h: u32, rgba: &[u8], max_w: u32) -> (u32, u32, Vec<u8>) {
    if w <= max_w {
        return (w, h, rgba.to_vec());
    }
    let f = (w as f64 / max_w as f64).ceil() as u32;
    let (nw, nh) = (w / f, h / f);
    let mut out = vec![0u8; (nw * nh * 4) as usize];
    for y in 0..nh {
        for x in 0..nw {
            let mut acc = [0u32; 4];
            for dy in 0..f {
                for dx in 0..f {
                    let i = (((y * f + dy) * w + x * f + dx) * 4) as usize;
                    for c in 0..4 {
                        acc[c] += rgba[i + c] as u32;
                    }
                }
            }
            let o = ((y * nw + x) * 4) as usize;
            for c in 0..4 {
                out[o + c] = (acc[c] / (f * f)) as u8;
            }
        }
    }
    (nw, nh, out)
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

pub fn screenshot_base64() -> Result<String, String> {
    use base64::Engine;
    let (w, h, rgba) = capture_rgba()?;
    let (w, h, rgba) = downscale(w, h, &rgba, super::input::shot_width());
    Ok(base64::engine::general_purpose::STANDARD.encode(encode_png(w, h, &rgba)?))
}

#[cfg(test)]
mod tests {
    #[test]
    fn downscale_and_png() {
        let rgba = vec![200u8; 2560 * 10 * 4];
        let (w, h, out) = super::downscale(2560, 10, &rgba, 1280);
        assert_eq!((w, h, out.len()), (1280, 5, 1280 * 5 * 4));
        let png = super::encode_png(w, h, &out).unwrap();
        assert_eq!(&png[1..4], b"PNG");
        let (w2, h2, back) = super::decode_png(&png).unwrap();
        assert_eq!((w2, h2), (1280, 5));
        assert_eq!(&back[..4], &out[..4]);
    }
}
