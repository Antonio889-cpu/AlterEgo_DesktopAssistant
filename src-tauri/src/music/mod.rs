//! Music Search and Play: Alter Ego cerca un brano su YouTube (yt-dlp), lo scarica in cache
//! e lo suona dal motore audio (stesso dispositivo di uscita, Mic-Link facoltativo).
//! Il lettore è una finestra a parte (player.html) con spettro, testo sincronizzato e
//! lo sprite di Alter Ego che cambia a tempo di musica.

mod analyze;
mod decode;
pub mod library;
mod lyrics;
mod ytdlp;

use crate::{audio::AudioEngine, update_config, AppState};
use serde::Serialize;
use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc, Mutex,
    },
    time::{Duration, Instant},
};
use tauri::{AppHandle, Emitter, Manager};

pub const PLAYER: &str = "player";
pub const VENUE: &str = "venue";
/// Dimensione di riferimento del lettore (px logici): il contenuto si scala con la finestra.
const PLAYER_W: f64 = 600.0;
const PLAYER_H: f64 = 248.0;
pub const PLAYER_MIN_W: f64 = 420.0;
pub const PLAYER_MAX_W: f64 = 1200.0;
const VENUE_W: f64 = 860.0;
const VENUE_H: f64 = 540.0;

/// Guadagno reale dalla posizione del cursore (0–1): curva quadratica, massimo −5 dB.
/// Così metà corsa è circa −17 dB (volume "normale") invece di quasi tutto il volume.
pub fn gain(slider: f64) -> f32 {
    let v = slider.clamp(0.0, 1.0);
    (0.56 * v * v) as f32
}

#[derive(Serialize, Clone, Debug, Default)]
pub struct Track {
    pub id: String,
    pub title: String,
    pub artist: String,
    pub duration: f64,
    pub thumbnail: Option<String>,
    pub url: Option<String>,
    #[serde(skip)]
    pub file: PathBuf,
    /// Testo sincronizzato: (secondi, riga).
    pub lyrics: Option<Vec<(f64, String)>>,
    /// Fonte del testo (LRCLIB, NetEase, YouTube…).
    pub lyrics_source: Option<String>,
    /// false = tempi stimati.
    pub lyrics_synced: bool,
}

#[derive(Default)]
struct Inner {
    queue: Vec<Track>,
    index: Option<usize>,
    /// Posizione (s) all'ultimo avvio/salto; si somma ai frame suonati da allora.
    base: f64,
    paused: bool,
    playing: bool,
    /// Messaggio di stato ("Cerco…", errori).
    status: String,
}

pub struct Music {
    inner: Mutex<Inner>,
    ctl: Mutex<Option<mpsc::Sender<decode::Ctl>>>,
    /// Ogni avvio di brano incrementa: i thread vecchi non toccano lo stato.
    gen: AtomicU64,
    data_dir: PathBuf,
    /// Un download alla volta.
    busy: tokio::sync::Mutex<()>,
    /// Brani recenti e preferiti (Music Venue).
    pub library: Mutex<library::Library>,
}

/// Stato inviato al lettore.
#[derive(Serialize, Clone)]
pub struct State {
    pub track: Option<Track>,
    pub position: f64,
    pub duration: f64,
    pub playing: bool,
    pub paused: bool,
    pub volume: f64,
    pub shuffle: bool,
    pub repeat: String,
    pub index: usize,
    pub queue_len: usize,
    pub status: String,
    pub reactive_sprite: bool,
    /// Il brano attuale è tra i preferiti.
    pub favorite: bool,
    pub venue_open: bool,
    /// La finestra del lettore mostra la Venue.
    pub expanded: bool,
    /// La Venue si apre dentro il lettore (altrimenti in una finestra a parte).
    pub venue_docked: bool,
}

impl Music {
    pub fn new(data_dir: &std::path::Path) -> Self {
        Self {
            inner: Mutex::new(Inner::default()),
            ctl: Mutex::new(None),
            gen: AtomicU64::new(0),
            data_dir: data_dir.to_path_buf(),
            busy: tokio::sync::Mutex::new(()),
            library: Mutex::new(library::load(data_dir)),
        }
    }
}

fn rate_for_position(app: &AppHandle) -> u32 {
    let (sp, _) = app.state::<AudioEngine>().music_rates();
    if sp == 0 { decode::FALLBACK_RATE } else { sp }
}

pub fn state(app: &AppHandle) -> State {
    let m = app.state::<Music>();
    let cfg = app.state::<AppState>().cfg.lock().unwrap().music.clone();
    let i = m.inner.lock().unwrap();
    let track = i.index.and_then(|x| i.queue.get(x)).cloned();
    let played = app.state::<AudioEngine>().music_played(false) as f64 / rate_for_position(app) as f64;
    let duration = track.as_ref().map(|t| t.duration).unwrap_or(0.0);
    let favorite = track.as_ref().is_some_and(|t| m.library.lock().unwrap().is_favorite(&t.id));
    let mut position = if i.playing { i.base + played } else { i.base };
    if duration > 0.0 {
        position = position.min(duration);
    }
    State {
        track,
        position,
        duration,
        playing: i.playing,
        paused: i.paused,
        volume: cfg.volume,
        shuffle: cfg.shuffle,
        repeat: cfg.repeat,
        index: i.index.unwrap_or(0),
        queue_len: i.queue.len(),
        status: i.status.clone(),
        reactive_sprite: cfg.reactive_sprite,
        favorite,
        venue_open: is_expanded() || app.get_webview_window(VENUE).is_some(),
        expanded: is_expanded(),
        venue_docked: cfg.venue_docked,
    }
}

