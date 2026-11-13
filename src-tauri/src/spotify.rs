//! Spotify (utenti Premium): Alter Ego cerca e suona i brani sul dispositivo Spotify dell'utente
//! (app desktop, telefono, web player) tramite la Web API.
//!
//! Collegamento: OAuth "Authorization Code + PKCE" (nessun segreto dell'app). L'utente crea un'app
//! gratuita su developer.spotify.com, imposta come Redirect URI `http://127.0.0.1:8898/callback`
//! e incolla il Client ID nelle impostazioni. Il refresh token sta nel Credential Manager.
//! Controllare la riproduzione (play, pausa, coda, volume) richiede Spotify Premium.

use crate::{keys, update_config, AppState};
use serde::Serialize;
use serde_json::{json, Value};
use std::{
    sync::Mutex,
    time::{Duration, Instant},
};
use tauri::{AppHandle, Manager};

pub const REDIRECT: &str = "http://127.0.0.1:8898/callback";
const PORT: u16 = 8898;
const SCOPES: &str = "user-read-playback-state user-modify-playback-state user-read-currently-playing user-read-private";
const API: &str = "https://api.spotify.com/v1";

/// Token di accesso in memoria (scade dopo ~1 ora; si rinnova con il refresh token).
static ACCESS: Mutex<Option<(String, Instant)>> = Mutex::new(None);

fn b64url(bytes: &[u8]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}

fn random(n: usize) -> Result<Vec<u8>, String> {
    let mut v = vec![0u8; n];
    getrandom::getrandom(&mut v).map_err(|e| e.to_string())?;
    Ok(v)
}

/// Coppia PKCE: verificatore e challenge S256.
pub fn pkce() -> Result<(String, String), String> {
    use sha2::Digest;
    let verifier = b64url(&random(48)?);
    let challenge = b64url(&sha2::Sha256::digest(verifier.as_bytes()));
    Ok((verifier, challenge))
}

