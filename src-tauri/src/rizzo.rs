//! Rizzo Flow installato e avviato da Alter Ego (router locale, vedi `saving`).
//!
//! Installazione (pulsante nelle impostazioni, sempre con conferma):
//! 1. `uv` se manca (installer ufficiale di Astral);
//! 2. il repository in `<dati locali dell'app>/rizzo-flow` (git, oppure lo zip di GitHub);
//! 3. `uv sync --locked` e `uv run rizzo download [--size …]` (modello ~4,4 GB o ~1,8 GB);
//! poi il router passa a Rizzo Flow e il server si avvia con Alter Ego (`rizzo serve`), e si chiude con lei.

use serde::Serialize;
use std::{
    io::Read,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        Mutex,
    },
    time::{Duration, Instant},
};
use tauri::{AppHandle, Emitter, Manager};

const REPO: &str = "https://github.com/Rizzo-AI-Academy/rizzo-flow";
const ZIP: &str = "https://codeload.github.com/Rizzo-AI-Academy/rizzo-flow/zip/refs/heads/main";
const MARK: &str = "alterego-installed.txt";

static CHILD: Mutex<Option<Child>> = Mutex::new(None);
static INSTALLING: AtomicBool = AtomicBool::new(false);

fn custom_dir(app: &AppHandle) -> Option<PathBuf> {
    let d = app.state::<crate::AppState>().cfg.lock().unwrap().rizzo.dir.trim().to_string();
    (!d.is_empty()).then(|| PathBuf::from(d))
}

pub fn dir(app: &AppHandle) -> PathBuf {
    custom_dir(app).unwrap_or_else(|| app.path().app_local_data_dir().unwrap_or_else(|_| std::env::temp_dir()).join("rizzo-flow"))
}

/// Taglie del modello scaricate (dal file segno dell'installazione). In una cartella scelta
/// dall'utente il segno non c'è: si usa la taglia delle impostazioni.
fn installed_sizes(app: &AppHandle) -> Vec<String> {
    let v: Vec<String> = std::fs::read_to_string(dir(app).join(MARK)).unwrap_or_default().split_whitespace().map(str::to_string).collect();
    if v.is_empty() && custom_dir(app).is_some() {
        return vec![app.state::<crate::AppState>().cfg.lock().unwrap().rizzo.size.clone()];
    }
    v
}

/// È una cartella di Rizzo Flow? (pyproject.toml che dichiara il pacchetto `rizzo`)
fn is_rizzo_dir(d: &Path) -> bool {
    std::fs::read_to_string(d.join("pyproject.toml")).is_ok_and(|t| t.to_lowercase().contains("rizzo"))
}

pub fn installed(app: &AppHandle) -> bool {
    !installed_sizes(app).is_empty() && dir(app).join("pyproject.toml").is_file()
}

/// Usa un Rizzo Flow già scaricato altrove ("" = torna alla cartella gestita da Alter Ego).
#[tauri::command]
pub fn rizzo_set_dir(app: AppHandle, path: String) -> Result<String, String> {
    let p = path.trim().trim_matches('"').to_string();
    if !p.is_empty() && !is_rizzo_dir(Path::new(&p)) {
        return Err(crate::i18n::tr("in questa cartella non c'è Rizzo Flow (manca pyproject.toml)", "Rizzo Flow is not in this folder (pyproject.toml missing)").into());
    }
    stop(&app);
    crate::update_config(&app, |c| {
        c.rizzo.dir = p.clone();
        if !p.is_empty() {
            c.saving.router = "rizzo".into();
            if local_port(&c.saving.router_url).is_none() {
                c.saving.router_url = "http://127.0.0.1:8017".into();
            }
        }
    });
    Ok(dir(&app).display().to_string())
}

fn hidden(cmd: &mut Command) -> &mut Command {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    cmd
}

fn works(program: &Path, arg: &str) -> bool {
    hidden(Command::new(program).arg(arg)).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).status().is_ok_and(|s| s.success())
}

fn exe(name: &str) -> String {
    if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_string()
    }
}

/// `uv`: nel PATH o dove lo mette il suo installer (~/.local/bin, ~/.cargo/bin).
pub(crate) fn find_uv() -> Option<PathBuf> {
    let mut c = vec![PathBuf::from(exe("uv"))];
    if let Some(home) = std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" }).map(PathBuf::from) {
        c.push(home.join(".local").join("bin").join(exe("uv")));
        c.push(home.join(".cargo").join("bin").join(exe("uv")));
    }
    c.into_iter().find(|p| works(p, "--version"))
}