pub fn emit_state(app: &AppHandle) {
    // a tutte le finestre: la principale lo usa per rimpicciolirsi durante la musica
    let _ = app.emit("music-state", state(app));
}

fn set_status(app: &AppHandle, s: &str) {
    app.state::<Music>().inner.lock().unwrap().status = s.to_string();
    emit_state(app);
}

// ---------- finestra del lettore ----------

// Il lettore può "espandersi" nella Music Venue (stessa finestra, pagina venue.html?docked=1)
// e tornare lettore; dalla Venue agganciata un tasto la separa in una finestra a parte.
/// La finestra del lettore mostra la Music Venue.
static EXPANDED: AtomicBool = AtomicBool::new(false);
/// Chiusura animata in corso (lettore / Venue separata): una riapertura la annulla.
static PLAYER_CLOSING: AtomicBool = AtomicBool::new(false);
static PLAYER_CLOSE_GEN: AtomicU64 = AtomicU64::new(0);
static VENUE_CLOSING: AtomicBool = AtomicBool::new(false);
static VENUE_CLOSE_GEN: AtomicU64 = AtomicU64::new(0);

pub fn is_expanded() -> bool {
    EXPANDED.load(Ordering::SeqCst)
}

fn player_width(app: &AppHandle) -> f64 {
    app.state::<AppState>().cfg.lock().unwrap().music.player_width.clamp(PLAYER_MIN_W, PLAYER_MAX_W)
}

fn venue_docked(app: &AppHandle) -> bool {
    app.state::<AppState>().cfg.lock().unwrap().music.venue_docked
}

/// Dimensione logica della finestra del lettore nella modalità data.
fn mode_size(app: &AppHandle, expanded: bool) -> (f64, f64) {
    if expanded {
        (VENUE_W, VENUE_H)
    } else {
        let w = player_width(app);
        (w, w * PLAYER_H / PLAYER_W)
    }
}

fn open_player(app: &AppHandle) {
    open_player_in(app, false);
}

