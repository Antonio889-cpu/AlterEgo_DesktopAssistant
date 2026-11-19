//! Accesso facoltativo (spento di default, a rischio dell'utente) all'API web non ufficiale di VRChat,
//! solo per cambiare avatar da Alter Ego. Minimo indispensabile:
//! GET /auth/user (Basic) → eventuale 2FA (TOTP, codice di riserva o email) → PUT /avatars/{id}/select.
//! La password non si salva mai; il cookie di sessione sta solo nel portachiavi del sistema
//! (Credential Manager su Windows, vedi keys.rs). User-Agent descrittivo come chiede VRChat.

use base64::Engine;
use serde_json::Value;
use std::time::Duration;

pub const BASE: &str = "https://api.vrchat.cloud/api/1";

pub fn user_agent() -> String {
    format!("AlterEgo-DesktopAssistant/{} (+https://github.com/Antonio889-cpu/AlterEgo_DesktopAssistant)", env!("CARGO_PKG_VERSION"))
}

/// Percent-encoding (RFC 3986): VRChat vuole nome e password codificati prima del Basic.
fn urlenc(s: &str) -> String {
    let mut o = String::new();
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) {
            o.push(b as char);
        } else {
            o.push_str(&format!("%{b:02X}"));
        }
    }
    o
}

pub fn basic(user: &str, pass: &str) -> String {
    format!("Basic {}", base64::engine::general_purpose::STANDARD.encode(format!("{}:{}", urlenc(user.trim()), urlenc(pass))))
}

/// Id di avatar valido (finisce dentro l'URL: niente altro).
pub fn valid_avatar_id(id: &str) -> bool {
    id.strip_prefix("avtr_").is_some_and(|r| r.len() == 36 && r.chars().all(|c| c.is_ascii_hexdigit() || c == '-'))
}

/// Cookie della sessione ("auth" e "twoFactorAuth").
#[derive(Default, Clone, Debug, PartialEq)]
pub struct Session {
    pub cookies: Vec<(String, String)>,
}

impl Session {
    pub fn header(&self) -> String {
        self.cookies.iter().map(|(k, v)| format!("{k}={v}")).collect::<Vec<_>>().join("; ")
    }
    pub fn from_header(h: &str) -> Self {
        let cookies = h
            .split(';')
            .filter_map(|p| p.trim().split_once('='))
            .filter(|(k, v)| !k.is_empty() && !v.is_empty())
            .map(|(k, v)| (k.trim().to_string(), v.trim().to_string()))
            .collect();
        Self { cookies }
    }
    /// Prende i cookie da una riga Set-Cookie (solo quelli che servono).
    pub fn absorb(&mut self, set_cookie: &str) {
        let Some((k, v)) = set_cookie.split(';').next().and_then(|kv| kv.split_once('=')) else { return };
        let (k, v) = (k.trim(), v.trim());
        if !["auth", "twoFactorAuth"].contains(&k) {
            return;
        }
        self.cookies.retain(|(n, _)| n != k);
        if !v.is_empty() {
            self.cookies.push((k.to_string(), v.to_string()));
        }
    }
    pub fn is_empty(&self) -> bool {
        self.cookies.is_empty()
    }
}

#[derive(Debug, PartialEq)]
pub enum Login {
    Ok { name: String, avatar: String },
    /// Serve il secondo fattore: metodi possibili ("totp", "otp", "emailOtp").
    TwoFactor(Vec<String>),
}

