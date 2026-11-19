//! Console Minecraft: tutto quello che succede al bot in diretta (chat del server, eventi, comandi
//! mandati, errori, ponte ViaProxy, login mascherati) per la finestra mc-console.html, più una riga
//! per scrivere in chat o mandare comandi dall'account di Alter Ego.
//!
//! In memoria resta un buffer limitato; su disco un file ruotato (`minecraft/console.log`, più
//! `console.1.log`… fino a 3 vecchi). Le password conosciute vengono coperte prima di tutto.

use serde::Serialize;
use std::{
    collections::VecDeque,
    io::Write,
    path::{Path, PathBuf},
    sync::Mutex,
};
use tauri::{AppHandle, Emitter, Manager};

/// Righe tenute in memoria.
pub const CAP: usize = 2000;
/// Il file ruota oltre questa dimensione.
pub const FILE_MAX: u64 = 1024 * 1024;
const KEEP: usize = 3;
pub const WINDOW: &str = "mc-console";

/// Tipi di riga (filtri della console).
pub const KINDS: &[&str] = &["chat", "sys", "event", "cmd", "error", "via", "auth"];

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Entry {
    pub id: u64,
    /// Millisecondi dal 1970.
    pub ts: i64,
    pub kind: String,
    pub text: String,
}

#[derive(Default)]
struct Console {
    buf: VecDeque<Entry>,
    next: u64,
    secrets: Vec<String>,
    file: Option<PathBuf>,
}

static CONSOLE: Mutex<Option<Console>> = Mutex::new(None);

fn with<R>(f: impl FnOnce(&mut Console) -> R) -> R {
    let mut g = CONSOLE.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    f(g.get_or_insert_with(Console::default))
}

/// Password da coprire in ogni riga (aggiunta quando si legge dal portachiavi).
pub fn add_secret(s: &str) {
    if s.chars().count() < 3 {
        return;
    }
    with(|c| {
        if !c.secrets.iter().any(|x| x == s) {
            c.secrets.push(s.to_string());
        }
    });
}

/// Copre le password conosciute e quelle dei comandi di login scritti a mano.
pub fn redact(text: &str) -> String {
    let secrets = with(|c| c.secrets.clone());
    super::auth::mask(&super::auth::mask_auth_cmd(text), &secrets)
}

/// Una riga leggibile: niente codici ANSI o colori di Minecraft, niente a capo, al massimo 600 caratteri.
pub fn clean(text: &str) -> String {
    let s = super::via::strip_ansi(text);
    let mut out = String::with_capacity(s.len());
    let mut it = s.chars();
    while let Some(c) = it.next() {
        match c {
            '§' => {
                it.next();
            }
            '\n' | '\r' | '\t' => out.push(' '),
            c if c.is_control() => {}
            c => out.push(c),
        }
    }
    let t = out.trim();
    if t.chars().count() > 600 {
        format!("{}…", t.chars().take(600).collect::<String>())
    } else {
        t.to_string()
    }
}

/// Ruota il file se è troppo grande: console.log → console.1.log → console.2.log …
pub fn rotate(path: &Path, max: u64, keep: usize) {
    if std::fs::metadata(path).map(|m| m.len() < max).unwrap_or(true) {
        return;
    }
    let name = |i: usize| path.with_file_name(format!("{}.{i}.log", path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default()));
    let _ = std::fs::remove_file(name(keep));
    for i in (1..keep).rev() {
        let _ = std::fs::rename(name(i), name(i + 1));
    }
    let _ = std::fs::rename(path, name(1));
}

fn now_ms() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}

/// Aggiunge una riga (coperta e pulita) senza app: per i test e per i thread senza handle.
fn push(kind: &str, text: &str) -> Option<(Entry, Option<PathBuf>)> {
    let kind = if KINDS.contains(&kind) { kind } else { "event" };
    let text = redact(&clean(text));
    if text.is_empty() {
        return None;
    }
    Some(with(|c| {
        c.next += 1;
        let e = Entry { id: c.next, ts: now_ms(), kind: kind.to_string(), text };
        c.buf.push_back(e.clone());
        while c.buf.len() > CAP {
            c.buf.pop_front();
        }
        (e, c.file.clone())
    }))
}

