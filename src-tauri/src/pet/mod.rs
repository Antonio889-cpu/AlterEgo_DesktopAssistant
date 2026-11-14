//! Pet: nei momenti di calma Alter Ego esce dalla TV e gira per il desktop come un piccolo
//! sprite. Cammina, salta, sta in piedi sulle finestre aperte e sulla barra delle applicazioni,
//! si può prendere e lanciare col mouse. Torna nella TV quando le parli, quando deve parlare
//! lei o dopo un po' (vedi `PetConfig`).
//!
//! - `physics.rs`: fisica e decisioni (logica pura, con i test);
//! - `surfaces.rs`: finestre aperte e schermo intero (Windows);
//! - qui: la finestra trasparente del pet, il ciclo a ~60 fps e i comandi.
//!
//! Gli sprite: quello incluso (Chihiro, `src/assets/pet/chihiro/`) o uno importato da un file
//! JSON (formato "c2dictionary" con `urlSprites`, `urlFront`… e `runFrames`) che viene
//! scaricato in `%APPDATA%\AlterEgo\pets\<nome>\`.

pub mod physics;
mod surfaces;

use crate::{assistant::Conversation, config::PetConfig, update_config, AppState};
use physics::{Anim, Area, Event, Pet, World};
use serde::{Deserialize, Serialize};
use std::{
    collections::VecDeque,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Mutex,
    },
    thread,
    time::{Duration, Instant},
};
use tauri::{AppHandle, Emitter, LogicalSize, Manager, PhysicalPosition, WebviewUrl, WebviewWindowBuilder};

pub const LABEL: &str = "pet";
/// Finestra del pet in pixel logici (× dimensione): un fotogramma da 128 più lo spazio per lo sticker.
const WIN_W: f64 = 128.0;
const WIN_H: f64 = 168.0;
/// Corpo predefinito (frazioni della finestra) finché la pagina non misura lo sprite.
const DEFAULT_HIT: [f64; 4] = [0.28, 0.45, 0.72, 1.0];

static ACTIVE: AtomicBool = AtomicBool::new(false);

pub fn is_active() -> bool {
    ACTIVE.load(Ordering::SeqCst)
}

#[derive(Default)]
pub struct PetRt {
    inner: Mutex<Inner>,
}

struct Grab {
    /// Cursore − piedi del pet al momento della presa.
    off: (f64, f64),
    start: Instant,
    from: (f64, f64),
    moved: bool,
    hist: VecDeque<(Instant, f64, f64)>,
}

#[derive(Default)]
struct Inner {
    pet: Option<Pet>,
    hit: Option<[f64; 4]>,
    grab: Option<Grab>,
    last_click: Option<Instant>,
    /// Quando torna nella TV da sola.
    until: Option<Instant>,
    /// Richiesta di tornare a casa dal ciclo (doppio click o tempo scaduto).
    go_home: bool,
}

fn lock(rt: &PetRt) -> std::sync::MutexGuard<'_, Inner> {
    rt.inner.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn cfg(app: &AppHandle) -> PetConfig {
    app.state::<AppState>().cfg.lock().unwrap_or_else(std::sync::PoisonError::into_inner).pet.clone()
}

fn seed() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos() as u64).unwrap_or(0x9E37_79B9)
}

// ---------- uscita e rientro ----------

/// Si può uscire adesso? (non mentre c'è un gioco o un video a schermo intero)
#[tauri::command]
pub fn pet_can_explore() -> bool {
    !is_active() && !surfaces::fullscreen_foreground()
}

