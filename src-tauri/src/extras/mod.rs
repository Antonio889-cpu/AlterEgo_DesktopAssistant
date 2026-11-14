//! Contenuti aggiuntivi: un pacchetto (ZIP) scaricato da un link diretto, con immagini, musica,
//! Line vocali ed effetti. Durante i giochi riconosciuti, ogni tanto e a caso, la finestra di
//! Alter Ego viene "presa" da un ospite (vedi `src/extras.js`): qui il riconoscimento dei giochi,
//! i tempi, lo screenshot per il modello AI, l'audio e l'installazione del pacchetto.
//!
//! Formato del pacchetto (`pack.json` alla radice dello ZIP):
//! ```json
//! { "format": 1, "name": "…", "nameTag": "…",
//!   "background": "background/office.jpg",
//!   "music": { "file": "music/theme.mp3", "volume": 0.55 },
//!   "sfx": { "intro": "sfx/intro.ogg", "outro": "sfx/outro.ogg" },
//!   "fonts": { "name": "fonts/….ttf", "text": "fonts/….ttf" },
//!   "sprites": { "idle": "sprites/idle.png", "laugh": "…" },
//!   "lines": [ { "file": "lines/hehe.ogg", "moods": ["laugh", "giggle"], "text": "Hehe" } ] }
//! ```
//! Gli umori `intro` e `outro` scelgono le Line di entrata e di uscita; lo sprite `leave` si usa
//! all'uscita.

mod games;
mod prompt;

use crate::{
    audio::{AudioEngine, Kind},
    update_config, AppState,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc, Mutex,
    },
    thread,
    time::{Duration, Instant},
};
use tauri::{AppHandle, Emitter, Manager};

/// Link diretto predefinito del pacchetto (lo ZIP con le risorse).
pub const DEFAULT_URL: &str = "https://alterego.attentoweb.it/asset.zip";

const MAX_ZIP: u64 = 150 * 1024 * 1024;

// ---------- pacchetto ----------

#[derive(Deserialize, Serialize, Clone, Default, Debug)]
#[serde(default, rename_all = "camelCase")]
pub struct Pack {
    pub format: u32,
    pub name: String,
    pub name_tag: String,
    pub background: String,
    pub music: Option<Music>,
    pub sfx: BTreeMap<String, String>,
    pub fonts: BTreeMap<String, String>,
    pub sprites: BTreeMap<String, String>,
    pub lines: Vec<Line>,
}

#[derive(Deserialize, Serialize, Clone, Default, Debug)]
#[serde(default)]
pub struct Music {
    pub file: String,
    pub volume: f64,
}

#[derive(Deserialize, Serialize, Clone, Default, Debug)]
#[serde(default)]
pub struct Line {
    pub file: String,
    pub moods: Vec<String>,
    pub text: String,
}

impl Pack {
    /// Umori che il modello può scegliere (gli sprite, tranne quello di uscita).
    pub fn moods(&self) -> Vec<String> {
        self.sprites.keys().filter(|k| k.as_str() != "leave").cloned().collect()
    }

    /// Tutti i percorsi citati, per controllarli all'installazione.
    fn files(&self) -> Vec<&str> {
        let mut v: Vec<&str> = vec![self.background.as_str()];
        v.extend(self.music.as_ref().map(|m| m.file.as_str()));
        v.extend(self.sfx.values().map(String::as_str));
        v.extend(self.fonts.values().map(String::as_str));
        v.extend(self.sprites.values().map(String::as_str));
        v.extend(self.lines.iter().map(|l| l.file.as_str()));
        v.into_iter().filter(|s| !s.is_empty()).collect()
    }
}

fn data_dir(app: &AppHandle) -> PathBuf {
    app.state::<crate::assistant::Conversation>().data_dir.clone()
}

fn pack_dir(app: &AppHandle) -> PathBuf {
    data_dir(app).join("extras")
}