/// Mostra il lettore; se non esiste lo crea (come lettore o già espanso nella Venue).
fn open_player_in(app: &AppHandle, expanded: bool) {
    if let Some(w) = app.get_webview_window(PLAYER) {
        // stava chiudendosi: si annulla e si riaccende
        if PLAYER_CLOSING.swap(false, Ordering::SeqCst) {
            let _ = app.emit_to(PLAYER, "player-tv", "on");
        }
        if !w.is_visible().unwrap_or(true) {
            let _ = w.show();
        }
        return;
    }
    EXPANDED.store(expanded, Ordering::SeqCst);
    let pos = {
        let st = app.state::<AppState>();
        let c = st.cfg.lock().unwrap();
        if expanded { c.music.venue_pos } else { c.music.player_pos }
    };
    let (lw, lh) = mode_size(app, expanded);
    let url = if expanded { "venue.html?docked=1" } else { "player.html" };
    let b = tauri::WebviewWindowBuilder::new(app, PLAYER, tauri::WebviewUrl::App(url.into()))
        .title("Alter Ego – Musica")
        .decorations(false)
        .transparent(true)
        .shadow(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(expanded)
        .focused(expanded)
        .visible(false)
        .inner_size(lw, lh);
    let b = if expanded { b.min_inner_size(620.0, 400.0) } else { b };
    let Ok(w) = b.build() else {
        EXPANDED.store(false, Ordering::SeqCst);
        return;
    };
    let scale = w.primary_monitor().ok().flatten().map(|m| m.scale_factor()).unwrap_or(1.0);
    let size = ((lw * scale) as i32, (lh * scale) as i32);
    // posizione salvata se non copre Alter Ego, altrimenti accanto alla finestra principale
    let saved = pos.filter(|p| !overlaps_main(app, Rect { x: p.0, y: p.1, w: size.0, h: size.1 }));
    match saved {
        Some((x, y)) => {
            let _ = w.set_position(tauri::PhysicalPosition::new(x, y));
        }
        None => place_clear(app, &w, size, &[]),
    }
    let _ = w.show();
    emit_state(app);
}

/// Cambia la dimensione della finestra del lettore per la modalità (la pagina poi cambia da sola).
fn apply_mode(app: &AppHandle, expanded: bool) {
    let Some(w) = app.get_webview_window(PLAYER) else { return };
    EXPANDED.store(expanded, Ordering::SeqCst);
    let (lw, lh) = mode_size(app, expanded);
    let scale = w.scale_factor().unwrap_or(1.0);
    let _ = w.set_resizable(expanded);
    let _ = w.set_min_size(if expanded { Some(tauri::LogicalSize::new(620.0, 400.0)) } else { None });
    let _ = w.set_size(tauri::LogicalSize::new(lw, lh));
    let size = ((lw * scale) as i32, (lh * scale) as i32);
    // resta dov'è se c'è posto, altrimenti accanto ad Alter Ego
    let fits = w.outer_position().ok().zip(w.current_monitor().ok().flatten()).is_some_and(|(p, m)| {
        let a = m.work_area();
        let area = Rect { x: a.position.x, y: a.position.y, w: a.size.width as i32, h: a.size.height as i32 };
        let r = Rect { x: p.x, y: p.y, w: size.0, h: size.1 };
        r.inside(&area) && !overlaps_main(app, r)
    });
    if !fits {
        place_clear(app, &w, size, &[]);
    }
    if expanded {
        let _ = w.set_focus();
    }
    emit_state(app);
}

/// Chiede alla finestra del lettore di passare a Venue (true) o lettore (false), con l'animazione.
fn request_mode(app: &AppHandle, expanded: bool) {
    if app.get_webview_window(PLAYER).is_some() {
        open_player_in(app, expanded); // annulla un'eventuale chiusura in corso
        if is_expanded() != expanded {
            let _ = app.emit_to(PLAYER, "player-mode", if expanded { "venue" } else { "player" });
        } else if expanded {
            if let Some(w) = app.get_webview_window(PLAYER) {
                let _ = w.set_focus();
            }
        }
    } else if expanded {
        open_player_in(app, true);
    }
}

// ---------- posizionamento: lettore e Venue non coprono la finestra principale ----------

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

impl Rect {
    fn of(w: &tauri::WebviewWindow) -> Option<Rect> {
        let p = w.outer_position().ok()?;
        let s = w.outer_size().ok()?;
        Some(Rect { x: p.x, y: p.y, w: s.width as i32, h: s.height as i32 })
    }
    fn overlaps(&self, o: &Rect) -> bool {
        self.x < o.x + o.w && o.x < self.x + self.w && self.y < o.y + o.h && o.y < self.y + self.h
    }
    fn union(&self, o: &Rect) -> Rect {
        let (x, y) = (self.x.min(o.x), self.y.min(o.y));
        Rect { x, y, w: (self.x + self.w).max(o.x + o.w) - x, h: (self.y + self.h).max(o.y + o.h) - y }
    }
    fn inside(&self, a: &Rect) -> bool {
        self.x >= a.x && self.y >= a.y && self.x + self.w <= a.x + a.w && self.y + self.h <= a.y + a.h
    }
}

fn main_rect(app: &AppHandle) -> Option<Rect> {
    let m = app.get_webview_window(crate::window::MAIN)?;
    if !m.is_visible().unwrap_or(false) {
        return None;
    }
    Rect::of(&m)
}

fn overlaps_main(app: &AppHandle, r: Rect) -> bool {
    main_rect(app).is_some_and(|m| m.overlaps(&r))
}

/// Prima posizione libera attorno all'ostacolo (sopra, a sinistra, sotto, a destra) dentro l'area di lavoro.
pub fn free_spot(obstacle: Rect, size: (i32, i32), area: Rect, gap: i32) -> Option<(i32, i32)> {
    let (w, h) = size;
    let cx = |x: i32| x.clamp(area.x, area.x + area.w - w);
    let cy = |y: i32| y.clamp(area.y, area.y + area.h - h);
    let cands = [
        (cx(obstacle.x + obstacle.w - w), obstacle.y - gap - h),
        (obstacle.x - gap - w, cy(obstacle.y + obstacle.h - h)),
        (cx(obstacle.x + obstacle.w - w), obstacle.y + obstacle.h + gap),
        (obstacle.x + obstacle.w + gap, cy(obstacle.y + obstacle.h - h)),
    ];
    cands.into_iter().find(|&(x, y)| {
        let r = Rect { x, y, w, h };
        r.inside(&area) && !r.overlaps(&obstacle)
    })
}

/// Mette `win` (dimensione `size`, pixel fisici) accanto ad Alter Ego e agli altri `extra`.
fn place_clear(app: &AppHandle, win: &tauri::WebviewWindow, size: (i32, i32), extra: &[Rect]) {
    let mon = app
        .get_webview_window(crate::window::MAIN)
        .and_then(|m| m.current_monitor().ok().flatten())
        .or_else(|| win.primary_monitor().ok().flatten());
    let Some(mon) = mon else { return };
    let a = mon.work_area();
    let area = Rect { x: a.position.x, y: a.position.y, w: a.size.width as i32, h: a.size.height as i32 };
    let gap = (12.0 * mon.scale_factor()) as i32;
    let mut obstacle = main_rect(app);
    for e in extra {
        obstacle = Some(obstacle.map(|o| o.union(e)).unwrap_or(*e));
    }
    let spot = match obstacle {
        Some(o) => free_spot(o, size, area, gap).or_else(|| main_rect(app).and_then(|m| free_spot(m, size, area, gap))),
        None => None,
    };
    let (x, y) = spot.unwrap_or((area.x + area.w - size.0 - 2 * gap, area.y + 2 * gap));
    let _ = win.set_position(tauri::PhysicalPosition::new(x, y));
}

/// Dopo un cambio di dimensione di Alter Ego (modalità compatta): sposta lettore e Venue se ora la coprono.
pub fn avoid_main(app: &AppHandle) {
    let Some(m) = main_rect(app) else { return };
    let mut placed = Vec::new();
    for label in [PLAYER, VENUE] {
        let Some(w) = app.get_webview_window(label) else { continue };
        let Some(r) = Rect::of(&w) else { continue };
        if r.overlaps(&m) || placed.iter().any(|p: &Rect| p.overlaps(&r)) {
            place_clear(app, &w, (r.w, r.h), &placed);
        }
        if let Some(r) = Rect::of(&w) {
            placed.push(r);
        }
    }
}

/// Ridimensiona il lettore (larghezza in px logici, proporzioni fisse).
pub fn resize_player(app: &AppHandle, width: f64) {
    let width = width.clamp(PLAYER_MIN_W, PLAYER_MAX_W);
    if let Some(w) = app.get_webview_window(PLAYER).filter(|_| !is_expanded()) {
        let _ = w.set_size(tauri::LogicalSize::new(width, width * PLAYER_H / PLAYER_W));
    }
    app.state::<AppState>().cfg.lock().unwrap().music.player_width = width;
    app.state::<AppState>().dirty.store(true, Ordering::Relaxed);
}

// ---------- Music Venue ----------

/// Apre la Music Venue: dentro il lettore (agganciata, predefinito) o in una finestra a parte.
pub fn open_venue(app: &AppHandle) {
    if venue_docked(app) {
        request_mode(app, true);
    } else {
        open_venue_window(app);
    }
}

fn open_venue_window(app: &AppHandle) {
    if let Some(w) = app.get_webview_window(VENUE) {
        if VENUE_CLOSING.swap(false, Ordering::SeqCst) {
            let _ = app.emit_to(VENUE, "venue-tv", "on");
        }
        let _ = w.show();
        let _ = w.set_focus();
        return;
    }
    let pos = app.state::<AppState>().cfg.lock().unwrap().music.venue_pos;
    let b = tauri::WebviewWindowBuilder::new(app, VENUE, tauri::WebviewUrl::App("venue.html".into()))
        .title("Music Venue")
        .decorations(false)
        .transparent(true)
        .shadow(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(true)
        .min_inner_size(620.0, 400.0)
        .visible(false)
        .inner_size(VENUE_W, VENUE_H);
    let Ok(w) = b.build() else { return };
    let scale = w.primary_monitor().ok().flatten().map(|m| m.scale_factor()).unwrap_or(1.0);
    let size = ((VENUE_W * scale) as i32, (VENUE_H * scale) as i32);
    let player = app.get_webview_window(PLAYER).and_then(|p| Rect::of(&p));
    let free = |r: Rect| !overlaps_main(app, r) && !player.is_some_and(|p| p.overlaps(&r));
    match pos.filter(|p| free(Rect { x: p.0, y: p.1, w: size.0, h: size.1 })) {
        Some((x, y)) => {
            let _ = w.set_position(tauri::PhysicalPosition::new(x, y));
        }
        None => place_clear(app, &w, size, &player.into_iter().collect::<Vec<_>>()),
    }
    let _ = w.show();
    let _ = w.set_focus();
    emit_state(app);
}

/// Chiude la Venue (separata con l'animazione glitch; agganciata: torna lettore o si chiude).
pub fn close_venue(app: &AppHandle) {
    close_venue_window(app);
    if is_expanded() {
        if state(app).playing {
            request_mode(app, false);
        } else {
            close_player(app);
        }
    }
}

fn close_venue_window(app: &AppHandle) {
    if let Some(w) = app.get_webview_window(VENUE) {
        VENUE_CLOSING.store(true, Ordering::SeqCst);
        let gen = VENUE_CLOSE_GEN.fetch_add(1, Ordering::SeqCst) + 1;
        let _ = app.emit_to(VENUE, "venue-tv", "off");
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(1200));
            // la pagina non ha risposto e nessuno l'ha riaperta nel frattempo
            if VENUE_CLOSING.load(Ordering::SeqCst) && VENUE_CLOSE_GEN.load(Ordering::SeqCst) == gen {
                VENUE_CLOSING.store(false, Ordering::SeqCst);
                let _ = w.close();
            }
        });
    }
}