/// Legge la risposta di GET /auth/user.
pub fn parse_user(status: u16, body: &str) -> Result<Login, String> {
    let v: Value = serde_json::from_str(body).unwrap_or(Value::Null);
    match status {
        200 => {
            if let Some(m) = v.get("requiresTwoFactorAuth").and_then(Value::as_array) {
                return Ok(Login::TwoFactor(m.iter().filter_map(Value::as_str).map(String::from).collect()));
            }
            let name = v.get("displayName").and_then(Value::as_str).unwrap_or("").to_string();
            if name.is_empty() {
                return Err(crate::i18n::tr("risposta di VRChat non riconosciuta", "unrecognized VRChat response").into());
            }
            Ok(Login::Ok { name, avatar: v.get("currentAvatar").and_then(Value::as_str).unwrap_or("").to_string() })
        }
        401 => Err(crate::i18n::tr("nome utente o password sbagliati (o sessione scaduta)", "wrong username or password (or session expired)").into()),
        429 => Err(crate::i18n::tr("troppi tentativi: VRChat chiede di aspettare", "too many attempts: VRChat asks you to wait").into()),
        s => Err(format!("VRChat: HTTP {s} {}", v.pointer("/error/message").and_then(Value::as_str).unwrap_or("").chars().take(120).collect::<String>())),
    }
}

/// Percorso della verifica per il metodo scelto.
pub fn verify_path(method: &str) -> &'static str {
    match method.to_ascii_lowercase().as_str() {
        "emailotp" | "email" => "auth/twofactorauth/emailotp/verify",
        "otp" => "auth/twofactorauth/otp/verify",
        _ => "auth/twofactorauth/totp/verify",
    }
}

fn base_req(rb: reqwest::RequestBuilder, s: &Session) -> reqwest::RequestBuilder {
    let rb = rb.header(reqwest::header::USER_AGENT, user_agent()).timeout(Duration::from_secs(20));
    if s.is_empty() { rb } else { rb.header(reqwest::header::COOKIE, s.header()) }
}

pub fn req_login(http: &reqwest::Client, base: &str, user: &str, pass: &str) -> reqwest::RequestBuilder {
    base_req(http.get(format!("{base}/auth/user")), &Session::default()).header(reqwest::header::AUTHORIZATION, basic(user, pass))
}

pub fn req_verify(http: &reqwest::Client, base: &str, s: &Session, method: &str, code: &str) -> reqwest::RequestBuilder {
    let code: String = code.chars().filter(|c| c.is_ascii_alphanumeric()).take(16).collect();
    base_req(http.post(format!("{base}/{}", verify_path(method))), s).json(&serde_json::json!({ "code": code }))
}

pub fn req_me(http: &reqwest::Client, base: &str, s: &Session) -> reqwest::RequestBuilder {
    base_req(http.get(format!("{base}/auth/user")), s)
}

pub fn req_select(http: &reqwest::Client, base: &str, s: &Session, avatar: &str) -> Result<reqwest::RequestBuilder, String> {
    if !valid_avatar_id(avatar) {
        return Err(crate::i18n::tr("id avatar non valido", "invalid avatar id").into());
    }
    Ok(base_req(http.put(format!("{base}/avatars/{avatar}/select")), s))
}

pub fn req_logout(http: &reqwest::Client, base: &str, s: &Session) -> reqwest::RequestBuilder {
    base_req(http.put(format!("{base}/logout")), s)
}

/// Manda e raccoglie (stato, corpo) aggiornando i cookie.
pub async fn send(rb: reqwest::RequestBuilder, s: &mut Session) -> Result<(u16, String), String> {
    let r = rb.send().await.map_err(|e| format!("VRChat: {}", crate::providers::describe_net_error(&e)))?;
    for c in r.headers().get_all(reqwest::header::SET_COOKIE) {
        if let Ok(c) = c.to_str() {
            s.absorb(c);
        }
    }
    let st = r.status().as_u16();
    Ok((st, r.text().await.unwrap_or_default()))
}

// ---------- portachiavi ----------

pub fn load_session() -> Session {
    crate::keys::get_slot("vrchat", "session").map(|h| Session::from_header(&h)).unwrap_or_default()
}

