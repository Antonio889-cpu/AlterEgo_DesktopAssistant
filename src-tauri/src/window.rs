//! Finestra principale: creazione, geometria per monitor, click-through.

use crate::{config::Geometry, AppState};
use std::{sync::atomic::Ordering, thread, time::Duration};
use tauri::{
    AppHandle, Emitter, Manager, Monitor, PhysicalPosition, PhysicalSize, WebviewUrl,
    WebviewWindow, WebviewWindowBuilder,
};

pub const MAIN: &str = "main";
const RATIO: f64 = 16.0 / 9.0;
/// Lato (px logici) della maniglia che resta cliccabile in click-through (angolo in alto a destra).
const HANDLE_LOGICAL: f64 = 34.0;
const MIN_WIDTH_LOGICAL: f64 = 320.0;

pub fn create_main(app: &AppHandle) -> tauri::Result<WebviewWindow> {
    let win = WebviewWindowBuilder::new(app, MAIN, WebviewUrl::App("index.html".into()))
        .title("Alter Ego")
        .decorations(false)
        .transparent(true)
        .shadow(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(true)
        .focused(false)
        .visible(false)
        .min_inner_size(MIN_WIDTH_LOGICAL, MIN_WIDTH_LOGICAL / RATIO)
        .inner_size(640.0, 360.0)
        .build()?;

    let geo = initial_geometry(app);
    // Prima la posizione (sceglie il monitor e quindi il DPI), poi la dimensione.
    win.set_position(PhysicalPosition::new(geo.x, geo.y))?;
    win.set_size(PhysicalSize::new(geo.width, geo.height))?;
    win.show()?;
    Ok(win)
}

fn monitor_name(m: &Monitor) -> String {
    m.name().cloned().unwrap_or_else(|| "sconosciuto".into())
}

/// Geometria predefinita: 30% della larghezza del monitor, in basso a destra.
fn default_geometry(m: &Monitor) -> Geometry {
    let area = m.work_area();
    let scale = m.scale_factor();
    let width = ((area.size.width as f64) * 0.30).max(MIN_WIDTH_LOGICAL * scale);
    let height = width / RATIO;
    let margin = (24.0 * scale) as i32;
    Geometry {
        x: area.position.x + area.size.width as i32 - width as i32 - margin,
        y: area.position.y + area.size.height as i32 - height as i32 - margin,
        width: width as u32,
        height: height as u32,
    }
}

/// Almeno 100 px della finestra devono cadere dentro il monitor.
fn visible_on(g: &Geometry, m: &Monitor) -> bool {
    let (mp, ms) = (m.position(), m.size());
    let ix = (g.x + g.width as i32).min(mp.x + ms.width as i32) - g.x.max(mp.x);
    let iy = (g.y + g.height as i32).min(mp.y + ms.height as i32) - g.y.max(mp.y);
    ix >= 100 && iy >= 100
}

fn initial_geometry(app: &AppHandle) -> Geometry {
    let state = app.state::<AppState>();
    let cfg = state.cfg.lock().unwrap();
    let monitors = app.available_monitors().unwrap_or_default();
    let target = cfg
        .last_monitor
        .as_ref()
        .and_then(|n| monitors.iter().find(|m| m.name() == Some(n)).cloned())
        .or_else(|| app.primary_monitor().ok().flatten())
        .or_else(|| monitors.first().cloned());
    let Some(mon) = target else {
        return Geometry { x: 100, y: 100, width: 640, height: 360 };
    };
    match cfg.geometry.get(&monitor_name(&mon)) {
        Some(g) if visible_on(g, &mon) => *g,
        _ => default_geometry(&mon),
    }
}

/// Chiamata a ogni Moved/Resized: aggiorna la geometria del monitor corrente.
pub fn record_geometry(win: &tauri::Window) {
    if !win.is_visible().unwrap_or(false) {
        return;
    }
    let (Ok(pos), Ok(size), Ok(Some(mon))) =
        (win.outer_position(), win.inner_size(), win.current_monitor())
    else {
        return;
    };
    if pos.x <= -30000 || size.width == 0 {
        return; // finestra minimizzata
    }
    let state = win.state::<AppState>();
    let name = monitor_name(&mon);
    let mut cfg = state.cfg.lock().unwrap();
    cfg.geometry.insert(
        name.clone(),
        Geometry { x: pos.x, y: pos.y, width: size.width, height: size.height },
    );
    cfg.last_monitor = Some(name);
    state.dirty.store(true, Ordering::Relaxed);
}

/// Riporta la finestra alla geometria predefinita sul monitor corrente.
pub fn reset_geometry(app: &AppHandle) {
    let Some(win) = app.get_webview_window(MAIN) else { return };
    let Ok(Some(mon)) = win.current_monitor().or_else(|_| app.primary_monitor()) else {
        return;
    };
    let g = default_geometry(&mon);
    let _ = win.set_position(PhysicalPosition::new(g.x, g.y));
    let _ = win.set_size(PhysicalSize::new(g.width, g.height));
}

/// Ridimensiona mantenendo 16:9. `width` in pixel fisici.
pub fn resize_keep_ratio(win: &WebviewWindow, width: f64) -> tauri::Result<()> {
    let scale = win.scale_factor()?;
    let max_w = win
        .current_monitor()?
        .map(|m| m.work_area().size.width as f64)
        .unwrap_or(3840.0);
    let w = width.clamp(MIN_WIDTH_LOGICAL * scale, max_w);
    win.set_size(PhysicalSize::new(w.round() as u32, (w / RATIO).round() as u32))
}

pub fn toggle_visibility(app: &AppHandle) {
    if let Some(win) = app.get_webview_window(MAIN) {
        if win.is_visible().unwrap_or(false) {
            let _ = win.hide();
        } else {
            let _ = win.show(); // nessun set_focus: non rubiamo il focus
        }
    }
}

pub fn set_click_through(app: &AppHandle, on: bool) {
    let state = app.state::<AppState>();
    state.click_through.store(on, Ordering::Relaxed);
    state.handle_hover.store(false, Ordering::Relaxed);
    if let Some(win) = app.get_webview_window(MAIN) {
        let _ = win.set_ignore_cursor_events(on);
    }
    if let Some(item) = state.tray_click_item.lock().unwrap().as_ref() {
        let _ = item.set_checked(on);
    }
    let _ = app.emit("click-through", on);
}

/// Con il click-through attivo la finestra ignora il mouse; questo thread controlla
/// la posizione del cursore e riattiva gli eventi solo sopra la maniglia.
/// A click-through spento dorme e non fa nulla.
pub fn spawn_handle_poller(app: AppHandle) {
    thread::spawn(move || loop {
        let state = app.state::<AppState>();
        if !state.click_through.load(Ordering::Relaxed) {
            thread::sleep(Duration::from_millis(300));
            continue;
        }
        if let (Some(win), Ok(cur)) = (app.get_webview_window(MAIN), app.cursor_position()) {
            if let (Ok(pos), Ok(size), Ok(scale)) =
                (win.outer_position(), win.outer_size(), win.scale_factor())
            {
                let hs = HANDLE_LOGICAL * scale;
                let right = (pos.x + size.width as i32) as f64;
                let top = pos.y as f64;
                let inside = win.is_visible().unwrap_or(false)
                    && cur.x >= right - hs
                    && cur.x < right
                    && cur.y >= top
                    && cur.y < top + hs;
                if inside != state.handle_hover.swap(inside, Ordering::Relaxed) {
                    let _ = win.set_ignore_cursor_events(!inside);
                    let _ = app.emit("handle-hover", inside);
                }
            }
        }
        thread::sleep(Duration::from_millis(80));
    });
}

/// Salva la config su disco al massimo una volta al secondo, solo se cambiata.
pub fn spawn_saver(app: AppHandle) {
    thread::spawn(move || loop {
        thread::sleep(Duration::from_secs(1));
        crate::flush_config(&app);
    });
}
