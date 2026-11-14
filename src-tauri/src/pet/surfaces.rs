//! Finestre aperte (per stare in piedi sopra) e altre domande al sistema operativo.
//! Solo Windows; su Linux il pet usa soltanto i bordi dei monitor e la barra.

use super::physics::WinRect;

#[cfg(windows)]
mod imp {
    use super::WinRect;
    use std::ffi::c_void;
    use windows_sys::Win32::{
        Foundation::{BOOL, HWND, LPARAM, RECT},
        Graphics::{
            Dwm::{DwmGetWindowAttribute, DWMWA_CLOAKED, DWMWA_EXTENDED_FRAME_BOUNDS},
            Gdi::{GetMonitorInfoW, MonitorFromWindow, MONITORINFO, MONITOR_DEFAULTTONEAREST},
        },
        UI::{
            Input::KeyboardAndMouse::{GetAsyncKeyState, VK_LBUTTON},
            WindowsAndMessaging::{
                EnumWindows, GetClassNameW, GetForegroundWindow, GetWindowLongW, GetWindowRect, GetWindowTextLengthW, IsIconic, IsWindowVisible,
                IsZoomed, GWL_EXSTYLE, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TRANSPARENT,
            },
        },
    };

    /// Finestre del desktop e della barra: non sono "finestre" su cui stare.
    const SHELL: [&str; 5] = ["Progman", "WorkerW", "Shell_TrayWnd", "Shell_SecondaryTrayWnd", "NotifyIconOverflowWindow"];

    unsafe extern "system" fn collect(hwnd: HWND, lp: LPARAM) -> BOOL {
        let v = &mut *(lp as *mut Vec<HWND>);
        v.push(hwnd);
        1
    }

    fn class(hwnd: HWND) -> String {
        let mut buf = [0u16; 128];
        let n = unsafe { GetClassNameW(hwnd, buf.as_mut_ptr(), buf.len() as i32) };
        String::from_utf16_lossy(&buf[..n.max(0) as usize])
    }

    fn bounds(hwnd: HWND) -> Option<RECT> {
        let mut r = RECT { left: 0, top: 0, right: 0, bottom: 0 };
        // senza l'ombra invisibile di Windows 10/11
        let hr = unsafe { DwmGetWindowAttribute(hwnd, DWMWA_EXTENDED_FRAME_BOUNDS as u32, &mut r as *mut RECT as *mut c_void, std::mem::size_of::<RECT>() as u32) };
        if hr != 0 && unsafe { GetWindowRect(hwnd, &mut r) } == 0 {
            return None;
        }
        Some(r)
    }

    /// Finestre visibili in ordine di z (la prima è quella davanti), escluso `skip`.
    pub fn windows(skip: isize) -> Vec<WinRect> {
        let mut list: Vec<HWND> = Vec::new();
        unsafe { EnumWindows(Some(collect), &mut list as *mut Vec<HWND> as LPARAM) };
        let mut out = Vec::new();
        for hwnd in list {
            if hwnd as isize == skip || unsafe { IsWindowVisible(hwnd) } == 0 || unsafe { IsIconic(hwnd) } != 0 {
                continue;
            }
            // finestre "nascoste" da DWM (altri desktop virtuali, app UWP sospese)
            let mut cloaked: u32 = 0;
            unsafe { DwmGetWindowAttribute(hwnd, DWMWA_CLOAKED as u32, &mut cloaked as *mut u32 as *mut c_void, 4) };
            if cloaked != 0 {
                continue;
            }
            let ex = unsafe { GetWindowLongW(hwnd, GWL_EXSTYLE) } as u32;
            // overlay trasparenti al mouse (es. quelli dei giochi e delle schede video), menu, tooltip
            if ex & WS_EX_TRANSPARENT != 0 || unsafe { GetWindowTextLengthW(hwnd) } == 0 {
                continue;
            }
            if SHELL.contains(&class(hwnd).as_str()) {
                continue;
            }
            let Some(r) = bounds(hwnd) else { continue };
            if r.right - r.left < 2 || r.bottom - r.top < 2 {
                continue;
            }
            let standable = ex & (WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE) == 0 && unsafe { IsZoomed(hwnd) } == 0;
            out.push(WinRect { id: hwnd as usize as u64, l: r.left as f64, t: r.top as f64, r: r.right as f64, b: r.bottom as f64, standable });
        }
        out
    }

    /// In primo piano c'è un'app a schermo intero (gioco, video, presentazione)?
    pub fn fullscreen_foreground() -> bool {
        let hwnd = unsafe { GetForegroundWindow() };
        if hwnd.is_null() || SHELL.contains(&class(hwnd).as_str()) {
            return false;
        }
        let mut r = RECT { left: 0, top: 0, right: 0, bottom: 0 };
        if unsafe { GetWindowRect(hwnd, &mut r) } == 0 {
            return false;
        }
        let mon = unsafe { MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST) };
        let mut mi: MONITORINFO = unsafe { std::mem::zeroed() };
        mi.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
        if unsafe { GetMonitorInfoW(mon, &mut mi) } == 0 {
            return false;
        }
        let m = mi.rcMonitor;
        r.left <= m.left && r.top <= m.top && r.right >= m.right && r.bottom >= m.bottom
    }

    /// Tasto sinistro del mouse premuto (per non perdere il rilascio durante il trascinamento).
    pub fn left_button_down() -> Option<bool> {
        Some(unsafe { GetAsyncKeyState(VK_LBUTTON as i32) } as u16 & 0x8000 != 0)
    }
}

#[cfg(not(windows))]
mod imp {
    use super::WinRect;

    pub fn windows(_skip: isize) -> Vec<WinRect> {
        Vec::new()
    }

    pub fn fullscreen_foreground() -> bool {
        false
    }

    pub fn left_button_down() -> Option<bool> {
        None
    }
}

pub use imp::{fullscreen_foreground, left_button_down, windows};
