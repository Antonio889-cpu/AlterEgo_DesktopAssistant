//! Modelli in locale con Ollama, gestiti da Alter Ego (Impostazioni → Modelli locali):
//! - Ollama trovato, installato (con conferma) e avviato se serve: su Windows l'installer ufficiale
//!   (in modalità silenziosa, senza amministratore), su Linux il pacchetto ufficiale estratto nella
//!   cartella dell'app ed eseguito con `ollama serve` (niente sudo);
//! - LFM3 di Liquid AI in un clic (`lfm3` dalla libreria di Ollama, oppure i GGUF ufficiali su Hugging Face);
//! - catalogo con ricerca e filtri (libreria ufficiale, modelli della community, GGUF su Hugging Face),
//!   download con avanzamento, modelli installati, download da un indirizzo (hf.co, Hugging Face, .gguf);
//! - GPU: elenco schede, quale usa Ollama (dal suo log), impostazioni per AMD (Vulkan/ROCm, scelta della
//!   scheda dedicata sui PC con grafica integrata Intel, flash attention, cache K/V ridotta).
//!
//! Fonti: docs.ollama.com (API, gpu, windows, linux), huggingface.co/docs/hub/ollama, scheda di LFM3-1.7B.

use crate::{config::LocalConfig, update_config, AppState};
use futures_util::StreamExt;
use serde::Serialize;
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        Mutex,
    },
    time::Duration,
};
use tauri::{AppHandle, Emitter, Manager};

/// Versione minima di Ollama per LFM3 (scheda del modello: Ollama v0.40.12).
pub const LFM3_MIN: &str = "0.40.12";
const DEFAULT_BASE: &str = "http://127.0.0.1:11434";

static SERVE: Mutex<Option<Child>> = Mutex::new(None);
static BUSY: AtomicBool = AtomicBool::new(false);
/// Download in corso: nome → flag per annullare.
static PULLS: Mutex<Option<HashMap<String, std::sync::Arc<AtomicBool>>>> = Mutex::new(None);
/// Pagina della libreria di Ollama (si scarica al massimo una volta all'ora).
static LIBRARY: Mutex<Option<(std::time::Instant, Vec<Model>)>> = Mutex::new(None);

// ---------- dove sta Ollama ----------

/// Indirizzo del server (dall'URL del provider Ollama, senza /v1).
pub fn base(app: &AppHandle) -> String {
    let url = app.state::<AppState>().cfg.lock().unwrap().ai.providers.get("ollama").map(|p| p.base_url.clone()).unwrap_or_default();
    let u = url.trim().trim_end_matches('/');
    if u.is_empty() {
        DEFAULT_BASE.into()
    } else {
        crate::providers::ollama_root(u)
    }
}

fn http(app: &AppHandle) -> reqwest::Client {
    app.state::<AppState>().ai.local.clone()
}

/// Cartella dell'Ollama installato da Alter Ego (solo Linux).
fn managed_dir(app: &AppHandle) -> PathBuf {
    app.path().app_local_data_dir().unwrap_or_else(|_| std::env::temp_dir()).join("ollama")
}

/// Eseguibile di Ollama: quello di Alter Ego, quello installato per l'utente, oppure nel PATH.
pub fn binary(app: &AppHandle) -> Option<PathBuf> {
    let exe = if cfg!(windows) { "ollama.exe" } else { "ollama" };
    let mut c = vec![managed_dir(app).join("bin").join(exe)];
    if let Some(l) = std::env::var_os("LOCALAPPDATA") {
        c.push(PathBuf::from(l).join("Programs").join("Ollama").join(exe));
    }
    c.extend(["/usr/local/bin/ollama", "/usr/bin/ollama"].map(PathBuf::from));
    if let Some(path) = std::env::var_os("PATH") {
        c.extend(std::env::split_paths(&path).map(|d| d.join(exe)));
    }
    c.into_iter().find(|p| p.is_file())
}

/// App di Ollama su Windows (avvia il server nella tray).
fn windows_app() -> Option<PathBuf> {
    let p = PathBuf::from(std::env::var_os("LOCALAPPDATA")?).join("Programs").join("Ollama").join("ollama app.exe");
    p.is_file().then_some(p)
}

/// Versione del server acceso, se risponde.
pub async fn version(app: &AppHandle) -> Option<String> {
    let v: Value = http(app).get(format!("{}/api/version", base(app))).timeout(Duration::from_secs(3)).send().await.ok()?.json().await.ok()?;
    v.get("version").and_then(Value::as_str).map(str::to_string)
}

/// a ≥ b per versioni "x.y.z" (le parti mancanti valgono 0; "0.40.12-rc1" → 0.40.12).
pub fn version_at_least(a: &str, b: &str) -> bool {
    let parse = |s: &str| -> Vec<u64> { s.trim().trim_start_matches('v').split(['.', '-', '+']).take(3).map(|p| p.parse().unwrap_or(0)).collect() };
    let (x, y) = (parse(a), parse(b));
    for i in 0..3 {
        let (p, q) = (x.get(i).copied().unwrap_or(0), y.get(i).copied().unwrap_or(0));
        if p != q {
            return p > q;
        }
    }
    true
}

// ---------- avanzamento ----------

#[derive(Serialize, Clone, Default)]
pub struct Progress {
    /// "install", "pull", "serve", "gpu".
    pub task: String,
    pub name: String,
    pub status: String,
    pub completed: u64,
    pub total: u64,
    pub done: bool,
    pub error: Option<String>,
}

fn emit(app: &AppHandle, p: Progress) {
    let _ = app.emit("local-progress", p);
}

fn step(app: &AppHandle, task: &str, name: &str, status: impl Into<String>) {
    emit(app, Progress { task: task.into(), name: name.into(), status: status.into(), ..Default::default() });
}

// ---------- avvio del server ----------

/// Variabili d'ambiente per il server secondo le impostazioni GPU (vedi docs.ollama.com/gpu).
pub fn server_env(c: &LocalConfig) -> Vec<(&'static str, String)> {
    let mut v = Vec::new();
    match c.backend.as_str() {
        "vulkan" => {
            v.push(("OLLAMA_VULKAN", "1".into()));
            // con ROCm presente, Vulkan resta l'unico backend AMD
            v.push(("ROCR_VISIBLE_DEVICES", "-1".into()));
            v.push(("HIP_VISIBLE_DEVICES", "-1".into()));
        }
        "rocm" => {
            v.push(("OLLAMA_VULKAN", "0".into()));
            if !c.hsa_override.is_empty() && !cfg!(windows) {
                v.push(("HSA_OVERRIDE_GFX_VERSION", c.hsa_override.clone()));
            }
        }
        "cpu" => {
            v.push(("OLLAMA_VULKAN", "0".into()));
            v.push(("GGML_VK_VISIBLE_DEVICES", "-1".into()));
            v.push(("ROCR_VISIBLE_DEVICES", "-1".into()));
            v.push(("CUDA_VISIBLE_DEVICES", "-1".into()));
        }
        _ => {}
    }
    if !c.vk_device.is_empty() && c.backend != "cpu" {
        v.push(("GGML_VK_VISIBLE_DEVICES", c.vk_device.clone()));
    }
    v.push(("OLLAMA_FLASH_ATTENTION", if c.flash_attention { "1" } else { "0" }.into()));
    if c.kv_cache != "f16" {
        v.push(("OLLAMA_KV_CACHE_TYPE", c.kv_cache.clone()));
    }
    v
}

/// Tutte le variabili che Alter Ego può impostare (per toglierle quando non servono più).
const ENV_NAMES: &[&str] = &[
    "OLLAMA_VULKAN", "ROCR_VISIBLE_DEVICES", "HIP_VISIBLE_DEVICES", "HSA_OVERRIDE_GFX_VERSION", "GGML_VK_VISIBLE_DEVICES",
    "CUDA_VISIBLE_DEVICES", "OLLAMA_FLASH_ATTENTION", "OLLAMA_KV_CACHE_TYPE",
];

fn serve_log(app: &AppHandle) -> PathBuf {
    app.state::<crate::assistant::Conversation>().data_dir.join("ollama-serve.log")
}

