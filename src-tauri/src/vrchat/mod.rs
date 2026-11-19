//! Alter Ego gioca a VRChat come un giocatore: account dedicato in modalità desktop sullo stesso PC,
//! guidato via OSC (UDP 127.0.0.1:9000 verso VRChat, 9001 da VRChat).
//!
//! - Corpo: /input/* (camminare, girarsi, saltare) con il cane da guardia di input.rs: tutto torna a
//!   zero alla fine di ogni passo, a stop, errori, panico del thread e uscita dall'app.
//! - Sensi senza vista: parametri dei contatti/PhysBones dell'avatar → cervello di moscerino
//!   (riflessi ed emozioni, zero token); udito dall'audio di VRChat (loopback) → VAD → trascrizione.
//! - Vista facoltativa: screenshot piccolo della finestra → descrizione corta (Ollama locale o cloud).
//! - Voce: Mic-Link (microfono virtuale da scegliere in VRChat) + chatbox (/chatbox/input, 144 caratteri).
//! - Cervello: comandi rapidi a parole chiave (zero token), il modello predefinito solo quando le
//!   parlano o succede qualcosa, con prompt corto in cache e stato in una riga.
//!
//! VRChat vieta l'uso automatizzato nei Termini (13.2): l'utente lo accetta una volta nelle impostazioni.

pub mod api;
pub mod avatars;
pub mod behave;
pub mod chatbox;
pub mod hearing;
pub mod input;
pub mod osc;
pub mod prompt;
pub mod quick;
pub mod quips;
pub mod senses;
pub mod vad;
pub mod vision;

use crate::{update_config, AppState};
use osc::{Arg, Msg};
use serde::Serialize;
use serde_json::{json, Value};
use std::{
    collections::{HashMap, VecDeque},
    net::{SocketAddr, UdpSocket},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc, Mutex,
    },
    time::{Duration, Instant},
};
use tauri::{AppHandle, Emitter, Manager};

enum Cmd {
    Ops(Vec<Value>),
    /// Testo per la chatbox (si divide in pagine).
    Say(String),
    Typing(bool),
    Stop,
}

pub struct Vrc {
    inner: Mutex<Inner>,
    pub ai: crate::providers::Ai,
    /// Client a parte per le occhiate e le battute: non interrompono le risposte.
    pub look_ai: crate::providers::Ai,
    pub quip_ai: crate::providers::Ai,
    busy: tokio::sync::Mutex<()>,
    ears: Arc<hearing::Ears>,
    tx: Mutex<Option<mpsc::Sender<Cmd>>>,
    running: AtomicBool,
    /// Indirizzo OSC di VRChat (per l'azzeramento d'emergenza all'uscita).
    target: Mutex<Option<SocketAddr>>,
    quips: Mutex<quips::Quips>,
    /// Sessione dell'API in attesa del secondo fattore.
    pending: Mutex<Option<api::Session>>,
}

impl Default for Vrc {
    fn default() -> Self {
        Self {
            inner: Mutex::new(Inner::default()),
            ai: crate::providers::Ai::with_kind("vrchat"),
            look_ai: crate::providers::Ai::with_kind("vrchat"),
            quip_ai: crate::providers::Ai::with_kind("vrchat"),
            busy: tokio::sync::Mutex::new(()),
            ears: Arc::new(hearing::Ears::default()),
            tx: Mutex::new(None),
            running: AtomicBool::new(false),
            target: Mutex::new(None),
            quips: Mutex::new(quips::Quips::default()),
            pending: Mutex::new(None),
        }
    }
}

#[derive(Default)]
struct Inner {
    last_rx: Option<Instant>,
    greeted: bool,
    avatar: Option<avatars::Avatar>,
    avatar_id: String,
    params: HashMap<String, f32>,
    mode: String,
    emotion: String,
    inputs: Vec<&'static str>,
    /// Frasi sentite (istanti, per "voci nell'ultimo minuto").
    voices: VecDeque<Instant>,
    voice_seq: u64,
    last_voice: Option<(u64, f32, f32)>,
    heard: VecDeque<String>,
    sight: Option<(Instant, vision::Sight)>,
    sight_seq: u64,
    look_now: bool,
    looking: bool,
    vision_error: Option<String>,
    last_touch: Option<(String, Instant)>,
    history: VecDeque<(String, String)>,
    talking_with: Option<Instant>,
    last_comment: Option<Instant>,
    last_quip: Option<Instant>,
    fly_out: Vec<(String, f32)>,
    mood: String,
    stress: f32,
    error: Option<String>,
}

fn vrc(app: &AppHandle) -> &Vrc {
    app.state::<Vrc>().inner()
}

fn lock(app: &AppHandle) -> std::sync::MutexGuard<'_, Inner> {
    vrc(app).inner.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn cfg_of(app: &AppHandle) -> crate::config::Config {
    app.state::<AppState>().cfg.lock().unwrap_or_else(std::sync::PoisonError::into_inner).clone()
}

fn en(cfg: &crate::config::Config) -> bool {
    crate::providers::prompt::reply_lang(cfg) == "en"
}

fn send_cmd(app: &AppHandle, c: Cmd) {
    if let Some(tx) = vrc(app).tx.lock().unwrap_or_else(std::sync::PoisonError::into_inner).as_ref() {
        let _ = tx.send(c);
    }
}

/// Alter Ego è in VRChat (thread OSC avviato).
pub fn running(app: &AppHandle) -> bool {
    vrc(app).running.load(Ordering::SeqCst)
}

/// Sta parlando adesso (la sua voce non va trascritta, e in push-to-talk tiene premuto Voice).
fn speaking(app: &AppHandle) -> bool {
    app.state::<crate::audio::AudioEngine>().voice_remaining() > 0.0
}

// ---------- sicurezza: tutto a zero ----------

/// Manda "tutto a zero" a VRChat da un socket nuovo (anche se il thread è bloccato o morto).
fn emergency_zero(target: SocketAddr) {
    let Ok(s) = UdpSocket::bind("127.0.0.1:0") else { return };
    let mut i = input::Inputs::default();
    for _ in 0..2 {
        for m in i.zero_all() {
            let _ = s.send_to(&osc::encode(&m), target);
        }
    }
    let _ = s.send_to(&osc::encode(&Msg::new("/chatbox/typing", vec![Arg::Bool(false)])), target);
}

/// Alla fine del thread (anche per panico): ingressi a zero.
struct Guard(SocketAddr);
impl Drop for Guard {
    fn drop(&mut self) {
        emergency_zero(self.0);
    }
}

// ---------- thread OSC: sensi, riflessi, corpo, chatbox ----------

fn param_addr(name: &str) -> String {
    format!("/avatar/parameters/{name}")
}

