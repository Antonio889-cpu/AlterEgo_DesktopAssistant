//! Piani senza token per Minecraft e smistamento degli eventi nuovi del bot (1.15).
//! - Richieste a parole riconosciute qui (zero token): "fammi una casetta 5x5 di pietra", "costruisci una torre
//!   alta 10", "costruisci la schematica castello", "duelliamo al meglio di 3", "metti in ordine l'inventario".
//!   Il bot prima fa la schematica mentale (mc_bot/schematic.js) e il piano (build.js), poi costruisce.
//! - Schematiche da file (.schem/.schematic/.litematic/.nbt) nella cartella `minecraft/schematics`: anteprima
//!   con `node schematic.js file` (anche quando Alter Ego non è nel mondo) e costruzione.
//! - Eventi del bot: osservazioni (observe.rs), dubbi da chiedere in chat, duelli (pvp.rs + Rizzo Flow),
//!   piani di costruzione per l'anteprima nella UI, attrezzi consumati da rifare.

use super::{observe, pvp, quick::Quick, rizzo};
use serde_json::{json, Value};
use std::{
    path::{Path, PathBuf},
    sync::Mutex,
    time::{Duration, Instant},
};
use tauri::{AppHandle, Emitter, Manager};

// ---------- richieste a parole ----------

/// Toglie punteggiatura e accenti semplici.
fn norm(text: &str) -> String {
    let t = text.to_lowercase().replace(['!', '?', '.', ',', ';', ':', '"', '\'', '…', '«', '»'], " ").replace(['à', 'á'], "a").replace(['è', 'é'], "e").replace(['ì', 'í'], "i").replace(['ò', 'ó'], "o").replace(['ù', 'ú'], "u");
    format!(" {} ", t.split_whitespace().collect::<Vec<_>>().join(" "))
}

const SHAPES: &[(&str, &str)] = &[
    ("casetta", "house"), ("casa", "house"), ("house", "house"), ("home", "house"), ("capanna", "hut"), ("rifugio", "hut"), ("hut", "hut"), ("shelter", "hut"),
    ("torre", "tower"), ("tower", "tower"), ("muro", "wall"), ("muretto", "wall"), ("wall", "wall"), ("piattaforma", "platform"), ("pavimento", "platform"),
    ("platform", "platform"), ("floor", "platform"), ("ponte", "bridge"), ("bridge", "bridge"), ("pilastro", "pillar"), ("colonna", "pillar"), ("pillar", "pillar"),
];

const MATERIALS: &[(&str, &str)] = &[
    ("mattoni di pietra", "stone_bricks"), ("stone bricks", "stone_bricks"), ("mattoni", "bricks"), ("bricks", "bricks"), ("abete", "spruce_planks"), ("spruce", "spruce_planks"),
    ("betulla", "birch_planks"), ("birch", "birch_planks"), ("quercia scura", "dark_oak_planks"), ("ciliegio", "cherry_planks"), ("cherry", "cherry_planks"),
    ("legno", "oak_planks"), ("legna", "oak_planks"), ("assi", "oak_planks"), ("wood", "oak_planks"), ("planks", "oak_planks"), ("pietra", "cobblestone"),
    ("sasso", "cobblestone"), ("cobble", "cobblestone"), ("stone", "cobblestone"), ("vetro", "glass"), ("glass", "glass"), ("terra", "dirt"), ("dirt", "dirt"),
    ("arenaria", "sandstone"), ("sandstone", "sandstone"), ("deepslate", "cobbled_deepslate"), ("ardesia", "cobbled_deepslate"),
];

/// Numeri: "5x5", "5 x 7", "5 per 5", "alta 10", "lungo 8".
fn sizes(t: &str) -> (Option<u32>, Option<u32>, Option<u32>) {
    let words: Vec<&str> = t.split_whitespace().collect();
    let num = |s: &str| s.parse::<u32>().ok().filter(|n| *n > 0 && *n <= 999);
    let (mut w, mut l, mut h) = (None, None, None);
    for (i, word) in words.iter().enumerate() {
        if let Some((a, b)) = word.split_once('x') {
            if let (Some(a), Some(b)) = (num(a), num(b)) {
                w = Some(a);
                l = Some(b);
                continue;
            }
        }
        if let Some(n) = num(word) {
            let next = words.get(i + 1).copied().unwrap_or("");
            let prev = if i > 0 { words[i - 1] } else { "" };
            if (next == "x" || next == "per" || next == "by") && words.get(i + 2).and_then(|x| num(x)).is_some() {
                w = Some(n);
                l = words.get(i + 2).and_then(|x| num(x));
            } else if ["alta", "alto", "tall", "high", "altezza"].contains(&prev) || ["tall", "high", "blocchi di altezza"].contains(&next) {
                h = Some(n);
            } else if (prev == "x" || prev == "per" || prev == "by") && w.is_some() {
                // già preso come secondo numero
            } else if w.is_none() {
                w = Some(n);
            }
        }
    }
    (w, l, h)
}