/// Avvia `ollama serve` gestito da Alter Ego (con le variabili della GPU).
fn spawn_serve(app: &AppHandle) -> Result<(), String> {
    let bin = binary(app).ok_or_else(|| crate::i18n::tr("Ollama non è installato", "Ollama is not installed").to_string())?;
    let c = app.state::<AppState>().cfg.lock().unwrap().local.clone();
    let log = std::fs::File::create(serve_log(app)).map_err(|e| e.to_string())?;
    let mut cmd = Command::new(&bin);
    cmd.arg("serve").stdin(Stdio::null()).stdout(log.try_clone().map_err(|e| e.to_string())?).stderr(log);
    for n in ENV_NAMES {
        cmd.env_remove(n);
    }
    for (k, v) in server_env(&c) {
        cmd.env(k, v);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000);
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }
    let child = cmd.spawn().map_err(|e| crate::tr!("Ollama non si avvia: {e}", "Ollama does not start: {e}"))?;
    crate::applog::info("ollama", &format!("avviato {} serve (pid {})", bin.display(), child.id()));
    *SERVE.lock().unwrap() = Some(child);
    Ok(())
}

/// Ollama acceso: se non risponde lo avvia (app di Windows, oppure `ollama serve`) e aspetta fino a 40 s.
pub async fn ensure_running(app: &AppHandle) -> Result<String, String> {
    if let Some(v) = version(app).await {
        return Ok(v);
    }
    if !is_local(&base(app)) {
        return Err(crate::i18n::tr("il server Ollama configurato non risponde", "the configured Ollama server does not answer").into());
    }
    match windows_app() {
        Some(a) if managed_env_is_user(app) => {
            // stesse variabili delle impostazioni GPU (Alter Ego potrebbe avere quelle vecchie)
            let c = app.state::<AppState>().cfg.lock().unwrap().local.clone();
            let mut cmd = Command::new(a);
            for n in ENV_NAMES {
                cmd.env_remove(n);
            }
            for (k, v) in server_env(&c) {
                cmd.env(k, v);
            }
            cmd.spawn().map_err(|e| e.to_string())?;
        }
        _ => spawn_serve(app)?,
    }
    for _ in 0..80 {
        tokio::time::sleep(Duration::from_millis(500)).await;
        if let Some(v) = version(app).await {
            return Ok(v);
        }
    }
    Err(crate::i18n::tr("Ollama non risponde dopo l'avvio (vedi il Log)", "Ollama does not answer after starting (see the Log)").into())
}

fn is_local(url: &str) -> bool {
    crate::providers::is_loopback(url)
}

/// Su Windows con l'app ufficiale le impostazioni vanno nelle variabili d'ambiente dell'utente
/// (docs.ollama.com/faq). Altrove Alter Ego avvia il server con le variabili giuste.
fn managed_env_is_user(app: &AppHandle) -> bool {
    cfg!(windows) && !managed_dir(app).join("bin").join("ollama.exe").is_file()
}

/// Ferma il server avviato da Alter Ego (all'uscita).
pub fn stop(_app: &AppHandle) {
    if let Some(mut c) = SERVE.lock().unwrap().take() {
        #[cfg(unix)]
        {
            let _ = Command::new("kill").args(["-TERM", &format!("-{}", c.id())]).status();
        }
        #[cfg(windows)]
        {
            let _ = c.kill();
        }
        let _ = c.wait();
    }
}

/// All'avvio di Alter Ego: se Ollama è il provider attivo, installato e spento, lo accende.
pub fn autostart(app: &AppHandle) {
    let (on, enabled) = {
        let st = app.state::<AppState>();
        let c = st.cfg.lock().unwrap();
        (c.local.autostart, c.ai.providers.get("ollama").map(|p| p.enabled).unwrap_or(false))
    };
    if !on || !enabled || binary(app).is_none() {
        return;
    }
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        if let Err(e) = ensure_running(&app).await {
            crate::applog::warn("ollama", &format!("avvio automatico: {e}"));
        }
    });
}

// ---------- installazione di Ollama ----------

async fn download(app: &AppHandle, url: &str, dest: &Path, task: &str, name: &str) -> Result<(), String> {
    let http = app.state::<AppState>().ai.http.clone();
    let resp = http.get(url).send().await.and_then(|r| r.error_for_status()).map_err(|e| crate::tr!("download non riuscito: {e}", "download failed: {e}"))?;
    let total = resp.content_length().unwrap_or(0);
    let tmp = dest.with_extension("part");
    let mut f = std::fs::File::create(&tmp).map_err(|e| e.to_string())?;
    let mut done = 0u64;
    let mut last = std::time::Instant::now();
    let mut stream = resp.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| e.to_string())?;
        std::io::Write::write_all(&mut f, &chunk).map_err(|e| e.to_string())?;
        done += chunk.len() as u64;
        if last.elapsed() > Duration::from_millis(400) {
            last = std::time::Instant::now();
            emit(app, Progress { task: task.into(), name: name.into(), status: crate::i18n::tr("download", "downloading").into(), completed: done, total, ..Default::default() });
        }
    }
    drop(f);
    std::fs::rename(&tmp, dest).map_err(|e| e.to_string())
}

/// Installa Ollama (o lo aggiorna). Chiede sempre conferma: scarica ed esegue software.
#[tauri::command]
pub async fn local_install(app: AppHandle) -> Result<String, String> {
    let q = if cfg!(windows) {
        crate::i18n::tr(
            "Scaricare e installare Ollama (installer ufficiale da ollama.com, per il tuo utente, senza amministratore)?",
            "Download and install Ollama (official installer from ollama.com, for your user, no administrator)?",
        )
    } else {
        crate::i18n::tr(
            "Scaricare Ollama (pacchetto ufficiale da ollama.com) nella cartella di Alter Ego e avviarlo? Non serve sudo.",
            "Download Ollama (official package from ollama.com) into Alter Ego's folder and start it? No sudo needed.",
        )
    };
    if !crate::actions::confirm_always(&app, q.into()).await {
        return Err(crate::i18n::tr("installazione annullata", "installation cancelled").into());
    }
    if BUSY.swap(true, Ordering::SeqCst) {
        return Err(crate::i18n::tr("c'è già un'installazione in corso", "an installation is already running").into());
    }
    let r = install(&app).await;
    BUSY.store(false, Ordering::SeqCst);
    match &r {
        Ok(v) => {
            crate::applog::info("ollama", &format!("installato, versione {v}"));
            emit(&app, Progress { task: "install".into(), status: crate::tr!("Ollama {v} pronto", "Ollama {v} ready"), done: true, ..Default::default() });
        }
        Err(e) => {
            crate::applog::error("ollama", &format!("installazione: {e}"));
            emit(&app, Progress { task: "install".into(), error: Some(e.clone()), done: true, ..Default::default() });
        }
    }
    r
}

async fn install(app: &AppHandle) -> Result<String, String> {
    let dir = managed_dir(app);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    if cfg!(windows) {
        let setup = dir.join("OllamaSetup.exe");
        download(app, "https://ollama.com/download/OllamaSetup.exe", &setup, "install", "Ollama").await?;
        step(app, "install", "Ollama", crate::i18n::tr("installazione…", "installing…"));
        // Inno Setup: installazione silenziosa nel profilo dell'utente
        let st = tauri::async_runtime::spawn_blocking(move || Command::new(&setup).args(["/VERYSILENT", "/SUPPRESSMSGBOXES", "/NORESTART"]).status())
            .await
            .map_err(|e| e.to_string())?
            .map_err(|e| e.to_string())?;
        if !st.success() {
            return Err(crate::tr!("l'installer di Ollama è terminato con errore ({st})", "the Ollama installer ended with an error ({st})"));
        }
    } else {
        // tar con --zstd ha bisogno di zstd
        if Command::new("zstd").arg("--version").stdout(Stdio::null()).stderr(Stdio::null()).status().is_err() {
            return Err(crate::i18n::tr(
                "per estrarre Ollama serve zstd (es. sudo apt install zstd), oppure installa Ollama con lo script ufficiale",
                "zstd is needed to extract Ollama (e.g. sudo apt install zstd), or install Ollama with the official script",
            )
            .into());
        }
        let pkg = dir.join("ollama-linux-amd64.tar.zst");
        download(app, "https://ollama.com/download/ollama-linux-amd64.tar.zst", &pkg, "install", "Ollama").await?;
        step(app, "install", "Ollama", crate::i18n::tr("estrazione…", "extracting…"));
        stop(app);
        let (p, d) = (pkg.clone(), dir.clone());
        let st = tauri::async_runtime::spawn_blocking(move || Command::new("tar").arg("--zstd").arg("-xf").arg(&p).arg("-C").arg(&d).status())
            .await
            .map_err(|e| e.to_string())?
            .map_err(|e| e.to_string())?;
        let _ = std::fs::remove_file(&pkg);
        if !st.success() {
            return Err(crate::i18n::tr("estrazione di Ollama non riuscita", "extracting Ollama failed").into());
        }
    }
    step(app, "install", "Ollama", crate::i18n::tr("avvio…", "starting…"));
    ensure_running(app).await
}

