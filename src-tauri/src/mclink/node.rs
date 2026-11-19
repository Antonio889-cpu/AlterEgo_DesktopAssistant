//! Node.js e le dipendenze del bot (Mineflayer), installati con un clic come Rizzo Flow:
//! 1. Node di sistema se c'è (≥ 18), altrimenti Node LTS portatile da nodejs.org nei dati locali dell'app,
//!    controllato con lo SHA-256 di SHASUMS256.txt;
//! 2. i file del bot (inclusi nell'app) in `<dati locali>/minecraft/bot`;
//! 3. `npm ci` con il package-lock.json incluso (versioni esatte).

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::atomic::{AtomicBool, Ordering},
};
use tauri::{AppHandle, Emitter, Manager};

/// File del bot scritti nella cartella dei dati (come l'addon di gmodlink).
pub const BOT: &[(&str, &str)] = &[
    ("package.json", include_str!("../../mc_bot/package.json")),
    ("package-lock.json", include_str!("../../mc_bot/package-lock.json")),
    ("bot.js", include_str!("../../mc_bot/bot.js")),
    ("lib.js", include_str!("../../mc_bot/lib.js")),
];

/// Ramo LTS di Node scaricato se manca (Jod, supporto fino ad aprile 2027).
const NODE_LINE: &str = "latest-v22.x";
const MIN_MAJOR: u32 = 18;

static INSTALLING: AtomicBool = AtomicBool::new(false);

pub fn installing() -> bool {
    INSTALLING.load(Ordering::SeqCst)
}

pub fn root(app: &AppHandle) -> PathBuf {
    app.path().app_local_data_dir().unwrap_or_else(|_| std::env::temp_dir()).join("minecraft")
}

pub fn bot_dir(app: &AppHandle) -> PathBuf {
    root(app).join("bot")
}

fn managed_dir(app: &AppHandle) -> PathBuf {
    root(app).join("node")
}

pub(crate) fn hidden(cmd: &mut Command) -> &mut Command {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    cmd
}

/// "v22.3.0" → (22, 3, 0).
pub fn parse_version(s: &str) -> Option<(u32, u32, u32)> {
    let t = s.trim().trim_start_matches('v');
    let mut it = t.split('.').map(|x| x.trim().parse::<u32>().ok());
    Some((it.next()??, it.next().flatten().unwrap_or(0), it.next().flatten().unwrap_or(0)))
}

/// Versione di un eseguibile `node` (None se non parte).
fn node_version(exe: &Path) -> Option<String> {
    let out = hidden(Command::new(exe).arg("--version")).stdin(Stdio::null()).stderr(Stdio::null()).output().ok()?;
    let v = String::from_utf8_lossy(&out.stdout).trim().to_string();
    (out.status.success() && parse_version(&v).is_some()).then_some(v)
}

fn exe(name: &str) -> String {
    if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_string()
    }
}