/// "fammi una casetta 5x5 di pietra" → comando build (o None se non è una richiesta di costruzione).
pub fn build_request(text: &str) -> Option<Value> {
    let t = norm(text);
    let verbs = [" costruisci", " costruiamo", " costruire", " fammi ", " fai ", " fare ", " crea ", " creami ", " build", " make ", " construct", " tirami su"];
    if !verbs.iter().any(|v| t.contains(v)) || t.contains(" schematic") {
        return None;
    }
    let shape = SHAPES.iter().find(|(w, _)| t.contains(&format!(" {w} ")) || t.contains(&format!(" {w}s "))).map(|x| x.1)?;
    let block = MATERIALS.iter().find(|(w, _)| t.contains(&format!(" {w} ")) || t.contains(&format!(" {w}s "))).map(|x| x.1.to_string());
    let block = block.or_else(|| t.split_whitespace().find(|w| w.contains('_') && w.chars().all(|c| c.is_ascii_lowercase() || c == '_')).map(str::to_string));
    let (w, l, h) = sizes(&t);
    let peaked = ["a punta", "spiovente", "peaked", "pointy", "a capanna"].iter().any(|w| t.contains(w));
    let mut o = json!({ "op": "build", "shape": shape });
    if let Some(w) = w {
        o["w"] = json!(w.min(15));
        o["l"] = json!(l.unwrap_or(w).min(15));
    }
    if let Some(h) = h {
        o["h"] = json!(h.min(20));
    }
    if let Some(b) = block {
        o["block"] = json!(b);
    }
    if peaked {
        o["peaked"] = json!(true);
    }
    Some(o)
}

/// Nome umano della costruzione ("casetta 5x5").
fn build_name(o: &Value, en: bool) -> String {
    let shape = o["shape"].as_str().unwrap_or("house");
    let name = match (shape, en) {
        ("house", false) => "casetta",
        ("hut", false) => "capanna",
        ("tower", false) => "torre",
        ("wall", false) => "muro",
        ("platform", false) => "piattaforma",
        ("bridge", false) => "ponte",
        ("pillar", false) => "pilastro",
        (s, _) => s,
    };
    match (o["w"].as_u64(), o["l"].as_u64()) {
        (Some(w), Some(l)) => format!("{name} {w}x{l}"),
        _ => name.to_string(),
    }
}

/// Schematica per nome nella cartella (il nome del file che contiene più parole della richiesta).
pub fn find_schematic(dir: &Path, name: &str) -> Option<PathBuf> {
    let want: Vec<String> = norm(name).split_whitespace().filter(|w| w.len() >= 2).map(str::to_string).collect();
    let mut best: Option<(usize, PathBuf)> = None;
    for e in std::fs::read_dir(dir).ok()?.flatten() {
        let p = e.path();
        let ext = p.extension().and_then(|x| x.to_str()).unwrap_or("").to_lowercase();
        if !["schem", "schematic", "litematic", "nbt"].contains(&ext.as_str()) {
            continue;
        }
        let stem = norm(&p.file_stem().and_then(|x| x.to_str()).unwrap_or("").replace(['_', '-'], " "));
        let score = want.iter().filter(|w| stem.contains(&format!(" {w} ")) || stem.contains(w.as_str())).count();
        if score > 0 && best.as_ref().is_none_or(|b| score > b.0) {
            best = Some((score, p));
        }
    }
    best.map(|b| b.1)
}