#[tauri::command]
pub async fn local_start(app: AppHandle) -> Result<String, String> {
    ensure_running(&app).await
}

#[tauri::command]
pub fn local_stop(app: AppHandle) {
    stop(&app);
}

// ---------- stato, modelli installati ----------

#[derive(Serialize, Clone, Default, Debug, PartialEq)]
pub struct Installed {
    pub name: String,
    pub size: u64,
    pub params: String,
    pub quant: String,
    pub family: String,
    /// Caricato in memoria: percentuale su GPU (None = non caricato).
    pub gpu_percent: Option<u32>,
}

#[derive(Serialize)]
pub struct Status {
    pub installed: bool,
    pub running: bool,
    pub version: String,
    pub lfm3_ok: bool,
    pub managed: bool,
    pub binary: String,
    pub base: String,
    pub models: Vec<Installed>,
    pub busy: bool,
}

pub fn parse_tags(tags: &Value, ps: &Value) -> Vec<Installed> {
    let loaded: HashMap<String, u32> = ps
        .get("models")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|m| {
            let size = m.get("size").and_then(Value::as_u64)?.max(1);
            let vram = m.get("size_vram").and_then(Value::as_u64).unwrap_or(0);
            Some((m.get("name").and_then(Value::as_str)?.to_string(), ((vram * 100) / size).min(100) as u32))
        })
        .collect();
    let mut v: Vec<Installed> = tags
        .get("models")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|m| {
            let name = m.get("name").and_then(Value::as_str)?.to_string();
            let d = m.get("details").cloned().unwrap_or(json!({}));
            let s = |k: &str| d.get(k).and_then(Value::as_str).unwrap_or("").to_string();
            Some(Installed {
                gpu_percent: loaded.get(&name).copied(),
                size: m.get("size").and_then(Value::as_u64).unwrap_or(0),
                params: s("parameter_size"),
                quant: s("quantization_level"),
                family: s("family"),
                name,
            })
        })
        .collect();
    v.sort_by(|a, b| a.name.cmp(&b.name));
    v
}

#[tauri::command]
pub async fn local_status(app: AppHandle) -> Status {
    let ver = version(&app).await;
    let mut models = Vec::new();
    if ver.is_some() {
        let h = http(&app);
        let b = base(&app);
        let tags: Value = match h.get(format!("{b}/api/tags")).timeout(Duration::from_secs(5)).send().await {
            Ok(r) => r.json().await.unwrap_or(json!({})),
            Err(_) => json!({}),
        };
        let ps: Value = match h.get(format!("{b}/api/ps")).timeout(Duration::from_secs(5)).send().await {
            Ok(r) => r.json().await.unwrap_or(json!({})),
            Err(_) => json!({}),
        };
        models = parse_tags(&tags, &ps);
    }
    let bin = binary(&app);
    Status {
        installed: bin.is_some() || ver.is_some(),
        running: ver.is_some(),
        lfm3_ok: ver.as_deref().is_some_and(|v| version_at_least(v, LFM3_MIN)),
        version: ver.unwrap_or_default(),
        managed: SERVE.lock().unwrap().is_some(),
        binary: bin.map(|p| p.display().to_string()).unwrap_or_default(),
        base: base(&app),
        models,
        busy: BUSY.load(Ordering::SeqCst),
    }
}

#[tauri::command]
pub async fn local_delete(app: AppHandle, name: String) -> Result<(), String> {
    let r = http(&app).delete(format!("{}/api/delete", base(&app))).json(&json!({ "model": name })).send().await.map_err(|e| e.to_string())?;
    if !r.status().is_success() {
        return Err(r.text().await.unwrap_or_default());
    }
    crate::applog::info("ollama", &format!("modello eliminato: {name}"));
    Ok(())
}

/// Usa il modello con Alter Ego: provider Ollama attivo (e primo, se `primary`), modello scelto,
/// parametri consigliati per LFM3 (temperatura 0.1, top_k 50, repeat penalty 1.1, contesto 32K).
#[tauri::command]
pub fn local_use(app: AppHandle, name: String, primary: bool) -> String {
    let lfm3 = name.to_lowercase().contains("lfm3");
    let vision = ["vl", "vision", "llava", "omni", "gemma3", "qwen2.5vl", "minicpm-v"].iter().any(|k| name.to_lowercase().contains(k));
    update_config(&app, |c| {
        let pc = c.ai.providers.entry("ollama".into()).or_default();
        pc.enabled = true;
        pc.model = name.clone();
        pc.vision = vision;
        if primary {
            c.ai.order.retain(|o| o != "ollama");
            c.ai.order.insert(0, "ollama".into());
        }
        if lfm3 {
            c.ai.temperature = 0.1;
            c.ai.ollama.top_k = 50;
            c.ai.ollama.repeat_penalty = 1.1;
            if c.ai.ollama.num_ctx == 0 {
                c.ai.ollama.num_ctx = 32768;
            }
        }
        c.ai.normalize();
    });
    crate::applog::info("ollama", &format!("modello in uso: {name}{}", if primary { " (principale)" } else { "" }));
    if lfm3 {
        crate::i18n::tr(
            "In uso, con i parametri consigliati da Liquid AI (temperatura 0.1, top_k 50, repeat penalty 1.1) e contesto 32K.",
            "In use, with Liquid AI's recommended settings (temperature 0.1, top_k 50, repeat penalty 1.1) and 32K context.",
        )
        .into()
    } else {
        crate::i18n::tr("In uso.", "In use.").into()
    }
}

// ---------- download (pull) ----------

/// Nome per `ollama pull` da quello che scrive l'utente: nomi di Ollama, hf.co/…, link di Hugging Face
/// (anche a un file .gguf), link di ollama.com. `Err` per gli URL diretti a un .gguf fuori da Hugging Face
/// (quelli si scaricano a parte, vedi `pull_gguf_url`).
pub fn normalize(input: &str) -> Result<String, String> {
    let s = input.trim().trim_end_matches('/');
    if s.is_empty() {
        return Err(crate::i18n::tr("scrivi un nome o un indirizzo", "type a name or an address").into());
    }
    let strip = |s: &str| s.trim_start_matches("https://").trim_start_matches("http://").trim_start_matches("www.").to_string();
    let t = strip(s);
    for host in ["huggingface.co/", "hf.co/"] {
        if let Some(rest) = t.strip_prefix(host) {
            // user/repo[/blob|resolve/main/file.gguf][:quant]
            let parts: Vec<&str> = rest.split('/').collect();
            if parts.len() < 2 {
                return Err(crate::i18n::tr("indirizzo di Hugging Face incompleto (serve utente/modello)", "incomplete Hugging Face address (user/model needed)").into());
            }
            let repo = format!("{}/{}", parts[0], parts[1].split(':').next().unwrap_or(""));
            let quant = parts[1].split_once(':').map(|(_, q)| q.to_string());
            let file = parts.iter().rev().find(|p| p.to_lowercase().ends_with(".gguf")).map(|f| f.to_string());
            let tag = file.or(quant).map(|q| format!(":{q}")).unwrap_or_default();
            return Ok(format!("hf.co/{repo}{tag}"));
        }
    }
    if let Some(rest) = t.strip_prefix("ollama.com/") {
        let rest = rest.strip_prefix("library/").unwrap_or(rest);
        return Ok(rest.to_string());
    }
    if s.starts_with("http://") || s.starts_with("https://") {
        return Err("gguf-url".into());
    }
    let ok = s.chars().all(|c| c.is_ascii_alphanumeric() || "._-:/".contains(c));
    if ok { Ok(s.to_string()) } else { Err(crate::i18n::tr("nome del modello non valido", "invalid model name").into()) }
}

