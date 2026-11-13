//! Push-to-talk tramite hook a basso livello (solo Windows): tasti del mouse (Mouse4, Mouse5,
//! MouseMiddle), che le hotkey globali non supportano, e combinazioni di tastiera, perché molti giochi
//! (Raw Input con RIDEV_NOHOTKEYS) bloccano le hotkey registrate ma non gli hook.
//! Gli eventi non vengono bloccati: arrivano comunque alle altre app.

use std::sync::{atomic::{AtomicU32, AtomicU8, Ordering}, OnceLock};
use tauri::AppHandle;

/// 0 = nessuno, 1 = Mouse4 (XButton1), 2 = Mouse5 (XButton2), 3 = tasto centrale.
static BUTTON: AtomicU8 = AtomicU8::new(0);
static APP: OnceLock<AppHandle> = OnceLock::new();
/// Combinazione da tastiera: bit 0-7 virtual key, 8 Ctrl, 9 Alt, 10 Shift, 11 Win (0 = nessuna).
static KEY: AtomicU32 = AtomicU32::new(0);

/// "Ctrl+Alt+Space" → virtual key e modificatori (vedi `KEY`). None per tasti non gestiti.
pub fn parse_combo(s: &str) -> Option<u32> {
    let mut mods = 0u32;
    let mut vk = 0u32;
    for part in s.split('+').map(|p| p.trim().to_ascii_lowercase()) {
        match part.as_str() {
            "ctrl" | "control" | "commandorcontrol" | "cmdorctrl" => mods |= 1 << 8,
            "alt" | "option" => mods |= 1 << 9,
            "shift" => mods |= 1 << 10,
            "super" | "win" | "meta" | "cmd" | "command" => mods |= 1 << 11,
            k => {
                let b = k.as_bytes();
                vk = match k {
                    _ if b.len() == 1 && (b[0].is_ascii_lowercase() || b[0].is_ascii_digit()) => b[0].to_ascii_uppercase() as u32,
                    _ if k.starts_with("key") && b.len() == 4 && b[3].is_ascii_lowercase() => b[3].to_ascii_uppercase() as u32,
                    _ if k.starts_with("digit") && b.len() == 6 && b[5].is_ascii_digit() => b[5] as u32,
                    _ if k.starts_with('f') && k[1..].parse::<u32>().is_ok_and(|n| (1..=24).contains(&n)) => 0x6F + k[1..].parse::<u32>().ok()?,
                    "space" => 0x20,
                    "enter" | "return" => 0x0D,
                    "tab" => 0x09,
                    "capslock" => 0x14,
                    "backquote" | "`" => 0xC0,
                    "insert" => 0x2D,
                    "delete" => 0x2E,
                    "home" => 0x24,
                    "end" => 0x23,
                    "pageup" => 0x21,
                    "pagedown" => 0x22,
                    "pause" => 0x13,
                    "scrolllock" => 0x91,
                    "up" | "arrowup" => 0x26,
                    "down" | "arrowdown" => 0x28,
                    "left" | "arrowleft" => 0x25,
                    "right" | "arrowright" => 0x27,
                    _ if k.starts_with("numpad") && b.len() == 7 && b[6].is_ascii_digit() => 0x60 + (b[6] - b'0') as u32,
                    _ => return None,
                };
            }
        }
    }
    (vk != 0).then_some(vk | mods)
}

/// Imposta la combinazione da tastiera del push-to-talk (0 = disattiva).
pub fn set_key(app: &AppHandle, key: u32) {
    let _ = APP.set(app.clone());
    KEY.store(key, Ordering::SeqCst);
    if key != 0 {
        imp::ensure_started();
    }
}

#[cfg_attr(not(windows), allow(dead_code))]
fn fire_key(pressed: bool) {
    if let Some(app) = APP.get() {
        crate::assistant::ptt(app, pressed);
    }
}

/// "Mouse4" → 1 ecc. None se la stringa non è un tasto del mouse.
pub fn parse(s: &str) -> Option<u8> {
    match s.trim().to_ascii_lowercase().as_str() {
        "mouse4" | "xbutton1" => Some(1),
        "mouse5" | "xbutton2" => Some(2),
        "mousemiddle" | "mouse3" => Some(3),
        _ => None,
    }
}

/// Imposta il tasto del mouse per il push-to-talk (0 = disattiva).
pub fn set_button(app: &AppHandle, button: u8) {
    let _ = APP.set(app.clone());
    BUTTON.store(button, Ordering::SeqCst);
    if button != 0 {
        imp::ensure_started();
    }
}

