//! Copia offline di Alter Net costruita sul PC dell'utente, non distribuita con l'app (gli asset del gioco
//! non vanno nel repository pubblico). Quando il gioco online è aperto, la pagina manda l'elenco delle risorse
//! che ha caricato davvero (`performance.getEntriesByType('resource')`, solo i percorsi sullo stesso server); qui si scaricano dal server quelle
//! sotto `/game/` e `/Everyfile/` in `%APPDATA%\AlterNet\offline`, con la stessa struttura del server.
//! Il file `complete` segna una copia scaricata senza errori: solo allora si usa offline.

use std::{
    collections::HashSet,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Mutex,
    },
};
use tauri::{AppHandle, Manager, Url};

pub const MARK: &str = "complete";
const TOP: &[&str] = &["game", "Everyfile"];
const EXT: &[&str] = &["html", "js", "css", "json", "png", "jpg", "jpeg", "webp", "gif", "svg", "ogg", "mp3", "wav", "m4a", "woff", "woff2", "ttf", "otf"];
/// File piccoli che cambiano con il gioco: si riscaricano una volta per sessione anche se ci sono già.
const FRESH: &[&str] = &["html", "js", "css", "json"];
const MAX_FILES: usize = 20_000;
const MAX_BYTES: usize = 50 * 1024 * 1024;

pub fn root() -> PathBuf {
    super::data_dir().join("offline")
}

/// La copia in %APPDATA% è utilizzabile?
pub fn ready() -> Option<PathBuf> {
    let r = root();
    (r.join(MARK).is_file() && r.join(super::GAME).is_file()).then_some(r)
}

/// Percorso locale (relativo) di una risorsa del server, solo per /game/ e /Everyfile/.
pub fn rel_path(server: &str, url: &str) -> Option<PathBuf> {
    if !url.starts_with(server) {
        return None;
    }
    let u = Url::parse(url).ok()?;
    let segs: Vec<&str> = u.path_segments()?.collect();
    if segs.len() < 2 || !TOP.contains(&segs[0]) {
        return None;
    }
    let mut p = PathBuf::new();
    for s in &segs {
        let ok = !s.is_empty() && *s != "." && *s != ".." && s.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'));
        if !ok {
            return None;
        }
        p.push(s);
    }
    let ext = Path::new(segs[segs.len() - 1]).extension()?.to_str()?.to_ascii_lowercase();
    EXT.contains(&ext.as_str()).then_some(p)
}

/// Pezzi dell'elenco in arrivo dalla pagina: "k/n:testo".
static PARTS: Mutex<Vec<Option<String>>> = Mutex::new(Vec::new());
static RUNNING: AtomicBool = AtomicBool::new(false);
static FRESHENED: Mutex<Option<HashSet<String>>> = Mutex::new(None);

pub fn on_chunk(app: &AppHandle, server: String, msg: &str) {
    let Some((head, body)) = msg.split_once(':') else { return };
    let Some((k, n)) = head.split_once('/') else { return };
    let (Ok(k), Ok(n)) = (k.parse::<usize>(), n.parse::<usize>()) else { return };
    if n == 0 || n > 2000 || k >= n {
        return;
    }
    let full = {
        let mut parts = PARTS.lock().unwrap();
        if k == 0 || parts.len() != n {
            *parts = vec![None; n];
        }
        parts[k] = Some(body.to_string());
        if parts.iter().all(Option::is_some) {
            let s: String = parts.drain(..).map(Option::unwrap).collect();
            Some(s)
        } else {
            None
        }
    };
    let Some(json) = full else { return };
    // percorsi sul server ("/game/index.html"…)
    let Ok(paths) = serde_json::from_str::<Vec<String>>(&json) else { return };
    let urls: Vec<String> = paths.iter().filter(|p| p.starts_with('/')).map(|p| format!("{server}{}", p.trim_start_matches('/'))).collect();
    if RUNNING.swap(true, Ordering::SeqCst) {
        return;
    }
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        download(&app, &server, urls).await;
        RUNNING.store(false, Ordering::SeqCst);
    });
}

async fn download(app: &AppHandle, server: &str, urls: Vec<String>) {
    let http = app.state::<crate::AppState>().ai.http.clone();
    let root = root();
    let mut seen = HashSet::new();
    let (mut got, mut failed) = (0usize, 0usize);
    let mut have_game = false;
    for url in urls.into_iter().take(MAX_FILES) {
        let Some(rel) = rel_path(server, &url) else { continue };
        if !seen.insert(rel.clone()) {
            continue;
        }
        have_game |= rel == Path::new(super::GAME);
        let dest = root.join(&rel);
        let ext = rel.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
        let fresh = FRESH.contains(&ext.as_str()) && FRESHENED.lock().unwrap().get_or_insert_with(HashSet::new).insert(url.clone());
        if dest.is_file() && !fresh {
            continue;
        }
        let res = async {
            let r = http.get(&url).send().await.and_then(|r| r.error_for_status()).map_err(|e| e.to_string())?;
            let b = r.bytes().await.map_err(|e| e.to_string())?;
            if b.len() > MAX_BYTES {
                return Err("file troppo grande".to_string());
            }
            let parent = dest.parent().ok_or("percorso")?;
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            let tmp = dest.with_extension(format!("{ext}.part"));
            std::fs::write(&tmp, &b).map_err(|e| e.to_string())?;
            std::fs::rename(&tmp, &dest).map_err(|e| e.to_string())
        }
        .await;
        match res {
            Ok(()) => got += 1,
            Err(_) => failed += 1,
        }
    }
    if failed == 0 && (have_game || root.join(super::GAME).is_file()) {
        let _ = std::fs::write(root.join(MARK), crate::update::VERSION);
    }
    if got > 0 || failed > 0 {
        crate::actions::log(app, "alter net offline", &format!("{got} file scaricati, {failed} errori"), if failed == 0 { "ok" } else { "parziale" });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const S: &str = "http://attentoweb.ddns.net:8080/";

    #[test]
    fn paths() {
        let p = |u: &str| rel_path(S, u).map(|p| p.to_string_lossy().replace('\\', "/"));
        assert_eq!(p("http://attentoweb.ddns.net:8080/game/index.html").as_deref(), Some("game/index.html"));
        assert_eq!(p("http://attentoweb.ddns.net:8080/game/assets/rooms/map_001.png?v=2").as_deref(), Some("game/assets/rooms/map_001.png"));
        assert_eq!(
            p("http://attentoweb.ddns.net:8080/Everyfile/char_001/frames/anim_wait_2/anim_wait_2_00.png").as_deref(),
            Some("Everyfile/char_001/frames/anim_wait_2/anim_wait_2_00.png")
        );
        for bad in [
            "http://attentoweb.ddns.net:8080/game/../secret.js",
            "http://attentoweb.ddns.net:8080/game/%2e%2e/x.js",
            "http://attentoweb.ddns.net:8080/server.js",
            "http://attentoweb.ddns.net:8080/events",
            "http://attentoweb.ddns.net:8080/game/run.exe",
            "http://attentoweb.ddns.net:8080/game/",
            "https://evil.example/game/index.html",
            "http://attentoweb.ddns.net:8080/game/a%5Cb.js",
        ] {
            assert_eq!(p(bad), None, "{bad}");
        }
    }
}
