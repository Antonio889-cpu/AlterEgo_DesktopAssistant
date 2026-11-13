//! Hermes Agent (Nous Research, MIT) come motore esterno: installazione ufficiale (con conferma),
//! `hermes setup` in una finestra di terminale, e uso da Alter Ego:
//! - strumento `hermes_task` (attivo se Hermes è installato, conferma a ogni uso) per delegare compiti lunghi;
//! - modalità "cervello" (spenta di default): ogni messaggio va a Hermes (`hermes -z`), Alter Ego fa voce e faccia.
//! Fonte: hermes-agent.nousresearch.com/docs (install.ps1, CLI reference: `-z` stampa solo la risposta finale).

use std::{
    path::{Path, PathBuf},
    process::Command,
    time::Duration,
};
use tauri::AppHandle;

const INSTALL_WIN: &str = "iex (irm https://hermes-agent.nousresearch.com/install.ps1)";
const INSTALL_UNIX: &str = "curl -fsSL https://hermes-agent.nousresearch.com/install.sh | bash";

/// Primo eseguibile esistente tra quelli nel PATH e nelle cartelle indicate (anche una sottocartella sotto).
pub fn which(names: &[&str], extra: &[PathBuf]) -> Option<PathBuf> {
    let exts: &[&str] = if cfg!(windows) { &[".exe", ".cmd", ".bat"] } else { &[""] };
    let mut dirs: Vec<PathBuf> = std::env::var_os("PATH").map(|p| std::env::split_paths(&p).collect()).unwrap_or_default();
    for d in extra {
        dirs.push(d.clone());
        dirs.extend(std::fs::read_dir(d).into_iter().flatten().flatten().map(|e| e.path()).filter(|p| p.is_dir()).flat_map(|p| [p.join("bin"), p.join("Scripts"), p]));
    }
    dirs.iter().flat_map(|d| names.iter().flat_map(move |n| exts.iter().map(move |e| d.join(format!("{n}{e}"))))).find(|p| p.is_file())
}

fn home(var: &str) -> PathBuf {
    PathBuf::from(std::env::var_os(var).unwrap_or_default())
}

pub fn find() -> Option<PathBuf> {
    which(&["hermes"], &[home("LOCALAPPDATA").join("hermes"), home(if cfg!(windows) { "USERPROFILE" } else { "HOME" }).join(".local").join("bin")])
}

fn command(exe: &Path) -> Command {
    // i .cmd vanno lanciati con cmd /c
    let mut c = if exe.extension().is_some_and(|e| e.eq_ignore_ascii_case("cmd") || e.eq_ignore_ascii_case("bat")) {
        let mut c = Command::new("cmd");
        c.arg("/c").arg(exe);
        c
    } else {
        Command::new(exe)
    };
    c.env("PYTHONIOENCODING", "utf-8");
    c
}

/// Esegue un compito con `hermes -z` e restituisce la risposta finale.
pub async fn ask(prompt: &str, timeout: Duration) -> Result<String, String> {
    let exe = find().ok_or_else(|| crate::i18n::tr("Hermes non è installato", "Hermes is not installed").to_string())?;
    let mut cmd = tokio::process::Command::from(command(&exe));
    cmd.arg("-z").arg(prompt).stdin(std::process::Stdio::null()).kill_on_drop(true);
    #[cfg(windows)]
    cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    let out = tokio::time::timeout(timeout, cmd.output())
        .await
        .map_err(|_| crate::tr!("Hermes non ha finito entro {} minuti", "Hermes did not finish within {} minutes", timeout.as_secs() / 60))?
        .map_err(|e| e.to_string())?;
    let text = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if out.status.success() && !text.is_empty() {
        return Ok(text);
    }
    let err = String::from_utf8_lossy(&out.stderr);
    let tail: String = err.lines().rev().take(6).collect::<Vec<_>>().into_iter().rev().collect::<Vec<_>>().join("\n");
    Err(crate::tr!("Hermes: {} {}", "Hermes: {} {}", if text.is_empty() { "nessuna risposta" } else { &text }, tail))
}