fn has_git() -> bool {
    works(Path::new(&exe("git")), "--version")
}

/// (host locale?, porta) dall'URL del router.
pub fn local_port(url: &str) -> Option<u16> {
    let rest = url.trim().strip_prefix("http://")?;
    let hostport = rest.split('/').next()?;
    let (host, port) = match hostport.rsplit_once(':') {
        Some((h, p)) => (h, p.parse().ok()?),
        None => (hostport, 80),
    };
    matches!(host, "127.0.0.1" | "localhost").then_some(port)
}

fn up(port: u16) -> bool {
    crate::alternet::url::reachable(&format!("http://127.0.0.1:{port}/"))
}

fn say(app: &AppHandle, phase: &str, text: impl Into<String>) {
    #[derive(Serialize, Clone)]
    struct Ev {
        phase: String,
        text: String,
    }
    let _ = app.emit("rizzo-status", Ev { phase: phase.into(), text: text.into() });
}

/// Esegue un comando mostrando l'ultima riga d'uscita (anche le barre di avanzamento con `\r`).
fn run_step(app: &AppHandle, log: &Path, label: &str, cmd: &mut Command) -> Result<(), String> {
    say(app, "install", label);
    let mut child = hidden(cmd)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .env("PYTHONIOENCODING", "utf-8")
        .env("PYTHONUNBUFFERED", "1")
        .spawn()
        .map_err(|e| format!("{label}: {e}"))?;
    let last = std::sync::Arc::new(Mutex::new((String::new(), Instant::now())));
    let mut readers = Vec::new();
    for stream in [child.stdout.take().map(|s| Box::new(s) as Box<dyn Read + Send>), child.stderr.take().map(|s| Box::new(s) as Box<dyn Read + Send>)].into_iter().flatten() {
        let (app, last, log, label) = (app.clone(), last.clone(), log.to_path_buf(), label.to_string());
        readers.push(std::thread::spawn(move || {
            let mut stream = stream;
            let mut buf = [0u8; 4096];
            let mut line = Vec::new();
            let mut file = std::fs::OpenOptions::new().create(true).append(true).open(&log).ok();
            while let Ok(n) = stream.read(&mut buf) {
                if n == 0 {
                    break;
                }
                if let Some(f) = file.as_mut() {
                    use std::io::Write;
                    let _ = f.write_all(&buf[..n]);
                }
                for &b in &buf[..n] {
                    if b == b'\n' || b == b'\r' {
                        let text = String::from_utf8_lossy(&line).trim().to_string();
                        line.clear();
                        if text.is_empty() {
                            continue;
                        }
                        let mut l = last.lock().unwrap();
                        l.0 = text.clone();
                        if l.1.elapsed() > Duration::from_millis(300) {
                            l.1 = Instant::now();
                            say(&app, "install", format!("{label} — {}", text.chars().take(160).collect::<String>()));
                        }
                    } else {
                        line.push(b);
                    }
                }
            }
        }));
    }
    let status = child.wait().map_err(|e| e.to_string())?;
    for r in readers {
        let _ = r.join();
    }
    if status.success() {
        Ok(())
    } else {
        let tail = last.lock().unwrap().0.clone();
        Err(crate::tr!("{} non riuscito ({}): {}", "{} failed ({}): {}", label, status, tail))
    }
}

/// `run_step` su un thread a parte (i passi durano anche decine di minuti).
pub(crate) async fn step(app: &AppHandle, log: &Path, label: &str, mut cmd: Command) -> Result<(), String> {
    let (app, log, label) = (app.clone(), log.to_path_buf(), label.to_string());
    tauri::async_runtime::spawn_blocking(move || run_step(&app, &log, &label, &mut cmd)).await.map_err(|e| e.to_string())?
}

