//! Registro dei costi: una riga JSON per ogni richiesta a pagamento (chat, riassunti,
//! ricerche web, trascrizioni) in `%APPDATA%\AlterEgo\costs.jsonl`. Il costo viene calcolato
//! al momento dai prezzi in Impostazioni → Costi (modificabili).

use crate::config::Price;
use serde::{Deserialize, Serialize};
use std::{io::Write, path::Path};

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(default)]
pub struct Entry {
    /// Unix secondi.
    pub ts: i64,
    /// "chat", "riassunto", "ricerca web", "pagina web", "trascrizione".
    pub kind: String,
    pub provider: String,
    pub model: String,
    /// Token di input non in cache.
    pub input: u64,
    /// Token di input letti dalla cache.
    pub cached: u64,
    pub output: u64,
    /// Chiamate (ricerca/pagina web).
    pub calls: u64,
    /// Secondi di audio (trascrizione).
    pub audio_secs: f64,
    /// Durata della richiesta.
    pub ms: u64,
    /// Costo riportato dal provider (OpenRouter), se c'è: ha la precedenza sui prezzi.
    pub reported_cost: Option<f64>,
}

pub fn append(path: &Path, e: &Entry) {
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
        if let Ok(line) = serde_json::to_string(e) {
            let _ = writeln!(f, "{line}");
        }
    }
}

/// Le ultime `n` righe, dalla più recente.
pub fn read(path: &Path, n: usize) -> Vec<Entry> {
    let text = std::fs::read_to_string(path).unwrap_or_default();
    text.lines().rev().filter_map(|l| serde_json::from_str(l).ok()).take(n).collect()
}

/// Prezzo più specifico per provider/modello (modello più lungo che fa da prefisso).
pub fn price_for<'a>(prices: &'a [Price], provider: &str, model: &str) -> Option<&'a Price> {
    prices
        .iter()
        .filter(|p| p.provider == provider && model.starts_with(p.model.as_str()))
        .max_by_key(|p| p.model.len())
}

/// Costo in dollari di una riga.
pub fn cost(prices: &[Price], e: &Entry) -> f64 {
    if let Some(c) = e.reported_cost {
        return c;
    }
    let Some(p) = price_for(prices, &e.provider, &e.model) else { return 0.0 };
    (e.input as f64 * p.input + e.cached as f64 * p.cached + e.output as f64 * p.output) / 1_000_000.0
        + e.calls as f64 * p.per_call
        + e.audio_secs / 3600.0 * p.per_hour
}

pub fn now() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::default_prices;

    #[test]
    fn costs_from_prices() {
        let prices = default_prices();
        let e = Entry { provider: "deepseek".into(), model: "deepseek-flash".into(), input: 1_000_000, cached: 1_000_000, output: 1_000_000, ..Default::default() };
        assert!((cost(&prices, &e) - (0.30 + 0.006 + 1.20)).abs() < 1e-9);
        let s = Entry { provider: "deepseek".into(), model: "search".into(), calls: 2, ..Default::default() };
        assert!((cost(&prices, &s) - 0.0015).abs() < 1e-9);
        let w = Entry { provider: "groq".into(), model: "whisper-large-v3-turbo".into(), audio_secs: 1800.0, ..Default::default() };
        assert!((cost(&prices, &w) - 0.02).abs() < 1e-9);
        let r = Entry { provider: "openrouter".into(), reported_cost: Some(0.0123), ..Default::default() };
        assert_eq!(cost(&prices, &r), 0.0123);
        assert_eq!(cost(&prices, &Entry { provider: "gemini".into(), input: 5, ..Default::default() }), 0.0);
    }

    #[test]
    fn log_roundtrip() {
        let p = std::env::temp_dir().join(format!("ae-costs-{}.jsonl", std::process::id()));
        let _ = std::fs::remove_file(&p);
        for i in 0..3 {
            append(&p, &Entry { ts: i, kind: "chat".into(), ..Default::default() });
        }
        let r = read(&p, 2);
        assert_eq!(r.iter().map(|e| e.ts).collect::<Vec<_>>(), vec![2, 1]);
        let _ = std::fs::remove_file(&p);
    }
}
