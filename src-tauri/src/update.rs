//! Link al sito e a GitHub, e aggiornamento all'ultima release pubblicata su GitHub:
//! scarica l'installer, controlla lo SHA-256 (SHA256SUMS.txt della release), chiede conferma,
//! lo avvia e chiude Alter Ego (impostazioni, chiavi e memoria restano).

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};

pub const REPO: &str = "Antonio889-cpu/AlterEgo_DesktopAssistant";
pub const SITE: &str = "https://alterego.attentoweb.it";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

pub fn github_url() -> String {
    format!("https://github.com/{REPO}")
}

pub fn releases_url() -> String {
    format!("https://github.com/{REPO}/releases/latest")
}

#[derive(Deserialize)]
struct GhAsset {
    name: String,
    browser_download_url: String,
    #[serde(default)]
    size: u64,
}

#[derive(Deserialize)]
struct GhRelease {
    tag_name: String,
    #[serde(default)]
    html_url: String,
    #[serde(default)]
    body: Option<String>,
    #[serde(default)]
    assets: Vec<GhAsset>,
}

#[derive(Serialize, Clone, Debug, Default)]
pub struct UpdateInfo {
    pub current: String,
    pub latest: String,
    pub newer: bool,
    pub page: String,
    pub notes: String,
    pub size: u64,
    #[serde(skip)]
    setup_url: Option<(String, String)>,
    #[serde(skip)]
    sums_url: Option<String>,
}

/// "v1.10.2" → [1, 10, 2]
fn parse(v: &str) -> Vec<u64> {
    v.trim().trim_start_matches(['v', 'V']).split(['.', '-', '+']).take(3).map(|p| p.parse().unwrap_or(0)).collect()
}

pub fn is_newer(latest: &str, current: &str) -> bool {
    let (mut a, mut b) = (parse(latest), parse(current));
    a.resize(3, 0);
    b.resize(3, 0);
    a > b
}

fn status(app: &AppHandle, text: &str) {
    let _ = app.emit("update-status", text.to_string());
}

pub async fn check(app: &AppHandle) -> Result<UpdateInfo, String> {
    let http = app.state::<crate::AppState>().ai.http.clone();
    let res = http
        .get(format!("https://api.github.com/repos/{REPO}/releases/latest"))
        .header("User-Agent", "AlterEgo-updater")
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .map_err(|e| crate::tr!("GitHub non raggiungibile: {e}", "GitHub unreachable: {e}"))?;
    if !res.status().is_success() {
        return Err(crate::tr!("GitHub ha risposto {} (nessuna release pubblica?)", "GitHub answered {} (no public release?)", res.status()));
    }
    let r: GhRelease = res.json().await.map_err(|e| e.to_string())?;
    // Windows: l'installer; Linux: l'AppImage (si sostituisce da sola)
    let suffix = if cfg!(windows) { "_setup.exe" } else { ".appimage" };
    let setup = r.assets.iter().find(|a| a.name.to_lowercase().ends_with(suffix));
    Ok(UpdateInfo {
        current: VERSION.into(),
        latest: r.tag_name.trim_start_matches('v').to_string(),
        newer: is_newer(&r.tag_name, VERSION),
        page: if r.html_url.is_empty() { releases_url() } else { r.html_url },
        notes: r.body.unwrap_or_default().chars().take(1500).collect(),
        size: setup.map(|a| a.size).unwrap_or(0),
        setup_url: setup.map(|a| (a.name.clone(), a.browser_download_url.clone())),
        sums_url: r.assets.iter().find(|a| a.name == "SHA256SUMS.txt").map(|a| a.browser_download_url.clone()),
    })
}

fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::Digest;
    sha2::Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect()
}

