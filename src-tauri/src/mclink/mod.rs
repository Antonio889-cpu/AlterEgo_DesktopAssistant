//! Alter Ego gioca con te a Minecraft (Java Edition): entra nel tuo mondo aperto in LAN (o in un server tuo)
//! con un account offline e gioca come una persona.
//!
//! Corpo, sensi e mani sono un bot Node.js con Mineflayer (mc_bot/, installato dall'app nei dati locali);
//! il cervello resta qui, come in gmodlink. Si parlano con righe JSON su stdin/stdout (vedi proto.rs):
//! comandi numerati verso il bot; stato compatto, sensi, eventi e conferme dal bot.
//!
//! Token: il comportamento continuo (seguire, guardare chi parla, difendersi, mangiare, raccogliere, esplorare)
//! gira nel bot; i comandi semplici sono parole chiave (quick.rs); i riflessi vengono dal cervello di
//! moscerino di gmodlink (reflex.rs); il modello si chiama solo quando qualcuno le parla o succede qualcosa
//! di importante, con un prompt corto e fisso (in cache) e lo stato in una riga. Quello che funziona diventa
//! un'abilità da rifare senza token, quello che non funziona una lezione (skills.rs).

pub mod auth;
pub mod console;
pub mod node;
pub mod observe;
pub mod plan;
pub mod prompt;
pub mod proto;
pub mod pvp;
pub mod quick;
pub mod quips;
pub mod reflex;
pub mod servers;
pub mod rizzo;
pub mod skills;
pub mod skin;
pub mod via;

use crate::gmodlink::fly;
use crate::{update_config, AppState};
use proto::{Incoming, LanWorld, McState};
use serde::Serialize;
use serde_json::{json, Value};
use std::{
    collections::{HashMap, VecDeque},
    io::{BufRead, BufReader, Write},
    path::PathBuf,
    process::{Child, ChildStdin, Command, Stdio},
    sync::Mutex,
    time::{Duration, Instant},
};
use tauri::{AppHandle, Emitter, Manager};

pub struct Mc {
    inner: Mutex<Inner>,
    fly: Mutex<FlyState>,
    pub ai: crate::providers::Ai,
    /// Client a parte per le battute: non interrompe le risposte in corso.
    quip_ai: crate::providers::Ai,
    /// Una richiesta al modello alla volta.
    busy: tokio::sync::Mutex<()>,
    quips: Mutex<crate::gmodlink::quips::Quips>,
    skills: Mutex<Option<skills::Store>>,
}

impl Default for Mc {
    fn default() -> Self {
        Self {
            inner: Mutex::new(Inner::default()),
            fly: Mutex::new(FlyState::default()),
            ai: crate::providers::Ai::with_kind("minecraft"),
            quip_ai: crate::providers::Ai::with_kind("minecraft"),
            busy: tokio::sync::Mutex::new(()),
            quips: Mutex::new(Default::default()),
            skills: Mutex::new(None),
        }
    }
}

#[derive(Default)]
struct Inner {
    child: Option<Child>,
    stdin: Option<ChildStdin>,
    /// Numero del processo del bot (le righe di uno vecchio si ignorano).
    gen: u64,
    next_id: u64,
    joined: bool,
    state: Option<McState>,
    state_at: Option<Instant>,
    senses: HashMap<String, f32>,
    sense_at: Option<Instant>,
    /// Versione, host e porta del mondo in cui è entrata.
    world: Value,
    /// Ultima espulsione: (codice, motivo).
    kick: Option<(String, String)>,
    error: Option<String>,
    /// Risposta di SkinsRestorer: (riuscito, messaggio).
    skin: Option<(bool, String)>,
    lan: Vec<LanWorld>,
    history: VecDeque<(String, String)>,
    overheard: VecDeque<String>,
    talking_with: Option<(String, Instant)>,
    last_comment: Option<Instant>,
    plans: Vec<Plan>,
}

/// Azioni mandate per una richiesta: quando arrivano tutte le conferme diventano un'abilità o una lezione.
struct Plan {
    request: String,
    name: Option<String>,
    ops: Vec<Value>,
    ids: Vec<u64>,
    acked: usize,
    failed: bool,
    /// Chiesta da qualcuno (non un'idea sua): se fallisce lo dice.
    asked: bool,
    at: Instant,
}

#[derive(Default)]
struct FlyState {
    brain: Option<Box<fly::Brain>>,
    mood: fly::Mood,
    out: Vec<(String, f32)>,
    last_tick: Option<Instant>,
    reflex_at: HashMap<&'static str, f64>,
    t0: Option<Instant>,
    activity: f32,
}

fn mc(app: &AppHandle) -> &Mc {
    app.state::<Mc>().inner()
}

fn lock(app: &AppHandle) -> std::sync::MutexGuard<'_, Inner> {
    app.state::<Mc>().inner().inner.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn fly_lock(app: &AppHandle) -> std::sync::MutexGuard<'_, FlyState> {
    app.state::<Mc>().inner().fly.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn cfg_of(app: &AppHandle) -> crate::config::Config {
    app.state::<AppState>().cfg.lock().unwrap_or_else(std::sync::PoisonError::into_inner).clone()
}

