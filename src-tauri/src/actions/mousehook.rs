//! Push-to-talk con i tasti del mouse (Mouse4, Mouse5, MouseMiddle) tramite hook a basso livello.
//! Le hotkey globali di Tauri non supportano il mouse, quindi serve questo hook (solo Windows).
//! L'evento del mouse non viene bloccato: arriva comunque alle altre app.

use std::sync::{atomic::{AtomicU8, Ordering}, OnceLock};
use tauri::AppHandle;

/// 0 = nessuno, 1 = Mouse4 (XButton1), 2 = Mouse5 (XButton2), 3 = tasto centrale.
static BUTTON: AtomicU8 = AtomicU8::new(0);
static APP: OnceLock<AppHandle> = OnceLock::new();

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
    use std::sync::Once;
    use windows_sys::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
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

    pub fn ensure_started() {
        START.call_once(|| {
            std::thread::spawn(|| unsafe {
                let h = SetWindowsHookExW(WH_MOUSE_LL, Some(hook), std::ptr::null_mut(), 0);
                if h.is_null() {
                    eprintln!("hook del mouse non installato");
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
