//! Ricerca e download dell'audio da YouTube con yt-dlp (scaricato da GitHub al primo uso
//! in `%APPDATA%\AlterEgo\tools`, insieme a Deno, il motore JavaScript che yt-dlp usa per YouTube).

use serde::Deserialize;
use std::{
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};

const YTDLP_URL: &str = "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp.exe";
const YTDLP_LINUX_URL: &str = "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp_linux";
const DENO_LINUX_URL: &str = "https://github.com/denoland/deno/releases/latest/download/deno-x86_64-unknown-linux-gnu.zip";
const DENO_URL: &str = "https://github.com/denoland/deno/releases/latest/download/deno-x86_64-pc-windows-msvc.zip";

pub struct Tools {
    pub ytdlp: PathBuf,
    pub deno: Option<PathBuf>,
}

/// Metadati stampati da yt-dlp.
#[derive(Deserialize, serde::Serialize, Default, Debug, Clone)]
#[serde(default)]
pub struct Info {
    pub id: String,
    pub title: String,
    pub track: Option<String>,
    pub artist: Option<String>,
    pub uploader: Option<String>,
    pub channel: Option<String>,
    pub duration: Option<f64>,
    pub thumbnail: Option<String>,
    pub ext: String,
    pub webpage_url: Option<String>,
}

fn tools_dir(data_dir: &Path) -> PathBuf {
    data_dir.join("tools")
}

async fn download(http: &reqwest::Client, url: &str, dest: &Path) -> Result<(), String> {
    use futures_util::StreamExt;
    use tokio::io::AsyncWriteExt;
    let resp = http
        .get(url)
        .timeout(Duration::from_secs(600))
        .send()
        .await
        .map_err(|e| format!("download di {url}: {}", crate::providers::describe_net_error(&e)))?;
    if !resp.status().is_success() {
        return Err(format!("download di {url}: HTTP {}", resp.status().as_u16()));
    }
    let part = dest.with_extension("part");
    let mut f = tokio::fs::File::create(&part).await.map_err(|e| e.to_string())?;
    let mut stream = resp.bytes_stream();
    while let Some(chunk) = stream.next().await {
        f.write_all(&chunk.map_err(|e| e.to_string())?).await.map_err(|e| e.to_string())?;
    }
    f.flush().await.map_err(|e| e.to_string())?;
    drop(f);
    tokio::fs::rename(&part, dest).await.map_err(|e| e.to_string())
}

fn command(exe: &Path) -> tokio::process::Command {
    let mut c = tokio::process::Command::new(exe);
    c.stdin(std::process::Stdio::null()).kill_on_drop(true);
    #[cfg(windows)]
    c.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    c
}

/// yt-dlp (+ Deno se possibile). `status` riceve i messaggi da mostrare all'utente.
pub async fn ensure(http: &reqwest::Client, data_dir: &Path, status: impl Fn(&str)) -> Result<Tools, String> {
    // per i test e per chi ha già yt-dlp: percorso esplicito
    if let Some(p) = std::env::var_os("ALTEREGO_YTDLP") {
        return Ok(Tools { ytdlp: PathBuf::from(p), deno: None });
    }
    let dir = tools_dir(data_dir);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    if !cfg!(windows) {
        return ensure_unix(http, &dir, status).await;
    }
    let ytdlp = dir.join("yt-dlp.exe");
    if !ytdlp.exists() {
        status(crate::i18n::tr("Scarico yt-dlp (solo la prima volta)…", "Downloading yt-dlp (first time only)…"));
        download(http, YTDLP_URL, &ytdlp).await?;
    }
    let deno = dir.join("deno.exe");
    if !deno.exists() {
        status(crate::i18n::tr("Scarico Deno, serve a yt-dlp per YouTube (solo la prima volta)…", "Downloading Deno, yt-dlp needs it for YouTube (first time only)…"));
        let zip = dir.join("deno.zip");
        match download(http, DENO_URL, &zip).await {
            Ok(()) => {
                // tar.exe (bsdtar) è incluso in Windows 10 1803+ e apre anche gli zip
                let ok = command(Path::new("tar"))
                    .arg("-xf")
                    .arg(&zip)
                    .arg("-C")
                    .arg(&dir)
                    .status()
                    .await
                    .map(|s| s.success())
                    .unwrap_or(false);
                let _ = std::fs::remove_file(&zip);
                if !ok {
                    eprintln!("estrazione di Deno non riuscita: yt-dlp proverà senza");
                }
            }
            Err(e) => eprintln!("Deno non scaricato ({e}): yt-dlp proverà senza"),
        }
    }
    Ok(Tools { ytdlp, deno: deno.exists().then_some(deno) })
}