#[derive(serde::Serialize)]
pub struct Status {
    installed: bool,
    path: String,
}

#[tauri::command]
pub fn hermes_status() -> Status {
    let p = find();
    Status { installed: p.is_some(), path: p.map(|p| p.display().to_string()).unwrap_or_default() }
}

/// Installa Hermes con lo script ufficiale (chiede conferma), poi apre `hermes setup`.
#[tauri::command]
pub async fn hermes_install(app: AppHandle) -> Result<String, String> {
    let q = crate::i18n::tr(
        "Installare Hermes Agent di Nous Research?\nUso lo script ufficiale (hermes-agent.nousresearch.com): scarica Python, Node e gli strumenti che servono in %LOCALAPPDATA%\\hermes. Poi si apre la configurazione guidata (scegli il modello AI).",
        "Install Nous Research's Hermes Agent?\nI'll use the official script (hermes-agent.nousresearch.com): it downloads Python, Node and the tools it needs into %LOCALAPPDATA%\\hermes. Then the setup wizard opens (pick the AI model).",
    );
    if !crate::actions::confirm_always(&app, q.into()).await {
        return Err("l'utente ha negato il permesso".into());
    }
    let log = std::env::temp_dir().join("alterego-hermes-install.log");
    let mut cmd;
    if cfg!(windows) {
        cmd = Command::new("powershell.exe");
        cmd.args(["-NoProfile", "-ExecutionPolicy", "ByPass", "-Command", INSTALL_WIN]);
    } else {
        cmd = Command::new("bash");
        cmd.args(["-c", INSTALL_UNIX]);
    }
    crate::rizzo::step(&app, &log, crate::i18n::tr("Installo Hermes", "Installing Hermes"), cmd).await?;
    let exe = find().ok_or_else(|| crate::i18n::tr("Hermes installato ma non trovato: riavvia Alter Ego", "Hermes installed but not found: restart Alter Ego").to_string())?;
    crate::actions::log(&app, "hermes", "installato", "ok");
    setup(&exe)?;
    Ok(crate::i18n::tr("Hermes installato: completa la configurazione nella finestra che si è aperta.", "Hermes installed: finish the setup in the window that opened.").into())
}

fn setup(exe: &Path) -> Result<(), String> {
    #[cfg(windows)]
    let r = Command::new("cmd").args(["/c", "start", "Hermes setup"]).arg(exe).arg("setup").spawn();
    #[cfg(not(windows))]
    let r = Command::new("x-terminal-emulator").arg("-e").arg(exe).arg("setup").spawn();
    r.map(|_| ()).map_err(|e| e.to_string())
}

/// Apre la configurazione guidata di Hermes (modello, chiavi) in un terminale.
#[tauri::command]
pub fn hermes_setup() -> Result<(), String> {
    setup(&find().ok_or_else(|| crate::i18n::tr("Hermes non è installato", "Hermes is not installed").to_string())?)
}

#[tauri::command]
pub fn hermes_settings(app: AppHandle, brain: bool) {
    crate::update_config(&app, |c| c.hermes_brain = brain);
}

#[cfg(test)]
mod tests {
    #[test]
    fn which_finds_in_subfolder() {
        let d = std::env::temp_dir().join(format!("ae-which-{}", std::process::id()));
        let bin = d.join("app").join("bin");
        std::fs::create_dir_all(&bin).unwrap();
        let name = if cfg!(windows) { "tool-xyz.exe" } else { "tool-xyz" };
        std::fs::write(bin.join(name), "").unwrap();
        assert_eq!(super::which(&["tool-xyz"], &[d.clone()]), Some(bin.join(name)));
        assert_eq!(super::which(&["nope-xyz"], &[d.clone()]), None);
        let _ = std::fs::remove_dir_all(&d);
    }
}
