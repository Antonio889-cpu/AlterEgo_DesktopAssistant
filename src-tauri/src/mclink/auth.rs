//! Registrazione e login sui server con AuthMe (o plugin simili: nLogin, LoginSecurity, OpeNLogin…).
//!
//! La prima volta Alter Ego manda il comando di registrazione, poi a ogni ingresso quello di login.
//! Capisce quale serve dai messaggi del server ("/register", "/login", "registrati", "please login"…),
//! aspetta un attimo come una persona e riprova al massimo qualche volta.
//!
//! La password sta nel portachiavi del sistema (keys.rs), mai in config.json né nei log: la console
//! la maschera e il bot la riceve solo dentro un comando che inizia con "/" (mai in chat pubblica).

use std::time::{Duration, Instant};

pub const DEFAULT_REGISTER: &str = "/register {password} {password}";
pub const DEFAULT_LOGIN: &str = "/login {password}";
/// Al massimo tanti tentativi per ingresso.
pub const MAX_TRIES: u32 = 3;
const MASK: &str = "••••";

/// Cosa chiede (o dice) il server.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Prompt {
    /// Chiede di registrarsi.
    Register,
    /// Chiede di fare il login.
    Login,
    /// Registrazione o login riusciti.
    Success,
    /// Password sbagliata.
    Wrong,
    /// L'account c'è già: serve il login.
    AlreadyRegistered,
    /// L'account non c'è: serve la registrazione.
    NotRegistered,
}

fn has(t: &str, words: &[&str]) -> bool {
    words.iter().any(|w| t.contains(w))
}

/// Toglie i codici colore (§a, &a) e porta in minuscolo.
fn plain(msg: &str) -> String {
    let mut out = String::with_capacity(msg.len());
    let mut it = msg.chars().peekable();
    while let Some(c) = it.next() {
        if c == '§' {
            it.next();
            continue;
        }
        out.push(c);
    }
    out.to_lowercase()
}

/// Capisce un messaggio del server (solo messaggi di sistema, non la chat dei giocatori).
pub fn detect(msg: &str) -> Option<Prompt> {
    let t = plain(msg);
    if t.trim().is_empty() || t.len() > 400 {
        return None;
    }
    if has(&t, &["wrong password", "incorrect password", "invalid password", "password errata", "password sbagliata", "password non corretta", "password incorretta"]) {
        return Some(Prompt::Wrong);
    }
    if has(&t, &["already registered", "already have registered", "already have an account", "già registrat", "gia registrat", "sei già iscritto"]) {
        return Some(Prompt::AlreadyRegistered);
    }
    if has(&t, &["not registered", "isn't registered", "is not registered", "non sei registrat", "non è registrat", "non risulti registrat"]) {
        return Some(Prompt::NotRegistered);
    }
    if has(&t, &[
        "successfully registered",
        "registered successfully",
        "registration successful",
        "registrazione completata",
        "registrazione effettuata",
        "registrato con successo",
        "registrata con successo",
        "successful login",
        "successfully logged",
        "logged in successfully",
        "login successful",
        "login effettuato",
        "accesso effettuato",
        "login eseguito",
        "autenticato con successo",
        "autenticata con successo",
        "you are now logged in",
    ]) {
        return Some(Prompt::Success);
    }
    let reg = has(&t, &["/register", "/reg ", "please register", "register with", "registrati", "registrarti", "per registrarti", "use /reg", "usa /reg"]);
    let login = has(&t, &["/login", "/log ", "/l <", "/l [", "please login", "please log in", "log in with", "login with", "effettua il login", "esegui il login", "fai il login", "accedi con", "per accedere", "authenticate with"]);
    match (reg, login) {
        (true, false) => Some(Prompt::Register),
        (false, true) => Some(Prompt::Login),
        // "registrati con /register o entra con /login": decide chi sa se l'account esiste
        (true, true) => Some(Prompt::Login),
        _ => None,
    }
}