pub(crate) async fn install_uv(app: &AppHandle, log: &Path) -> Result<PathBuf, String> {
    let mut cmd;
    #[cfg(windows)]
    {
        cmd = Command::new("powershell.exe");
        cmd.args(["-NoProfile", "-ExecutionPolicy", "ByPass", "-Command", "irm https://astral.sh/uv/install.ps1 | iex"]);
    }
    #[cfg(not(windows))]
    {
        cmd = Command::new("sh");
        cmd.args(["-c", "if command -v curl >/dev/null; then curl -LsSf https://astral.sh/uv/install.sh | sh; else wget -qO- https://astral.sh/uv/install.sh | sh; fi"]);
    }
    step(app, log, crate::i18n::tr("Installo uv", "Installing uv"), cmd).await?;
    find_uv().ok_or_else(|| crate::i18n::tr("uv installato ma non trovato: riavvia Alter Ego e riprova", "uv installed but not found: restart Alter Ego and try again").into())
}

async fn fetch_repo(app: &AppHandle, d: &Path, log: &Path) -> Result<(), String> {
    if d.join("pyproject.toml").is_file() {
        if d.join(".git").is_dir() && has_git() {
            // aggiorna, ma se non riesce va bene la copia che c'è
            let mut g = Command::new(exe("git"));
            g.args(["pull", "--ff-only"]).current_dir(d);
            let _ = step(app, log, crate::i18n::tr("Aggiorno Rizzo Flow", "Updating Rizzo Flow"), g).await;
        }
        return Ok(());
    }
    let parent = d.parent().ok_or("cartella")?;
    std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    if d.exists() {
        let _ = std::fs::remove_dir_all(d);
    }
    if has_git() {
        let mut g = Command::new(exe("git"));
        g.args(["clone", "--depth", "1", REPO]).arg(d);
        return step(app, log, crate::i18n::tr("Scarico Rizzo Flow (git)", "Downloading Rizzo Flow (git)"), g).await;
    }
    // senza git: lo zip del branch principale
    say(app, "install", crate::i18n::tr("Scarico Rizzo Flow (zip)", "Downloading Rizzo Flow (zip)"));
    let http = app.state::<crate::AppState>().ai.http.clone();
    let bytes = http.get(ZIP).header("User-Agent", "AlterEgo").send().await.and_then(|r| r.error_for_status()).map_err(|e| e.to_string())?.bytes().await.map_err(|e| e.to_string())?;
    let tmp = parent.join("rizzo-flow-download");
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).map_err(|e| e.to_string())?;
    let zip = tmp.join("rizzo-flow.zip");
    std::fs::write(&zip, &bytes).map_err(|e| e.to_string())?;
    let mut x;
    if cfg!(windows) {
        x = Command::new("tar"); // bsdtar di Windows 10+ apre gli zip
        x.arg("-xf").arg(&zip).arg("-C").arg(&tmp);
    } else {
        x = Command::new("unzip");
        x.arg("-q").arg(&zip).arg("-d").arg(&tmp);
    }
    step(app, log, crate::i18n::tr("Estraggo Rizzo Flow", "Extracting Rizzo Flow"), x).await?;
    let inner = std::fs::read_dir(&tmp).map_err(|e| e.to_string())?.flatten().map(|e| e.path()).find(|p| p.join("pyproject.toml").is_file()).ok_or("zip senza pyproject.toml")?;
    std::fs::rename(&inner, d).map_err(|e| e.to_string())?;
    let _ = std::fs::remove_dir_all(&tmp);
    Ok(())
}

async fn install_flow(app: &AppHandle, size: &str) -> Result<(), String> {
    let d = dir(app);
    std::fs::create_dir_all(d.parent().unwrap_or(&d)).map_err(|e| e.to_string())?;
    let log = d.parent().unwrap_or(&d).join("rizzo-install.log");
    let _ = std::fs::write(&log, "");
    let uv = match find_uv() {
        Some(u) => u,
        None => install_uv(app, &log).await?,
    };
    fetch_repo(app, &d, &log).await?;
    let mut sync = Command::new(&uv);
    sync.args(["sync", "--locked"]).current_dir(&d);
    step(app, &log, crate::i18n::tr("Preparo l'ambiente Python (uv sync)", "Preparing the Python environment (uv sync)"), sync).await?;
    let mut dl = Command::new(&uv);
    dl.args(["run", "rizzo", "download", "--size", size]).current_dir(&d);
    step(app, &log, crate::i18n::tr("Scarico il modello e llama.cpp (rizzo download)", "Downloading the model and llama.cpp (rizzo download)"), dl).await?;
    let mut sizes = installed_sizes(app);
    if !sizes.iter().any(|s| s == size) {
        sizes.push(size.to_string());
    }
    std::fs::write(d.join(MARK), sizes.join(" ")).map_err(|e| e.to_string())
}

