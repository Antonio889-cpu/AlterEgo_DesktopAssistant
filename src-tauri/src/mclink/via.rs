//! Ogni versione di Minecraft: ponte ViaProxy.
//!
//! Mineflayer conosce le versioni dalla 1.8.8 alla 26.1. Prima di entrare l'app chiede al server la sua
//! versione con un ping di stato (`node bot.js --ping`, minecraft-protocol); se Mineflayer non la conosce,
//! avvia in locale ViaProxy (https://github.com/ViaVersion/ViaProxy: ViaVersion, ViaBackwards, ViaRewind,
//! ViaLegacy; GPL-3.0, scaricato a parte, non incluso nell'app) su una porta libera di 127.0.0.1 verso il
//! server, e il bot entra nel ponte con l'ultima versione che conosce. ViaProxy traduce i pacchetti.
//!
//! Serve Java ≥ 17: quello di sistema se basta, altrimenti un JRE portatile Eclipse Temurin (Adoptium)
//! nei dati locali, controllato con lo SHA-256 dell'API di Adoptium e del file .sha256.txt della release.
//! Il JAR di ViaProxy ha la versione e lo SHA-256 fissati qui (verificati a mano sul repository Maven di
//! ViaVersion e sulla release di GitHub, che sono due build diverse con due impronte diverse).

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    io::{BufRead, BufReader, Write},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        Mutex,
    },
    time::{Duration, Instant},
};
use tauri::{AppHandle, Emitter, Manager};

pub const VIAPROXY_VERSION: &str = "3.4.14";
/// Da dove scaricare il JAR, con lo SHA-256 di quel file (le due build non sono identiche byte per byte).
pub const SOURCES: &[(&str, &str)] = &[
    ("https://repo.viaversion.com/net/raphimc/ViaProxy/3.4.14/ViaProxy-3.4.14.jar", "727a93545672adf2d4fe00023ec02906b34da837b8a38b15c0bb2c8b75fbdb77"),
    ("https://github.com/ViaVersion/ViaProxy/releases/download/v3.4.14/ViaProxy-3.4.14.jar", "2894bbfb2f4342f8fde887af6c689313efd5b0e46a8006775eea0e21e2948ab5"),
];
/// ViaProxy 3.4 è compilato per Java 17 (classi versione 61).
pub const JAVA_MIN: u32 = 17;
/// Java portatile scaricato se manca (LTS).
const JRE_FEATURE: u32 = 21;
/// ViaProxy senza versione: riconosce da solo i server dalla 1.7 in su.
const TARGET_AUTO: &str = "";

static INSTALLING: AtomicBool = AtomicBool::new(false);

pub fn installing() -> bool {
    INSTALLING.load(Ordering::SeqCst)
}

pub fn dir(app: &AppHandle) -> PathBuf {
    super::node::root(app).join("viaproxy")
}

fn java_dir(app: &AppHandle) -> PathBuf {
    super::node::root(app).join("java")
}

fn jar(app: &AppHandle) -> PathBuf {
    dir(app).join(format!("ViaProxy-{VIAPROXY_VERSION}.jar"))
}

fn marker(app: &AppHandle) -> PathBuf {
    dir(app).join(format!("ViaProxy-{VIAPROXY_VERSION}.ok"))
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

/// Il JAR c'è ed è stato controllato (l'impronta è scritta accanto dopo il download).
pub fn jar_ok(app: &AppHandle) -> bool {
    jar(app).is_file() && std::fs::read_to_string(marker(app)).is_ok_and(|h| SOURCES.iter().any(|(_, s)| *s == h.trim()))
}

// ---------- Java ----------

/// Versione principale da `java -version`: "21.0.13" → 21, "1.8.0_202" → 8, "25-ea" → 25.
pub fn parse_java_major(out: &str) -> Option<u32> {
    let line = out.lines().find(|l| l.contains("version"))?;
    let q = line.split('"').nth(1)?;
    let mut it = q.split(['.', '-', '_', '+']);
    let first: u32 = it.next()?.trim().parse().ok()?;
    if first == 1 {
        return it.next()?.parse().ok();
    }
    Some(first)
}

fn java_major(exe: &Path) -> Option<u32> {
    let out = super::node::hidden(Command::new(exe).arg("-version")).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::piped()).output().ok()?;
    if !out.status.success() {
        return None;
    }
    parse_java_major(&String::from_utf8_lossy(&out.stderr))
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct JavaInfo {
    pub path: String,
    pub major: u32,
    /// Scaricato dall'app.
    pub managed: bool,
}

fn exe(name: &str) -> String {
    if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_string()
    }
}

/// `bin/java` dentro una cartella estratta (anche Contents/Home/bin/java su macOS).
fn find_bin(root: &Path, depth: u32) -> Option<PathBuf> {
    let p = root.join("bin").join(exe("java"));
    if p.is_file() {
        return Some(p);
    }
    if depth == 0 {
        return None;
    }
    let mut dirs: Vec<PathBuf> = std::fs::read_dir(root).ok()?.flatten().map(|e| e.path()).filter(|p| p.is_dir()).collect();
    dirs.sort();
    dirs.iter().find_map(|d| find_bin(d, depth - 1))
}