/// Collegamenti contatti → sensi: quelli scelti, altrimenti automatici dall'avatar.
fn contact_map(cfg: &crate::config::VrchatConfig, avatar: Option<&avatars::Avatar>) -> Vec<(String, String)> {
    if !cfg.contacts.is_empty() {
        return cfg.contacts.iter().map(|c| (c.param.clone(), c.sense.clone())).collect();
    }
    avatar.map(|a| avatars::auto_senses(&a.params)).unwrap_or_default()
}

/// Il parametro si può scrivere? (avatar sconosciuto: si prova lo stesso)
fn writable(avatar: Option<&avatars::Avatar>, name: &str) -> bool {
    avatar.is_none_or(|a| a.params.iter().any(|p| p.name == name && p.input))
}

fn run(app: AppHandle, rx: mpsc::Receiver<Cmd>, target: SocketAddr, sock: UdpSocket) {
    let _guard = Guard(target);
    let out = match UdpSocket::bind("127.0.0.1:0") {
        Ok(s) => s,
        Err(e) => {
            lock(&app).error = Some(e.to_string());
            return;
        }
    };
    let tx = |m: &Msg| {
        let _ = out.send_to(&osc::encode(m), target);
    };
    let _ = sock.set_read_timeout(Some(Duration::from_millis(40)));
    let mut inputs = input::Inputs::default();
    for m in inputs.zero_all() {
        tx(&m);
    }
    let start_cfg = cfg_of(&app).vrchat;
    let mut body = behave::Body::default();
    body.seed(crate::costs::now() as u64);
    body.set_mode(if start_cfg.follow { behave::Mode::Follow } else { behave::Mode::Idle }, Instant::now());
    let mut brain: Option<Box<crate::gmodlink::fly::Brain>> = None;
    let mut mood = crate::gmodlink::fly::Mood::default();
    let mut edges = senses::Edges::default();
    let mut chat: VecDeque<String> = VecDeque::new();
    let mut next_chat = Instant::now();
    let mut resets: Vec<(String, Arg, Instant)> = Vec::new();
    let mut last_tick = Instant::now();
    let mut last_reflex = Instant::now() - Duration::from_secs(5);
    let mut voice_on = false;
    let mut voice_off_at: Option<Instant> = None;
    let mut auto_wander = false;
    let mut buf = [0u8; 4096];
    loop {
        // 1. pacchetti da VRChat
        while let Ok((n, _)) = sock.recv_from(&mut buf) {
            let msgs = osc::decode(&buf[..n]);
            let mut i = lock(&app);
            i.last_rx = Some(Instant::now());
            for m in msgs {
                if m.addr == "/avatar/change" {
                    if let Some(id) = m.args.first().and_then(Arg::as_str) {
                        i.avatar_id = id.to_string();
                        i.params.clear();
                        i.avatar = None;
                    }
                } else if let Some(p) = m.addr.strip_prefix("/avatar/parameters/") {
                    if let Some(v) = m.args.first().and_then(Arg::as_f32) {
                        i.params.insert(p.to_string(), v);
                    }
                }
            }
        }
        // avatar nuovo: legge il suo file OSC (nome e parametri)
        let need = {
            let i = lock(&app);
            (!i.avatar_id.is_empty() && i.avatar.as_ref().is_none_or(|a| a.id != i.avatar_id)).then(|| i.avatar_id.clone())
        };
        if let Some(id) = need {
            let dirs = avatars::osc_dirs(&cfg_of(&app).vrchat.osc_dir);
            let a = avatars::find(&dirs, &id).unwrap_or(avatars::Avatar { id: id.clone(), name: id.clone(), user: String::new(), params: Vec::new(), mtime: 0 });
            crate::applog::info("vrchat", &format!("avatar: {} ({} parametri)", a.name, a.params.len()));
            remember(&app, &format!("[you changed avatar: {}]", a.name), "");
            lock(&app).avatar = Some(a);
        }
        // 2. comandi
        let mut stop = false;
        while let Ok(c) = rx.try_recv() {
            match c {
                Cmd::Stop => stop = true,
                Cmd::Typing(on) => tx(&Msg::new("/chatbox/typing", vec![Arg::Bool(on)])),
                Cmd::Say(t) => {
                    chat.extend(chatbox::pages(&t, chatbox::MAX));
                    tx(&Msg::new("/chatbox/typing", vec![Arg::Bool(false)]));
                }
                Cmd::Ops(ops) => {
                    let cfg = cfg_of(&app).vrchat;
                    let avatar = lock(&app).avatar.clone();
                    for op in ops {
                        match apply_op(&op, &cfg, avatar.as_ref(), &mut body) {
                            Applied::Stop => {
                                for m in inputs.zero_all() {
                                    tx(&m);
                                }
                            }
                            Applied::Param(name, arg, secs) => {
                                tx(&Msg::new(&param_addr(&name), vec![arg.clone()]));
                                resets.retain(|(n, _, _)| *n != name);
                                let zero = if matches!(arg, Arg::Bool(_)) { Arg::Bool(false) } else { Arg::Int(0) };
                                resets.push((name, zero, Instant::now() + Duration::from_secs_f32(secs)));
                            }
                            Applied::Leave => stop = true,
                            Applied::Done => auto_wander = false,
                        }
                    }
                }
            }
        }
        if stop {
            break;
        }
        // 3. dieci volte al secondo: sensi, cervello, corpo, cane da guardia
        let now = Instant::now();
        if now.duration_since(last_tick) < Duration::from_millis(100) {
            continue;
        }
        let ms = (now.duration_since(last_tick).as_secs_f32() * 1000.0).clamp(10.0, 300.0);
        last_tick = now;
        let full = cfg_of(&app);
        let cfg = &full.vrchat;
        let (params, avatar, voice, sight, quiet) = {
            let i = lock(&app);
            let quiet = i.voices.back().is_none_or(|t| t.elapsed() > Duration::from_secs(90)) && i.last_touch.as_ref().is_none_or(|t| t.1.elapsed() > Duration::from_secs(90));
            let sight = i.sight.as_ref().filter(|(t, _)| t.elapsed() < Duration::from_secs(cfg.look_secs as u64 * 2 + 4)).and_then(|(_, s)| s.nearest.clone().map(|(d, dist)| (i.sight_seq, d, dist)));
            (i.params.clone(), i.avatar.clone(), i.last_voice, sight, quiet)
        };
        let map = contact_map(cfg, avatar.as_ref());
        let sense = senses::from_params(&params, &map);
        for kind in edges.update(&params, &map) {
            on_touch(&app, kind);
        }
        // cervello di moscerino
        if cfg.fly {
            if brain.is_none() {
                brain = crate::gmodlink::fly::Brain::load().map(Box::new);
            }
            if let Some(b) = brain.as_mut() {
                b.run(ms, &sense);
                let mut o = b.read(ms);
                crate::gmodlink::fly::gate(&mut o, &sense);
                mood.update(ms / 1000.0, &sense, &o);
                if cfg.reflexes && last_reflex.elapsed() > Duration::from_millis(1200) {
                    let r = senses::reflexes(&o);
                    if !r.is_empty() {
                        last_reflex = now;
                        for (inp, v, d) in r.into_iter().rev() {
                            body.reflex(inp, v, d);
                        }
                    }
                }
                let mut i = lock(&app);
                i.fly_out = o;
                i.mood = mood.line();
                i.stress = mood.stress;
            }
        }
        // annoiata: va in giro da sola (se permesso); una voce la ferma
        if cfg.explore && quiet && body.mode == behave::Mode::Idle {
            body.set_mode(behave::Mode::Wander, now);
            auto_wander = true;
        } else if auto_wander && !quiet && body.mode == behave::Mode::Wander {
            body.set_mode(if cfg.follow { behave::Mode::Follow } else { behave::Mode::Idle }, now);
            auto_wander = false;
        }
        let cues = behave::Cues { voice, sight, explore: cfg.explore || !auto_wander, glances: cfg.gestures };
        if let Some(s) = body.tick(now, &cues) {
            if let Some(m) = inputs.set(s.input, s.value, Some(Duration::from_millis(s.ms)), now) {
                tx(&m);
            }
        }
        // microfono in push-to-talk: Voice premuto mentre parla (con una coda breve)
        if cfg.mic_mode == "ptt" {
            if speaking(&app) {
                voice_off_at = None;
                if !voice_on {
                    voice_on = true;
                    if let Some(m) = inputs.set("Voice", 1.0, None, now) {
                        tx(&m);
                    }
                }
            } else if voice_on {
                let at = *voice_off_at.get_or_insert(now + Duration::from_millis(350));
                if now >= at {
                    voice_on = false;
                    voice_off_at = None;
                    if let Some(m) = inputs.set("Voice", 0.0, None, now) {
                        tx(&m);
                    }
                }
            }
        }
        for m in inputs.tick(now) {
            if m.addr == "/input/Voice" {
                voice_on = false;
            }
            tx(&m);
        }
        // chatbox: una pagina alla volta
        if now >= next_chat {
            if let Some(p) = chat.pop_front() {
                tx(&Msg::new("/chatbox/input", vec![Arg::Str(p.clone()), Arg::Bool(true), Arg::Bool(false)]));
                next_chat = now + chatbox::dwell(&p);
            }
        }
        // parametri da rimettere a zero (emote, espressioni)
        resets.retain(|(n, z, t)| {
            if now >= *t {
                tx(&Msg::new(&param_addr(n), vec![z.clone()]));
                false
            } else {
                true
            }
        });
        let mut i = lock(&app);
        i.mode = body.mode.name().to_string();
        i.inputs = inputs.active();
        let first_contact = !i.greeted && i.last_rx.is_some();
        if first_contact {
            i.greeted = true;
        }
        drop(i);
        if first_contact {
            crate::applog::info("vrchat", "VRChat risponde via OSC");
            greet(&app);
        }
    }
    for _ in 0..3 {
        for m in inputs.zero_all() {
            tx(&m);
        }
    }
    tx(&Msg::new("/chatbox/typing", vec![Arg::Bool(false)]));
}

