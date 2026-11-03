//! Kokoro-82M locale: installazione (script PowerShell in una console visibile) e
//! avvio/arresto del server HTTP (`tools/kokoro/server.py`) insieme ad Alter Ego.
//! In alternativa si può usare un server già esistente (es. Kokoro-FastAPI) indicandone l'URL
//! e spegnendo "avvia insieme ad Alter Ego".

use serde::Serialize;
use std::{
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::Mutex,
    time::{Duration, Instant},
};

const SERVER_PY: &str = include_str!("../../../tools/kokoro/server.py");
const SETUP_PS1: &str = include_str!("../../../tools/kokoro/setup.ps1");

pub struct Kokoro {
    dir: PathBuf,
    child: Mutex<Option<Child>>,
}

#[derive(Serialize, Clone, Debug)]
pub struct KokoroStatus {
    pub installed: bool,
    /// Processo del server avviato da Alter Ego e ancora vivo.
    pub running: bool,
    /// Il server risponde e il modello è caricato.
    pub ready: bool,
    pub message: String,
    pub dir: String,
}

/// `http://127.0.0.1:8880/v1` → (`http://127.0.0.1:8880`, 8880)
pub fn root_and_port(url: &str) -> Result<(String, u16), String> {
    let u = reqwest::Url::parse(url.trim()).map_err(|_| format!("URL non valido: {url}"))?;
    let port = u.port_or_known_default().ok_or("porta mancante")?;
    Ok((format!("{}://{}:{}", u.scheme(), u.host_str().unwrap_or("127.0.0.1"), port), port))
}

fn is_local(url: &str) -> bool {
    crate::providers::is_loopback(url)
}

impl Kokoro {
    pub fn new(data_dir: &Path) -> Self {
        Self { dir: data_dir.join("kokoro"), child: Mutex::new(None) }
    }

    fn python(&self) -> PathBuf {
        if cfg!(windows) {
            self.dir.join("venv").join("Scripts").join("python.exe")
        } else {
            self.dir.join("venv").join("bin").join("python")
        }
    }

    pub fn installed(&self) -> bool {
        self.python().exists() && self.dir.join("installed.ok").exists()
    }

    /// Scrive (o aggiorna) server e script di installazione nella cartella dati.
    fn write_files(&self) -> Result<(), String> {
        std::fs::create_dir_all(&self.dir).map_err(|e| e.to_string())?;
        std::fs::write(self.dir.join("server.py"), SERVER_PY).map_err(|e| e.to_string())?;
        // BOM UTF-8: PowerShell 5 altrimenti legge lo script come ANSI
        let mut ps = vec![0xEF, 0xBB, 0xBF];
        ps.extend_from_slice(SETUP_PS1.as_bytes());
        std::fs::write(self.dir.join("setup.ps1"), ps).map_err(|e| e.to_string())
    }

