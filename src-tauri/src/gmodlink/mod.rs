//! Alter Ego gioca con te in Garry's Mod (sandbox) tramite l'addon "Alter Ego Link"
//! (src-tauri/gmod_addon, installato dall'app nella cartella addons del gioco).
//!
//! L'addon è solo corpo, sensi e mani; il cervello resta qui. Si parlano con due file in
//! `garrysmod/data/alterego/`: `state.json` (scritto dal gioco: stato compatto ed eventi) e
//! `cmd.json` (scritto da qui: comandi numerati + battito ogni 2 s).
//!
//! Token: il comportamento continuo (seguire, guardare, tenere oggetti) gira nel gioco; i comandi
//! semplici ("seguimi", "salta") sono parole chiave; il modello si chiama solo quando qualcuno le
//! parla o succede qualcosa di importante, con un prompt corto e fisso (in cache) e lo stato in una
//! riga. I compiti lunghi diventano piccoli script Lua che girano nel gioco senza altri token.

pub mod prompt;
pub mod quick;

use crate::{update_config, AppState};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::VecDeque,
    path::{Path, PathBuf},
    sync::Mutex,
    time::{Duration, Instant, SystemTime},
};
use tauri::{AppHandle, Emitter, Manager};

/// File dell'addon (cartella `addons/alterego_link`).
const ADDON: &[(&str, &str)] = &[
    ("addon.json", include_str!("../../gmod_addon/addon.json")),
    ("lua/alterego/sandbox.lua", include_str!("../../gmod_addon/lua/alterego/sandbox.lua")),
    ("lua/alterego/body_lambda.lua", include_str!("../../gmod_addon/lua/alterego/body_lambda.lua")),
    ("lua/autorun/server/sv_alterego_link.lua", include_str!("../../gmod_addon/lua/autorun/server/sv_alterego_link.lua")),
    ("lua/autorun/client/cl_alterego_link.lua", include_str!("../../gmod_addon/lua/autorun/client/cl_alterego_link.lua")),
];
const ADDON_DIR: &str = "alterego_link";

// ---------- stato del gioco (state.json) ----------

#[derive(Deserialize, Default, Debug, Clone)]
#[serde(default)]
pub struct GState {
    pub t: f64,
    pub map: String,
    pub seq: f64,
    pub events: Vec<Value>,
    pub tasks: Vec<String>,
    pub bot: Option<Bot>,
    pub players: Vec<Player>,
    pub near: Vec<Near>,
    /// Lambda Players è installato nel gioco.
    pub lambda: bool,
}

#[derive(Deserialize, Default, Debug, Clone)]
#[serde(default)]
pub struct Bot {
    /// "citizen" o "lambda".
    pub body: Option<String>,
    pub hp: f64,
    pub mode: String,
    pub weapon: Option<String>,
    pub held: Option<String>,
    pub following: Option<String>,
}

#[derive(Deserialize, Default, Debug, Clone)]
#[serde(default)]
pub struct Player {
    pub name: String,
    pub host: bool,
    pub hp: f64,
    pub d: Option<f64>,
    pub dir: Option<String>,
    pub looking: bool,
}

#[derive(Deserialize, Default, Debug, Clone)]
#[serde(default)]
pub struct Near {
    pub name: String,
    pub id: f64,
    pub d: f64,
    pub dir: Option<String>,
    pub hostile: Option<bool>,
}

// JSON vuoto di Lua: `{}` al posto di `[]`
fn lua_list<'de, D: serde::Deserializer<'de>, T: serde::de::DeserializeOwned>(d: D) -> Result<Vec<T>, D::Error> {
    let v = Value::deserialize(d)?;
    Ok(match v {
        Value::Array(a) => a.into_iter().filter_map(|x| serde_json::from_value(x).ok()).collect(),
        _ => Vec::new(),
    })
}