/// Alter Ego esce dalla TV. NB: async perché crea una finestra (su Windows da un comando
/// sincrono bloccherebbe l'app).
#[tauri::command]
pub async fn pet_explore(app: AppHandle) -> Result<bool, String> {
    if is_active() {
        return Ok(true);
    }
    if surfaces::fullscreen_foreground() {
        return Ok(false);
    }
    let main = app.get_webview_window(crate::window::MAIN).ok_or("finestra assente")?;
    let (Ok(pos), Ok(size)) = (main.outer_position(), main.outer_size()) else {
        return Ok(false);
    };
    let pc = cfg(&app);
    let win = match app.get_webview_window(LABEL) {
        Some(w) => w,
        None => WebviewWindowBuilder::new(&app, LABEL, WebviewUrl::App("pet.html".into()))
            .title("Alter Ego – Pet")
            .decorations(false)
            .transparent(true)
            .shadow(false)
            .always_on_top(true)
            .skip_taskbar(true)
            .resizable(false)
            .focused(false)
            .focusable(false)
            .visible(false)
            .inner_size(WIN_W * pc.size, WIN_H * pc.size)
            .build()
            .map_err(|e| e.to_string())?,
    };
    let scale = main.scale_factor().unwrap_or(1.0);
    let k = scale * pc.size;
    // esce dal centro in basso della TV con un saltello
    let (fx, fy) = (pos.x as f64 + size.width as f64 / 2.0, pos.y as f64 + size.height as f64 * 0.8);
    {
        let rt = app.state::<PetRt>();
        let mut g = lock(&rt);
        let hit = g.hit.unwrap_or(DEFAULT_HIT);
        let ws = win_size(&win, k);
        let mut pet = Pet::new(fx, fy, k, (hit[2] - hit[0]) * ws.0 / 2.0, (hit[3] - hit[1]) * ws.1, seed());
        let dir = if seed().is_multiple_of(2) { 1.0 } else { -1.0 };
        pet.release(dir * 160.0 * k, -820.0 * k);
        let p = window_pos(&pet, hit, ws);
        let _ = win.set_position(PhysicalPosition::new(p.0, p.1));
        g.pet = Some(pet);
        g.grab = None;
        g.go_home = false;
        g.until = (pc.stay_minutes > 0).then(|| {
            let mins = pc.stay_minutes as f64 * (0.6 + 0.4 * (seed() % 1000) as f64 / 1000.0);
            Instant::now() + Duration::from_secs_f64(mins * 60.0)
        });
    }
    ACTIVE.store(true, Ordering::SeqCst);
    crate::window::hide_main(&app, false);
    let _ = win.show();
    // dopo show: su Linux (GTK) una finestra mai mostrata non accetta il click-through
    let _ = win.set_ignore_cursor_events(true);
    let _ = app.emit_to(LABEL, "pet-spawn", ());
    notify(&app, true);
    crate::applog::info("pet", "Alter Ego esce a esplorare il desktop");
    Ok(true)
}

/// Ferma l'esplorazione (senza mostrare la TV). Restituisce true se era fuori.
/// La chiama `window::show_main`, quindi ogni modo di far ricomparire Alter Ego la richiama.
pub fn stop(app: &AppHandle) -> bool {
    if !ACTIVE.swap(false, Ordering::SeqCst) {
        return false;
    }
    if let Some(w) = app.get_webview_window(LABEL) {
        let _ = w.set_ignore_cursor_events(true);
        let _ = w.hide();
    }
    let rt = app.state::<PetRt>();
    let mut g = lock(&rt);
    g.grab = None;
    g.go_home = false;
    drop(g);
    notify(app, false);
    true
}

/// Torna nella TV (la finestra principale riappare).
#[tauri::command]
pub fn pet_home(app: AppHandle) {
    stop(&app);
    crate::window::show_main(&app);
}

/// Esce o rientra (menu della tray).
pub fn toggle(app: &AppHandle) {
    if is_active() {
        pet_home(app.clone());
    } else {
        pet_go(app.clone());
    }
}

/// Esce adesso (Impostazioni, tray): la TV mostra "START!" e poi chiama `pet_explore`.
#[tauri::command]
pub fn pet_go(app: AppHandle) {
    let _ = app.emit_to(crate::window::MAIN, "pet-go", ());
}

#[tauri::command]
pub fn pet_active() -> bool {
    is_active()
}

fn notify(app: &AppHandle, active: bool) {
    let _ = app.emit("pet", serde_json::json!({ "active": active }));
    crate::tray::refresh(app);
}

