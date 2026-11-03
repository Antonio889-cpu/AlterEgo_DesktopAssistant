//! Testi sincronizzati da LRCLIB (https://lrclib.net, gratuito, senza chiave).

use serde::Deserialize;
use std::time::Duration;

#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct Item {
    track_name: String,
    artist_name: String,
    duration: f64,
    synced_lyrics: Option<String>,
}

/// "[01:23.45] testo" → [(83.45, "testo")]. Righe vuote tenute (pause strumentali).
pub fn parse_lrc(lrc: &str) -> Vec<(f64, String)> {
    let mut out = Vec::new();
    for line in lrc.lines() {
        let mut rest = line.trim();
        let mut times = Vec::new();
        while let Some(r) = rest.strip_prefix('[') {
            let Some((tag, after)) = r.split_once(']') else { break };
            let Some((m, s)) = tag.split_once(':') else { break };
            match (m.trim().parse::<f64>(), s.trim().parse::<f64>()) {
                (Ok(m), Ok(s)) => times.push(m * 60.0 + s),
                _ => break,
            }
            rest = after;
        }
        for t in times {
            out.push((t, rest.trim().to_string()));
        }
    }
    out.sort_by(|a, b| a.0.total_cmp(&b.0));
    out
}

/// Cerca il testo sincronizzato più adatto (durata entro 5 secondi se nota).
pub async fn find(http: &reqwest::Client, title: &str, artist: &str, duration: f64) -> Option<Vec<(f64, String)>> {
    let mut queries: Vec<Vec<(&str, &str)>> = Vec::new();
    if !artist.is_empty() {
        queries.push(vec![("track_name", title), ("artist_name", artist)]);
    }
    let q = format!("{title} {artist}");
    queries.push(vec![("q", q.trim())]);
    for params in queries {
        let resp = http
            .get("https://lrclib.net/api/search")
            .query(&params)
            .header("User-Agent", "AlterEgo/1.1 (assistente desktop personale)")
            .timeout(Duration::from_secs(10))
            .send()
            .await
            .ok()?;
        let items: Vec<Item> = resp.json().await.unwrap_or_default();
        let fits = |i: &&Item| duration <= 0.0 || i.duration <= 0.0 || (i.duration - duration).abs() <= 5.0;
        if let Some(best) = items.iter().filter(|i| i.synced_lyrics.is_some()).find(fits) {
            let lines = parse_lrc(best.synced_lyrics.as_deref().unwrap_or(""));
            if !lines.is_empty() {
                eprintln!("testo: {} – {}", best.track_name, best.artist_name);
                return Some(lines);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lrc() {
        let l = parse_lrc("[ar:x]\n[00:01.50] Zankoku na tenshi no you ni\n[00:04.00][01:00.00]Shounen yo shinwa ni nare\n[00:03.00]");
        assert_eq!(l.len(), 4);
        assert_eq!(l[0], (1.5, "Zankoku na tenshi no you ni".into()));
        assert_eq!(l[1], (3.0, String::new()));
        assert_eq!(l[3].0, 60.0);
    }
}