fn data_dir(app: &AppHandle) -> PathBuf {
    app.state::<crate::assistant::Conversation>().data_dir.clone()
}

fn with_skills<R>(app: &AppHandle, f: impl FnOnce(&mut skills::Store) -> R) -> R {
    let dir = data_dir(app);
    let m = mc(app);
    let mut s = m.skills.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    if s.is_none() {
        *s = Some(skills::load(&dir));
    }
    f(s.as_mut().unwrap())
}

fn save_skills(app: &AppHandle) {
    let dir = data_dir(app);
    let st = mc(app).skills.lock().unwrap_or_else(std::sync::PoisonError::into_inner).clone();
    if let Some(st) = st {
        skills::save(&dir, &st);
    }
}

/// Alter Ego è nel mondo adesso.
pub fn active(app: &AppHandle) -> bool {
    if !cfg_of(app).minecraft.enabled {
        return false;
    }
    let i = lock(app);
    i.joined && i.state_at.is_some_and(|t| t.elapsed() < Duration::from_secs(6))
}

fn running(app: &AppHandle) -> bool {
    let mut i = lock(app);
    match i.child.as_mut().map(|c| c.try_wait()) {
        Some(Ok(None)) => true,
        Some(_) => {
            i.child = None;
            i.stdin = None;
            i.joined = false;
            false
        }
        None => false,
    }
}

/// Manda comandi al bot (numerati). Restituisce i numeri usati.
pub fn send(app: &AppHandle, ops: Vec<Value>) -> Vec<u64> {
    let mut ids = Vec::new();
    let mut sent = Vec::new();
    let mut i = lock(app);
    for op in ops {
        i.next_id += 1;
        let id = i.next_id;
        let line = proto::cmd_line(id, &op);
        let ok = i.stdin.as_mut().is_some_and(|s| s.write_all(line.as_bytes()).and_then(|_| s.flush()).is_ok());
        if ok {
            ids.push(id);
            sent.push(op);
        }
    }
    drop(i);
    // nella console Minecraft (password coperte)
    for op in &sent {
        console::on_send(app, op);
    }
    ids
}

/// Impostazioni che il bot usa da solo (comportamento continuo, battute).
fn bot_cfg(app: &AppHandle) -> Value {
    let cfg = cfg_of(app);
    let m = &cfg.minecraft;
    let quips = mc(app).quips.lock().unwrap_or_else(std::sync::PoisonError::into_inner).lines.clone();
    json!({
        "lang": crate::providers::prompt::reply_lang(&cfg),
        "initiative": m.initiative,
        "follow": m.follow,
        "gather": m.gather,
        "build": m.build,
        "explore": m.explore,
        "fly": m.fly,
        "player": m.player.trim(),
        "quips": quips,
        "observe": m.observe,
        "tidy": m.tidy,
        "duels": m.duels,
        "ask": m.ask,
        "schem_dir": plan::schem_dir(app).display().to_string(),
    })
}

// ---------- processo del bot ----------

/// Avvia il bot ed entra nel mondo.
pub fn start(app: &AppHandle) -> Result<(), String> {
    if running(app) {
        return Ok(());
    }
    let cfg = cfg_of(app).minecraft;
    let node = node::find(app, &cfg.node).ok_or_else(|| crate::i18n::tr("Node.js non trovato: premi «Installa» qui sotto", "Node.js not found: press \"Install\" below").to_string())?;
    let dir = node::bot_dir(app);
    if !node::deps_ready(app) {
        return Err(crate::i18n::tr("il bot non è installato o va aggiornato: premi «Installa / aggiorna»", "the bot isn't installed or needs an update: press \"Install / update\"").into());
    }
    node::write_bot(&dir)?;
    // skin per CustomSkinLoader (la vede chi ha la mod)
    if cfg.csl {
        if let Some(csl) = skin::dot_minecraft().and_then(|m| skin::csl_dir(&m)) {
            match skin::csl_install(&csl, &cfg.username, &skin::current(&data_dir(app), &cfg.skin)) {
                Ok(p) => crate::applog::info("minecraft", &format!("skin per CustomSkinLoader: {}", p.display())),
                Err(e) => crate::applog::warn("minecraft", &format!("skin CustomSkinLoader: {e}")),
            }
        }
    }
    let log = std::fs::File::create(node::root(app).join("bot.log")).map_err(|e| e.to_string())?;
    let mut cmd = Command::new(&node.path);
    cmd.arg("bot.js").current_dir(&dir).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(log);
    let mut child = node::hidden(&mut cmd).spawn().map_err(|e| crate::tr!("avvio del bot non riuscito: {}", "could not start the bot: {}", e))?;
    let stdout = child.stdout.take().ok_or("stdout")?;
    let gen = {
        let mut i = lock(app);
        i.gen += 1;
        i.stdin = child.stdin.take();
        i.child = Some(child);
        i.joined = false;
        i.kick = None;
        i.error = None;
        i.skin = None;
        i.plans.clear();
        if i.next_id == 0 {
            i.next_id = 1;
        }
        i.gen
    };
    let a = app.clone();
    let _ = std::thread::Builder::new().name("minecraft-bot".into()).spawn(move || {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            if lock(&a).gen != gen {
                break;
            }
            if let Some(m) = proto::parse_line(&line) {
                on_incoming(&a, m);
            }
        }
        let mut i = lock(&a);
        if i.gen == gen {
            let was = i.joined;
            i.joined = false;
            i.stdin = None;
            if let Some(mut c) = i.child.take() {
                let _ = c.wait();
            }
            drop(i);
            // il ponte serviva solo a questo bot
            via::stop_proxy(&a);
            if was {
                crate::applog::info("minecraft", "bot uscito dal mondo");
            }
            let _ = a.emit("minecraft-link", "left");
        }
    });
    let skin_cmd = skin::skin_cmd(&cfg.skin_cmd, cfg.slim).unwrap_or_default();
    // nativo o ponte ViaProxy (ping di stato), poi "connect" (via.rs)
    via::launch(
        app,
        json!({
            "op": "connect",
            "host": if cfg.host.trim().is_empty() { "127.0.0.1" } else { cfg.host.trim() },
            "port": cfg.port,
            "username": cfg.username,
            "version": cfg.version.trim(),
            "skin": skin_cmd,
            "cfg": bot_cfg(app),
        }),
        node.path.clone(),
        dir.clone(),
        gen,
    );
    crate::applog::info("minecraft", &format!("entro in {}:{} come {} (node {})", cfg.host, if cfg.port == 0 { "LAN".to_string() } else { cfg.port.to_string() }, cfg.username, node.version));
    let _ = app.emit("minecraft-link", "starting");
    Ok(())
}