// ---------- mouse ----------

/// Preso col tasto sinistro.
#[tauri::command]
pub fn pet_grab(app: AppHandle) {
    let Ok(cur) = app.cursor_position() else { return };
    let rt = app.state::<PetRt>();
    let mut g = lock(&rt);
    let Some(pet) = g.pet.as_mut() else { return };
    let off = (cur.x - pet.x, cur.y - pet.y);
    pet.grab();
    let now = Instant::now();
    g.grab = Some(Grab { off, start: now, from: (cur.x, cur.y), moved: false, hist: VecDeque::from([(now, cur.x, cur.y)]) });
}

/// Lasciato andare (anche quando il ciclo vede il tasto rilasciato).
#[tauri::command]
pub fn pet_release(app: AppHandle) {
    release(&app);
}

fn release(app: &AppHandle) {
    let rt = app.state::<PetRt>();
    let mut g = lock(&rt);
    let Some(grab) = g.grab.take() else { return };
    let Some(pet) = g.pet.as_mut() else { return };
    if !grab.moved && grab.start.elapsed() < Duration::from_millis(450) {
        // click: saltello e faccina felice; doppio click: torna nella TV
        pet.drop_in_place();
        pet.hop_in_air();
        let now = Instant::now();
        if g.last_click.is_some_and(|t| now.duration_since(t) < Duration::from_millis(450)) {
            g.last_click = None;
            g.go_home = true;
        } else {
            g.last_click = Some(now);
            let _ = app.emit_to(LABEL, "pet-sticker", 1);
        }
        return;
    }
    // lancio: velocità del cursore negli ultimi ~90 ms
    let (mut vx, mut vy) = (0.0, 0.0);
    if let (Some(a), Some(b)) = (grab.hist.front(), grab.hist.back()) {
        let dt = b.0.duration_since(a.0).as_secs_f64();
        if dt > 0.01 {
            vx = (b.1 - a.1) / dt;
            vy = (b.2 - a.2) / dt;
        }
    }
    pet.release(vx, vy);
}

/// La pagina ha misurato lo sprite: rettangolo del corpo in frazioni della finestra.
#[tauri::command]
pub fn pet_ready(app: AppHandle, hit: [f64; 4]) {
    if hit.iter().any(|v| !v.is_finite() || *v < 0.0 || *v > 1.0) || hit[2] <= hit[0] || hit[3] <= hit[1] {
        return;
    }
    let rt = app.state::<PetRt>();
    lock(&rt).hit = Some(hit);
}

// ---------- ciclo ----------

/// Dimensione vera della finestra in pixel fisici (il sistema può non dare quella chiesta:
/// su Linux GTK la allarga), altrimenti quella teorica.
fn win_size(win: &tauri::WebviewWindow, k: f64) -> (f64, f64) {
    match win.outer_size() {
        Ok(s) if s.width > 0 && s.height > 0 => (s.width as f64, s.height as f64),
        _ => (WIN_W * k, WIN_H * k),
    }
}

/// Posizione della finestra con i piedi del pet in (x, y).
fn window_pos(pet: &Pet, hit: [f64; 4], (ww, wh): (f64, f64)) -> (i32, i32) {
    ((pet.x - (hit[0] + hit[2]) / 2.0 * ww).round() as i32, (pet.y - hit[3] * wh).round() as i32)
}

fn areas(app: &AppHandle) -> Vec<Area> {
    app.available_monitors()
        .unwrap_or_default()
        .iter()
        .map(|m| {
            let a = m.work_area();
            Area { l: a.position.x as f64, t: a.position.y as f64, r: (a.position.x + a.size.width as i32) as f64, b: (a.position.y + a.size.height as i32) as f64 }
        })
        .collect()
}

#[derive(Serialize, Clone, PartialEq)]
struct AnimMsg {
    anim: &'static str,
    right: bool,
}