enum Applied {
    Done,
    Stop,
    Leave,
    Param(String, Arg, f32),
}

/// Esegue un comando nel thread OSC.
fn apply_op(op: &Value, cfg: &crate::config::VrchatConfig, avatar: Option<&avatars::Avatar>, body: &mut behave::Body) -> Applied {
    let now = Instant::now();
    let s = |k: &str| op[k].as_str().unwrap_or("").to_string();
    match op["op"].as_str().unwrap_or("") {
        "follow" => body.set_mode(behave::Mode::Follow, now),
        "stay" => body.set_mode(behave::Mode::Stay, now),
        "come" => body.set_mode(behave::Mode::Come(now + Duration::from_secs(12)), now),
        "wander" => body.set_mode(behave::Mode::Wander, now),
        "stop" => {
            body.set_mode(behave::Mode::Idle, now);
            return Applied::Stop;
        }
        "leave" => return Applied::Leave,
        "look" | "turn" | "walk" | "back" | "jump" => body.act(op["op"].as_str().unwrap_or(""), &s("dir"), op["secs"].as_f64().unwrap_or(2.0) as f32),
        "emote" => {
            let v = senses::parse_map(&cfg.emotes).into_iter().find(|(k, _)| *k == s("name")).map(|x| x.1);
            if let (true, Some(v), false) = (cfg.gestures, v, cfg.emote_param.is_empty()) {
                if writable(avatar, &cfg.emote_param) {
                    return Applied::Param(cfg.emote_param.clone(), Arg::Int(v), 4.0);
                }
            }
        }
        "expr" => {
            let v = senses::parse_map(&cfg.expressions).into_iter().find(|(k, _)| *k == s("emotion")).map(|x| x.1);
            if let (true, Some(v), false) = (cfg.gestures, v, cfg.expr_param.is_empty()) {
                if writable(avatar, &cfg.expr_param) {
                    return Applied::Param(cfg.expr_param.clone(), Arg::Int(v), 6.0);
                }
            }
        }
        _ => {}
    }
    Applied::Done
}

// ---------- parlare ----------

/// Aspetta il suo turno: non parla sopra a chi sta parlando vicino (al massimo ~4 s).
async fn wait_turn(app: &AppHandle) {
    let ears = vrc(app).ears.clone();
    let mut waited = false;
    for _ in 0..28 {
        if !ears.talking.load(Ordering::Relaxed) {
            break;
        }
        waited = true;
        tokio::time::sleep(Duration::from_millis(150)).await;
    }
    if waited {
        tokio::time::sleep(Duration::from_millis(350)).await;
    }
}

/// Testo nella chatbox ed espressione dell'avatar (la voce la fa il frontend con "app-say").
fn express(app: &AppHandle, emotion: &str, text: &str) {
    lock(app).emotion = emotion.to_string();
    send_cmd(app, Cmd::Ops(vec![json!({ "op": "expr", "emotion": emotion })]));
    if cfg_of(app).vrchat.chatbox && !text.trim().is_empty() {
        send_cmd(app, Cmd::Say(text.to_string()));
    }
}

/// Parla da sola (battute, eventi): chatbox + voce sul PC (che va in VRChat con Mic-Link).
async fn say(app: &AppHandle, emotion: &str, text: &str) {
    if text.trim().is_empty() {
        return;
    }
    wait_turn(app).await;
    express(app, emotion, text);
    let _ = app.emit("app-say", format!("[{emotion}]\n{text}"));
}

