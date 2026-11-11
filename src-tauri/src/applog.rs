//! Registro completo dell'app (`app.log`, una riga JSON per evento): richieste al modello, strumenti,
//! errori, avvisi, panic. Si vede in Impostazioni → Log. Mai chiavi o password: chi scrive qui
//! passa testo già ripulito (vedi `keys::redact`), e le password di SQL Server non arrivano mai qui.
//! Quando supera 5 MB il file diventa `app.log.1` e si ricomincia.

use serde::Serialize;
use serde_json::{json, Value};
use std::{
    io::Write,
    path::PathBuf,
    sync::{Mutex, OnceLock},
};
use tauri::{AppHandle, Emitter};

static PATH: OnceLock<PathBuf> = OnceLock::new();
static APP: OnceLock<AppHandle> = OnceLock::new();
static LOCK: Mutex<()> = Mutex::new(());
const MAX_BYTES: u64 = 5 * 1024 * 1024;

pub fn init(app: &AppHandle, data_dir: &std::path::Path) {
    let _ = PATH.set(data_dir.join("app.log"));
    let _ = APP.set(app.clone());
    // un panic in un thread viene scritto nel log invece di sparire
    let prev = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let where_ = info.location().map(|l| format!("{}:{}", l.file(), l.line())).unwrap_or_default();
        let msg = info
            .payload()
            .downcast_ref::<&str>()
            .map(|s| s.to_string())
            .or_else(|| info.payload().downcast_ref::<String>().cloned())
            .unwrap_or_default();
        write("error", "panic", &format!("{msg} ({where_})"));
        prev(info);
    }));
    write("info", "app", &format!("Alter Ego {} avviato ({})", crate::update::VERSION, std::env::consts::OS));
}

#[derive(Serialize, Clone)]
struct Line<'a> {
    ts: i64,
    level: &'a str,
    area: &'a str,
    msg: &'a str,
}

/// Scrive una riga. `level`: "info", "warn", "error", "debug".
pub fn write(level: &str, area: &str, msg: &str) {
    let msg: String = msg.chars().take(4000).collect();
    #[cfg(debug_assertions)]
    eprintln!("[{level}] {area}: {msg}");
    let Some(path) = PATH.get() else { return };
    let line = Line { ts: crate::memory::now(), level, area, msg: &msg };
    let Ok(text) = serde_json::to_string(&line) else { return };
    {
        let _g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
        if std::fs::metadata(path).map(|m| m.len() > MAX_BYTES).unwrap_or(false) {
            let _ = std::fs::rename(path, path.with_extension("log.1"));
        }
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
            let _ = writeln!(f, "{text}");
        }
    }
    if let Some(app) = APP.get() {
        let _ = app.emit("app-log", &line);
    }
}

pub fn info(area: &str, msg: &str) {
    write("info", area, msg)
}
pub fn warn(area: &str, msg: &str) {
    write("warn", area, msg)
}
pub fn error(area: &str, msg: &str) {
    write("error", area, msg)
}

/// Ultime righe del log (dalla più recente), con filtro facoltativo su livello e testo.
#[tauri::command]
pub fn app_log(limit: Option<usize>, level: Option<String>, query: Option<String>) -> Vec<Value> {
    let Some(path) = PATH.get() else { return Vec::new() };
    let mut text = std::fs::read_to_string(path.with_extension("log.1")).unwrap_or_default();
    text.push_str(&std::fs::read_to_string(path).unwrap_or_default());
    let level = level.unwrap_or_default();
    let q = query.unwrap_or_default().to_lowercase();
    text.lines()
        .rev()
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
        .filter(|v| level.is_empty() || level == "all" || v["level"] == json!(level) || (level == "warn" && v["level"] == "error"))
        .filter(|v| q.is_empty() || v["msg"].as_str().unwrap_or("").to_lowercase().contains(&q) || v["area"].as_str().unwrap_or("").to_lowercase().contains(&q))
        .take(limit.unwrap_or(1000).min(20_000))
        .collect()
}

#[tauri::command]
pub fn app_log_clear() -> Result<(), String> {
    let Some(path) = PATH.get() else { return Ok(()) };
    let _ = std::fs::remove_file(path.with_extension("log.1"));
    std::fs::write(path, b"").map_err(|e| e.to_string())
}

/// Percorso del file (per aprirlo o allegarlo a una segnalazione).
#[tauri::command]
pub fn app_log_path() -> String {
    PATH.get().map(|p| p.display().to_string()).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    #[test]
    fn writes_and_filters() {
        let dir = std::env::temp_dir().join(format!("ae-applog-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let _ = super::PATH.set(dir.join("app.log"));
        super::info("test", "prima riga");
        super::error("test", "qualcosa è andato storto");
        let all = super::app_log(Some(10), None, None);
        assert!(all.len() >= 2);
        assert_eq!(all[0]["level"], "error");
        let errs = super::app_log(Some(10), Some("error".into()), Some("storto".into()));
        assert_eq!(errs.len(), 1);
    }
}
