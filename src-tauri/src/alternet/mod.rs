//! Alter Net (gioco web multiplayer) dentro Alter Ego: una finestra WebView senza bordi, a schermo intero,
//! con un profilo WebView2 suo (%APPDATA%\AlterNet\WebView2), così nome, avatar e lingua del gioco restano
//! separati da quelli di Alter Ego.
//!
//! - Server: %APPDATA%\AlterNet\server.txt, altrimenti `DEFAULT`. Ogni indirizzo passa da `clean_url`.
//! - Se il server non risponde si apre la copia offline (`offline/game/index.html`, accanto all'exe o nelle
//!   risorse), che gioca in giocatore singolo; senza copia offline compare la pagina per cambiare indirizzo.
//! - Navigazione consentita solo verso il server, i file dentro `offline/`, `about:` e `data:`; nuove finestre negate.
//!   La finestra non ha accesso ai comandi di Alter Ego (nessuna capability).
//! - La pagina parla con l'app con `window.ipc.postMessage("fullscreen" | "server:<url>" | "offline")`:
//!   lo script iniettato intercetta questi messaggi e li passa attraverso il titolo del documento.
//!   F11 alterna schermo intero e finestra (sempre senza bordi), Esc due volte chiude.
//! - La copia offline non è distribuita con l'app: la costruisce `sync` scaricando dal server, mentre si gioca online,
//!   i file che il gioco ha usato (oppure si mette a mano una cartella `offline/` accanto all'exe).

pub mod sync;
pub mod url;

use base64::Engine;
use std::{
    path::{Component, Path, PathBuf},
    sync::Mutex,
};
use tauri::{AppHandle, Manager, Url, WebviewUrl, WebviewWindow, WebviewWindowBuilder};
use url::{clean_url, reachable};

pub const LABEL: &str = "alternet";
pub const DEFAULT: &str = "http://attentoweb.ddns.net:8080/";
const GAME: &str = "game/index.html";
const TITLE_TAG: &str = "\u{2063}alternet:";

/// Dove può andare la pagina: server attuale e cartella offline.
struct Allowed {
    server: String,
    offline: Option<PathBuf>,
}
static ALLOWED: Mutex<Allowed> = Mutex::new(Allowed { server: String::new(), offline: None });

fn data_dir() -> PathBuf {
    std::env::var_os("APPDATA").map(PathBuf::from).unwrap_or_else(std::env::temp_dir).join("AlterNet")
}

pub fn server_url() -> String {
    std::fs::read_to_string(data_dir().join("server.txt")).ok().and_then(|s| clean_url(&s)).unwrap_or_else(|| DEFAULT.to_string())
}

fn save_server(url: &str) {
    let _ = std::fs::create_dir_all(data_dir());
    let _ = std::fs::write(data_dir().join("server.txt"), url);
}

/// La copia offline del gioco: nelle risorse dell'installer o accanto all'exe (versione portable).
pub fn offline_dir(app: &AppHandle) -> Option<PathBuf> {
    let mut dirs = Vec::new();
    if let Ok(r) = app.path().resource_dir() {
        dirs.push(r.join("offline"));
    }
    if let Some(exe_dir) = std::env::current_exe().ok().and_then(|e| e.parent().map(Path::to_path_buf)) {
        dirs.push(exe_dir.join("offline"));
    }
    dirs.into_iter().find(|d| d.join(GAME).is_file()).or_else(sync::ready)
}

fn norm_component(c: &Component) -> String {
    let s = c.as_os_str().to_string_lossy().to_string();
    if cfg!(windows) {
        s.to_lowercase()
    } else {
        s
    }
}

/// `path` sta dentro `dir`? (niente `..`, maiuscole indifferenti su Windows)
fn inside(path: &Path, dir: &Path) -> bool {
    if path.components().any(|c| matches!(c, Component::ParentDir)) {
        return false;
    }
    let p: Vec<String> = path.components().filter(|c| !matches!(c, Component::CurDir)).map(|c| norm_component(&c)).collect();
    let d: Vec<String> = dir.components().filter(|c| !matches!(c, Component::CurDir)).map(|c| norm_component(&c)).collect();
    !d.is_empty() && p.len() > d.len() && p[..d.len()] == d[..]
}

