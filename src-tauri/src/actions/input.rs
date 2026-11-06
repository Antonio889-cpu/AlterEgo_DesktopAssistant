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

/// Linux: `xdotool` (X11; su Wayland solo per le app XWayland).
#[cfg(not(windows))]
pub mod pointer {
    use std::process::Command;

    fn xdo(args: &[&str]) -> Result<String, String> {
        let out = Command::new("xdotool").args(args).output().map_err(|_| {
            crate::i18n::tr("su Linux mouse e tastiera simulati usano xdotool: installalo (es. sudo apt install xdotool)", "on Linux simulated mouse and keyboard use xdotool: install it (e.g. sudo apt install xdotool)").to_string()
        })?;
        if out.status.success() {
            Ok(String::from_utf8_lossy(&out.stdout).to_string())
        } else {
            Err(format!("xdotool: {}", String::from_utf8_lossy(&out.stderr).trim()))
        }
    }

    pub(super) fn keys(names: &[&str]) -> Result<(), String> {
        xdo(&["key", "--clearmodifiers", &names.join("+")]).map(|_| ())
    }

    pub fn screen_size() -> (i32, i32) {
        xdo(&["getdisplaygeometry"])
            .ok()
            .and_then(|s| {
                let mut it = s.split_whitespace().filter_map(|n| n.parse().ok());
                Some((it.next()?, it.next()?))
            })
            .unwrap_or((1920, 1080))
    }

    pub fn cursor() -> (i32, i32) {
        // "X=123\nY=456\n…"
        xdo(&["getmouselocation", "--shell"])
            .ok()
            .and_then(|s| {
                let v = |k: &str| s.lines().find_map(|l| l.strip_prefix(k)).and_then(|n| n.trim().parse().ok());
                Some((v("X=")?, v("Y=")?))
            })
            .unwrap_or((0, 0))
    }

    pub fn move_to(x: i32, y: i32) -> Result<(), String> {
        xdo(&["mousemove", &x.to_string(), &y.to_string()]).map(|_| ())
    }

    pub fn click(button: &str, double: bool) -> Result<(), String> {
        let b = match button {
            "right" => "3",
            "middle" => "2",
            _ => "1",
        };
        if double {
            xdo(&["click", "--repeat", "2", "--delay", "60", b])
        } else {
            xdo(&["click", b])
        }
        .map(|_| ())
    }

    /// Rotella: positivo = su, negativo = giù (in "scatti").
    pub fn scroll(clicks: i32, horizontal: bool) -> Result<(), String> {
        let b = match (horizontal, clicks > 0) {
            (false, true) => "4",
            (false, false) => "5",
            (true, true) => "7",
            (true, false) => "6",
        };
        xdo(&["click", "--repeat", &clicks.unsigned_abs().clamp(1, 50).to_string(), "--delay", "20", b]).map(|_| ())
    }

    pub fn type_text(text: &str) -> Result<(), String> {
        for (i, line) in text.replace('\r', "").split('\n').enumerate() {
            if i > 0 {
                keys(&["Return"])?;
            }
            if !line.is_empty() {
                xdo(&["type", "--delay", "8", "--", line])?;
            }
        }
        Ok(())
    }

    pub fn foreground() -> isize {
        xdo(&["getactivewindow"]).ok().and_then(|s| s.trim().parse().ok()).unwrap_or(0)
    }

    pub fn set_foreground(h: isize) {
        if h != 0 {
            let _ = xdo(&["windowactivate", &h.to_string()]);
        }
    }

    pub fn foreground_is_ours() -> bool {
        xdo(&["getactivewindow", "getwindowpid"]).ok().and_then(|s| s.trim().parse::<u32>().ok()) == Some(std::process::id())
    }
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
    /// Codice virtual-key di Windows → nome del tasto X11 (keysym) per xdotool.
    pub fn keysym(vk: u16) -> Option<String> {
        Some(match vk {
            0x11 => "ctrl".into(),
            0x12 => "alt".into(),
            0x10 => "shift".into(),
            0x5B => "super".into(),
            0x20 => "space".into(),
            0x0D => "Return".into(),
            0x09 => "Tab".into(),
            0x1B => "Escape".into(),
            0x08 => "BackSpace".into(),
            0x2D => "Insert".into(),
            0x2E => "Delete".into(),
            0x24 => "Home".into(),
            0x23 => "End".into(),
            0x21 => "Prior".into(),
            0x22 => "Next".into(),
            0x25 => "Left".into(),
            0x26 => "Up".into(),
            0x27 => "Right".into(),
            0x28 => "Down".into(),
            0x2C => "Print".into(),
            0x13 => "Pause".into(),
            0xAF => "XF86AudioRaiseVolume".into(),
            0xAE => "XF86AudioLowerVolume".into(),
            0xAD => "XF86AudioMute".into(),
            0xB0 => "XF86AudioNext".into(),
            0xB1 => "XF86AudioPrev".into(),
            0xB3 => "XF86AudioPlay".into(),
            0x30..=0x39 => ((vk as u8) as char).to_string(),
            0x41..=0x5A => ((vk as u8) as char).to_ascii_lowercase().to_string(),
            0x60..=0x69 => format!("KP_{}", vk - 0x60),
            0x70..=0x87 => format!("F{}", vk - 0x6F),
            _ => return None,
        })
    }

    pub fn press(vks: &[u16]) -> Result<(), String> {
        let names: Vec<String> = vks.iter().map(|&v| keysym(v).ok_or_else(|| format!("tasto {v:#x} non supportato"))).collect::<Result<_, _>>()?;
        super::pointer::keys(&names.iter().map(String::as_str).collect::<Vec<_>>())
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

    #[cfg(not(windows))]
    #[test]
    fn linux_keysyms() {
        let names: Vec<String> = parse_combo("Ctrl+Shift+F5").unwrap().into_iter().map(|v| super::imp::keysym(v).unwrap()).collect();
        assert_eq!(names, ["ctrl", "shift", "F5"]);
        assert_eq!(super::imp::keysym(b'S' as u16).as_deref(), Some("s"));
        assert_eq!(super::imp::keysym(0x62).as_deref(), Some("KP_2"));
    }
}
