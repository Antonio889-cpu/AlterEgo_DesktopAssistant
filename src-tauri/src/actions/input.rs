//! Tasti simulati (SendInput): scorciatoie delle clip e tasti multimediali del volume.

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