/// Percorso dentro la cartella del pacchetto (niente `..` né percorsi assoluti).
fn inside(dir: &Path, rel: &str) -> Option<PathBuf> {
    let p = Path::new(rel);
    if rel.is_empty() || p.is_absolute() || p.components().any(|c| !matches!(c, std::path::Component::Normal(_))) {
        return None;
    }
    Some(dir.join(p))
}

pub fn load(dir: &Path) -> Option<Pack> {
    let text = std::fs::read_to_string(dir.join("pack.json")).ok()?;
    let pack: Pack = serde_json::from_str(text.trim_start_matches('\u{feff}')).ok()?;
    (!pack.sprites.is_empty()).then_some(pack)
}

fn installed(app: &AppHandle) -> Option<Pack> {
    load(&pack_dir(app))
}

fn pack_url(app: &AppHandle) -> String {
    let u = app.state::<AppState>().cfg.lock().unwrap_or_else(std::sync::PoisonError::into_inner).extras.url.trim().to_string();
    if u.is_empty() { DEFAULT_URL.to_string() } else { u }
}

/// Link di condivisione di Google Drive e Dropbox → link di download diretto.
pub fn direct_link(url: &str) -> String {
    let u = url.trim();
    if let Some(rest) = u.split("drive.google.com/file/d/").nth(1) {
        let id: String = rest.chars().take_while(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_').collect();
        return format!("https://drive.usercontent.google.com/download?id={id}&export=download&confirm=t");
    }
    if u.contains("drive.google.com") {
        if let Some(id) = u.split("id=").nth(1).map(|s| s.split('&').next().unwrap_or("")) {
            return format!("https://drive.usercontent.google.com/download?id={id}&export=download&confirm=t");
        }
    }
    if u.contains("dropbox.com") {
        let base = u.replace("?dl=0", "").replace("&dl=0", "");
        return if base.contains('?') { format!("{base}&dl=1") } else { format!("{base}?dl=1") };
    }
    u.to_string()
}

/// Estrae lo ZIP in `dest` (che viene creata) e controlla `pack.json` e i file citati.
pub fn extract(zip_bytes: &[u8], dest: &Path) -> Result<Pack, String> {
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(zip_bytes)).map_err(|e| crate::tr!("archivio non valido: {e}", "invalid archive: {e}"))?;
    // pacchetto dentro una cartella dello ZIP: si usa quella come radice
    let root = (0..zip.len())
        .filter_map(|i| zip.by_index(i).ok().and_then(|f| f.enclosed_name()))
        .filter(|p| p.file_name().is_some_and(|n| n == "pack.json") && !p.components().any(|c| c.as_os_str() == "__MACOSX"))
        .min_by_key(|p| p.components().count())
        .ok_or_else(|| crate::i18n::tr("nel pacchetto manca pack.json", "pack.json is missing from the package").to_string())?
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_default();
    std::fs::create_dir_all(dest).map_err(|e| e.to_string())?;
    let mut total = 0u64;
    for i in 0..zip.len() {
        let mut f = zip.by_index(i).map_err(|e| e.to_string())?;
        let Some(name) = f.enclosed_name() else { continue };
        let Ok(rel) = name.strip_prefix(&root) else { continue };
        if f.is_dir() || rel.as_os_str().is_empty() || name.components().any(|c| c.as_os_str() == "__MACOSX") {
            continue;
        }
        total += f.size();
        if total > MAX_ZIP * 2 {
            return Err(crate::i18n::tr("pacchetto troppo grande", "package too large").into());
        }
        let out = dest.join(rel);
        if let Some(p) = out.parent() {
            std::fs::create_dir_all(p).map_err(|e| e.to_string())?;
        }
        let mut w = std::fs::File::create(&out).map_err(|e| e.to_string())?;
        std::io::copy(&mut f, &mut w).map_err(|e| e.to_string())?;
    }
    let pack = load(dest).ok_or_else(|| crate::i18n::tr("pack.json non valido o senza immagini", "pack.json invalid or without images").to_string())?;
    for rel in pack.files() {
        let ok = inside(dest, rel).is_some_and(|p| p.is_file());
        if !ok {
            return Err(crate::tr!("nel pacchetto manca {rel}", "{rel} is missing from the package"));
        }
    }
    Ok(pack)
}