/// Java da usare: JAVA_HOME, quello del PATH, oppure quello scaricato dall'app (se ≥ 17).
pub fn find_java(app: &AppHandle) -> Option<JavaInfo> {
    let mut cand: Vec<(PathBuf, bool)> = Vec::new();
    if let Some(h) = std::env::var_os("JAVA_HOME") {
        cand.push((PathBuf::from(h).join("bin").join(exe("java")), false));
    }
    cand.push((PathBuf::from(exe("java")), false));
    if let Some(m) = find_bin(&java_dir(app), 3) {
        cand.push((m, true));
    }
    cand.into_iter().find_map(|(p, managed)| java_major(&p).filter(|m| *m >= JAVA_MIN).map(|major| JavaInfo { path: p.display().to_string(), major, managed }))
}

/// Nomi di Adoptium per questo sistema.
pub fn adoptium_platform(os: &str, arch: &str) -> Option<(&'static str, &'static str)> {
    let o = match os {
        "windows" => "windows",
        "linux" => "linux",
        "macos" => "mac",
        _ => return None,
    };
    let a = match arch {
        "x86_64" => "x64",
        "aarch64" => "aarch64",
        _ => return None,
    };
    Some((o, a))
}

pub fn adoptium_url(feature: u32, os: &str, arch: &str) -> String {
    format!("https://api.adoptium.net/v3/assets/latest/{feature}/hotspot?os={os}&architecture={arch}&image_type=jre&vendor=eclipse")
}

#[derive(Debug, Clone, PartialEq)]
pub struct JrePackage {
    pub name: String,
    pub link: String,
    pub sha256: String,
    pub checksum_link: String,
    pub size: u64,
    pub version: String,
}

fn is_sha256(s: &str) -> bool {
    s.len() == 64 && s.chars().all(|c| c.is_ascii_hexdigit())
}

/// Dalla risposta dell'API di Adoptium: il pacchetto JRE (zip o tar.gz) con il suo SHA-256.
pub fn parse_adoptium(v: &Value) -> Option<JrePackage> {
    v.as_array()?.iter().find_map(|a| {
        let b = &a["binary"];
        if b["image_type"] != "jre" {
            return None;
        }
        let p = &b["package"];
        let name = p["name"].as_str()?;
        let link = p["link"].as_str()?;
        let sum = p["checksum"].as_str()?;
        let ok = (name.ends_with(".zip") || name.ends_with(".tar.gz")) && link.starts_with("https://") && is_sha256(sum);
        ok.then(|| JrePackage {
            name: name.to_string(),
            link: link.to_string(),
            sha256: sum.to_lowercase(),
            checksum_link: p["checksum_link"].as_str().unwrap_or("").to_string(),
            size: p["size"].as_u64().unwrap_or(0),
            version: a["version"]["openjdk_version"].as_str().or(a["release_name"].as_str()).unwrap_or("").to_string(),
        })
    })
}

/// Da un file .sha256 ("hash  nome" oppure solo "hash"): l'impronta per `name`.
pub fn parse_sha256_file(text: &str, name: &str) -> Option<String> {
    text.lines().find_map(|l| {
        let mut it = l.split_whitespace();
        let h = it.next()?;
        let f = it.next().map(|f| f.trim_start_matches('*'));
        (is_sha256(h) && f.is_none_or(|f| f == name || f.ends_with(&format!("/{name}")))).then(|| h.to_lowercase())
    })
}

// ---------- download ----------

fn say(app: &AppHandle, phase: &str, text: impl Into<String>) {
    let text = text.into();
    let _ = app.emit("minecraft-via", json!({ "phase": phase, "text": text }));
}

/// Scarica `url` in `dest` controllando lo SHA-256 mentre arriva. Il file resta solo se torna.
async fn download(app: &AppHandle, url: &str, dest: &Path, want: &str, label: &str) -> Result<(), String> {
    use futures_util::StreamExt;
    let http = app.state::<crate::AppState>().ai.http.clone();
    let resp = http.get(url).send().await.and_then(|r| r.error_for_status()).map_err(|e| format!("{label}: {e}"))?;
    let total = resp.content_length().unwrap_or(0);
    let part = dest.with_extension("part");
    let mut f = std::fs::File::create(&part).map_err(|e| e.to_string())?;
    let mut hasher = Sha256::new();
    let (mut got, mut last) = (0u64, 101u64);
    let mut stream = resp.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| format!("{label}: {e}"))?;
        f.write_all(&chunk).map_err(|e| e.to_string())?;
        hasher.update(&chunk);
        got += chunk.len() as u64;
        let pct = (got * 100).checked_div(total).unwrap_or(0);
        if pct != last && pct % 5 == 0 {
            last = pct;
            say(app, "install", crate::tr!("Scarico {} — {}%", "Downloading {} — {}%", label, pct));
        }
    }
    drop(f);
    let have = hex(&hasher.finalize());
    if have != want.to_lowercase() {
        let _ = std::fs::remove_file(&part);
        return Err(crate::tr!("SHA-256 di {} non corrisponde: download scartato", "SHA-256 of {} doesn't match: download discarded", label));
    }
    std::fs::rename(&part, dest).map_err(|e| e.to_string())?;
    super::console::log(app, "via", &format!("{label}: sha256 {have} ok"));
    Ok(())
}