/// Legge state.json (le liste vuote di Lua arrivano come `{}`).
pub fn parse_state(raw: &str) -> Option<GState> {
    #[derive(Deserialize)]
    struct Raw {
        #[serde(default)]
        t: f64,
        #[serde(default)]
        map: String,
        #[serde(default)]
        seq: f64,
        #[serde(default, deserialize_with = "lua_list")]
        events: Vec<Value>,
        #[serde(default, deserialize_with = "lua_list")]
        tasks: Vec<String>,
        #[serde(default)]
        bot: Option<Bot>,
        #[serde(default, deserialize_with = "lua_list")]
        players: Vec<Player>,
        #[serde(default, deserialize_with = "lua_list")]
        near: Vec<Near>,
        #[serde(default)]
        lambda: bool,
    }
    let r: Raw = serde_json::from_str(raw).ok()?;
    Some(GState { t: r.t, map: r.map, seq: r.seq, events: r.events, tasks: r.tasks, bot: r.bot, players: r.players, near: r.near, lambda: r.lambda })
}

// ---------- stato dell'app ----------

pub struct Link {
    inner: Mutex<Inner>,
    pub ai: crate::providers::Ai,
    /// Una richiesta al modello alla volta.
    busy: tokio::sync::Mutex<()>,
}

#[derive(Default)]
struct Inner {
    dir: Option<PathBuf>,
    installed_for: Option<PathBuf>,
    cmds: VecDeque<Value>,
    next_id: u64,
    dirty: bool,
    last_write: Option<Instant>,
    state: Option<GState>,
    state_mtime: Option<SystemTime>,
    state_at: Option<Instant>,
    last_seq: Option<f64>,
    /// Scambi recenti (domanda compatta, risposta) per il contesto del modello.
    history: VecDeque<(String, String)>,
    /// Chat dei giocatori non rivolte a lei (contesto, senza chiamare il modello).
    overheard: VecDeque<String>,
    last_comment: Option<Instant>,
    /// A chi ha risposto per ultimo e quando (per capire se un messaggio è rivolto a lei).
    talking_with: Option<(String, Instant)>,
    game_running: bool,
}

impl Default for Link {
    fn default() -> Self {
        Self { inner: Mutex::new(Inner { next_id: first_id(), ..Default::default() }), ai: crate::providers::Ai::with_kind("gmod"), busy: tokio::sync::Mutex::new(()) }
    }
}

/// Id dei comandi: crescenti anche dopo un riavvio dell'app (l'addon ignora quelli già visti).
fn first_id() -> u64 {
    SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(1)
}

