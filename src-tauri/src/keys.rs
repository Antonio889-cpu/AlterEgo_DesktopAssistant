//! Chiavi API nel Windows Credential Manager (crate `keyring`).
//! Ogni provider può avere più chiavi ("slot"): la config salva solo id ed etichetta,
//! la chiave sta nel Credential Manager come `AlterEgo / api-key:<provider>:<id>`.
//! Le chiavi non tornano mai al frontend e non finiscono nei log.

const SERVICE: &str = "AlterEgo";

fn entry(account: &str) -> Result<keyring::Entry, String> {
    keyring::Entry::new(SERVICE, account).map_err(|e| crate::tr!("Credential Manager non disponibile: {e}", "Credential Manager not available: {e}"))
}

fn account(provider: &str, slot: &str) -> String {
    format!("api-key:{provider}:{slot}")
}

#[cfg(test)]
thread_local! {
    pub static TEST_KEYS: std::cell::RefCell<std::collections::HashMap<String, String>> = Default::default();
}

pub fn get_slot(provider: &str, slot: &str) -> Option<String> {
    #[cfg(test)]
    if let Some(k) = TEST_KEYS.with(|m| m.borrow().get(&account(provider, slot)).cloned()) {
        return Some(k);
    }
    entry(&account(provider, slot)).ok()?.get_password().ok().filter(|k| !k.is_empty())
}

pub fn set_slot(provider: &str, slot: &str, key: &str) -> Result<(), String> {
    let key = key.trim();
    if key.is_empty() {
        return Err("chiave vuota".into());
    }
    entry(&account(provider, slot))?.set_password(key).map_err(|e| format!("salvataggio chiave fallito: {e}"))
}

pub fn delete_slot(provider: &str, slot: &str) -> Result<(), String> {
    match entry(&account(provider, slot))?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(format!("cancellazione chiave fallita: {e}")),
    }
}

/// Versioni precedenti salvavano una sola chiave come `api-key:<provider>`:
/// la sposta nello slot "1". Restituisce true se ha migrato qualcosa.
pub fn migrate_legacy(provider: &str) -> bool {
    let Ok(old) = entry(&format!("api-key:{provider}")) else { return false };
    let Ok(key) = old.get_password() else { return false };
    if set_slot(provider, "1", &key).is_ok() {
        let _ = old.delete_credential();
        return true;
    }
    false
}

/// Toglie una chiave da un testo (es. messaggio d'errore del provider) prima di mostrarlo.
pub fn redact(text: &str, key: Option<&str>) -> String {
    match key {
        Some(k) if k.len() >= 6 => text.replace(k, "••••"),
        _ => text.to_string(),
    }
}