/// Comandi riconosciuti qui (dopo quick.rs, prima del modello). `dir`: cartella delle schematiche.
pub fn find(text: &str, who: &str, en: bool, dir: &Path, cfg: &crate::config::MinecraftConfig) -> Option<Quick> {
    let t = norm(text);
    if t.split_whitespace().count() > 16 {
        return None;
    }
    let pick = |it: &str, e: &str| if en { e.to_string() } else { it.to_string() };
    // schematica da file
    for key in [" schematica ", " schematic ", " schem "] {
        if let Some(i) = t.find(key) {
            if !(t.contains("costru") || t.contains("build") || t.contains("fai") || t.contains("fammi")) {
                break;
            }
            let name = t[i + key.len()..].trim();
            let Some(p) = find_schematic(dir, name) else {
                return Some(Quick { ops: vec![], emotion: "embarrassed", reply: pick(&format!("non trovo la schematica «{name}»: aggiungila dalle impostazioni"), &format!("i can't find the schematic \"{name}\": add it in the settings")) });
            };
            let fname = p.file_name().and_then(|x| x.to_str()).unwrap_or("").to_string();
            return Some(Quick {
                ops: vec![json!({ "op": "build_file", "path": p.display().to_string() })],
                emotion: "happy",
                reply: pick(&format!("ok! guardo prima la schematica {fname}, conto i blocchi e poi costruisco"), &format!("ok! checking the schematic {fname} first, counting blocks, then building")),
            });
        }
    }
    // costruzione a parole
    if let Some(o) = build_request(text) {
        if !cfg.build {
            return None;
        }
        let name = build_name(&o, en);
        return Some(Quick {
            ops: vec![o],
            emotion: "happy",
            reply: pick(&format!("ok! prima la immagino: {name}, conto i blocchi, tolgo quello che c'è in mezzo e costruisco"), &format!("ok! picturing it first: {name}, counting blocks, clearing the spot, then building")),
        });
    }
    // duello
    let duel = [" duelliamo", " duello ", " sfidami", " sfida ", " 1v1 ", " pvp ", " duel ", " duel me", " lets duel", " let s duel", " fight me", " combattiamo", " facciamo a botte"];
    if duel.iter().any(|w| t.contains(w)) && !t.contains(" no ") {
        if !cfg.duels {
            return Some(Quick { ops: vec![], emotion: "embarrassed", reply: pick("niente duelli per ora, sono spenti nelle impostazioni", "no duels for now, they're turned off in the settings") });
        }
        return Some(Quick {
            ops: vec![json!({ "op": "duel", "who": who, "text": text.chars().take(160).collect::<String>() })],
            emotion: "happy",
            reply: pick("duello! mi metto a distanza, aspetta il via", "duel! getting in position, wait for GO"),
        });
    }
    if ["ordine l inventario", "ordina l inventario", "sistema l inventario", "metti in ordine", "sort your inventory", "sort inventory", "tidy up", "clean your inventory"].iter().any(|w| t.contains(w)) {
        return Some(Quick { ops: vec![json!({ "op": "sort" })], emotion: "happy", reply: pick("ok, metto in ordine (la spazzatura in una cassa se c'è)", "ok, tidying up (junk in a chest if there's one)") });
    }
    if ["mettiti l armatura", "metti l armatura", "indossa l armatura", "equip armor", "put on armor", "wear armor", "put your armor on"].iter().any(|w| t.contains(w)) {
        return Some(Quick { ops: vec![json!({ "op": "armor" })], emotion: "happy", reply: pick("armatura su!", "armor on!") });
    }
    None
}

// ---------- stato condiviso ----------

struct Pending {
    who: String,
    key: String,
    question: String,
    at: Instant,
    /// Comandi da mandare dopo la risposta (es. la costruzione in attesa del posto).
    then: Vec<Value>,
}

#[derive(Default)]
struct Ext {
    obs: Option<observe::Store>,
    pvp: Option<pvp::Store>,
    pending: Option<Pending>,
    plan: Value,
    plan_at: i64,
    obs_saved: Option<Instant>,
    graph_at: Option<Instant>,
    habits_sent: Option<Instant>,
    worn: Vec<(String, Instant)>,
}

static EXT: Mutex<Ext> = Mutex::new(Ext { obs: None, pvp: None, pending: None, plan: Value::Null, plan_at: 0, obs_saved: None, graph_at: None, habits_sent: None, worn: Vec::new() });

fn ext() -> std::sync::MutexGuard<'static, Ext> {
    EXT.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn with_obs<R>(dir: &Path, f: impl FnOnce(&mut observe::Store) -> R) -> R {
    let mut e = ext();
    if e.obs.is_none() {
        e.obs = Some(observe::load(dir));
    }
    f(e.obs.as_mut().unwrap())
}

fn with_pvp<R>(dir: &Path, f: impl FnOnce(&mut pvp::Store) -> R) -> R {
    let mut e = ext();
    if e.pvp.is_none() {
        e.pvp = Some(pvp::load(dir));
    }
    f(e.pvp.as_mut().unwrap())
}

fn save_obs(dir: &Path, force: bool) {
    let st = {
        let mut e = ext();
        if !force && e.obs_saved.is_some_and(|t| t.elapsed() < Duration::from_secs(60)) {
            return;
        }
        e.obs_saved = Some(Instant::now());
        e.obs.clone()
    };
    if let Some(st) = st {
        observe::save(dir, &st);
    }
}

fn save_pvp(dir: &Path) {
    if let Some(st) = ext().pvp.clone() {
        pvp::save(dir, &st);
    }
}

fn dir_of(app: &AppHandle) -> PathBuf {
    super::data_dir(app)
}

pub fn schem_dir(app: &AppHandle) -> PathBuf {
    super::node::root(app).join("schematics")
}

fn en(app: &AppHandle) -> bool {
    crate::providers::prompt::reply_lang(&super::cfg_of(app)) == "en"
}

fn owner(app: &AppHandle) -> String {
    let i = super::lock(app);
    i.state.as_ref().map(|s| s.owner.clone()).filter(|o| !o.is_empty()).unwrap_or_else(|| super::cfg_of(app).minecraft.player.trim().to_string())
}