/// Node portatile scaricato dall'app (dentro `minecraft/node`).
fn managed_node(app: &AppHandle) -> PathBuf {
    let d = managed_dir(app);
    if cfg!(windows) {
        d.join("node.exe")
    } else {
        d.join("bin").join("node")
    }
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct NodeInfo {
    pub path: String,
    pub version: String,
    /// Scaricato dall'app (non quello di sistema).
    pub managed: bool,
}

/// Node da usare: cartella scelta, Node di sistema (≥ 18), oppure quello scaricato dall'app.
pub fn find(app: &AppHandle, custom: &str) -> Option<NodeInfo> {
    let ok = |v: &str| parse_version(v).is_some_and(|(m, _, _)| m >= MIN_MAJOR);
    let c = custom.trim();
    if !c.is_empty() {
        let p = PathBuf::from(c);
        let p = if p.is_dir() { p.join(exe("node")) } else { p };
        if let Some(v) = node_version(&p).filter(|v| ok(v)) {
            return Some(NodeInfo { path: p.display().to_string(), version: v, managed: false });
        }
    }
    if let Some(v) = node_version(Path::new(&exe("node"))).filter(|v| ok(v)) {
        return Some(NodeInfo { path: exe("node"), version: v, managed: false });
    }
    let m = managed_node(app);
    node_version(&m).filter(|v| ok(v)).map(|v| NodeInfo { path: m.display().to_string(), version: v, managed: true })
}

/// Le dipendenze del bot sono installate e aggiornate (segno con l'impronta del package-lock).
pub fn deps_ready(app: &AppHandle) -> bool {
    let d = bot_dir(app);
    d.join("node_modules").join("mineflayer").join("package.json").is_file() && std::fs::read_to_string(d.join(".alterego-deps")).ok().as_deref() == Some(&lock_hash())
}

fn lock_hash() -> String {
    hex(&Sha256::digest(BOT[1].1.as_bytes()))
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

/// Scrive (o aggiorna) i file del bot. true se ha cambiato qualcosa.
pub fn write_bot(dir: &Path) -> Result<bool, String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let mut changed = false;
    for (rel, content) in BOT {
        let p = dir.join(rel);
        if std::fs::read_to_string(&p).ok().as_deref() == Some(*content) {
            continue;
        }
        std::fs::write(&p, content).map_err(|e| format!("{}: {e}", p.display()))?;
        changed = true;
    }
    Ok(changed)
}

/// Archivio di Node per questo sistema ("win-x64.zip", "linux-x64.tar.xz"…).
pub fn platform_suffix(os: &str, arch: &str) -> Option<&'static str> {
    Some(match (os, arch) {
        ("windows", "x86_64") => "win-x64.zip",
        ("windows", "aarch64") => "win-arm64.zip",
        ("linux", "x86_64") => "linux-x64.tar.xz",
        ("linux", "aarch64") => "linux-arm64.tar.xz",
        ("macos", "aarch64") => "darwin-arm64.tar.gz",
        ("macos", "x86_64") => "darwin-x64.tar.gz",
        _ => return None,
    })
}

/// Da SHASUMS256.txt: (nome del file, sha256) dell'archivio per `suffix`.
pub fn pick_dist(shasums: &str, suffix: &str) -> Option<(String, String)> {
    shasums.lines().find_map(|l| {
        let mut it = l.split_whitespace();
        let (hash, file) = (it.next()?, it.next()?.trim_start_matches('*'));
        let ok = file.starts_with("node-v") && file.ends_with(&format!("-{suffix}")) && hash.len() == 64 && hash.chars().all(|c| c.is_ascii_hexdigit());
        ok.then(|| (file.to_string(), hash.to_lowercase()))
    })
}

fn say(app: &AppHandle, phase: &str, text: impl Into<String>) {
    #[derive(Serialize, Clone)]
    struct Ev {
        phase: String,
        text: String,
    }
    let _ = app.emit("minecraft-install", Ev { phase: phase.into(), text: text.into() });
}