pub fn save_session(s: &Session) -> Result<(), String> {
    if s.is_empty() {
        return crate::keys::delete_slot("vrchat", "session");
    }
    crate::keys::set_slot("vrchat", "session", &s.header())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_login_requests_without_network() {
        let http = reqwest::Client::new();
        // Basic con nome e password codificati (spazi, due punti, caratteri speciali)
        let b = basic(" Makoto N ", "p@ss:wörd");
        let dec = String::from_utf8(base64::engine::general_purpose::STANDARD.decode(b.trim_start_matches("Basic ")).unwrap()).unwrap();
        assert_eq!(dec, "Makoto%20N:p%40ss%3Aw%C3%B6rd");
        let r = req_login(&http, BASE, "Makoto N", "p@ss:wörd").build().unwrap();
        assert_eq!(r.method(), reqwest::Method::GET);
        assert_eq!(r.url().as_str(), "https://api.vrchat.cloud/api/1/auth/user");
        assert_eq!(r.headers()["authorization"], b.as_str());
        let ua = r.headers()["user-agent"].to_str().unwrap();
        assert!(ua.starts_with("AlterEgo-DesktopAssistant/") && ua.contains("github.com"), "{ua}");
        assert!(r.headers().get("cookie").is_none());
        // 2FA: cookie della sessione, codice ripulito, percorso giusto
        let mut s = Session::default();
        s.absorb("auth=authcookie_123; Path=/; HttpOnly");
        s.absorb("other=x; Path=/");
        let v = req_verify(&http, BASE, &s, "totp", " 123 456 ").build().unwrap();
        assert_eq!(v.method(), reqwest::Method::POST);
        assert_eq!(v.url().path(), "/api/1/auth/twofactorauth/totp/verify");
        assert_eq!(v.headers()["cookie"], "auth=authcookie_123");
        assert_eq!(std::str::from_utf8(v.body().unwrap().as_bytes().unwrap()).unwrap(), "{\"code\":\"123456\"}");
        assert_eq!(verify_path("emailOtp"), "auth/twofactorauth/emailotp/verify");
        assert_eq!(verify_path("otp"), "auth/twofactorauth/otp/verify");
        // dopo la verifica: due cookie; select dell'avatar
        s.absorb("twoFactorAuth=tfa_456; Path=/");
        let id = "avtr_c38a1615-5bf5-42b4-84eb-a8b6c37cbd11";
        let sel = req_select(&http, BASE, &s, id).unwrap().build().unwrap();
        assert_eq!(sel.method(), reqwest::Method::PUT);
        assert_eq!(sel.url().path(), format!("/api/1/avatars/{id}/select"));
        assert_eq!(sel.headers()["cookie"], "auth=authcookie_123; twoFactorAuth=tfa_456");
        assert!(req_select(&http, BASE, &s, "avtr_../../logout").is_err());
        assert!(req_select(&http, BASE, &s, "usr_c38a1615-5bf5-42b4-84eb-a8b6c37cbd11").is_err());
        assert_eq!(req_logout(&http, BASE, &s).build().unwrap().url().path(), "/api/1/logout");
        assert_eq!(req_me(&http, BASE, &s).build().unwrap().headers()["cookie"], s.header().as_str());
        // cookie salvati come una riga e riletti; cookie svuotato = tolto
        assert_eq!(Session::from_header(&s.header()), s);
        s.absorb("auth=; Max-Age=0");
        assert_eq!(s.header(), "twoFactorAuth=tfa_456");
    }

    #[test]
    fn parses_auth_responses() {
        assert_eq!(parse_user(200, r#"{"requiresTwoFactorAuth":["totp","otp"]}"#), Ok(Login::TwoFactor(vec!["totp".into(), "otp".into()])));
        assert_eq!(parse_user(200, r#"{"requiresTwoFactorAuth":["emailOtp"]}"#), Ok(Login::TwoFactor(vec!["emailOtp".into()])));
        assert_eq!(
            parse_user(200, r#"{"id":"usr_1","displayName":"AE Bot","currentAvatar":"avtr_x"}"#),
            Ok(Login::Ok { name: "AE Bot".into(), avatar: "avtr_x".into() })
        );
        assert!(parse_user(401, r#"{"error":{"message":"Invalid Username/Email or Password","status_code":401}}"#).is_err());
        assert!(parse_user(429, "").is_err());
        assert!(parse_user(500, r#"{"error":{"message":"boom"}}"#).unwrap_err().contains("boom"));
        assert!(parse_user(200, "{}").is_err());
    }
}