#[derive(Serialize, Clone)]
pub struct Status {
    installed: bool,
    name: String,
    url: String,
    default_url: bool,
    events: bool,
    asked: bool,
}

#[tauri::command]
pub fn extras_status(app: AppHandle) -> Status {
    let p = installed(&app);
    let cfg = app.state::<AppState>().cfg.lock().unwrap_or_else(std::sync::PoisonError::into_inner).extras.clone();
    Status { installed: p.is_some(), name: p.map(|p| p.name).unwrap_or_default(), url: pack_url(&app), default_url: cfg.url.trim().is_empty(), events: cfg.events, asked: cfg.asked }
}

static INSTALLING: AtomicBool = AtomicBool::new(false);

/// Scarica e installa il pacchetto dal link (quello delle impostazioni o il predefinito).
/// Avanzamento con l'evento "extras-progress" { done, total } (byte).
#[tauri::command]
pub async fn extras_install(app: AppHandle, url: Option<String>) -> Result<String, String> {
    if let Some(u) = url.filter(|u| !u.trim().is_empty()) {
        let u = u.trim().to_string();
        update_config(&app, |c| c.extras.url = if u == DEFAULT_URL { String::new() } else { u.clone() });
    }
    let link = direct_link(&pack_url(&app));
    if !(link.starts_with("https://") || link.starts_with("http://")) {
        return Err(crate::i18n::tr("serve un link diretto http(s)", "a direct http(s) link is needed").into());
    }
    if INSTALLING.swap(true, Ordering::SeqCst) {
        return Err(crate::i18n::tr("download già in corso", "download already in progress").into());
    }
    let r = install(&app, &link).await;
    INSTALLING.store(false, Ordering::SeqCst);
    update_config(&app, |c| c.extras.asked = true);
    match &r {
        Ok(n) => crate::applog::info("extras", &format!("pacchetto installato: {n}")),
        Err(e) => crate::applog::warn("extras", &format!("installazione fallita: {e}")),
    }
    let _ = app.emit("extras-changed", ());
    r
}

async fn install(app: &AppHandle, link: &str) -> Result<String, String> {
    use futures_util::StreamExt;
    let client = reqwest::Client::builder().timeout(Duration::from_secs(600)).build().map_err(|e| e.to_string())?;
    let r = client.get(link).send().await.map_err(|e| crate::tr!("download: {e}", "download: {e}"))?;
    if !r.status().is_success() {
        return Err(format!("HTTP {}", r.status().as_u16()));
    }
    let total = r.content_length().unwrap_or(0);
    if total > MAX_ZIP {
        return Err(crate::i18n::tr("pacchetto troppo grande", "package too large").into());
    }
    let mut bytes = Vec::with_capacity(total as usize);
    let mut stream = r.bytes_stream();
    let mut last = Instant::now();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| crate::tr!("download: {e}", "download: {e}"))?;
        bytes.extend_from_slice(&chunk);
        if bytes.len() as u64 > MAX_ZIP {
            return Err(crate::i18n::tr("pacchetto troppo grande", "package too large").into());
        }
        if last.elapsed() > Duration::from_millis(150) {
            last = Instant::now();
            let _ = app.emit("extras-progress", serde_json::json!({ "done": bytes.len(), "total": total }));
        }
    }
    let _ = app.emit("extras-progress", serde_json::json!({ "done": bytes.len(), "total": bytes.len() }));
    if !bytes.starts_with(b"PK") {
        return Err(crate::i18n::tr("il link non porta a un file ZIP (serve un link diretto)", "the link doesn't lead to a ZIP file (a direct link is needed)").into());
    }
    let dir = pack_dir(app);
    let tmp = dir.with_extension("new");
    let _ = std::fs::remove_dir_all(&tmp);
    let pack = {
        let tmp = tmp.clone();
        tauri::async_runtime::spawn_blocking(move || extract(&bytes, &tmp)).await.map_err(|e| e.to_string())?
    };
    let pack = match pack {
        Ok(p) => p,
        Err(e) => {
            let _ = std::fs::remove_dir_all(&tmp);
            return Err(e);
        }
    };
    let old = dir.with_extension("old");
    let _ = std::fs::remove_dir_all(&old);
    if dir.exists() {
        std::fs::rename(&dir, &old).map_err(|e| e.to_string())?;
    }
    std::fs::rename(&tmp, &dir).map_err(|e| e.to_string())?;
    let _ = std::fs::remove_dir_all(&old);
    clear_cache();
    Ok(if pack.name.is_empty() { "ok".into() } else { pack.name })
}