/// Una finestra del lettore o della Venue è stata distrutta.
pub fn window_destroyed(app: &AppHandle, label: &str) {
    if label == PLAYER {
        EXPANDED.store(false, Ordering::SeqCst);
        PLAYER_CLOSING.store(false, Ordering::SeqCst);
    } else if label == VENUE {
        VENUE_CLOSING.store(false, Ordering::SeqCst);
    }
    emit_state(app);
}

pub fn record_venue_position(win: &tauri::Window) {
    if let Ok(p) = win.outer_position() {
        let app = win.app_handle();
        app.state::<AppState>().cfg.lock().unwrap().music.venue_pos = Some((p.x, p.y));
        app.state::<AppState>().dirty.store(true, Ordering::Relaxed);
    }
}

fn entry_of(t: &Track) -> library::Entry {
    library::Entry {
        id: t.id.clone(),
        title: t.title.clone(),
        artist: t.artist.clone(),
        duration: t.duration,
        thumbnail: t.thumbnail.clone(),
        url: t.url.clone(),
        ..Default::default()
    }
}

fn save_library(app: &AppHandle) {
    let m = app.state::<Music>();
    let lib = m.library.lock().unwrap().clone();
    library::save(&m.data_dir, &lib);
    let _ = app.emit("venue-library", lib);
}

/// Aggiunge/toglie il brano attuale dai preferiti. Restituisce il nuovo stato.
pub fn favorite_current(app: &AppHandle, want: Option<bool>) -> Result<bool, String> {
    let Some(t) = state(app).track else {
        return Err(crate::i18n::tr("non sta suonando niente", "nothing is playing").into());
    };
    let m = app.state::<Music>();
    let now = {
        let mut lib = m.library.lock().unwrap();
        let is = lib.is_favorite(&t.id);
        if want.is_none_or(|w| w != is) { lib.toggle_favorite(entry_of(&t)) } else { is }
    };
    save_library(app);
    emit_state(app);
    Ok(now)
}

