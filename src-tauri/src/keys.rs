//! Chiavi API nel Windows Credential Manager (crate `keyring`).
//! Le chiavi non tornano mai al frontend e non finiscono nei log:
//! il frontend può solo salvarle, cancellarle e sapere se esistono.

const SERVICE: &str = "AlterEgo";

fn entry(provider: &str) -> Result<keyring::Entry, String> {
    keyring::Entry::new(SERVICE, &format!("api-key:{provider}"))
        .map_err(|e| format!("Credential Manager non disponibile: {e}"))
}

pub fn get(provider: &str) -> Option<String> {
    entry(provider).ok()?.get_password().ok().filter(|k| !k.is_empty())
}

pub fn set(provider: &str, key: &str) -> Result<(), String> {
    let key = key.trim();
    let e = entry(provider)?;
    if key.is_empty() {
        return match e.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(err) => Err(format!("cancellazione chiave fallita: {err}")),
        };
    }
    e.set_password(key).map_err(|err| format!("salvataggio chiave fallito: {err}"))
}

pub fn has(provider: &str) -> bool {
    get(provider).is_some()
}

/// Toglie una chiave da un testo (es. messaggio d'errore del provider) prima di mostrarlo.
pub fn redact(text: &str, key: Option<&str>) -> String {
    match key {
        Some(k) if k.len() >= 6 => text.replace(k, "••••"),
        _ => text.to_string(),
    }
}
