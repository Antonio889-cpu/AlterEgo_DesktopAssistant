//! Ricerca e download dell'audio da YouTube con yt-dlp (scaricato da GitHub al primo uso
//! in `%APPDATA%\AlterEgo\tools`, insieme a Deno, il motore JavaScript che yt-dlp usa per YouTube).

use serde::Deserialize;
use std::{
    path::{Path, PathBuf},
    time::Duration,
};

const YTDLP_URL: &str = "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp.exe";
const DENO_URL: &str = "https://github.com/denoland/deno/releases/latest/download/deno-x86_64-pc-windows-msvc.zip";

pub struct Tools {
    pub ytdlp: PathBuf,
    pub deno: Option<PathBuf>,
}

/// Metadati stampati da yt-dlp.
#[derive(Deserialize, Default, Debug)]
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
        // fuori da Windows: quello installato nel sistema
        return Ok(Tools { ytdlp: PathBuf::from("yt-dlp"), deno: None });
    }
    let ytdlp = dir.join("yt-dlp.exe");
    if !ytdlp.exists() {
        status("Scarico yt-dlp (solo la prima volta)…");
        download(http, YTDLP_URL, &ytdlp).await?;
    }
    let deno = dir.join("deno.exe");
    if !deno.exists() {
        status("Scarico Deno, serve a yt-dlp per YouTube (solo la prima volta)…");
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

/// Aggiorna yt-dlp (YouTube cambia spesso: le versioni vecchie smettono di funzionare).
pub async fn update(tools: &Tools) -> Result<(), String> {
    let out = command(&tools.ytdlp).arg("-U").output().await.map_err(|e| e.to_string())?;
    if out.status.success() { Ok(()) } else { Err(String::from_utf8_lossy(&out.stderr).chars().take(300).collect()) }
}

/// Cerca (o apre il link) e scarica l'audio in `cache`. Restituisce metadati e file.
pub async fn fetch(tools: &Tools, query: &str, cache: &Path) -> Result<(Info, PathBuf), String> {
    std::fs::create_dir_all(cache).map_err(|e| e.to_string())?;
    let q = query.trim();
    let target = if q.starts_with("http://") || q.starts_with("https://") { q.to_string() } else { format!("ytsearch1:{q}") };
    let mut c = command(&tools.ytdlp);
    c.args(["--no-playlist", "--no-warnings", "--no-progress", "--no-simulate", "--no-mtime"])
        // AAC in contenitore MP4: lo decodifichiamo senza ffmpeg
        .args(["-f", "bestaudio[ext=m4a]/bestaudio[acodec^=mp4a]/140"])
        .arg("-o")
        .arg(cache.join("%(id)s.%(ext)s"))
        .args(["--print", "%(.{id,title,track,artist,uploader,channel,duration,thumbnail,ext,webpage_url})j"]);
    if let Some(d) = &tools.deno {
        c.arg("--js-runtimes").arg(format!("deno:{}", d.display()));
    }
    c.arg(&target);
    let out = tokio::time::timeout(Duration::from_secs(180), c.output())
        .await
        .map_err(|_| "yt-dlp non ha finito entro 3 minuti".to_string())?
        .map_err(|e| format!("impossibile avviare yt-dlp: {e}"))?;
    let stdout = String::from_utf8_lossy(&out.stdout);
    let info: Option<Info> = stdout.lines().rev().find_map(|l| serde_json::from_str(l.trim()).ok());
    if !out.status.success() || info.is_none() {
        let err = String::from_utf8_lossy(&out.stderr);
        let line = err.lines().rev().find(|l| l.contains("ERROR")).unwrap_or(err.trim());
        return Err(if line.is_empty() { "nessun risultato".into() } else { line.chars().take(300).collect() });
    }
    let info = info.unwrap();
    let file = cache.join(format!("{}.{}", info.id, if info.ext.is_empty() { "m4a" } else { &info.ext }));
    if !file.exists() {
        return Err("yt-dlp non ha scaricato il file audio".into());
    }
    Ok((info, file))
}

/// Tiene solo i `keep` file più recenti nella cache.
pub fn prune(cache: &Path, keep: usize) {
    let Ok(rd) = std::fs::read_dir(cache) else { return };
    let mut files: Vec<_> = rd
        .filter_map(|e| e.ok())
        .filter_map(|e| Some((e.metadata().ok()?.modified().ok()?, e.path())))
        .collect();
    files.sort_by(|a, b| b.0.cmp(&a.0));
    for (_, p) in files.into_iter().skip(keep) {
        let _ = std::fs::remove_file(p);
    }
}

/// Titolo e artista leggibili da metadati YouTube spesso disordinati.
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