/// Mostra il lettore (se c'è musica).
pub fn show_player(app: &AppHandle) {
    open_player(app);
}

/// Ferma la musica e chiude il lettore (anche se mostra la Venue agganciata).
pub fn stop_music(app: &AppHandle) {
    stop(app);
    if is_expanded() {
        close_player(app);
    }
}

/// Chiude il lettore con l'animazione glitch (la finestra la esegue e poi chiama `player_closed`).
fn close_player(app: &AppHandle) {
    if let Some(w) = app.get_webview_window(PLAYER) {
        PLAYER_CLOSING.store(true, Ordering::SeqCst);
        let gen = PLAYER_CLOSE_GEN.fetch_add(1, Ordering::SeqCst) + 1;
        // nome proprio del lettore: "tv" è della finestra principale
        let _ = app.emit_to(PLAYER, "player-tv", "off");
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(1200));
            if PLAYER_CLOSING.load(Ordering::SeqCst) && PLAYER_CLOSE_GEN.load(Ordering::SeqCst) == gen {
                PLAYER_CLOSING.store(false, Ordering::SeqCst);
                let _ = w.close(); // se la pagina non ha risposto
            }
        });
    }
}

/// Fine dell'animazione di chiusura del lettore (solo se nel frattempo non è stato riaperto).
#[tauri::command]
pub async fn player_closed(app: AppHandle) {
    if PLAYER_CLOSING.swap(false, Ordering::SeqCst) {
        if let Some(w) = app.get_webview_window(PLAYER) {
            let _ = w.close();
        }
    }
}

/// La finestra del lettore è stata spostata: ricorda dove (come lettore o come Venue).
pub fn record_player_position(win: &tauri::Window) {
    if let Ok(p) = win.outer_position() {
        let app = win.app_handle();
        {
            let st = app.state::<AppState>();
            let mut c = st.cfg.lock().unwrap();
            if is_expanded() { c.music.venue_pos = Some((p.x, p.y)) } else { c.music.player_pos = Some((p.x, p.y)) }
        }
        app.state::<AppState>().dirty.store(true, Ordering::Relaxed);
    }
}

// ---------- riproduzione ----------

/// Avvia il brano `index` della coda da `start` secondi.
fn start_track(app: &AppHandle, index: usize, start: f64) {
    let m = app.state::<Music>();
    let gen = m.gen.fetch_add(1, Ordering::SeqCst) + 1;
    if let Some(tx) = m.ctl.lock().unwrap().take() {
        let _ = tx.send(decode::Ctl::Stop);
    }
    let file = {
        let mut i = m.inner.lock().unwrap();
        let Some(t) = i.queue.get(index) else { return };
        let f = t.file.clone();
        m.library.lock().unwrap().played(entry_of(t));
        i.index = Some(index);
        i.base = start;
        i.playing = true;
        i.paused = false;
        i.status.clear();
        f
    };
    save_library(app);
    let engine = app.state::<AudioEngine>();
    engine.set_music_paused(false);
    let vol = app.state::<AppState>().cfg.lock().unwrap().music.volume;
    engine.set_music_gain(gain(vol));
    let (tx, rx) = mpsc::channel();
    *m.ctl.lock().unwrap() = Some(tx);
    open_player(app);
    emit_state(app);
    let app = app.clone();
    std::thread::Builder::new()
        .name("musica".into())
        .spawn(move || {
            let engine = app.state::<AudioEngine>();
            let app2 = app.clone();
            let on_seek = move |t: f64| {
                let m = app2.state::<Music>();
                if m.gen.load(Ordering::SeqCst) == gen {
                    m.inner.lock().unwrap().base = t;
                    app2.state::<AudioEngine>().music_played(true);
                }
            };
            let r = decode::play(&engine, &file, start, &rx, &on_seek);
            if app.state::<Music>().gen.load(Ordering::SeqCst) != gen {
                return; // sostituito da un altro brano
            }
            match r {
                Ok(decode::End::Finished) => advance(&app, true),
                Ok(decode::End::Stopped) => {}
                Err(e) => {
                    eprintln!("musica: {e}");
                    set_status(&app, &crate::tr!("Errore: {e}", "Error: {e}"));
                    advance(&app, true);
                }
            }
        })
        .expect("thread musica");
}

/// Prossimo brano secondo shuffle/repeat. `auto`: fine naturale del brano.
fn advance(app: &AppHandle, auto: bool) {
    let cfg = app.state::<AppState>().cfg.lock().unwrap().music.clone();
    let (len, cur) = {
        let m = app.state::<Music>();
        let i = m.inner.lock().unwrap();
        (i.queue.len(), i.index.unwrap_or(0))
    };
    if len == 0 {
        return stop(app);
    }
    let next = if auto && cfg.repeat == "one" {
        Some(cur)
    } else if cfg.shuffle && len > 1 {
        let r = (std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().subsec_nanos() as usize) % (len - 1);
        Some(if r >= cur { r + 1 } else { r })
    } else if cur + 1 < len {
        Some(cur + 1)
    } else if cfg.repeat == "all" {
        Some(0)
    } else {
        None
    };
    match next {
        Some(n) => start_track(app, n, 0.0),
        None if auto => stop(app),
        None => {
            // "avanti" sull'ultimo brano: riparte da capo
            start_track(app, cur, 0.0)
        }
    }
}