/// Linux: yt-dlp del sistema se c'è, altrimenti il binario ufficiale (yt-dlp_linux) e Deno nella cartella strumenti.
async fn ensure_unix(http: &reqwest::Client, dir: &Path, status: impl Fn(&str)) -> Result<Tools, String> {
    let system = PathBuf::from("yt-dlp");
    let system_ok = command(&system).arg("--version").output().await.is_ok_and(|o| o.status.success());
    let ytdlp = if system_ok {
        system
    } else {
        let p = dir.join("yt-dlp");
        if !p.exists() {
            status(crate::i18n::tr("Scarico yt-dlp (solo la prima volta)…", "Downloading yt-dlp (first time only)…"));
            download(http, YTDLP_LINUX_URL, &p).await?;
            make_executable(&p);
        }
        p
    };
    let deno_sys = PathBuf::from("deno");
    if command(&deno_sys).arg("--version").output().await.is_ok_and(|o| o.status.success()) {
        return Ok(Tools { ytdlp, deno: Some(deno_sys) });
    }
    let deno = dir.join("deno");
    if !deno.exists() {
        status(crate::i18n::tr("Scarico Deno, serve a yt-dlp per YouTube (solo la prima volta)…", "Downloading Deno, yt-dlp needs it for YouTube (first time only)…"));
        let zip = dir.join("deno.zip");
        if download(http, DENO_LINUX_URL, &zip).await.is_ok() {
            let ok = command(Path::new("unzip")).arg("-o").arg("-q").arg(&zip).arg("-d").arg(dir).status().await.is_ok_and(|s| s.success());
            let _ = std::fs::remove_file(&zip);
            if ok {
                make_executable(&deno);
            } else {
                eprintln!("estrazione di Deno non riuscita (serve unzip): yt-dlp proverà senza");
            }
        }
    }
    Ok(Tools { ytdlp, deno: deno.exists().then_some(deno) })
}

#[cfg(unix)]
fn make_executable(p: &Path) {
    use std::os::unix::fs::PermissionsExt;
    let _ = std::fs::set_permissions(p, std::fs::Permissions::from_mode(0o755));
}
#[cfg(not(unix))]
fn make_executable(_p: &Path) {}

/// Aggiorna yt-dlp (YouTube cambia spesso: le versioni vecchie smettono di funzionare).
pub async fn update(tools: &Tools) -> Result<(), String> {
    let out = command(&tools.ytdlp).arg("-U").output().await.map_err(|e| e.to_string())?;
    if out.status.success() { Ok(()) } else { Err(String::from_utf8_lossy(&out.stderr).chars().take(300).collect()) }
}

/// Download in corso (o finito) di un brano: il lettore lo suona mentre yt-dlp scrive.
#[derive(Debug)]
pub struct Download {
    /// File finale nella cache.
    pub file: PathBuf,
    /// File parziale che yt-dlp scrive durante il download (rinominato in `file` alla fine).
    pub part: PathBuf,
    pub done: AtomicBool,
    pub failed: AtomicBool,
}

impl Download {
    pub fn finished(file: PathBuf) -> Arc<Self> {
        let part = part_of(&file);
        Arc::new(Self { file, part, done: AtomicBool::new(true), failed: AtomicBool::new(false) })
    }
    pub fn is_done(&self) -> bool {
        self.done.load(Ordering::SeqCst)
    }
    pub fn is_failed(&self) -> bool {
        self.failed.load(Ordering::SeqCst)
    }
    /// Finito o fallito: il file non crescerà più.
    pub fn settled(&self) -> bool {
        self.is_done() || self.is_failed()
    }
}