/// Installa (o aggiorna) Rizzo Flow, poi lo usa come router e lo avvia.
pub async fn install(app: &AppHandle, size: &str) -> Result<String, String> {
    let size = if size == "1.7b" { "1.7b" } else { "4b" };
    if INSTALLING.swap(true, Ordering::SeqCst) {
        return Err(crate::i18n::tr("installazione già in corso", "installation already running").into());
    }
    let q = crate::tr!(
        "Installare Rizzo Flow in {}?\nScarico da GitHub il codice di Rizzo AI Academy, uv (se manca) e il modello {} (circa {}). Ci vuole un po' e serve spazio su disco.",
        "Install Rizzo Flow in {}?\nI'll download Rizzo AI Academy's code from GitHub, uv (if missing) and the {} model (about {}). It takes a while and needs disk space.",
        dir(app).display(),
        size.to_uppercase(),
        if size == "4b" { "4,4 GB" } else { "1,8 GB" }
    );
    if !crate::actions::confirm_always(app, q).await {
        INSTALLING.store(false, Ordering::SeqCst);
        return Err("l'utente ha negato il permesso".into());
    }
    stop(app);
    let r = install_flow(app, size).await;
    INSTALLING.store(false, Ordering::SeqCst);
    match r {
        Ok(()) => {
            crate::update_config(app, |c| {
                c.saving.router = "rizzo".into();
                c.rizzo.autostart = true;
                c.rizzo.size = size.into();
                if local_port(&c.saving.router_url).is_none() {
                    c.saving.router_url = "http://127.0.0.1:8017".into();
                }
            });
            crate::actions::log(app, "rizzo flow", &format!("installato ({size})"), "ok");
            let started = start(app);
            let text = match started {
                Ok(()) => crate::i18n::tr("Rizzo Flow installato: il server si sta avviando e da ora parte con Alter Ego.", "Rizzo Flow installed: the server is starting and from now on starts with Alter Ego.").to_string(),
                Err(e) => crate::tr!("Rizzo Flow installato, ma il server non è partito: {}", "Rizzo Flow installed, but the server didn't start: {}", e),
            };
            say(app, "done", text.clone());
            Ok(text)
        }
        Err(e) => {
            crate::actions::log(app, "rizzo flow", "installazione", &e);
            say(app, "error", e.clone());
            Err(e)
        }
    }
}

fn child_alive() -> bool {
    let mut c = CHILD.lock().unwrap();
    match c.as_mut().map(|ch| ch.try_wait()) {
        Some(Ok(None)) => true,
        Some(_) => {
            *c = None;
            false
        }
        None => false,
    }
}

/// Avvia `rizzo serve` sulla porta dell'URL del router (se è locale e non risponde già nessuno).
pub fn start(app: &AppHandle) -> Result<(), String> {
    let (url, size) = {
        let st = app.state::<crate::AppState>();
        let c = st.cfg.lock().unwrap();
        (c.saving.router_url.clone(), c.rizzo.size.clone())
    };
    let port = local_port(&url).ok_or_else(|| crate::i18n::tr("l'URL di Rizzo Flow non è locale: va avviato a parte", "Rizzo Flow's URL is not local: start it yourself").to_string())?;
    if child_alive() || up(port) {
        return Ok(());
    }
    if !installed(app) {
        return Err(crate::i18n::tr("Rizzo Flow non è installato", "Rizzo Flow is not installed").into());
    }
    let sizes = installed_sizes(app);
    let size = if sizes.contains(&size) { size } else { sizes[0].clone() };
    let uv = find_uv().ok_or_else(|| crate::i18n::tr("uv non trovato", "uv not found").to_string())?;
    let d = dir(app);
    let log = std::fs::File::create(d.join("alterego-serve.log")).map_err(|e| e.to_string())?;
    let mut cmd = Command::new(uv);
    // --no-sync: niente rete all'avvio, l'ambiente l'ha già preparato l'installazione
    cmd.args(["run", "--no-sync", "rizzo", "serve", "--host", "127.0.0.1", "--port", &port.to_string(), "--size", &size])
        .current_dir(&d)
        .stdin(Stdio::null())
        .stdout(log.try_clone().map_err(|e| e.to_string())?)
        .stderr(log)
        .env("PYTHONIOENCODING", "utf-8");
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0); // per chiudere anche Python insieme a uv
    }
    let child = hidden(&mut cmd).spawn().map_err(|e| crate::tr!("avvio di Rizzo Flow non riuscito: {e}", "could not start Rizzo Flow: {e}"))?;
    *CHILD.lock().unwrap() = Some(child);
    Ok(())
}