fn stop(app: &AppHandle) {
    let m = app.state::<Music>();
    m.gen.fetch_add(1, Ordering::SeqCst);
    if let Some(tx) = m.ctl.lock().unwrap().take() {
        let _ = tx.send(decode::Ctl::Stop);
    }
    {
        let mut i = m.inner.lock().unwrap();
        i.playing = false;
        i.paused = false;
        i.base = 0.0;
    }
    app.state::<AudioEngine>().music_clear();
    emit_state(app);
    // la Venue agganciata resta aperta anche senza musica
    if !is_expanded() {
        close_player(app);
    }
}

/// Cerca, scarica e suona (o accoda). Restituisce un messaggio per il modello.
pub async fn play(app: &AppHandle, query: &str, enqueue: bool) -> Result<String, String> {
    let query = query.trim();
    if query.is_empty() {
        return Err(crate::i18n::tr("cosa devo cercare?", "what should I look for?").into());
    }
    let m = app.state::<Music>();
    let _guard = m.busy.lock().await;
    open_player(app);
    set_status(app, &crate::tr!("Cerco «{query}»…", "Searching “{query}”…"));
    let http = app.state::<AppState>().ai.http.clone();
    let app2 = app.clone();
    let result = async {
        let tools = ytdlp::ensure(&http, &m.data_dir, |s| set_status(&app2, s)).await?;
        let cache = m.data_dir.join("music_cache");
        set_status(app, &crate::tr!("Scarico «{query}» da YouTube…", "Downloading “{query}” from YouTube…"));
        let r = match ytdlp::fetch(&tools, query, &cache).await {
            Ok(r) => r,
            Err(e) if cfg!(windows) && std::env::var_os("ALTEREGO_YTDLP").is_none() => {
                // YouTube cambia spesso: si aggiorna yt-dlp e si riprova una volta
                set_status(app, crate::i18n::tr("Aggiorno yt-dlp e riprovo…", "Updating yt-dlp and retrying…"));
                ytdlp::update(&tools).await.map_err(|u| format!("{e} (aggiornamento non riuscito: {u})"))?;
                ytdlp::fetch(&tools, query, &cache).await?
            }
            Err(e) => return Err(e),
        };
        ytdlp::prune(&cache, 30);
        Ok::<_, String>(r)
    }
    .await;
    let (info, file) = match result {
        Ok(r) => r,
        Err(e) => {
            set_status(app, &crate::tr!("Non trovato: {e}", "Not found: {e}"));
            if !m.inner.lock().unwrap().playing && !is_expanded() {
                close_player(app);
            }
            return Err(e);
        }
    };
    let (title, artist) = ytdlp::clean_meta(&info);
    let duration = info.duration.filter(|d| *d > 0.0).or_else(|| decode::duration(&file)).unwrap_or(0.0);
    let track = Track {
        id: info.id.clone(),
        title: title.clone(),
        artist: artist.clone(),
        duration,
        thumbnail: info.thumbnail.clone(),
        url: info.webpage_url.clone(),
        file,
        lyrics: None,
        lyrics_source: None,
        lyrics_synced: false,
    };
    let (index, now) = {
        let mut i = m.inner.lock().unwrap();
        let busy = i.playing;
        if enqueue && busy {
            i.queue.push(track);
            i.status.clear();
            (i.queue.len() - 1, false)
        } else {
            // suona subito: va subito dopo il brano attuale
            let at = i.index.map(|x| x + 1).unwrap_or(i.queue.len()).min(i.queue.len());
            i.queue.insert(at, track);
            (at, true)
        }
    };
    if now {
        start_track(app, index, 0.0);
    } else {
        emit_state(app);
    }
    // testo sincronizzato in background
    if app.state::<AppState>().cfg.lock().unwrap().music.lyrics {
        let app = app.clone();
        let id = info.id.clone();
        let raw = info.title.clone();
        let url = info.webpage_url.clone().unwrap_or_else(|| format!("https://www.youtube.com/watch?v={}", info.id));
        let (title, artist) = (title.clone(), artist.clone());
        tauri::async_runtime::spawn(async move {
            let http = app.state::<AppState>().ai.http.clone();
            let mut found = lyrics::find(&http, &title, &raw, &artist, duration, None).await;
            // niente testo sincronizzato: prova i sottotitoli del video
            if !found.as_ref().is_some_and(|l| l.synced) {
                let data_dir = app.state::<Music>().data_dir.clone();
                if let Ok(tools) = ytdlp::ensure(&http, &data_dir, |_| {}).await {
                    if let Some(vtt) = ytdlp::subtitles(&tools, &url, &data_dir.join("music_cache").join("subs")).await {
                        if let Some(l) = lyrics::find(&http, "", "", "", 0.0, Some(vtt)).await.filter(|l| l.synced) {
                            found = Some(l);
                        }
                    }
                }
            }
            let m = app.state::<Music>();
            let mut i = m.inner.lock().unwrap();
            for t in i.queue.iter_mut().filter(|t| t.id == id) {
                match &found {
                    Some(l) => {
                        t.lyrics = Some(l.lines.clone());
                        t.lyrics_source = Some(l.source.clone());
                        t.lyrics_synced = l.synced;
                    }
                    None => t.lyrics_source = Some(String::new()), // cercato, non trovato
                }
            }
            drop(i);
            emit_state(&app);
        });
    }
    let s = state(app);
    let t = s.track.clone().unwrap_or_default();
    Ok(if now {
        crate::tr!("In riproduzione: «{}» di {} ({}).", "Now playing: “{}” by {} ({}).", t.title, t.artist, fmt_time(t.duration))
    } else {
        crate::tr!("Aggiunto in coda (posizione {}): «{title}» di {artist}.", "Added to the queue (position {}): “{title}” by {artist}.", index + 1)
    })
}

