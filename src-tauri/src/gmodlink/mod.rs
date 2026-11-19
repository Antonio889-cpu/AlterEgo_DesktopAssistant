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

pub mod fly;
pub mod memory;
mod mind;
pub mod persona;
pub mod prompt;
pub mod quick;
pub mod quips;

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
    ("lua/alterego/fly.lua", include_str!("../../gmod_addon/lua/alterego/fly.lua")),
    ("lua/alterego/human.lua", include_str!("../../gmod_addon/lua/alterego/human.lua")),
    ("lua/alterego/sit.lua", include_str!("../../gmod_addon/lua/alterego/sit.lua")),
    ("lua/alterego/play.lua", include_str!("../../gmod_addon/lua/alterego/play.lua")),
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
    /// Giocatori che la bersagliano (punteggio di rivalità, solo ≥ 1).
    pub rivals: std::collections::HashMap<String, f64>,
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
    /// Dove è seduta ("ground", "jeep#12"…).
    pub sitting: Option<String>,
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
    /// Sta parlando in voce adesso (lei aspetta il suo turno).
    pub talking: bool,
    /// false = lei non lo vede né lo sente (sa solo che è in partita).
    pub seen: Option<bool>,
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

// tabella di Lua che può arrivare vuota come `[]`
fn lua_map<'de, D: serde::Deserializer<'de>>(d: D) -> Result<std::collections::HashMap<String, f64>, D::Error> {
    let v = Value::deserialize(d)?;
    Ok(match v {
        Value::Object(o) => o.into_iter().filter_map(|(k, x)| x.as_f64().map(|f| (k, f))).collect(),
        _ => Default::default(),
    })
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
        #[serde(default, deserialize_with = "lua_map")]
        rivals: std::collections::HashMap<String, f64>,
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
    Some(GState { t: r.t, map: r.map, seq: r.seq, events: r.events, tasks: r.tasks, rivals: r.rivals, bot: r.bot, players: r.players, near: r.near, lambda: r.lambda })
}

// ---------- stato dell'app ----------

pub struct Link {
    inner: Mutex<Inner>,
    /// Cervello di moscerino (a parte: la simulazione non blocca il resto).
    fly: Mutex<FlyState>,
    pub ai: crate::providers::Ai,
    /// Una richiesta al modello alla volta.
    busy: tokio::sync::Mutex<()>,
    /// Personalità, scherzi in prova, obiettivo lungo.
    mind: Mutex<mind::Mind>,
    /// Battute brevi generate dal modello (per l'addon).
    quips: Mutex<quips::Quips>,
    /// Client a parte per le battute: non interrompe le risposte in corso.
    quip_ai: crate::providers::Ai,
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
    /// Partita in corso (per la memoria delle partite).
    session: Option<memory::Log>,
}

impl Default for Link {
    fn default() -> Self {
        Self {
            inner: Mutex::new(Inner { next_id: first_id(), ..Default::default() }),
            fly: Mutex::new(FlyState::default()),
            ai: crate::providers::Ai::with_kind("gmod"),
            busy: tokio::sync::Mutex::new(()),
            mind: Mutex::new(mind::Mind::default()),
            quips: Mutex::new(quips::Quips::default()),
            quip_ai: crate::providers::Ai::with_kind("gmod"),
        }
    }
}

#[derive(Default)]
struct FlyState {
    brain: Option<Box<fly::Brain>>,
    mood: fly::Mood,
    senses: std::collections::HashMap<String, f32>,
    sense_mtime: Option<SystemTime>,
    sense_at: Option<Instant>,
    last_tick: Option<Instant>,
    out: Vec<(String, f32)>,
    /// Spike al secondo (attività mostrata sopra la testa).
    activity: f32,
}

fn fly_lock(app: &AppHandle) -> std::sync::MutexGuard<'_, FlyState> {
    app.state::<Link>().inner().fly.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Nomi dei sensi in uso: predefiniti + gruppi opzionali scelti.
fn fly_senses(cfg: &crate::config::FlyConfig) -> Vec<&'static str> {
    let mut v: Vec<&'static str> = fly::DEFAULT_SENSES.to_vec();
    for (g, names) in fly::OPTIONAL {
        if cfg.senses.iter().any(|s| s == g) {
            v.extend_from_slice(names);
        }
    }
    v
}

