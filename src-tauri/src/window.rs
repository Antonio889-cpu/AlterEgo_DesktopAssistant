//! Finestra principale: creazione, geometria per monitor, click-through.

use crate::{config::Geometry, AppState};
use std::{
    sync::{
        atomic::{AtomicBool, AtomicU8, Ordering},
        Mutex,
    },
    thread,
    time::Duration,
};
use tauri::{
    AppHandle, Emitter, Manager, Monitor, PhysicalPosition, PhysicalSize, WebviewUrl,
    WebviewWindow, WebviewWindowBuilder,
};

pub const MAIN: &str = "main";
const RATIO: f64 = 16.0 / 9.0;
/// Lato (px logici) della maniglia che resta cliccabile in click-through (angolo in alto a destra).
const HANDLE_LOGICAL: f64 = 34.0;
const MIN_WIDTH_LOGICAL: f64 = 320.0;

/// `visible` = false con l'avvio silenzioso (si apre dalla tray o con la hotkey).
pub fn create_main(app: &AppHandle, visible: bool, on_top: bool) -> tauri::Result<WebviewWindow> {
    let win = WebviewWindowBuilder::new(app, MAIN, WebviewUrl::App("index.html".into()))
        .title("Alter Ego")
        .decorations(false)
        .transparent(true)
        .shadow(false)
        .always_on_top(on_top)
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
    if visible {
        win.show()?;
    }
    Ok(win)
}

fn monitor_name(m: &Monitor) -> String {
    m.name().cloned().unwrap_or_else(|| "sconosciuto".into())
}

pub fn open_settings(app: &AppHandle) -> tauri::Result<()> {
    if let Some(w) = app.get_webview_window("settings") {
        w.show()?;
        w.unminimize()?;
        return w.set_focus();
    }
    WebviewWindowBuilder::new(app, "settings", WebviewUrl::App("settings.html".into()))
        .title("Alter Ego – Impostazioni")
        .inner_size(760.0, 720.0)
        .min_inner_size(560.0, 480.0)
        .center()
        .build()?;
    Ok(())
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
    let cfg = state.cfg.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
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
    save_geometry(win.app_handle(), win.is_visible().unwrap_or(false), win.outer_position().ok(), win.inner_size().ok(), win.current_monitor().ok().flatten());
}