fn now_secs() -> u64 {
    SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

fn lock(app: &AppHandle) -> std::sync::MutexGuard<'_, Inner> {
    app.state::<Link>().inner().inner.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Alter Ego è nel gioco adesso (stato aggiornato da poco e corpo presente).
pub fn active(app: &AppHandle) -> bool {
    if !app.state::<AppState>().cfg.lock().unwrap_or_else(std::sync::PoisonError::into_inner).gmodlink.enabled {
        return false;
    }
    let i = lock(app);
    i.state_at.is_some_and(|t| t.elapsed() < Duration::from_secs(6)) && i.state.as_ref().is_some_and(|s| s.bot.is_some())
}

/// Il gioco è aperto con l'addon attivo (anche se lei non è ancora entrata).
pub fn linked(app: &AppHandle) -> bool {
    let i = lock(app);
    i.state_at.is_some_and(|t| t.elapsed() < Duration::from_secs(6))
}

/// Parole che fanno pensare al gioco (contro quelle "da PC" come musica o ricerche web).
const GAME_WORDS: &[&str] = &[
    "gioco", "game", "gmod", "garry", "mappa", "map", "spawn", "spara", "sparale", "sparagli", "uccidi", "prendi", "porta", "costruisci",
    "segui", "vieni", "salta", "lancia", "physgun", "veicolo", "macchina", "zombie", "gallina", "galline", "npc", "build", "shoot",
    "kill", "bring", "grab", "throw", "vehicle", "chicken", "task", "script",
];

/// La frase detta ad Alter Ego (push-to-talk o testo nell'app) va al cervello del gioco?
/// Le richieste "da PC" (musica, web, file, promemoria…) restano all'assistente normale.
pub fn wants(app: &AppHandle, text: &str) -> bool {
    let cfg = cfg_of(app);
    if !cfg.gmodlink.enabled || !linked(app) {
        return false;
    }
    let low = format!(" {} ", text.to_lowercase());
    let game = GAME_WORDS.iter().chain(prompt::ITEMS.iter()).any(|w| low.contains(w));
    let desktop = crate::saving::pick_groups(text).iter().any(|g| ["musica", "web", "file", "promemoria", "database", "grafo"].contains(g));
    if desktop && !game {
        return false;
    }
    if active(app) {
        return true;
    }
    // non è ancora nel gioco: solo "entra / vieni a giocare"
    quick::find(text, "", cfg.gmodlink.name.trim(), false, false).is_some()
}

/// Accoda comandi per l'addon (scritti al prossimo giro, ~0,25 s).
pub fn send(app: &AppHandle, ops: Vec<Value>) {
    if ops.is_empty() {
        return;
    }
    let mut i = lock(app);
    for mut op in ops {
        i.next_id += 1;
        op["id"] = json!(i.next_id);
        i.cmds.push_back(op);
    }
    while i.cmds.len() > 30 {
        i.cmds.pop_front();
    }
    i.dirty = true;
}

// ---------- cartella del gioco ----------

/// `.../GarrysMod/garrysmod` (dove stanno addons e data).
fn game_dir_from(p: &Path) -> Option<PathBuf> {
    let root = p.ancestors().find(|a| a.file_name().is_some_and(|n| n.to_string_lossy().eq_ignore_ascii_case("GarrysMod")))?;
    let g = root.join("garrysmod");
    g.is_dir().then_some(g)
}

/// Cartelle delle librerie Steam (dalla cartella predefinita e da libraryfolders.vdf).
fn steam_libraries() -> Vec<PathBuf> {
    let mut roots: Vec<PathBuf> = Vec::new();
    if cfg!(windows) {
        for p in ["C:\\Program Files (x86)\\Steam", "C:\\Program Files\\Steam"] {
            roots.push(PathBuf::from(p));
        }
    } else if let Some(h) = std::env::var_os("HOME").map(PathBuf::from) {
        roots.push(h.join(".steam/steam"));
        roots.push(h.join(".local/share/Steam"));
    }
    let mut libs = Vec::new();
    for r in roots {
        if let Ok(vdf) = std::fs::read_to_string(r.join("steamapps").join("libraryfolders.vdf")) {
            for line in vdf.lines() {
                let l = line.trim();
                if let Some(rest) = l.strip_prefix("\"path\"") {
                    let p = rest.trim().trim_matches('"').replace("\\\\", "\\");
                    libs.push(PathBuf::from(p));
                }
            }
        }
        libs.push(r);
    }
    libs.dedup();
    libs
}

/// Cartella `garrysmod`: impostazione, gioco aperto o librerie Steam.
pub fn find_game_dir(custom: &str) -> Option<PathBuf> {
    let c = custom.trim();
    if !c.is_empty() {
        let p = PathBuf::from(c);
        if p.join("addons").is_dir() {
            return Some(p);
        }
        if p.join("garrysmod").join("addons").is_dir() {
            return Some(p.join("garrysmod"));
        }
    }
    if let Some(g) = crate::apps::exe_path(&["gmod", "hl2"]).and_then(|p| game_dir_from(&p)) {
        return Some(g);
    }
    steam_libraries().into_iter().map(|l| l.join("steamapps/common/GarrysMod/garrysmod")).find(|p| p.join("addons").is_dir())
}

/// Scrive (o aggiorna) l'addon. true se ha cambiato qualcosa.
pub fn install(dir: &Path) -> Result<bool, String> {
    let base = dir.join("addons").join(ADDON_DIR);
    let mut changed = false;
    for (rel, content) in ADDON {
        let p = base.join(rel);
        if std::fs::read_to_string(&p).ok().as_deref() == Some(*content) {
            continue;
        }
        if let Some(parent) = p.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
        }
        std::fs::write(&p, content).map_err(|e| format!("{}: {e}", p.display()))?;
        changed = true;
    }
    std::fs::create_dir_all(dir.join("data").join("alterego")).map_err(|e| e.to_string())?;
    Ok(changed)
}