/// Scarica un modello (con avanzamento sull'evento `local-progress`). Restituisce il nome installato.
#[tauri::command]
pub async fn local_pull(app: AppHandle, name: String) -> Result<String, String> {
    let model = match normalize(&name) {
        Ok(m) => m,
        Err(e) if e == "gguf-url" => return pull_gguf_url(&app, name.trim()).await,
        Err(e) => return Err(e),
    };
    ensure_running(&app).await?;
    let cancel = std::sync::Arc::new(AtomicBool::new(false));
    PULLS.lock().unwrap().get_or_insert_with(HashMap::new).insert(model.clone(), cancel.clone());
    let r = pull(&app, &model, &cancel).await;
    PULLS.lock().unwrap().get_or_insert_with(HashMap::new).remove(&model);
    match &r {
        Ok(_) => {
            crate::applog::info("ollama", &format!("scaricato {model}"));
            emit(&app, Progress { task: "pull".into(), name: model.clone(), status: "success".into(), done: true, ..Default::default() });
        }
        Err(e) => {
            crate::applog::warn("ollama", &format!("download di {model}: {e}"));
            emit(&app, Progress { task: "pull".into(), name: model.clone(), error: Some(e.clone()), done: true, ..Default::default() });
        }
    }
    r.map(|_| model)
}

async fn pull(app: &AppHandle, model: &str, cancel: &AtomicBool) -> Result<(), String> {
    let resp = http(app)
        .post(format!("{}/api/pull", base(app)))
        .json(&json!({ "model": model, "stream": true }))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(explain_pull(&resp.text().await.unwrap_or_default()));
    }
    let mut stream = resp.bytes_stream();
    let mut buf = Vec::new();
    let mut last = std::time::Instant::now() - Duration::from_secs(1);
    loop {
        if cancel.load(Ordering::SeqCst) {
            return Err(crate::i18n::tr("download annullato (riprende da dove era rimasto)", "download cancelled (it resumes where it stopped)").into());
        }
        let chunk = match tokio::time::timeout(Duration::from_secs(120), stream.next()).await {
            Err(_) => return Err(crate::i18n::tr("il download si è fermato (nessun dato per 2 minuti)", "the download stopped (no data for 2 minutes)").into()),
            Ok(None) => break,
            Ok(Some(c)) => c.map_err(|e| e.to_string())?,
        };
        buf.extend_from_slice(&chunk);
        while let Some(nl) = buf.iter().position(|b| *b == b'\n') {
            let line: Vec<u8> = buf.drain(..=nl).collect();
            let Ok(v) = serde_json::from_slice::<Value>(&line) else { continue };
            if let Some(e) = v.get("error").and_then(Value::as_str) {
                return Err(explain_pull(e));
            }
            let status = v.get("status").and_then(Value::as_str).unwrap_or("").to_string();
            if status == "success" {
                return Ok(());
            }
            if last.elapsed() > Duration::from_millis(300) {
                last = std::time::Instant::now();
                emit(
                    app,
                    Progress {
                        task: "pull".into(),
                        name: model.into(),
                        status,
                        completed: v.get("completed").and_then(Value::as_u64).unwrap_or(0),
                        total: v.get("total").and_then(Value::as_u64).unwrap_or(0),
                        ..Default::default()
                    },
                );
            }
        }
    }
    Ok(())
}

fn explain_pull(e: &str) -> String {
    let l = e.to_lowercase();
    if l.contains("newer version") || l.contains("requires a newer") || l.contains("412") {
        crate::tr!("{e} (aggiorna Ollama: «Installa / aggiorna Ollama»)", "{e} (update Ollama: “Install / update Ollama”)")
    } else if l.contains("file does not exist") || l.contains("not found") {
        crate::tr!("{e} (nome o quantizzazione inesistente)", "{e} (name or quantization does not exist)")
    } else {
        e.to_string()
    }
}

#[tauri::command]
pub fn local_cancel(name: String) {
    if let Some(f) = PULLS.lock().unwrap().as_ref().and_then(|m| m.get(&name)) {
        f.store(true, Ordering::SeqCst);
    }
}

/// .gguf da un indirizzo qualsiasi: download, SHA-256, `POST /api/blobs`, `POST /api/create`.
async fn pull_gguf_url(app: &AppHandle, url: &str) -> Result<String, String> {
    use sha2::Digest;
    let file = url.split(['?', '#']).next().unwrap_or(url).rsplit('/').next().unwrap_or("model.gguf").to_string();
    if !file.to_lowercase().ends_with(".gguf") {
        return Err(crate::i18n::tr("l'indirizzo deve puntare a un file .gguf (o a un modello di Hugging Face / Ollama)", "the address must point to a .gguf file (or a Hugging Face / Ollama model)").into());
    }
    ensure_running(app).await?;
    let name: String = file.trim_end_matches(".gguf").trim_end_matches(".GGUF").to_lowercase().chars().map(|c| if c.is_ascii_alphanumeric() || c == '.' || c == '-' { c } else { '-' }).collect();
    let dir = managed_dir(app).join("downloads");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let dest = dir.join(&file);
    download(app, url, &dest, "pull", &name).await?;
    step(app, "pull", &name, crate::i18n::tr("controllo SHA-256…", "checking SHA-256…"));
    let d2 = dest.clone();
    let digest = tauri::async_runtime::spawn_blocking(move || -> Result<String, String> {
        let mut f = std::fs::File::open(&d2).map_err(|e| e.to_string())?;
        let mut h = sha2::Sha256::new();
        std::io::copy(&mut f, &mut h).map_err(|e| e.to_string())?;
        Ok(format!("sha256:{:x}", h.finalize()))
    })
    .await
    .map_err(|e| e.to_string())??;
    step(app, "pull", &name, crate::i18n::tr("copia in Ollama…", "copying into Ollama…"));
    let body = reqwest::Body::from(tokio::fs::File::open(&dest).await.map_err(|e| e.to_string())?);
    let r = http(app).post(format!("{}/api/blobs/{digest}", base(app))).body(body).send().await.map_err(|e| e.to_string())?;
    if !r.status().is_success() {
        return Err(r.text().await.unwrap_or_default());
    }
    let r = http(app)
        .post(format!("{}/api/create", base(app)))
        .json(&json!({ "model": name, "files": { file.clone(): digest }, "stream": false }))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    let ok = r.status().is_success();
    let text = r.text().await.unwrap_or_default();
    let _ = std::fs::remove_file(&dest);
    if !ok {
        return Err(text);
    }
    crate::applog::info("ollama", &format!("creato {name} da {url}"));
    emit(app, Progress { task: "pull".into(), name: name.clone(), status: "success".into(), done: true, ..Default::default() });
    Ok(name)
}

// ---------- catalogo ----------

#[derive(Serialize, Clone, Default, Debug, PartialEq)]
pub struct Model {
    /// Nome da scaricare (`lfm3`, `utente/modello`, `hf.co/utente/repo`).
    pub name: String,
    pub description: String,
    /// "tools", "vision", "thinking", "embedding"…
    pub capabilities: Vec<String>,
    /// "1.7b", "8b"…
    pub sizes: Vec<String>,
    pub downloads: String,
    pub updated: String,
    /// "library", "community", "huggingface".
    pub source: String,
}