fn extract(archive: &Path, to: &Path) -> Result<(), String> {
    if archive.to_string_lossy().ends_with(".zip") {
        let f = std::fs::File::open(archive).map_err(|e| e.to_string())?;
        zip::ZipArchive::new(f).map_err(|e| e.to_string())?.extract(to).map_err(|e| e.to_string())
    } else {
        let st = super::node::hidden(Command::new("tar").arg("-xzf").arg(archive).arg("-C").arg(to)).stdin(Stdio::null()).status().map_err(|e| e.to_string())?;
        if st.success() {
            Ok(())
        } else {
            Err(format!("tar: {st}"))
        }
    }
}

/// JRE Temurin portatile da Adoptium.
async fn download_java(app: &AppHandle) -> Result<JavaInfo, String> {
    let (os, arch) = adoptium_platform(std::env::consts::OS, std::env::consts::ARCH).ok_or_else(|| crate::i18n::tr("sistema non supportato: installa Java 17 o più recente", "unsupported system: install Java 17 or newer").to_string())?;
    let http = app.state::<crate::AppState>().ai.http.clone();
    say(app, "install", crate::i18n::tr("Cerco Java (Eclipse Temurin) su adoptium.net…", "Looking for Java (Eclipse Temurin) on adoptium.net…"));
    let v: Value = http.get(adoptium_url(JRE_FEATURE, os, arch)).send().await.and_then(|r| r.error_for_status()).map_err(|e| e.to_string())?.json().await.map_err(|e| e.to_string())?;
    let pkg = parse_adoptium(&v).ok_or("Adoptium: pacchetto JRE non trovato")?;
    // seconda fonte per l'impronta: il file .sha256.txt accanto al pacchetto
    if !pkg.checksum_link.is_empty() {
        match http.get(&pkg.checksum_link).send().await.and_then(|r| r.error_for_status()) {
            Ok(r) => {
                let t = r.text().await.unwrap_or_default();
                if let Some(h) = parse_sha256_file(&t, &pkg.name) {
                    if h != pkg.sha256 {
                        return Err(crate::tr!("SHA-256 di {} diverso tra API e release: download annullato", "SHA-256 of {} differs between API and release: download cancelled", pkg.name));
                    }
                }
            }
            Err(e) => super::console::log(app, "via", &crate::tr!("{}: .sha256.txt non letto ({}), uso l'impronta dell'API", "{}: .sha256.txt not read ({}), using the API checksum", pkg.name, e)),
        }
    }
    let root = java_dir(app);
    let tmp = super::node::root(app).join("java-download");
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).map_err(|e| e.to_string())?;
    let arch_file = tmp.join(&pkg.name);
    download(app, &pkg.link, &arch_file, &pkg.sha256, &pkg.name).await?;
    say(app, "install", crate::tr!("Estraggo {}…", "Extracting {}…", pkg.name));
    let out = tmp.join("x");
    std::fs::create_dir_all(&out).map_err(|e| e.to_string())?;
    let (a, o) = (arch_file.clone(), out.clone());
    tauri::async_runtime::spawn_blocking(move || extract(&a, &o)).await.map_err(|e| e.to_string())??;
    let _ = std::fs::remove_dir_all(&root);
    std::fs::rename(&out, &root).map_err(|e| e.to_string())?;
    let _ = std::fs::remove_dir_all(&tmp);
    let bin = find_bin(&root, 3).ok_or_else(|| crate::i18n::tr("Java scaricato ma non trovato nell'archivio", "Java downloaded but not found in the archive").to_string())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&bin, std::fs::Permissions::from_mode(0o755));
    }
    let major = java_major(&bin).ok_or_else(|| crate::i18n::tr("Java scaricato ma non parte", "Java downloaded but it doesn't start").to_string())?;
    super::console::log(app, "via", &crate::tr!("Java {} portatile pronto ({})", "portable Java {} ready ({})", pkg.version, bin.display()));
    Ok(JavaInfo { path: bin.display().to_string(), major, managed: true })
}

async fn download_jar(app: &AppHandle) -> Result<(), String> {
    let d = dir(app);
    std::fs::create_dir_all(&d).map_err(|e| e.to_string())?;
    let mut last = String::new();
    for (url, sha) in SOURCES {
        let host = url.split('/').nth(2).unwrap_or(url);
        say(app, "install", crate::tr!("Scarico ViaProxy {} da {}…", "Downloading ViaProxy {} from {}…", VIAPROXY_VERSION, host));
        match download(app, url, &jar(app), sha, &format!("ViaProxy-{VIAPROXY_VERSION}.jar")).await {
            Ok(()) => {
                std::fs::write(marker(app), sha).map_err(|e| e.to_string())?;
                return Ok(());
            }
            Err(e) => {
                super::console::log(app, "via", &format!("{host}: {e}"));
                last = e;
            }
        }
    }
    Err(last)
}