fn installed(dir: &Path) -> bool {
    dir.join("addons").join(ADDON_DIR).join("lua/autorun/server/sv_alterego_link.lua").is_file()
}

// ---------- ponte ----------

fn write_cmds(app: &AppHandle, dir: &Path) {
    let cfg = app.state::<AppState>().cfg.lock().unwrap_or_else(std::sync::PoisonError::into_inner).gmodlink.clone();
    let body = {
        let i = lock(app);
        json!({
            "t": now_secs(),
            "cmds": i.cmds.iter().collect::<Vec<_>>(),
            "config": {
                "model": cfg.model.trim(),
                "name": cfg.name.trim(),
                "auto_join": cfg.auto_join,
                "tasks": cfg.tasks,
                "body": cfg.body,
            },
        })
    };
    let d = dir.join("data").join("alterego");
    let _ = std::fs::create_dir_all(&d);
    let tmp = d.join("cmd.json.tmp");
    // scrittura atomica: il gioco non legge mai un file a metà
    if std::fs::write(&tmp, body.to_string()).is_ok() {
        let _ = std::fs::rename(&tmp, d.join("cmd.json"));
    }
}

/// Thread unico: ogni 0,25 s legge lo stato e scrive i comandi, solo con il gioco aperto.
pub fn spawn(app: AppHandle) {
    let _ = std::thread::Builder::new().name("gmodlink".into()).spawn(move || {
        let mut next_check = Instant::now();
        loop {
            std::thread::sleep(Duration::from_millis(250));
            let cfg = app.state::<AppState>().cfg.lock().unwrap_or_else(std::sync::PoisonError::into_inner).gmodlink.clone();
            if !cfg.enabled {
                std::thread::sleep(Duration::from_millis(750));
                continue;
            }
            if Instant::now() >= next_check {
                next_check = Instant::now() + Duration::from_secs(3);
                let running = crate::apps::exe_path(&["gmod", "hl2"]).is_some_and(|p| p.to_string_lossy().to_lowercase().contains("garrysmod"));
                let dir = if running { find_game_dir(&cfg.dir) } else { None };
                let mut i = lock(&app);
                if running != i.game_running {
                    crate::applog::info("gmodlink", if running { "Garry's Mod aperto" } else { "Garry's Mod chiuso" });
                }
                i.game_running = running;
                i.dir = dir.clone();
                if !running {
                    i.state = None;
                    i.state_at = None;
                    i.installed_for = None;
                }
                // addon sempre aggiornato (ha effetto al prossimo caricamento della mappa)
                if let Some(d) = dir.filter(|d| i.installed_for.as_ref() != Some(d)) {
                    i.installed_for = Some(d.clone());
                    drop(i);
                    match install(&d) {
                        Ok(true) => crate::applog::info("gmodlink", &format!("addon installato/aggiornato in {}", d.display())),
                        Ok(false) => {}
                        Err(e) => crate::applog::warn("gmodlink", &format!("installazione addon: {e}")),
                    }
                }
            }
            let Some(dir) = lock(&app).dir.clone() else { continue };
            // comandi + battito
            let write = {
                let i = lock(&app);
                i.dirty || i.last_write.is_none_or(|t| t.elapsed() > Duration::from_secs(2))
            };
            if write {
                write_cmds(&app, &dir);
                let mut i = lock(&app);
                i.dirty = false;
                i.last_write = Some(Instant::now());
            }
            // stato
            let path = dir.join("data").join("alterego").join("state.json");
            let Ok(mtime) = std::fs::metadata(&path).and_then(|m| m.modified()) else { continue };
            if lock(&app).state_mtime == Some(mtime) {
                continue;
            }
            let Some(s) = std::fs::read_to_string(&path).ok().and_then(|r| parse_state(&r)) else { continue };
            // file vecchio di una partita precedente
            if now_secs() as f64 - s.t > 10.0 {
                lock(&app).state_mtime = Some(mtime);
                continue;
            }
            let events = {
                let mut i = lock(&app);
                i.state_mtime = Some(mtime);
                i.state_at = Some(Instant::now());
                let last = match i.last_seq {
                    // nuova mappa: la numerazione riparte
                    Some(l) if s.seq >= l => l,
                    Some(_) => 0.0,
                    // primo stato dopo l'avvio dell'app: gli eventi vecchi non contano
                    None => s.seq,
                };
                i.last_seq = Some(s.seq);
                let ev: Vec<Value> = s.events.iter().filter(|e| e["seq"].as_f64().unwrap_or(0.0) > last).cloned().collect();
                i.state = Some(s);
                ev
            };
            for e in events {
                on_event(&app, e);
            }
        }
    });
}