fn inv(app: &AppHandle) -> String {
    super::lock(app).state.as_ref().map(|s| s.inv.clone()).unwrap_or_default()
}

/// Rizzo Flow da usare (indirizzo), se è il router e l'opzione di Minecraft è accesa.
fn rizzo_url(app: &AppHandle) -> Option<String> {
    let cfg = super::cfg_of(app);
    (cfg.minecraft.rizzo && cfg.saving.router == "rizzo" && !cfg.saving.router_url.trim().is_empty()).then(|| cfg.saving.router_url.trim().to_string())
}

fn http_for(app: &AppHandle, url: &str) -> reqwest::Client {
    app.state::<crate::AppState>().ai.client_for(url).clone()
}

/// Ricordo nel grafo della memoria (titolo fisso: si aggiorna invece di moltiplicarsi).
fn to_graph(app: &AppHandle, title: &str, body: &str) {
    if body.trim().is_empty() {
        return;
    }
    let conv = app.state::<crate::assistant::Conversation>();
    if let Some(m) = conv.memory.as_ref() {
        if let Err(e) = m.remember_node(crate::memory::graph::GAME, title, &format!("Minecraft: {body}")) {
            crate::applog::warn("memoria", &format!("profilo Minecraft non salvato nel grafo: {e}"));
        }
    }
}

/// Dice una riga in chat (e sul PC) senza modello.
fn say(app: &AppHandle, text: &str, emotion: &str) {
    super::send(app, vec![json!({ "op": "say", "text": text })]);
    let _ = app.emit("app-say", format!("[{emotion}]\n{text}"));
}

// ---------- eventi ----------

/// Eventi nuovi del bot (chiamato da mod.rs per i tipi che conosce questo modulo).
pub fn on_event(app: &AppHandle, kind: &str, e: &Value) {
    let s = |k: &str| e[k].as_str().unwrap_or("").to_string();
    let dir = dir_of(app);
    let now = super::now_secs();
    match kind {
        "observe" => {
            if !super::cfg_of(app).minecraft.observe {
                return;
            }
            let who = s("who");
            let (habits, line) = with_obs(&dir, |st| {
                let p = st.players.entry(who.clone()).or_default();
                p.merge(&e["s"], now);
                (p.habits(), p.line(&who))
            });
            save_obs(&dir, false);
            // abitudini al bot (al massimo ogni 2 minuti) e nel grafo (ogni 10)
            let (send_h, graph) = {
                let mut x = ext();
                let sh = x.habits_sent.is_none_or(|t| t.elapsed() > Duration::from_secs(120));
                if sh {
                    x.habits_sent = Some(Instant::now());
                }
                let g = x.graph_at.is_none_or(|t| t.elapsed() > Duration::from_secs(600));
                if g {
                    x.graph_at = Some(Instant::now());
                }
                (sh, g)
            };
            if send_h && who == owner(app) {
                super::send(app, vec![json!({ "op": "habits", "h": habits })]);
            }
            if graph && !line.is_empty() {
                to_graph(app, &format!("Minecraft: abitudini di {who}"), &line);
            }
        }
        "observed" => {
            let what = if s("kind") == "crafted" { format!("[you saw {} craft {}]", s("who"), s("item")) } else { format!("[{} took off with an elytra]", s("who")) };
            super::remember(app, &what, "");
        }
        "doubt" => doubt(app, &s("key"), e),
        "duel_need_tactic" => {
            let app = app.clone();
            let ev = e.clone();
            tauri::async_runtime::spawn(async move { tactic(&app, &ev).await });
        }
        "duel_round" => {
            let line = with_pvp(&dir, |st| st.on_round(e, now));
            save_pvp(&dir);
            super::remember(app, &format!("[{line}]"), "");
        }
        "duel_end" => {
            let who = s("who");
            let line = with_pvp(&dir, |st| st.line(&who));
            to_graph(app, &format!("Minecraft: duelli con {who}"), &line);
            super::comment(app, &format!("[event] the duel with {who} is over: you won {} rounds, lost {}. Say gg and a short friendly comment.", e["wins"], e["losses"]), true);
        }
        "plan" => {
            {
                let mut x = ext();
                x.plan = e.clone();
                x.plan_at = now;
            }
            crate::applog::info("minecraft", &format!("piano: {}", s("line")));
            super::remember(app, &format!("[your build plan: {}]", s("line")), "");
            let _ = app.emit("minecraft-plan", now);
        }
        "build_status" if s("phase") == "done" => {
            let missing = e["missing"].as_object().map(|m| m.iter().map(|(k, v)| format!("{v} {k}")).collect::<Vec<_>>().join(", ")).unwrap_or_default();
            super::remember(app, &format!("[build finished: {} blocks placed{}]", e["placed"], if missing.is_empty() { String::new() } else { format!(", missing {missing}") }), "");
        }
        "craft_plan" => super::remember(app, &format!("[craft plan for {}: {}]", s("item"), s("tree")), ""),
        // attrezzi quasi rotti: li rifà prima che si rompano (zero token, al massimo uno ogni 5 minuti per tipo)
        "worn" => {
            if !super::cfg_of(app).minecraft.tidy {
                return;
            }
            let items: Vec<String> = e["items"].as_array().map(|a| a.iter().filter_map(|x| x.as_str().map(str::to_string)).collect()).unwrap_or_default();
            let todo: Vec<Value> = {
                let mut x = ext();
                x.worn.retain(|(_, t)| t.elapsed() < Duration::from_secs(300));
                let mut v = Vec::new();
                for it in items.iter().filter(|n| ["_pickaxe", "_axe", "_sword", "_shovel"].iter().any(|t| n.ends_with(t))).take(1) {
                    if x.worn.iter().any(|(n, _)| n == it) {
                        continue;
                    }
                    x.worn.push((it.clone(), Instant::now()));
                    v.push(json!({ "op": "craft", "item": it, "n": 1 }));
                }
                v
            };
            if !todo.is_empty() {
                super::send(app, todo);
            }
        }
        _ => {}
    }
}