fn enc(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

fn client_id(app: &AppHandle) -> Result<String, String> {
    let id = app.state::<AppState>().cfg.lock().unwrap_or_else(std::sync::PoisonError::into_inner).spotify.client_id.trim().to_string();
    if id.is_empty() {
        Err(crate::i18n::tr("manca il Client ID di Spotify (Impostazioni → Voce e audio → Spotify)", "Spotify Client ID missing (Settings → Voice & audio → Spotify)").into())
    } else {
        Ok(id)
    }
}

fn open_browser(url: &str) {
    #[cfg(windows)]
    let _ = std::process::Command::new("rundll32").args(["url.dll,FileProtocolHandler", url]).spawn();
    #[cfg(not(windows))]
    let _ = std::process::Command::new("xdg-open").arg(url).spawn();
}

/// Aspetta il ritorno del browser su 127.0.0.1:8898/callback e restituisce `code` (controllando `state`).
async fn wait_callback(expected_state: &str) -> Result<String, String> {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", PORT))
        .await
        .map_err(|e| crate::tr!("porta {PORT} occupata: {e}", "port {PORT} busy: {e}"))?;
    let deadline = Instant::now() + Duration::from_secs(180);
    loop {
        let left = deadline.saturating_duration_since(Instant::now());
        let (mut sock, _) = tokio::time::timeout(left, listener.accept())
            .await
            .map_err(|_| crate::i18n::tr("collegamento non completato entro 3 minuti", "connection not completed within 3 minutes").to_string())?
            .map_err(|e| e.to_string())?;
        let mut buf = vec![0u8; 8192];
        let n = sock.read(&mut buf).await.unwrap_or(0);
        let req = String::from_utf8_lossy(&buf[..n]).to_string();
        let path = req.split_whitespace().nth(1).unwrap_or("").to_string();
        if !path.starts_with("/callback") {
            let _ = sock.write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n").await;
            continue;
        }
        let query = path.split_once('?').map(|(_, q)| q).unwrap_or("");
        let get = |k: &str| query.split('&').find_map(|kv| kv.split_once('=').filter(|(a, _)| *a == k).map(|(_, v)| v.to_string()));
        let ok = get("state").as_deref() == Some(expected_state) && get("code").is_some();
        let msg = if ok {
            crate::i18n::tr("Spotify collegato ad Alter Ego. Puoi chiudere questa scheda.", "Spotify is connected to Alter Ego. You can close this tab.")
        } else {
            crate::i18n::tr("Collegamento non riuscito: torna su Alter Ego e riprova.", "Connection failed: go back to Alter Ego and try again.")
        };
        let html = format!(
            "<!doctype html><meta charset=utf-8><title>Alter Ego</title><body style=\"background:#061a02;color:#c9ff8e;font:20px monospace;display:grid;place-items:center;height:90vh\"><p>{msg}</p></body>"
        );
        let resp = format!("HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{html}", html.len());
        let _ = sock.write_all(resp.as_bytes()).await;
        if let Some(err) = get("error") {
            return Err(format!("Spotify: {err}"));
        }
        if ok {
            return Ok(get("code").unwrap_or_default());
        }
    }
}

async fn token_request(app: &AppHandle, form: &[(&str, &str)]) -> Result<Value, String> {
    let resp = app
        .state::<AppState>()
        .ai
        .http
        .post("https://accounts.spotify.com/api/token")
        .form(form)
        .timeout(Duration::from_secs(20))
        .send()
        .await
        .map_err(|e| format!("Spotify: {}", crate::providers::describe_net_error(&e)))?;
    let status = resp.status();
    let v: Value = resp.json().await.unwrap_or(json!({}));
    if !status.is_success() {
        let d = v.get("error_description").or_else(|| v.get("error")).and_then(Value::as_str).unwrap_or("errore");
        return Err(format!("Spotify (HTTP {}): {d}", status.as_u16()));
    }
    Ok(v)
}

fn store_tokens(v: &Value) -> Result<String, String> {
    let access = v.get("access_token").and_then(Value::as_str).ok_or("Spotify: token mancante")?.to_string();
    let secs = v.get("expires_in").and_then(Value::as_u64).unwrap_or(3600);
    if let Some(r) = v.get("refresh_token").and_then(Value::as_str) {
        keys::set_slot("spotify", "refresh", r)?;
    }
    *ACCESS.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = Some((access.clone(), Instant::now() + Duration::from_secs(secs.saturating_sub(60))));
    Ok(access)
}

/// Collega l'account: apre il browser, aspetta il ritorno, salva il refresh token.
pub async fn connect(app: &AppHandle) -> Result<String, String> {
    let id = client_id(app)?;
    let (verifier, challenge) = pkce()?;
    let state = b64url(&random(16)?);
    let url = format!(
        "https://accounts.spotify.com/authorize?client_id={}&response_type=code&redirect_uri={}&code_challenge_method=S256&code_challenge={challenge}&scope={}&state={state}",
        enc(&id),
        enc(REDIRECT),
        enc(SCOPES)
    );
    let wait = wait_callback(&state);
    open_browser(&url);
    let code = wait.await?;
    let v = token_request(app, &[
        ("grant_type", "authorization_code"),
        ("code", &code),
        ("redirect_uri", REDIRECT),
        ("client_id", &id),
        ("code_verifier", &verifier),
    ])
    .await?;
    store_tokens(&v)?;
    let me = api(app, reqwest::Method::GET, "/me", None).await?.unwrap_or(json!({}));
    let name = me.get("display_name").and_then(Value::as_str).unwrap_or("Spotify").to_string();
    let premium = me.get("product").and_then(Value::as_str) == Some("premium");
    let account = if premium { name } else { format!("{name} (Free)") };
    update_config(app, |c| {
        c.spotify.connected = true;
        c.spotify.account = account.clone();
    });
    Ok(account)
}

pub fn disconnect(app: &AppHandle) {
    let _ = keys::delete_slot("spotify", "refresh");
    *ACCESS.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = None;
    update_config(app, |c| {
        c.spotify.connected = false;
        c.spotify.account.clear();
    });
}

async fn access_token(app: &AppHandle) -> Result<String, String> {
    if let Some((t, until)) = ACCESS.lock().unwrap_or_else(std::sync::PoisonError::into_inner).clone() {
        if Instant::now() < until {
            return Ok(t);
        }
    }
    let refresh = keys::get_slot("spotify", "refresh")
        .ok_or(crate::i18n::tr("Spotify non è collegato (Impostazioni → Voce e audio → Spotify)", "Spotify is not connected (Settings → Voice & audio → Spotify)"))?;
    let id = client_id(app)?;
    let v = token_request(app, &[("grant_type", "refresh_token"), ("refresh_token", &refresh), ("client_id", &id)]).await?;
    store_tokens(&v)
}

/// Chiamata alla Web API. Ok(None) = risposta vuota (204).
async fn api(app: &AppHandle, method: reqwest::Method, path: &str, body: Option<Value>) -> Result<Option<Value>, String> {
    let token = access_token(app).await?;
    let mut req = app.state::<AppState>().ai.http.request(method, format!("{API}{path}")).bearer_auth(token).timeout(Duration::from_secs(20));
    req = match body {
        Some(b) => req.json(&b),
        None => req.header("Content-Length", "0"),
    };
    let resp = req.send().await.map_err(|e| format!("Spotify: {}", crate::providers::describe_net_error(&e)))?;
    let status = resp.status();
    let text = resp.text().await.unwrap_or_default();
    if status.is_success() {
        return Ok(serde_json::from_str(&text).ok());
    }
    let v: Value = serde_json::from_str(&text).unwrap_or(json!({}));
    let reason = v.pointer("/error/reason").and_then(Value::as_str).unwrap_or("");
    let msg = v.pointer("/error/message").and_then(Value::as_str).unwrap_or("").to_string();
    Err(match (status.as_u16(), reason) {
        (403, "PREMIUM_REQUIRED") | (403, _) if msg.to_lowercase().contains("premium") || reason == "PREMIUM_REQUIRED" => {
            crate::i18n::tr("Spotify: per controllare la riproduzione serve Spotify Premium", "Spotify: controlling playback requires Spotify Premium").into()
        }
        (404, "NO_ACTIVE_DEVICE") | (404, _) if reason == "NO_ACTIVE_DEVICE" || msg.to_lowercase().contains("device") => "NO_ACTIVE_DEVICE".into(),
        (401, _) => {
            *ACCESS.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = None;
            crate::i18n::tr("Spotify: accesso scaduto, ricollega l'account", "Spotify: access expired, reconnect the account").into()
        }
        (code, _) => format!("Spotify (HTTP {code}): {msg}"),
    })
}

/// Dispositivo su cui suonare: quello attivo, altrimenti il PC, altrimenti il primo.
async fn pick_device(app: &AppHandle) -> Result<Option<String>, String> {
    let v = api(app, reqwest::Method::GET, "/me/player/devices", None).await?.unwrap_or(json!({}));
    let list = v.get("devices").and_then(Value::as_array).cloned().unwrap_or_default();
    let pick = list
        .iter()
        .find(|d| d.get("is_active").and_then(Value::as_bool) == Some(true))
        .or_else(|| list.iter().find(|d| d.get("type").and_then(Value::as_str) == Some("Computer")))
        .or_else(|| list.first());
    Ok(pick.and_then(|d| d.get("id").and_then(Value::as_str)).map(str::to_string))
}

/// Comando di riproduzione; senza dispositivo attivo ne sceglie uno (e se serve apre l'app Spotify).
async fn player(app: &AppHandle, method: reqwest::Method, path: &str, body: Option<Value>) -> Result<(), String> {
    match api(app, method.clone(), path, body.clone()).await {
        Err(e) if e == "NO_ACTIVE_DEVICE" => {
            let mut dev = pick_device(app).await?;
            if dev.is_none() {
                // apre l'app Spotify del PC e riprova
                open_browser("spotify:");
                tokio::time::sleep(Duration::from_secs(5)).await;
                dev = pick_device(app).await?;
            }
            let dev = dev.ok_or(crate::i18n::tr(
                "nessun dispositivo Spotify attivo: apri Spotify sul PC o sul telefono",
                "no active Spotify device: open Spotify on your PC or phone",
            ))?;
            let sep = if path.contains('?') { '&' } else { '?' };
            api(app, method, &format!("{path}{sep}device_id={}", enc(&dev)), body).await.map(|_| ())
        }
        r => r.map(|_| ()),
    }
}

#[derive(Serialize, Clone, Debug, Default)]
pub struct SpTrack {
    pub id: String,
    pub uri: String,
    pub title: String,
    pub artist: String,
    pub duration: f64,
    pub image: Option<String>,
}

pub fn parse_track(t: &Value) -> Option<SpTrack> {
    Some(SpTrack {
        id: t.get("id")?.as_str()?.to_string(),
        uri: t.get("uri")?.as_str()?.to_string(),
        title: t.get("name").and_then(Value::as_str).unwrap_or("").to_string(),
        artist: t
            .get("artists")
            .and_then(Value::as_array)
            .map(|a| a.iter().filter_map(|x| x.get("name").and_then(Value::as_str)).collect::<Vec<_>>().join(", "))
            .unwrap_or_default(),
        duration: t.get("duration_ms").and_then(Value::as_f64).unwrap_or(0.0) / 1000.0,
        image: t.pointer("/album/images/0/url").and_then(Value::as_str).map(str::to_string),
    })
}

/// Cerca e suona (o mette in coda). `query` può essere anche un link o un URI Spotify.
pub async fn play(app: &AppHandle, query: &str, enqueue: bool) -> Result<String, String> {
    let q = query.trim();
    let track = if let Some(id) = q.strip_prefix("spotify:track:").or_else(|| q.split("open.spotify.com/track/").nth(1).map(|r| r.split(['?', '/']).next().unwrap_or(r))) {
        api(app, reqwest::Method::GET, &format!("/tracks/{}", enc(id)), None).await?.as_ref().and_then(parse_track)
    } else {
        let v = api(app, reqwest::Method::GET, &format!("/search?type=track&limit=1&q={}", enc(q)), None).await?.unwrap_or(json!({}));
        v.pointer("/tracks/items/0").and_then(parse_track)
    };
    let t = track.ok_or_else(|| crate::tr!("Spotify: nessun brano trovato per «{q}»", "Spotify: no track found for “{q}”"))?;
    if enqueue {
        player(app, reqwest::Method::POST, &format!("/me/player/queue?uri={}", enc(&t.uri)), None).await?;
    } else {
        player(app, reqwest::Method::PUT, "/me/player/play", Some(json!({ "uris": [t.uri] }))).await?;
    }
    // va anche nella Music Venue (recenti)
    crate::music::record_external(app, crate::music::library::Entry {
        id: format!("sp:{}", t.id),
        title: t.title.clone(),
        artist: t.artist.clone(),
        duration: t.duration,
        thumbnail: t.image.clone(),
        url: Some(t.uri.clone()),
        ..Default::default()
    });
    Ok(if enqueue {
        crate::tr!("Su Spotify, in coda: «{}» di {}.", "On Spotify, queued: “{}” by {}.", t.title, t.artist)
    } else {
        crate::tr!("Su Spotify: «{}» di {}.", "On Spotify: “{}” by {}.", t.title, t.artist)
    })
}

/// Comandi (stessi di music_control).
pub async fn control(app: &AppHandle, action: &str, value: Option<f64>) -> Result<String, String> {
    use reqwest::Method;
    let tr = crate::i18n::tr;
    match action {
        "pause" => player(app, Method::PUT, "/me/player/pause", None).await.map(|_| tr("Spotify in pausa.", "Spotify paused.").into()),
        "resume" => player(app, Method::PUT, "/me/player/play", None).await.map(|_| tr("Spotify riprende.", "Spotify resumes.").into()),
        "next" => player(app, Method::POST, "/me/player/next", None).await.map(|_| tr("Brano successivo su Spotify.", "Next track on Spotify.").into()),
        "previous" => player(app, Method::POST, "/me/player/previous", None).await.map(|_| tr("Brano precedente su Spotify.", "Previous track on Spotify.").into()),
        "stop" => player(app, Method::PUT, "/me/player/pause", None).await.map(|_| tr("Spotify fermato.", "Spotify stopped.").into()),
        "volume" => {
            let v = value.unwrap_or(60.0).clamp(0.0, 100.0).round() as u32;
            player(app, Method::PUT, &format!("/me/player/volume?volume_percent={v}"), None).await.map(|_| crate::tr!("Volume di Spotify al {v}%.", "Spotify volume at {v}%."))
        }
        "favorite" | "unfavorite" => Err(tr("i preferiti di Spotify si gestiscono nell'app Spotify", "Spotify favorites are managed in the Spotify app").into()),
        _ => {
            let v = api(app, Method::GET, "/me/player", None).await?;
            Ok(match v.as_ref().and_then(|v| v.get("item")).and_then(parse_track) {
                Some(t) => {
                    let playing = v.as_ref().and_then(|v| v.get("is_playing")).and_then(Value::as_bool).unwrap_or(false);
                    format!("{} «{}» di {} (Spotify).", if playing { "Sta suonando" } else { "In pausa:" }, t.title, t.artist)
                }
                None => tr("Su Spotify non sta suonando niente.", "Nothing is playing on Spotify.").into(),
            })
        }
    }
}

/// Spotify è attivo e collegato.
pub fn active(app: &AppHandle) -> bool {
    let c = app.state::<AppState>().cfg.lock().unwrap_or_else(std::sync::PoisonError::into_inner).spotify.clone();
    c.enabled && c.connected
}

#[derive(Serialize)]
pub struct Status {
    pub connected: bool,
    pub account: String,
    pub redirect: &'static str,
}

#[tauri::command]
pub async fn spotify_connect(app: AppHandle, client_id: String) -> Result<String, String> {
    let id = client_id.trim().to_string();
    update_config(&app, |c| c.spotify.client_id = id);
    connect(&app).await
}

#[tauri::command]
pub fn spotify_disconnect(app: AppHandle) {
    disconnect(&app);
}

#[tauri::command]
pub fn spotify_settings(app: AppHandle, enabled: bool) {
    update_config(&app, |c| c.spotify.enabled = enabled);
}

#[tauri::command]
pub fn spotify_status(app: AppHandle) -> Status {
    let c = app.state::<AppState>().cfg.lock().unwrap_or_else(std::sync::PoisonError::into_inner).spotify.clone();
    Status { connected: c.connected && keys::get_slot("spotify", "refresh").is_some(), account: c.account, redirect: REDIRECT }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pkce_and_track() {
        let (v, c) = pkce().unwrap();
        assert!(v.len() >= 43 && v.len() <= 128);
        assert_eq!(c.len(), 43);
        let t = json!({"id": "1", "uri": "spotify:track:1", "name": "Megalovania", "duration_ms": 157000,
            "artists": [{"name": "Toby Fox"}], "album": {"images": [{"url": "https://i.scdn.co/image/x"}]}});
        let p = parse_track(&t).unwrap();
        assert_eq!((p.title.as_str(), p.artist.as_str(), p.duration), ("Megalovania", "Toby Fox", 157.0));
        assert_eq!(enc("a b:c"), "a%20b%3Ac");
    }
}
