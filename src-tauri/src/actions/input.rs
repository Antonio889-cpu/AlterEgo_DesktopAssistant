//! Tasti e mouse simulati (SendInput): scorciatoie delle clip, tasti multimediali del volume
//! e il controllo di mouse e tastiera da parte di Alter Ego (permesso "Mouse e tastiera").

/// Codici virtual-key di Windows per una combinazione tipo "Ctrl+Shift+S" o "F8".
pub fn parse_combo(combo: &str) -> Result<Vec<u16>, String> {
    let mut mods = Vec::new();
    let mut key = None;
    for part in combo.split('+').map(|p| p.trim()).filter(|p| !p.is_empty()) {
        let up = part.to_ascii_uppercase();
        let vk: u16 = match up.as_str() {
            "CTRL" | "CONTROL" => { mods.push(0x11); continue; }
            "ALT" => { mods.push(0x12); continue; }
            "SHIFT" => { mods.push(0x10); continue; }
            "WIN" | "SUPER" | "META" => { mods.push(0x5B); continue; }
            "SPACE" => 0x20,
            "ENTER" | "RETURN" => 0x0D,
            "TAB" => 0x09,
            "ESC" | "ESCAPE" => 0x1B,
            "BACKSPACE" => 0x08,
            "INSERT" => 0x2D,
            "DELETE" | "DEL" => 0x2E,
            "HOME" => 0x24,
            "END" => 0x23,
            "PAGEUP" => 0x21,
            "PAGEDOWN" => 0x22,
            "LEFT" => 0x25,
            "UP" => 0x26,
            "RIGHT" => 0x27,
            "DOWN" => 0x28,
            "PRINTSCREEN" => 0x2C,
            "PAUSE" => 0x13,
            s if s.len() == 1 && s.as_bytes()[0].is_ascii_alphanumeric() => s.as_bytes()[0] as u16,
            s if s.starts_with('F') && s[1..].parse::<u16>().map(|n| (1..=24).contains(&n)).unwrap_or(false) => {
                0x70 + s[1..].parse::<u16>().unwrap() - 1
            }
            s if s.starts_with("NUMPAD") && s[6..].parse::<u16>().map(|n| n <= 9).unwrap_or(false) => {
                0x60 + s[6..].parse::<u16>().unwrap()
            }
            _ => return Err(crate::tr!("tasto non riconosciuto: {part}", "key not recognized: {part}")),
        };
        if key.replace(vk).is_some() {
            return Err(crate::i18n::tr("una combinazione può avere un solo tasto principale", "a combination can have only one main key").into());
        }
    }
    let key = key.ok_or(crate::i18n::tr("manca il tasto principale (es. Ctrl+Shift+S)", "the main key is missing (e.g. Ctrl+Shift+S)"))?;
    mods.push(key);
    Ok(mods)
}

#[cfg(windows)]
mod imp {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::*;

    fn event(vk: u16, up: bool) -> INPUT {
        // tasti "estesi": frecce, ins/canc, home/fine, pag
        let extended = matches!(vk, 0x21..=0x28 | 0x2D | 0x2E | 0x5B | 0xAD..=0xAF);
        let scan = unsafe { MapVirtualKeyW(vk as u32, MAPVK_VK_TO_VSC) } as u16;
        let mut flags = 0;
        if up {
            flags |= KEYEVENTF_KEYUP;
        }
        if extended {
            flags |= KEYEVENTF_EXTENDEDKEY;
        }
        INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 { ki: KEYBDINPUT { wVk: vk, wScan: scan, dwFlags: flags, time: 0, dwExtraInfo: 0 } },
        }
    }

    fn send(events: &[INPUT]) -> Result<(), String> {
        let sent = unsafe { SendInput(events.len() as u32, events.as_ptr(), std::mem::size_of::<INPUT>() as i32) };
        if sent as usize == events.len() { Ok(()) } else { Err(crate::i18n::tr("Windows ha bloccato i tasti simulati", "Windows blocked the simulated keys").into()) }
    }

    /// Preme la combinazione: modificatori giù, tasto, poi tutto su (con piccole pause:
    /// alcuni programmi di registrazione ignorano gli eventi troppo ravvicinati).
    pub fn press(vks: &[u16]) -> Result<(), String> {
        for vk in vks {
            send(&[event(*vk, false)])?;
            std::thread::sleep(std::time::Duration::from_millis(25));
        }
        std::thread::sleep(std::time::Duration::from_millis(60));
        for vk in vks.iter().rev() {
            send(&[event(*vk, true)])?;
            std::thread::sleep(std::time::Duration::from_millis(15));
        }
        Ok(())
    }
}