/// Un giro del cervello: legge i sensi dal gioco, simula il tempo passato, scrive le uscite.
fn fly_tick(app: &AppHandle, dir: &Path, cfg: &crate::config::FlyConfig) {
    let d = dir.join("data").join("alterego");
    let mut f = fly_lock(app);
    if f.brain.is_none() {
        match fly::Brain::load() {
            Some(b) => {
                crate::applog::info("gmodlink", &format!("cervello di moscerino: {} neuroni, {} sinapsi", b.neurons(), b.synapses()));
                f.brain = Some(Box::new(b));
            }
            None => return,
        }
    }
    let path = d.join("sense.json");
    if let Ok(mtime) = std::fs::metadata(&path).and_then(|m| m.modified()) {
        if f.sense_mtime != Some(mtime) {
            f.sense_mtime = Some(mtime);
            if let Some(v) = std::fs::read_to_string(&path).ok().and_then(|r| serde_json::from_str::<Value>(&r).ok()) {
                let allowed = fly_senses(cfg);
                f.senses = v["s"]
                    .as_object()
                    .map(|o| {
                        o.iter()
                            .filter(|(k, _)| allowed.contains(&k.as_str()))
                            .filter_map(|(k, x)| x.as_f64().map(|x| (k.clone(), (x as f32).clamp(0.0, 1.0))))
                            .collect()
                    })
                    .unwrap_or_default();
                f.sense_at = Some(Instant::now());
            }
        }
    }
    // sensi vecchi (gioco in pausa, lei fuori): si spengono
    if f.sense_at.is_none_or(|t| t.elapsed() > Duration::from_millis(1500)) {
        f.senses.clear();
    }
    let ms = f.last_tick.map(|t| t.elapsed().as_secs_f32() * 1000.0).unwrap_or(100.0).clamp(10.0, 300.0);
    f.last_tick = Some(Instant::now());
    let senses = f.senses.clone();
    let FlyState { brain, mood, out, activity, .. } = &mut *f;
    let Some(b) = brain.as_mut() else { return };
    b.run(ms, &senses);
    *out = b.read(ms);
    fly::gate(out, &senses);
    *activity = b.take_total() as f32 / (ms / 1000.0);
    mood.update(ms / 1000.0, &senses, out);
    let body = json!({
        "t": now_secs(),
        "o": out.iter().map(|(k, v)| (k.clone(), json!(v))).collect::<serde_json::Map<_, _>>(),
        "a": activity.round(),
    });
    drop(f);
    let tmp = d.join("fly.json.tmp");
    if std::fs::write(&tmp, body.to_string()).is_ok() {
        let _ = std::fs::rename(&tmp, d.join("fly.json"));
    }
}

fn data_dir(app: &AppHandle) -> PathBuf {
    app.state::<crate::assistant::Conversation>().data_dir.clone()
}

/// Una partita finita diventa un nodo "game" della memoria principale (memoria a grafo), collegato
/// ai giocatori e alle cose nominate. Usata anche dalle altre integrazioni di gioco.
pub fn game_to_graph(app: &AppHandle, game: &str, start: i64, place: &str, line: &str) {
    let conv = app.state::<crate::assistant::Conversation>();
    let Some(m) = conv.memory.as_ref() else { return };
    let title = if place.is_empty() { format!("{game} {start}") } else { format!("{game}: {place} ({start})") };
    if let Err(e) = m.remember_node(crate::memory::graph::GAME, &title, &format!("{game}: {line}")) {
        crate::applog::warn("memoria", &format!("partita non salvata nel grafo: {e}"));
    }
}

/// Fine di una sessione di Minecraft o VRChat: le ultime battute diventano un nodo del grafo
/// (zero token: niente riassunto del modello, solo le frasi tagliate).
pub fn session_to_graph(app: &AppHandle, game: &str, place: &str, history: &[(String, String)]) {
    let line = session_line(history);
    if !line.is_empty() {
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0);
        game_to_graph(app, game, now, place, &line);
    }
}

fn session_line(history: &[(String, String)]) -> String {
    let cut = |t: &str, n: usize| t.chars().take(n).collect::<String>();
    let mut line = history.iter().filter(|(u, _)| !u.trim().is_empty()).map(|(u, a)| format!("«{}» → «{}»", cut(u.trim(), 90), cut(a.trim(), 90))).collect::<Vec<_>>().join(" · ");
    if line.chars().count() > 480 {
        line = cut(&line, 480) + "…";
    }
    line
}