/// Compila un modello di comando: {password} (o {pass}), {player}, {name}, {me}.
/// Il risultato deve essere un comando ("/…"), su una riga, al massimo 256 caratteri.
pub fn render(template: &str, password: &str, player: &str, me: &str) -> Result<String, String> {
    let t = template.trim();
    if !t.starts_with('/') || t.len() < 2 {
        return Err(crate::i18n::tr("il comando deve iniziare con «/»", "the command must start with \"/\"").into());
    }
    if (t.contains("{password}") || t.contains("{pass}")) && password.is_empty() {
        return Err(crate::i18n::tr("manca la password", "no password set").into());
    }
    if password.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return Err(crate::i18n::tr("la password non può contenere spazi", "the password can't contain spaces").into());
    }
    let out = t.replace("{password}", password).replace("{pass}", password).replace("{player}", player).replace("{me}", me).replace("{name}", me);
    if out.contains('\n') || out.contains('\r') || out.chars().count() > 256 {
        return Err(crate::i18n::tr("comando troppo lungo o su più righe", "command too long or on several lines").into());
    }
    Ok(out)
}

/// Copre le password in un testo (log, console, errori).
pub fn mask(text: &str, secrets: &[String]) -> String {
    let mut out = text.to_string();
    let mut s: Vec<&String> = secrets.iter().filter(|s| s.chars().count() >= 3).collect();
    // le più lunghe prima: una password che ne contiene un'altra resta coperta tutta
    s.sort_by_key(|x| std::cmp::Reverse(x.len()));
    for k in s {
        out = out.replace(k.as_str(), MASK);
    }
    out
}

/// Comandi di login scritti a mano (console): si coprono anche senza conoscere la password.
pub fn mask_auth_cmd(text: &str) -> String {
    let t = text.trim_start();
    let low = t.to_lowercase();
    let head = low.split_whitespace().next().unwrap_or("");
    let auth = ["/login", "/l", "/log", "/register", "/reg", "/changepassword", "/changepass", "/cp", "/unregister", "/premium", "/2fa"].contains(&head);
    if !auth {
        return text.to_string();
    }
    let mut parts = t.split_whitespace();
    let cmd = parts.next().unwrap_or("");
    let n = parts.count();
    if n == 0 {
        return cmd.to_string();
    }
    format!("{cmd} {}", vec![MASK; n].join(" "))
}

/// Password nuova: 16 caratteri facili da scrivere (niente 0/O, 1/l), dal generatore del sistema.
pub fn generate() -> String {
    const SET: &[u8] = b"abcdefghijkmnpqrstuvwxyzABCDEFGHJKLMNPQRSTUVWXYZ23456789";
    let mut buf = [0u8; 64];
    let _ = getrandom::getrandom(&mut buf);
    let mut out = String::new();
    for b in buf {
        // scarta i byte che renderebbero la scelta non uniforme
        if (b as usize) < 256 - 256 % SET.len() {
            out.push(SET[b as usize % SET.len()] as char);
        }
        if out.len() == 16 {
            break;
        }
    }
    out
}

/// Nome della voce nel portachiavi: un account per server e per nome.
pub fn slot(addr: &str, username: &str) -> String {
    format!("{}@{}", username.to_lowercase(), addr.to_lowercase())
}

const KEY_PROVIDER: &str = "minecraft";

pub fn get_password(addr: &str, username: &str) -> Option<String> {
    crate::keys::get_slot(KEY_PROVIDER, &slot(addr, username))
}

pub fn set_password(addr: &str, username: &str, password: &str) -> Result<(), String> {
    let p = password.trim();
    if p.chars().count() < 4 || p.chars().count() > 64 || p.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return Err(crate::i18n::tr("password non valida: da 4 a 64 caratteri, senza spazi", "invalid password: 4 to 64 characters, no spaces").into());
    }
    crate::keys::set_slot(KEY_PROVIDER, &slot(addr, username), p)
}

pub fn clear_password(addr: &str, username: &str) -> Result<(), String> {
    crate::keys::delete_slot(KEY_PROVIDER, &slot(addr, username))
}