fn part_of(file: &Path) -> PathBuf {
    let mut p = file.as_os_str().to_owned();
    p.push(".part");
    PathBuf::from(p)
}

/// Brani già scaricati: ricerca (normalizzata) → metadati. Così un brano già sentito parte
/// subito, senza chiedere niente a YouTube.
const INDEX: &str = "index.json";

fn index_key(query: &str) -> String {
    query.trim().to_lowercase().split_whitespace().collect::<Vec<_>>().join(" ")
}

fn load_index(cache: &Path) -> std::collections::HashMap<String, Info> {
    std::fs::read_to_string(cache.join(INDEX)).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
}

fn remember(cache: &Path, query: &str, info: &Info) {
    let mut idx = load_index(cache);
    idx.insert(index_key(query), info.clone());
    // le voci dei file non più in cache se ne vanno
    idx.retain(|_, i| cache.join(format!("{}.{}", i.id, ext_of(i))).exists());
    if let Ok(s) = serde_json::to_string(&idx) {
        let _ = std::fs::write(cache.join(INDEX), s);
    }
}

fn ext_of(info: &Info) -> &str {
    if info.ext.is_empty() { "m4a" } else { &info.ext }
}

/// Brano già in cache per questa ricerca (file completo).
pub fn cached(query: &str, cache: &Path) -> Option<(Info, Arc<Download>)> {
    let info = load_index(cache).remove(&index_key(query))?;
    let file = cache.join(format!("{}.{}", info.id, ext_of(&info)));
    if !file.is_file() || part_of(&file).exists() {
        return None;
    }
    // il brano risulta "usato adesso" per la pulizia della cache
    let _ = std::fs::File::options().append(true).open(&file).and_then(|f| f.set_modified(std::time::SystemTime::now()));
    Some((info, Download::finished(file)))
}

/// Byte scritti prima di far partire la riproduzione (l'inizio del file ha l'indice del brano).
const START_BYTES: u64 = 96 * 1024;