/// Un dubbio del bot: risposta già nota, domanda in chat, o niente (il bot usa il suo valore di riserva).
fn doubt(app: &AppHandle, key: &str, e: &Value) {
    let dir = dir_of(app);
    let now = super::now_secs();
    let who = e["who"].as_str().map(str::to_string).filter(|w| !w.is_empty()).unwrap_or_else(|| owner(app));
    let known = with_obs(&dir, |st| st.answer(&who, key).map(|a| a.value.clone()));
    if let Some(v) = known {
        super::send(app, vec![json!({ "op": "answer", "key": key, "value": v })]);
        return;
    }
    let cfg = super::cfg_of(app);
    let q = observe::question(key, e, en(app));
    let can = cfg.minecraft.ask && q.is_some() && with_obs(&dir, |st| st.may_ask(&who, key, now));
    if !can {
        super::send(app, vec![json!({ "op": "answer", "key": key, "value": Value::Null })]);
        return;
    }
    let q = q.unwrap_or_default();
    with_obs(&dir, |st| st.mark_asked(&who, key, now));
    ext().pending = Some(Pending { who: who.clone(), key: key.to_string(), question: q.clone(), at: Instant::now(), then: Vec::new() });
    say(app, &q, "thinking");
}

/// Chiede il posto prima di costruire (solo la prima volta con questo giocatore). true = ha chiesto (la
/// costruzione parte dopo la risposta).
pub fn ask_build_spot(app: &AppHandle, who: &str, ops: &[Value]) -> bool {
    let cfg = super::cfg_of(app);
    if !cfg.minecraft.ask || !ops.iter().any(|o| o["op"] == "build" || o["op"] == "build_file") {
        return false;
    }
    let dir = dir_of(app);
    let now = super::now_secs();
    if !with_obs(&dir, |st| st.may_ask(who, "build_spot", now)) {
        return false;
    }
    let q = observe::question("build_spot", &Value::Null, en(app)).unwrap_or_default();
    with_obs(&dir, |st| st.mark_asked(who, "build_spot", now));
    ext().pending = Some(Pending { who: who.to_string(), key: "build_spot".into(), question: q.clone(), at: Instant::now(), then: ops.to_vec() });
    say(app, &q, "thinking");
    true
}

/// Il messaggio in chat è la risposta a una domanda di Alter Ego? Se sì la usa e la ricorda (true = gestito).
pub fn take_answer(app: &AppHandle, from: &str, text: &str) -> bool {
    let (key, question) = {
        let x = ext();
        match x.pending.as_ref() {
            Some(p) if p.at.elapsed() < Duration::from_secs(120) && (p.who.is_empty() || p.who == from) => (p.key.clone(), p.question.clone()),
            _ => return false,
        }
    };
    if let Some(v) = observe::interpret(&key, text) {
        answered(app, from, &key, v, text);
        return true;
    }
    // non è una risposta chiara: Rizzo Flow (se c'è) decide; altrimenti è un messaggio normale
    let Some(url) = rizzo_url(app) else { return false };
    if key != "can_break" {
        return false;
    }
    let app = app.clone();
    let (from, text) = (from.to_string(), text.to_string());
    tauri::async_runtime::spawn(async move {
        let http = http_for(&app, &url);
        let res = rizzo::ask(&http, &url, json!({ "question": question, "reply": text }), &rizzo::answer_questions(&question, &[])).await;
        match res.as_ref().and_then(|v| rizzo::noul(v, "yes")) {
            Some(p) if p >= 0.65 => answered(&app, &from, &key, json!(true), &text),
            Some(p) if p <= 0.35 => answered(&app, &from, &key, json!(false), &text),
            _ => {
                if let Ok(r) = super::handle(&app, &from, &text, None).await {
                    let _ = app.emit("app-say", r);
                }
            }
        }
    });
    true
}