/// Scrive una riga nella console (finestra aperta + file).
pub fn log(app: &AppHandle, kind: &str, text: &str) {
    if with(|c| c.file.is_none()) {
        let p = super::node::root(app).join("console.log");
        with(|c| c.file = Some(p));
    }
    let Some((e, file)) = push(kind, text) else { return };
    if let Some(p) = file {
        if let Some(d) = p.parent() {
            let _ = std::fs::create_dir_all(d);
        }
        rotate(&p, FILE_MAX, KEEP);
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&p) {
            let t = chrono_like(e.ts);
            let _ = writeln!(f, "{t} [{}] {}", e.kind, e.text);
        }
    }
    let _ = app.emit("minecraft-console", &e);
}

/// "2026-11-19 18:04:05" in UTC (senza dipendenze).
fn chrono_like(ms: i64) -> String {
    let s = ms / 1000;
    let (days, rem) = (s.div_euclid(86400), s.rem_euclid(86400));
    // giorni → data civile (algoritmo di Howard Hinnant)
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{y:04}-{m:02}-{d:02} {:02}:{:02}:{:02}Z", rem / 3600, rem % 3600 / 60, rem % 60)
}

/// Righe dopo `since` (0 = tutte).
pub fn list(since: u64) -> Vec<Entry> {
    with(|c| c.buf.iter().filter(|e| e.id > since).cloned().collect())
}

pub fn clear() {
    with(|c| c.buf.clear());
}

/// Comandi e chat mandati al bot (dalla mente o dalla console): una riga "cmd".
pub fn on_send(app: &AppHandle, op: &serde_json::Value) {
    match op["op"].as_str().unwrap_or("") {
        "say" => log(app, "cmd", &format!("» {}", op["text"].as_str().unwrap_or(""))),
        // comandi con la password: il testo coperto arriva già in "shown"
        "cmd" => log(app, if op["auth"].as_bool().unwrap_or(false) { "auth" } else { "cmd" }, op["shown"].as_str().or(op["text"].as_str()).unwrap_or("")),
        "connect" => log(app, "event", &crate::tr!("connessione a {}:{} come {}", "connecting to {}:{} as {}", op["host"].as_str().unwrap_or("?"), op["port"], op["username"].as_str().unwrap_or("?"))),
        "leave" => log(app, "event", crate::i18n::tr("uscita dal mondo", "leaving the world")),
        _ => {}
    }
}

/// Una riga dalla console: chat se è testo, comando se inizia con "/".
/// Le password conosciute non vanno mai in chat pubblica.
pub fn prepare_input(text: &str, secrets: &[String]) -> Result<serde_json::Value, String> {
    let t = text.trim();
    if t.is_empty() {
        return Err(crate::i18n::tr("niente da mandare", "nothing to send").into());
    }
    if t.chars().count() > 256 || t.contains('\n') {
        return Err(crate::i18n::tr("massimo 256 caratteri su una riga", "at most 256 characters on one line").into());
    }
    let secret = secrets.iter().any(|s| s.chars().count() >= 3 && t.contains(s.as_str()));
    if t.starts_with('/') {
        let shown = super::auth::mask(&super::auth::mask_auth_cmd(t), secrets);
        return Ok(serde_json::json!({ "op": "cmd", "text": t, "shown": shown, "auth": secret || shown != t }));
    }
    if secret {
        return Err(crate::i18n::tr("questa riga contiene la password: in chat pubblica non la scrivo", "this line contains the password: I won't write it in public chat").into());
    }
    Ok(serde_json::json!({ "op": "say", "text": t }))
}

// ---------- comandi ----------

#[tauri::command]
pub fn minecraft_console_list(since: Option<u64>) -> Vec<Entry> {
    list(since.unwrap_or(0))
}

#[tauri::command]
pub fn minecraft_console_clear() {
    clear();
}