/// Controllo di mouse e tastiera (solo Windows).
#[cfg(windows)]
pub mod pointer {
    use windows_sys::Win32::Foundation::POINT;
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::*;
    use windows_sys::Win32::UI::WindowsAndMessaging::*;

    fn send(events: &[INPUT]) -> Result<(), String> {
        let sent = unsafe { SendInput(events.len() as u32, events.as_ptr(), std::mem::size_of::<INPUT>() as i32) };
        if sent as usize == events.len() { Ok(()) } else { Err(crate::i18n::tr("Windows ha bloccato l'input simulato", "Windows blocked the simulated input").into()) }
    }

    fn mouse(flags: u32, data: i32) -> INPUT {
        INPUT {
            r#type: INPUT_MOUSE,
            Anonymous: INPUT_0 { mi: MOUSEINPUT { dx: 0, dy: 0, mouseData: data as _, dwFlags: flags, time: 0, dwExtraInfo: 0 } },
        }
    }

    fn unicode(unit: u16, up: bool) -> INPUT {
        INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT { wVk: 0, wScan: unit, dwFlags: KEYEVENTF_UNICODE | if up { KEYEVENTF_KEYUP } else { 0 }, time: 0, dwExtraInfo: 0 },
            },
        }
    }

    /// Dimensione dello schermo principale in pixel.
    pub fn screen_size() -> (i32, i32) {
        unsafe { (GetSystemMetrics(SM_CXSCREEN), GetSystemMetrics(SM_CYSCREEN)) }
    }

    pub fn cursor() -> (i32, i32) {
        let mut p = POINT { x: 0, y: 0 };
        unsafe { GetCursorPos(&mut p) };
        (p.x, p.y)
    }

    pub fn move_to(x: i32, y: i32) -> Result<(), String> {
        if unsafe { SetCursorPos(x, y) } == 0 { Err("cursore non spostato".into()) } else { Ok(()) }
    }

    /// `button`: "left", "right", "middle".
    pub fn click(button: &str, double: bool) -> Result<(), String> {
        let (down, up) = match button {
            "right" => (MOUSEEVENTF_RIGHTDOWN, MOUSEEVENTF_RIGHTUP),
            "middle" => (MOUSEEVENTF_MIDDLEDOWN, MOUSEEVENTF_MIDDLEUP),
            _ => (MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP),
        };
        for _ in 0..if double { 2 } else { 1 } {
            send(&[mouse(down, 0)])?;
            std::thread::sleep(std::time::Duration::from_millis(30));
            send(&[mouse(up, 0)])?;
            std::thread::sleep(std::time::Duration::from_millis(60));
        }
        Ok(())
    }

    /// Rotella: positivo = su, negativo = giù (in "scatti").
    pub fn scroll(clicks: i32, horizontal: bool) -> Result<(), String> {
        send(&[mouse(if horizontal { MOUSEEVENTF_HWHEEL } else { MOUSEEVENTF_WHEEL }, clicks * 120)])
    }

    /// Scrive un testo carattere per carattere (Unicode); "\n" = Invio.
    pub fn type_text(text: &str) -> Result<(), String> {
        for ch in text.chars() {
            if ch == '\n' {
                super::imp::press(&[0x0D])?;
                continue;
            }
            if ch == '\r' {
                continue;
            }
            let mut buf = [0u16; 2];
            for unit in ch.encode_utf16(&mut buf).iter() {
                send(&[unicode(*unit, false), unicode(*unit, true)])?;
            }
            std::thread::sleep(std::time::Duration::from_millis(8));
        }
        Ok(())
    }

    /// Finestra in primo piano (per rimetterla dopo una conferma).
    pub fn foreground() -> isize {
        unsafe { GetForegroundWindow() as isize }
    }

    pub fn set_foreground(h: isize) {
        if h != 0 {
            unsafe { SetForegroundWindow(h as _) };
        }
    }

    /// La finestra in primo piano è di Alter Ego.
    pub fn foreground_is_ours() -> bool {
        let mut pid = 0u32;
        unsafe {
            GetWindowThreadProcessId(GetForegroundWindow(), &mut pid);
            pid == windows_sys::Win32::System::Threading::GetCurrentProcessId()
        }
    }
}