fn allowed_nav(to: &Url, a: &Allowed) -> bool {
    match to.scheme() {
        "about" | "data" => true,
        "http" | "https" => !a.server.is_empty() && to.as_str().starts_with(&a.server),
        "file" => match (&a.offline, to.to_file_path()) {
            (Some(dir), Ok(p)) => inside(&p, dir),
            _ => false,
        },
        _ => false,
    }
}

fn game_url(server: &str) -> Option<Url> {
    Url::parse(&format!("{server}{GAME}")).ok()
}

fn offline_url(dir: &Path) -> Option<Url> {
    Url::from_file_path(dir.join(GAME)).ok()
}

/// Pagina per cambiare indirizzo. `change`: aperta apposta (non per errore).
fn start_page(server: &str, change: bool, offline: bool) -> Url {
    let html = include_str!("start.html")
        .replace("__SERVER__", server) // passato da clean_url: solo caratteri sicuri
        .replace("__MODE__", if change { "change" } else { "error" })
        .replace("__OFFLINE__", if offline { "true" } else { "false" });
    let b64 = base64::engine::general_purpose::STANDARD.encode(html);
    Url::parse(&format!("data:text/html;charset=utf-8;base64,{b64}")).expect("data url")
}

/// Iniettato in ogni pagina della finestra Alter Net.
const BRIDGE: &str = r#"(function () {
  if (window.__alternetBridge) return;
  window.__alternetBridge = true;
  var TAG = '\u2063alternet:', n = 0, saved = null, timer = 0;
  function relay(m) {
    if (saved === null) saved = document.title;
    document.title = TAG + (++n) + ':' + m;
    clearTimeout(timer);
    timer = setTimeout(function () { if (document.title.indexOf(TAG) === 0) document.title = saved; saved = null; }, 80);
  }
  // elenco delle risorse caricate dal gioco online, per la copia offline
  try { performance.setResourceTimingBufferSize(20000); } catch (e) {}
  function sendAssets() {
    if (!/^https?:$/.test(location.protocol) || !/\/game\/(index\.html)?$/.test(location.pathname)) return;
    var list = [location.pathname], seen = {};
    try {
      performance.getEntriesByType('resource').forEach(function (e) {
        var u = new URL(e.name);
        if (u.origin === location.origin && !seen[u.pathname]) { seen[u.pathname] = 1; list.push(u.pathname); }
      });
    } catch (e) {}
    var s = JSON.stringify(list), parts = [];
    for (var i = 0; i < s.length; i += 8000) parts.push(s.slice(i, i + 8000));
    var k = 0;
    (function next() { if (k < parts.length) { relay('assets:' + k + '/' + parts.length + ':' + parts[k]); k++; setTimeout(next, 50); } })();
  }
  addEventListener('load', function () { setTimeout(sendAssets, 8000); setTimeout(sendAssets, 90000); });
  function ours(s) { return typeof s === 'string' && (s === 'fullscreen' || s === 'offline' || s.indexOf('server:') === 0); }
  try {
    var wv = window.chrome && window.chrome.webview;
    if (wv) {
      var orig = wv.postMessage.bind(wv);
      var wrap = function (s) { if (ours(s)) { relay(s); return; } return orig(s); };
      try { wv.postMessage = wrap; } catch (e) {}
      if (wv.postMessage !== wrap) { try { Object.defineProperty(wv, 'postMessage', { value: wrap }); } catch (e) {} }
    }
  } catch (e) {}
  var lastEsc = 0;
  addEventListener('keydown', function (e) {
    if (e.key === 'F11') { e.preventDefault(); relay('fullscreen'); }
    else if (e.key === 'Escape' && !e.repeat) {
      var now = Date.now();
      if (now - lastEsc < 600) { lastEsc = 0; relay('close'); } else { lastEsc = now; }
    }
  }, true);
})();"#;