/// Scarica Node portatile e controlla lo SHA-256.
async fn download_node(app: &AppHandle, log: &Path) -> Result<NodeInfo, String> {
    use futures_util::StreamExt;
    let suffix = platform_suffix(std::env::consts::OS, std::env::consts::ARCH).ok_or_else(|| crate::i18n::tr("sistema non supportato: installa Node.js a mano", "unsupported system: install Node.js yourself").to_string())?;
    let http = app.state::<crate::AppState>().ai.http.clone();
    let base = format!("https://nodejs.org/dist/{NODE_LINE}/");
    say(app, "install", crate::i18n::tr("Cerco l'ultima versione di Node.js LTS…", "Looking for the latest Node.js LTS…"));
    let sums = http.get(format!("{base}SHASUMS256.txt")).send().await.and_then(|r| r.error_for_status()).map_err(|e| e.to_string())?.text().await.map_err(|e| e.to_string())?;
    let (file, want) = pick_dist(&sums, suffix).ok_or("SHASUMS256.txt: archivio non trovato")?;
    let root = root(app);
    std::fs::create_dir_all(&root).map_err(|e| e.to_string())?;
    let tmp = root.join("node-download");
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).map_err(|e| e.to_string())?;
    let arch = tmp.join(&file);
    let resp = http.get(format!("{base}{file}")).send().await.and_then(|r| r.error_for_status()).map_err(|e| e.to_string())?;
    let total = resp.content_length().unwrap_or(0);
    let mut stream = resp.bytes_stream();
    let mut f = std::fs::File::create(&arch).map_err(|e| e.to_string())?;
    let mut hasher = Sha256::new();
    let (mut got, mut last_pct) = (0u64, 101u64);
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| e.to_string())?;
        use std::io::Write;
        f.write_all(&chunk).map_err(|e| e.to_string())?;
        hasher.update(&chunk);
        got += chunk.len() as u64;
        let pct = if total > 0 { got * 100 / total } else { 0 };
        if pct != last_pct && pct % 5 == 0 {
            last_pct = pct;
            say(app, "install", crate::tr!("Scarico {} — {}%", "Downloading {} — {}%", file, pct));
        }
    }
    drop(f);
    let have = hex(&hasher.finalize());
    if have != want {
        let _ = std::fs::remove_dir_all(&tmp);
        return Err(crate::tr!("SHA-256 di {} non corrisponde: download scartato", "SHA-256 of {} doesn't match: download discarded", file));
    }
    let _ = std::fs::OpenOptions::new().create(true).append(true).open(log).and_then(|mut l| {
        use std::io::Write;
        writeln!(l, "{file} sha256 {have} ok")
    });
    say(app, "install", crate::tr!("Estraggo {}…", "Extracting {}…", file));
    let (a, t) = (arch.clone(), tmp.clone());
    tauri::async_runtime::spawn_blocking(move || extract(&a, &t)).await.map_err(|e| e.to_string())??;
    let inner = std::fs::read_dir(&tmp).map_err(|e| e.to_string())?.flatten().map(|e| e.path()).find(|p| p.is_dir() && p.file_name().is_some_and(|n| n.to_string_lossy().starts_with("node-v"))).ok_or("archivio di Node vuoto")?;
    let dest = managed_dir(app);
    let _ = std::fs::remove_dir_all(&dest);
    std::fs::rename(&inner, &dest).map_err(|e| e.to_string())?;
    let _ = std::fs::remove_dir_all(&tmp);
    let p = managed_node(app);
    let v = node_version(&p).ok_or_else(|| crate::i18n::tr("Node.js scaricato ma non parte", "Node.js downloaded but it doesn't start").to_string())?;
    Ok(NodeInfo { path: p.display().to_string(), version: v, managed: true })
}

/// Estrae lo zip (Windows) con la libreria zip, i tar con `tar`.
fn extract(archive: &Path, to: &Path) -> Result<(), String> {
    let name = archive.to_string_lossy().to_string();
    if name.ends_with(".zip") {
        let f = std::fs::File::open(archive).map_err(|e| e.to_string())?;
        let mut z = zip::ZipArchive::new(f).map_err(|e| e.to_string())?;
        z.extract(to).map_err(|e| e.to_string())
    } else {
        let st = hidden(Command::new("tar").arg("-xf").arg(archive).arg("-C").arg(to)).stdin(Stdio::null()).status().map_err(|e| e.to_string())?;
        if st.success() {
            Ok(())
        } else {
            Err(format!("tar: {st}"))
        }
    }
}

/// `npm-cli.js` accanto a un Node (portatile o di sistema).
pub fn npm_cli(node: &Path) -> Option<PathBuf> {
    let dir = node.parent()?;
    let c = [dir.join("node_modules/npm/bin/npm-cli.js"), dir.join("../lib/node_modules/npm/bin/npm-cli.js"), dir.join("../share/nodejs/npm/bin/npm-cli.js"), dir.join("../lib/nodejs/npm/bin/npm-cli.js")];
    c.into_iter().find(|p| p.is_file())
}

/// Percorso vero di `node` quando è "node" del PATH.
fn resolve(node: &str) -> PathBuf {
    let p = PathBuf::from(node);
    if p.components().count() > 1 {
        return p;
    }
    let cmd = if cfg!(windows) { "where" } else { "which" };
    hidden(Command::new(cmd).arg(node))
        .stderr(Stdio::null())
        .output()
        .ok()
        .and_then(|o| String::from_utf8_lossy(&o.stdout).lines().next().map(|l| PathBuf::from(l.trim())))
        .filter(|p| p.is_file())
        .map(|p| std::fs::canonicalize(&p).unwrap_or(p))
        .unwrap_or(p)
}