#[cfg(not(windows))]
pub mod pointer {
    const NO: &str = "mouse e tastiera simulati funzionano solo su Windows";
    pub fn screen_size() -> (i32, i32) { (1920, 1080) }
    pub fn cursor() -> (i32, i32) { (500, 500) }
    pub fn move_to(_x: i32, _y: i32) -> Result<(), String> { Err(NO.into()) }
    pub fn click(_b: &str, _d: bool) -> Result<(), String> { Err(NO.into()) }
    pub fn scroll(_c: i32, _h: bool) -> Result<(), String> { Err(NO.into()) }
    pub fn type_text(_t: &str) -> Result<(), String> { Err(NO.into()) }
    pub fn foreground() -> isize { 0 }
    pub fn set_foreground(_h: isize) {}
    pub fn foreground_is_ours() -> bool { false }
}

/// Larghezza massima degli screenshot (Impostazioni → Risparmio token).
static SHOT_W: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(1280);

pub fn set_shot_width(w: u32) {
    SHOT_W.store(w.clamp(512, 1920), std::sync::atomic::Ordering::Relaxed);
}

pub fn shot_width() -> u32 {
    SHOT_W.load(std::sync::atomic::Ordering::Relaxed)
}

/// Griglia delle coordinate usata dal modello: quella dello screenshot (ridotto a `shot_width`).
/// Restituisce (larghezza, altezza, fattore griglia→schermo).
pub fn grid() -> (i32, i32, f64) {
    let (w, h) = pointer::screen_size();
    let f = ((w as f64) / shot_width() as f64).ceil().max(1.0);
    ((w as f64 / f) as i32, (h as f64 / f) as i32, f)
}

#[cfg(not(windows))]
mod imp {
    pub fn press(_vks: &[u16]) -> Result<(), String> {
        Err("i tasti simulati funzionano solo su Windows".into())
    }
}

pub fn send_combo(combo: &str) -> Result<(), String> {
    imp::press(&parse_combo(combo)?)
}

/// Tasti multimediali del volume: "up", "down", "mute".
pub fn volume(action: &str, steps: u32) -> Result<(), String> {
    let vk = match action {
        "up" => 0xAF,
        "down" => 0xAE,
        "mute" => return imp::press(&[0xAD]),
        _ => return Err("azione volume non valida".into()),
    };
    for _ in 0..steps.clamp(1, 25) {
        imp::press(&[vk])?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::parse_combo;

    #[test]
    fn combos() {
        assert_eq!(parse_combo("Ctrl+Shift+S").unwrap(), vec![0x11, 0x10, b'S' as u16]);
        assert_eq!(parse_combo("Alt+F10").unwrap(), vec![0x12, 0x79]);
        assert_eq!(parse_combo("F8").unwrap(), vec![0x77]);
        assert!(parse_combo("Ctrl+").is_err());
        assert!(parse_combo("Ctrl+A+B").is_err());
    }
}