fn answered(app: &AppHandle, from: &str, key: &str, v: Value, text: &str) {
    let dir = dir_of(app);
    let now = super::now_secs();
    let then = ext().pending.take().map(|p| p.then).unwrap_or_default();
    with_obs(&dir, |st| st.store_answer(from, key, v.clone(), text, now));
    save_obs(&dir, true);
    let en = en(app);
    let ok = if en { "ok, got it! i'll remember" } else { "ok, capito! me lo ricordo" };
    say(app, ok, "happy");
    super::remember(app, &format!("{from}: {text}"), &format!("[happy]\n{ok}"));
    if key == "build_spot" {
        if v == json!("base") {
            say(app, if en { "i don't know where the base is yet, so i'll build right here" } else { "non so ancora dov'è la base, costruisco qui" }, "embarrassed");
        }
        if !then.is_empty() {
            super::send(app, then);
        }
    } else {
        super::send(app, vec![json!({ "op": "answer", "key": key, "value": v })]);
        if key == "duel_rules" {
            with_pvp(&dir, |st| st.opponents.entry(from.to_string()).or_default().rules = text.chars().take(80).collect());
            save_pvp(&dir);
        }
    }
    let line = with_obs(&dir, |st| st.players.get(from).map(|p| p.line(from)).unwrap_or_default());
    to_graph(app, &format!("Minecraft: abitudini di {from}"), &line);
}

/// Tattica per il prossimo round: euristica (UCB + counter), oppure Rizzo Flow in una chiamata con le altre domande.
async fn tactic(app: &AppHandle, e: &Value) {
    let dir = dir_of(app);
    let who = e["who"].as_str().unwrap_or("").to_string();
    let mut have = pvp::Have::from_inv(&inv(app));
    // stile preferito dal giocatore ("solo spada"): niente scudo e arco
    if with_obs(&dir, |st| st.answer(&who, "fight_style").map(|a| a.value == json!("sword"))).unwrap_or(false) {
        have.shield = false;
        have.bow = false;
    }
    let opp = with_pvp(&dir, |st| st.opponents.get(&who).cloned().unwrap_or_default());
    let rng = crate::gmodlink::persona::Rng::new().f() as f64;
    let mut pick = pvp::choose(&opp, &have, rng, 0.12);
    let mut mimic = opp.mimic.clone();
    let mut ask_style = false;
    if let Some(url) = rizzo_url(app) {
        let http = http_for(app, &url);
        let state = json!({ "opponent": who, "round": e["round"], "record": format!("{}-{}", opp.wins, opp.losses), "opponent_style": opp.style.line(), "last_round_style": e["style"], "heuristic_pick": pick });
        if let Some(v) = rizzo::ask(&http, &url, state, &rizzo::tactic_questions(&pvp::options(&opp, &have))).await {
            if let Some((c, p)) = rizzo::choice(&v, "tactic") {
                if p >= 0.35 && pvp::TACTICS.iter().any(|(k, _)| *k == c) {
                    pick = c;
                }
            }
            if rizzo::noul(&v, "mimic").is_some_and(|p| p < 0.3) {
                mimic = None;
            }
            ask_style = rizzo::noul(&v, "ask_style").is_some_and(|p| p >= 0.6);
        }
    } else {
        // senza Rizzo: chiede lo stile dopo il primo round con un avversario nuovo
        ask_style = opp.rounds == 1;
    }
    crate::applog::info("minecraft", &format!("duello con {who}: tattica {pick}"));
    let m = mimic.map(|m| json!({ "dist": m.dist, "strafe": m.strafe }));
    super::send(app, vec![json!({ "op": "duel_tactic", "tactic": pick, "mimic": m })]);
    if ask_style && super::cfg_of(app).minecraft.ask {
        let now = super::now_secs();
        if with_obs(&dir, |st| st.may_ask(&who, "fight_style", now)) {
            if let Some(q) = observe::question("fight_style", &Value::Null, en(app)) {
                with_obs(&dir, |st| st.mark_asked(&who, "fight_style", now));
                ext().pending = Some(Pending { who: who.clone(), key: "fight_style".into(), question: q.clone(), at: Instant::now(), then: Vec::new() });
                say(app, &q, "thinking");
            }
        }
    }
}

/// Righe in più per il prompt: abitudini e duelli di chi parla (vuoto se non c'è niente).
pub fn context(app: &AppHandle, who: &str) -> String {
    if who.is_empty() {
        return String::new();
    }
    let dir = dir_of(app);
    let a = with_obs(&dir, |st| st.players.get(who).map(|p| p.line(who)).unwrap_or_default());
    let b = with_pvp(&dir, |st| st.line(who));
    let mut out = String::new();
    if !a.is_empty() {
        out.push_str(&format!("[what you learned watching {who}] {a}"));
    }
    if !b.is_empty() {
        if !out.is_empty() {
            out.push('\n');
        }
        out.push_str(&format!("[{b}]"));
    }
    out
}

