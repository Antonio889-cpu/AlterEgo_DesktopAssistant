//! Server veri: impostazioni per server (host:porta), login con AuthMe e simili, e Alter Ego che ti segue
//! tra lobby, warp, mondi e sottoserver (BungeeCord, Velocity).
//!
//! Dai messaggi del server capisce inviti al party e richieste di teletrasporto (/tpa); dalla chat capisce
//! le richieste del giocatore ("vieni al warp spawn", "entra nel mio party", "vieni nel server survival");
//! dallo stato capisce quando il giocatore sparisce dalla vista (warp, cambio mondo) o dalla tab list
//! (cambio di sottoserver). Se non sa dove sia andato, chiede. Tutti i comandi automatici passano da un
//! limite al minuto, e i comandi con la password solo da auth.rs (mai in chat pubblica).

use super::auth;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::VecDeque,
    sync::Mutex,
    time::{Duration, Instant},
};
use tauri::AppHandle;

/// Impostazioni di un server (in config.json, senza password).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct ServerProfile {
    /// "host:porta" in minuscolo ("lan" per il mondo aperto in LAN).
    pub addr: String,
    /// Registrazione e login automatici.
    pub auth: bool,
    pub register_cmd: String,
    pub login_cmd: String,
    /// L'account esiste già su questo server: solo login.
    pub registered: bool,
    /// Ti segue tra lobby, mondi e sottoserver.
    pub follow: bool,
    /// Accetta i tuoi inviti (party, /tpa).
    pub accept_invites: bool,
    pub party_cmd: String,
    pub tpa_accept_cmd: String,
    pub tpa_to_cmd: String,
    pub server_cmd: String,
    pub warp_cmd: String,
    pub hub_cmd: String,
    /// Per chiederti dove sei quando non ti vede (messaggio privato: funziona anche tra sottoserver).
    pub ask_cmd: String,
    /// Comandi automatici al minuto, al massimo.
    pub max_per_min: u32,
}

impl Default for ServerProfile {
    fn default() -> Self {
        Self {
            addr: String::new(),
            auth: true,
            register_cmd: auth::DEFAULT_REGISTER.into(),
            login_cmd: auth::DEFAULT_LOGIN.into(),
            registered: false,
            follow: true,
            accept_invites: true,
            party_cmd: "/party accept {player}".into(),
            tpa_accept_cmd: "/tpaccept".into(),
            tpa_to_cmd: "/tpa {player}".into(),
            server_cmd: "/server {name}".into(),
            warp_cmd: "/warp {name}".into(),
            hub_cmd: "/hub".into(),
            ask_cmd: "/msg {player} {text}".into(),
            max_per_min: 6,
        }
    }
}

/// Chiave di un server: "host:porta" ("lan" con porta 0).
pub fn addr_key(host: &str, port: u16) -> String {
    let h = host.trim().trim_end_matches('.').to_lowercase();
    if port == 0 {
        "lan".into()
    } else {
        format!("{}:{port}", if h.is_empty() { "127.0.0.1" } else { &h })
    }
}

fn clean_cmd(s: &str) -> String {
    let t: String = s.trim().chars().filter(|c| !c.is_control()).take(120).collect();
    // vuoto = spento; altrimenti deve essere un comando
    if t.is_empty() || t.starts_with('/') {
        t
    } else {
        format!("/{t}")
    }
}

/// Pulisce l'elenco dei server (al salvataggio delle impostazioni).
pub fn normalize(list: &mut Vec<ServerProfile>) {
    for p in list.iter_mut() {
        p.addr = p.addr.trim().to_lowercase().chars().filter(|c| !c.is_whitespace()).take(260).collect();
        for c in [&mut p.register_cmd, &mut p.login_cmd, &mut p.party_cmd, &mut p.tpa_accept_cmd, &mut p.tpa_to_cmd, &mut p.server_cmd, &mut p.warp_cmd, &mut p.hub_cmd, &mut p.ask_cmd] {
            *c = clean_cmd(c);
        }
        if p.register_cmd.is_empty() {
            p.register_cmd = auth::DEFAULT_REGISTER.into();
        }
        if p.login_cmd.is_empty() {
            p.login_cmd = auth::DEFAULT_LOGIN.into();
        }
        p.max_per_min = p.max_per_min.clamp(1, 30);
    }
    list.retain(|p| !p.addr.is_empty());
    let mut seen = std::collections::HashSet::new();
    list.retain(|p| seen.insert(p.addr.clone()));
    list.truncate(64);
}

pub fn profile_in(list: &[ServerProfile], addr: &str) -> ServerProfile {
    list.iter().find(|p| p.addr == addr).cloned().unwrap_or_else(|| ServerProfile { addr: addr.to_string(), ..Default::default() })
}

