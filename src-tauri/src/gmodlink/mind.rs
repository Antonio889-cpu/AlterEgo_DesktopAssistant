//! Collega personalità (persona.rs) e obiettivi lunghi al resto di gmodlink:
//! - scherzi e attività: rifà un'abitudine (zero token) o chiede al modello di inventarne una nuova,
//!   poi per 25 secondi guarda come reagiscono i giocatori e aggiorna le preferenze;
//! - obiettivi lunghi ([goal …]): finché non sono raggiunti chiede al modello il passo successivo
//!   ogni volta che il passo prima è finito (Rizzo Flow, se c'è, controlla prima se è già fatto).

use super::persona::{self, Choice, Persona};
use super::{cfg_of, comment, data_dir, en, lock, remember, send, Link};
use serde_json::{json, Value};
use std::collections::VecDeque;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Manager};

const TRIAL_SECS: u64 = 25;
const GOAL_MAX_STEPS: u32 = 40;
const GOAL_MAX_MINUTES: u64 = 90;

#[derive(Default)]
pub struct Mind {
    persona: Option<Persona>,
    /// Il modello deve inventare qualcosa: (tipo, vittima, quando).
    pub invent: Option<(String, String, Instant)>,
    /// Prova in corso: come reagiscono a un'abitudine.
    pub trial: Option<Trial>,
    pub goal: Option<Goal>,
    /// Azioni nell'ultima risposta del modello (per capire se un passo ha fatto qualcosa).
    pub last_ops: usize,
    rng: Option<persona::Rng>,
}

pub struct Trial {
    pub id: String,
    pub who: String,
    pub lines: Vec<String>,
    pub hits: u32,
    pub failed: bool,
}

pub struct Goal {
    pub text: String,
    pub steps: u32,
    pub log: VecDeque<String>,
    pub started: Instant,
    pub last: Instant,
    pub empty: u32,
    pub busy: bool,
}

fn mind(app: &AppHandle) -> std::sync::MutexGuard<'_, Mind> {
    app.state::<Link>().inner().mind.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn now() -> i64 {
    super::now_secs() as i64
}

fn with_persona<R>(app: &AppHandle, f: impl FnOnce(&mut Persona) -> R) -> R {
    let dir = data_dir(app);
    let mut m = mind(app);
    if m.persona.is_none() {
        m.persona = Some(persona::load(&dir));
    }
    f(m.persona.as_mut().unwrap())
}

fn save_persona(app: &AppHandle) {
    let dir = data_dir(app);
    let p = mind(app).persona.clone();
    if let Some(p) = p {
        persona::save(&dir, &p);
    }
}

/// Per le impostazioni e il monitor.
pub fn view(app: &AppHandle) -> Value {
    let (traits, top, exp) = with_persona(app, |p| (p.traits.clone(), p.top(6), p.experiences));
    let m = mind(app);
    let goal = m.goal.as_ref().map(|g| json!({ "text": g.text, "steps": g.steps, "last": g.log.back() }));
    json!({ "traits": traits, "habits": top, "experiences": exp, "goal": goal })
}

pub fn forget(app: &AppHandle) {
    persona::forget(&data_dir(app));
    mind(app).persona = Some(Persona::default());
}

// ---------- scherzi e attività ----------

/// L'addon ha voglia di fare uno scherzo (`kind` "prank") o di giocare ("play").
pub fn want(app: &AppHandle, kind: &str, who: &str, why: &str, recent: &str) {
    let cfg = cfg_of(app);
    let g = &cfg.gmodlink;
    if mind(app).goal.is_some() && kind == "play" {
        return; // ha già da fare
    }
    let chaos = g.unpredictability as f32 / 100.0;
    let reuse = if g.grow {
        let mut m = mind(app);
        let mut rng = m.rng.take().unwrap_or_default();
        drop(m);
        let pick = with_persona(app, |p| match p.choose(kind, chaos, g.tasks, now(), &mut rng) {
            Choice::Reuse(h) => Some(h.clone()),
            Choice::Invent => None,
        });
        mind(app).rng = Some(rng);
        pick
    } else {
        None
    };
    if let Some(h) = reuse {
        // un classico: zero token
        let target = if who.is_empty() { host(app) } else { who.to_string() };
        send(app, persona::ops_for(&h, &target));
        let it = !en(app);
        let line = match (kind, it) {
            ("prank", true) => ["Ehehe, questa la conosco!", "Il mio classico!", "Pronto per un déjà-vu?"][(now() % 3) as usize],
            ("prank", false) => ["Hehe, I know this one!", "My classic!", "Ready for a déjà vu?"][(now() % 3) as usize],
            (_, true) => ["Ho un'idea!", "Guarda qua!", "Rifacciamolo!"][(now() % 3) as usize],
            _ => ["I've got an idea!", "Watch this!", "Let's do it again!"][(now() % 3) as usize],
        };
        super::speak(app, "happy", line);
        start_trial(app, &h.id, &target);
        return;
    }
    // qualcosa di nuovo: lo inventa il modello
    let summary = if g.grow { with_persona(app, |p| p.summary(kind)) } else { String::new() };
    mind(app).invent = Some((kind.to_string(), who.to_string(), Instant::now()));
    let ask = if kind == "prank" {
        format!(
            "[event] {who} keeps messing with you ({why}). Get them back with a prank YOU invent: creative and surprising{}. \
Prefer a short Lua [task good_name] using `target` (spawn_at, push, freeze, color, scale, place, boom…), or [act] lines. {summary} Say one playful line.",
            if recent.is_empty() { String::new() } else { format!(", not like what you already did ({recent})") }
        )
    } else {
        format!(
            "[event] nothing is happening and you're bored: invent something fun to do right now (build something silly, a mini-game, \
a harmless prank on a friend…), as a short Lua [task good_name] or [act] lines. {summary} Say one line about it."
        )
    };
    comment(app, &ask, kind == "prank");
}