/// Dimentica profili osservati e di combattimento.
pub fn forget(dir: &Path) {
    observe::forget(dir);
    pvp::forget(dir);
    let mut x = ext();
    x.obs = Some(observe::Store::default());
    x.pvp = Some(pvp::Store::default());
    x.pending = None;
}

// ---------- comandi per le impostazioni ----------

/// Anteprima di un file di schematica con `node schematic.js` (materiali, misure, blocchi per la vista).
fn preview_file(app: &AppHandle, p: &Path) -> Result<Value, String> {
    let custom = super::cfg_of(app).minecraft.node.clone();
    let node = super::node::find(app, &custom).ok_or_else(|| crate::i18n::tr("Node.js non trovato: premi «Installa» qui sotto", "Node.js not found: press \"Install\" below").to_string())?;
    let dir = super::node::bot_dir(app);
    super::node::write_bot(&dir)?;
    if !dir.join("node_modules").join("prismarine-nbt").is_dir() {
        return Err(crate::i18n::tr("il bot va aggiornato: premi «Installa / aggiorna»", "the bot needs an update: press \"Install / update\"").into());
    }
    let out = super::node::hidden(std::process::Command::new(&node.path).arg("schematic.js").arg(p).current_dir(&dir)).stdin(std::process::Stdio::null()).stderr(std::process::Stdio::null()).output().map_err(|e| e.to_string())?;
    let line = String::from_utf8_lossy(&out.stdout);
    let v: Value = serde_json::from_str(line.trim()).map_err(|_| crate::i18n::tr("schematica non leggibile", "unreadable schematic").to_string())?;
    if v["ok"] != json!(true) {
        return Err(v["err"].as_str().unwrap_or("?").to_string());
    }
    Ok(v)
}

fn safe_name(name: &str) -> String {
    let p = Path::new(name);
    let stem: String = p.file_stem().and_then(|x| x.to_str()).unwrap_or("schematica").chars().filter(|c| c.is_alphanumeric() || *c == '_' || *c == '-' || *c == ' ').take(60).collect();
    let ext = p.extension().and_then(|x| x.to_str()).unwrap_or("").to_lowercase();
    format!("{}.{ext}", if stem.trim().is_empty() { "schematica".into() } else { stem.trim().to_string() })
}

/// Salva una schematica scelta nella UI (base64) e ne restituisce l'anteprima.
#[tauri::command]
pub async fn minecraft_schematic_load(app: AppHandle, name: String, data: String) -> Result<Value, String> {
    use base64::Engine;
    let ext = Path::new(&name).extension().and_then(|x| x.to_str()).unwrap_or("").to_lowercase();
    if !["schem", "schematic", "litematic", "nbt"].contains(&ext.as_str()) {
        return Err(crate::i18n::tr("formato non supportato (.schem, .schematic, .litematic, .nbt)", "unsupported format (.schem, .schematic, .litematic, .nbt)").into());
    }
    let b64 = data.split_once(',').map(|x| x.1).unwrap_or(&data);
    let bytes = base64::engine::general_purpose::STANDARD.decode(b64.trim()).map_err(|_| crate::i18n::tr("file non leggibile", "unreadable file").to_string())?;
    if bytes.len() > 16 * 1024 * 1024 {
        return Err(crate::i18n::tr("file troppo grande (massimo 16 MB)", "file too big (max 16 MB)").into());
    }
    let dir = schem_dir(&app);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let p = dir.join(safe_name(&name));
    std::fs::write(&p, &bytes).map_err(|e| e.to_string())?;
    let a = app.clone();
    let mut v = tauri::async_runtime::spawn_blocking(move || preview_file(&a, &p)).await.map_err(|e| e.to_string())??;
    v["file"] = json!(safe_name(&name));
    Ok(v)
}

/// Schematiche salvate (nomi dei file).
#[tauri::command]
pub fn minecraft_schematics(app: AppHandle) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(schem_dir(&app))
        .map(|r| r.flatten().filter_map(|e| e.file_name().to_str().map(str::to_string)).filter(|n| ["schem", "schematic", "litematic", "nbt"].iter().any(|x| n.to_lowercase().ends_with(&format!(".{x}")))).collect())
        .unwrap_or_default();
    v.sort();
    v
}

/// Anteprima di una schematica salvata.
#[tauri::command]
pub async fn minecraft_schematic_preview(app: AppHandle, file: String) -> Result<Value, String> {
    let p = schem_dir(&app).join(safe_name(&file));
    let a = app.clone();
    tauri::async_runtime::spawn_blocking(move || preview_file(&a, &p)).await.map_err(|e| e.to_string())?
}