/// Cerca (o apre il link) e fa partire il download dell'audio in `cache`. Torna appena il brano
/// è identificato e i primi dati sono su disco: il resto arriva mentre suona (`Download`).
pub async fn fetch(tools: &Tools, query: &str, cache: &Path) -> Result<(Info, Arc<Download>), String> {
    use tokio::io::{AsyncBufReadExt, AsyncReadExt};
    std::fs::create_dir_all(cache).map_err(|e| e.to_string())?;
    let q = query.trim();
    let target = if q.starts_with("http://") || q.starts_with("https://") { q.to_string() } else { format!("ytsearch1:{q}") };
    let mut c = command(&tools.ytdlp);
    c.args(["--no-playlist", "--no-warnings", "--no-progress", "--no-simulate", "--no-mtime"])
        // AAC in contenitore MP4: lo decodifichiamo senza ffmpeg
        .args(["-f", "bestaudio[ext=m4a]/bestaudio[acodec^=mp4a]/140"])
        .arg("-o")
        .arg(cache.join("%(id)s.%(ext)s"))
        // i metadati si stampano prima del download
        .args(["--print", "%(.{id,title,track,artist,uploader,channel,duration,thumbnail,ext,webpage_url})j"]);
    if let Some(d) = &tools.deno {
        c.arg("--js-runtimes").arg(format!("deno:{}", d.display()));
    }
    c.arg(&target).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped());
    let mut child = c.spawn().map_err(|e| crate::tr!("impossibile avviare yt-dlp: {e}", "cannot start yt-dlp: {e}"))?;
    let mut stdout = tokio::io::BufReader::new(child.stdout.take().ok_or("yt-dlp: stdout")?).lines();
    let mut stderr = child.stderr.take().ok_or("yt-dlp: stderr")?;
    let err_text = Arc::new(std::sync::Mutex::new(String::new()));
    {
        let err_text = err_text.clone();
        tauri::async_runtime::spawn(async move {
            let mut buf = String::new();
            let _ = stderr.read_to_string(&mut buf).await;
            *err_text.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = buf;
        });
    }
    let fail = |err_text: &Arc<std::sync::Mutex<String>>| -> String {
        let err = err_text.lock().unwrap_or_else(std::sync::PoisonError::into_inner).clone();
        let line = err.lines().rev().find(|l| l.contains("ERROR")).unwrap_or(err.trim()).to_string();
        if line.is_empty() { crate::i18n::tr("nessun risultato", "no results").into() } else { line.chars().take(300).collect() }
    };
    // 1. metadati (ricerca su YouTube)
    let info: Info = loop {
        let line = tokio::time::timeout(Duration::from_secs(120), stdout.next_line())
            .await
            .map_err(|_| crate::i18n::tr("yt-dlp non ha risposto entro 2 minuti", "yt-dlp didn't answer within 2 minutes").to_string())?
            .map_err(|e| e.to_string())?;
        match line {
            Some(l) => {
                if let Ok(i) = serde_json::from_str::<Info>(l.trim()) {
                    break i;
                }
            }
            None => {
                let _ = child.wait().await;
                tokio::time::sleep(Duration::from_millis(50)).await;
                return Err(fail(&err_text));
            }
        }
    };
    let file = cache.join(format!("{}.{}", info.id, ext_of(&info)));
    let dl = Arc::new(Download { part: part_of(&file), file, done: AtomicBool::new(false), failed: AtomicBool::new(false) });
    // 2. il download continua in background; alla fine si segna (e si ricorda la ricerca)
    {
        let dl = dl.clone();
        let (cache, query, info2) = (cache.to_path_buf(), q.to_string(), info.clone());
        tauri::async_runtime::spawn(async move {
            while let Ok(Some(_)) = stdout.next_line().await {}
            let ok = child.wait().await.is_ok_and(|s| s.success()) && dl.file.is_file();
            if ok {
                dl.done.store(true, Ordering::SeqCst);
                remember(&cache, &query, &info2);
                prune(&cache, KEEP);
            } else {
                dl.failed.store(true, Ordering::SeqCst);
            }
        });
    }
    // 3. si aspetta l'inizio del file
    let t0 = std::time::Instant::now();
    loop {
        if dl.is_done() {
            return Ok((info, dl));
        }
        if dl.is_failed() {
            tokio::time::sleep(Duration::from_millis(50)).await;
            return Err(if dl.file.is_file() { crate::i18n::tr("download interrotto", "download interrupted").into() } else { fail(&err_text) });
        }
        let have = std::fs::metadata(&dl.part).or_else(|_| std::fs::metadata(&dl.file)).map(|m| m.len()).unwrap_or(0);
        if have >= START_BYTES {
            return Ok((info, dl));
        }
        if t0.elapsed() > Duration::from_secs(120) {
            return Err(crate::i18n::tr("il download non parte", "the download doesn't start").into());
        }
        tokio::time::sleep(Duration::from_millis(60)).await;
    }
}

/// Brani tenuti nella cache (i più recenti): restano sul PC e la volta dopo partono subito.
pub const KEEP: usize = 300;

/// Sottotitoli caricati a mano sul video (VTT), usati come testo se le altre fonti non hanno niente.
pub async fn subtitles(tools: &Tools, url: &str, dir: &Path) -> Option<String> {
    let _ = std::fs::create_dir_all(dir);
    let mut c = command(&tools.ytdlp);
    c.args(["--skip-download", "--write-subs", "--no-warnings", "--no-playlist"])
        .args(["--sub-langs", "ja,en,it,ja.*,en.*,it.*", "--sub-format", "vtt"])
        .arg("-o")
        .arg(dir.join("%(id)s"));
    if let Some(d) = &tools.deno {
        c.arg("--js-runtimes").arg(format!("deno:{}", d.display()));
    }
    c.arg(url);
    let _ = tokio::time::timeout(Duration::from_secs(60), c.output()).await.ok()?.ok()?;
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir).ok()?.filter_map(|e| e.ok().map(|e| e.path())).filter(|p| p.extension().is_some_and(|e| e == "vtt")).collect();
    files.sort();
    let text = files.first().and_then(|p| std::fs::read_to_string(p).ok());
    for f in files {
        let _ = std::fs::remove_file(f);
    }
    text
}