/// Chiude la partita in corso e la mette nell'archivio delle partite.
fn end_session(app: &AppHandle) {
    let log = lock(app).session.take();
    if let Some(log) = log {
        if cfg_of(app).gmodlink.memory && log.finish(&data_dir(app), now_secs() as i64) {
            crate::applog::info("gmodlink", "partita salvata nella memoria delle partite");
            // anche nella memoria a grafo: in chat "ti ricordi quella partita?" la ritrova
            if let Some(last) = memory::load(&data_dir(app)).sessions.last() {
                game_to_graph(app, "Garry's Mod", last.start, &last.map, &last.line);
            }
        }
    }
}

/// "stress 0.6, fled" per la riga di stato (vuoto se calma o spento).
fn instinct(app: &AppHandle, cfg: &crate::config::GmodLinkConfig) -> String {
    if !cfg.fly.enabled || !cfg.fly.mood {
        return String::new();
    }
    fly_lock(app).mood.line()
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

fn quips_lock(app: &AppHandle) -> std::sync::MutexGuard<'_, quips::Quips> {
    app.state::<Link>().inner().quips.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Chiede al modello un pacchetto di battute brevi, se serve (`low`: una categoria sta finendo).
fn ask_quips(app: &AppHandle, low: bool) {
    let cfg = cfg_of(app);
    let key = format!("{}/{}", crate::providers::prompt::reply_lang(&cfg), cfg.personality.pronouns());
    {
        let mut q = quips_lock(app);
        if !q.due(&key, low) {
            return;
        }
        q.busy = true;
        q.asked = Some(Instant::now());
    }
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let mut ai = cfg.ai.clone();
        ai.thinking = "off".into();
        ai.max_tokens = 900;
        let system = quips::system(crate::providers::prompt::pronoun_rule(&cfg), crate::providers::prompt::language_rule(&cfg));
        let msgs = vec![crate::providers::ChatMessage::new("user", &quips::user())];
        let ch = tauri::ipc::Channel::new(|_| Ok(()));
        let link = app.state::<Link>();
        let res = crate::providers::chat(&link.quip_ai, ai, system, msgs, None, ch).await;
        let mut q = quips_lock(&app);
        q.busy = false;
        match res.map(|r| quips::parse(&r.text)) {
            Ok(Some(b)) => {
                let n: usize = b.values().map(Vec::len).sum();
                q.store(&key, b);
                drop(q);
                lock(&app).dirty = true;
                crate::applog::info("gmodlink", &format!("battute nuove: {n} righe"));
            }
            Ok(None) => crate::applog::warn("gmodlink", "battute: risposta del modello non leggibile (restano quelle di riserva)"),
            Err(e) => crate::applog::warn("gmodlink", &format!("battute: {e}")),
        }
    });
}