/// Risposta alla domanda del primo avvio ("più tardi": non si chiede più).
#[tauri::command]
pub fn extras_asked(app: AppHandle) {
    update_config(&app, |c| c.extras.asked = true);
}

#[tauri::command]
pub fn extras_settings(app: AppHandle, events: bool, url: Option<String>) {
    update_config(&app, |c| {
        c.extras.events = events;
        if let Some(u) = url {
            let u = u.trim().to_string();
            c.extras.url = if u == DEFAULT_URL { String::new() } else { u };
        }
    });
}

#[tauri::command]
pub fn extras_remove(app: AppHandle) -> Result<(), String> {
    let dir = pack_dir(&app);
    if dir.exists() {
        std::fs::remove_dir_all(dir).map_err(|e| e.to_string())?;
    }
    clear_cache();
    let _ = app.emit("extras-changed", ());
    Ok(())
}

// ---------- risorse per la pagina ----------

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Assets {
    name_tag: String,
    background: String,
    fonts: BTreeMap<String, String>,
    sprites: BTreeMap<String, String>,
}

fn data_uri(path: &Path) -> String {
    use base64::Engine;
    let mime = match path.extension().and_then(|e| e.to_str()).map(|e| e.to_ascii_lowercase()).as_deref() {
        Some("png") => "image/png",
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("webp") => "image/webp",
        Some("ttf") => "font/ttf",
        Some("otf") => "font/otf",
        Some("woff2") => "font/woff2",
        Some("woff") => "font/woff",
        _ => "application/octet-stream",
    };
    std::fs::read(path).map(|b| format!("data:{mime};base64,{}", base64::engine::general_purpose::STANDARD.encode(b))).unwrap_or_default()
}

#[tauri::command]
pub async fn extras_assets(app: AppHandle) -> Result<Assets, String> {
    let dir = pack_dir(&app);
    let pack = load(&dir).ok_or("pacchetto assente")?;
    tauri::async_runtime::spawn_blocking(move || {
        let uri = |rel: &str| inside(&dir, rel).map(|p| data_uri(&p)).unwrap_or_default();
        Assets {
            name_tag: if pack.name_tag.is_empty() { pack.name.clone() } else { pack.name_tag.clone() },
            background: uri(&pack.background),
            fonts: pack.fonts.iter().map(|(k, v)| (k.clone(), uri(v))).collect(),
            sprites: pack.sprites.iter().map(|(k, v)| (k.clone(), uri(v))).collect(),
        }
    })
    .await
    .map_err(|e| e.to_string())
}

// ---------- audio: Line, effetti, musica ----------

/// Line decodificate (campioni mono, rate), per non ridecodificarle a ogni pagina.
/// Campioni mono condivisi e frequenza.
type Clip = (std::sync::Arc<Vec<f32>>, u32);
static CACHE: Mutex<BTreeMap<String, Clip>> = Mutex::new(BTreeMap::new());

fn clear_cache() {
    CACHE.lock().unwrap_or_else(std::sync::PoisonError::into_inner).clear();
}