fn anim_name(a: Anim) -> &'static str {
    match a {
        Anim::Idle => "idle",
        Anim::Run => "run",
        Anim::Jump => "jump",
        Anim::Fall => "fall",
        Anim::Held => "held",
    }
}

/// Thread del pet: dorme finché il pet non esce, poi gira a ~60 fps.
pub fn spawn(app: AppHandle) {
    let _ = thread::Builder::new().name("pet".into()).spawn(move || {
        let mut world = World::default();
        let mut last_areas: Option<Instant> = None;
        let mut last_world = Instant::now();
        let mut last_tick = Instant::now();
        let mut last_pos = (i32::MIN, i32::MIN);
        let mut last_anim: Option<AnimMsg> = None;
        let mut cursor_on = false;
        let mut paused = false;
        // pixel fisici per pixel logico (DPI del monitor del pet × dimensione)
        let mut k = 1.0;
        let mut ws = (WIN_W, WIN_H);
        #[cfg_attr(not(windows), allow(unused_mut))]
        let mut skip: isize = 0;
        let mut ticks: u64 = 0;
        loop {
            if !is_active() {
                last_areas = None;
                last_anim = None;
                last_pos = (i32::MIN, i32::MIN);
                cursor_on = false;
                paused = false;
                thread::sleep(Duration::from_millis(200));
                last_tick = Instant::now();
                continue;
            }
            let frame_start = Instant::now();
            let Some(win) = app.get_webview_window(LABEL) else {
                thread::sleep(Duration::from_millis(100));
                continue;
            };
            ticks += 1;
            // monitor: ogni 2 s (cambiano di rado), con la scala DPI di quello dove sta il pet
            if last_areas.is_none_or(|t| t.elapsed() > Duration::from_secs(2)) {
                world.areas = areas(&app);
                last_areas = Some(Instant::now());
                let size = cfg(&app).size;
                let scale = win.scale_factor().unwrap_or(1.0);
                if (k - scale * size).abs() > 1e-3 {
                    k = scale * size;
                    let _ = win.set_size(LogicalSize::new(WIN_W * size, WIN_H * size));
                }
                ws = win_size(&win, k);
                // la finestra del pet non è una piattaforma
                #[cfg(windows)]
                {
                    skip = win.hwnd().map(|h| h.0 as isize).unwrap_or(0);
                }
            }
            // gioco o video a schermo intero davanti: il pet si nasconde e aspetta
            if ticks.is_multiple_of(30) {
                let full = surfaces::fullscreen_foreground();
                if full != paused {
                    paused = full;
                    let _ = if full { win.hide() } else { win.show() };
                }
            }
            if paused {
                thread::sleep(Duration::from_millis(250));
                last_tick = Instant::now();
                continue;
            }
            // finestre aperte: ~20 volte al secondo
            if ticks % 3 == 1 || world.platforms.is_empty() || last_world.elapsed() > Duration::from_millis(60) {
                let floors = World::from_areas(world.areas.clone());
                let mut platforms = floors.platforms;
                platforms.extend(physics::platforms_from_windows(&surfaces::windows(skip), &world.areas, 70.0 * k));
                world.platforms = platforms;
                last_world = Instant::now();
            }
            let cur = app.cursor_position().ok();
            let dt = last_tick.elapsed().as_secs_f64();
            last_tick = Instant::now();

            let rt = app.state::<PetRt>();
            let mut g = lock(&rt);
            // tasto rilasciato fuori dalla finestra (Windows): il rilascio non va perso
            if g.grab.is_some() && surfaces::left_button_down() == Some(false) {
                drop(g);
                release(&app);
                g = lock(&rt);
            }
            let hit = g.hit.unwrap_or(DEFAULT_HIT);
            if let Some(pet) = g.pet.as_mut() {
                // dimensioni del corpo aggiornate (DPI del monitor, dimensione, sprite misurato)
                pet.k = k;
                pet.half_w = (hit[2] - hit[0]) * ws.0 / 2.0;
                pet.height = (hit[3] - hit[1]) * ws.1;
            }
            let mut sticker = None;
            if let (Some(grab), Some(c)) = (g.grab.as_mut(), cur) {
                let now = Instant::now();
                grab.hist.push_back((now, c.x, c.y));
                while grab.hist.len() > 2 && now.duration_since(grab.hist[0].0) > Duration::from_millis(90) {
                    grab.hist.pop_front();
                }
                if !grab.moved && ((c.x - grab.from.0).abs() > 6.0 || (c.y - grab.from.1).abs() > 6.0) {
                    grab.moved = true;
                    sticker = Some(2);
                }
                let off = grab.off;
                if let Some(pet) = g.pet.as_mut() {
                    pet.move_held(c.x - off.0, c.y - off.1);
                }
            }
            let mut events = Vec::new();
            if let Some(pet) = g.pet.as_mut() {
                events = pet.step(dt, &world);
            }
            if events.contains(&Event::HardLanding) {
                sticker = Some(2);
            }
            let held = g.grab.is_some();
            let expired = g.until.is_some_and(|t| Instant::now() >= t) && !held && g.pet.as_ref().is_some_and(|p| p.on_ground());
            let go_home = std::mem::take(&mut g.go_home) || expired;
            let msg = g.pet.as_ref().map(|p| AnimMsg { anim: anim_name(p.anim()), right: p.facing_right });
            let pos = g.pet.as_ref().map(|p| window_pos(p, hit, ws));
            drop(g);

            if go_home {
                pet_home(app.clone());
                continue;
            }
            if let Some(p) = pos {
                if p != last_pos {
                    let _ = win.set_position(PhysicalPosition::new(p.0, p.1));
                    last_pos = p;
                }
            }
            if msg != last_anim {
                if let Some(m) = &msg {
                    let _ = app.emit_to(LABEL, "pet-anim", m);
                }
                last_anim = msg;
            }
            if let Some(n) = sticker {
                let _ = app.emit_to(LABEL, "pet-sticker", n);
            }
            // il mouse funziona solo sopra lo sprite: il resto della finestra è trasparente ai click
            if ticks.is_multiple_of(2) || held {
                let inside = held
                    || match (cur, last_pos) {
                        (Some(c), wp) if wp.0 != i32::MIN => {
                            let (l, t) = (wp.0 as f64 + hit[0] * ws.0, wp.1 as f64 + hit[1] * ws.1);
                            let (r, b) = (wp.0 as f64 + hit[2] * ws.0, wp.1 as f64 + hit[3] * ws.1);
                            c.x >= l && c.x < r && c.y >= t && c.y < b
                        }
                        _ => false,
                    };
                if inside != cursor_on {
                    cursor_on = inside;
                    let _ = win.set_ignore_cursor_events(!inside);
                }
            }
            let spent = frame_start.elapsed();
            if spent < Duration::from_millis(16) {
                thread::sleep(Duration::from_millis(16) - spent);
            }
        }
    });
}