// ---------- eventi dal gioco ----------

fn cfg_of(app: &AppHandle) -> crate::config::Config {
    app.state::<AppState>().cfg.lock().unwrap_or_else(std::sync::PoisonError::into_inner).clone()
}

fn en(app: &AppHandle) -> bool {
    crate::providers::prompt::reply_lang(&cfg_of(app)) == "en"
}

/// Parla: riga nella chat del gioco + voce e box di Alter Ego sul PC.
fn speak(app: &AppHandle, emotion: &str, text: &str) {
    if text.trim().is_empty() {
        return;
    }
    send(app, vec![json!({ "op": "say", "text": text })]);
    let _ = app.emit("app-say", format!("[{emotion}]\n{text}"));
}

/// Il messaggio è rivolto a lei?
fn addressed(app: &AppHandle, from: &str, is_host: bool, text: &str) -> bool {
    let t = text.trim();
    if t.starts_with('!') || t.starts_with('/') {
        return false;
    }
    let cfg = cfg_of(app);
    let name = cfg.gmodlink.name.trim().to_lowercase();
    let low = t.to_lowercase();
    if ["alter", "ego", "chihiro"].iter().any(|w| low.contains(w)) || (!name.is_empty() && low.contains(&name)) {
        return true;
    }
    let i = lock(app);
    let players = i.state.as_ref().map(|s| s.players.len()).unwrap_or(1);
    let recent = i.talking_with.as_ref().is_some_and(|(w, at)| w == from && at.elapsed() < Duration::from_secs(if is_host { 90 } else { 40 }));
    // da solo con lei: tutto quello che scrive chi ospita è per lei
    (is_host && players <= 1) || recent
}

fn on_event(app: &AppHandle, e: Value) {
    let kind = e["kind"].as_str().unwrap_or("");
    let s = |k: &str| e[k].as_str().unwrap_or("").to_string();
    match kind {
        "chat" => {
            let (from, text, is_host) = (s("from"), s("text"), e["host"].as_bool().unwrap_or(false));
            if !addressed(app, &from, is_host, &text) {
                let mut i = lock(app);
                i.overheard.push_back(format!("{from}: {}", text.chars().take(160).collect::<String>()));
                while i.overheard.len() > 6 {
                    i.overheard.pop_front();
                }
                return;
            }
            let app = app.clone();
            tauri::async_runtime::spawn(async move {
                match handle(&app, &from, &text, None).await {
                    Ok(r) => {
                        let _ = app.emit("app-say", r);
                    }
                    Err(err) => crate::applog::warn("gmodlink", &err),
                }
            });
        }
        "say" => {
            // frase scritta da un task: niente modello
            speak(app, "happy", &s("text"));
        }
        "notify" => comment(app, &format!("[your task {} says] {}", s("task"), s("text")), true),
        "task" => {
            let status = s("status");
            if status == "error" {
                let err = s("error");
                crate::applog::warn("gmodlink", &format!("task {}: {err}", s("name")));
                let line = if en(app) {
                    format!("My script \"{}\" doesn't work… ({})", s("name"), err.chars().take(80).collect::<String>())
                } else {
                    format!("Il mio script «{}» non funziona… ({})", s("name"), err.chars().take(80).collect::<String>())
                };
                speak(app, "embarrassed", &line);
                remember(app, &format!("[task {} failed: {err}]", s("name")), "");
            } else if status == "done" {
                remember(app, &format!("[task {} finished]", s("name")), "");
            }
        }
        "hurt" => comment(app, &format!("[event] {} hit you ({} damage)", s("by"), e["dmg"]), false),
        "died" => comment(app, &format!("[event] you were killed by {}; you'll respawn in a moment", s("by")), false),
        "player_died" => comment(app, &format!("[event] {} died ({})", s("who"), s("by")), false),
        // scelti nel menu del gioco (C → Alter Ego): si salvano nelle impostazioni
        "model" => {
            let m = s("model");
            update_config(app, |c| c.gmodlink.model = m);
            lock(app).dirty = true;
        }
        "body" => {
            let b = s("body");
            update_config(app, |c| {
                c.gmodlink.body = b;
                c.gmodlink.normalize();
            });
        }
        "name" => {
            let n = s("name");
            if !n.trim().is_empty() {
                update_config(app, |c| {
                    c.gmodlink.name = n;
                    c.gmodlink.normalize();
                });
            }
        }
        "joined" | "left" => {
            let _ = app.emit("gmod-link", kind);
        }
        _ => {}
    }
}