/// Installa il ponte (Java se manca, poi ViaProxy). Restituisce una riga di riepilogo.
pub async fn install(app: &AppHandle) -> Result<String, String> {
    if INSTALLING.swap(true, Ordering::SeqCst) {
        return Err(crate::i18n::tr("installazione già in corso", "installation already running").into());
    }
    let r = install_inner(app).await;
    INSTALLING.store(false, Ordering::SeqCst);
    match &r {
        Ok(s) => {
            crate::actions::log(app, "minecraft", &format!("ponte ViaProxy installato ({s})"), "ok");
            say(app, "done", s.clone());
        }
        Err(e) => {
            crate::actions::log(app, "minecraft", "ponte ViaProxy", e);
            say(app, "error", e.clone());
        }
    }
    r
}

async fn install_inner(app: &AppHandle) -> Result<String, String> {
    let a = app.clone();
    let java = tauri::async_runtime::spawn_blocking(move || find_java(&a)).await.ok().flatten();
    let need_jar = !jar_ok(app);
    if let (Some(j), false) = (&java, need_jar) {
        return Ok(crate::tr!("Pronto: ViaProxy {} e Java {}.", "Ready: ViaProxy {} and Java {}.", VIAPROXY_VERSION, j.major));
    }
    let what = match (&java, need_jar) {
        (Some(j), _) => crate::tr!(
            "Installare il ponte per tutte le versioni di Minecraft?\nUso Java {} già installato e scarico ViaProxy {} (circa 49 MB, GPL-3.0, controllato con SHA-256) da repo.viaversion.com in {}.",
            "Install the bridge for every Minecraft version?\nI'll use the installed Java {} and download ViaProxy {} (about 49 MB, GPL-3.0, SHA-256 checked) from repo.viaversion.com into {}.",
            j.major,
            VIAPROXY_VERSION,
            dir(app).display()
        ),
        (None, true) => crate::tr!(
            "Installare il ponte per tutte le versioni di Minecraft?\nScarico Java {} portatile (Eclipse Temurin da adoptium.net, circa 50 MB, controllato con SHA-256) e ViaProxy {} (circa 49 MB, GPL-3.0, controllato con SHA-256) da repo.viaversion.com in {}.",
            "Install the bridge for every Minecraft version?\nI'll download portable Java {} (Eclipse Temurin from adoptium.net, about 50 MB, SHA-256 checked) and ViaProxy {} (about 49 MB, GPL-3.0, SHA-256 checked) from repo.viaversion.com into {}.",
            JRE_FEATURE,
            VIAPROXY_VERSION,
            super::node::root(app).display()
        ),
        (None, false) => crate::tr!(
            "Il ponte ViaProxy ha bisogno di Java 17 o più recente.\nScarico Java {} portatile (Eclipse Temurin da adoptium.net, circa 50 MB, controllato con SHA-256) in {}?",
            "The ViaProxy bridge needs Java 17 or newer.\nDownload portable Java {} (Eclipse Temurin from adoptium.net, about 50 MB, SHA-256 checked) into {}?",
            JRE_FEATURE,
            java_dir(app).display()
        ),
    };
    if !crate::actions::confirm_always(app, what).await {
        return Err("the user denied permission".into());
    }
    let java = match java {
        Some(j) => j,
        None => download_java(app).await?,
    };
    if !jar_ok(app) {
        download_jar(app).await?;
    }
    Ok(crate::tr!("Pronto: ViaProxy {} e Java {}.", "Ready: ViaProxy {} and Java {}.", VIAPROXY_VERSION, java.major))
}

// ---------- scelta: nativo o ponte ----------

/// Risposta di `node bot.js --ping` (minecraft-protocol `ping` + versioni di Mineflayer).
#[derive(Deserialize, Serialize, Default, Debug, Clone, PartialEq)]
#[serde(default)]
pub struct Ping {
    pub ok: bool,
    pub host: String,
    pub port: u16,
    /// Numero di protocollo del server.
    pub protocol: i64,
    /// Nome della versione detto dal server ("Paper 1.21.4", "26.3"…).
    pub name: String,
    pub motd: String,
    /// Versione che Mineflayer sa parlare con questo protocollo (None = non la conosce).
    pub native: Option<String>,
    /// Ultima versione di Mineflayer.
    pub latest: String,
    pub err: String,
}

pub fn parse_ping(out: &str) -> Option<Ping> {
    out.lines().find_map(|l| {
        let v: Value = serde_json::from_str(l.trim()).ok()?;
        (v["t"] == "ping").then(|| serde_json::from_value(v).ok()).flatten()
    })
}

#[derive(Debug, Clone, PartialEq)]
pub enum Route {
    /// Mineflayer da solo, con questa versione ("" = la trova lui).
    Native(String),
    /// Attraverso ViaProxy: il bot parla `client`, ViaProxy parla al server `target` ("" = automatica).
    Bridge { client: String, target: String },
}

/// Modalità "auto", "native" o "via"; `configured` = versione scritta nelle impostazioni.
pub fn route(mode: &str, configured: &str, ping: Option<&Ping>) -> Route {
    let configured = configured.trim();
    let latest = ping.map(|p| p.latest.clone()).filter(|l| !l.is_empty()).unwrap_or_default();
    match mode {
        "native" => Route::Native(configured.to_string()),
        // via ponte sempre: la versione scritta (se c'è) è quella del server
        "via" => Route::Bridge { client: latest, target: if configured.is_empty() { TARGET_AUTO.to_string() } else { configured.to_string() } },
        _ => match ping.filter(|p| p.ok) {
            // il server non risponde al ping: si prova come prima
            None => Route::Native(configured.to_string()),
            Some(p) => match &p.native {
                Some(v) => Route::Native(if configured.is_empty() { v.clone() } else { configured.to_string() }),
                None => Route::Bridge { client: latest, target: TARGET_AUTO.to_string() },
            },
        },
    }
}