fn fmt_time(s: f64) -> String {
    let s = s.max(0.0) as u64;
    format!("{}:{:02}", s / 60, s % 60)
}

pub fn toggle_pause(app: &AppHandle) {
    let m = app.state::<Music>();
    let paused = {
        let mut i = m.inner.lock().unwrap();
        if !i.playing {
            return;
        }
        i.paused = !i.paused;
        i.paused
    };
    app.state::<AudioEngine>().set_music_paused(paused);
    emit_state(app);
}

/// Comandi dal modello (strumento music_control).
pub async fn control(app: &AppHandle, action: &str, value: Option<f64>) -> Result<String, String> {
    let s = state(app);
    let need_track = || if s.playing { Ok(()) } else { Err(crate::i18n::tr("non sta suonando niente", "nothing is playing").to_string()) };
    match action {
        "pause" => {
            need_track()?;
            if !s.paused {
                toggle_pause(app);
            }
            Ok(crate::i18n::tr("In pausa.", "Paused.").into())
        }
        "resume" => {
            need_track()?;
            if s.paused {
                toggle_pause(app);
            }
            Ok(crate::i18n::tr("Riprendo.", "Resuming.").into())
        }
        "next" => {
            need_track()?;
            advance(app, false);
            Ok(crate::tr!("Ora suona: {}", "Now playing: {}", state(app).track.map(|t| t.title).unwrap_or_default()))
        }
        "previous" => {
            need_track()?;
            previous(app);
            Ok(crate::tr!("Ora suona: {}", "Now playing: {}", state(app).track.map(|t| t.title).unwrap_or_default()))
        }
        "stop" => {
            stop(app);
            Ok(crate::i18n::tr("Musica fermata.", "Music stopped.").into())
        }
        "favorite" | "unfavorite" => {
            let on = favorite_current(app, Some(action == "favorite"))?;
            Ok(if on { crate::i18n::tr("Aggiunto ai preferiti.", "Added to favorites.") } else { crate::i18n::tr("Tolto dai preferiti.", "Removed from favorites.") }.into())
        }
        "volume" => {
            let v = (value.unwrap_or(60.0) / 100.0).clamp(0.0, 1.0);
            set_volume(app, v);
            Ok(crate::tr!("Volume musica al {}%.", "Music volume at {}%.", (v * 100.0).round()))
        }
        _ => Ok(match s.track {
            Some(t) if s.playing => format!(
                "{} «{}» di {} — {} / {} (brano {} di {}).",
                if s.paused { "In pausa:" } else { "Sta suonando" },
                t.title,
                t.artist,
                fmt_time(s.position),
                fmt_time(s.duration),
                s.index + 1,
                s.queue_len
            ),
            _ => crate::i18n::tr("Non sta suonando niente.", "Nothing is playing.").into(),
        }),
    }
}

fn previous(app: &AppHandle) {
    let s = state(app);
    // come i lettori veri: dopo 3 secondi "indietro" riparte dall'inizio
    let idx = if s.position > 3.0 || s.index == 0 { s.index } else { s.index - 1 };
    start_track(app, idx, 0.0);
}

fn set_volume(app: &AppHandle, v: f64) {
    app.state::<AudioEngine>().set_music_gain(gain(v));
    update_config(app, |c| c.music.volume = v);
    emit_state(app);
}

// ---------- analisi (spettro + sprite) e posizione ----------

#[derive(Serialize, Clone)]
struct Spectrum {
    bands: Vec<u8>,
    beat: bool,
    sprite: Option<&'static str>,
}

/// Thread che, mentre suona, manda lo spettro (~25 fps) e lo stato (2 volte al secondo) al lettore.
pub fn spawn_analyzer(app: AppHandle) {
    std::thread::Builder::new()
        .name("spettro".into())
        .spawn(move || {
            let mut a = analyze::Analyzer::new();
            let mut last_state = Instant::now();
            let mut last = Instant::now();
            loop {
                std::thread::sleep(Duration::from_millis(40));
                let (playing, paused) = {
                    let m = app.state::<Music>();
                    let i = m.inner.lock().unwrap();
                    (i.playing, i.paused)
                };
                if !playing || app.get_webview_window(PLAYER).is_none() {
                    std::thread::sleep(Duration::from_millis(200));
                    continue;
                }
                let dt = last.elapsed().as_secs_f32();
                last = Instant::now();
                let rate = rate_for_position(&app);
                let samples = if paused { vec![0.0; analyze::FFT] } else { app.state::<AudioEngine>().music_analysis(analyze::FFT) };
                if samples.len() >= 256 {
                    let f = a.run(&samples, rate, dt);
                    let _ = app.emit_to(PLAYER, "music-spectrum", Spectrum { bands: f.bands, beat: f.beat, sprite: f.sprite });
                }
                if last_state.elapsed() > Duration::from_millis(500) {
                    last_state = Instant::now();
                    emit_state(&app);
                }
            }
        })
        .expect("thread spettro");
}