// ---------- sprite: incluso e importati ----------

/// Campi del JSON → nome del file salvato.
const FILES: [(&str, &str); 6] = [
    ("urlSprites", "sprites.png"),
    ("urlFront", "front.png"),
    ("urlSide", "side.png"),
    ("urlStart", "start.png"),
    ("urlSticker1", "sticker1.png"),
    ("urlSticker2", "sticker2.png"),
];
const BUNDLED_JSON: &str = include_str!("../../../src/assets/pet/chihiro/pet.json");
const BUNDLED_DIR: &str = "assets/pet/chihiro";

/// Descrizione di un pet (il JSON "c2dictionary" o lo stesso oggetto senza `data`).
#[derive(Deserialize, Default, Clone)]
#[serde(default, rename_all = "camelCase")]
struct PetDef {
    name: String,
    long_name: String,
    author: String,
    run_frames: u32,
    url_sprites: String,
    url_front: String,
    url_side: String,
    url_start: String,
    url_sticker1: String,
    url_sticker2: String,
}

impl PetDef {
    fn parse(text: &str) -> Result<Self, String> {
        let v: serde_json::Value = serde_json::from_str(text.trim_start_matches('\u{feff}')).map_err(|e| crate::tr!("JSON non valido: {e}", "invalid JSON: {e}"))?;
        let data = v.get("data").cloned().unwrap_or(v);
        let d: PetDef = serde_json::from_value(data).map_err(|e| crate::tr!("JSON non valido: {e}", "invalid JSON: {e}"))?;
        if d.url_sprites.trim().is_empty() {
            return Err(crate::i18n::tr("manca \"urlSprites\" (il foglio con le animazioni)", "\"urlSprites\" is missing (the sheet with the animations)").into());
        }
        Ok(d)
    }