#[cfg_attr(not(windows), allow(dead_code))]
fn fire(button: u8, pressed: bool) {
    if button != 0 && BUTTON.load(Ordering::SeqCst) == button {
        if let Some(app) = APP.get() {
            crate::assistant::ptt(app, pressed);
        }
    }
}

#[cfg(windows)]
mod imp {
    use std::sync::{atomic::{AtomicBool, Ordering}, Once};
    use windows_sys::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::*;
    use windows_sys::Win32::UI::WindowsAndMessaging::*;

    static START: Once = Once::new();

    unsafe extern "system" fn hook(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        if code >= 0 {
            let info = &*(lparam as *const MSLLHOOKSTRUCT);
            let x = ((info.mouseData >> 16) & 0xFFFF) as u8; // 1 = XBUTTON1, 2 = XBUTTON2
            match wparam as u32 {
                WM_XBUTTONDOWN => super::fire(x, true),
                WM_XBUTTONUP => super::fire(x, false),
                WM_MBUTTONDOWN => super::fire(3, true),
                WM_MBUTTONUP => super::fire(3, false),
                _ => {}
            }
        }
        CallNextHookEx(std::ptr::null_mut(), code, wparam, lparam)
    }

    static DOWN: AtomicBool = AtomicBool::new(false);

    unsafe extern "system" fn kb_hook(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        let key = super::KEY.load(Ordering::Relaxed);
        if code >= 0 && key != 0 {
            let vk = (*(lparam as *const KBDLLHOOKSTRUCT)).vkCode;
            let down = matches!(wparam as u32, WM_KEYDOWN | WM_SYSKEYDOWN);
            let held = |v: u16| GetAsyncKeyState(v as i32) as u16 & 0x8000 != 0;
            let mods_ok = (key & 1 << 8 == 0 || held(VK_CONTROL))
                && (key & 1 << 9 == 0 || held(VK_MENU))
                && (key & 1 << 10 == 0 || held(VK_SHIFT))
                && (key & 1 << 11 == 0 || held(VK_LWIN) || held(VK_RWIN));
            let is_mod = |bit: u32, vks: &[u16]| key & bit != 0 && vks.contains(&(vk as u16));
            let releases_mod = is_mod(1 << 8, &[VK_CONTROL, VK_LCONTROL, VK_RCONTROL])
                || is_mod(1 << 9, &[VK_MENU, VK_LMENU, VK_RMENU])
                || is_mod(1 << 10, &[VK_SHIFT, VK_LSHIFT, VK_RSHIFT])
                || is_mod(1 << 11, &[VK_LWIN, VK_RWIN]);
            if vk == key & 0xFF && down && mods_ok {
                if !DOWN.swap(true, Ordering::SeqCst) {
                    super::fire_key(true);
                }
            } else if !down && (vk == key & 0xFF || releases_mod) && DOWN.swap(false, Ordering::SeqCst) {
                super::fire_key(false);
            }
        }
        CallNextHookEx(std::ptr::null_mut(), code, wparam, lparam)
    }

    pub fn ensure_started() {
        START.call_once(|| {
            std::thread::spawn(|| unsafe {
                let h = SetWindowsHookExW(WH_MOUSE_LL, Some(hook), std::ptr::null_mut(), 0);
                let k = SetWindowsHookExW(WH_KEYBOARD_LL, Some(kb_hook), std::ptr::null_mut(), 0);
                if h.is_null() || k.is_null() {
                    crate::applog::warn("hotkey", "hook di mouse/tastiera non installato");
                }
                if h.is_null() && k.is_null() {
                    return;
                }
                let mut msg: MSG = std::mem::zeroed();
                while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {
                    TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
            });
        });
    }
}

#[cfg(not(windows))]
mod imp {
    pub fn ensure_started() {}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn combos() {
        assert_eq!(parse_combo("Ctrl+Alt+Space"), Some(0x20 | 1 << 8 | 1 << 9));
        assert_eq!(parse_combo("F8"), Some(0x77));
        assert_eq!(parse_combo("Shift+KeyV"), Some(0x56 | 1 << 10));
        assert_eq!(parse_combo("CapsLock"), Some(0x14));
        assert_eq!(parse_combo("Ctrl+Alt"), None);
        assert_eq!(parse_combo("Ctrl+Boh"), None);
    }
}