fn save_geometry(app: &AppHandle, visible: bool, pos: Option<PhysicalPosition<i32>>, size: Option<PhysicalSize<u32>>, mon: Option<Monitor>) {
    // in modalità compatta (musica) la geometria salvata resta quella normale
    if !visible || COMPACT.load(Ordering::SeqCst) || ANIMATING.load(Ordering::SeqCst) {
        return;
    }
    let (Some(pos), Some(size), Some(mon)) = (pos, size, mon) else { return };
    if pos.x <= -30000 || size.width == 0 {
        return; // finestra minimizzata
    }
    let state = app.state::<AppState>();
    let name = monitor_name(&mon);
    let mut cfg = state.cfg.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    cfg.geometry.insert(name.clone(), Geometry { x: pos.x, y: pos.y, width: size.width, height: size.height });
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

// ---------- modalità compatta (durante la musica) ----------

/// La finestra è rimpicciolita.
static COMPACT: AtomicBool = AtomicBool::new(false);
/// Animazione di ridimensionamento in corso.
static ANIMATING: AtomicBool = AtomicBool::new(false);
/// Richiesta arrivata durante un'animazione: 0 nessuna, 1 compatta, 2 normale.
static PENDING_COMPACT: AtomicU8 = AtomicU8::new(0);
/// Dimensione normale (pixel fisici) da ripristinare.
static NORMAL_SIZE: Mutex<Option<(u32, u32)>> = Mutex::new(None);
/// Larghezza compatta: 45% di quella normale, minimo 220 px logici.
const COMPACT_RATIO: f64 = 0.45;
const COMPACT_MIN_LOGICAL: f64 = 220.0;

pub fn is_compact() -> bool {
    COMPACT.load(Ordering::SeqCst)
}

/// Angolo a cui restare "attaccati": quello del monitor più vicino al centro della finestra.
fn anchor(win: &WebviewWindow, pos: PhysicalPosition<i32>, size: PhysicalSize<u32>) -> (bool, bool) {
    let Ok(Some(m)) = win.current_monitor() else { return (true, true) };
    let a = m.work_area();
    let cx = pos.x + size.width as i32 / 2;
    let cy = pos.y + size.height as i32 / 2;
    (cx > a.position.x + a.size.width as i32 / 2, cy > a.position.y + a.size.height as i32 / 2)
}

/// Ridimensiona in ~180 ms tenendo fermo l'angolo `(right, bottom)`.
fn animate_to(win: WebviewWindow, from: (PhysicalPosition<i32>, PhysicalSize<u32>), to_size: PhysicalSize<u32>, (right, bottom): (bool, bool), done: impl FnOnce() + Send + 'static) {
    ANIMATING.store(true, Ordering::SeqCst);
    thread::spawn(move || {
        let (p0, s0) = from;
        let (ax, ay) = (if right { p0.x + s0.width as i32 } else { p0.x }, if bottom { p0.y + s0.height as i32 } else { p0.y });
        const STEPS: u32 = 9;
        for i in 1..=STEPS {
            let t = i as f64 / STEPS as f64;
            let e = 1.0 - (1.0 - t).powi(3); // ease-out
            let w = (s0.width as f64 + (to_size.width as f64 - s0.width as f64) * e).round() as u32;
            let h = (s0.height as f64 + (to_size.height as f64 - s0.height as f64) * e).round() as u32;
            let x = if right { ax - w as i32 } else { ax };
            let y = if bottom { ay - h as i32 } else { ay };
            let _ = win.set_size(PhysicalSize::new(w, h));
            let _ = win.set_position(PhysicalPosition::new(x, y));
            thread::sleep(Duration::from_millis(20));
        }
        ANIMATING.store(false, Ordering::SeqCst);
        done();
        // richiesta arrivata durante l'animazione (es. Alter Ego ha iniziato a parlare)
        match PENDING_COMPACT.swap(0, Ordering::SeqCst) {
            1 => set_compact(win.app_handle(), true),
            2 => set_compact(win.app_handle(), false),
            _ => {}
        }
    });
}

/// Rimpicciolisce (on) o riporta alla dimensione normale la finestra principale.
pub fn set_compact(app: &AppHandle, on: bool) {
    let Some(win) = app.get_webview_window(MAIN) else { return };
    if ANIMATING.load(Ordering::SeqCst) {
        PENDING_COMPACT.store(if on { 1 } else { 2 }, Ordering::SeqCst);
        return;
    }
    if COMPACT.load(Ordering::SeqCst) == on || !win.is_visible().unwrap_or(false) {
        return;
    }
    let (Ok(pos), Ok(size), Ok(scale)) = (win.outer_position(), win.inner_size(), win.scale_factor()) else { return };
    let corner = anchor(&win, pos, size);
    if on {
        *NORMAL_SIZE.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = Some((size.width, size.height));
        let w = (size.width as f64 * COMPACT_RATIO).max(COMPACT_MIN_LOGICAL * scale).min(size.width as f64);
        let _ = win.set_min_size(Some(tauri::LogicalSize::new(COMPACT_MIN_LOGICAL, COMPACT_MIN_LOGICAL / RATIO)));
        COMPACT.store(true, Ordering::SeqCst);
        let target = PhysicalSize::new(w.round() as u32, (w / RATIO).round() as u32);
        animate_to(win, (pos, size), target, corner, || {});
    } else {
        let Some((w, h)) = NORMAL_SIZE.lock().unwrap_or_else(std::sync::PoisonError::into_inner).take() else {
            COMPACT.store(false, Ordering::SeqCst);
            return;
        };
        let w2 = win.clone();
        animate_to(win, (pos, size), PhysicalSize::new(w, h), corner, move || {
            COMPACT.store(false, Ordering::SeqCst);
            let _ = w2.set_min_size(Some(tauri::LogicalSize::new(MIN_WIDTH_LOGICAL, MIN_WIDTH_LOGICAL / RATIO)));
            // salva la posizione finale (la finestra può essere stata spostata da compatta)
            save_geometry(w2.app_handle(), true, w2.outer_position().ok(), w2.inner_size().ok(), w2.current_monitor().ok().flatten());
            // tornata grande: il lettore e la Venue non devono coprirla
            crate::music::avoid_main(w2.app_handle());
        });
    }
}

pub fn toggle_visibility(app: &AppHandle) {
    if let Some(win) = app.get_webview_window(MAIN) {
        if win.is_visible().unwrap_or(false) && PENDING.load(Ordering::SeqCst) == 0 {
            hide_main(app, false);
        } else {
            show_main(app);
        }
    }
}

// ---------- animazione "vecchia TV" ----------
// Nascondere/uscire: il frontend fa spegnere lo schermo e poi chiama `tv_done`.
// Se non risponde (webview bloccata) si procede comunque dopo un attimo.

/// 0 = niente in corso, 1 = nascondi, 2 = esci.
static PENDING: AtomicU8 = AtomicU8::new(0);
const TV_FALLBACK: Duration = Duration::from_millis(1500);

/// Mostra la finestra (se era nascosta) e fa accendere lo schermo.
pub fn show_main(app: &AppHandle) {
    let Some(win) = app.get_webview_window(MAIN) else { return };
    // se Alter Ego era fuori a esplorare (pet), rientra nella TV
    crate::pet::stop(app);
    if PENDING.swap(0, Ordering::SeqCst) == 2 {
        PENDING.store(2, Ordering::SeqCst); // l'uscita non si annulla
        return;
    }
    // l'animazione "accensione" solo se la finestra era davvero nascosta
    // (non quando si apre il box di testo o una conferma con la finestra già visibile)
    if !win.is_visible().unwrap_or(true) {
        let _ = win.show(); // nessun set_focus: non rubiamo il focus
        let _ = app.emit_to(MAIN, "tv", "on");
    }
}

/// Nasconde la finestra (o esce dall'app) dopo l'animazione di spegnimento.
pub fn hide_main(app: &AppHandle, quit: bool) {
    let anim = app.state::<AppState>().cfg.lock().unwrap_or_else(std::sync::PoisonError::into_inner).tv_animation;
    let visible = app.get_webview_window(MAIN).is_some_and(|w| w.is_visible().unwrap_or(false));
    PENDING.store(if quit { 2 } else { 1 }, Ordering::SeqCst);
    if !anim || !visible {
        return tv_finish(app);
    }
    let _ = app.emit_to(MAIN, "tv", if quit { "quit" } else { "off" });
    let app = app.clone();
    thread::spawn(move || {
        thread::sleep(TV_FALLBACK);
        tv_finish(&app);
    });
}

fn tv_finish(app: &AppHandle) {
    match PENDING.swap(0, Ordering::SeqCst) {
        1 => {
            if let Some(w) = app.get_webview_window(MAIN) {
                let _ = w.hide();
            }
        }
        2 => {
            crate::flush_config(app);
            app.exit(0);
        }
        _ => {}
    }
}

/// Il frontend ha finito l'animazione di spegnimento.
#[tauri::command]
pub fn tv_done(app: AppHandle) {
    tv_finish(&app);
}

pub fn set_click_through(app: &AppHandle, on: bool) {
    let state = app.state::<AppState>();
    state.click_through.store(on, Ordering::Relaxed);
    state.handle_hover.store(false, Ordering::Relaxed);
    if let Some(win) = app.get_webview_window(MAIN) {
        let _ = win.set_ignore_cursor_events(on);
    }
    if let Some(item) = state.tray_click_item.lock().unwrap_or_else(std::sync::PoisonError::into_inner).as_ref() {
        let _ = item.set_checked(on);
    }
    // anche le impostazioni vedono lo stato aggiornato
    let _ = app.emit("config", crate::front_config(app));
    let _ = app.emit("click-through", on);
}

/// Con il click-through attivo la finestra ignora il mouse; questo thread controlla
/// la posizione del cursore e riattiva gli eventi solo sopra la maniglia.
/// A click-through spento dorme e non fa nulla.
pub fn spawn_handle_poller(app: AppHandle) {
    let _ = thread::Builder::new().name("maniglia".into()).spawn(move || loop {
        let state = app.state::<AppState>();
        if !state.click_through.load(Ordering::Relaxed) || state.modal.load(Ordering::Relaxed) {
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
    let _ = thread::Builder::new().name("salva-config".into()).spawn(move || loop {
        thread::sleep(Duration::from_secs(1));
        crate::flush_config(&app);
    });
}