fn remember(app: &AppHandle, user: &str, reply: &str) {
    let mut i = lock(app);
    i.history.push_back((user.to_string(), reply.to_string()));
    while i.history.len() > 8 {
        i.history.pop_front();
    }
}

fn quip(app: &AppHandle, kind: &str) -> String {
    let e = en(&cfg_of(app));
    vrc(app).quips.lock().unwrap_or_else(std::sync::PoisonError::into_inner).pick(kind, e)
}

/// Appena arrivata: un saluto (battuta, zero token).
fn greet(app: &AppHandle) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_millis(1500)).await;
        let line = quip(&app, "greet");
        say(&app, "happy", &line).await;
    });
}

/// Qualcuno l'ha toccata (contatto dell'avatar): reazione breve senza token, non troppo spesso.
fn on_touch(app: &AppHandle, kind: &'static str) {
    {
        let mut i = lock(app);
        i.last_touch = Some((kind.to_string(), Instant::now()));
        if i.last_quip.is_some_and(|t| t.elapsed() < Duration::from_secs(5)) {
            return;
        }
        i.last_quip = Some(Instant::now());
    }
    if cfg_of(app).vrchat.chatter == "off" {
        return;
    }
    remember(app, &format!("[someone gave you a {kind}]"), "");
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        // reazione umana: un attimo di ritardo
        tokio::time::sleep(Duration::from_millis(350)).await;
        let emo = if kind == "hit" { "surprised" } else if kind == "headpat" || kind == "hug" { "happy" } else { "embarrassed" };
        let line = quip(&app, kind);
        say(&app, emo, &line).await;
    });
}

/// Chiede al modello un pacchetto di battute (una volta per lingua e pronomi).
fn ask_quips(app: &AppHandle) {
    let cfg = cfg_of(app);
    let key = format!("{}/{}", crate::providers::prompt::reply_lang(&cfg), cfg.personality.pronouns());
    {
        let mut q = vrc(app).quips.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        if q.busy || q.key == key {
            return;
        }
        q.busy = true;
    }
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let mut ai = cfg.ai.clone();
        ai.thinking = "off".into();
        ai.max_tokens = 600;
        let system = quips::system(crate::providers::prompt::pronoun_rule(&cfg), crate::providers::prompt::language_rule(&cfg));
        let msgs = vec![crate::providers::ChatMessage::new("user", &quips::user())];
        let ch = tauri::ipc::Channel::new(|_| Ok(()));
        let res = crate::providers::chat(&vrc(&app).quip_ai, ai, system, msgs, None, ch).await;
        let mut q = vrc(&app).quips.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        q.busy = false;
        match res.map(|r| quips::parse(&r.text)) {
            Ok(Some(b)) => q.store(&key, b),
            Ok(None) => crate::applog::warn("vrchat", "battute: risposta non leggibile (restano quelle di riserva)"),
            Err(e) => crate::applog::warn("vrchat", &format!("battute: {e}")),
        }
    });
}

// ---------- udito ----------

/// È rivolto a lei? (nome, oppure conversazione in corso)
fn addressed(app: &AppHandle, text: &str) -> bool {
    let cfg = cfg_of(app);
    let low = text.to_lowercase();
    let name = cfg.name_tag.trim().to_lowercase();
    if ["alter", "ego", "chihiro"].iter().any(|w| low.contains(w)) || (name.len() > 2 && low.contains(&name)) {
        return true;
    }
    lock(app).talking_with.is_some_and(|t| t.elapsed() < Duration::from_secs(40))
}

/// Frase sentita dall'audio di VRChat (già tagliata dal VAD).
fn on_segment(app: &AppHandle, seg: vad::Segment) {
    {
        let mut i = lock(app);
        i.voice_seq += 1;
        i.last_voice = Some((i.voice_seq, seg.balance, seg.level));
        i.voices.push_back(Instant::now());
        while i.voices.front().is_some_and(|t| t.elapsed() > Duration::from_secs(60)) {
            i.voices.pop_front();
        }
    }
    let cfg = cfg_of(app).vrchat;
    if !cfg.answer && !cfg.talk_others {
        return; // basta il VAD (turni e "a orecchio"): niente trascrizioni da pagare
    }
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let Ok(wav) = hearing::wav(&seg) else { return };
        let text = match crate::assistant::transcribe(&app, wav).await {
            Ok(t) => t,
            Err(e) => {
                crate::applog::warn("vrchat", &format!("trascrizione: {e}"));
                return;
            }
        };
        if hearing::hallucination(&text) {
            return;
        }
        let text: String = text.chars().take(300).collect();
        crate::applog::info("vrchat", &format!("sentito: {text}"));
        let to_me = addressed(&app, &text);
        let full = cfg_of(&app);
        let others = full.vrchat.talk_others && {
            let gap = match full.vrchat.chatter.as_str() {
                "often" => 30,
                "normal" => 60,
                "low" => 150,
                _ => u64::MAX,
            };
            lock(&app).last_comment.is_none_or(|t| t.elapsed() > Duration::from_secs(gap))
        };
        if !(to_me && full.vrchat.answer) && !others {
            let mut i = lock(&app);
            i.heard.push_back(text.clone());
            while i.heard.len() > 6 {
                i.heard.pop_front();
            }
            return;
        }
        if others && !to_me {
            lock(&app).last_comment = Some(Instant::now());
        }
        match handle(&app, "", &text, true).await {
            Ok(r) => {
                let _ = app.emit("app-say", r);
            }
            Err(e) => crate::applog::warn("vrchat", &e),
        }
    });
}

// ---------- vista ----------

/// Un'occhiata: screenshot → descrizione (locale o cloud). Restituisce errore leggibile.
async fn look(app: &AppHandle) -> Result<vision::Sight, String> {
    let cfg = cfg_of(app);
    let img = tauri::async_runtime::spawn_blocking(vision::capture).await.map_err(|e| e.to_string())??;
    let raw = match cfg.vrchat.vision.as_str() {
        "local" => {
            crate::localai::ensure_running(app).await?;
            let http = app.state::<AppState>().ai.local.clone();
            vision::describe_local(&http, &crate::localai::base(app), &cfg.vrchat.vision_model, &img).await?
        }
        "cloud" => {
            let mut ai = cfg.ai.clone();
            if !ai.order.iter().any(|id| ai.providers.get(id).is_some_and(|p| p.enabled && p.vision)) {
                return Err(crate::i18n::tr("nessun provider con la visione attiva (Impostazioni → AI)", "no provider with vision enabled (Settings → AI)").into());
            }
            ai.thinking = "off".into();
            ai.max_tokens = 200;
            let mut m = crate::providers::ChatMessage::new("user", "Describe it.");
            m.image = Some(img);
            let ch = tauri::ipc::Channel::new(|_| Ok(()));
            crate::providers::chat(&vrc(app).look_ai, ai, vision::PROMPT.to_string(), vec![m], None, ch).await?.text
        }
        _ => return Err("off".into()),
    };
    Ok(vision::parse(&raw))
}