/// Scarica e avvia l'installer della nuova versione (chiede sempre conferma).
pub async fn install(app: &AppHandle) -> Result<String, String> {
    status(app, crate::i18n::tr("Controllo gli aggiornamenti…", "Checking for updates…"));
    let info = check(app).await?;
    if !info.newer {
        status(app, "");
        return Ok(crate::tr!("Alter Ego è già aggiornata (versione {}).", "Alter Ego is up to date (version {}).", info.current));
    }
    #[cfg(not(windows))]
    let appimage = std::env::var_os("APPIMAGE").map(std::path::PathBuf::from).filter(|p| p.is_file());
    #[cfg(not(windows))]
    if appimage.is_none() {
        // pacchetto .deb o build a mano: si aggiorna dal sito delle release
        status(app, "");
        let _ = open_link("releases");
        return Ok(crate::tr!(
            "C'è la versione {}: ho aperto la pagina della release, scarica il pacchetto .deb o l'AppImage.",
            "Version {} is out: I opened the release page, download the .deb package or the AppImage.",
            info.latest
        ));
    }
    let Some((name, url)) = info.setup_url.clone() else {
        return Err(crate::tr!("la release {} non ha l'installer: scaricalo da {}", "release {} has no installer: download it from {}", info.latest, info.page));
    };
    let q = crate::tr!(
        "Aggiornare Alter Ego da {} a {}?\nScarico l'installer da GitHub ({:.0} MB), lo avvio e chiudo Alter Ego.",
        "Update Alter Ego from {} to {}?\nI'll download the installer from GitHub ({:.0} MB), run it and close Alter Ego.",
        info.current,
        info.latest,
        info.size as f64 / 1_048_576.0
    );
    if !crate::actions::confirm_always(app, q).await {
        status(app, "");
        return Err("l'utente ha negato il permesso".into());
    }
    let http = app.state::<crate::AppState>().ai.http.clone();
    status(app, &crate::tr!("Scarico la versione {}…", "Downloading version {}…", info.latest));
    let bytes = http
        .get(&url)
        .header("User-Agent", "AlterEgo-updater")
        .send()
        .await
        .and_then(|r| r.error_for_status())
        .map_err(|e| crate::tr!("download fallito: {e}", "download failed: {e}"))?
        .bytes()
        .await
        .map_err(|e| crate::tr!("download interrotto: {e}", "download interrupted: {e}"))?;
    // controllo dell'integrità, se la release ha l'elenco degli hash
    if let Some(sums) = &info.sums_url {
        let text = http.get(sums).header("User-Agent", "AlterEgo-updater").send().await.map_err(|e| e.to_string())?.text().await.map_err(|e| e.to_string())?;
        let want = text.lines().find(|l| l.trim_end().ends_with(&name)).and_then(|l| l.split_whitespace().next()).map(str::to_lowercase);
        match want {
            Some(w) if w == sha256_hex(&bytes) => {}
            Some(_) => return Err(crate::i18n::tr("il file scaricato non corrisponde allo SHA-256 della release: aggiornamento annullato", "the downloaded file does not match the release SHA-256: update cancelled").into()),
            None => {}
        }
    }
    #[cfg(windows)]
    let path = {
        let path = std::env::temp_dir().join(&name);
        std::fs::write(&path, &bytes).map_err(|e| e.to_string())?;
        path
    };
    // Linux: la nuova AppImage prende il posto di quella in uso
    #[cfg(not(windows))]
    let path = {
        use std::os::unix::fs::PermissionsExt;
        let target = appimage.clone().unwrap_or_default();
        let tmp = target.with_extension("new");
        std::fs::write(&tmp, &bytes).map_err(|e| e.to_string())?;
        std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o755)).map_err(|e| e.to_string())?;
        std::fs::rename(&tmp, &target).map_err(|e| e.to_string())?;
        target
    };
    crate::actions::log(app, "aggiornamento", &format!("{} → {}", info.current, info.latest), "installer avviato");
    status(app, crate::i18n::tr("Avvio l'installer…", "Starting the installer…"));
    #[cfg(windows)]
    std::process::Command::new(&path).spawn().map_err(|e| crate::tr!("installer non avviato: {e}", "installer did not start: {e}"))?;
    // la nuova versione parte quando questa è chiusa (istanza singola)
    #[cfg(not(windows))]
    std::process::Command::new("sh")
        .arg("-c")
        .arg("sleep 4; exec \"$0\"")
        .arg(&path)
        .spawn()
        .map_err(|e| crate::tr!("nuova versione non avviata: {e}", "new version did not start: {e}"))?;
    let app2 = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(1500));
        crate::window::hide_main(&app2, true);
    });
    Ok(crate::tr!("Installo la versione {}: a tra poco!", "Installing version {}: see you soon!", info.latest))
}

/// Apre un link dell'app nel browser: "site", "github", "releases".
pub fn open_link(which: &str) -> Result<(), String> {
    let url = match which {
        "site" => SITE.to_string(),
        "github" => github_url(),
        "releases" => releases_url(),
        "spotify_dev" => "https://developer.spotify.com/dashboard".to_string(),
        "rizzo" => "https://github.com/Rizzo-AI-Academy/rizzo-flow".to_string(),
        "graphify" => "https://github.com/Graphify-Labs/graphify".to_string(),
        "lfm3" => "https://www.liquid.ai/blog/lfm3".to_string(),
        _ => return Err("link sconosciuto".into()),
    };
    #[cfg(windows)]
    let r = std::process::Command::new("explorer").arg(&url).spawn();
    #[cfg(not(windows))]
    let r = std::process::Command::new("xdg-open").arg(&url).spawn();
    r.map(|_| ()).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn update_check(app: AppHandle) -> Result<UpdateInfo, String> {
    check(&app).await
}

#[tauri::command]
pub async fn update_install(app: AppHandle) -> Result<String, String> {
    install(&app).await
}

#[tauri::command]
pub fn open_link_cmd(which: String) -> Result<(), String> {
    open_link(&which)
}

#[tauri::command]
pub fn app_version() -> &'static str {
    VERSION
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions() {
        assert!(is_newer("v1.3.0", "1.2.0"));
        assert!(is_newer("1.10.0", "1.9.9"));
        assert!(!is_newer("v1.2.0", "1.2.0"));
        assert!(!is_newer("1.1.9", "1.2.0"));
    }
}
