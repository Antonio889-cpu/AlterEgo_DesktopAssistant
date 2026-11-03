//! Music Search and Play: Alter Ego cerca un brano su YouTube (yt-dlp), lo scarica in cache
//! e lo suona dal motore audio (stesso dispositivo di uscita, Mic-Link facoltativo).
//! Il lettore è una finestra a parte (player.html) con spettro, testo sincronizzato e
//! lo sprite di Alter Ego che cambia a tempo di musica.

mod analyze;
mod decode;
mod lyrics;
mod ytdlp;

use crate::{audio::AudioEngine, update_config, AppState};
use serde::Serialize;
use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicU64, Ordering},
        mpsc, Mutex,
    },
    time::{Duration, Instant},
};
use tauri::{AppHandle, Emitter, Manager};

pub const PLAYER: &str = "player";
const PLAYER_W: f64 = 600.0;
const PLAYER_H: f64 = 248.0;

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
}

impl Music {
    pub fn new(data_dir: &std::path::Path) -> Self {
        Self {
            inner: Mutex::new(Inner::default()),
            ctl: Mutex::new(None),
            gen: AtomicU64::new(0),
            data_dir: data_dir.to_path_buf(),
            busy: tokio::sync::Mutex::new(()),
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
    }
}

pub fn emit_state(app: &AppHandle) {
    let _ = app.emit_to(PLAYER, "music-state", state(app));
}

fn set_status(app: &AppHandle, s: &str) {
    app.state::<Music>().inner.lock().unwrap().status = s.to_string();
    emit_state(app);
}

// ---------- finestra del lettore ----------

fn open_player(app: &AppHandle) {
    if let Some(w) = app.get_webview_window(PLAYER) {
        if !w.is_visible().unwrap_or(true) {
            let _ = w.show();
        }
        return;
    }
    let pos = app.state::<AppState>().cfg.lock().unwrap().music.player_pos;
    let b = tauri::WebviewWindowBuilder::new(app, PLAYER, tauri::WebviewUrl::App("player.html".into()))
        .title("Alter Ego – Musica")
        .decorations(false)
        .transparent(true)
        .shadow(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(false)
        .focused(false)
        .visible(false)
        .inner_size(PLAYER_W, PLAYER_H);
    let Ok(w) = b.build() else { return };
    // posizione salvata, altrimenti in alto a destra del monitor principale
    let placed = pos.map(|(x, y)| w.set_position(tauri::PhysicalPosition::new(x, y)).is_ok()).unwrap_or(false);
    if !placed {
        if let Ok(Some(m)) = w.primary_monitor() {
            let a = m.work_area();
            // dimensione nota (outer_size prima di mostrare la finestra non è affidabile)
            let size = (PLAYER_W * m.scale_factor()) as i32;
            let margin = (24.0 * m.scale_factor()) as i32;
            let _ = w.set_position(tauri::PhysicalPosition::new(a.position.x + a.size.width as i32 - size - margin, a.position.y + margin));
        }
    }
    let _ = w.show();
}

/// Chiude il lettore con l'animazione "TV" (la finestra la esegue e poi chiama `player_closed`).
fn close_player(app: &AppHandle) {
    if let Some(w) = app.get_webview_window(PLAYER) {
        let _ = app.emit_to(PLAYER, "tv", "off");
        let w = w.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(1200));
            let _ = w.close(); // se la pagina non ha risposto
        });
    }
}

#[tauri::command]
pub fn player_closed(app: AppHandle) {
    if let Some(w) = app.get_webview_window(PLAYER) {
        let _ = w.close();
    }
}

/// La finestra del lettore è stata spostata: ricorda dove.
pub fn record_player_position(win: &tauri::Window) {
    if let Ok(p) = win.outer_position() {
        let app = win.app_handle();
        app.state::<AppState>().cfg.lock().unwrap().music.player_pos = Some((p.x, p.y));
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
        i.index = Some(index);
        i.base = start;
        i.playing = true;
        i.paused = false;
        i.status.clear();
        f
    };
    let engine = app.state::<AudioEngine>();
    engine.set_music_paused(false);
    let vol = app.state::<AppState>().cfg.lock().unwrap().music.volume;
    engine.set_music_gain(vol as f32);
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
                    set_status(&app, &format!("Errore: {e}"));
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
    close_player(app);
}