/// Occhiate a ritmo: mentre segue o va in giro ogni `look_secs`, da ferma ogni 30 s, subito su richiesta.
fn spawn_eyes(app: &AppHandle) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let mut last: Option<Instant> = None;
        while running(&app) {
            tokio::time::sleep(Duration::from_millis(500)).await;
            let cfg = cfg_of(&app).vrchat;
            if cfg.vision == "off" {
                continue;
            }
            let (moving, now_req) = {
                let i = lock(&app);
                (["follow", "come", "wander"].contains(&i.mode.as_str()), i.look_now)
            };
            let gap = Duration::from_secs(if moving { cfg.look_secs as u64 } else { 30.max(cfg.look_secs as u64) });
            let due = last.is_none_or(|t| t.elapsed() >= gap) || (now_req && last.is_none_or(|t| t.elapsed() >= Duration::from_secs(3)));
            if !due {
                continue;
            }
            last = Some(Instant::now());
            {
                let mut i = lock(&app);
                i.look_now = false;
                i.looking = true;
            }
            let r = look(&app).await;
            let mut i = lock(&app);
            i.looking = false;
            match r {
                Ok(s) => {
                    i.sight_seq += 1;
                    i.sight = Some((Instant::now(), s));
                    i.vision_error = None;
                }
                Err(e) => {
                    if i.vision_error.as_deref() != Some(e.as_str()) {
                        crate::applog::warn("vrchat", &format!("vista: {e}"));
                    }
                    i.vision_error = Some(e);
                }
            }
        }
    });
}

// ---------- cervello: comandi rapidi e modello ----------

/// Parole che fanno pensare a VRChat (contro quelle "da PC" come musica o ricerche web).
const VR_WORDS: &[&str] = &["vrchat", "avatar", "mondo", "world", "segui", "follow", "vieni", "come", "salta", "jump", "balla", "dance", "saluta", "wave", "gira", "turn", "guarda", "look"];

/// La frase detta ad Alter Ego (push-to-talk o testo nell'app) va a VRChat?
pub fn wants(app: &AppHandle, text: &str) -> bool {
    let cfg = cfg_of(app);
    let low = format!(" {} ", text.to_lowercase());
    let vr = VR_WORDS.iter().any(|w| low.contains(w));
    if !running(app) {
        return cfg.vrchat.accepted_risk && quick::find(text, cfg.name_tag.trim(), false, false).is_some() && low.contains("vrchat");
    }
    let desktop = crate::saving::pick_groups(text).iter().any(|g| ["musica", "web", "file", "promemoria", "database", "grafo"].contains(g));
    !(desktop && !vr)
}

/// Unisce la cronologia in messaggi user/assistant alternati e aggiunge `user` in fondo.
fn messages(history: &[(String, String)], user: String) -> Vec<crate::providers::ChatMessage> {
    use crate::providers::ChatMessage;
    let mut merged: Vec<ChatMessage> = Vec::new();
    for (u, a) in history {
        for m in [ChatMessage::new("user", u), ChatMessage::new("assistant", a)] {
            if m.content.is_empty() {
                continue;
            }
            match merged.last_mut() {
                Some(last) if last.role == m.role => {
                    last.content.push('\n');
                    last.content.push_str(&m.content);
                }
                _ => merged.push(m),
            }
        }
    }
    let mut user = user;
    if let Some(last) = merged.pop_if(|m| m.role == "user") {
        user = format!("{}\n{user}", last.content);
    }
    if merged.first().is_some_and(|m| m.role == "assistant") {
        merged.remove(0);
    }
    merged.push(ChatMessage::new("user", &user));
    merged
}

/// Risponde a `text` (detto dall'utente col push-to-talk, oppure sentito vicino se `heard`).
/// Restituisce "[emozione]\ntesto" per la voce e il box sul PC; il testo va anche nella chatbox.
pub async fn handle(app: &AppHandle, from: &str, text: &str, heard: bool) -> Result<String, String> {
    let cfg = cfg_of(app);
    let e = en(&cfg);
    // 1. comandi semplici: zero token (con un attimo di ritardo, come una persona)
    if let Some(q) = quick::find(text, cfg.name_tag.trim(), running(app), e) {
        if q.ops.iter().any(|o| o["op"] == "join") {
            start(app)?;
        } else if q.ops.iter().any(|o| o["op"] == "leave") {
            express(app, q.emotion, &q.reply);
            let app2 = app.clone();
            tauri::async_runtime::spawn(async move {
                tokio::time::sleep(Duration::from_secs(4)).await;
                stop(&app2);
            });
        } else {
            send_cmd(app, Cmd::Ops(q.ops.clone()));
        }
        tokio::time::sleep(Duration::from_millis(300)).await;
        if !q.ops.iter().any(|o| o["op"] == "leave") {
            express(app, q.emotion, &q.reply);
        }
        remember(app, &format!("{}: {text}", if from.is_empty() { "someone" } else { from }), &q.reply);
        lock(app).talking_with = Some(Instant::now());
        return Ok(format!("[{}]\n{}", q.emotion, q.reply));
    }
    if !running(app) {
        return Err(crate::i18n::tr("Alter Ego non è in VRChat", "Alter Ego is not in VRChat").into());
    }
    // 2. il modello (una richiesta alla volta)
    let v = vrc(app);
    let _guard = v.busy.lock().await;
    send_cmd(app, Cmd::Typing(true));
    let (view, history, heard_ctx) = {
        let mut i = lock(app);
        let view = prompt::View {
            mode: i.mode.clone(),
            avatar: i.avatar.as_ref().map(|a| a.name.clone()).unwrap_or_default(),
            moving: !i.inputs.is_empty(),
            voices: i.voices.iter().filter(|t| t.elapsed() < Duration::from_secs(60)).count(),
            touch: i.last_touch.as_ref().filter(|t| t.1.elapsed() < Duration::from_secs(120)).map(|(k, t)| (k.clone(), t.elapsed().as_secs())),
            instinct: if cfg.vrchat.fly { i.mood.clone() } else { String::new() },
            sight: i.sight.as_ref().filter(|(t, _)| t.elapsed() < Duration::from_secs(90)).map(|(t, s)| (t.elapsed().as_secs(), s.text.clone())),
        };
        let h: Vec<(String, String)> = i.history.iter().cloned().collect();
        let o: Vec<String> = i.heard.drain(..).collect();
        (view, h, o)
    };
    let mut user = prompt::state_line(&view);
    if !heard_ctx.is_empty() {
        user.push_str(&format!("\n[heard around] {}", heard_ctx.join(" / ")));
    }
    let line = if heard {
        format!("someone nearby said: {text}")
    } else {
        format!("{}: {}", if from.is_empty() { "your friend" } else { from }, text.chars().take(400).collect::<String>())
    };
    user.push('\n');
    user.push_str(&line);
    let mut ai = cfg.ai.clone();
    ai.thinking = "off".into();
    ai.max_tokens = 400;
    let system = prompt::system(
        crate::providers::prompt::language_rule(&cfg),
        crate::providers::prompt::pronoun_rule(&cfg),
        cfg.user_name.trim(),
        cfg.vrchat.vision != "off",
        cfg.vrchat.fly,
    );
    let ch = tauri::ipc::Channel::new(|_| Ok(()));
    let res = crate::providers::chat(&v.ai, ai, system, messages(&history, user), None, ch).await;
    send_cmd(app, Cmd::Typing(false));
    let r = prompt::parse(&res?.text);
    crate::applog::info("vrchat", &format!("risposta: {} azioni, {} caratteri", r.ops.len(), r.text.chars().count()));
    let leave = r.ops.iter().any(|o| o["op"] == "leave");
    send_cmd(app, Cmd::Ops(r.ops.iter().filter(|o| o["op"] != "leave").cloned().collect()));
    if r.ops.iter().any(|o| ["follow", "come", "wander", "look"].contains(&o["op"].as_str().unwrap_or(""))) {
        lock(app).look_now = true;
    }
    wait_turn(app).await;
    express(app, &r.emotion, &r.text);
    let mut mem = format!("[{}]\n{}", r.emotion, r.text);
    for o in &r.ops {
        mem.push_str(&format!("\n[act] {}", o["op"].as_str().unwrap_or("")));
    }
    remember(app, &line, &mem);
    lock(app).talking_with = Some(Instant::now());
    if leave {
        let app2 = app.clone();
        tauri::async_runtime::spawn(async move {
            tokio::time::sleep(Duration::from_secs(4)).await;
            stop(&app2);
        });
    }
    Ok(format!("[{}]\n{}", r.emotion, if r.text.is_empty() { "…".into() } else { r.text }))
}

