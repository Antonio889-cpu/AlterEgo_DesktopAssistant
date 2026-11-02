//! Clip vocali: se la frase detta (o scritta) corrisponde a una frase configurata,
//! invia la scorciatoia del programma di registrazione. Nessun modello AI coinvolto.

use crate::config::ClipConfig;

/// Minuscole, senza accenti né punteggiatura, spazi singoli. "Alter-Ego, CLIPPA!" → "alter ego clippa".
pub fn normalize(s: &str) -> String {
    let mapped: String = s
        .to_lowercase()
        .chars()
        .map(|c| match c {
            'à' | 'á' | 'â' => 'a',
            'è' | 'é' | 'ê' => 'e',
            'ì' | 'í' => 'i',
            'ò' | 'ó' => 'o',
            'ù' | 'ú' => 'u',
            c if c.is_alphanumeric() => c,
            _ => ' ',
        })
        .collect();
    let joined = mapped.split_whitespace().collect::<Vec<_>>().join(" ");
    // Whisper scrive spesso "alterego" o "alter-ego"
    joined.replace("alterego", "alter ego")
}

/// La frase corrisponde se il testo, normalizzato, contiene una delle frasi come parole intere
/// e non è molto più lungo (così "come si fa a clippare un video?" non scatta).
pub fn matches(cfg: &ClipConfig, text: &str) -> bool {
    if !cfg.enabled {
        return false;
    }
    let t = normalize(text);
    let words = t.split(' ').count();
    cfg.phrases.iter().map(|p| normalize(p)).filter(|p| !p.is_empty()).any(|p| {
        let padded = format!(" {t} ");
        padded.contains(&format!(" {p} ")) && words <= p.split(' ').count() + 3
    })
}

/// Invia le scorciatoie dei programmi attivi. Restituisce i nomi riusciti o l'errore.
pub fn trigger(cfg: &ClipConfig) -> Result<Vec<String>, String> {
    let active: Vec<_> = cfg.targets.iter().filter(|t| t.enabled && !t.hotkey.trim().is_empty()).collect();
    if active.is_empty() {
        return Err("nessun programma di clip attivo: sceglilo in Impostazioni → Clip".into());
    }
    let mut ok = Vec::new();
    let mut errors = Vec::new();
    for t in active {
        match super::input::send_combo(&t.hotkey) {
            Ok(()) => ok.push(t.name.clone()),
            Err(e) => errors.push(format!("{}: {e}", t.name)),
        }
    }
    if ok.is_empty() { Err(errors.join("; ")) } else { Ok(ok) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn phrase_matching() {
        let cfg = ClipConfig::default();
        assert!(matches(&cfg, "Alter Ego, clippa!"));
        assert!(matches(&cfg, "AlterEgo clip that."));
        assert!(matches(&cfg, "ok alter-ego clip that please"));
        assert!(matches(&cfg, "Clippa."));
        assert!(!matches(&cfg, "come si fa a clippare un video su youtube?"));
        assert!(!matches(&cfg, "ieri ho visto una clip che parlava di alter ego e di molte altre cose"));
        let off = ClipConfig { enabled: false, ..Default::default() };
        assert!(!matches(&off, "alter ego clippa"));
    }
}