fn clip(dir: &Path, rel: &str) -> Option<Clip> {
    let mut c = CACHE.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    if let Some(v) = c.get(rel) {
        return Some(v.clone());
    }
    let (s, r) = crate::music::decode::decode_mono(&inside(dir, rel)?, 12.0).ok()?;
    let v = (std::sync::Arc::new(s), r);
    c.insert(rel.to_string(), v.clone());
    Some(v)
}

fn pick<'a>(pack: &'a Pack, mood: &str, avoid: &str) -> Option<&'a Line> {
    let mut cands: Vec<&Line> = pack.lines.iter().filter(|l| l.moods.iter().any(|m| m == mood)).collect();
    if cands.is_empty() && mood != "intro" && mood != "outro" {
        // umore senza Line: una qualsiasi che non sia di entrata/uscita
        cands = pack.lines.iter().filter(|l| !l.moods.iter().any(|m| m == "intro" || m == "outro")).collect();
    }
    if cands.len() > 1 {
        cands.retain(|l| l.file != avoid);
    }
    (!cands.is_empty()).then(|| cands[(seed() as usize) % cands.len()])
}

fn seed() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos() as u64 ^ (d.as_secs() << 17)).unwrap_or(7)
}

static LAST_LINE: Mutex<String> = Mutex::new(String::new());

/// Suona una Line dell'umore `mood` (o "intro"/"outro"). Restituisce i secondi.
#[tauri::command]
pub async fn extras_line(app: AppHandle, mood: String) -> f32 {
    let dir = pack_dir(&app);
    let Some(pack) = load(&dir) else { return 0.0 };
    let avoid = LAST_LINE.lock().unwrap_or_else(std::sync::PoisonError::into_inner).clone();
    let Some(line) = pick(&pack, &mood, &avoid).cloned() else { return 0.0 };
    *LAST_LINE.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = line.file.clone();
    let Ok(Some((s, r))) = tauri::async_runtime::spawn_blocking(move || clip(&dir, &line.file)).await else { return 0.0 };
    let engine = app.state::<AudioEngine>();
    engine.stop(); // una Line alla volta, come nel gioco
    engine.play(&s, r, Kind::Voice)
}

/// Effetto del pacchetto ("intro", "outro").
#[tauri::command]
pub async fn extras_sfx(app: AppHandle, name: String) {
    let dir = pack_dir(&app);
    let Some(rel) = load(&dir).and_then(|p| p.sfx.get(&name).cloned()) else { return };
    if let Ok(Some((s, r))) = tauri::async_runtime::spawn_blocking(move || clip(&dir, &rel)).await {
        app.state::<AudioEngine>().play(&s, r, Kind::Fx);
    }
}

static MUSIC_GEN: AtomicU64 = AtomicU64::new(0);
static MUSIC_TX: Mutex<Option<mpsc::Sender<crate::music::decode::Ctl>>> = Mutex::new(None);

/// Musica del pacchetto in loop (on) o sfumata e fermata (off). Se il lettore musicale sta
/// usando il canale, non si tocca niente.
#[tauri::command]
pub fn extras_music(app: AppHandle, on: bool) {
    let gen = MUSIC_GEN.fetch_add(1, Ordering::SeqCst) + 1;
    if !on {
        let Some(tx) = MUSIC_TX.lock().unwrap_or_else(std::sync::PoisonError::into_inner).take() else { return };
        thread::spawn(move || {
            let engine = app.state::<AudioEngine>();
            for i in (0..10).rev() {
                engine.set_music_gain(current_gain() * i as f32 / 10.0);
                thread::sleep(Duration::from_millis(60));
            }
            let _ = tx.send(crate::music::decode::Ctl::Stop);
            thread::sleep(Duration::from_millis(120));
            engine.music_clear();
            restore_gain(&app);
        });
        return;
    }
    if crate::music::player_active(&app) || MUSIC_TX.lock().unwrap_or_else(std::sync::PoisonError::into_inner).is_some() {
        return;
    }
    let dir = pack_dir(&app);
    let Some(m) = load(&dir).and_then(|p| p.music) else { return };
    let Some(path) = inside(&dir, &m.file) else { return };
    let (tx, rx) = mpsc::channel();
    *MUSIC_TX.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = Some(tx);
    let vol = if m.volume > 0.0 { m.volume.min(1.0) as f32 } else { 0.55 };
    *GAIN.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = vol;
    thread::spawn(move || {
        let engine = app.state::<AudioEngine>();
        engine.set_music_paused(false);
        engine.set_music_gain(vol);
        while MUSIC_GEN.load(Ordering::SeqCst) == gen {
            match crate::music::decode::play(&engine, &path, 0.0, &rx, &|_| {}) {
                Ok(crate::music::decode::End::Finished) => continue,
                _ => break,
            }
        }
    });
}