/// Costruisce una schematica salvata (Alter Ego deve essere nel mondo).
#[tauri::command]
pub fn minecraft_schematic_build(app: AppHandle, file: String) -> Result<(), String> {
    if !super::active(&app) {
        return Err(crate::i18n::tr("Alter Ego non è nel mondo", "Alter Ego isn't in the world").into());
    }
    let p = schem_dir(&app).join(safe_name(&file));
    if !p.is_file() {
        return Err(crate::i18n::tr("file della schematica non trovato", "schematic file not found").into());
    }
    super::send(&app, vec![json!({ "op": "build_file", "path": p.display().to_string() })]);
    Ok(())
}

/// Ultimo piano di costruzione (anteprima, materiali, ostacoli) e righe di quello che ha imparato.
#[tauri::command]
pub fn minecraft_plan(app: AppHandle) -> Value {
    let dir = dir_of(&app);
    let people: Vec<String> = with_obs(&dir, |st| st.players.iter().map(|(k, p)| p.line(k)).filter(|l| !l.is_empty()).collect());
    let duels: Vec<String> = with_pvp(&dir, |st| st.opponents.keys().map(|k| st.line(k)).collect());
    let x = ext();
    json!({ "plan": x.plan, "at": x.plan_at, "people": people, "duels": duels })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_requests_in_words() {
        assert_eq!(build_request("fammi una casetta 5x5"), Some(json!({ "op": "build", "shape": "house", "w": 5, "l": 5 })));
        assert_eq!(build_request("Alter Ego, costruisci una casa 7 per 5 di pietra con il tetto a punta!"), Some(json!({ "op": "build", "shape": "house", "w": 7, "l": 5, "block": "cobblestone", "peaked": true })));
        assert_eq!(build_request("costruiamo una torre alta 10 di mattoni di pietra"), Some(json!({ "op": "build", "shape": "tower", "h": 10, "block": "stone_bricks" })));
        assert_eq!(build_request("build a 9x9 platform out of oak_planks"), Some(json!({ "op": "build", "shape": "platform", "w": 9, "l": 9, "block": "oak_planks" })));
        assert_eq!(build_request("make a house 99x99")
            .map(|v| (v["w"].clone(), v["l"].clone())), Some((json!(15), json!(15))), "misure tagliate");
        assert_eq!(build_request("che bella casa"), None, "senza verbo non è una richiesta");
        assert_eq!(build_request("costruisci la schematica castello"), None, "le schematiche da file a parte");
        assert_eq!(build_request("fai qualcosa"), None);
    }

    #[test]
    fn quick_plans_and_schematics() {
        let dir = std::env::temp_dir().join(format!("ae-mc-plan-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("Castello_Medievale.schem"), b"x").unwrap();
        std::fs::write(dir.join("casa-moderna.litematic"), b"x").unwrap();
        std::fs::write(dir.join("note.txt"), b"x").unwrap();
        let cfg = crate::config::MinecraftConfig::default();
        let q = find("costruisci la schematica castello", "Makoto", false, &dir, &cfg).unwrap();
        assert!(q.ops[0]["op"] == "build_file" && q.ops[0]["path"].as_str().unwrap().ends_with("Castello_Medievale.schem"));
        assert!(find("build the schematic modern house casa moderna", "M", true, &dir, &cfg).unwrap().ops[0]["path"].as_str().unwrap().ends_with(".litematic"));
        let miss = find("costruisci la schematica astronave", "M", false, &dir, &cfg).unwrap();
        assert!(miss.ops.is_empty() && miss.reply.contains("astronave"));
        assert_eq!(find_schematic(&dir, "note"), None, "solo file di schematica");
        let b = find("fammi una casetta 5x5", "M", false, &dir, &cfg).unwrap();
        assert!(b.reply.contains("casetta 5x5") && b.ops[0]["op"] == "build");
        let d = find("duelliamo al meglio di 3!", "Makoto", false, &dir, &cfg).unwrap();
        assert_eq!(d.ops[0]["op"], "duel");
        assert_eq!(d.ops[0]["who"], "Makoto");
        assert!(d.ops[0]["text"].as_str().unwrap().contains("meglio di 3"));
        let off = crate::config::MinecraftConfig { duels: false, ..Default::default() };
        assert!(find("1v1?", "M", true, &dir, &off).unwrap().ops.is_empty(), "duelli spenti: lo dice");
        assert_eq!(find("metti in ordine l'inventario", "M", false, &dir, &cfg).unwrap().ops[0]["op"], "sort");
        assert_eq!(find("put your armor on", "M", true, &dir, &cfg).unwrap().ops[0]["op"], "armor");
        assert!(find("ciao come stai", "M", false, &dir, &cfg).is_none());
        let nob = crate::config::MinecraftConfig { build: false, ..Default::default() };
        assert!(find("fammi una casetta", "M", false, &dir, &nob).is_none());
        assert_eq!(safe_name("../../etc/Castello!.schem"), "Castello.schem");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