/// Aggiunge una riga al contesto senza chiamare il modello.
fn remember(app: &AppHandle, user: &str, reply: &str) {
    let mut i = lock(app);
    i.history.push_back((user.to_string(), reply.to_string()));
    while i.history.len() > 6 {
        i.history.pop_front();
    }
}

/// Commento spontaneo su un evento, se la "chiacchiera" lo permette (`force`: richiesto da un task).
fn comment(app: &AppHandle, what: &str, force: bool) {
    let cfg = cfg_of(app);
    let gap = match cfg.gmodlink.chatter.as_str() {
        "off" if !force => return,
        "often" => 45,
        "normal" => 90,
        _ => 240,
    };
    {
        let mut i = lock(app);
        if !force && i.last_comment.is_some_and(|t| t.elapsed() < Duration::from_secs(gap)) {
            return;
        }
        i.last_comment = Some(Instant::now());
    }
    let app = app.clone();
    let what = what.to_string();
    tauri::async_runtime::spawn(async move {
        match handle(&app, "", "", Some(&what)).await {
            Ok(r) => {
                let _ = app.emit("app-say", r);
            }
            Err(err) => crate::applog::warn("gmodlink", &err),
        }
    });
}

/// Risponde a `text` detto da `from` (o reagisce a `event`). Restituisce "[emozione]\ntesto"
/// (per la voce e il box sul PC); la riga va anche nella chat del gioco.
pub async fn handle(app: &AppHandle, from: &str, text: &str, event: Option<&str>) -> Result<String, String> {
    let cfg = cfg_of(app);
    let en = crate::providers::prompt::reply_lang(&cfg) == "en";
    let host_name = {
        let i = lock(app);
        i.state.as_ref().and_then(|s| s.players.iter().find(|p| p.host).map(|p| p.name.clone())).unwrap_or_default()
    };
    let from = if from.is_empty() && event.is_none() { host_name.clone() } else { from.to_string() };
    // 1. comandi semplici: zero token
    if event.is_none() {
        let in_game = lock(app).state.as_ref().is_some_and(|s| s.bot.is_some());
        if let Some(q) = quick::find(text, &from, cfg.gmodlink.name.trim(), in_game, en) {
            send(app, q.ops);
            send(app, vec![json!({ "op": "say", "text": q.reply })]);
            remember(app, &format!("{from}: {text}"), &q.reply);
            lock(app).talking_with = Some((from.clone(), Instant::now()));
            return Ok(format!("[{}]\n{}", q.emotion, q.reply));
        }
    }
    // 2. il modello (una richiesta alla volta)
    let link = app.state::<Link>();
    let _guard = link.busy.lock().await;
    let (state_line, history, overheard) = {
        let mut i = lock(app);
        let st = i.state.as_ref().map(prompt::state_line).unwrap_or_else(|| "[state] unknown".into());
        let h: Vec<(String, String)> = i.history.iter().cloned().collect();
        let o: Vec<String> = i.overheard.drain(..).collect();
        (st, h, o)
    };
    let mut user = state_line;
    if !overheard.is_empty() {
        user.push_str(&format!("\n[chat] {}", overheard.join(" / ")));
    }
    let line = match event {
        Some(ev) => ev.to_string(),
        None => format!("{from}: {}", text.chars().take(400).collect::<String>()),
    };
    user.push('\n');
    user.push_str(&line);
    // cronologia corta: solo le righe di chat (lo stato vale solo per adesso)
    let mut msgs = Vec::new();
    for (u, a) in &history {
        msgs.push(crate::providers::ChatMessage::new("user", u));
        if !a.is_empty() {
            msgs.push(crate::providers::ChatMessage::new("assistant", a));
        }
    }
    // la cronologia deve alternare user/assistant: le righe senza risposta si uniscono
    let mut merged: Vec<crate::providers::ChatMessage> = Vec::new();
    for m in msgs {
        match merged.last_mut() {
            Some(last) if last.role == m.role => {
                last.content.push('\n');
                last.content.push_str(&m.content);
            }
            _ => merged.push(m),
        }
    }
    if let Some(last) = merged.pop_if(|m| m.role == "user") {
        user = format!("{}\n{user}", last.content);
    }
    if merged.first().is_some_and(|m| m.role == "assistant") {
        merged.remove(0);
    }
    merged.push(crate::providers::ChatMessage::new("user", &user));
    let mut ai = cfg.ai.clone();
    ai.thinking = "off".into();
    ai.max_tokens = 700;
    let system = prompt::system(crate::providers::prompt::language_rule(&cfg), cfg.user_name.trim(), cfg.gmodlink.tasks);
    let ch = tauri::ipc::Channel::new(|_| Ok(()));
    let res = crate::providers::chat(&link.ai, ai, system, merged, None, ch).await?;
    let r = prompt::parse(&res.text);
    crate::applog::info("gmodlink", &format!("risposta: {} azioni, {} caratteri", r.ops.len(), r.text.chars().count()));
    // niente script se disattivati nelle impostazioni
    let ops: Vec<Value> = r.ops.into_iter().filter(|o| cfg.gmodlink.tasks || o["op"] != "task").collect();
    send(app, ops.clone());
    if !r.text.is_empty() {
        send(app, vec![json!({ "op": "say", "text": r.text })]);
    }
    // in memoria solo il testo e i nomi dei task (il codice no: costa token)
    let mut reply_mem = format!("[{}]\n{}", r.emotion, r.text);
    for o in &ops {
        match o["op"].as_str() {
            Some("task") => reply_mem.push_str(&format!("\n[task {}]", o["name"].as_str().unwrap_or(""))),
            Some(op) => reply_mem.push_str(&format!("\n[act] {op}")),
            None => {}
        }
    }
    remember(app, &line, &reply_mem);
    if !from.is_empty() {
        lock(app).talking_with = Some((from, Instant::now()));
    }
    Ok(format!("[{}]\n{}", r.emotion, if r.text.is_empty() { "…".into() } else { r.text }))
}