// ---------- comandi del lettore ----------

#[tauri::command]
pub fn music_state(app: AppHandle) -> State {
    state(&app)
}

#[tauri::command]
pub async fn music_toggle(app: AppHandle) {
    toggle_pause(&app);
}

#[tauri::command]
pub async fn music_next(app: AppHandle) {
    advance(&app, false);
}

#[tauri::command]
pub async fn music_prev(app: AppHandle) {
    previous(&app);
}

#[tauri::command]
pub fn music_seek(app: AppHandle, secs: f64) {
    if let Some(tx) = app.state::<Music>().ctl.lock().unwrap().as_ref() {
        let _ = tx.send(decode::Ctl::Seek(secs.max(0.0)));
    }
}

#[tauri::command]
pub fn music_volume(app: AppHandle, volume: f64) {
    set_volume(&app, volume.clamp(0.0, 1.0));
}

#[tauri::command]
pub fn music_shuffle(app: AppHandle) {
    update_config(&app, |c| c.music.shuffle = !c.music.shuffle);
    emit_state(&app);
}

#[tauri::command]
pub fn music_repeat(app: AppHandle) {
    update_config(&app, |c| {
        c.music.repeat = match c.music.repeat.as_str() {
            "off" => "all",
            "all" => "one",
            _ => "off",
        }
        .into()
    });
    emit_state(&app);
}

#[tauri::command]
pub async fn music_stop(app: AppHandle) {
    stop(&app);
}

/// Dalla finestra Impostazioni: prova (cerca e suona).
#[tauri::command]
pub async fn music_play(app: AppHandle, query: String) -> Result<String, String> {
    play(&app, &query, false).await
}

#[tauri::command]
pub fn music_settings(app: AppHandle, lyrics: bool, reactive_sprite: bool, compact: Option<bool>, music_sprite: Option<bool>) {
    update_config(&app, |c| {
        if let Some(v) = music_sprite {
            c.music.music_sprite = v;
        }
        c.music.lyrics = lyrics;
        c.music.reactive_sprite = reactive_sprite;
        if let Some(v) = compact {
            c.music.compact = v;
        }
    });
    if compact == Some(false) {
        crate::window::set_compact(&app, false);
    }
    emit_state(&app);
}

// NB: su Windows creare o chiudere finestre da un comando sincrono blocca l'app: questi sono async.

#[tauri::command]
pub async fn player_resize(app: AppHandle, width: f64) {
    resize_player(&app, width);
}

/// Dalla pagina del lettore/Venue agganciata: ridimensiona per la nuova modalità (poi la pagina cambia).
#[tauri::command]
pub async fn player_expand(app: AppHandle, on: bool) {
    apply_mode(&app, on);
}

#[tauri::command]
pub async fn venue_open(app: AppHandle) {
    open_venue(&app);
}

#[tauri::command]
pub async fn venue_close(app: AppHandle) {
    close_venue(&app);
}

/// Separa la Venue in una finestra a parte (e lo ricorda).
#[tauri::command]
pub async fn venue_detach(app: AppHandle) {
    update_config(&app, |c| c.music.venue_docked = false);
    open_venue_window(&app);
    if is_expanded() {
        if state(&app).playing {
            request_mode(&app, false);
        } else {
            close_player(&app);
        }
    }
}

/// Riaggancia la Venue al lettore (e lo ricorda).
#[tauri::command]
pub async fn venue_dock(app: AppHandle) {
    update_config(&app, |c| c.music.venue_docked = true);
    close_venue_window(&app);
    request_mode(&app, true);
}

/// Fine dell'animazione di chiusura della Venue separata.
#[tauri::command]
pub async fn venue_closed(app: AppHandle) {
    if VENUE_CLOSING.swap(false, Ordering::SeqCst) {
        if let Some(w) = app.get_webview_window(VENUE) {
            let _ = w.close();
        }
    }
}

#[tauri::command]
pub fn venue_library(app: AppHandle) -> library::Library {
    app.state::<Music>().library.lock().unwrap().clone()
}

/// Preferito: `id` vuoto = brano attuale.
#[tauri::command]
pub fn venue_favorite(app: AppHandle, id: String) -> Result<bool, String> {
    if id.is_empty() {
        return favorite_current(&app, None);
    }
    let m = app.state::<Music>();
    let now = {
        let mut lib = m.library.lock().unwrap();
        let e = lib.find(&id).cloned().ok_or("brano non trovato")?;
        lib.toggle_favorite(e)
    };
    save_library(&app);
    emit_state(&app);
    Ok(now)
}

#[tauri::command]
pub fn venue_remove(app: AppHandle, id: String) {
    app.state::<Music>().library.lock().unwrap().remove_recent(&id);
    save_library(&app);
}

/// Riascolta un brano della libreria (dal link YouTube) o cerca una frase.
#[tauri::command]
pub async fn venue_play(app: AppHandle, id: String, query: Option<String>, enqueue: Option<bool>) -> Result<String, String> {
    let q = match query.filter(|q| !q.trim().is_empty()) {
        Some(q) => q,
        None => {
            let lib = app.state::<Music>().library.lock().unwrap().clone();
            let e = lib.find(&id).ok_or("brano non trovato")?;
            e.url.clone().unwrap_or_else(|| format!("https://www.youtube.com/watch?v={}", e.id))
        }
    };
    play(&app, &q, enqueue.unwrap_or(false)).await
}