/// Riempie un modello di comando ({player}, {name}, {text}). None se spento o non è un comando.
pub fn render(template: &str, player: &str, name: &str, text: &str) -> Option<String> {
    let t = template.trim();
    if !t.starts_with('/') || t.contains("{password}") || t.contains("{pass}") {
        return None;
    }
    let out = t.replace("{player}", player).replace("{name}", name).replace("{text}", text);
    (out.len() > 1 && !out.contains('\n') && out.chars().count() <= 256).then_some(out)
}

// ---------- capire i messaggi ----------

/// Toglie i colori (§a) e porta in minuscolo.
fn plain(s: &str) -> String {
    let mut out = String::new();
    let mut it = s.chars();
    while let Some(c) = it.next() {
        if c == '§' {
            it.next();
        } else {
            out.push(c);
        }
    }
    out
}

fn is_name_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

/// Posizione del nome `name` come parola intera (senza maiuscole).
fn find_word(hay: &str, name: &str) -> Option<usize> {
    if name.is_empty() {
        return None;
    }
    // ASCII: le posizioni restano quelle del testo originale
    let h = hay.to_ascii_lowercase();
    let n = name.to_ascii_lowercase();
    let mut from = 0;
    while let Some(i) = h[from..].find(&n) {
        let s = from + i;
        let e = s + n.len();
        let before = h[..s].chars().next_back().is_none_or(|c| !is_name_char(c));
        let after = h[e..].chars().next().is_none_or(|c| !is_name_char(c));
        if before && after {
            return Some(s);
        }
        from = e;
    }
    None
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Invite {
    /// Invito al party.
    Party,
    /// Chiede ad Alter Ego di teletrasportarsi da lui/lei (/tpahere): si accetta con /tpaccept.
    TpaHere,
    /// Vuole teletrasportarsi da Alter Ego (/tpa): si accetta con /tpaccept.
    Tpa,
}

/// Invito o richiesta di teletrasporto da `from` in un messaggio del server.
pub fn parse_invite(msg: &str, from: &str) -> Option<Invite> {
    let p = plain(msg);
    find_word(&p, from)?;
    let t = p.to_lowercase();
    let has = |w: &[&str]| w.iter().any(|x| t.contains(x));
    if has(&["requested that you teleport to them", "requested you to teleport to them", "wants you to teleport to them", "asked you to teleport to them", "ti ha chiesto di teletrasportarti", "vuole che tu ti teletrasporti", "ha richiesto che tu ti teletrasporti", "chiede di teletrasportarti da"]) {
        return Some(Invite::TpaHere);
    }
    if has(&["requested to teleport to you", "wants to teleport to you", "has requested to teleport", "vuole teletrasportarsi da te", "ha richiesto di teletrasportarsi da te", "chiede di teletrasportarsi da te", "vuole teletrasportarsi da"]) {
        return Some(Invite::Tpa);
    }
    if has(&["party", "gruppo", "squadra"]) && has(&["invit", "invite"]) {
        return Some(Invite::Party);
    }
    None
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Travel {
    Warp(String),
    Server(String),
    Hub,
    Party,
    /// Alter Ego chiede di teletrasportarsi dal giocatore (/tpa {player}).
    TpaTo,
}

const STOP: &[&str] = &[
    "al", "il", "lo", "la", "i", "gli", "le", "un", "una", "di", "del", "dello", "della", "nel", "nello", "nella", "sul", "sullo", "sulla", "in", "a", "da", "mio", "mia", "tuo", "the", "to", "my", "a", "an", "of", "on", "into", "server", "warp", "è", "e",
];
const MOVE: &[&str] = &[
    "vieni", "vai", "entra", "passa", "raggiungimi", "torna", "andiamo", "spostati", "seguimi", "unisciti", "accetta", "teletrasportati", "come", "go", "join", "switch", "back", "return", "lets", "let's", "move", "follow", "meet", "accept", "teleport", "hop",
];

fn tokens(text: &str) -> Vec<String> {
    plain(text)
        .to_lowercase()
        .split(|c: char| c.is_whitespace() || ",;!?:()[]\"«»".contains(c))
        .map(|w| w.trim_matches(|c: char| c == '.' || c == '\'' || c == '’').to_string())
        .flat_map(|w| {
            // "all'hub" → "all", "hub"
            w.split(['\'', '’']).map(String::from).collect::<Vec<_>>()
        })
        .filter(|w| !w.is_empty())
        .collect()
}

fn name_after(tok: &[String], key: &str) -> Option<String> {
    let i = tok.iter().position(|w| w == key)?;
    tok[i + 1..].iter().find(|w| !STOP.contains(&w.as_str())).filter(|w| w.len() >= 2 && w.len() <= 32 && w.chars().all(|c| c.is_ascii_alphanumeric() || "_-".contains(c))).cloned()
}

/// Richiesta di spostarsi detta dal giocatore in chat.
pub fn parse_travel(text: &str) -> Option<Travel> {
    let tok = tokens(text);
    if tok.is_empty() || tok.len() > 14 {
        return None;
    }
    let moving = tok.iter().any(|w| MOVE.contains(&w.as_str())) || tok.len() <= 3;
    let has = |w: &[&str]| tok.iter().any(|x| w.contains(&x.as_str()));
    if !moving {
        return None;
    }
    if has(&["party", "gruppo"]) {
        return Some(Travel::Party);
    }
    if has(&["hub", "lobby"]) {
        return Some(Travel::Hub);
    }
    if let Some(n) = name_after(&tok, "warp") {
        return Some(Travel::Warp(n));
    }
    if let Some(n) = name_after(&tok, "server") {
        return Some(Travel::Server(n));
    }
    let tp = has(&["tp", "tpa", "teletrasportati", "teleport", "teletrasporto", "tippati"]);
    if tp && has(&["me"]) {
        return Some(Travel::TpaTo);
    }
    None
}

/// Chi parla in una riga di chat del server ("<Nome> ciao", "[VIP] Nome » ciao", "Nome: ciao"):
/// il testo se a parlare è `who`.
pub fn speaker(line: &str, who: &str) -> Option<String> {
    let p = plain(line);
    let i = find_word(&p, who)?;
    if i > 48 {
        return None;
    }
    let rest = &p[i + who.len()..];
    // "<Nome> testo"
    if p[..i].ends_with('<') && rest.starts_with('>') {
        return Some(rest[1..].trim().to_string()).filter(|s| !s.is_empty());
    }
    // prima del nome solo ranghi ("[VIP] "), non un altro che parla ("<Altro> Nome: …")
    if p[..i].contains(['>', ':', '»']) {
        return None;
    }
    // messaggio privato "[Nome -> me] testo"
    if let Some(r) = rest.trim_start().strip_prefix("->") {
        let j = r.find(']')?;
        let msg = r[j + 1..].trim();
        return (!msg.is_empty()).then(|| msg.to_string());
    }
    let cut = rest.char_indices().take(28).find(|(_, c)| [':', '»', '>', '→'].contains(c))?;
    let between = &rest[..cut.0];
    if between.split_whitespace().count() > 4 {
        return None;
    }
    let msg = rest[cut.0 + cut.1.len_utf8()..].trim_start_matches(['>', ' ', ':']).trim();
    (!msg.is_empty()).then(|| msg.to_string())
}

/// Server dietro un proxy (marchio del server: "BungeeCord … <- Paper", "Paper (Velocity)"…).
pub fn proxy_brand(brand: &str) -> bool {
    let b = brand.to_lowercase();
    ["bungee", "waterfall", "velocity", "travertine", "flamecord", "lightfall", "gate"].iter().any(|w| b.contains(w))
}

// ---------- limite e sparizioni ----------

/// Al massimo `max` comandi in un minuto (scorrevole).
#[derive(Debug, Default)]
pub struct RateLimit {
    sent: VecDeque<u64>,
}

impl RateLimit {
    pub fn allow(&mut self, now_ms: u64, max: u32) -> bool {
        while self.sent.front().is_some_and(|t| now_ms.saturating_sub(*t) >= 60_000) {
            self.sent.pop_front();
        }
        if self.sent.len() >= max as usize {
            return false;
        }
        self.sent.push_back(now_ms);
        true
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lost {
    /// Ancora nella tab list ma sparito dalla vista da vicino (warp, cambio mondo).
    Vanished,
    /// Uscito dalla tab list (cambio di sottoserver, o uscito dal gioco).
    Left,
}

/// Segue dove sta il giocatore per capire quando sparisce all'improvviso.
#[derive(Debug, Default)]
pub struct Tracker {
    seen_at: Option<u64>,
    near: bool,
    lost: bool,
}

impl Tracker {
    /// `dist`: distanza se lo vede, None se non lo vede; `in_tab`: è nella tab list.
    pub fn update(&mut self, now_ms: u64, in_tab: bool, dist: Option<f64>) -> Option<Lost> {
        if let Some(d) = dist {
            self.seen_at = Some(now_ms);
            self.near = d < 48.0;
            self.lost = false;
            return None;
        }
        let since = self.seen_at.map(|t| now_ms.saturating_sub(t))?;
        if self.lost || !self.near {
            return None;
        }
        if !in_tab {
            self.lost = true;
            return Some(Lost::Left);
        }
        if since > 8_000 {
            self.lost = true;
            return Some(Lost::Vanished);
        }
        None
    }

    /// Alter Ego ha cambiato mondo o è rinata: si riparte da zero.
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    pub fn is_lost(&self) -> bool {
        self.lost
    }
}

// ---------- stato della partita ----------

#[derive(Default)]
struct Net {
    auth: auth::Session,
    limit: RateLimit,
    tracker: Tracker,
    /// Ultima richiesta di spostamento (per non farla due volte: arriva da "chat" e da "sys").
    last_req: Option<(String, Instant)>,
    brand: String,
    asked_at: Option<Instant>,
    joined_at: Option<Instant>,
}

static NET: Mutex<Option<Net>> = Mutex::new(None);

fn net<R>(f: impl FnOnce(&mut Net) -> R) -> R {
    let mut g = NET.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    f(g.get_or_insert_with(Net::default))
}

fn now_ms() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

/// Server attuale (dalle impostazioni) e il suo profilo.
fn current(app: &AppHandle) -> (String, ServerProfile, crate::config::MinecraftConfig) {
    let cfg = super::cfg_of(app).minecraft;
    let addr = addr_key(&cfg.host, cfg.port);
    let p = profile_in(&cfg.servers, &addr);
    (addr, p, cfg)
}

/// Con chi gioca: dallo stato del bot, altrimenti il nome nelle impostazioni.
fn owner(app: &AppHandle) -> String {
    let set = super::cfg_of(app).minecraft.player.trim().to_string();
    let st = super::lock(app).state.as_ref().map(|s| s.owner.clone()).unwrap_or_default();
    if st.is_empty() {
        set
    } else {
        st
    }
}

fn en(app: &AppHandle) -> bool {
    crate::providers::prompt::reply_lang(&super::cfg_of(app)) == "en"
}

/// Pausa da persona, poi i comandi (in un thread: non blocca chi legge il bot).
fn later(app: &AppHandle, min_ms: u64, max_ms: u64, ops: Vec<Value>) {
    let app = app.clone();
    let r = crate::gmodlink::persona::Rng::new().f() as f64;
    let wait = Duration::from_millis(min_ms + ((max_ms - min_ms) as f64 * r) as u64);
    std::thread::spawn(move || {
        std::thread::sleep(wait);
        super::send(&app, ops);
    });
}

/// Un comando automatico, se il limite al minuto lo permette.
fn auto_cmd(app: &AppHandle, p: &ServerProfile, cmd: String, why: &str) -> bool {
    if !net(|n| n.limit.allow(now_ms(), p.max_per_min)) {
        super::console::log(app, "error", &crate::tr!("troppi comandi in un minuto: salto {} ({})", "too many commands in a minute: skipping {} ({})", cmd, why));
        return false;
    }
    super::console::log(app, "event", &format!("{why} → {cmd}"));
    later(app, 900, 2600, vec![json!({ "op": "cmd", "text": cmd })]);
    true
}

/// Login o registrazione: la password esce dal portachiavi solo qui, dentro un comando "/".
fn do_auth(app: &AppHandle, addr: &str, p: &ServerProfile, cfg: &crate::config::MinecraftConfig, act: auth::Action) {
    let mut pw = auth::get_password(addr, &cfg.username);
    if pw.is_none() && act == auth::Action::Register {
        let g = auth::generate();
        match auth::set_password(addr, &cfg.username, &g) {
            Ok(()) => {
                super::console::log(app, "auth", crate::i18n::tr("password nuova generata e salvata nel portachiavi del sistema", "new password generated and saved in the system keychain"));
                pw = Some(g);
            }
            Err(e) => super::console::log(app, "error", &crate::tr!("portachiavi: {}", "keychain: {}", e)),
        }
    }
    let Some(pw) = pw else {
        super::console::log(app, "error", crate::i18n::tr("il server chiede il login ma non c'è una password salvata per questo server", "the server asks to log in but there's no saved password for this server"));
        return;
    };
    super::console::add_secret(&pw);
    let tpl = if act == auth::Action::Register { &p.register_cmd } else { &p.login_cmd };
    match auth::render(tpl, &pw, &owner(app), &cfg.username) {
        Ok(cmd) => {
            if !net(|n| n.limit.allow(now_ms(), p.max_per_min)) {
                return;
            }
            let shown = super::console::redact(&cmd);
            super::console::log(app, "auth", &format!("{} → {shown}", if act == auth::Action::Register { crate::i18n::tr("registrazione", "register") } else { "login" }));
            later(app, 1200, 3500, vec![json!({ "op": "cmd", "text": cmd, "shown": shown, "auth": true })]);
        }
        Err(e) => super::console::log(app, "error", &format!("login: {e}")),
    }
}

/// Segna l'account come registrato nel profilo del server (una volta).
fn mark_registered(app: &AppHandle, addr: &str) {
    let addr = addr.to_string();
    crate::update_config(app, |c| {
        let mut p = profile_in(&c.minecraft.servers, &addr);
        if p.registered {
            return;
        }
        p.registered = true;
        c.minecraft.servers.retain(|x| x.addr != addr);
        c.minecraft.servers.push(p);
    });
}

fn on_system(app: &AppHandle, text: &str) {
    let (addr, p, cfg) = current(app);
    // login e registrazione (solo nei primi minuti dopo l'ingresso o un cambio di server)
    let fresh = net(|n| n.joined_at.is_some_and(|t| t.elapsed() < Duration::from_secs(300)));
    if p.auth && fresh && addr != "lan" {
        if let Some(prompt) = auth::detect(text) {
            let (act, registered_now, wrong) = net(|n| {
                let a = n.auth.on_message(prompt, p.registered, Instant::now());
                (a, n.auth.just_registered, n.auth.wrong)
            });
            if wrong && prompt == auth::Prompt::Wrong {
                super::console::log(app, "error", crate::i18n::tr("password sbagliata per questo server: cambiala nelle impostazioni (Server: accesso)", "wrong password for this server: change it in the settings (Server: access)"));
            }
            if registered_now && !p.registered {
                mark_registered(app, &addr);
            }
            if let Some(a) = act {
                do_auth(app, &addr, &p, &cfg, a);
            }
            return;
        }
    }
    // inviti e richieste di teletrasporto (solo dal giocatore con cui gioca)
    let who = owner(app);
    if p.accept_invites && !who.is_empty() {
        if let Some(inv) = parse_invite(text, &who) {
            let cmd = match inv {
                Invite::Party => render(&p.party_cmd, &who, &who, ""),
                Invite::TpaHere | Invite::Tpa => render(&p.tpa_accept_cmd, &who, &who, ""),
            };
            if let Some(c) = cmd {
                auto_cmd(app, &p, c, &crate::tr!("invito da {} ({:?})", "invite from {} ({:?})", who, inv));
            }
        }
    }
}

/// Richiesta di spostamento del giocatore: true se l'ha gestita (la chat non va al modello).
fn on_request(app: &AppHandle, from: &str, text: &str) -> bool {
    let (addr, p, _) = current(app);
    if addr == "lan" || !p.follow {
        return false;
    }
    let who = owner(app);
    if who.is_empty() || !from.eq_ignore_ascii_case(&who) {
        return false;
    }
    let Some(t) = parse_travel(text) else { return false };
    let key = text.trim().to_lowercase();
    let dup = net(|n| {
        let d = n.last_req.as_ref().is_some_and(|(k, at)| *k == key && at.elapsed() < Duration::from_secs(5));
        n.last_req = Some((key.clone(), Instant::now()));
        d
    });
    if dup {
        return true;
    }
    let cmd = match &t {
        Travel::Warp(n) => render(&p.warp_cmd, &who, n, ""),
        Travel::Server(n) => render(&p.server_cmd, &who, n, ""),
        Travel::Hub => render(&p.hub_cmd, &who, "", ""),
        Travel::Party => render(&p.party_cmd, &who, &who, ""),
        Travel::TpaTo => render(&p.tpa_to_cmd, &who, &who, ""),
    };
    let Some(cmd) = cmd else { return false };
    let reply = if en(app) { "On my way!" } else { "Arrivo!" };
    super::send(app, vec![json!({ "op": "say", "text": reply })]);
    net(|n| n.tracker.reset());
    auto_cmd(app, &p, cmd, &crate::tr!("{} chiede: {:?}", "{} asks: {:?}", who, t))
}

/// Non sa dove sia andato il giocatore: glielo chiede (una volta per sparizione, al massimo ogni 2 minuti).
fn on_lost(app: &AppHandle, how: Lost) {
    let (addr, p, _) = current(app);
    let who = owner(app);
    if addr == "lan" || !p.follow || who.is_empty() {
        return;
    }
    let ok = net(|n| {
        let ok = n.asked_at.is_none_or(|t| t.elapsed() > Duration::from_secs(120));
        if ok {
            n.asked_at = Some(Instant::now());
        }
        ok
    });
    if !ok {
        return;
    }
    let network = net(|n| proxy_brand(&n.brand));
    let text = if en(app) { "where did you go? Tell me the warp or the server and I'll come" } else { "dove ti trovo? Dimmi il warp o il server e arrivo" };
    super::console::log(app, "event", &crate::tr!("{} non si vede più ({:?})", "{} is out of sight ({:?})", who, how));
    // nella tab list e sullo stesso server: in chat; altrimenti in privato (arriva anche su un altro sottoserver)
    if how == Lost::Vanished && !network {
        super::send(app, vec![json!({ "op": "say", "text": format!("{who}, {text}") })]);
    } else if let Some(c) = render(&p.ask_cmd, &who, "", text) {
        auto_cmd(app, &p, c, crate::i18n::tr("chiedo dove sei", "asking where you are"));
    }
}

/// Stato dal bot (ogni secondo): il giocatore è sparito?
pub fn on_state(app: &AppHandle, s: &super::proto::McState) {
    let who = if s.owner.is_empty() { super::cfg_of(app).minecraft.player.trim().to_string() } else { s.owner.clone() };
    if who.is_empty() {
        return;
    }
    let pl = s.players.iter().find(|p| p.name.eq_ignore_ascii_case(&who));
    let dist = pl.and_then(|p| if p.seen == Some(false) { None } else { p.d });
    if let Some(l) = net(|n| n.tracker.update(now_ms(), pl.is_some(), dist)) {
        on_lost(app, l);
    }
}

/// Evento dal bot, prima del cervello. Restituisce l'evento da passare avanti (forse cambiato) o None.
pub fn on_event(app: &AppHandle, mut e: Value) -> Option<Value> {
    let kind = e["kind"].as_str().unwrap_or("").to_string();
    let s = |k: &str| e[k].as_str().unwrap_or("").to_string();
    match kind.as_str() {
        // ogni messaggio del server (anche la chat dei giocatori, con il formato del server)
        "sys" => {
            let text = s("text");
            let pos = s("pos");
            super::console::log(app, if pos == "chat" { "chat" } else { "sys" }, &text);
            if pos != "chat" {
                on_system(app, &text);
            }
            let who = owner(app);
            if let Some(msg) = speaker(&text, &who) {
                on_request(app, &who, &msg);
            }
            None
        }
        "world" => {
            super::console::log(app, "event", &crate::tr!("mondo: {}", "world: {}", s("dim")));
            net(|n| n.tracker.reset());
            None
        }
        "net" => {
            let brand = s("brand");
            let changed = net(|n| {
                let c = !n.brand.is_empty() && n.brand != brand;
                n.brand = brand.clone();
                if c {
                    // nuovo sottoserver: forse un altro login
                    n.auth = auth::Session::default();
                    n.joined_at = Some(Instant::now());
                    n.tracker.reset();
                }
                c
            });
            super::console::log(app, "event", &format!("{}: {brand}{}", if changed { crate::i18n::tr("nuovo server", "new server") } else { "server" }, if proxy_brand(&brand) { crate::i18n::tr(" (rete con proxy)", " (proxy network)") } else { "" }));
            None
        }
        "chat" => {
            if on_request(app, &s("from"), &s("text")) {
                return None;
            }
            Some(e)
        }
        "joined" => {
            net(|n| {
                *n = Net { limit: std::mem::take(&mut n.limit), ..Default::default() };
                n.joined_at = Some(Instant::now());
            });
            let version = s("version");
            super::via::real_world(&mut e);
            super::console::log(app, "event", &crate::tr!("nel mondo: Minecraft {} su {}:{}{}", "in the world: Minecraft {} on {}:{}{}", version, e["host"].as_str().unwrap_or(""), e["port"], if e["via"].is_string() { crate::i18n::tr(" (ponte ViaProxy)", " (ViaProxy bridge)") } else { "" }));
            Some(e)
        }
        "kicked" => {
            super::console::log(app, "error", &crate::tr!("espulsione dal server: {}", "kicked by the server: {}", s("reason")));
            Some(e)
        }
        "error" => {
            super::console::log(app, "error", &s("msg"));
            Some(e)
        }
        "left" => {
            super::console::log(app, "event", &crate::tr!("uscita dal mondo {}", "left the world {}", s("reason")));
            Some(e)
        }
        "said" => {
            super::console::log(app, "cmd", &format!("» {}", s("text")));
            Some(e)
        }
        "player_leave" => {
            super::console::log(app, "event", &crate::tr!("{} ha lasciato il gioco", "{} left the game", s("name")));
            if s("name").eq_ignore_ascii_case(&owner(app)) {
                if let Some(l) = net(|n| n.tracker.update(now_ms(), false, None)) {
                    on_lost(app, l);
                }
            }
            Some(e)
        }
        "player_join" => {
            super::console::log(app, "event", &crate::tr!("{} è nel gioco", "{} joined the game", s("name")));
            Some(e)
        }
        "died" => {
            super::console::log(app, "event", crate::i18n::tr("morte: si rinasce allo spawn", "died: respawning at spawn"));
            net(|n| n.tracker.reset());
            Some(e)
        }
        "skin" => {
            super::console::log(app, "event", &format!("SkinsRestorer: {}", s("msg")));
            Some(e)
        }
        "lan" => {
            super::console::log(app, "event", &crate::tr!("mondo LAN «{}» su {}:{}", "LAN world «{}» on {}:{}", s("motd"), s("host"), e["port"]));
            Some(e)
        }
        _ => Some(e),
    }
}

// ---------- comandi per le impostazioni ----------

#[derive(Serialize)]
pub struct ServerInfo {
    pub addr: String,
    pub profile: ServerProfile,
    /// C'è una password salvata (mai la password).
    pub has_password: bool,
}

#[tauri::command]
pub fn minecraft_server_get(app: AppHandle) -> ServerInfo {
    let (addr, profile, cfg) = current(&app);
    let has_password = addr != "lan" && auth::get_password(&addr, &cfg.username).is_some();
    ServerInfo { addr, profile, has_password }
}

#[tauri::command]
pub fn minecraft_server_set(app: AppHandle, profile: ServerProfile) {
    let (addr, _, _) = current(&app);
    crate::update_config(&app, |c| {
        let mut p = profile;
        p.addr = addr.clone();
        c.minecraft.servers.retain(|x| x.addr != addr);
        c.minecraft.servers.push(p);
        normalize(&mut c.minecraft.servers);
    });
}

/// Salva la password scelta (vuota = ne genera una).
#[tauri::command]
pub fn minecraft_password_set(app: AppHandle, password: String) -> Result<(), String> {
    let (addr, _, cfg) = current(&app);
    if addr == "lan" {
        return Err(crate::i18n::tr("nel mondo in LAN non serve una password", "a LAN world doesn't need a password").into());
    }
    let pw = if password.trim().is_empty() { auth::generate() } else { password.trim().to_string() };
    auth::set_password(&addr, &cfg.username, &pw)?;
    super::console::add_secret(&pw);
    super::console::log(&app, "auth", &crate::tr!("password per {} salvata nel portachiavi", "password for {} saved in the keychain", addr));
    Ok(())
}

#[tauri::command]
pub fn minecraft_password_clear(app: AppHandle) -> Result<(), String> {
    let (addr, _, cfg) = current(&app);
    auth::clear_password(&addr, &cfg.username)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profiles() {
        assert_eq!(addr_key("Play.Example.NET.", 25565), "play.example.net:25565");
        assert_eq!(addr_key("192.168.1.5", 0), "lan");
        let mut l = vec![
            ServerProfile { addr: " A:1 ".into(), warp_cmd: "warp {name}".into(), register_cmd: "".into(), max_per_min: 900, ..Default::default() },
            ServerProfile { addr: "a:1".into(), ..Default::default() },
            ServerProfile { addr: "".into(), ..Default::default() },
        ];
        normalize(&mut l);
        assert_eq!(l.len(), 1);
        assert_eq!(l[0].warp_cmd, "/warp {name}");
        assert_eq!(l[0].register_cmd, auth::DEFAULT_REGISTER);
        assert_eq!(l[0].max_per_min, 30);
        assert_eq!(profile_in(&l, "b:2").addr, "b:2");
    }

    #[test]
    fn command_templates() {
        assert_eq!(render("/warp {name}", "Makoto", "spawn", "").as_deref(), Some("/warp spawn"));
        assert_eq!(render("/party accept {player}", "Makoto", "", "").as_deref(), Some("/party accept Makoto"));
        assert_eq!(render("/msg {player} {text}", "Makoto", "", "dove sei?").as_deref(), Some("/msg Makoto dove sei?"));
        assert_eq!(render("", "Makoto", "", ""), None, "spento");
        assert_eq!(render("warp {name}", "", "x", ""), None, "non è un comando");
        assert_eq!(render("/login {password}", "", "", ""), None, "le password solo da auth.rs");
    }

    #[test]
    fn invites() {
        assert_eq!(parse_invite("§eMakoto §ahas invited you to join their party! Type /party accept Makoto", "Makoto"), Some(Invite::Party));
        assert_eq!(parse_invite("Makoto ti ha invitato nel suo party", "makoto"), Some(Invite::Party));
        assert_eq!(parse_invite("[Party] Invito ricevuto da Makoto", "Makoto"), Some(Invite::Party));
        assert_eq!(parse_invite("Makoto has requested that you teleport to them.", "Makoto"), Some(Invite::TpaHere));
        assert_eq!(parse_invite("Makoto has requested to teleport to you.", "Makoto"), Some(Invite::Tpa));
        assert_eq!(parse_invite("Makoto vuole teletrasportarsi da te.", "Makoto"), Some(Invite::Tpa));
        // da un altro giocatore: niente
        assert_eq!(parse_invite("Griefer has invited you to join their party!", "Makoto"), None);
        assert_eq!(parse_invite("Makoto2 has invited you to join their party!", "Makoto"), None, "nome intero");
        assert_eq!(parse_invite("Makoto joined the game", "Makoto"), None);
    }

    #[test]
    fn travel_requests() {
        assert_eq!(parse_travel("vieni al warp spawn"), Some(Travel::Warp("spawn".into())));
        assert_eq!(parse_travel("Come to warp Mines!"), Some(Travel::Warp("mines".into())));
        assert_eq!(parse_travel("warp shop"), Some(Travel::Warp("shop".into())));
        assert_eq!(parse_travel("entra nel mio party"), Some(Travel::Party));
        assert_eq!(parse_travel("join my party pls"), Some(Travel::Party));
        assert_eq!(parse_travel("vieni nel server survival"), Some(Travel::Server("survival".into())));
        assert_eq!(parse_travel("let's go to server skyblock"), Some(Travel::Server("skyblock".into())));
        assert_eq!(parse_travel("torna all'hub"), Some(Travel::Hub));
        assert_eq!(parse_travel("go back to the lobby"), Some(Travel::Hub));
        assert_eq!(parse_travel("teletrasportati da me"), Some(Travel::TpaTo));
        assert_eq!(parse_travel("tp to me"), Some(Travel::TpaTo));
        // chiacchiere: niente
        assert_eq!(parse_travel("questo server è bellissimo davvero"), None);
        assert_eq!(parse_travel("ciao come stai"), None);
        assert_eq!(parse_travel("vieni qui"), None);
    }

    #[test]
    fn speakers() {
        assert_eq!(speaker("<Makoto> vieni al warp spawn", "Makoto").as_deref(), Some("vieni al warp spawn"));
        assert_eq!(speaker("§7[VIP] §fMakoto §8» §fentra nel mio party", "makoto").as_deref(), Some("entra nel mio party"));
        assert_eq!(speaker("[Survival] Makoto: ciao", "Makoto").as_deref(), Some("ciao"));
        assert_eq!(speaker("[Makoto -> me] torna all'hub", "Makoto").as_deref(), Some("torna all'hub"));
        assert_eq!(speaker("Makoto whispers to you: ciao", "Makoto").as_deref(), Some("ciao"));
        assert_eq!(speaker("<Altro> Makoto: vieni al warp x", "Makoto"), None, "parla un altro");
        assert_eq!(speaker("Makoto joined the game", "Makoto"), None);
        assert_eq!(speaker("ciao", ""), None);
    }

    #[test]
    fn rate_limit() {
        let mut r = RateLimit::default();
        let ok = (0..10).filter(|i| r.allow(i * 1000, 6)).count();
        assert_eq!(ok, 6);
        assert!(!r.allow(30_000, 6));
        assert!(r.allow(61_000, 6), "un minuto dopo il primo");
    }

    #[test]
    fn notices_when_player_vanishes() {
        let mut t = Tracker::default();
        assert_eq!(t.update(0, true, None), None, "mai visto");
        assert_eq!(t.update(1000, true, Some(5.0)), None);
        assert_eq!(t.update(5000, true, None), None, "ancora presto");
        assert_eq!(t.update(10_000, true, None), Some(Lost::Vanished));
        assert_eq!(t.update(20_000, true, None), None, "una volta sola");
        assert!(t.is_lost());
        assert_eq!(t.update(21_000, true, Some(3.0)), None);
        assert!(!t.is_lost());
        assert_eq!(t.update(22_000, false, None), Some(Lost::Left), "uscito dalla tab list");
        // lontano quando è sparito: niente domande
        let mut t = Tracker::default();
        t.update(0, true, Some(90.0));
        assert_eq!(t.update(20_000, true, None), None);
        // dopo un cambio di mondo di Alter Ego si riparte
        let mut t = Tracker::default();
        t.update(0, true, Some(2.0));
        t.reset();
        assert_eq!(t.update(20_000, true, None), None);
    }

    #[test]
    fn proxies() {
        assert!(proxy_brand("BungeeCord (git:BungeeCord-Bootstrap:1.21-R0.1-SNAPSHOT:abc) <- Paper"));
        assert!(proxy_brand("Paper (Velocity)"));
        assert!(!proxy_brand("vanilla"));
        assert!(!proxy_brand("Paper"));
    }
}