// ---------- comandi per le impostazioni ----------

#[derive(Serialize)]
pub struct Status {
    /// Cartella `garrysmod` trovata.
    pub dir: Option<String>,
    pub installed: bool,
    pub game_running: bool,
    /// Il gioco risponde (addon caricato in una mappa).
    pub linked: bool,
    /// Alter Ego è nel gioco.
    pub in_game: bool,
    /// Lambda Players installato (None = non si sa: gioco non collegato).
    pub lambda: Option<bool>,
    /// Corpo in uso ("citizen" / "lambda").
    pub body: Option<String>,
}

#[tauri::command]
pub async fn gmodlink_status(app: AppHandle) -> Status {
    let custom = cfg_of(&app).gmodlink.dir.clone();
    let dir = tauri::async_runtime::spawn_blocking(move || find_game_dir(&custom)).await.ok().flatten();
    let i = lock(&app);
    let fresh = i.state_at.is_some_and(|t| t.elapsed() < Duration::from_secs(6));
    Status {
        installed: dir.as_deref().is_some_and(installed),
        dir: dir.map(|d| d.display().to_string()),
        game_running: i.game_running,
        linked: fresh,
        in_game: fresh && i.state.as_ref().is_some_and(|s| s.bot.is_some()),
        lambda: i.state.as_ref().map(|s| s.lambda),
        body: i.state.as_ref().and_then(|s| s.bot.as_ref()).and_then(|b| b.body.clone()),
    }
}