/// Esce dal mondo e chiude il bot.
pub fn stop(app: &AppHandle) {
    let (history, place) = {
        let mut i = lock(app);
        let h: Vec<(String, String)> = std::mem::take(&mut i.history).into();
        (h, i.world["host"].as_str().unwrap_or_default().to_string())
    };
    crate::gmodlink::session_to_graph(app, "Minecraft", &place, &history);
    send(app, vec![json!({ "op": "leave" })]);
    let app = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(1500));
        let mut i = lock(&app);
        i.stdin = None; // il bot si chiude quando stdin si chiude
        if let Some(mut c) = i.child.take() {
            if !matches!(c.try_wait(), Ok(Some(_))) {
                let _ = c.kill();
            }
            let _ = c.wait();
        }
        i.joined = false;
        drop(i);
        let _ = app.emit("minecraft-link", "left");
    });
}

fn on_incoming(app: &AppHandle, m: Incoming) {
    match m {
        Incoming::Ready => {}
        Incoming::State(s) => {
            servers::on_state(app, &s);
            let mut i = lock(app);
            i.state = Some(*s);
            i.state_at = Some(Instant::now());
        }
        Incoming::Sense(s) => {
            let mut i = lock(app);
            i.senses = s;
            i.sense_at = Some(Instant::now());
        }
        Incoming::Lan(w) => lock(app).lan = w,
        Incoming::Ack { id, op, ok, err, info } => on_ack(app, id, &op, ok, &err, &info),
        Incoming::Event(e) => on_event(app, e),
    }
}

/// Conferma di un comando: chiude i piani (abilità imparate o lezioni).
fn on_ack(app: &AppHandle, id: u64, op: &str, ok: bool, err: &str, info: &Value) {
    if !ok && op != "connect" {
        crate::applog::warn("minecraft", &format!("{op}: {err}"));
    }
    if op == "connect" && !ok {
        lock(app).error = Some(err.to_string());
        let _ = app.emit("minecraft-link", "error");
        return;
    }
    let learn = cfg_of(app).minecraft.learn;
    let done = {
        let mut i = lock(app);
        i.plans.retain(|p| p.at.elapsed() < Duration::from_secs(15 * 60));
        let Some(pos) = i.plans.iter().position(|p| p.ids.contains(&id)) else { return };
        let p = &mut i.plans[pos];
        p.acked += 1;
        let mut fail = None;
        if !ok && !p.failed {
            p.failed = true;
            let idx = p.ids.iter().position(|x| *x == id).unwrap_or(0);
            fail = Some((p.ops.get(idx).cloned().unwrap_or(json!({ "op": op })), p.request.clone(), p.asked));
        }
        let finished = p.acked >= p.ids.len();
        let plan = if finished { Some(i.plans.remove(pos)) } else { None };
        (fail, plan)
    };
    let (fail, plan) = done;
    if let Some((opv, request, asked)) = fail {
        if learn {
            with_skills(app, |s| s.add_lesson(&opv, err, &request, now_secs()));
            save_skills(app);
        }
        let what = prompt::summary(&opv);
        remember(app, &format!("[your action {what} failed: {err}]"), "");
        if asked && !err.to_lowercase().contains("stopped") && !err.contains("fermata") {
            comment(app, &format!("[event] your action \"{what}\" failed: {err}. Tell your friend briefly, maybe suggest what's missing."), true);
        }
    }
    if let Some(p) = plan {
        if !p.failed {
            if learn {
                if let Some(name) = with_skills(app, |s| s.add_skill(p.name.as_deref(), &p.ops, &p.request, now_secs())) {
                    save_skills(app);
                    crate::applog::info("minecraft", &format!("abilità imparata/rinforzata: {name}"));
                }
            }
            if p.asked && p.ops.len() >= 2 {
                send(app, vec![json!({ "op": "quip", "kind": "done" })]);
            }
            let _ = info;
        }
    }
}