// ---------- entrare e uscire ----------

/// Entra in VRChat: apre l'OSC, il corpo, l'udito e gli occhi.
pub fn start(app: &AppHandle) -> Result<(), String> {
    let cfg = cfg_of(app).vrchat;
    if !cfg.accepted_risk {
        return Err(crate::i18n::tr("prima leggi e accetta l'avviso su VRChat nelle impostazioni", "first read and accept the VRChat notice in the settings").into());
    }
    let v = vrc(app);
    if v.running.swap(true, Ordering::SeqCst) {
        return Ok(());
    }
    let target: SocketAddr = match format!("{}:{}", if cfg.host == "localhost" { "127.0.0.1" } else { &cfg.host }, cfg.send_port).parse() {
        Ok(t) => t,
        Err(e) => {
            v.running.store(false, Ordering::SeqCst);
            return Err(e.to_string());
        }
    };
    let sock = match UdpSocket::bind(("127.0.0.1", cfg.recv_port)) {
        Ok(s) => s,
        Err(e) => {
            v.running.store(false, Ordering::SeqCst);
            return Err(crate::tr!("porta OSC {} occupata (un'altra app OSC è aperta?): {e}", "OSC port {} busy (is another OSC app open?): {e}", cfg.recv_port));
        }
    };
    *v.target.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = Some(target);
    {
        let mut i = lock(app);
        let history = std::mem::take(&mut i.history);
        *i = Inner { history, ..Default::default() };
    }
    let (tx, rx) = mpsc::channel();
    *v.tx.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = Some(tx);
    let a = app.clone();
    let spawned = std::thread::Builder::new().name("vrchat-osc".into()).spawn(move || {
        let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| run(a.clone(), rx, target, sock)));
        if r.is_err() {
            crate::applog::error("vrchat", "il thread OSC si è fermato per un errore: ingressi azzerati");
            lock(&a).error = Some("errore interno: ingressi azzerati".into());
        }
        vrc(&a).running.store(false, Ordering::SeqCst);
        vrc(&a).ears.stop.store(true, Ordering::SeqCst);
        *vrc(&a).tx.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = None;
    });
    if let Err(e) = spawned {
        v.running.store(false, Ordering::SeqCst);
        return Err(e.to_string());
    }
    crate::applog::info("vrchat", &format!("entrata: OSC {target} / porta {}", cfg.recv_port));
    if cfg.hearing {
        let app_mute = app.clone();
        let tail: Arc<Mutex<Option<Instant>>> = Arc::new(Mutex::new(None));
        let app_seg = app.clone();
        hearing::start(
            cfg.hearing_device.clone(),
            v.ears.clone(),
            move || {
                let mut t = tail.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
                if speaking(&app_mute) {
                    *t = Some(Instant::now());
                    return true;
                }
                // la coda della sua voce (eco, riverbero) non conta
                t.is_some_and(|x| x.elapsed() < Duration::from_millis(700))
            },
            move |seg| on_segment(&app_seg, seg),
        );
    }
    spawn_eyes(app);
    ask_quips(app);
    Ok(())
}

/// Esce: ferma corpo, udito e occhi (ingressi a zero).
pub fn stop(app: &AppHandle) {
    let history: Vec<(String, String)> = std::mem::take(&mut lock(app).history).into();
    crate::gmodlink::session_to_graph(app, "VRChat", "", &history);
    let v = vrc(app);
    send_cmd(app, Cmd::Stop);
    v.ears.stop.store(true, Ordering::SeqCst);
    if let Some(t) = *v.target.lock().unwrap_or_else(std::sync::PoisonError::into_inner) {
        emergency_zero(t);
    }
    crate::applog::info("vrchat", "uscita da VRChat");
}

/// Uscita dall'app: tutto a zero subito, anche se il thread non risponde.
pub fn shutdown(app: &AppHandle) {
    if running(app) {
        stop(app);
    }
}

// ---------- comandi per le impostazioni ----------

#[derive(Serialize)]
pub struct Status {
    pub running: bool,
    /// VRChat manda pacchetti OSC (ultimi 5 s).
    pub connected: bool,
    pub osc: String,
    pub avatar: Option<Value>,
    pub mode: String,
    pub emotion: String,
    pub inputs: Vec<&'static str>,
    pub hearing: Value,
    pub vision: Value,
    pub fly: Value,
    pub account: String,
    pub logged_in: bool,
    pub mic_link: bool,
    pub error: Option<String>,
}