static GAIN: Mutex<f32> = Mutex::new(0.55);

fn current_gain() -> f32 {
    *GAIN.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Volume del lettore musicale com'era prima.
fn restore_gain(app: &AppHandle) {
    let v = app.state::<AppState>().cfg.lock().unwrap_or_else(std::sync::PoisonError::into_inner).music.volume;
    app.state::<AudioEngine>().set_music_gain(crate::music::gain(v));
}

// ---------- finestra ----------

/// Ingrandisce (on) o ripristina la finestra principale. Async: muove finestre.
#[tauri::command]
pub async fn extras_window(app: AppHandle, on: bool) {
    crate::window::takeover(&app, on);
}

// ---------- testo dal modello AI ----------

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Page {
    pub mood: String,
    pub text: String,
}

/// Cosa dirà l'ospite: screenshot (rimpicciolito) + gioco → poche pagine con l'umore.
/// Se il modello non risponde: frasi di riserva.
#[tauri::command]
pub async fn extras_prepare(app: AppHandle, game: Option<String>) -> Vec<Page> {
    let Some(pack) = installed(&app) else { return prompt::fallback(&[], crate::i18n::en()) };
    let moods = pack.moods();
    let en = crate::i18n::en();
    let game = game.or_else(|| games::detect().map(|g| g.to_string()));
    match ask(&app, &pack, game.as_deref(), en).await {
        Ok(p) if !p.is_empty() => p,
        Ok(_) => prompt::fallback(&moods, en),
        Err(e) => {
            crate::applog::warn("extras", &format!("testo di riserva: {e}"));
            prompt::fallback(&moods, en)
        }
    }
}

async fn ask(app: &AppHandle, pack: &Pack, game: Option<&str>, en: bool) -> Result<Vec<Page>, String> {
    let state = app.state::<AppState>();
    let mut cfg = state.cfg.lock().unwrap_or_else(std::sync::PoisonError::into_inner).ai.clone();
    // risposta breve, niente ragionamento: pochi token
    cfg.thinking = "off".into();
    cfg.max_tokens = 420;
    cfg.temperature = 1.0;
    cfg.top_p = 1.0;
    let vision = cfg.order.iter().any(|id| cfg.providers.get(id).is_some_and(|p| p.enabled && p.vision));
    let image = if vision {
        tauri::async_runtime::spawn_blocking(|| crate::actions::screen::small_base64(768)).await.ok().and_then(Result::ok)
    } else {
        None
    };
    let moods = pack.moods();
    let mut msg = crate::providers::ChatMessage::new("user", &prompt::user(game, image.is_some()));
    msg.image = image;
    let ai = &app.state::<Extras>().ai;
    let ch = tauri::ipc::Channel::new(|_| Ok(()));
    let res = crate::providers::chat(ai, cfg, prompt::system(&moods, en), vec![msg], None, ch).await?;
    Ok(prompt::parse(&res.text, &moods))
}

// ---------- quando succede ----------

pub struct Extras {
    pub ai: crate::providers::Ai,
}

impl Default for Extras {
    fn default() -> Self {
        Self { ai: crate::providers::Ai::with_kind("evento") }
    }
}

/// Fine dell'ultimo evento (secondi da UNIX_EPOCH) e richiesta in corso.
static LAST_END: AtomicU64 = AtomicU64::new(0);
static RUNNING: AtomicBool = AtomicBool::new(false);
/// Minuti minimi tra due eventi e minuti di gioco prima del primo.
const COOLDOWN_MIN: u64 = 25;
const FIRST_AFTER_MIN: u64 = 6;
/// Probabilità a ogni controllo (ogni 30 s) quando l'evento è "permesso": in media ~10 minuti.
const CHANCE: f64 = 0.05;

fn now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// Fa partire un evento (`manual`: scorciatoia di prova, ignora tempi e giochi).
pub fn trigger(app: &AppHandle, manual: bool) {
    if installed(app).is_none() {
        let _ = app.emit("app-say", crate::tr!("[worried]\nNon trovo i contenuti aggiuntivi: scaricali da Impostazioni → Generale.", "[worried]\nI can't find the extra content: download it from Settings → General."));
        return;
    }
    if RUNNING.swap(true, Ordering::SeqCst) {
        return;
    }
    let game = games::detect();
    let _ = app.emit_to(crate::window::MAIN, "extras-go", serde_json::json!({ "game": game, "manual": manual }));
    // se la pagina non risponde entro 5 minuti si riparte da zero
    thread::spawn(|| {
        thread::sleep(Duration::from_secs(300));
        RUNNING.store(false, Ordering::SeqCst);
    });
}

/// La pagina ha finito (`shown`: l'evento c'è stato; false = rifiutato perché occupata).
#[tauri::command]
pub fn extras_done(shown: bool) {
    if shown {
        LAST_END.store(now(), Ordering::SeqCst);
    }
    RUNNING.store(false, Ordering::SeqCst);
}

/// Controlla ogni 30 s se c'è un gioco riconosciuto e, ogni tanto, fa partire un evento.
pub fn spawn(app: AppHandle) {
    let _ = thread::Builder::new().name("extras".into()).spawn(move || {
        let mut playing_since: Option<u64> = None;
        let mut rng = seed();
        loop {
            thread::sleep(Duration::from_secs(30));
            let on = app.state::<AppState>().cfg.lock().unwrap_or_else(std::sync::PoisonError::into_inner).extras.events;
            if !on || RUNNING.load(Ordering::SeqCst) || installed(&app).is_none() {
                continue;
            }
            if games::detect().is_none() {
                playing_since = None;
                continue;
            }
            let t = now();
            let since = *playing_since.get_or_insert(t);
            if t - since < FIRST_AFTER_MIN * 60 || t.saturating_sub(LAST_END.load(Ordering::SeqCst)) < COOLDOWN_MIN * 60 {
                continue;
            }
            rng ^= rng << 13;
            rng ^= rng >> 7;
            rng ^= rng << 17;
            if (rng % 10_000) as f64 / 10_000.0 < CHANCE {
                trigger(&app, false);
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn links_and_paths() {
        assert_eq!(direct_link("https://www.dropbox.com/s/abc/asset.zip?dl=0"), "https://www.dropbox.com/s/abc/asset.zip?dl=1");
        assert_eq!(
            direct_link("https://drive.google.com/file/d/1AbC-d_E/view?usp=sharing"),
            "https://drive.usercontent.google.com/download?id=1AbC-d_E&export=download&confirm=t"
        );
        assert_eq!(direct_link(" https://example.com/a.zip "), "https://example.com/a.zip");
        let d = Path::new("/tmp/x");
        assert!(inside(d, "sprites/a.png").is_some());
        assert!(inside(d, "../a.png").is_none());
        assert!(inside(d, "/etc/passwd").is_none());
        assert!(inside(d, "").is_none());
    }

    fn zip_of(files: &[(&str, &[u8])]) -> Vec<u8> {
        use std::io::Write;
        let mut buf = std::io::Cursor::new(Vec::new());
        {
            let mut z = zip::ZipWriter::new(&mut buf);
            let opt = zip::write::SimpleFileOptions::default();
            for (n, b) in files {
                z.start_file(*n, opt).unwrap();
                z.write_all(b).unwrap();
            }
            z.finish().unwrap();
        }
        buf.into_inner()
    }

    #[test]
    fn extract_checks_the_pack() {
        let tmp = std::env::temp_dir().join(format!("ae_extras_{}", seed()));
        let json = br#"{"name":"T","sprites":{"idle":"sprites/idle.png"},"lines":[{"file":"lines/a.ogg","moods":["intro"]}]}"#;
        // dentro una cartella, con i file di macOS da ignorare
        let ok = zip_of(&[("Pack/pack.json", json), ("Pack/sprites/idle.png", b"x"), ("Pack/lines/a.ogg", b"y"), ("__MACOSX/Pack/._a.ogg", b"z")]);
        let p = extract(&ok, &tmp.join("a")).unwrap();
        assert_eq!(p.name, "T");
        assert!(tmp.join("a/sprites/idle.png").is_file());
        assert!(!tmp.join("a/__MACOSX").exists());
        // file citato mancante
        let missing = zip_of(&[("pack.json", json), ("sprites/idle.png", b"x")]);
        assert!(extract(&missing, &tmp.join("b")).unwrap_err().contains("lines/a.ogg"));
        // niente pack.json
        assert!(extract(&zip_of(&[("x.txt", b"x")]), &tmp.join("c")).is_err());
        assert!(extract(b"non zip", &tmp.join("d")).is_err());
        // percorsi fuori dalla cartella: ignorati
        let evil = zip_of(&[("pack.json", br#"{"sprites":{"idle":"i.png"}}"#), ("i.png", b"x"), ("../evil.txt", b"x")]);
        extract(&evil, &tmp.join("e")).unwrap();
        assert!(!tmp.join("evil.txt").exists());
        let _ = std::fs::remove_dir_all(tmp);
    }

    #[test]
    fn picks_lines_by_mood() {
        let mk = |f: &str, m: &[&str]| Line { file: f.into(), moods: m.iter().map(|s| s.to_string()).collect(), text: String::new() };
        let pack = Pack { lines: vec![mk("a", &["intro"]), mk("b", &["laugh"]), mk("c", &["laugh", "smug"]), mk("d", &["outro"])], ..Default::default() };
        assert_eq!(pick(&pack, "intro", "").unwrap().file, "a");
        assert_eq!(pick(&pack, "outro", "").unwrap().file, "d");
        assert_eq!(pick(&pack, "laugh", "b").unwrap().file, "c");
        // umore senza Line: una qualsiasi tranne entrata/uscita
        let f = pick(&pack, "angry", "").unwrap().file.clone();
        assert!(f == "b" || f == "c");
    }

    /// Il pacchetto vero, se presente in locale (AE_EXTRAS_ZIP=percorso).
    #[test]
    fn real_pack_if_present() {
        let Ok(p) = std::env::var("AE_EXTRAS_ZIP") else { return };
        let bytes = std::fs::read(p).unwrap();
        let tmp = std::env::temp_dir().join(format!("ae_extras_real_{}", seed()));
        let pack = extract(&bytes, &tmp).unwrap();
        assert!(pack.sprites.contains_key("idle") && pack.sprites.contains_key("leave"));
        assert!(pack.lines.iter().any(|l| l.moods.iter().any(|m| m == "intro")));
        for l in &pack.lines {
            let (s, r) = crate::music::decode::decode_mono(&tmp.join(&l.file), 12.0).unwrap();
            assert!(r > 8000 && s.len() > r as usize / 4, "{}", l.file);
        }
        let _ = std::fs::remove_dir_all(tmp);
    }
}