/// "host:porta", con le quadre per IPv6.
pub fn addr(host: &str, port: u16) -> String {
    if host.contains(':') && !host.starts_with('[') {
        format!("[{host}]:{port}")
    } else {
        format!("{host}:{port}")
    }
}

/// Ping del server con il bot (una tantum, ~1-6 s). Con porta 0 cerca prima il mondo in LAN.
fn ping(node: &str, bot_dir: &Path, host: &str, port: u16) -> Option<Ping> {
    let mut child = super::node::hidden(Command::new(node).arg("bot.js").arg("--ping").arg(host).arg(port.to_string()).current_dir(bot_dir)).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn().ok()?;
    let out = child.stdout.take()?;
    let reader = std::thread::spawn(move || {
        let mut s = String::new();
        let _ = std::io::Read::read_to_string(&mut BufReader::new(out), &mut s);
        s
    });
    let t0 = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if t0.elapsed() < Duration::from_secs(20) => std::thread::sleep(Duration::from_millis(100)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                break;
            }
        }
    }
    parse_ping(&reader.join().unwrap_or_default())
}

// ---------- processo di ViaProxy ----------

struct Proxy {
    child: Child,
    port: u16,
    /// Server vero (host, porta).
    target: (String, u16),
    version: String,
}

static PROXY: Mutex<Option<Proxy>> = Mutex::new(None);

fn proxy() -> std::sync::MutexGuard<'static, Option<Proxy>> {
    PROXY.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Codici di colore ANSI tolti (i log di ViaProxy sono colorati).
pub fn strip_ansi(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut it = s.chars().peekable();
    while let Some(c) = it.next() {
        if c == '\x1b' {
            if it.peek() == Some(&'[') {
                it.next();
                for d in it.by_ref() {
                    if d.is_ascii_alphabetic() {
                        break;
                    }
                }
            }
            continue;
        }
        out.push(c);
    }
    out
}

/// Porta libera su 127.0.0.1.
pub fn free_port() -> Option<u16> {
    std::net::TcpListener::bind(("127.0.0.1", 0)).ok()?.local_addr().ok().map(|a| a.port())
}

/// Il ponte è acceso: (porta locale, server vero, versione del bot).
pub fn active() -> Option<(u16, (String, u16), String)> {
    let mut g = proxy();
    let alive = g.as_mut().is_some_and(|p| matches!(p.child.try_wait(), Ok(None)));
    if !alive {
        *g = None;
        return None;
    }
    g.as_ref().map(|p| (p.port, p.target.clone(), p.version.clone()))
}

/// Argomenti di ViaProxy (modalità riga di comando, solo su 127.0.0.1, account offline).
pub fn proxy_args(jar: &str, port: u16, host: &str, tport: u16, target: &str) -> Vec<String> {
    let mut a: Vec<String> = ["-Xms64M", "-Xmx1G", "-jar", jar, "cli", "--bind-address"].iter().map(|s| s.to_string()).collect();
    a.push(format!("127.0.0.1:{port}"));
    a.push("--target-address".into());
    a.push(addr(host, tport));
    for (k, v) in [("--auth-method", "NONE"), ("--proxy-online-mode", "false"), ("--log-ips", "false")] {
        a.push(k.into());
        a.push(v.into());
    }
    if !target.trim().is_empty() {
        a.push("--target-version".into());
        a.push(target.trim().into());
    }
    a
}