    fn url(&self, key: &str) -> &str {
        match key {
            "urlSprites" => &self.url_sprites,
            "urlFront" => &self.url_front,
            "urlSide" => &self.url_side,
            "urlStart" => &self.url_start,
            "urlSticker1" => &self.url_sticker1,
            "urlSticker2" => &self.url_sticker2,
            _ => "",
        }
    }
}

/// Quello che serve alle pagine per disegnare il pet.
#[derive(Serialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct PetAssets {
    /// Vuoto = incluso.
    id: String,
    name: String,
    long_name: String,
    author: String,
    run_frames: u32,
    sprites: String,
    front: String,
    side: String,
    start: String,
    sticker1: String,
    sticker2: String,
}

fn pets_dir(app: &AppHandle) -> PathBuf {
    app.state::<Conversation>().data_dir.join("pets")
}

fn data_uri(path: &Path) -> String {
    use base64::Engine;
    std::fs::read(path).map(|b| format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(b))).unwrap_or_default()
}

fn assets_from(id: &str, def: &PetDef, file: impl Fn(&str) -> String) -> PetAssets {
    PetAssets {
        id: id.into(),
        name: def.name.clone(),
        long_name: def.long_name.clone(),
        author: def.author.clone(),
        run_frames: def.run_frames.clamp(1, 16),
        sprites: file("sprites.png"),
        front: file("front.png"),
        side: file("side.png"),
        start: file("start.png"),
        sticker1: file("sticker1.png"),
        sticker2: file("sticker2.png"),
    }
}

fn bundled() -> PetAssets {
    let def = PetDef::parse(BUNDLED_JSON).unwrap_or_default();
    assets_from("", &def, |f| format!("{BUNDLED_DIR}/{f}"))
}

fn load_skin(app: &AppHandle, id: &str) -> Option<PetAssets> {
    let dir = pets_dir(app).join(id);
    let def = PetDef::parse(&std::fs::read_to_string(dir.join("pet.json")).ok()?).ok()?;
    let a = assets_from(id, &def, |f| data_uri(&dir.join(f)));
    (!a.sprites.is_empty()).then_some(a)
}

/// Sprite del pet in uso (se quello importato manca si torna a quello incluso).
#[tauri::command]
pub fn pet_assets(app: AppHandle) -> PetAssets {
    let skin = cfg(&app).skin;
    if skin.is_empty() {
        return bundled();
    }
    load_skin(&app, &skin).unwrap_or_else(bundled)
}

#[derive(Serialize)]
pub struct PetEntry {
    id: String,
    name: String,
}

/// Pet disponibili: quello incluso e quelli importati.
#[tauri::command]
pub fn pet_list(app: AppHandle) -> Vec<PetEntry> {
    let b = bundled();
    let mut out = vec![PetEntry { id: String::new(), name: format!("{} ({})", b.name, crate::i18n::tr("incluso", "built in")) }];
    if let Ok(rd) = std::fs::read_dir(pets_dir(&app)) {
        let mut v: Vec<PetEntry> = rd
            .flatten()
            .filter_map(|e| {
                let id = e.file_name().to_string_lossy().to_string();
                let def = PetDef::parse(&std::fs::read_to_string(e.path().join("pet.json")).ok()?).ok()?;
                Some(PetEntry { name: if def.name.is_empty() { id.clone() } else { def.name }, id })
            })
            .collect();
        v.sort_by_key(|e| e.name.to_lowercase());
        out.extend(v);
    }
    out
}