/// Cosa mandare adesso.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Register,
    Login,
}

/// Stato del login in un ingresso (si azzera a ogni ingresso nel server).
#[derive(Debug, Default)]
pub struct Session {
    pub tries: u32,
    pub last: Option<Instant>,
    pub done: bool,
    /// Ultimo comando mandato (per capire "già registrato" dopo una registrazione).
    pub sent: Option<Action>,
    /// Registrazione riuscita in questo ingresso (da salvare nel profilo del server).
    pub just_registered: bool,
    pub wrong: bool,
}

impl Session {
    /// Messaggio del server → azione da fare (o niente). `registered`: il profilo dice che l'account esiste.
    pub fn on_message(&mut self, p: Prompt, registered: bool, now: Instant) -> Option<Action> {
        match p {
            Prompt::Success => {
                if self.sent == Some(Action::Register) {
                    self.just_registered = true;
                }
                self.done = true;
                None
            }
            Prompt::Wrong => {
                // password sbagliata: riprovare non serve, lo dice nella console
                self.wrong = true;
                self.done = true;
                None
            }
            _ if self.done => None,
            _ => {
                let want = match p {
                    Prompt::Register | Prompt::NotRegistered if !(registered && p == Prompt::Register) => Action::Register,
                    Prompt::Register => Action::Login,
                    Prompt::AlreadyRegistered => {
                        self.just_registered = true;
                        Action::Login
                    }
                    _ => Action::Login,
                };
                // tra un tentativo e l'altro almeno 6 secondi (i server ripetono il messaggio di continuo)
                if self.tries >= MAX_TRIES || self.last.is_some_and(|t| now.duration_since(t) < Duration::from_secs(6)) {
                    return None;
                }
                self.tries += 1;
                self.last = Some(now);
                self.sent = Some(want);
                Some(want)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn templates() {
        assert_eq!(render(DEFAULT_REGISTER, "abc123", "Makoto", "AlterEgo").unwrap(), "/register abc123 abc123");
        assert_eq!(render(DEFAULT_LOGIN, "abc123", "", "").unwrap(), "/login abc123");
        assert_eq!(render("  /l {pass}", "xyz9", "", "").unwrap(), "/l xyz9");
        assert_eq!(render("/party accept {player}", "", "Makoto", "AlterEgo").unwrap(), "/party accept Makoto");
        // mai in chat pubblica
        assert!(render("login {password}", "abc123", "", "").is_err());
        assert!(render("{password}", "abc123", "", "").is_err());
        assert!(render("/", "abc123", "", "").is_err());
        assert!(render("/login {password}", "", "", "").is_err(), "senza password");
        assert!(render("/login {password}", "a b", "", "").is_err(), "spazi");
        assert!(render(&format!("/x {}", "y".repeat(300)), "", "", "").is_err());
    }

    #[test]
    fn masks_passwords() {
        let s = vec!["hunter22".to_string(), "ab".to_string()];
        assert_eq!(mask("/login hunter22", &s), "/login ••••");
        assert_eq!(mask("/register hunter22 hunter22", &s), "/register •••• ••••");
        assert_eq!(mask("ab resta (troppo corta per coprirla)", &s), "ab resta (troppo corta per coprirla)");
        let nested = vec!["pass".to_string(), "password1".to_string()];
        assert_eq!(mask("x password1 y", &nested), "x •••• y");
        assert_eq!(mask_auth_cmd("/login segreta"), "/login ••••");
        assert_eq!(mask_auth_cmd("/REGISTER a b"), "/REGISTER •••• ••••");
        assert_eq!(mask_auth_cmd("/l x"), "/l ••••");
        assert_eq!(mask_auth_cmd("/warp spawn"), "/warp spawn");
        assert_eq!(mask_auth_cmd("ciao /login x"), "ciao /login x");
    }

    #[test]
    fn detects_prompts() {
        assert_eq!(detect("§cPlease, register to the server with the command: /register <password> <ConfirmPassword>"), Some(Prompt::Register));
        assert_eq!(detect("§cPlease, login with the command: /login <password>"), Some(Prompt::Login));
        assert_eq!(detect("Per favore, registrati con: /register <password> <password>"), Some(Prompt::Register));
        assert_eq!(detect("Effettua il login con /login <password>"), Some(Prompt::Login));
        assert_eq!(detect("Please log in using /l <password>"), Some(Prompt::Login));
        assert_eq!(detect("§aSuccessfully registered!"), Some(Prompt::Success));
        assert_eq!(detect("§2Successful login!"), Some(Prompt::Success));
        assert_eq!(detect("Login effettuato!"), Some(Prompt::Success));
        assert_eq!(detect("§cWrong password!"), Some(Prompt::Wrong));
        assert_eq!(detect("Password errata!"), Some(Prompt::Wrong));
        assert_eq!(detect("§cYou already have registered this username!"), Some(Prompt::AlreadyRegistered));
        assert_eq!(detect("§cThis user is already registered!"), Some(Prompt::AlreadyRegistered));
        assert_eq!(detect("This user isn't registered! Use /register"), Some(Prompt::NotRegistered));
        assert_eq!(detect("Register with /register or log in with /login"), Some(Prompt::Login));
        assert_eq!(detect("Makoto joined the game"), None);
        assert_eq!(detect(""), None);
    }

    #[test]
    fn session_flow() {
        let t0 = Instant::now();
        let mut s = Session::default();
        // account nuovo: registrazione
        assert_eq!(s.on_message(Prompt::Register, false, t0), Some(Action::Register));
        // il server ripete il messaggio subito: si aspetta
        assert_eq!(s.on_message(Prompt::Register, false, t0 + Duration::from_secs(2)), None);
        assert_eq!(s.on_message(Prompt::Success, false, t0 + Duration::from_secs(3)), None);
        assert!(s.done && s.just_registered);
        assert_eq!(s.on_message(Prompt::Login, true, t0 + Duration::from_secs(20)), None, "già fatto");

        // account già registrato: anche se chiede /register usa il login
        let mut s = Session::default();
        assert_eq!(s.on_message(Prompt::Register, true, t0), Some(Action::Login));
        // "già registrato" dopo una registrazione: passa al login
        let mut s = Session::default();
        assert_eq!(s.on_message(Prompt::Register, false, t0), Some(Action::Register));
        assert_eq!(s.on_message(Prompt::AlreadyRegistered, false, t0 + Duration::from_secs(7)), Some(Action::Login));
        assert!(s.just_registered);
        // tentativi limitati
        let mut s = Session::default();
        let mut n = 0;
        for i in 0..10 {
            if s.on_message(Prompt::Login, true, t0 + Duration::from_secs(i * 7)).is_some() {
                n += 1;
            }
        }
        assert_eq!(n, MAX_TRIES);
        // password sbagliata: si ferma
        let mut s = Session::default();
        assert_eq!(s.on_message(Prompt::Login, true, t0), Some(Action::Login));
        assert_eq!(s.on_message(Prompt::Wrong, true, t0 + Duration::from_secs(1)), None);
        assert!(s.wrong);
        assert_eq!(s.on_message(Prompt::Login, true, t0 + Duration::from_secs(30)), None);
    }

    #[test]
    fn passwords() {
        let a = generate();
        let b = generate();
        assert_eq!(a.len(), 16);
        assert_ne!(a, b);
        assert!(a.chars().all(|c| c.is_ascii_alphanumeric() && !"0O1lI".contains(c)));
        assert_eq!(slot("Play.Example.net:25565", "AlterEgo"), "alterego@play.example.net:25565");
        crate::keys::TEST_KEYS.with(|m| m.borrow_mut().insert("api-key:minecraft:alterego@x:1".into(), "segreta".into()));
        assert_eq!(get_password("x:1", "AlterEgo").as_deref(), Some("segreta"));
        assert!(set_password("x:1", "AlterEgo", "a b c").is_err());
        assert!(set_password("x:1", "AlterEgo", "abc").is_err());
    }
}