fn speak_pc(app: &AppHandle, emotion: &str, text: &str) {
    if !text.trim().is_empty() {
        let _ = app.emit("app-say", format!("[{emotion}]\n{text}"));
    }
}

/// Il messaggio in chat è rivolto a lei?
fn addressed(app: &AppHandle, from: &str, whisper: bool, text: &str) -> bool {
    let t = text.trim();
    if t.starts_with('/') || t.starts_with('!') {
        return false;
    }
    if whisper {
        return true;
    }
    let cfg = cfg_of(app);
    let name = cfg.minecraft.username.to_lowercase();
    let low = t.to_lowercase();
    if ["alter", "ego", "chihiro"].iter().any(|w| low.contains(w)) || (!name.is_empty() && low.contains(&name)) {
        return true;
    }
    let i = lock(app);
    let others = i.state.as_ref().map(|s| s.players.len()).unwrap_or(1);
    let recent = i.talking_with.as_ref().is_some_and(|(w, at)| w == from && at.elapsed() < Duration::from_secs(60));
    // da solo con lei: tutto quello che scrivi è per lei
    others <= 1 || recent
}

fn on_event(app: &AppHandle, e: Value) {
    // console, login, inviti, spostamenti tra server (servers.rs)
    let Some(e) = servers::on_event(app, e) else { return };
    let kind = e["kind"].as_str().unwrap_or("").to_string();
    let s = |k: &str| e[k].as_str().unwrap_or("").to_string();
    match kind.as_str() {
        "chat" => {
            let (from, text, whisper) = (s("from"), s("text"), e["whisper"].as_bool().unwrap_or(false));
            // risposta a una domanda di Alter Ego (zero token)
            if plan::take_answer(app, &from, &text) {
                return;
            }
            if !addressed(app, &from, whisper, &text) {
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
                    Err(err) => crate::applog::warn("minecraft", &err),
                }
            });
        }
        // battuta fatta dal bot (zero token): voce e box sul PC
        "said" => speak_pc(app, e["emotion"].as_str().filter(|x| prompt::EMOTIONS.split('|').any(|k| k == *x)).unwrap_or("happy"), &s("text")),
        "joined" => {
            {
                let mut i = lock(app);
                i.joined = true;
                i.world = json!({ "version": e["version"], "host": e["host"], "port": e["port"] });
                i.kick = None;
                i.error = None;
                i.state_at = Some(Instant::now());
            }
            crate::applog::info("minecraft", &format!("nel mondo: Minecraft {} su {}:{}", s("version"), s("host"), e["port"]));
            let _ = app.emit("minecraft-link", "joined");
            ask_quips(app, false);
        }
        "left" => {
            lock(app).joined = false;
            let _ = app.emit("minecraft-link", "left");
        }
        "kicked" => {
            let reason = s("reason");
            let code = proto::kick_code(&reason).to_string();
            crate::applog::warn("minecraft", &format!("espulsa dal server ({code}): {reason}"));
            lock(app).kick = Some((code, reason));
            let _ = app.emit("minecraft-link", "kicked");
        }
        "error" => {
            let msg = s("msg");
            crate::applog::warn("minecraft", &format!("bot: {msg}"));
            let code = if e["code"].is_string() { s("code") } else { proto::kick_code(&msg).to_string() };
            let mut i = lock(app);
            if !i.joined && code != "other" {
                i.kick = Some((code, msg.clone()));
            }
            i.error = Some(msg);
            drop(i);
            let _ = app.emit("minecraft-link", "error");
        }
        "lan" => {
            if let Ok(w) = serde_json::from_value::<LanWorld>(e.clone()) {
                lock(app).lan = vec![w];
            }
        }
        "skin" => {
            let ok = e["ok"].as_bool().unwrap_or(false);
            crate::applog::info("minecraft", &format!("SkinsRestorer: {}", s("msg")));
            lock(app).skin = Some((ok, s("msg")));
        }
        "died" => comment(app, "[event] you died; you'll respawn at spawn and your items are where you died", false),
        "hurt" if e["by"].is_string() => remember(app, &format!("[{} hit you]", s("by")), ""),
        "player_join" => comment(app, &format!("[event] {} joined the world", s("name")), false),
        "player_leave" => remember(app, &format!("[{} left the world]", s("name")), ""),
        "found" => comment(app, &format!("[event] you spotted {} {} blocks away", s("block"), e["d"]), false),
        "night" => remember(app, "[night is falling]", ""),
        "ate" => remember(app, &format!("[you ate {}]", s("what")), ""),
        "idea" => remember(app, &format!("[on your own you started: {}]", s("op")), ""),
        "observe" | "observed" | "doubt" | "duel_need_tactic" | "duel_round" | "duel_end" | "plan" | "build_status" | "craft_plan" | "worn" => plan::on_event(app, &kind, &e),
        _ => {}
    }
}