/// Nome della cartella: lettere e numeri del nome del pet.
fn slug(name: &str) -> String {
    let s: String = name
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_lowercase() } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|p| !p.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    let s: String = s.chars().take(40).collect();
    if s.is_empty() { "pet".into() } else { s }
}

/// Controlla che il foglio sia 2 righe di fotogrammi quadrati (riga 1: fermo, fermo, salto,
/// caduta; riga 2: corsa).
fn check_sheet(bytes: &[u8], run_frames: u32) -> Result<(), String> {
    let dec = png::Decoder::new(std::io::Cursor::new(bytes));
    let reader = dec.read_info().map_err(|_| crate::i18n::tr("il foglio sprite non è un PNG valido", "the sprite sheet is not a valid PNG").to_string())?;
    let (w, h) = (reader.info().width, reader.info().height);
    let frame = h / 2;
    if h < 16 || h % 2 != 0 || w % frame != 0 || w / frame < 4.max(run_frames) {
        return Err(crate::tr!(
            "foglio sprite {w}×{h}: servono 2 righe di fotogrammi quadrati (almeno 4 per riga)",
            "sprite sheet {w}×{h}: it needs 2 rows of square frames (at least 4 per row)"
        ));
    }
    Ok(())
}

fn is_png(bytes: &[u8]) -> bool {
    bytes.starts_with(b"\x89PNG\r\n\x1a\n")
}

async fn download(client: &reqwest::Client, url: &str) -> Result<Vec<u8>, String> {
    const MAX: usize = 12 * 1024 * 1024;
    let r = client.get(url).send().await.map_err(|e| format!("{url}: {e}"))?;
    if !r.status().is_success() {
        return Err(format!("{url}: HTTP {}", r.status().as_u16()));
    }
    if r.content_length().is_some_and(|n| n as usize > MAX) {
        return Err(crate::tr!("{url}: file troppo grande", "{url}: file too large"));
    }
    let b = r.bytes().await.map_err(|e| format!("{url}: {e}"))?;
    if b.len() > MAX {
        return Err(crate::tr!("{url}: file troppo grande", "{url}: file too large"));
    }
    if !is_png(&b) {
        return Err(crate::tr!("{url}: non è un'immagine PNG", "{url}: not a PNG image"));
    }
    Ok(b.to_vec())
}

/// Importa un pet da un file JSON: scarica le immagini nella cartella `pets\<nome>\`
/// e lo mette in uso. Restituisce il nome.
#[tauri::command]
pub async fn pet_import(app: AppHandle, json: String) -> Result<String, String> {
    let def = PetDef::parse(&json)?;
    let id = slug(if def.name.trim().is_empty() { &def.long_name } else { &def.name });
    let client = reqwest::Client::builder().timeout(Duration::from_secs(30)).build().map_err(|e| e.to_string())?;
    let mut files = Vec::new();
    for (key, file) in FILES {
        let url = def.url(key).trim();
        if url.is_empty() {
            continue;
        }
        if !(url.starts_with("https://") || url.starts_with("http://")) {
            return Err(crate::tr!("\"{key}\": serve un indirizzo http(s)", "\"{key}\": an http(s) address is needed"));
        }
        let bytes = download(&client, url).await?;
        if key == "urlSprites" {
            check_sheet(&bytes, def.run_frames)?;
        }
        files.push((file, bytes));
    }
    let dir = pets_dir(&app).join(&id);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    for (file, bytes) in files {
        std::fs::write(dir.join(file), bytes).map_err(|e| e.to_string())?;
    }
    std::fs::write(dir.join("pet.json"), json.as_bytes()).map_err(|e| e.to_string())?;
    update_config(&app, |c| c.pet.skin = id.clone());
    let _ = app.emit("pet-assets", ());
    crate::applog::info("pet", &format!("pet importato: {id}"));
    Ok(if def.name.is_empty() { id } else { def.name })
}