fn on_message(w: WebviewWindow, msg: String) {
    let here = w.url().ok();
    let from_start = here.as_ref().is_some_and(|u| u.scheme() == "data");
    if let Some(chunk) = msg.strip_prefix("assets:") {
        // solo dal gioco sul server attuale
        let server = ALLOWED.lock().unwrap().server.clone();
        if here.as_ref().is_some_and(|u| !server.is_empty() && u.as_str().starts_with(&server)) {
            sync::on_chunk(w.app_handle(), server, chunk);
        }
        return;
    }
    match msg.as_str() {
        "fullscreen" => {
            let full = w.is_fullscreen().unwrap_or(false);
            let _ = w.set_fullscreen(!full);
            if full {
                let _ = w.set_size(tauri::LogicalSize::new(1280.0, 720.0));
                let _ = w.center();
            }
        }
        "close" => {
            let _ = w.close();
        }
        "offline" if from_start => {
            let dir = ALLOWED.lock().unwrap().offline.clone();
            if let Some(u) = dir.as_deref().and_then(offline_url) {
                let _ = w.navigate(u);
            }
        }
        m => {
            // un nuovo indirizzo si accetta solo dalla pagina locale per cambiarlo, non dal gioco
            let Some(new) = m.strip_prefix("server:").filter(|_| from_start).and_then(clean_url) else { return };
            tauri::async_runtime::spawn(async move {
                let probe = new.clone();
                let ok = tauri::async_runtime::spawn_blocking(move || reachable(&probe)).await.unwrap_or(false);
                match (ok, game_url(&new)) {
                    (true, Some(u)) => {
                        save_server(&new);
                        ALLOWED.lock().unwrap().server = new;
                        let _ = w.navigate(u);
                    }
                    _ => {
                        let _ = w.eval(&format!("showError('{new}')"));
                    }
                }
            });
        }
    }
}

/// Cosa aprire: gioco online, copia offline o pagina dell'indirizzo.
enum Target {
    Online,
    Offline,
    Start,
}

async fn choose(app: &AppHandle, change: bool) -> (Url, Target) {
    let server = server_url();
    let offline = offline_dir(app);
    {
        let mut a = ALLOWED.lock().unwrap();
        a.server = server.clone();
        a.offline = offline.clone();
    }
    if change {
        return (start_page(&server, true, offline.is_some()), Target::Start);
    }
    let probe = server.clone();
    let up = tauri::async_runtime::spawn_blocking(move || reachable(&probe)).await.unwrap_or(false);
    if up {
        if let Some(u) = game_url(&server) {
            return (u, Target::Online);
        }
    }
    if let Some(u) = offline.as_deref().and_then(offline_url) {
        return (u, Target::Offline);
    }
    (start_page(&server, false, false), Target::Start)
}

/// Apre (o porta davanti) Alter Net. `change`: apre la pagina per cambiare indirizzo del server.
pub async fn open(app: &AppHandle, change: bool) -> Result<String, String> {
    use crate::i18n::tr;
    if let Some(w) = app.get_webview_window(LABEL) {
        if change {
            let (u, _) = choose(app, true).await;
            let _ = w.navigate(u);
        }
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
        return Ok(tr("Alter Net è già aperto.", "Alter Net is already open.").into());
    }
    let (url, target) = choose(app, change).await;
    let _ = std::fs::create_dir_all(data_dir().join("WebView2"));
    WebviewWindowBuilder::new(app, LABEL, WebviewUrl::External(url))
        .title("Alter Net")
        .inner_size(1280.0, 720.0)
        .min_inner_size(640.0, 360.0)
        .decorations(false)
        .fullscreen(true)
        .data_directory(data_dir().join("WebView2"))
        .initialization_script(BRIDGE)
        .on_navigation(|to| allowed_nav(to, &ALLOWED.lock().unwrap()))
        .on_new_window(|_, _| tauri::webview::NewWindowResponse::Deny)
        .on_document_title_changed(|w, title| {
            if let Some(rest) = title.strip_prefix(TITLE_TAG) {
                if let Some((_, msg)) = rest.split_once(':') {
                    on_message(w, msg.to_string());
                }
            }
        })
        .build()
        .map_err(|e| e.to_string())?;
    Ok(match target {
        Target::Online => tr("Alter Net aperto (online). F11 per la finestra, Esc due volte per chiudere.", "Alter Net opened (online). F11 for windowed, Esc twice to close."),
        Target::Offline => tr(
            "Il server di Alter Net non risponde: apro la versione offline (giocatore singolo). Esc due volte per chiudere.",
            "The Alter Net server doesn't answer: opening the offline version (single player). Esc twice to close.",
        ),
        Target::Start => tr("Alter Net aperto sulla pagina dell'indirizzo del server.", "Alter Net opened on the server address page."),
    }
    .into())
}