fn now_secs() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

/// Aggiunge una riga al contesto senza chiamare il modello.
fn remember(app: &AppHandle, user: &str, reply: &str) {
    let mut i = lock(app);
    i.history.push_back((user.to_string(), reply.to_string()));
    while i.history.len() > 6 {
        i.history.pop_front();
    }
}

/// Commento spontaneo su un evento, più spesso con più iniziativa (`force`: va detto comunque).
fn comment(app: &AppHandle, what: &str, force: bool) {
    let ini = cfg_of(app).minecraft.initiative;
    if ini == 0 && !force {
        return;
    }
    let gap = 300u64.saturating_sub(ini as u64 * 24 / 10).max(45);
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
            Err(err) => crate::applog::warn("minecraft", &err),
        }
    });
}

/// "adrenaline 0.6, dodged something" dal cervello di moscerino (vuoto se calma o spento).
fn instinct(app: &AppHandle) -> String {
    if !cfg_of(app).minecraft.fly {
        return String::new();
    }
    fly_lock(app).mood.line()
}

/// Chiede al modello un pacchetto di battute, se serve.
fn ask_quips(app: &AppHandle, low: bool) {
    let cfg = cfg_of(app);
    let key = format!("{}/{}", crate::providers::prompt::reply_lang(&cfg), cfg.personality.pronouns());
    {
        let mut q = mc(app).quips.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
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
        let m = mc(&app);
        let res = crate::providers::chat(&m.quip_ai, ai, system, msgs, None, ch).await;
        let mut q = m.quips.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        q.busy = false;
        match res.map(|r| quips::parse(&r.text)) {
            Ok(Some(b)) => {
                q.store(&key, b);
                let lines = q.lines.clone();
                drop(q);
                send(&app, vec![json!({ "op": "quips", "lines": lines })]);
                crate::applog::info("minecraft", "battute nuove");
            }
            Ok(None) => crate::applog::warn("minecraft", "battute: risposta non leggibile (restano quelle di riserva)"),
            Err(e) => crate::applog::warn("minecraft", &format!("battute: {e}")),
        }
    });
}

/// Parole che fanno pensare al gioco (contro quelle "da PC" come musica o ricerche web).
const GAME_WORDS: &[&str] = &[
    "minecraft", "mondo", "world", "blocco", "blocchi", "block", "costruisci", "costruiamo", "build", "scava", "mine", "raccogli", "collect", "craft",
    "piccone", "pickaxe", "spada", "sword", "legna", "wood", "pietra", "stone", "ferro", "iron", "diamant", "diamond", "zombie", "creeper", "scheletro",
    "skeleton", "casa", "house", "letto", "bed", "notte", "night", "seguimi", "follow", "vieni", "come", "fermati", "stay", "mangia", "eat", "esplora",
    "explore", "inventario", "inventory", "fornace", "furnace",
];

/// La frase detta ad Alter Ego (push-to-talk o testo nell'app) va al cervello di Minecraft?
/// Le richieste "da PC" (musica, web, file, promemoria…) restano all'assistente normale.
pub fn wants(app: &AppHandle, text: &str) -> bool {
    let cfg = cfg_of(app);
    if !cfg.minecraft.enabled {
        return false;
    }
    if !active(app) {
        // non è nel mondo: solo "vieni a giocare a Minecraft"
        return quick::is_join(text) && node::deps_ready(app);
    }
    let low = format!(" {} ", text.to_lowercase());
    let game = GAME_WORDS.iter().any(|w| low.contains(w));
    let desktop = crate::saving::pick_groups(text).iter().any(|g| ["musica", "web", "file", "promemoria", "database", "grafo"].contains(g));
    !(desktop && !game)
}

/// Pausa da persona prima di rispondere (ms).
fn human_pause(min: u64, max: u64) -> Duration {
    let r = crate::gmodlink::persona::Rng::new().f() as f64;
    Duration::from_millis(min + ((max - min) as f64 * r) as u64)
}