#[tauri::command]
pub fn vrchat_status(app: AppHandle) -> Status {
    let cfg = cfg_of(&app).vrchat;
    let mic_link = app.state::<crate::audio::AudioEngine>().status().link_active;
    let v = vrc(&app);
    let ears = &v.ears;
    let i = lock(&app);
    let run = v.running.load(Ordering::SeqCst);
    Status {
        running: run,
        connected: run && i.last_rx.is_some_and(|t| t.elapsed() < Duration::from_secs(5)),
        osc: format!("{}:{} → {}", cfg.host, cfg.recv_port, cfg.send_port),
        avatar: i.avatar.as_ref().map(|a| json!({ "id": a.id, "name": a.name, "params": a.params.len() })),
        mode: i.mode.clone(),
        emotion: i.emotion.clone(),
        inputs: i.inputs.clone(),
        hearing: json!({
            "on": run && cfg.hearing,
            "device": ears.device.lock().unwrap_or_else(std::sync::PoisonError::into_inner).clone(),
            "error": ears.error.lock().unwrap_or_else(std::sync::PoisonError::into_inner).clone(),
            "talking": ears.talking.load(Ordering::Relaxed),
            "voices": i.voices.iter().filter(|t| t.elapsed() < Duration::from_secs(60)).count(),
            "last": i.heard.back(),
        }),
        vision: json!({
            "mode": cfg.vision,
            "looking": i.looking,
            "error": i.vision_error,
            "last": i.sight.as_ref().map(|(t, s)| json!({ "text": s.text, "secs": t.elapsed().as_secs() })),
        }),
        fly: json!({ "out": i.fly_out, "stress": i.stress, "mood": i.mood }),
        account: cfg.account.clone(),
        logged_in: cfg.login && !cfg.account.is_empty(),
        mic_link,
        error: i.error.clone(),
    }
}

#[tauri::command]
pub fn vrchat_settings(app: AppHandle, settings: crate::config::VrchatConfig) {
    update_config(&app, |c| {
        // il consenso e l'account si cambiano solo con i loro comandi
        let (acc, account) = (c.vrchat.accepted_risk, c.vrchat.account.clone());
        c.vrchat = settings;
        c.vrchat.accepted_risk = acc;
        c.vrchat.account = account;
        c.vrchat.normalize();
    });
}

/// L'utente ha letto l'avviso sui Termini di VRChat (si mostra una volta).
#[tauri::command]
pub fn vrchat_accept(app: AppHandle) {
    update_config(&app, |c| c.vrchat.accepted_risk = true);
    crate::applog::info("vrchat", "avviso sui Termini di VRChat accettato");
}

#[tauri::command]
pub fn vrchat_join(app: AppHandle, join: bool) -> Result<(), String> {
    if join {
        start(&app)
    } else {
        stop(&app);
        Ok(())
    }
}

/// Avatar dai file OSC di VRChat (senza login).
#[tauri::command]
pub async fn vrchat_avatars(app: AppHandle) -> Vec<avatars::Avatar> {
    let dir = cfg_of(&app).vrchat.osc_dir.clone();
    tauri::async_runtime::spawn_blocking(move || avatars::list(&avatars::osc_dirs(&dir))).await.unwrap_or_default()
}

/// Modelli di visione: installati in Ollama e consigliati.
#[tauri::command]
pub async fn vrchat_vision_models(app: AppHandle) -> Value {
    let http = app.state::<AppState>().ai.local.clone();
    let installed = vision::local_models(&http, &crate::localai::base(&app)).await;
    json!({ "installed": installed, "recommended": vision::RECOMMENDED })
}

#[derive(Serialize)]
pub struct LoginView {
    /// "ok" o "2fa".
    pub step: String,
    pub methods: Vec<String>,
    pub account: String,
}

/// Accesso facoltativo (solo per cambiare avatar). Senza `code`: nome e password; con `code`: secondo fattore.
#[tauri::command]
pub async fn vrchat_login(app: AppHandle, username: String, password: String, code: Option<String>, method: Option<String>) -> Result<LoginView, String> {
    if !cfg_of(&app).vrchat.login {
        return Err(crate::i18n::tr("attiva prima l'accesso facoltativo", "turn on the optional login first").into());
    }
    let http = app.state::<AppState>().ai.http.clone();
    let v = vrc(&app);
    let mut s = match code.as_deref().filter(|c| !c.trim().is_empty()) {
        None => {
            if username.trim().is_empty() || password.is_empty() {
                return Err(crate::i18n::tr("scrivi nome utente e password", "enter username and password").into());
            }
            let mut s = api::Session::default();
            let (st, body) = api::send(api::req_login(&http, api::BASE, &username, &password), &mut s).await?;
            match api::parse_user(st, &body)? {
                api::Login::TwoFactor(methods) => {
                    *v.pending.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = Some(s);
                    return Ok(LoginView { step: "2fa".into(), methods, account: String::new() });
                }
                api::Login::Ok { .. } => s,
            }
        }
        Some(c) => {
            let mut s = v.pending.lock().unwrap_or_else(std::sync::PoisonError::into_inner).take().ok_or_else(|| crate::i18n::tr("rifai l'accesso", "log in again").to_string())?;
            let m = method.unwrap_or_else(|| "totp".into());
            let (st, body) = api::send(api::req_verify(&http, api::BASE, &s, &m, c), &mut s).await?;
            let ok = st == 200 && serde_json::from_str::<Value>(&body).ok().and_then(|x| x["verified"].as_bool()).unwrap_or(false);
            if !ok {
                *v.pending.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = Some(s);
                return Err(crate::i18n::tr("codice non valido", "invalid code").into());
            }
            s
        }
    };
    let (st, body) = api::send(api::req_me(&http, api::BASE, &s), &mut s).await?;
    let name = match api::parse_user(st, &body)? {
        api::Login::Ok { name, .. } => name,
        api::Login::TwoFactor(_) => return Err(crate::i18n::tr("secondo fattore non accettato", "second factor not accepted").into()),
    };
    api::save_session(&s)?;
    update_config(&app, |c| c.vrchat.account = name.clone());
    crate::applog::info("vrchat", "accesso all'account VRChat riuscito (cookie nel portachiavi)");
    Ok(LoginView { step: "ok".into(), methods: Vec::new(), account: name })
}

#[tauri::command]
pub async fn vrchat_logout(app: AppHandle) -> Result<(), String> {
    let mut s = api::load_session();
    if !s.is_empty() {
        let http = app.state::<AppState>().ai.http.clone();
        let _ = api::send(api::req_logout(&http, api::BASE, &s), &mut s).await;
    }
    api::save_session(&api::Session::default())?;
    *vrc(&app).pending.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = None;
    update_config(&app, |c| c.vrchat.account.clear());
    Ok(())
}