/// Avvia ViaProxy verso `host:tport` e aspetta che ascolti. Restituisce la porta locale.
pub fn start_proxy(app: &AppHandle, host: &str, tport: u16, target: &str, client: &str) -> Result<u16, String> {
    stop_proxy(app);
    let java = find_java(app).ok_or_else(|| crate::i18n::tr("Java 17 o più recente non trovato: premi «Installa il ponte»", "Java 17 or newer not found: press \"Install the bridge\"").to_string())?;
    if !jar_ok(app) {
        return Err(crate::i18n::tr("ViaProxy non è installato: premi «Installa il ponte»", "ViaProxy isn't installed: press \"Install the bridge\"").into());
    }
    let port = free_port().ok_or("nessuna porta libera")?;
    let d = dir(app);
    let args = proxy_args(&jar(app).display().to_string(), port, host, tport, target);
    let mut child = super::node::hidden(Command::new(&java.path).args(&args).current_dir(&d))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| crate::tr!("avvio di ViaProxy non riuscito: {}", "could not start ViaProxy: {}", e))?;
    super::console::log(app, "via", &crate::tr!("ViaProxy {} (Java {}) su 127.0.0.1:{} → {}", "ViaProxy {} (Java {}) on 127.0.0.1:{} → {}", VIAPROXY_VERSION, java.major, port, addr(host, tport)));
    let ready = std::sync::Arc::new(AtomicBool::new(false));
    let last = std::sync::Arc::new(Mutex::new(String::new()));
    let log_path = d.join("viaproxy.log");
    let _ = std::fs::write(&log_path, "");
    for s in [child.stdout.take().map(|s| Box::new(s) as Box<dyn std::io::Read + Send>), child.stderr.take().map(|s| Box::new(s) as Box<dyn std::io::Read + Send>)].into_iter().flatten() {
        let (app, ready, last, log_path) = (app.clone(), ready.clone(), last.clone(), log_path.clone());
        let _ = std::thread::Builder::new().name("viaproxy-log".into()).spawn(move || {
            let mut file = std::fs::OpenOptions::new().append(true).open(&log_path).ok();
            for line in BufReader::new(s).lines().map_while(Result::ok) {
                let l = strip_ansi(&line);
                if l.trim().is_empty() || l.contains("Picked up ") {
                    continue;
                }
                if let Some(f) = file.as_mut() {
                    let _ = writeln!(f, "{l}");
                }
                // "[12:00:00] [main/INFO] (ViaProxy) messaggio" → "messaggio"
                let msg = l.split_once(") ").map(|x| x.1).unwrap_or(&l).trim().to_string();
                let low = msg.to_lowercase();
                if low.contains("binding proxy server") {
                    ready.store(true, Ordering::SeqCst);
                }
                *last.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = msg.clone();
                super::console::log(&app, if low.contains("error") || low.contains("exception") { "error" } else { "via" }, &format!("ViaProxy: {msg}"));
            }
        });
    }
    *proxy() = Some(Proxy { child, port, target: (host.to_string(), tport), version: client.to_string() });
    // pronto quando lo dice o quando la porta accetta connessioni
    let t0 = Instant::now();
    while t0.elapsed() < Duration::from_secs(60) {
        if ready.load(Ordering::SeqCst) {
            std::thread::sleep(Duration::from_millis(300));
            return Ok(port);
        }
        if active().is_none() {
            let l = last.lock().unwrap_or_else(std::sync::PoisonError::into_inner).clone();
            return Err(crate::tr!("ViaProxy si è chiuso: {}", "ViaProxy exited: {}", l));
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    stop_proxy(app);
    Err(crate::i18n::tr("ViaProxy non è partito in tempo", "ViaProxy didn't start in time").into())
}

/// Chiude ViaProxy (bot uscito o app chiusa).
pub fn stop_proxy(app: &AppHandle) {
    if shutdown() {
        super::console::log(app, "via", crate::i18n::tr("ViaProxy chiuso", "ViaProxy closed"));
    }
}

/// Chiude ViaProxy senza app (uscita). true se era acceso.
pub fn shutdown() -> bool {
    let Some(mut p) = proxy().take() else { return false };
    // "exit" sulla console di ViaProxy, poi la chiusura forzata
    if let Some(mut s) = p.child.stdin.take() {
        let _ = s.write_all(b"exit\n");
    }
    let t0 = Instant::now();
    while t0.elapsed() < Duration::from_millis(1500) {
        if matches!(p.child.try_wait(), Ok(Some(_))) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    let _ = p.child.kill();
    let _ = p.child.wait();
    true
}

/// Il bot entra nel mondo: decide nativo o ponte (ping di stato), avvia ViaProxy se serve, poi
/// manda `connect`. Gira in un thread: il ping e l'avvio di Java richiedono qualche secondo.
pub fn launch(app: &AppHandle, mut connect: Value, node: String, bot_dir: PathBuf, gen: u64) {
    let app = app.clone();
    let _ = std::thread::Builder::new().name("minecraft-route".into()).spawn(move || {
        let cfg = super::cfg_of(&app).minecraft;
        let mode = cfg.version_mode.clone();
        let host = connect["host"].as_str().unwrap_or("127.0.0.1").to_string();
        let port = connect["port"].as_u64().unwrap_or(0) as u16;
        let p = if mode == "native" {
            None
        } else {
            super::console::log(&app, "via", &crate::tr!("Chiedo la versione al server {}…", "Asking {} for its version…", if port == 0 { "LAN".to_string() } else { addr(&host, port) }));
            ping(&node, &bot_dir, &host, port)
        };
        // il bot è stato fermato nel frattempo
        if super::lock(&app).gen != gen {
            return;
        }
        if let Some(p) = &p {
            if p.ok {
                super::console::log(&app, "via", &crate::tr!("server: {} (protocollo {}){}", "server: {} (protocol {}){}", p.name, p.protocol, p.native.as_ref().map(|v| format!(", Mineflayer {v}")).unwrap_or_else(|| crate::i18n::tr(", non supportato da Mineflayer", ", not supported by Mineflayer").into())));
                // LAN: porta trovata dal ping
                if port == 0 && p.port != 0 {
                    connect["host"] = json!(p.host);
                    connect["port"] = json!(p.port);
                }
            } else {
                super::console::log(&app, "via", &crate::tr!("ping non riuscito: {}", "ping failed: {}", p.err));
            }
        }
        let (rhost, rport) = (connect["host"].as_str().unwrap_or("127.0.0.1").to_string(), connect["port"].as_u64().unwrap_or(0) as u16);
        match route(&mode, &cfg.version, p.as_ref()) {
            Route::Native(v) => {
                if !v.is_empty() {
                    connect["version"] = json!(v);
                }
                stop_proxy(&app);
            }
            Route::Bridge { client, target } => {
                if rport == 0 {
                    super::on_event(&app, json!({ "kind": "error", "code": "bridge", "msg": crate::i18n::tr("nessun mondo in LAN trovato per il ponte", "no LAN world found for the bridge") }));
                    super::stop(&app);
                    return;
                }
                match start_proxy(&app, &rhost, rport, &target, &client) {
                    Ok(lp) => {
                        connect["host"] = json!("127.0.0.1");
                        connect["port"] = json!(lp);
                        connect["version"] = json!(client);
                        super::console::log(&app, "via", &crate::tr!("entro attraverso il ponte con Minecraft {}", "joining through the bridge as Minecraft {}", if client.is_empty() { "?".into() } else { client.clone() }));
                    }
                    Err(e) => {
                        super::on_event(&app, json!({ "kind": "error", "code": "bridge", "msg": e }));
                        super::stop(&app);
                        return;
                    }
                }
            }
        }
        if super::lock(&app).gen == gen {
            super::send(&app, vec![connect]);
        }
    });
}

/// Mentre il ponte è acceso il mondo è quello vero (non 127.0.0.1:porta del ponte).
pub fn real_world(e: &mut Value) {
    if let Some((lp, (h, p), _)) = active() {
        if e["port"].as_u64() == Some(lp as u64) {
            e["host"] = json!(h);
            e["port"] = json!(p);
            e["via"] = json!(VIAPROXY_VERSION);
        }
    }
}

// ---------- comandi ----------

#[derive(Serialize)]
pub struct ViaStatus {
    pub java: Option<JavaInfo>,
    pub jar: bool,
    pub version: &'static str,
    pub installing: bool,
    /// Ponte acceso: porta locale e server vero.
    pub active: Option<Value>,
    pub java_min: u32,
}

#[tauri::command]
pub async fn minecraft_via_status(app: AppHandle) -> ViaStatus {
    let a = app.clone();
    let java = tauri::async_runtime::spawn_blocking(move || find_java(&a)).await.ok().flatten();
    ViaStatus {
        java,
        jar: jar_ok(&app),
        version: VIAPROXY_VERSION,
        installing: installing(),
        active: active().map(|(port, (h, p), v)| json!({ "port": port, "target": addr(&h, p), "client": v })),
        java_min: JAVA_MIN,
    }
}

#[tauri::command]
pub async fn minecraft_via_install(app: AppHandle) -> Result<String, String> {
    install(&app).await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ping(ok: bool, native: Option<&str>) -> Ping {
        Ping { ok, protocol: 777, name: "26.3".into(), native: native.map(String::from), latest: "26.1".into(), ..Default::default() }
    }

    #[test]
    fn picks_native_or_bridge() {
        // automatica: Mineflayer conosce la versione
        assert_eq!(route("auto", "", Some(&ping(true, Some("1.21.4")))), Route::Native("1.21.4".into()));
        // la versione scritta a mano vince
        assert_eq!(route("auto", "1.21.4", Some(&ping(true, Some("1.21.5")))), Route::Native("1.21.4".into()));
        // non la conosce: ponte con l'ultima versione di Mineflayer
        assert_eq!(route("auto", "", Some(&ping(true, None))), Route::Bridge { client: "26.1".into(), target: "".into() });
        assert_eq!(route("auto", "1.21.4", Some(&ping(true, None))), Route::Bridge { client: "26.1".into(), target: "".into() });
        // ping fallito: come prima
        assert_eq!(route("auto", "", Some(&ping(false, None))), Route::Native("".into()));
        assert_eq!(route("auto", "1.20.1", None), Route::Native("1.20.1".into()));
        // nativa: niente ponte
        assert_eq!(route("native", "", Some(&ping(true, None))), Route::Native("".into()));
        // via ponte: sempre, con la versione scritta come versione del server
        assert_eq!(route("via", "", Some(&ping(true, Some("1.21.4")))), Route::Bridge { client: "26.1".into(), target: "".into() });
        assert_eq!(route("via", "b1.7.3", Some(&ping(false, None))), Route::Bridge { client: "26.1".into(), target: "b1.7.3".into() });
    }

    #[test]
    fn reads_ping_line() {
        let out = "log a caso\n{\"t\":\"ping\",\"ok\":true,\"host\":\"127.0.0.1\",\"port\":25565,\"protocol\":777,\"name\":\"26.3\",\"motd\":\"A Minecraft Server\",\"native\":null,\"latest\":\"26.1\"}\n";
        let p = parse_ping(out).unwrap();
        assert!(p.ok && p.native.is_none() && p.protocol == 777 && p.latest == "26.1");
        assert!(parse_ping("niente").is_none());
        let p = parse_ping("{\"t\":\"ping\",\"ok\":false,\"err\":\"ECONNREFUSED\",\"latest\":\"26.1\"}").unwrap();
        assert!(!p.ok && p.err == "ECONNREFUSED");
    }

    #[test]
    fn java_versions() {
        assert_eq!(parse_java_major("openjdk version \"21.0.13.0.3\" 2026-10-24\nOpenJDK Runtime Environment"), Some(21));
        assert_eq!(parse_java_major("java version \"1.8.0_202\"\nJava(TM) SE Runtime"), Some(8));
        assert_eq!(parse_java_major("openjdk version \"17\" 2021-09-14"), Some(17));
        assert_eq!(parse_java_major("openjdk version \"25-ea\" 2025-09-16"), Some(25));
        assert_eq!(parse_java_major("Picked up JAVA_TOOL_OPTIONS: -Dx\nopenjdk version \"11.0.2\" 2019-01-15"), Some(11));
        assert_eq!(parse_java_major("bash: java: command not found"), None);
    }

    #[test]
    fn adoptium_and_checksums() {
        let v: Value = serde_json::from_str(r#"[{"binary":{"image_type":"jre","package":{"checksum":"18F44FAB9065217F3BA61D0EEF8A7B5F3C3713AA7293001B0D4AC10EDE3850F0","checksum_link":"https://github.com/adoptium/temurin21-binaries/releases/download/jdk-21.0.13.1%2B2/OpenJDK21U-jre_x64_linux_hotspot_21.0.13.1_2.tar.gz.sha256.txt","link":"https://github.com/adoptium/temurin21-binaries/releases/download/jdk-21.0.13.1%2B2/OpenJDK21U-jre_x64_linux_hotspot_21.0.13.1_2.tar.gz","name":"OpenJDK21U-jre_x64_linux_hotspot_21.0.13.1_2.tar.gz","size":50000000}},"release_name":"jdk-21.0.13.1+2","version":{"openjdk_version":"21.0.13.1+2-LTS"}}]"#).unwrap();
        let p = parse_adoptium(&v).unwrap();
        assert_eq!(p.sha256, "18f44fab9065217f3ba61d0eef8a7b5f3c3713aa7293001b0d4ac10ede3850f0");
        assert_eq!(p.version, "21.0.13.1+2-LTS");
        assert!(p.name.ends_with(".tar.gz"));
        // niente JDK, niente checksum finti, niente http
        assert!(parse_adoptium(&serde_json::json!([{ "binary": { "image_type": "jdk", "package": { "checksum": "a".repeat(64), "link": "https://x/y.zip", "name": "y.zip" } } }])).is_none());
        assert!(parse_adoptium(&serde_json::json!([{ "binary": { "image_type": "jre", "package": { "checksum": "abc", "link": "https://x/y.zip", "name": "y.zip" } } }])).is_none());
        assert!(parse_adoptium(&serde_json::json!([{ "binary": { "image_type": "jre", "package": { "checksum": "a".repeat(64), "link": "http://x/y.zip", "name": "y.zip" } } }])).is_none());
        assert!(parse_adoptium(&serde_json::json!({})).is_none());
        let f = "18f44fab9065217f3ba61d0eef8a7b5f3c3713aa7293001b0d4ac10ede3850f0  OpenJDK21U-jre_x64_linux_hotspot_21.0.13.1_2.tar.gz\n";
        assert_eq!(parse_sha256_file(f, "OpenJDK21U-jre_x64_linux_hotspot_21.0.13.1_2.tar.gz").as_deref(), Some(&f[..64]));
        assert_eq!(parse_sha256_file(f, "altro.zip"), None);
        assert_eq!(parse_sha256_file(&"B".repeat(64), "x").unwrap(), "b".repeat(64));
        assert_eq!(parse_sha256_file("ciao", "x"), None);
        assert_eq!(adoptium_platform("windows", "x86_64"), Some(("windows", "x64")));
        assert_eq!(adoptium_platform("macos", "aarch64"), Some(("mac", "aarch64")));
        assert_eq!(adoptium_platform("freebsd", "x86_64"), None);
        assert!(adoptium_url(21, "linux", "x64").contains("/latest/21/hotspot?os=linux&architecture=x64&image_type=jre"));
        // impronte fissate del JAR: esadecimali da 64
        assert!(SOURCES.iter().all(|(u, h)| u.starts_with("https://") && u.contains(VIAPROXY_VERSION) && is_sha256(h)));
    }

    #[test]
    fn proxy_bits() {
        assert_eq!(strip_ansi("\x1b[34m[19:18:40]\x1b[m \x1b[32m[main/INFO]\x1b[m ciao"), "[19:18:40] [main/INFO] ciao");
        assert_eq!(addr("::1", 25565), "[::1]:25565");
        assert_eq!(addr("play.example.net", 25565), "play.example.net:25565");
        let a = proxy_args("/x/ViaProxy.jar", 40000, "mc.example.net", 25566, "");
        let s = a.join(" ");
        assert!(s.contains("-jar /x/ViaProxy.jar cli --bind-address 127.0.0.1:40000 --target-address mc.example.net:25566"));
        assert!(s.contains("--auth-method NONE") && !s.contains("--target-version"));
        assert!(proxy_args("j", 1, "h", 2, "1.8.x").join(" ").ends_with("--target-version 1.8.x"));
        assert!(free_port().is_some_and(|p| p > 0));
    }
}