fn strip_tags(s: &str) -> String {
    let mut out = String::new();
    let mut inside = false;
    for c in s.chars() {
        match c {
            '<' => inside = true,
            '>' => inside = false,
            c if !inside => out.push(c),
            _ => {}
        }
    }
    out.replace("&amp;", "&").replace("&nbsp;", " ").replace("&#39;", "'").replace("&quot;", "\"").split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Pagina https://ollama.com/library: un `<li>` per modello con nome, descrizione, etichette e download.
pub fn parse_library(html: &str) -> Vec<Model> {
    let mut out = Vec::new();
    for block in html.split("<li").skip(1) {
        let block = block.split("</li>").next().unwrap_or("");
        let Some(h) = block.find("href=\"/library/") else { continue };
        let name: String = block[h + 15..].chars().take_while(|c| *c != '"').collect();
        if name.is_empty() || name.contains('/') {
            continue;
        }
        let desc = block.find("<p").and_then(|p| block[p..].find('>').map(|e| p + e + 1)).map(|a| strip_tags(&block[a..a + block[a..].find("</p>").unwrap_or(0)])).unwrap_or_default();
        let mut caps = Vec::new();
        let mut sizes = Vec::new();
        for chip in block.split("rounded-md").skip(1) {
            let txt = chip.split('>').nth(1).map(|t| t.split('<').next().unwrap_or("").trim().to_string()).unwrap_or_default();
            if txt.is_empty() {
                continue;
            }
            let l = txt.to_lowercase();
            if l.chars().next().is_some_and(|c| c.is_ascii_digit()) && (l.ends_with('b') || l.ends_with('m') || l.contains('x')) {
                sizes.push(l);
            } else {
                caps.push(l);
            }
        }
        let num = |label: &str| -> String {
            block.find(label).map(|i| strip_tags(&block[block[..i].rfind("<span >").unwrap_or(i)..i])).unwrap_or_default()
        };
        out.push(Model { name, description: desc, capabilities: caps, sizes, downloads: num("Pulls"), updated: String::new(), source: "library".into() });
    }
    out
}

/// Pagina https://ollama.com/search?q=…: modelli della community (`utente/modello`) e ufficiali.
pub fn parse_search(html: &str) -> Vec<Model> {
    let mut out: Vec<Model> = Vec::new();
    for block in html.split("<li").skip(1) {
        let block = block.split("</li>").next().unwrap_or("");
        let Some(h) = block.find("href=\"/") else { continue };
        let path: String = block[h + 7..].chars().take_while(|c| *c != '"').collect();
        if path.starts_with("public") || !path.contains('/') && !block.contains("<h2") {
            continue;
        }
        let name = path.strip_prefix("library/").unwrap_or(&path).to_string();
        if name.is_empty() || out.iter().any(|m| m.name == name) {
            continue;
        }
        let downloads = block.find(" downloads\"").map(|i| block[..i].rsplit('"').next().unwrap_or("").to_string()).unwrap_or_default();
        let updated = block.find("title=\"Updated ").map(|i| block[i + 15..].chars().take_while(|c| *c != '"').collect()).unwrap_or_default();
        let desc = block.find("<p").and_then(|p| block[p..].find('>').map(|e| p + e + 1)).map(|a| strip_tags(&block[a..a + block[a..].find("</p>").unwrap_or(0)])).unwrap_or_default();
        out.push(Model {
            source: if name.contains('/') { "community".into() } else { "library".into() },
            sizes: size_in_name(&name).map(|s| vec![s]).unwrap_or_default(),
            name,
            description: desc,
            downloads,
            updated,
            ..Default::default()
        });
    }
    out
}

/// API di Hugging Face (`/api/models?filter=gguf`).
pub fn parse_hf(v: &Value) -> Vec<Model> {
    v.as_array()
        .into_iter()
        .flatten()
        .filter_map(|m| {
            let id = m.get("id").and_then(Value::as_str)?.to_string();
            let tags: Vec<String> = m.get("tags").and_then(Value::as_array).into_iter().flatten().filter_map(|t| t.as_str().map(str::to_lowercase)).collect();
            let mut caps = Vec::new();
            if m.get("pipeline_tag").and_then(Value::as_str).is_some_and(|p| p.contains("image")) || tags.iter().any(|t| t.contains("vision")) {
                caps.push("vision".into());
            }
            if tags.iter().any(|t| t.contains("tool") || t.contains("function-calling")) {
                caps.push("tools".into());
            }
            let size = size_in_name(&id).map(|s| vec![s]).unwrap_or_default();
            Some(Model {
                name: format!("hf.co/{id}"),
                description: m.get("pipeline_tag").and_then(Value::as_str).unwrap_or("").to_string(),
                capabilities: caps,
                sizes: size,
                downloads: m.get("downloads").and_then(Value::as_u64).map(|d| d.to_string()).unwrap_or_default(),
                updated: m.get("lastModified").and_then(Value::as_str).unwrap_or("").chars().take(10).collect(),
                source: "huggingface".into(),
            })
        })
        .collect()
}

/// "Qwen3-8B-abliterated-GGUF" → "8b".
fn size_in_name(name: &str) -> Option<String> {
    let l = name.to_lowercase();
    l.split(|c: char| !(c.is_ascii_alphanumeric() || c == '.'))
        .find(|p| p.ends_with('b') && p.len() > 1 && p[..p.len() - 1].parse::<f64>().is_ok())
        .map(str::to_string)
}

/// Miliardi di parametri da "1.7b", "70b", "135m".
fn billions(s: &str) -> Option<f64> {
    let l = s.to_lowercase();
    if let Some(n) = l.strip_suffix('b') {
        return n.parse().ok();
    }
    l.strip_suffix('m').and_then(|n| n.parse::<f64>().ok()).map(|n| n / 1000.0)
}

/// Filtri del catalogo: parole (tutte presenti in nome o descrizione), capacità, dimensione massima.
pub fn filter(models: Vec<Model>, words: &[String], caps: &[String], max_b: f64) -> Vec<Model> {
    models
        .into_iter()
        .filter(|m| {
            let hay = format!("{} {}", m.name, m.description).to_lowercase();
            words.iter().all(|w| hay.contains(&w.to_lowercase()))
                && caps.iter().all(|c| m.capabilities.iter().any(|x| x == c))
                && (max_b <= 0.0 || m.sizes.is_empty() || m.sizes.iter().filter_map(|s| billions(s)).any(|b| b <= max_b))
        })
        .collect()
}

/// Cerca modelli. `source`: "library", "community", "huggingface". `chips`: parole da includere
/// (abliterated, uncensored…). `caps`: capacità richieste. `max_b`: miliardi di parametri massimi (0 = tutti).
#[tauri::command]
pub async fn local_search(app: AppHandle, source: String, query: String, chips: Vec<String>, caps: Vec<String>, max_b: f64) -> Result<Vec<Model>, String> {
    let http = app.state::<AppState>().ai.http.clone();
    let q = query.trim().to_string();
    let mut words: Vec<String> = q.split_whitespace().map(str::to_string).collect();
    words.extend(chips.iter().cloned());
    let term = if q.is_empty() { chips.first().cloned().unwrap_or_default() } else { q.clone() };
    let get = |url: String| {
        let http = http.clone();
        async move {
            http.get(url).timeout(Duration::from_secs(20)).send().await.and_then(|r| r.error_for_status()).map_err(|e| crate::tr!("catalogo non raggiungibile: {e}", "catalog not reachable: {e}"))
        }
    };
    let models = match source.as_str() {
        "huggingface" => {
            let url = format!(
                "https://huggingface.co/api/models?filter=gguf&sort=downloads&direction=-1&limit=60&search={}",
                urlencode(&term)
            );
            parse_hf(&get(url).await?.json::<Value>().await.map_err(|e| e.to_string())?)
        }
        "community" => {
            let url = format!("https://ollama.com/search?q={}", urlencode(&term));
            parse_search(&get(url).await?.text().await.map_err(|e| e.to_string())?)
        }
        _ => {
            let cached = LIBRARY.lock().unwrap().as_ref().filter(|(t, _)| t.elapsed() < Duration::from_secs(3600)).map(|(_, m)| m.clone());
            match cached {
                Some(m) => m,
                None => {
                    let m = parse_library(&get("https://ollama.com/library".into()).await?.text().await.map_err(|e| e.to_string())?);
                    *LIBRARY.lock().unwrap() = Some((std::time::Instant::now(), m.clone()));
                    m
                }
            }
        }
    };
    // per Hugging Face la ricerca è già fatta dall'API sul primo termine
    let words = if source == "huggingface" && q.is_empty() { chips[chips.len().min(1)..].to_vec() } else { words };
    Ok(filter(models, &words, &caps, max_b).into_iter().take(80).collect())
}

fn urlencode(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => (b as char).to_string(),
            b' ' => "+".into(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

/// File GGUF di un repository di Hugging Face (per scegliere la quantizzazione).
#[tauri::command]
pub async fn local_hf_files(app: AppHandle, repo: String) -> Result<Vec<(String, u64)>, String> {
    let repo = repo.trim().trim_start_matches("hf.co/").trim_start_matches("huggingface.co/").to_string();
    if repo.split('/').count() != 2 {
        return Err("repo non valido".into());
    }
    let http = app.state::<AppState>().ai.http.clone();
    let v: Value = http
        .get(format!("https://huggingface.co/api/models/{repo}/tree/main"))
        .timeout(Duration::from_secs(20))
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;
    Ok(v.as_array()
        .into_iter()
        .flatten()
        .filter_map(|f| {
            let p = f.get("path").and_then(Value::as_str)?;
            (p.to_lowercase().ends_with(".gguf") && !p.to_lowercase().starts_with("mmproj")).then(|| (p.to_string(), f.get("size").and_then(Value::as_u64).unwrap_or(0)))
        })
        .collect())
}

// ---------- LFM3 ----------

/// Varianti di LFM3 proposte (nome per Ollama, descrizione, GB circa). Fonte: ollama.com/library/lfm3
/// e huggingface.co/LiquidAI (collezione LFM3).
pub const LFM3: &[(&str, &str, f64)] = &[
    ("lfm3", "LFM3 1.7B, Q4 (predefinito)", 1.1),
    ("lfm3:1.7b-qad-q4_0", "LFM3 1.7B, Q4_0 QAD (addestrato per la quantizzazione)", 1.05),
    ("lfm3:1.7b-q8_0", "LFM3 1.7B, Q8_0 (più preciso)", 1.85),
    ("hf.co/LiquidAI/LFM3-VL-2B-GGUF:Q4_K_M", "LFM3-VL 2B, Q4_K_M (vede gli screenshot, sperimentale)", 1.3),
];

/// Installa e usa LFM3: Ollama (se manca, con conferma), versione minima, download, configurazione.
#[tauri::command]
pub async fn local_lfm3(app: AppHandle, variant: String, primary: bool) -> Result<String, String> {
    let variant = if LFM3.iter().any(|(n, _, _)| *n == variant) { variant } else { "lfm3".into() };
    if binary(&app).is_none() && version(&app).await.is_none() {
        local_install(app.clone()).await?;
    }
    let v = ensure_running(&app).await?;
    if !variant.starts_with("hf.co/") && !version_at_least(&v, LFM3_MIN) {
        return Err(crate::tr!(
            "LFM3 richiede Ollama {LFM3_MIN} o più recente (hai la {v}): usa «Installa / aggiorna Ollama»",
            "LFM3 needs Ollama {LFM3_MIN} or newer (you have {v}): use “Install / update Ollama”"
        ));
    }
    let name = local_pull(app.clone(), variant).await?;
    Ok(local_use(app, name, primary))
}

// ---------- GPU ----------

#[derive(Serialize, Clone, Default, Debug, PartialEq)]
pub struct Gpu {
    pub name: String,
    pub vendor: String,
    /// MB di memoria video (0 = sconosciuta).
    pub vram_mb: u64,
    pub integrated: bool,
}

/// Dispositivo che Ollama ha trovato all'avvio (righe `msg="inference compute"` del suo log).
#[derive(Serialize, Clone, Default, Debug, PartialEq)]
pub struct OllamaDevice {
    pub id: String,
    pub library: String,
    pub description: String,
    pub kind: String,
    pub total: String,
}

/// `chiave=valore` o `chiave="valore con spazi"` di una riga di log di Ollama.
fn kv(line: &str) -> HashMap<String, String> {
    let mut m = HashMap::new();
    let mut rest = line;
    while let Some(eq) = rest.find('=') {
        let key = rest[..eq].rsplit(' ').next().unwrap_or("").to_string();
        let after = &rest[eq + 1..];
        let (val, next) = if let Some(stripped) = after.strip_prefix('"') {
            let end = stripped.find('"').unwrap_or(stripped.len());
            (stripped[..end].to_string(), &stripped[(end + 1).min(stripped.len())..])
        } else {
            let end = after.find(' ').unwrap_or(after.len());
            (after[..end].to_string(), &after[end..])
        };
        if !key.is_empty() {
            m.insert(key, val);
        }
        rest = next;
    }
    m
}

/// Ultimo gruppo di dispositivi nel log (l'ultimo avvio del server).
pub fn parse_ollama_log(log: &str) -> Vec<OllamaDevice> {
    let mut groups: Vec<Vec<OllamaDevice>> = Vec::new();
    let mut prev = false;
    for line in log.lines() {
        if line.contains("msg=\"inference compute\"") {
            let m = kv(line);
            let d = OllamaDevice {
                id: m.get("id").cloned().unwrap_or_default(),
                library: m.get("library").cloned().unwrap_or_default(),
                description: m.get("description").or_else(|| m.get("name")).cloned().unwrap_or_default(),
                kind: m.get("type").cloned().unwrap_or_default(),
                total: m.get("total").cloned().unwrap_or_default(),
            };
            if !prev {
                groups.push(Vec::new());
            }
            groups.last_mut().unwrap().push(d);
            prev = true;
        } else if !line.trim().is_empty() {
            prev = false;
        }
    }
    groups.pop().unwrap_or_default()
}

fn ollama_log_path(app: &AppHandle) -> Option<PathBuf> {
    if SERVE.lock().unwrap().is_some() {
        return Some(serve_log(app));
    }
    let mut c = Vec::new();
    if let Some(l) = std::env::var_os("LOCALAPPDATA") {
        c.push(PathBuf::from(l).join("Ollama").join("server.log"));
    }
    c.push(serve_log(app));
    if let Some(h) = std::env::var_os("HOME") {
        c.push(PathBuf::from(h).join(".ollama").join("logs").join("server.log"));
    }
    c.into_iter().find(|p| p.is_file())
}

/// Schede video del PC (nome, produttore, memoria, integrata o no).
pub fn list_gpus() -> Vec<Gpu> {
    #[cfg(windows)]
    {
        // DriverDesc + HardwareInformation.qwMemorySize dal registro (AdapterRAM si ferma a 4 GB)
        let ps = r#"Get-ChildItem 'HKLM:\SYSTEM\CurrentControlSet\Control\Class\{4d36e968-e325-11ce-bfc1-08002be10318}' -ErrorAction SilentlyContinue | ForEach-Object { $p = Get-ItemProperty $_.PSPath -ErrorAction SilentlyContinue; if ($p.DriverDesc) { "$($p.DriverDesc)`t$($p.'HardwareInformation.qwMemorySize')`t$($p.ProviderName)" } }"#;
        let out = Command::new("powershell")
            .args(["-NoProfile", "-NonInteractive", "-Command", ps])
            .stdin(Stdio::null())
            .output_hidden()
            .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
            .unwrap_or_default();
        let mut v: Vec<Gpu> = out
            .lines()
            .filter_map(|l| {
                let p: Vec<&str> = l.split('\t').collect();
                let name = p.first()?.trim().to_string();
                if name.is_empty() || name.to_lowercase().contains("basic display") || name.to_lowercase().contains("remote") {
                    return None;
                }
                let vram = p.get(1).and_then(|s| s.trim().parse::<u64>().ok()).unwrap_or(0) / (1024 * 1024);
                Some(classify(name, vram))
            })
            .collect();
        v.dedup();
        v
    }
    #[cfg(not(windows))]
    {
        let mut v = Vec::new();
        if let Ok(rd) = std::fs::read_dir("/sys/class/drm") {
            for e in rd.flatten() {
                let n = e.file_name().to_string_lossy().to_string();
                if !n.starts_with("card") || n.contains('-') {
                    continue;
                }
                let dev = e.path().join("device");
                let vendor = std::fs::read_to_string(dev.join("vendor")).unwrap_or_default();
                let vram = std::fs::read_to_string(dev.join("mem_info_vram_total")).ok().and_then(|s| s.trim().parse::<u64>().ok()).unwrap_or(0) / (1024 * 1024);
                let slot = std::fs::read_link(&dev).ok().and_then(|p| p.file_name().map(|f| f.to_string_lossy().to_string())).unwrap_or_default();
                let name = Command::new("lspci")
                    .args(["-s", &slot, "-mm"])
                    .output()
                    .ok()
                    .map(|o| {
                        let t = String::from_utf8_lossy(&o.stdout).to_string();
                        let q: Vec<&str> = t.split('"').collect();
                        format!("{} {}", q.get(3).copied().unwrap_or(""), q.get(5).copied().unwrap_or("")).trim().to_string()
                    })
                    .filter(|s| !s.is_empty())
                    .unwrap_or_else(|| match vendor.trim() {
                        "0x1002" => "AMD Radeon".into(),
                        "0x8086" => "Intel Graphics".into(),
                        "0x10de" => "NVIDIA".into(),
                        other => format!("GPU {other}"),
                    });
                v.push(classify(name, vram));
            }
        }
        v
    }
}

#[cfg(windows)]
trait HiddenOutput {
    fn output_hidden(&mut self) -> std::io::Result<std::process::Output>;
}
#[cfg(windows)]
impl HiddenOutput for Command {
    fn output_hidden(&mut self) -> std::io::Result<std::process::Output> {
        use std::os::windows::process::CommandExt;
        self.creation_flags(0x0800_0000).output()
    }
}

pub fn classify(name: String, vram_mb: u64) -> Gpu {
    let l = name.to_lowercase();
    let vendor = if l.contains("amd") || l.contains("radeon") || l.contains("advanced micro") {
        "AMD"
    } else if l.contains("intel") {
        "Intel"
    } else if l.contains("nvidia") || l.contains("geforce") || l.contains("rtx") {
        "NVIDIA"
    } else {
        "?"
    };
    // integrate: Intel UHD/Iris/Arc integrata, AMD "Radeon(TM) Graphics"/Vega senza numero di serie RX
    let integrated = (vendor == "Intel" && !l.contains("arc a") && !l.contains("arc b"))
        || (vendor == "AMD" && !l.contains("rx ") && !l.contains("pro w") && !l.contains("instinct") && (l.contains("graphics") || l.contains("vega")));
    Gpu { name, vendor: vendor.into(), vram_mb, integrated }
}

/// Consiglio per la scheda dedicata (testo per l'utente) e backend suggerito.
pub fn advice(gpus: &[Gpu]) -> (String, &'static str) {
    let amd = gpus.iter().find(|g| g.vendor == "AMD" && !g.integrated);
    let igpu = gpus.iter().any(|g| g.integrated);
    match amd {
        Some(g) => {
            let l = g.name.to_lowercase();
            let rx6000 = l.contains("rx 6");
            let mut s = if cfg!(windows) && (rx6000 || !l.contains("rx 7")) {
                crate::tr!(
                    "{}: su Windows ROCm supporta solo le RX 7000, quindi Ollama usa Vulkan (veloce, nessuna installazione in più).",
                    "{}: on Windows ROCm only supports the RX 7000 series, so Ollama uses Vulkan (fast, nothing else to install).",
                    g.name
                )
            } else if rx6000 && !l.contains("6800") && !l.contains("69") {
                crate::tr!(
                    "{}: Vulkan consigliato; con ROCm su Linux serve HSA_OVERRIDE_GFX_VERSION=10.3.0.",
                    "{}: Vulkan recommended; with ROCm on Linux HSA_OVERRIDE_GFX_VERSION=10.3.0 is needed.",
                    g.name
                )
            } else {
                crate::tr!("{}: supportata (ROCm o Vulkan).", "{}: supported (ROCm or Vulkan).", g.name)
            };
            if igpu {
                s.push_str(crate::i18n::tr(
                    " C'è anche una grafica integrata: scegli la scheda dedicata qui sotto, così Ollama non usa l'iGPU (più lenta e meno stabile).",
                    " There is also an integrated GPU: pick the dedicated card below, so Ollama doesn't use the iGPU (slower and less stable).",
                ));
            }
            (s, "vulkan")
        }
        None => (String::new(), "auto"),
    }
}

#[derive(Serialize)]
pub struct GpuInfo {
    pub gpus: Vec<Gpu>,
    pub devices: Vec<OllamaDevice>,
    pub advice: String,
    pub suggested_backend: String,
    pub log: String,
    pub settings: LocalConfig,
    /// Su Windows con l'app di Ollama le impostazioni vanno nelle variabili dell'utente (riavvio di Ollama).
    pub user_env: bool,
}

#[tauri::command]
pub async fn local_gpu(app: AppHandle) -> GpuInfo {
    let gpus = tauri::async_runtime::spawn_blocking(list_gpus).await.unwrap_or_default();
    let log_path = ollama_log_path(&app);
    // il log può contenere byte non UTF-8 (barre di avanzamento): lettura tollerante
    let devices = log_path.as_ref().and_then(|p| std::fs::read(p).ok()).map(|b| parse_ollama_log(&String::from_utf8_lossy(&b))).unwrap_or_default();
    let (advice, backend) = advice(&gpus);
    GpuInfo {
        gpus,
        devices,
        advice,
        suggested_backend: backend.into(),
        log: log_path.map(|p| p.display().to_string()).unwrap_or_default(),
        settings: app.state::<AppState>().cfg.lock().unwrap().local.clone(),
        user_env: managed_env_is_user(&app),
    }
}

/// Salva le impostazioni GPU e riavvia Ollama per applicarle (con conferma se va riavviato).
#[tauri::command]
pub async fn local_gpu_apply(app: AppHandle, mut settings: LocalConfig, restart: bool) -> Result<String, String> {
    settings.normalize();
    update_config(&app, |c| c.local = settings.clone());
    if !restart {
        return Ok(crate::i18n::tr("Salvato.", "Saved.").into());
    }
    if !crate::actions::confirm_always(&app, crate::i18n::tr("Riavviare Ollama con le nuove impostazioni GPU?", "Restart Ollama with the new GPU settings?").into()).await {
        return Err(crate::i18n::tr("annullato", "cancelled").into());
    }
    if managed_env_is_user(&app) {
        #[cfg(windows)]
        {
            // variabili dell'utente (docs.ollama.com/faq), poi riavvio dell'app di Ollama
            let env = server_env(&settings);
            let mut script = String::new();
            for n in ENV_NAMES {
                match env.iter().find(|(k, _)| k == n) {
                    Some((_, v)) => script.push_str(&format!("[Environment]::SetEnvironmentVariable('{n}','{v}','User');")),
                    None => script.push_str(&format!("[Environment]::SetEnvironmentVariable('{n}',$null,'User');")),
                }
            }
            script.push_str("Stop-Process -Name 'ollama app','ollama' -Force -ErrorAction SilentlyContinue");
            let _ = Command::new("powershell").args(["-NoProfile", "-NonInteractive", "-Command", &script]).stdin(Stdio::null()).output_hidden();
            tokio::time::sleep(Duration::from_secs(2)).await;
            if let Some(a) = windows_app() {
                // l'app eredita le variabili appena scritte solo se le passiamo noi (il nostro processo ha quelle vecchie)
                let mut c = Command::new(a);
                for n in ENV_NAMES {
                    c.env_remove(n);
                }
                for (k, v) in &env {
                    c.env(k, v);
                }
                c.spawn().map_err(|e| e.to_string())?;
            }
        }
    } else {
        stop(&app);
        tokio::time::sleep(Duration::from_millis(800)).await;
        if version(&app).await.is_some() {
            return Err(crate::i18n::tr(
                "Ollama è avviato come servizio di sistema: per cambiare la GPU usa `sudo systemctl edit ollama` (variabili Environment=…) e riavvialo",
                "Ollama runs as a system service: to change the GPU use `sudo systemctl edit ollama` (Environment=… variables) and restart it",
            )
            .into());
        }
    }
    let v = ensure_running(&app).await?;
    crate::applog::info("ollama", &format!("riavviato con backend {} (vk {:?})", settings.backend, settings.vk_device));
    Ok(crate::tr!("Ollama {v} riavviato con le nuove impostazioni.", "Ollama {v} restarted with the new settings."))
}

#[tauri::command]
pub fn local_settings(app: AppHandle, autostart: bool) {
    update_config(&app, |c| c.local.autostart = autostart);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Pagine vere salvate a mano: AE_LIB_HTML, AE_SEARCH_HTML, AE_HF_JSON (percorsi di file).
    #[test]
    fn real_pages() {
        if let Ok(p) = std::env::var("AE_LIB_HTML") {
            let m = parse_library(&std::fs::read_to_string(p).unwrap());
            let lfm3 = m.iter().find(|x| x.name == "lfm3").expect("lfm3 nella libreria");
            println!("library: {} modelli; lfm3 = {:?}", m.len(), lfm3);
            assert!(m.len() > 50 && lfm3.capabilities.contains(&"tools".to_string()));
        }
        if let Ok(p) = std::env::var("AE_SEARCH_HTML") {
            let m = parse_search(&std::fs::read_to_string(p).unwrap());
            println!("search: {} risultati, primi: {:?}", m.len(), m.iter().take(3).collect::<Vec<_>>());
            assert!(m.len() > 5 && m.iter().all(|x| !x.name.starts_with("public")));
        }
        if let Ok(p) = std::env::var("AE_OLLAMA_LOG") {
            let d = parse_ollama_log(&String::from_utf8_lossy(&std::fs::read(p).unwrap()));
            println!("ollama log: {d:?}");
            assert!(!d.is_empty());
        }
        if let Ok(p) = std::env::var("AE_HF_JSON") {
            let m = parse_hf(&serde_json::from_str(&std::fs::read_to_string(p).unwrap()).unwrap());
            println!("hf: {} risultati, primi: {:?}", m.len(), m.iter().take(2).collect::<Vec<_>>());
            assert!(!m.is_empty());
        }
    }

    #[test]
    fn versions() {
        assert!(version_at_least("0.40.12", LFM3_MIN));
        assert!(version_at_least("0.41.0", LFM3_MIN));
        assert!(version_at_least("1.0.0-rc1", LFM3_MIN));
        assert!(!version_at_least("0.40.11", LFM3_MIN));
        assert!(!version_at_least("0.12.6", LFM3_MIN));
    }

    #[test]
    fn names_and_urls() {
        assert_eq!(normalize("lfm3").unwrap(), "lfm3");
        assert_eq!(normalize("https://huggingface.co/LiquidAI/LFM3-1.7B-GGUF").unwrap(), "hf.co/LiquidAI/LFM3-1.7B-GGUF");
        assert_eq!(
            normalize("https://huggingface.co/LiquidAI/LFM3-1.7B-GGUF/blob/main/LFM3-1.7B-Q8_0.gguf").unwrap(),
            "hf.co/LiquidAI/LFM3-1.7B-GGUF:LFM3-1.7B-Q8_0.gguf"
        );
        assert_eq!(normalize("hf.co/bartowski/Llama-3.2-3B-Instruct-GGUF:IQ3_M").unwrap(), "hf.co/bartowski/Llama-3.2-3B-Instruct-GGUF:IQ3_M");
        assert_eq!(normalize("https://ollama.com/library/lfm3:1.7b-q8_0").unwrap(), "lfm3:1.7b-q8_0");
        assert_eq!(normalize("https://ollama.com/orcarouter/LFM3-1.7B-abliterated").unwrap(), "orcarouter/LFM3-1.7B-abliterated");
        assert_eq!(normalize("https://example.com/m.gguf").unwrap_err(), "gguf-url");
        assert!(normalize("rm -rf /").is_err());
    }

    #[test]
    fn library_and_search_pages() {
        let lib = r#"<li  class="flex"><a href="/library/lfm3" class="group"><div title="lfm3"><h2><div><span class="x">lfm3</span></div></h2>
<p class="max-w-lg">LFM3-1.7B is a tri-hybrid model for local agents.</p></div><div><div class="flex">
<span  class="inline-flex items-center rounded-md bg-indigo-50">tools</span><span  class="inline-flex rounded-md">thinking</span>
<span  class="inline-flex items-center rounded-md bg-[#ddf4ff]">1.7b</span></div><p><span class="flex items-center"><span >73.2K</span>
<span class="hidden sm:flex">&nbsp;Pulls</span></span></p></div></a></li>"#;
        let m = parse_library(lib);
        assert_eq!(m.len(), 1);
        assert_eq!(m[0].name, "lfm3");
        assert_eq!(m[0].capabilities, ["tools", "thinking"]);
        assert_eq!(m[0].sizes, ["1.7b"]);
        assert_eq!(m[0].downloads, "73.2K");
        assert!(m[0].description.starts_with("LFM3-1.7B"));
        let s = r#"<li class="border-t"><a href="/orcarouter/LFM3-1.7B-abliterated" class="group"><div><h2 title="x"><span >orcarouter/LFM3-1.7B-abliterated</span></h2>
<span class="flex-none" title="Updated November 11, 2026">12h</span></div><div><span class="inline-flex" title="34 downloads"><span >34</span></span></div></a></li>"#;
        let r = parse_search(s);
        assert_eq!(r[0].name, "orcarouter/LFM3-1.7B-abliterated");
        assert_eq!((r[0].downloads.as_str(), r[0].updated.as_str(), r[0].source.as_str()), ("34", "November 11, 2026", "community"));
    }

    #[test]
    fn hf_and_filters() {
        let v = json!([{ "id": "mlabonne/Qwen3-8B-abliterated-GGUF", "downloads": 1200, "tags": ["gguf"], "pipeline_tag": "text-generation", "lastModified": "2026-10-01T00:00:00Z" },
                       { "id": "x/Big-70B-GGUF", "downloads": 5, "tags": ["gguf"], "pipeline_tag": "image-text-to-text" }]);
        let m = parse_hf(&v);
        assert_eq!(m[0].name, "hf.co/mlabonne/Qwen3-8B-abliterated-GGUF");
        assert_eq!(m[0].sizes, ["8b"]);
        assert_eq!(m[1].capabilities, ["vision"]);
        assert_eq!(filter(m.clone(), &["abliterated".into()], &[], 0.0).len(), 1);
        assert_eq!(filter(m.clone(), &[], &[], 10.0).len(), 1);
        assert_eq!(filter(m, &[], &["vision".into()], 0.0)[0].name, "hf.co/x/Big-70B-GGUF");
    }

    #[test]
    fn installed_models() {
        let tags = json!({"models": [{"name": "lfm3:latest", "size": 1100000000u64, "details": {"parameter_size": "1.7B", "quantization_level": "Q4_K_M", "family": "lfm3"}}]});
        let ps = json!({"models": [{"name": "lfm3:latest", "size": 2000, "size_vram": 1500}]});
        let v = parse_tags(&tags, &ps);
        assert_eq!(v[0].gpu_percent, Some(75));
        assert_eq!(v[0].quant, "Q4_K_M");
    }

    #[test]
    fn gpu_log_and_advice() {
        let log = r#"time=2026-11-01T10:00:00 level=INFO source=types.go:42 msg="inference compute" id=0 library=Vulkan name=Vulkan0 description="Intel(R) UHD Graphics 770" type=iGPU total="16.0 GiB" available="8.0 GiB"
time=2026-11-01T10:00:00 level=INFO source=types.go:42 msg="inference compute" id=1 library=Vulkan name=Vulkan1 description="AMD Radeon RX 6600" type=discrete total="8.0 GiB" available="7.6 GiB"
time=2026-11-01T10:00:01 level=INFO msg="Listening on 127.0.0.1:11434"
time=2026-11-02T10:00:00 level=INFO source=types.go:42 msg="inference compute" id=1 library=Vulkan name=Vulkan1 description="AMD Radeon RX 6600" type=discrete total="8.0 GiB" available="7.6 GiB"
"#;
        let d = parse_ollama_log(log);
        assert_eq!(d.len(), 1); // solo l'ultimo avvio
        assert_eq!((d[0].id.as_str(), d[0].library.as_str(), d[0].description.as_str(), d[0].total.as_str()), ("1", "Vulkan", "AMD Radeon RX 6600", "8.0 GiB"));
        let gpus = vec![classify("Intel(R) UHD Graphics 770".into(), 0), classify("AMD Radeon RX 6600".into(), 8192)];
        assert!(gpus[0].integrated && !gpus[1].integrated);
        let (a, b) = advice(&gpus);
        assert_eq!(b, "vulkan");
        assert!(a.contains("RX 6600") && a.contains("integrata"), "{a}");
        assert!(classify("AMD Radeon(TM) Graphics".into(), 512).integrated);
    }

    #[test]
    fn env_for_backends() {
        let mut c = LocalConfig { backend: "vulkan".into(), vk_device: "1".into(), ..Default::default() };
        let e = server_env(&c);
        assert!(e.contains(&("GGML_VK_VISIBLE_DEVICES", "1".into())) && e.contains(&("OLLAMA_VULKAN", "1".into())));
        assert!(e.contains(&("OLLAMA_KV_CACHE_TYPE", "q8_0".into())) && e.contains(&("OLLAMA_FLASH_ATTENTION", "1".into())));
        c.backend = "cpu".into();
        assert!(server_env(&c).contains(&("GGML_VK_VISIBLE_DEVICES", "-1".into())));
        assert!(!server_env(&c).contains(&("GGML_VK_VISIBLE_DEVICES", "1".into())));
    }
}