/// Aspetta il suo turno: non parla sopra a chi sta parlando in voce (al massimo ~4 s).
async fn wait_turn(app: &AppHandle) {
    let mut waited = false;
    for _ in 0..28 {
        let busy = lock(app).state.as_ref().is_some_and(|s| s.players.iter().any(|p| p.talking));
        if !busy {
            break;
        }
        waited = true;
        tokio::time::sleep(Duration::from_millis(150)).await;
    }
    if waited {
        // una piccola pausa dopo che l'altro ha finito, come una persona
        tokio::time::sleep(Duration::from_millis(350)).await;
    }
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
        if op["op"] == "spawn" {
            if let (Some(log), Some(w)) = (i.session.as_mut(), op["what"].as_str()) {
                log.spawned(w);
            }
        }
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
    let full = cfg_of(app);
    let lang = crate::providers::prompt::reply_lang(&full).to_string();
    let pronouns = full.personality.pronouns();
    let cfg = full.gmodlink;
    let skills = mind::skills(app);
    let (quips, quips_v) = {
        let q = quips_lock(app);
        (q.lines.clone(), q.v)
    };
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
                "play": {
                    "mischief": cfg.mischief,
                    "idle": cfg.idle_play,
                    "lang": lang,
                    "chaos": cfg.unpredictability as f64 / 100.0,
                    "tough": cfg.toughness,
                    "pronouns": pronouns,
                    "skills": skills,
                    "quips": quips,
                    "quips_v": quips_v,
                },
                "fly": {
                    "on": cfg.fly.enabled,
                    "reflexes": cfg.fly.reflexes,
                    "hud": cfg.fly.hud,
                    "senses": if cfg.fly.enabled { fly_senses(&cfg.fly) } else { Vec::new() },
                },
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
            let fly_on = {
                let g = cfg_of(&app).gmodlink;
                g.enabled && g.fly.enabled
            };
            // col cervello di moscerino acceso: 10 giri al secondo (riflessi pronti)
            std::thread::sleep(Duration::from_millis(if fly_on && active(&app) { 100 } else { 250 }));
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
                // gioco chiuso o fermo da 2 minuti: la partita è finita
                let stale = i.state_at.is_none_or(|t| t.elapsed() > Duration::from_secs(120));
                let ended = i.session.is_some() && (!running || stale);
                drop(i);
                if ended {
                    end_session(&app);
                }
                let mut i = lock(&app);
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
            if cfg.fly.enabled && active(&app) {
                fly_tick(&app, &dir, &cfg.fly);
            }
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
                // partita: nuova mappa = nuova partita
                let map_changed = i.session.as_ref().is_some_and(|l| l.map != s.map);
                if map_changed {
                    if let Some(old) = i.session.take() {
                        if cfg.memory {
                            old.finish(&data_dir(&app), now_secs() as i64);
                        }
                    }
                }
                if s.bot.is_some() && i.session.is_none() {
                    i.session = Some(memory::Log::new(now_secs() as i64, &s.map));
                }
                if let Some(log) = i.session.as_mut() {
                    for p in &s.players {
                        log.players.insert(p.name.clone());
                    }
                    for e in &ev {
                        log.event(e);
                    }
                }
                i.state = Some(s);
                ev
            };
            for e in events {
                on_event(&app, e);
            }
            if cfg.goals {
                mind::goal_tick(&app);
            }
            // battute fresche all'inizio della partita e ogni tanto (una richiesta piccola)
            if active(&app) {
                ask_quips(&app, false);
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
    mind::on_event(app, &e);
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
            if let Some(log) = lock(app).session.as_mut() {
                log.said(&from, &text);
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
            // frase di un task o battuta immediata della giocatrice: niente modello
            let emo = e["emotion"].as_str().filter(|x| prompt::EMOTIONS.split('|').any(|k| k == *x)).unwrap_or("happy");
            speak(app, emo, &s("text"));
        }
        "prank" => remember(app, &format!("[you pranked {} ({})]", s("who"), s("kind")), ""),
        // le battute generate stanno finendo
        "quips_low" => ask_quips(app, true),
        // un gesto riuscito o sbagliato: pratica (sbaglia meno col tempo) e, se sbaglia, se lo ricorda
        "try" => {
            let ok = e["ok"].as_bool().unwrap_or(true);
            mind::practice(app, &s("what"), ok);
            if !ok {
                let what = match s("what").as_str() {
                    "catch" => "you tried to catch something and fumbled it",
                    "throw" => "you meant to throw something but it slipped",
                    "jump" => "you botched a jump",
                    "tool" => "you pulled out the wrong weapon",
                    "build" => "your tower came out crooked and fell",
                    _ => "you messed something up",
                };
                remember(app, &format!("[{what}; you laughed it off]"), "");
            }
        }
        // qualcuno le ha lanciato qualcosa: lo sa e se lo ricorda (niente modello: la battuta l'ha già fatta il gioco)
        "thrown" => {
            let by = s("by");
            let how = match s("how").as_str() {
                "caught" => "you caught it with the physgun",
                "returned" => "you caught it and threw it back",
                "jumped" => "you jumped out of the way",
                "fumbled" => "you tried to catch it but fumbled",
                "fled" => "you ran from it",
                "hit" => "it hit you",
                "ignored" => "you didn't even flinch",
                _ => "you dodged it",
            };
            let who = if by.is_empty() { "something came flying".to_string() } else { format!("{by} threw a {} at you", s("what")) };
            remember(app, &format!("[{who}; {how}]"), "");
        }
        // annoiata o in cerca di vendetta: rifà una delle sue abitudini o se ne inventa una (personalità)
        "idea" => mind::want(app, "play", "", "", ""),
        "prank_idea" => mind::want(app, "prank", &s("who"), &s("why"), &s("recent")),
        // dal menu del gioco
        "monitor" => {
            let app = app.clone();
            tauri::async_runtime::spawn(async move {
                let _ = gmodlink_monitor_open(app).await;
            });
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
        // riflesso del cervello di moscerino
        "ate" => comment(app, &format!("[event] by instinct you just ate a {} (your fly brain tasted something sweet)", s("what")), false),
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
            if kind == "joined" && cfg_of(app).gmodlink.monitor_auto {
                let app = app.clone();
                tauri::async_runtime::spawn(async move {
                    let _ = gmodlink_monitor_open(app).await;
                });
            }
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

/// Ricordi delle partite passate da dare al modello adesso, se servono:
/// alla prima risposta della partita una riga sull'ultima volta con questi giocatori; poi solo se la frase
/// li chiede (parole chiave, oppure Rizzo Flow nei casi dubbi).
async fn memories(app: &AppHandle, cfg: &crate::config::Config, text: &str) -> Option<String> {
    if !cfg.gmodlink.memory {
        return None;
    }
    let (present, first) = {
        let mut i = lock(app);
        let present: Vec<String> = i.state.as_ref().map(|s| s.players.iter().map(|p| p.name.clone()).collect()).unwrap_or_default();
        let first = i.session.as_mut().is_some_and(|l| !std::mem::replace(&mut l.recalled, true));
        (present, first)
    };
    let want = match memory::need(text) {
        memory::Need::Yes => true,
        memory::Need::Maybe if cfg.saving.router == "rizzo" => memory::rizzo_wants(&cfg.saving.router_url, text).await.unwrap_or(false),
        _ => false,
    };
    if !want && !first {
        return None;
    }
    let dir = data_dir(app);
    let st = tauri::async_runtime::spawn_blocking(move || memory::load(&dir)).await.ok()?;
    let out = if want { memory::recall(&st, &present, 3) } else { memory::brief(&st, &present) };
    (!out.is_empty()).then_some(out)
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
            // "basta"/"stop" ferma anche l'obiettivo lungo
            mind::goal_line(app, None, &q.ops);
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
        let i = lock(app);
        let mut st = i.state.as_ref().map(prompt::state_line).unwrap_or_else(|| "[state] unknown".into());
        drop(i);
        let inst = instinct(app, &cfg.gmodlink);
        if !inst.is_empty() {
            st.push_str(&format!(" | instinct: {inst}"));
        }
        let mut i = lock(app);
        let h: Vec<(String, String)> = i.history.iter().cloned().collect();
        let o: Vec<String> = i.overheard.drain(..).collect();
        (st, h, o)
    };
    let mut user = state_line;
    if let Some(mem) = memories(app, &cfg, if event.is_none() { text } else { "" }).await {
        user.push('\n');
        user.push_str(&mem);
    }
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
    let fly_mood = cfg.gmodlink.fly.enabled && cfg.gmodlink.fly.mood;
    let system = prompt::system(
        crate::providers::prompt::language_rule(&cfg),
        crate::providers::prompt::pronoun_rule(&cfg),
        cfg.user_name.trim(),
        cfg.gmodlink.tasks,
        fly_mood,
    );
    let ch = tauri::ipc::Channel::new(|_| Ok(()));
    let res = crate::providers::chat(&link.ai, ai, system, merged, None, ch).await?;
    let r = prompt::parse(&res.text);
    crate::applog::info("gmodlink", &format!("risposta: {} azioni, {} caratteri", r.ops.len(), r.text.chars().count()));
    // niente script se disattivati nelle impostazioni
    let mut ops: Vec<Value> = r.ops.into_iter().filter(|o| cfg.gmodlink.tasks || o["op"] != "task").collect();
    // invenzioni (scherzi/attività) e obiettivi lunghi
    mind::after_reply(app, event.is_some(), &mut ops);
    mind::goal_line(app, r.goal.as_deref(), &ops);
    send(app, ops.clone());
    if !r.text.is_empty() {
        wait_turn(app).await;
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
    /// Cervello di moscerino: dimensioni e uscite adesso.
    pub fly: FlyStatus,
    /// Partite ricordate nella memoria delle partite.
    pub games: usize,
    /// Personalità (tratti, abitudini preferite) e obiettivo lungo in corso.
    pub persona: Value,
}

#[derive(Serialize)]
pub struct FlyStatus {
    pub neurons: usize,
    pub synapses: usize,
    pub out: Vec<(String, f32)>,
    pub stress: f32,
    pub activity: f32,
}

#[tauri::command]
pub async fn gmodlink_status(app: AppHandle) -> Status {
    let custom = cfg_of(&app).gmodlink.dir.clone();
    let dir = tauri::async_runtime::spawn_blocking(move || find_game_dir(&custom)).await.ok().flatten();
    let persona = mind::view(&app);
    let i = lock(&app);
    let fresh = i.state_at.is_some_and(|t| t.elapsed() < Duration::from_secs(6));
    Status {
        persona,
        installed: dir.as_deref().is_some_and(installed),
        dir: dir.map(|d| d.display().to_string()),
        game_running: i.game_running,
        linked: fresh,
        in_game: fresh && i.state.as_ref().is_some_and(|s| s.bot.is_some()),
        lambda: i.state.as_ref().map(|s| s.lambda),
        body: i.state.as_ref().and_then(|s| s.bot.as_ref()).and_then(|b| b.body.clone()),
        games: memory::count(&data_dir(&app)),
        fly: {
            let (neurons, synapses) = fly::size();
            let f = fly_lock(&app);
            let live = fresh && f.last_tick.is_some_and(|t| t.elapsed() < Duration::from_secs(2));
            FlyStatus {
                neurons,
                synapses,
                out: if live { f.out.clone() } else { Vec::new() },
                stress: f.mood.stress,
                activity: if live { f.activity } else { 0.0 },
            }
        },
    }
}

/// Dati per il FruitFly Monitor (leggero: si chiede 5 volte al secondo).
#[tauri::command]
pub fn gmodlink_fly_view(app: AppHandle) -> Value {
    let cfg = cfg_of(&app).gmodlink;
    let persona = mind::view(&app);
    let (fresh, in_game, name) = {
        let i = lock(&app);
        let fresh = i.state_at.is_some_and(|t| t.elapsed() < Duration::from_secs(6));
        (fresh, fresh && i.state.as_ref().is_some_and(|s| s.bot.is_some()), cfg.name.clone())
    };
    let (neurons, synapses) = fly::size();
    let f = fly_lock(&app);
    let live = in_game && cfg.fly.enabled && f.last_tick.is_some_and(|t| t.elapsed() < Duration::from_secs(2));
    let empty = std::collections::HashMap::new();
    let senses = if live { &f.senses } else { &empty };
    let out: Vec<(String, f32)> = if live { f.out.clone() } else { Vec::new() };
    let tr = &persona["traits"];
    let view = fly::monitor(
        senses,
        &out,
        if live { f.mood.stress } else { 0.0 },
        if live { f.activity } else { 0.0 },
        tr["curiosity"].as_f64().unwrap_or(0.5) as f32,
        tr["mischief"].as_f64().unwrap_or(0.5) as f32,
    );
    json!({
        "name": name,
        "status": if live { "active" } else if in_game { "brain_off" } else if fresh { "waiting" } else { "offline" },
        "neurons": neurons,
        "synapses": synapses,
        "mood": view,
        "out": out,
        "instinct": f.mood.line(),
        "persona": persona,
    })
}

/// Apre (o porta davanti) il FruitFly Monitor.
#[tauri::command]
pub async fn gmodlink_monitor_open(app: AppHandle) -> Result<(), String> {
    use tauri::{WebviewUrl, WebviewWindowBuilder};
    if let Some(w) = app.get_webview_window("flymonitor") {
        let _ = w.unminimize();
        let _ = w.show();
        return w.set_focus().map_err(|e| e.to_string());
    }
    WebviewWindowBuilder::new(&app, "flymonitor", WebviewUrl::App("flymonitor.html".into()))
        .title("FruitFly Monitor")
        .inner_size(760.0, 430.0)
        .min_inner_size(520.0, 320.0)
        .decorations(false)
        .always_on_top(true)
        .build()
        .map(|_| ())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn gmodlink_persona_forget(app: AppHandle) {
    mind::forget(&app);
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

/// Dimentica tutte le partite (memoria delle partite).
#[tauri::command]
pub fn gmodlink_forget(app: AppHandle) {
    memory::forget(&data_dir(&app));
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
    fn session_line_is_short_and_skips_empty() {
        assert_eq!(session_line(&[]), "");
        let h = vec![("".to_string(), "battuta".to_string()), ("ciao".to_string(), "ehi!".to_string())];
        assert_eq!(session_line(&h), "«ciao» → «ehi!»");
        let long = vec![("x".repeat(500), "y".repeat(500)); 8];
        assert!(session_line(&long).chars().count() <= 481);
    }

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