/// Risponde a `text` detto da `from` (o reagisce a `event`). Restituisce "[emozione]\ntesto"
/// (per la voce e il box sul PC); la riga va anche nella chat del gioco.
pub async fn handle(app: &AppHandle, from: &str, text: &str, event: Option<&str>) -> Result<String, String> {
    let cfg = cfg_of(app);
    let en = crate::providers::prompt::reply_lang(&cfg) == "en";
    // dal PC (push-to-talk): parla il giocatore con cui gioca
    let owner = {
        let i = lock(app);
        i.state.as_ref().map(|s| s.owner.clone()).filter(|o| !o.is_empty()).unwrap_or_else(|| cfg.minecraft.player.trim().to_string())
    };
    let from = if from.is_empty() && event.is_none() { owner.clone() } else { from.to_string() };
    if event.is_none() && !active(app) {
        start(app)?;
        let line = if en { "Coming! Joining your world…" } else { "Arrivo! Sto entrando nel tuo mondo…" };
        return Ok(format!("[happy]\n{line}"));
    }
    // 1. comandi semplici: zero token
    if event.is_none() {
        if let Some(q) = quick::find(text, &from, &cfg.minecraft.username, en).or_else(|| plan::find(text, &from, en, &plan::schem_dir(app), &cfg.minecraft)) {
            tokio::time::sleep(human_pause(200, 600)).await;
            let leave = q.ops.iter().any(|o| o["op"] == "leave");
            send(app, vec![json!({ "op": "say", "text": q.reply })]);
            if leave {
                stop(app);
            } else if !plan::ask_build_spot(app, &from, &q.ops) {
                send(app, q.ops);
            }
            remember(app, &format!("{from}: {text}"), &q.reply);
            lock(app).talking_with = Some((from, Instant::now()));
            return Ok(format!("[{}]\n{}", q.emotion, q.reply));
        }
    }
    // 2. il modello (una richiesta alla volta)
    let m = mc(app);
    let _guard = m.busy.lock().await;
    let (mut user, history, overheard) = {
        let mut i = lock(app);
        let st = i.state.as_ref().map(proto::state_line).unwrap_or_else(|| "[state] unknown".into());
        let h: Vec<(String, String)> = i.history.iter().cloned().collect();
        let o: Vec<String> = i.overheard.drain(..).collect();
        (st, h, o)
    };
    let inst = instinct(app);
    if !inst.is_empty() {
        user.push_str(&format!(" | instinct: {inst}"));
    }
    if cfg.minecraft.learn {
        let block = with_skills(app, |s| s.prompt_block(event.unwrap_or(text)));
        if !block.is_empty() {
            user.push('\n');
            user.push_str(&block);
        }
    }
    if cfg.minecraft.observe || cfg.minecraft.duels {
        let extra = plan::context(app, if event.is_none() { &from } else { &owner });
        if !extra.is_empty() {
            user.push('\n');
            user.push_str(&extra);
        }
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
    // cronologia corta: solo le righe di chat (lo stato vale solo per adesso); deve alternare user/assistant
    let mut merged: Vec<crate::providers::ChatMessage> = Vec::new();
    for (u, a) in &history {
        for (role, c) in [("user", u), ("assistant", a)] {
            if c.is_empty() {
                continue;
            }
            match merged.last_mut() {
                Some(last) if last.role == role => {
                    last.content.push('\n');
                    last.content.push_str(c);
                }
                _ => merged.push(crate::providers::ChatMessage::new(role, c)),
            }
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
    ai.max_tokens = 600;
    let system = prompt::system(crate::providers::prompt::language_rule(&cfg), crate::providers::prompt::pronoun_rule(&cfg), cfg.user_name.trim(), cfg.minecraft.fly);
    let ch = tauri::ipc::Channel::new(|_| Ok(()));
    let res = crate::providers::chat(&m.ai, ai, system, merged, None, ch).await?;
    let r = prompt::parse(&res.text);
    // abilità imparate: si espandono qui (zero token)
    let mut ops: Vec<Value> = Vec::new();
    for o in r.ops {
        if o["op"] == "skill" {
            let name = o["name"].as_str().unwrap_or("").to_string();
            match with_skills(app, |s| s.get(&name).map(|k| k.ops.clone())) {
                Some(k) => ops.extend(k),
                None => remember(app, &format!("[you don't know a skill called {name} yet]"), ""),
            }
        } else if cfg.minecraft.build || o["op"] != "build" {
            ops.push(o);
        }
    }
    crate::applog::info("minecraft", &format!("risposta: {} azioni, {} caratteri", ops.len(), r.text.chars().count()));
    if !r.text.is_empty() {
        tokio::time::sleep(human_pause(250, 700)).await;
        send(app, vec![json!({ "op": "say", "text": r.text })]);
    }
    let ids = send(app, ops.clone());
    if !ids.is_empty() && ids.len() == ops.len() {
        lock(app).plans.push(Plan { request: if event.is_some() { String::new() } else { text.to_string() }, name: r.skill.clone(), ops: ops.clone(), ids, acked: 0, failed: false, asked: event.is_none(), at: Instant::now() });
    }
    let mut reply_mem = format!("[{}]\n{}", r.emotion, r.text);
    for o in &ops {
        reply_mem.push_str(&format!("\n[act] {}", prompt::summary(o)));
    }
    remember(app, &line, &reply_mem);
    if !from.is_empty() && event.is_none() {
        lock(app).talking_with = Some((from, Instant::now()));
    }
    Ok(format!("[{}]\n{}", r.emotion, if r.text.is_empty() { "…".into() } else { r.text }))
}

// ---------- cervello di moscerino ----------

/// Thread dei riflessi: 10 giri al secondo mentre è nel mondo (sensi dal bot → cervello → riflessi al bot).
pub fn spawn(app: AppHandle) {
    let _ = std::thread::Builder::new().name("minecraft-fly".into()).spawn(move || loop {
        let on = {
            let c = cfg_of(&app).minecraft;
            c.enabled && c.fly
        };
        let live = on && active(&app);
        std::thread::sleep(Duration::from_millis(if live { 100 } else { 500 }));
        if !live {
            continue;
        }
        fly_tick(&app);
    });
}

fn fly_tick(app: &AppHandle) {
    let senses = {
        let i = lock(app);
        if i.sense_at.is_some_and(|t| t.elapsed() < Duration::from_millis(1200)) {
            i.senses.clone()
        } else {
            HashMap::new()
        }
    };
    let mut f = fly_lock(app);
    if f.brain.is_none() {
        match fly::Brain::load() {
            Some(b) => {
                crate::applog::info("minecraft", &format!("cervello di moscerino: {} neuroni, {} sinapsi", b.neurons(), b.synapses()));
                f.brain = Some(Box::new(b));
                f.t0 = Some(Instant::now());
            }
            None => return,
        }
    }
    let ms = f.last_tick.map(|t| t.elapsed().as_secs_f32() * 1000.0).unwrap_or(100.0).clamp(10.0, 300.0);
    f.last_tick = Some(Instant::now());
    let now = f.t0.map(|t| t.elapsed().as_secs_f64()).unwrap_or(0.0);
    let FlyState { brain, mood, out, reflex_at, activity, .. } = &mut *f;
    let Some(b) = brain.as_mut() else { return };
    b.run(ms, &senses);
    *out = b.read(ms);
    fly::gate(out, &senses);
    *activity = b.take_total() as f32 / (ms / 1000.0);
    mood.update(ms / 1000.0, &senses, out);
    let picked = reflex::decide(out, now, reflex_at);
    drop(f);
    if !picked.is_empty() {
        send(app, picked.into_iter().map(|k| json!({ "op": "reflex", "kind": k })).collect());
    }
}

// ---------- comandi per le impostazioni ----------

#[derive(Serialize)]
pub struct Status {
    /// Node.js trovato (versione) e se l'ha scaricato l'app.
    pub node: Option<node::NodeInfo>,
    /// Mineflayer e plugin installati e aggiornati.
    pub installed: bool,
    pub installing: bool,
    /// Processo del bot avviato.
    pub running: bool,
    /// Alter Ego è nel mondo.
    pub in_game: bool,
    pub world: Value,
    pub players: Vec<String>,
    /// Ultima espulsione: codice ("online_mode", "whitelist", "version"…) e motivo.
    pub kick: Option<(String, String)>,
    pub error: Option<String>,
    /// Risposta di SkinsRestorer (riuscito, messaggio).
    pub skin: Option<(bool, String)>,
    /// CustomSkinLoader trovato nella cartella .minecraft.
    pub csl: bool,
    pub skills: usize,
    pub lessons: usize,
    pub stress: f32,
    pub instinct: String,
}

#[tauri::command]
pub async fn minecraft_status(app: AppHandle) -> Status {
    let custom = cfg_of(&app).minecraft.node.clone();
    let a = app.clone();
    let (node, installed, csl) = tauri::async_runtime::spawn_blocking(move || (node::find(&a, &custom), node::deps_ready(&a), skin::dot_minecraft().and_then(|m| skin::csl_dir(&m)).is_some()))
        .await
        .unwrap_or((None, false, false));
    let (skills, lessons) = with_skills(&app, |s| (s.skills.len(), s.lessons.len()));
    let run = running(&app);
    let in_game = active(&app);
    let (stress, instinct) = {
        let f = fly_lock(&app);
        (if in_game { f.mood.stress } else { 0.0 }, if in_game { f.mood.line() } else { String::new() })
    };
    let i = lock(&app);
    Status {
        node,
        installed,
        installing: node::installing(),
        running: run,
        in_game,
        world: if in_game { i.world.clone() } else { Value::Null },
        players: if in_game { i.state.as_ref().map(|s| s.players.iter().map(|p| p.name.clone()).collect()).unwrap_or_default() } else { Vec::new() },
        kick: i.kick.clone(),
        error: i.error.clone(),
        skin: i.skin.clone(),
        csl,
        skills,
        lessons,
        stress,
        instinct,
    }
}

#[tauri::command]
pub fn minecraft_settings(app: AppHandle, settings: crate::config::MinecraftConfig) {
    update_config(&app, |c| {
        // i profili dei server si cambiano solo da minecraft_server_set
        let servers = std::mem::take(&mut c.minecraft.servers);
        c.minecraft = settings;
        c.minecraft.servers = servers;
        c.minecraft.normalize();
    });
    send(&app, vec![json!({ "op": "config", "cfg": bot_cfg(&app) })]);
}

#[tauri::command]
pub async fn minecraft_install(app: AppHandle) -> Result<String, String> {
    let custom = cfg_of(&app).minecraft.node.clone();
    let n = node::install(&app, &custom).await?;
    Ok(n.version)
}

/// Entra (o esce) dal mondo: pulsanti nelle impostazioni.
#[tauri::command]
pub async fn minecraft_join(app: AppHandle, join: bool) -> Result<(), String> {
    if join {
        update_config(&app, |c| c.minecraft.enabled = true);
        let a = app.clone();
        tauri::async_runtime::spawn_blocking(move || start(&a)).await.map_err(|e| e.to_string())?
    } else {
        stop(&app);
        Ok(())
    }
}

/// Cerca i mondi aperti in LAN (annunci UDP sulla porta 4445, ~3,5 s).
#[tauri::command]
pub async fn minecraft_scan_lan(app: AppHandle) -> Result<Vec<LanWorld>, String> {
    let custom = cfg_of(&app).minecraft.node.clone();
    let a = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let node = node::find(&a, &custom).ok_or_else(|| crate::i18n::tr("Node.js non trovato: premi «Installa» qui sotto", "Node.js not found: press \"Install\" below").to_string())?;
        let dir = node::bot_dir(&a);
        node::write_bot(&dir)?;
        let out = node::hidden(Command::new(&node.path).arg("bot.js").arg("--scan-lan").current_dir(&dir)).stdin(Stdio::null()).stderr(Stdio::null()).output().map_err(|e| e.to_string())?;
        let text = String::from_utf8_lossy(&out.stdout);
        let worlds = text.lines().find_map(|l| match proto::parse_line(l) {
            Some(Incoming::Lan(w)) => Some(w),
            _ => None,
        });
        let w = worlds.unwrap_or_default();
        lock(&a).lan = w.clone();
        Ok(w)
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Sceglie la skin: `data` = PNG in base64 (skin da file), vuoto = skin inclusa.
#[tauri::command]
pub fn minecraft_skin_set(app: AppHandle, data: String) -> Result<String, String> {
    use base64::Engine;
    if data.trim().is_empty() {
        update_config(&app, |c| c.minecraft.skin = "alterego".into());
        return Ok("alterego".into());
    }
    let b64 = data.split_once(',').map(|x| x.1).unwrap_or(&data);
    let bytes = base64::engine::general_purpose::STANDARD.decode(b64.trim()).map_err(|_| crate::i18n::tr("file non leggibile", "unreadable file").to_string())?;
    let (w, h) = skin::validate(&bytes)?;
    let p = skin::custom_path(&data_dir(&app));
    if let Some(d) = p.parent() {
        std::fs::create_dir_all(d).map_err(|e| e.to_string())?;
    }
    std::fs::write(&p, &bytes).map_err(|e| e.to_string())?;
    update_config(&app, |c| c.minecraft.skin = "file".into());
    Ok(format!("{w}x{h}"))
}

/// PNG della skin in uso (base64) per l'anteprima.
#[tauri::command]
pub fn minecraft_skin_get(app: AppHandle) -> String {
    use base64::Engine;
    let choice = cfg_of(&app).minecraft.skin;
    base64::engine::general_purpose::STANDARD.encode(skin::current(&data_dir(&app), &choice))
}

/// Copia la skin per CustomSkinLoader. Restituisce il file scritto.
#[tauri::command]
pub fn minecraft_skin_csl(app: AppHandle) -> Result<String, String> {
    let cfg = cfg_of(&app).minecraft;
    let mc = skin::dot_minecraft().ok_or_else(|| crate::i18n::tr("cartella .minecraft non trovata", ".minecraft folder not found").to_string())?;
    let csl = skin::csl_dir(&mc).ok_or_else(|| crate::i18n::tr("CustomSkinLoader non trovato: installa la mod e avvia Minecraft una volta", "CustomSkinLoader not found: install the mod and start Minecraft once").to_string())?;
    skin::csl_install(&csl, &cfg.username, &skin::current(&data_dir(&app), &cfg.skin)).map(|p| p.display().to_string())
}

/// Salva il PNG della skin in uso nella cartella dei dati e la apre (per caricarlo su skinsrestorer.net/upload).
#[tauri::command]
pub fn minecraft_skin_export(app: AppHandle) -> Result<String, String> {
    let cfg = cfg_of(&app).minecraft;
    let d = node::root(&app);
    std::fs::create_dir_all(&d).map_err(|e| e.to_string())?;
    let p = d.join(format!("{}-skin.png", cfg.username));
    std::fs::write(&p, skin::current(&data_dir(&app), &cfg.skin)).map_err(|e| e.to_string())?;
    #[cfg(windows)]
    let r = Command::new("explorer").arg(&d).spawn();
    #[cfg(target_os = "macos")]
    let r = Command::new("open").arg(&d).spawn();
    #[cfg(all(unix, not(target_os = "macos")))]
    let r = Command::new("xdg-open").arg(&d).spawn();
    let _ = r;
    Ok(p.display().to_string())
}

/// Dimentica abilità e lezioni imparate giocando.
#[tauri::command]
pub fn minecraft_forget(app: AppHandle) {
    skills::forget(&data_dir(&app));
    plan::forget(&data_dir(&app));
    *mc(&app).skills.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = Some(skills::Store::default());
}