/// Ferma il server avviato da Alter Ego (non uno avviato a mano).
pub fn stop(_app: &AppHandle) {
    if let Some(mut c) = CHILD.lock().unwrap().take() {
        let pid = c.id().to_string();
        #[cfg(windows)]
        let _ = hidden(Command::new("taskkill").args(["/PID", &pid, "/T", "/F"])).stdout(Stdio::null()).stderr(Stdio::null()).status();
        #[cfg(unix)]
        let _ = Command::new("kill").args(["-TERM", &format!("-{pid}")]).stdout(Stdio::null()).stderr(Stdio::null()).status();
        let _ = c.kill();
        let _ = c.wait();
    }
}

/// All'avvio di Alter Ego.
pub fn autostart(app: &AppHandle) {
    let on = app.state::<crate::AppState>().cfg.lock().unwrap().rizzo.autostart;
    if on && installed(app) {
        let app = app.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_secs(2));
            if let Err(e) = start(&app) {
                crate::actions::log(&app, "rizzo flow", "avvio automatico", &e);
            }
        });
    }
}

#[derive(Serialize)]
pub struct Status {
    installed: bool,
    installing: bool,
    running: bool,
    managed: bool,
    uv: bool,
    git: bool,
    dir: String,
    sizes: Vec<String>,
    autostart: bool,
    size: String,
}

#[tauri::command]
pub async fn rizzo_status(app: AppHandle) -> Status {
    let (url, autostart, size) = {
        let st = app.state::<crate::AppState>();
        let c = st.cfg.lock().unwrap();
        (c.saving.router_url.clone(), c.rizzo.autostart, c.rizzo.size.clone())
    };
    let a = app.clone();
    let (size2, fallback_size) = (size.clone(), size.clone());
    tauri::async_runtime::spawn_blocking(move || Status {
        installed: installed(&a),
        installing: INSTALLING.load(Ordering::SeqCst),
        running: local_port(&url).is_some_and(up),
        managed: child_alive(),
        uv: find_uv().is_some(),
        git: has_git(),
        dir: dir(&a).display().to_string(),
        sizes: installed_sizes(&a),
        autostart,
        size: size2,
    })
    .await
    .unwrap_or(Status { installed: false, installing: false, running: false, managed: false, uv: false, git: false, dir: String::new(), sizes: vec![], autostart, size: fallback_size })
}

#[tauri::command]
pub async fn rizzo_install(app: AppHandle, size: String) -> Result<String, String> {
    install(&app, &size).await
}

#[tauri::command]
pub async fn rizzo_start(app: AppHandle) -> Result<(), String> {
    let a = app.clone();
    tauri::async_runtime::spawn_blocking(move || start(&a)).await.map_err(|e| e.to_string())?
}

#[tauri::command]
pub fn rizzo_stop(app: AppHandle) {
    stop(&app);
}

#[tauri::command]
pub fn rizzo_settings(app: AppHandle, autostart: bool, size: String) {
    crate::update_config(&app, |c| {
        c.rizzo.autostart = autostart;
        c.rizzo.size = if size == "1.7b" { "1.7b".into() } else { "4b".into() };
    });
}

#[tauri::command]
pub fn rizzo_open_dir(app: AppHandle) -> Result<(), String> {
    let d = dir(&app);
    let target = if d.exists() { d } else { d.parent().map(Path::to_path_buf).unwrap_or(d) };
    #[cfg(windows)]
    let r = Command::new("explorer").arg(&target).spawn();
    #[cfg(not(windows))]
    let r = Command::new("xdg-open").arg(&target).spawn();
    r.map(|_| ()).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ports() {
        assert_eq!(local_port("http://127.0.0.1:8017"), Some(8017));
        assert_eq!(local_port("http://localhost:9000/"), Some(9000));
        assert_eq!(local_port("http://192.168.1.5:8017"), None);
        assert_eq!(local_port("https://127.0.0.1:8017"), None);
    }
}