fn host(app: &AppHandle) -> String {
    lock(app).state.as_ref().and_then(|s| s.players.iter().find(|p| p.host).map(|p| p.name.clone())).unwrap_or_default()
}

/// Dopo una risposta del modello: se stava inventando, l'invenzione diventa un'abitudine (e si prova).
pub fn after_reply(app: &AppHandle, event: bool, ops: &mut [Value]) {
    mind(app).last_ops = ops.len();
    if !event {
        return;
    }
    let Some((kind, who, at)) = mind(app).invent.take() else { return };
    if at.elapsed() > Duration::from_secs(90) || ops.is_empty() {
        return;
    }
    let target = if who.is_empty() { host(app) } else { who };
    for o in ops.iter_mut() {
        if o["op"] == "task" {
            o["target"] = json!(target);
        }
    }
    if !cfg_of(app).gmodlink.grow {
        return;
    }
    let task = ops.iter().find(|o| o["op"] == "task");
    let (id, code, acts) = match task {
        Some(t) => (t["name"].as_str().unwrap_or("idea").to_string(), t["code"].as_str().unwrap_or("").to_string(), Vec::new()),
        None => {
            let acts: Vec<Value> = ops.iter().filter(|o| o["op"] != "say").cloned().collect();
            let name = acts.iter().filter_map(|o| o["op"].as_str()).take(3).collect::<Vec<_>>().join("_");
            (format!("{kind}_{name}"), String::new(), persona::generalize(&acts, &target))
        }
    };
    with_persona(app, |p| p.add(&kind, &id, &code, acts, now()));
    save_persona(app);
    start_trial(app, &id, &target);
}

fn start_trial(app: &AppHandle, id: &str, who: &str) {
    mind(app).trial = Some(Trial { id: id.into(), who: who.into(), lines: Vec::new(), hits: 0, failed: false });
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_secs(TRIAL_SECS)).await;
        let Some(t) = mind(&app).trial.take() else { return };
        let cfg = cfg_of(&app);
        let react = if t.lines.is_empty() {
            0.0
        } else if cfg.saving.router == "rizzo" {
            match persona::rizzo_enjoyed(&cfg.saving.router_url, &t.id, &t.lines).await {
                Some(p) => (p - 0.5) * 2.0,
                None => persona::keyword_reaction(&t.lines),
            }
        } else {
            persona::keyword_reaction(&t.lines)
        };
        // è andata a buon fine (+), ha fatto ridere (+), si sono arrabbiati (-), hanno risposto al fuoco (un po' +: si gioca)
        let reward = if t.failed { -0.6 } else { (0.15 + 0.85 * react + 0.1 * t.hits.min(2) as f32).clamp(-1.0, 1.0) };
        let note = with_persona(&app, |p| p.learn(&t.id, reward, t.failed, now()));
        save_persona(&app);
        crate::applog::info("gmodlink", &format!("abitudine {}: reazione {react:.2}, ricompensa {reward:.2}", t.id));
        if let Some(n) = note {
            remember(&app, &n, "");
        }
        let _ = t.who;
    });
}

/// Eventi dal gioco utili alla prova in corso e all'obiettivo.
pub fn on_event(app: &AppHandle, e: &Value) {
    let s = |k: &str| e[k].as_str().unwrap_or("").to_string();
    let mut m = mind(app);
    match e["kind"].as_str().unwrap_or("") {
        "chat" => {
            if let Some(t) = m.trial.as_mut() {
                if t.lines.len() < 8 {
                    t.lines.push(format!("{}: {}", s("from"), s("text").chars().take(140).collect::<String>()));
                }
            }
        }
        "hurt" => {
            if let Some(t) = m.trial.as_mut() {
                if s("by") == t.who {
                    t.hits += 1;
                }
            }
        }
        "task" => {
            let (name, status) = (s("name"), s("status"));
            if let Some(t) = m.trial.as_mut() {
                if t.id == name && status == "error" {
                    t.failed = true;
                }
            }
            if let Some(g) = m.goal.as_mut() {
                let line = match status.as_str() {
                    "done" => format!("task {name} finished"),
                    "error" => format!("task {name} FAILED: {}", s("error").chars().take(120).collect::<String>()),
                    "timeout" | "cancelled" => format!("task {name} {status}"),
                    _ => String::new(),
                };
                if !line.is_empty() {
                    g.log.push_back(line);
                    while g.log.len() > 8 {
                        g.log.pop_front();
                    }
                }
            }
        }
        "notify" => {
            if let Some(g) = m.goal.as_mut() {
                g.log.push_back(format!("{} says: {}", s("task"), s("text").chars().take(120).collect::<String>()));
                while g.log.len() > 8 {
                    g.log.pop_front();
                }
            }
        }
        _ => {}
    }
}

