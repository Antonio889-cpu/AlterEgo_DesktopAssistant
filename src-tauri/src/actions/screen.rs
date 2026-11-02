//! Screenshot dello schermo principale (GDI), ridotto a max 1280 px di larghezza, PNG in base64.

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

#[cfg(not(windows))]
fn capture_rgba() -> Result<(u32, u32, Vec<u8>), String> {
    Err("lo screenshot funziona solo su Windows".into())
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
    let (w, h, rgba) = downscale(w, h, &rgba, 1280);
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
    }
}