/// Tiene solo i `keep` file più recenti nella cache.
pub fn prune(cache: &Path, keep: usize) {
    let Ok(rd) = std::fs::read_dir(cache) else { return };
    let mut files: Vec<_> = rd
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_file() && e.file_name() != INDEX && !e.file_name().to_string_lossy().ends_with(".part"))
        .filter_map(|e| Some((e.metadata().ok()?.modified().ok()?, e.path())))
        .collect();
    files.sort_by(|a, b| b.0.cmp(&a.0));
    for (_, p) in files.into_iter().skip(keep) {
        let _ = std::fs::remove_file(p);
    }
}

/// Titolo e artista leggibili da metadati YouTube spesso disordinati.
/// Titolo e autore da mostrare: quelli di YouTube così come sono (titolo del video, nome del canale).
/// I metadati "musicali" di YouTube (`track`/`artist`) a volte sono di un altro brano: non si usano qui.
pub fn display_meta(info: &Info) -> (String, String) {
    let title = info.title.split_whitespace().collect::<Vec<_>>().join(" ");
    let channel = info
        .channel
        .clone()
        .or_else(|| info.uploader.clone())
        .unwrap_or_default()
        .trim_end_matches(" - Topic")
        .trim()
        .to_string();
    (title, channel)
}

/// Titolo e artista "puliti", solo per cercare il testo della canzone.
pub fn clean_meta(info: &Info) -> (String, String) {
    let strip = |s: &str| -> String {
        let mut t = s.to_string();
        // "(Official Video)", "[MV]", "【...】" ecc.
        for (open, close) in [('(', ')'), ('[', ']'), ('【', '】')] {
            while let (Some(a), Some(b)) = (t.find(open), t.rfind(close)) {
                if b <= a {
                    break;
                }
                let inner = t[a..=b].to_lowercase();
                let noise = ["official", "video", "audio", "lyric", "mv", "hd", "4k", "remaster", "visualizer", "op", "ed"];
                if noise.iter().any(|n| inner.contains(n)) {
                    t.replace_range(a..b + close.len_utf8(), "");
                } else {
                    break;
                }
            }
        }
        t.split_whitespace().collect::<Vec<_>>().join(" ")
    };
    let channel = info
        .artist
        .clone()
        .or_else(|| info.channel.clone())
        .or_else(|| info.uploader.clone())
        .unwrap_or_default()
        .trim_end_matches(" - Topic")
        .to_string();
    if let Some(track) = info.track.as_deref().filter(|t| !t.is_empty()) {
        return (strip(track), channel);
    }
    let title = strip(&info.title);
    match title.split_once(" - ") {
        Some((a, b)) if info.artist.is_none() => (b.trim().to_string(), a.trim().to_string()),
        _ => (title, channel),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metadata_cleanup() {
        let i = Info { title: "Yoko Takahashi - A Cruel Angel's Thesis (Official Video)".into(), channel: Some("Some Channel".into()), ..Default::default() };
        assert_eq!(clean_meta(&i), ("A Cruel Angel's Thesis".into(), "Yoko Takahashi".into()));
        let i = Info { title: "x".into(), track: Some("A Cruel Angel's Thesis".into()), artist: Some("Yoko Takahashi".into()), ..Default::default() };
        assert_eq!(clean_meta(&i), ("A Cruel Angel's Thesis".into(), "Yoko Takahashi".into()));
        let i = Info { title: "Zankoku na Tenshi no Thesis [4K]".into(), uploader: Some("Evangelion - Topic".into()), ..Default::default() };
        assert_eq!(clean_meta(&i), ("Zankoku na Tenshi no Thesis".into(), "Evangelion".into()));
    }
}