pub fn close(app: &AppHandle) -> bool {
    app.get_webview_window(LABEL).map(|w| w.close().is_ok()).unwrap_or(false)
}

#[derive(serde::Serialize)]
pub struct Info {
    server: String,
    offline: bool,
    open: bool,
}

#[tauri::command]
pub async fn alternet_open(app: AppHandle, change: Option<bool>) -> Result<String, String> {
    open(&app, change.unwrap_or(false)).await
}

#[tauri::command]
pub fn alternet_info(app: AppHandle) -> Info {
    Info { server: server_url(), offline: offline_dir(&app).is_some(), open: app.get_webview_window(LABEL).is_some() }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn a(offline: Option<&str>) -> Allowed {
        Allowed { server: DEFAULT.into(), offline: offline.map(PathBuf::from) }
    }

    #[test]
    fn navigation() {
        let off = if cfg!(windows) { r"C:\Apps\AlterEgo\offline" } else { "/opt/alterego/offline" };
        let al = a(Some(off));
        let ok = |s: &str| allowed_nav(&Url::parse(s).unwrap(), &al);
        assert!(ok("http://attentoweb.ddns.net:8080/game/index.html"));
        assert!(ok("about:blank"));
        assert!(ok("data:text/html,hi"));
        assert!(!ok("http://attentoweb.ddns.net:8081/game/"));
        assert!(!ok("https://evil.example/"));
        assert!(!ok("http://attentoweb.ddns.net.evil.example/"));
        assert!(!ok("javascript:alert(1)"));
        let file = |p: &str| ok(Url::from_file_path(p).unwrap().as_str());
        if cfg!(windows) {
            assert!(file(r"C:\Apps\AlterEgo\offline\game\index.html"));
            assert!(file(r"c:\apps\alterego\offline\Everyfile\a.png"));
            assert!(!file(r"C:\Apps\AlterEgo\config.json"));
            assert!(!ok("file:///C:/Apps/AlterEgo/offline/../config.json"));
        } else {
            assert!(file("/opt/alterego/offline/game/index.html"));
            assert!(file("/opt/alterego/offline/Everyfile/a.png"));
            assert!(!file("/opt/alterego/config.json"));
            assert!(!file("/opt/alterego/offline2/x.html"));
            assert!(!ok("file:///opt/alterego/offline/../config.json"));
        }
        assert!(!allowed_nav(&Url::parse("file:///etc/passwd").unwrap(), &a(None)));
    }

    #[test]
    fn start_page_is_data_url() {
        let u = start_page(DEFAULT, true, true);
        assert_eq!(u.scheme(), "data");
        let b64 = u.as_str().split_once("base64,").unwrap().1;
        let html = String::from_utf8(base64::engine::general_purpose::STANDARD.decode(b64).unwrap()).unwrap();
        assert!(html.contains("showInfo('http://attentoweb.ddns.net:8080/')"));
        assert!(html.contains("off.hidden = !true"));
        assert!(!html.contains("__"));
    }
}