/// Cerca, scarica e suona (o accoda). Restituisce un messaggio per il modello.
pub async fn play(app: &AppHandle, query: &str, enqueue: bool) -> Result<String, String> {
    let query = query.trim();
    if query.is_empty() {
        return Err("cosa devo cercare?".into());
    }
    let m = app.state::<Music>();
    let _guard = m.busy.lock().await;
    open_player(app);
    set_status(app, &format!("Cerco «{query}»…"));
    let http = app.state::<AppState>().ai.http.clone();
    let app2 = app.clone();
    let result = async {
        let tools = ytdlp::ensure(&http, &m.data_dir, |s| set_status(&app2, s)).await?;
        let cache = m.data_dir.join("music_cache");
        set_status(app, &format!("Scarico «{query}» da YouTube…"));
        let r = match ytdlp::fetch(&tools, query, &cache).await {
            Ok(r) => r,
            Err(e) if cfg!(windows) && std::env::var_os("ALTEREGO_YTDLP").is_none() => {
                // YouTube cambia spesso: si aggiorna yt-dlp e si riprova una volta
                set_status(app, "Aggiorno yt-dlp e riprovo…");
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
            set_status(app, &format!("Non trovato: {e}"));
            if !m.inner.lock().unwrap().playing {
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
        let (title, artist) = (title.clone(), artist.clone());
        tauri::async_runtime::spawn(async move {
            let http = app.state::<AppState>().ai.http.clone();
            if let Some(lines) = lyrics::find(&http, &title, &artist, duration).await {
                let m = app.state::<Music>();
                for t in m.inner.lock().unwrap().queue.iter_mut().filter(|t| t.id == id) {
                    t.lyrics = Some(lines.clone());
                }
                emit_state(&app);
            }
        });
    }
    let s = state(app);
    let t = s.track.clone().unwrap_or_default();
    Ok(if now {
        format!("In riproduzione: «{}» di {} ({}).", t.title, t.artist, fmt_time(t.duration))
    } else {
        format!("Aggiunto in coda (posizione {}): «{title}» di {artist}.", index + 1)
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
    let need_track = || if s.playing { Ok(()) } else { Err("non sta suonando niente".to_string()) };
    match action {
        "pause" => {
            need_track()?;
            if !s.paused {
                toggle_pause(app);
            }
            Ok("In pausa.".into())
        }
        "resume" => {
            need_track()?;
            if s.paused {
                toggle_pause(app);
            }
            Ok("Riprendo.".into())
        }
        "next" => {
            need_track()?;
            advance(app, false);
            Ok(format!("Ora suona: {}", state(app).track.map(|t| t.title).unwrap_or_default()))
        }
        "previous" => {
            need_track()?;
            previous(app);
            Ok(format!("Ora suona: {}", state(app).track.map(|t| t.title).unwrap_or_default()))
        }
        "stop" => {
            stop(app);
            Ok("Musica fermata.".into())
        }
        "volume" => {
            let v = (value.unwrap_or(70.0) / 100.0).clamp(0.0, 1.0);
            set_volume(app, v);
            Ok(format!("Volume musica al {}%.", (v * 100.0).round()))
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
            _ => "Non sta suonando niente.".into(),
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
    app.state::<AudioEngine>().set_music_gain(v as f32);
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
pub fn music_toggle(app: AppHandle) {
    toggle_pause(&app);
}

#[tauri::command]
pub fn music_next(app: AppHandle) {
    advance(&app, false);
}

#[tauri::command]
pub fn music_prev(app: AppHandle) {
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
pub fn music_stop(app: AppHandle) {
    stop(&app);
}

/// Dalla finestra Impostazioni: prova (cerca e suona).
#[tauri::command]
pub async fn music_play(app: AppHandle, query: String) -> Result<String, String> {
    play(&app, &query, false).await
}

#[tauri::command]
pub fn music_settings(app: AppHandle, lyrics: bool, reactive_sprite: bool) {
    update_config(&app, |c| {
        c.music.lyrics = lyrics;
        c.music.reactive_sprite = reactive_sprite;
    });
    emit_state(&app);
}