/// `npm ci` nella cartella del bot, con le righe d'uscita come avanzamento.
fn npm_ci(app: &AppHandle, node: &str, dir: &Path, log: &Path) -> Result<(), String> {
    let node_path = resolve(node);
    let args = ["ci", "--omit=dev", "--no-audit", "--no-fund", "--loglevel=http"];
    let mut cmd = match npm_cli(&node_path) {
        Some(cli) => {
            let mut c = Command::new(&node_path);
            c.arg(cli).args(args);
            c
        }
        None if cfg!(windows) => {
            let mut c = Command::new("cmd");
            c.args(["/C", "npm"]).args(args);
            c
        }
        None => {
            let mut c = Command::new("npm");
            c.args(args);
            c
        }
    };
    // npm deve trovare questo node anche negli script
    if let Some(d) = node_path.parent() {
        let path = std::env::var_os("PATH").unwrap_or_default();
        let mut paths = vec![d.to_path_buf()];
        paths.extend(std::env::split_paths(&path));
        if let Ok(p) = std::env::join_paths(paths) {
            cmd.env("PATH", p);
        }
    }
    let mut child = hidden(cmd.current_dir(dir)).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().map_err(|e| format!("npm: {e}"))?;
    let label = crate::i18n::tr("Installo Mineflayer (npm ci)", "Installing Mineflayer (npm ci)");
    say(app, "install", label);
    let mut readers = Vec::new();
    let last = std::sync::Arc::new(std::sync::Mutex::new(String::new()));
    for s in [child.stdout.take().map(|s| Box::new(s) as Box<dyn std::io::Read + Send>), child.stderr.take().map(|s| Box::new(s) as Box<dyn std::io::Read + Send>)].into_iter().flatten() {
        let (app, log, last) = (app.clone(), log.to_path_buf(), last.clone());
        readers.push(std::thread::spawn(move || {
            let mut file = std::fs::OpenOptions::new().create(true).append(true).open(&log).ok();
            let mut n = 0u32;
            for line in BufReader::new(s).lines().map_while(Result::ok) {
                if let Some(f) = file.as_mut() {
                    use std::io::Write;
                    let _ = writeln!(f, "{line}");
                }
                let t = line.trim().to_string();
                if t.is_empty() {
                    continue;
                }
                *last.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = t.clone();
                n += 1;
                if n % 3 == 0 {
                    say(&app, "install", format!("{label} — {}", t.chars().take(140).collect::<String>()));
                }
            }
        }));
    }
    let st = child.wait().map_err(|e| e.to_string())?;
    for r in readers {
        let _ = r.join();
    }
    if st.success() {
        Ok(())
    } else {
        Err(crate::tr!("npm ci non riuscito ({}): {}", "npm ci failed ({}): {}", st, last.lock().unwrap_or_else(std::sync::PoisonError::into_inner)))
    }
}

/// Installa (o aggiorna) Node se manca e le dipendenze del bot. Restituisce il Node usato.
pub async fn install(app: &AppHandle, custom: &str) -> Result<NodeInfo, String> {
    if INSTALLING.swap(true, Ordering::SeqCst) {
        return Err(crate::i18n::tr("installazione già in corso", "installation already running").into());
    }
    let r = install_inner(app, custom).await;
    INSTALLING.store(false, Ordering::SeqCst);
    match &r {
        Ok(n) => {
            crate::actions::log(app, "minecraft", &format!("bot installato (node {})", n.version), "ok");
            say(app, "done", crate::tr!("Pronto: Node.js {} e Mineflayer installati.", "Ready: Node.js {} and Mineflayer installed.", n.version));
        }
        Err(e) => {
            crate::actions::log(app, "minecraft", "installazione", e);
            say(app, "error", e.clone());
        }
    }
    r
}