/// Sceglie il pet ("" = quello incluso).
#[tauri::command]
pub fn pet_select(app: AppHandle, id: String) -> Result<(), String> {
    if !id.is_empty() && load_skin(&app, &id).is_none() {
        return Err(crate::i18n::tr("pet non trovato o senza foglio sprite", "pet not found or without a sprite sheet").into());
    }
    update_config(&app, |c| c.pet.skin = id);
    let _ = app.emit("pet-assets", ());
    Ok(())
}

/// Elimina un pet importato.
#[tauri::command]
pub fn pet_remove(app: AppHandle, id: String) -> Result<(), String> {
    if id.is_empty() || id.contains(['/', '\\', '.']) {
        return Err(crate::i18n::tr("il pet incluso non si può eliminare", "the built-in pet can't be deleted").into());
    }
    let dir = pets_dir(&app).join(&id);
    if dir.exists() {
        std::fs::remove_dir_all(dir).map_err(|e| e.to_string())?;
    }
    if cfg(&app).skin == id {
        update_config(&app, |c| c.pet.skin = String::new());
    }
    let _ = app.emit("pet-assets", ());
    Ok(())
}

#[tauri::command]
pub fn pet_open_dir(app: AppHandle) -> Result<(), String> {
    let dir = pets_dir(&app);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    #[cfg(windows)]
    let r = std::process::Command::new("explorer").arg(&dir).spawn();
    #[cfg(not(windows))]
    let r = std::process::Command::new("xdg-open").arg(&dir).spawn();
    r.map(|_| ()).map_err(|e| e.to_string())
}

/// Impostazioni del pet (scheda Pet).
#[tauri::command]
pub fn pet_settings(app: AppHandle, enabled: bool, idle_minutes: u32, frequency: String, stay_minutes: u32, size: f64) {
    update_config(&app, |c| {
        c.pet.enabled = enabled;
        c.pet.idle_minutes = idle_minutes;
        c.pet.frequency = frequency;
        c.pet.stay_minutes = stay_minutes;
        c.pet.size = size;
        c.pet.normalize();
    });
    // la nuova durata vale da subito anche se è già fuori
    let pc = cfg(&app);
    let rt = app.state::<PetRt>();
    let mut g = lock(&rt);
    if g.until.is_some() || pc.stay_minutes == 0 {
        g.until = (pc.stay_minutes > 0).then(|| Instant::now() + Duration::from_secs(pc.stay_minutes as u64 * 60));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_c2dictionary_and_flat_json() {
        let d = PetDef::parse(BUNDLED_JSON).unwrap();
        assert_eq!(d.name, "Chihiro");
        assert_eq!(d.run_frames, 4);
        let flat = r#"{"name":"Test Pet","urlSprites":"https://example.com/s.png","runFrames":6}"#;
        let d = PetDef::parse(flat).unwrap();
        assert_eq!((d.name.as_str(), d.run_frames), ("Test Pet", 6));
        assert!(PetDef::parse(r#"{"data":{"name":"x"}}"#).is_err());
        assert!(PetDef::parse("non json").is_err());
        assert_eq!(slug("Chihiro Fujisaki!"), "chihiro-fujisaki");
        assert_eq!(slug("../.."), "pet");
    }

    #[test]
    fn bundled_sheet_is_valid() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../src").join(BUNDLED_DIR);
        let sheet = std::fs::read(dir.join("sprites.png")).unwrap();
        check_sheet(&sheet, 4).unwrap();
        for (_, f) in FILES {
            assert!(is_png(&std::fs::read(dir.join(f)).unwrap()), "{f}");
        }
        let a = bundled();
        assert_eq!(a.sprites, "assets/pet/chihiro/sprites.png");
        assert_eq!(a.run_frames, 4);
        // un foglio che non è a fotogrammi quadrati viene rifiutato
        let bad = crate::actions::screen::encode_png(300, 64, &vec![0u8; 300 * 64 * 4]).unwrap();
        assert!(check_sheet(&bad, 4).is_err());
    }
}