// ---------- obiettivi lunghi ----------

/// La risposta del modello ha una riga [goal …] (o un "stop").
pub fn goal_line(app: &AppHandle, goal: Option<&str>, ops: &[Value]) {
    if ops.iter().any(|o| o["op"] == "stop") {
        if let Some(g) = mind(app).goal.take() {
            remember(app, &format!("[you stopped your goal: {}]", g.text), "");
        }
        return;
    }
    let Some(goal) = goal else { return };
    match goal {
        "done" => finish(app, true),
        "drop" => finish(app, false),
        text => {
            if !cfg_of(app).gmodlink.goals {
                return;
            }
            let mut m = mind(app);
            if m.goal.as_ref().is_some_and(|g| g.text == text) {
                return;
            }
            m.goal = Some(Goal { text: text.into(), steps: 0, log: VecDeque::new(), started: Instant::now(), last: Instant::now(), empty: 0, busy: false });
            drop(m);
            crate::applog::info("gmodlink", &format!("nuovo obiettivo: {text}"));
        }
    }
}

fn finish(app: &AppHandle, done: bool) {
    let Some(g) = mind(app).goal.take() else { return };
    crate::applog::info("gmodlink", &format!("obiettivo {}: {}", if done { "raggiunto" } else { "lasciato" }, g.text));
    remember(app, &format!("[goal {}: {} ({} steps)]", if done { "achieved" } else { "dropped" }, g.text, g.steps), "");
    if let Some(log) = lock(app).session.as_mut() {
        log.event(&json!({ "kind": "goal", "text": g.text, "done": done }));
    }
    if done {
        with_persona(app, |p| {
            p.traits.curiosity = (p.traits.curiosity + 0.02).min(0.95);
            p.experiences += 1;
        });
        save_persona(app);
    }
}

/// Chiamato dal giro principale: se l'obiettivo aspetta il passo successivo, lo chiede.
pub fn goal_tick(app: &AppHandle) {
    let tasks_running = lock(app).state.as_ref().is_some_and(|s| !s.tasks.is_empty());
    let in_game = lock(app).state.as_ref().is_some_and(|s| s.bot.is_some());
    let mut m = mind(app);
    let Some(g) = m.goal.as_mut() else { return };
    if g.busy || tasks_running || !in_game {
        return;
    }
    let gap = if g.steps == 0 { 4 } else { 6 };
    if g.last.elapsed() < Duration::from_secs(gap) {
        return;
    }
    if g.steps >= GOAL_MAX_STEPS || g.started.elapsed() > Duration::from_secs(GOAL_MAX_MINUTES * 60) || g.empty >= 3 {
        drop(m);
        finish(app, false);
        return;
    }
    g.busy = true;
    g.steps += 1;
    let (text, step, log) = (g.text.clone(), g.steps, g.log.iter().cloned().collect::<Vec<_>>().join("; "));
    drop(m);
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let cfg = cfg_of(&app);
        // Rizzo Flow (senza token): è già fatto?
        if step > 2 && cfg.saving.router == "rizzo" {
            let st = lock(&app).state.as_ref().map(super::prompt::state_line).unwrap_or_default();
            if let Some(p) = persona::rizzo_goal_done(&cfg.saving.router_url, &text, &log, &st).await {
                if p >= 0.8 {
                    finish(&app, true);
                    comment(&app, &format!("[event] you just completed your goal \"{text}\". Celebrate in one short line."), true);
                    return;
                }
            }
        }
        let ev = format!(
            "[goal step {step}/{GOAL_MAX_STEPS}] Your long-term goal: {text}. Progress so far: {}. Look at the state and do the NEXT step only: \
preferably one Lua [task] that keeps going until that step is complete (spawn things, then arrange them with place(); explore with explore()…). \
When the whole goal is achieved write [goal done]; if it's impossible write [goal drop]. Talk: one short line or nothing.",
            if log.is_empty() { "just started".to_string() } else { log }
        );
        let r = super::handle(&app, "", "", Some(&ev)).await;
        let mut m = mind(&app);
        let idle = r.is_err() || m.last_ops == 0;
        if let Some(g) = m.goal.as_mut() {
            g.busy = false;
            g.last = Instant::now();
            g.empty = if idle { g.empty + 1 } else { 0 };
        }
    });
}