/// Manda una riga dalla console (chat o comando) dall'account di Alter Ego.
#[tauri::command]
pub fn minecraft_console_send(app: AppHandle, text: String) -> Result<(), String> {
    if !super::running(&app) {
        return Err(crate::i18n::tr("Alter Ego non è nel mondo", "Alter Ego isn't in the world").into());
    }
    let secrets = with(|c| c.secrets.clone());
    let op = prepare_input(&text, &secrets)?;
    if super::send(&app, vec![op]).is_empty() {
        return Err(crate::i18n::tr("il bot non risponde", "the bot isn't answering").into());
    }
    Ok(())
}

/// Apre la finestra della console (o la porta davanti).
#[tauri::command]
// async: su Windows una finestra creata da un comando sincrono resta bianca
pub async fn minecraft_console_open(app: AppHandle) -> Result<(), String> {
    use tauri::{WebviewUrl, WebviewWindowBuilder};
    if let Some(w) = app.get_webview_window(WINDOW) {
        let _ = w.unminimize();
        let _ = w.show();
        return w.set_focus().map_err(|e| e.to_string());
    }
    WebviewWindowBuilder::new(&app, WINDOW, WebviewUrl::App("mc-console.html".into()))
        .title(crate::i18n::tr("Alter Ego – Console Minecraft", "Alter Ego – Minecraft Console"))
        .inner_size(1000.0, 660.0)
        .min_inner_size(560.0, 420.0)
        .center()
        .build()
        .map(|_| ())
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn buffer_and_masking() {
        add_secret("hunter22");
        let (e, _) = push("auth", "/login hunter22").unwrap();
        assert_eq!(e.text, "/login ••••");
        let (e2, _) = push("strano", "§aCiao\x1b[32m mondo\n!").unwrap();
        assert_eq!(e2.kind, "event");
        assert_eq!(e2.text, "Ciao mondo !");
        assert!(push("sys", "   ").is_none());
        assert!(list(e.id - 1).iter().any(|x| x.id == e2.id));
        for i in 0..(CAP + 10) {
            push("chat", &format!("riga {i}"));
        }
        assert!(list(0).len() <= CAP);
        assert!(clean(&"x".repeat(700)).chars().count() <= 601);
    }

    #[test]
    fn input_lines() {
        let s = vec!["segreta9".to_string()];
        let c = prepare_input("/warp spawn", &s).unwrap();
        assert_eq!((c["op"].as_str(), c["auth"].as_bool()), (Some("cmd"), Some(false)));
        let c = prepare_input("/login segreta9", &s).unwrap();
        assert_eq!(c["shown"], "/login ••••");
        assert_eq!(c["auth"], true);
        let c = prepare_input("/msg Makoto la password è segreta9", &s).unwrap();
        assert_eq!(c["shown"], "/msg Makoto la password è ••••");
        assert_eq!(prepare_input("ciao a tutti", &s).unwrap()["op"], "say");
        assert!(prepare_input("la mia password è segreta9", &s).is_err(), "mai in chat pubblica");
        assert!(prepare_input("  ", &s).is_err());
        assert!(prepare_input(&"x".repeat(300), &s).is_err());
    }

    #[test]
    fn rotates_files() {
        let d = std::env::temp_dir().join(format!("ae-mc-console-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        let p = d.join("console.log");
        for round in 0..5 {
            std::fs::write(&p, vec![b'x'; 200 + round]).unwrap();
            rotate(&p, 100, 3);
            assert!(!p.exists());
        }
        assert!(d.join("console.1.log").exists() && d.join("console.3.log").exists());
        assert!(!d.join("console.4.log").exists());
        // l'ultimo ruotato è il più recente
        assert_eq!(std::fs::metadata(d.join("console.1.log")).unwrap().len(), 204);
        std::fs::write(&p, b"poco").unwrap();
        rotate(&p, 100, 3);
        assert!(p.exists());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn dates() {
        assert_eq!(chrono_like(0), "1970-01-01 00:00:00Z");
        assert_eq!(chrono_like(1_795_111_200_000), "2026-11-19 18:00:00Z");
    }
}