/// Sceglie l'avatar: con l'accesso attivo lo indossa subito (API), altrimenti lo ricorda e basta.
#[tauri::command]
pub async fn vrchat_select_avatar(app: AppHandle, id: String) -> Result<String, String> {
    if !id.is_empty() && !api::valid_avatar_id(&id) {
        return Err(crate::i18n::tr("id avatar non valido", "invalid avatar id").into());
    }
    update_config(&app, |c| c.vrchat.avatar = id.clone());
    let cfg = cfg_of(&app).vrchat;
    if id.is_empty() || !cfg.login || cfg.account.is_empty() {
        return Ok(crate::i18n::tr("scelto: indossalo dal menu Avatar di VRChat", "selected: put it on from VRChat's Avatar menu").into());
    }
    let http = app.state::<AppState>().ai.http.clone();
    let mut s = api::load_session();
    let (st, body) = api::send(api::req_select(&http, api::BASE, &s, &id)?, &mut s).await?;
    if st == 401 {
        update_config(&app, |c| c.vrchat.account.clear());
        return Err(crate::i18n::tr("sessione scaduta: rifai l'accesso", "session expired: log in again").into());
    }
    if !(200..300).contains(&st) {
        return Err(format!("VRChat: HTTP {st} {}", body.chars().take(120).collect::<String>()));
    }
    let _ = api::save_session(&s);
    Ok(crate::i18n::tr("avatar cambiato", "avatar changed").into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn history_alternates_roles() {
        let h = vec![("a: ciao".to_string(), "[happy]\nciao!".to_string()), ("[someone gave you a headpat]".to_string(), String::new())];
        let m = messages(&h, "[state] x\nb: come va?".into());
        assert_eq!(m.iter().map(|x| x.role.as_str()).collect::<Vec<_>>(), vec!["user", "assistant", "user"]);
        assert!(m[2].content.starts_with("[someone gave you a headpat]\n[state]"));
        assert_eq!(messages(&[], "x".into()).len(), 1);
        let only_reply = vec![(String::new(), "ciao".to_string())];
        assert_eq!(messages(&only_reply, "x".into()).len(), 1, "non si comincia con l'assistente");
    }

    #[test]
    fn apply_ops_on_body() {
        let cfg = crate::config::VrchatConfig { expr_param: "Expression".into(), ..Default::default() };
        let a = avatars::parse(include_str!("fixtures/avtr_test.json")).unwrap();
        let mut b = behave::Body::default();
        assert!(matches!(apply_op(&json!({ "op": "follow" }), &cfg, Some(&a), &mut b), Applied::Done));
        assert_eq!(b.mode, behave::Mode::Follow);
        assert!(matches!(apply_op(&json!({ "op": "stop" }), &cfg, Some(&a), &mut b), Applied::Stop));
        assert!(matches!(apply_op(&json!({ "op": "leave" }), &cfg, Some(&a), &mut b), Applied::Leave));
        match apply_op(&json!({ "op": "emote", "name": "dance" }), &cfg, Some(&a), &mut b) {
            Applied::Param(n, Arg::Int(5), _) => assert_eq!(n, "VRCEmote"),
            _ => panic!("emote"),
        }
        match apply_op(&json!({ "op": "expr", "emotion": "sad" }), &cfg, Some(&a), &mut b) {
            Applied::Param(n, Arg::Int(2), _) => assert_eq!(n, "Expression"),
            _ => panic!("espressione"),
        }
        // parametro che l'avatar non ha (o non scrivibile): niente
        let no = crate::config::VrchatConfig { emote_param: "VelocityZ".into(), ..Default::default() };
        assert!(matches!(apply_op(&json!({ "op": "emote", "name": "wave" }), &no, Some(&a), &mut b), Applied::Done));
        let off = crate::config::VrchatConfig { gestures: false, ..Default::default() };
        assert!(matches!(apply_op(&json!({ "op": "emote", "name": "wave" }), &off, Some(&a), &mut b), Applied::Done));
        // contatti: automatici, oppure quelli scelti
        assert!(contact_map(&cfg, Some(&a)).iter().any(|(p, s)| p == "HeadPat" && s == "touch"));
        let manual = crate::config::VrchatConfig { contacts: vec![crate::config::ContactMap { param: "Ear".into(), sense: "touch".into() }], ..Default::default() };
        assert_eq!(contact_map(&manual, Some(&a)), vec![("Ear".to_string(), "touch".to_string())]);
    }

    #[test]
    fn emergency_zero_reaches_vrchat() {
        // finto VRChat in ascolto: deve ricevere ogni ingresso a zero
        let fake = UdpSocket::bind("127.0.0.1:0").unwrap();
        fake.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
        let addr = fake.local_addr().unwrap();
        drop(Guard(addr));
        let mut got = std::collections::HashSet::new();
        let mut buf = [0u8; 1024];
        while let Ok((n, _)) = fake.recv_from(&mut buf) {
            for m in osc::decode(&buf[..n]) {
                if let Some(name) = m.addr.strip_prefix("/input/") {
                    assert!(matches!(m.args[..], [Arg::Int(0)] | [Arg::Float(0.0)]), "{m:?}");
                    got.insert(name.to_string());
                }
            }
            if got.len() == input::BUTTONS.len() + input::AXES.len() {
                break;
            }
        }
        assert_eq!(got.len(), input::BUTTONS.len() + input::AXES.len());
    }

    #[test]
    fn config_normalizes() {
        let mut c = crate::config::VrchatConfig {
            host: "10.0.0.5".into(),
            send_port: 80,
            recv_port: 9000,
            vision: "x-ray".into(),
            look_secs: 0,
            contacts: vec![crate::config::ContactMap { param: "A".into(), sense: "touch".into() }, crate::config::ContactMap { param: "B".into(), sense: "magic".into() }],
            emote_param: "VRC Emote; rm -rf".into(),
            avatar: "usr_123".into(),
            ..Default::default()
        };
        c.normalize();
        assert_eq!((c.host.as_str(), c.send_port, c.recv_port), ("127.0.0.1", 9000, 9001));
        assert_eq!(c.vision, "off");
        assert_eq!(c.look_secs, 3);
        assert_eq!(c.contacts.len(), 1);
        assert_eq!(c.emote_param, "VRCEmoterm-rf");
        assert!(c.avatar.is_empty());
        let d = crate::config::VrchatConfig::default();
        assert!(!d.accepted_risk && !d.login && d.vision == "off" && d.hearing);
    }
}