    /// Apre una console con l'installazione (Python, PyTorch, kokoro, download del modello).
    pub fn install(&self, cuda: bool) -> Result<(), String> {
        self.stop();
        self.write_files()?;
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NEW_CONSOLE: u32 = 0x0000_0010;
            let mut cmd = Command::new("powershell.exe");
            cmd.args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-File"]).arg(self.dir.join("setup.ps1"));
            cmd.arg("-Dir").arg(&self.dir);
            if cuda {
                cmd.arg("-Cuda");
            }
            cmd.creation_flags(CREATE_NEW_CONSOLE).spawn().map(|_| ()).map_err(|e| format!("impossibile avviare l'installazione: {e}"))
        }
        #[cfg(not(windows))]
        {
            let _ = cuda;
            Err(format!(
                "installazione automatica solo su Windows: crea un venv in {} con `pip install kokoro` e un file installed.ok",
                self.dir.display()
            ))
        }
    }

    pub fn running(&self) -> bool {
        let mut c = self.child.lock().unwrap();
        match c.as_mut().map(|ch| ch.try_wait()) {
            Some(Ok(None)) => true,
            Some(_) => {
                *c = None;
                false
            }
            None => false,
        }
    }

    /// Avvia il server (se installato e non già avviato). Solo per URL locali.
    pub fn start(&self, url: &str) -> Result<(), String> {
        if self.running() {
            return Ok(());
        }
        if !is_local(url) {
            return Err("l'URL di Kokoro non è locale: il server va avviato a parte".into());
        }
        if !self.installed() {
            return Err("Kokoro non è installato (Impostazioni → Voce → Installa Kokoro)".into());
        }
        self.write_files()?;
        let (_, port) = root_and_port(url)?;
        let log = std::fs::File::create(self.dir.join("server.log")).map_err(|e| e.to_string())?;
        let mut cmd = Command::new(self.python());
        cmd.arg(self.dir.join("server.py"))
            .args(["--host", "127.0.0.1", "--port", &port.to_string(), "--exit-with-parent"])
            .current_dir(&self.dir)
            // pipe aperta finché Alter Ego vive: se si chiude (anche per un crash) il server esce
            .stdin(Stdio::piped())
            .stdout(log.try_clone().map_err(|e| e.to_string())?)
            .stderr(log)
            // niente richieste a Hugging Face a ogni avvio se il modello è già in cache
            .env("HF_HUB_DISABLE_TELEMETRY", "1")
            .env("PYTHONIOENCODING", "utf-8");
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            cmd.creation_flags(CREATE_NO_WINDOW);
        }
        let child = cmd.spawn().map_err(|e| format!("avvio del server Kokoro non riuscito: {e}"))?;
        *self.child.lock().unwrap() = Some(child);
        Ok(())
    }

    pub fn stop(&self) {
        if let Some(mut c) = self.child.lock().unwrap().take() {
            let _ = c.kill();
            let _ = c.wait();
        }
    }

    fn last_log_line(&self) -> Option<String> {
        let text = std::fs::read_to_string(self.dir.join("server.log")).ok()?;
        text.lines().rev().find(|l| !l.trim().is_empty()).map(|l| l.chars().take(200).collect())
    }

    pub async fn status(&self, http: &reqwest::Client, url: &str) -> KokoroStatus {
        let installed = self.installed();
        let running = self.running();
        let (ready, message) = match probe(http, url).await {
            Ok(()) => (true, "pronto".to_string()),
            // server avviato ma non pronto: l'ultima riga del suo log dice perché (caricamento, errore…)
            Err(e) if running => (false, match self.last_log_line() {
                Some(l) => format!("{e} — log: {l}"),
                None => e,
            }),
            Err(e) => (false, e),
        };
        KokoroStatus { installed, running, ready, message, dir: self.dir.display().to_string() }
    }

    /// Attende che il server sia pronto (avviandolo se serve e se consentito).
    pub async fn ensure_ready(&self, http: &reqwest::Client, url: &str, autostart: bool) -> Result<(), String> {
        if probe(http, url).await.is_ok() {
            return Ok(());
        }
        if autostart && is_local(url) && self.installed() {
            self.start(url)?;
        }
        if !self.running() {
            return probe(http, url).await;
        }
        // il primo caricamento del modello può richiedere qualche decina di secondi
        let t0 = Instant::now();
        loop {
            tokio::time::sleep(Duration::from_millis(500)).await;
            match probe(http, url).await {
                Ok(()) => return Ok(()),
                Err(e) if t0.elapsed() > Duration::from_secs(90) || !self.running() => {
                    return Err(self.last_log_line().unwrap_or(e));
                }
                Err(_) => {}
            }
        }
    }
}

/// GET /health sul server (Kokoro-FastAPI e il nostro server lo espongono entrambi).
async fn probe(http: &reqwest::Client, url: &str) -> Result<(), String> {
    let (root, _) = root_and_port(url)?;
    let resp = http
        .get(format!("{root}/health"))
        // su Windows una porta chiusa su 127.0.0.1 risponde dopo ~2 s: margine abbondante
        .timeout(Duration::from_secs(6))
        .send()
        .await
        .map_err(|e| {
            if e.is_connect() {
                format!("il server non risponde su {root} ({})", crate::providers::describe_net_error(&e))
            } else {
                crate::providers::describe_net_error(&e)
            }
        })?;
    match resp.status().as_u16() {
        200 => Ok(()),
        503 => Err("modello in caricamento…".into()),
        c => Err(format!("risposta inattesa (HTTP {c})")),
    }
}

impl Drop for Kokoro {
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn url_parts() {
        assert_eq!(root_and_port("http://127.0.0.1:8880/v1").unwrap(), ("http://127.0.0.1:8880".into(), 8880));
        assert_eq!(root_and_port("http://localhost/v1").unwrap().1, 80);
        assert!(is_local("http://localhost:8880/v1"));
        assert!(!is_local("https://api.example.com/v1"));
    }

    /// Richiede tools/mock_openai.py in esecuzione.
    #[tokio::test]
    async fn health_probe() {
        let http = reqwest::Client::new();
        assert!(probe(&http, "http://127.0.0.1:18080/v1").await.is_ok());
        assert!(probe(&http, "http://127.0.0.1:1/v1").await.is_err());
        let k = Kokoro::new(&std::env::temp_dir().join("ae-kokoro-test"));
        assert!(!k.installed());
        assert!(k.ensure_ready(&http, "http://127.0.0.1:1/v1", true).await.is_err());
    }
}