async fn install_inner(app: &AppHandle, custom: &str) -> Result<NodeInfo, String> {
    let (a, c) = (app.clone(), custom.to_string());
    let found = tauri::async_runtime::spawn_blocking(move || find(&a, &c)).await.ok().flatten();
    let q = match &found {
        Some(n) => crate::tr!(
            "Preparare Alter Ego per Minecraft?\nUso Node.js {} già installato e scarico da npm Mineflayer e i suoi plugin (circa 650 MB su disco, licenza MIT) in {}.",
            "Set up Alter Ego for Minecraft?\nI'll use the installed Node.js {} and download Mineflayer and its plugins from npm (about 650 MB on disk, MIT license) into {}.",
            n.version,
            root(app).display()
        ),
        None => crate::tr!(
            "Preparare Alter Ego per Minecraft?\nScarico Node.js LTS portatile da nodejs.org (circa 30 MB, controllato con SHA-256) e da npm Mineflayer e i suoi plugin (circa 650 MB su disco, licenza MIT) in {}.",
            "Set up Alter Ego for Minecraft?\nI'll download portable Node.js LTS from nodejs.org (about 30 MB, SHA-256 checked) and Mineflayer and its plugins from npm (about 650 MB on disk, MIT license) into {}.",
            root(app).display()
        ),
    };
    if !crate::actions::confirm_always(app, q).await {
        return Err("the user denied permission".into());
    }
    let root = root(app);
    std::fs::create_dir_all(&root).map_err(|e| e.to_string())?;
    let log = root.join("install.log");
    let _ = std::fs::write(&log, "");
    let node = match found {
        Some(n) => n,
        None => download_node(app, &log).await?,
    };
    let dir = bot_dir(app);
    write_bot(&dir)?;
    let (a, n, d, l) = (app.clone(), node.path.clone(), dir.clone(), log.clone());
    tauri::async_runtime::spawn_blocking(move || npm_ci(&a, &n, &d, &l)).await.map_err(|e| e.to_string())??;
    std::fs::write(dir.join(".alterego-deps"), lock_hash()).map_err(|e| e.to_string())?;
    Ok(node)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions() {
        assert_eq!(parse_version("v22.23.5\n"), Some((22, 23, 5)));
        assert_eq!(parse_version("18"), Some((18, 0, 0)));
        assert_eq!(parse_version("node"), None);
        assert_eq!(parse_version(""), None);
    }

    #[test]
    fn picks_archive_from_shasums() {
        let sums = "\
f0dda0bae79678ab6eb3ca02179f4fe534463ce6f68ab942d2b49b057e478433  node-v22.23.5-aix-ppc64.tar.gz
e7131995759b61a0170b646010d2ad35a5d37b4cb0d471f3e8317eac6f742f3d  node-v22.23.5-darwin-arm64.tar.gz
d0c68c321161bb2c6483efea9a343010dea8e5e7f60d73c6a7eba140016d662f  node-v22.23.5-darwin-arm64.tar.xz
AAAA  node-v22.23.5-linux-x64.tar.gz
1111111111111111111111111111111111111111111111111111111111111111  node-v22.23.5-linux-x64.tar.xz
2222222222222222222222222222222222222222222222222222222222222222 *node-v22.23.5-win-x64.zip
3333333333333333333333333333333333333333333333333333333333333333  node-v22.23.5-win-x64.7z
";
        assert_eq!(pick_dist(sums, "win-x64.zip"), Some(("node-v22.23.5-win-x64.zip".into(), "2".repeat(64))));
        assert_eq!(pick_dist(sums, "linux-x64.tar.xz"), Some(("node-v22.23.5-linux-x64.tar.xz".into(), "1".repeat(64))));
        assert_eq!(pick_dist(sums, "darwin-arm64.tar.gz").map(|x| x.1), Some("e7131995759b61a0170b646010d2ad35a5d37b4cb0d471f3e8317eac6f742f3d".into()));
        assert_eq!(pick_dist(sums, "win-arm64.zip"), None);
        assert_eq!(pick_dist("", "win-x64.zip"), None);
        assert_eq!(platform_suffix("windows", "x86_64"), Some("win-x64.zip"));
        assert_eq!(platform_suffix("linux", "x86_64"), Some("linux-x64.tar.xz"));
        assert_eq!(platform_suffix("freebsd", "x86_64"), None);
    }

    #[test]
    fn writes_bot_files() {
        let dir = std::env::temp_dir().join(format!("ae-mc-bot-{}", std::process::id()));
        assert!(write_bot(&dir).unwrap());
        assert!(!write_bot(&dir).unwrap(), "già aggiornato");
        assert!(dir.join("bot.js").is_file() && dir.join("package-lock.json").is_file());
        // versioni esatte nel package.json
        let pkg: serde_json::Value = serde_json::from_str(BOT[0].1).unwrap();
        for (k, v) in pkg["dependencies"].as_object().unwrap() {
            let v = v.as_str().unwrap();
            assert!(v.chars().next().unwrap().is_ascii_digit(), "{k} {v}: versione esatta");
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
}