#[tauri::command]
pub async fn gmodlink_install(app: AppHandle) -> Result<String, String> {
    let custom = cfg_of(&app).gmodlink.dir.clone();
    let dir = tauri::async_runtime::spawn_blocking(move || find_game_dir(&custom))
        .await
        .ok()
        .flatten()
        .ok_or_else(|| crate::i18n::tr("cartella di Garry's Mod non trovata: indicala qui sotto", "Garry's Mod folder not found: enter it below").to_string())?;
    install(&dir)?;
    lock(&app).installed_for = Some(dir.clone());
    Ok(dir.display().to_string())
}

#[tauri::command]
pub fn gmodlink_settings(app: AppHandle, settings: crate::config::GmodLinkConfig) {
    update_config(&app, |c| {
        c.gmodlink = settings;
        c.gmodlink.normalize();
    });
    lock(&app).dirty = true;
}

/// Entra / esce dal gioco (pulsanti nelle impostazioni).
#[tauri::command]
pub fn gmodlink_join(app: AppHandle, join: bool) {
    send(&app, vec![json!({ "op": if join { "join" } else { "leave" } })]);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn state_with_lua_empty_tables() {
        // Lua scrive {} per le liste vuote
        let s = parse_state(r#"{"t":5,"seq":2,"events":{},"tasks":{},"players":[{"name":"Makoto","host":true,"hp":100}],"near":{}}"#).unwrap();
        assert_eq!((s.seq, s.events.len(), s.players.len(), s.near.len()), (2.0, 0, 1, 0));
        assert!(s.bot.is_none());
        // GMod può scrivere i numeri come decimali
        let s = parse_state(r#"{"t":5.0,"seq":4.0,"events":[{"seq":4.0,"kind":"chat","from":"A","text":"ciao"}],"bot":{"hp":80.0,"mode":"stay"}}"#).unwrap();
        assert_eq!(s.events[0]["kind"], "chat");
        assert_eq!(s.bot.unwrap().mode, "stay");
    }

    /// Stato scritto dall'addon vero (generato da gmod_addon/test/run_tests.py).
    #[test]
    fn parses_real_addon_state() {
        let s = parse_state(include_str!("../../gmod_addon/test/sample_state.json")).unwrap();
        assert!(s.bot.is_some() && !s.players.is_empty() && !s.near.is_empty());
        let line = prompt::state_line(&s);
        assert!(line.starts_with("[state] you 100hp follow Makoto") && line.contains("Makoto (host)") && line.contains("near: "), "{line}");
        assert!(s.events.iter().any(|e| e["kind"] == "joined"));
    }

    #[test]
    fn installs_addon_files() {
        let dir = std::env::temp_dir().join(format!("ae-gmod-{}", std::process::id()));
        let g = dir.join("GarrysMod").join("garrysmod");
        std::fs::create_dir_all(g.join("addons")).unwrap();
        assert_eq!(find_game_dir(&dir.join("GarrysMod").to_string_lossy()), Some(g.clone()));
        assert!(install(&g).unwrap());
        assert!(installed(&g));
        assert!(!install(&g).unwrap(), "già aggiornato: niente da scrivere");
        assert!(g.join("data/alterego").is_dir());
        assert_eq!(game_dir_from(&dir.join("GarrysMod/bin/win64/gmod.exe")), Some(g));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
