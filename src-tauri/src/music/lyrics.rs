//! Testi sincronizzati, con più fonti in ordine:
//! 1. LRCLIB `/api/get` (titolo + artista + durata esatti)
//! 2. LRCLIB `/api/search` con più varianti di titolo/artista
//! 3. NetEase Cloud Music (molti brani asiatici e anime)
//! 4. sottotitoli del video YouTube (solo quelli caricati a mano, via yt-dlp)
//! 5. testo NON sincronizzato (LRCLIB/NetEase) distribuito sulla durata del brano
//! Ogni fonte che fallisce (rete, formato) passa semplicemente alla successiva.

use serde_json::Value;
use std::time::Duration;

const UA: &str = "AlterEgo/1.2 (assistente desktop personale; https://github.com/Antonio889-cpu/AlterEgo_DesktopAssistant)";

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Lyrics {
    pub lines: Vec<(f64, String)>,
    /// Da dove arriva il testo (per il lettore).
    pub source: String,
    /// false = tempi stimati (testo non sincronizzato).
    pub synced: bool,
}

/// "[01:23.45] testo" → [(83.45, "testo")]. Tiene le righe vuote (pause) e applica [offset:±ms].
pub fn parse_lrc(lrc: &str) -> Vec<(f64, String)> {
    let mut out = Vec::new();
    let mut offset = 0.0;
    for line in lrc.lines() {
        let mut rest = line.trim();
        if let Some(v) = rest.strip_prefix("[offset:").and_then(|r| r.strip_suffix(']')) {
            // offset positivo = testo in anticipo
            offset = v.trim().parse::<f64>().unwrap_or(0.0) / 1000.0;
            continue;
        }
        let mut times = Vec::new();
        while let Some(r) = rest.strip_prefix('[') {
            let Some((tag, after)) = r.split_once(']') else { break };
            let Some((m, s)) = tag.split_once(':') else { break };
            match (m.trim().parse::<f64>(), s.trim().replace(':', ".").parse::<f64>()) {
                (Ok(m), Ok(s)) => times.push(m * 60.0 + s),
                _ => break,
            }
            rest = after;
        }
        for t in times {
            out.push((t, rest.trim().to_string()));
        }
    }
    for l in out.iter_mut() {
        l.0 = (l.0 - offset).max(0.0);
    }
    out.sort_by(|a, b| a.0.total_cmp(&b.0));
    out
}

/// WebVTT dei sottotitoli → righe con il tempo d'inizio (senza tag e doppioni consecutivi).
pub fn parse_vtt(vtt: &str) -> Vec<(f64, String)> {
    fn ts(s: &str) -> Option<f64> {
        let s = s.trim().replace(',', ".");
        let parts: Vec<&str> = s.split(':').collect();
        let (h, m, sec) = match parts.as_slice() {
            [h, m, s] => (h.parse::<f64>().ok()?, m.parse::<f64>().ok()?, s.parse::<f64>().ok()?),
            [m, s] => (0.0, m.parse::<f64>().ok()?, s.parse::<f64>().ok()?),
            _ => return None,
        };
        Some(h * 3600.0 + m * 60.0 + sec)
    }
    let strip_tags = |t: &str| {
        let mut out = String::new();
        let mut inside = false;
        for c in t.chars() {
            match c {
                '<' => inside = true,
                '>' => inside = false,
                c if !inside => out.push(c),
                _ => {}
            }
        }
        out.replace("&amp;", "&").replace("&nbsp;", " ").trim().to_string()
    };
    let mut out: Vec<(f64, String)> = Vec::new();
    let mut lines = vtt.lines().peekable();
    while let Some(l) = lines.next() {
        let Some((a, _)) = l.split_once("-->") else { continue };
        let Some(start) = ts(a) else { continue };
        let mut text = Vec::new();
        while let Some(n) = lines.peek() {
            if n.trim().is_empty() {
                break;
            }
            text.push(strip_tags(n));
            lines.next();
        }
        let t = text.join(" ").replace(['♪', '♫'], "").trim().to_string();
        if t.is_empty() || out.last().is_some_and(|(_, prev)| *prev == t) {
            continue;
        }
        out.push((start, t));
    }
    out
}

/// Testo senza tempi → righe distribuite tra l'8% e il 92% della durata (stima).
pub fn spread_plain(text: &str, duration: f64) -> Vec<(f64, String)> {
    let lines: Vec<String> = text.lines().map(|l| l.trim().to_string()).filter(|l| !l.is_empty()).collect();
    if lines.is_empty() || duration <= 0.0 {
        return Vec::new();
    }
    let (start, span) = (duration * 0.08, duration * 0.84);
    let step = span / lines.len() as f64;
    lines.into_iter().enumerate().map(|(i, l)| (start + i as f64 * step, l)).collect()
}

/// Varianti del titolo da provare: pulito, senza "feat.", senza parti dopo " / " o " | ".
pub fn title_variants(title: &str, raw_title: &str) -> Vec<String> {
    let mut v: Vec<String> = Vec::new();
    let mut push = |s: String| {
        let s = s.split_whitespace().collect::<Vec<_>>().join(" ");
        if !s.is_empty() && !v.iter().any(|x| x.eq_ignore_ascii_case(&s)) {
            v.push(s);
        }
    };
    push(title.to_string());
    let lower = title.to_lowercase();
    for cut in [" feat.", " ft.", " featuring ", " (feat", " (ft"] {
        if let Some(i) = lower.find(cut) {
            push(title[..i].to_string());
        }
    }
    for sep in [" / ", " | ", " ／ "] {
        if let Some((a, _)) = title.split_once(sep) {
            push(a.to_string());
        }
    }
    // titolo con virgolette o parentesi giapponesi: 「…」
    if let (Some(a), Some(b)) = (title.find('「'), title.find('」')) {
        if b > a {
            push(title[a + '「'.len_utf8()..b].to_string());
        }
    }
    push(raw_title.to_string());
    v
}

fn fits(want: f64, got: f64, tol: f64) -> bool {
    want <= 0.0 || got <= 0.0 || (want - got).abs() <= tol
}

async fn get_json(http: &reqwest::Client, url: &str, query: &[(&str, String)], referer: Option<&str>) -> Option<Value> {
    let mut req = http.get(url).query(query).header("User-Agent", UA).timeout(Duration::from_secs(10));
    if let Some(r) = referer {
        req = req.header("Referer", r);
    }
    match req.send().await {
        Ok(r) if r.status().is_success() => r.json().await.ok(),
        Ok(r) => {
            eprintln!("testi: {url} → HTTP {}", r.status().as_u16());
            None
        }
        Err(e) => {
            eprintln!("testi: {url} → {}", crate::providers::describe_net_error(&e));
            None
        }
    }
}

/// Risultato LRCLIB: testo sincronizzato oppure, in mancanza, quello semplice.
fn lrclib_pick(items: &[Value], duration: f64) -> (Option<String>, Option<String>) {
    let dur = |i: &Value| i.get("duration").and_then(Value::as_f64).unwrap_or(0.0);
    let synced = |i: &Value| i.get("syncedLyrics").and_then(Value::as_str).filter(|s| !s.trim().is_empty()).map(str::to_string);
    let plain = |i: &Value| i.get("plainLyrics").and_then(Value::as_str).filter(|s| !s.trim().is_empty()).map(str::to_string);
    let best_synced = items
        .iter()
        .filter(|i| fits(duration, dur(i), 5.0))
        .find_map(synced)
        .or_else(|| items.iter().filter(|i| fits(duration, dur(i), 15.0)).find_map(synced));
    let best_plain = items.iter().filter(|i| fits(duration, dur(i), 10.0)).find_map(plain);
    (best_synced, best_plain)
}

async fn lrclib(http: &reqwest::Client, titles: &[String], artist: &str, duration: f64, plain_out: &mut Option<String>) -> Option<Vec<(f64, String)>> {
    let base = "https://lrclib.net/api";
    // 1. corrispondenza esatta
    if !artist.is_empty() && duration > 0.0 {
        let q = vec![("track_name", titles[0].clone()), ("artist_name", artist.to_string()), ("duration", format!("{}", duration.round() as i64))];
        if let Some(v) = get_json(http, &format!("{base}/get"), &q, None).await {
            let (s, p) = lrclib_pick(std::slice::from_ref(&v), 0.0);
            if plain_out.is_none() {
                *plain_out = p;
            }
            if let Some(s) = s {
                return Some(parse_lrc(&s));
            }
        }
    }
    // 2. ricerche
    let mut queries: Vec<Vec<(&str, String)>> = Vec::new();
    for t in titles {
        if !artist.is_empty() {
            queries.push(vec![("track_name", t.clone()), ("artist_name", artist.to_string())]);
            queries.push(vec![("q", format!("{t} {artist}"))]);
        }
        queries.push(vec![("track_name", t.clone())]);
    }
    for q in queries.into_iter().take(8) {
        let Some(Value::Array(items)) = get_json(http, &format!("{base}/search"), &q, None).await else { continue };
        let (s, p) = lrclib_pick(&items, duration);
        if plain_out.is_none() {
            *plain_out = p;
        }
        if let Some(s) = s {
            return Some(parse_lrc(&s));
        }
    }
    None
}

async fn netease(http: &reqwest::Client, titles: &[String], artist: &str, duration: f64, plain_out: &mut Option<String>) -> Option<Vec<(f64, String)>> {
    let referer = Some("https://music.163.com/");
    for t in titles.iter().take(3) {
        let q = vec![("s", format!("{t} {artist}").trim().to_string()), ("type", "1".into()), ("limit", "8".into())];
        let Some(v) = get_json(http, "https://music.163.com/api/search/get/web", &q, referer).await else { continue };
        let songs = v.pointer("/result/songs").and_then(Value::as_array).cloned().unwrap_or_default();
        let ms = |s: &Value| s.get("duration").and_then(Value::as_f64).unwrap_or(0.0) / 1000.0;
        let ids: Vec<i64> = songs
            .iter()
            .filter(|s| fits(duration, ms(s), 6.0))
            .chain(songs.iter())
            .filter_map(|s| s.get("id").and_then(Value::as_i64))
            .fold(Vec::new(), |mut acc, id| {
                if !acc.contains(&id) {
                    acc.push(id);
                }
                acc
            })
            .into_iter()
            .take(3)
            .collect();
        for id in ids {
            let q = vec![("id", id.to_string()), ("lv", "1".into()), ("kv", "1".into()), ("tv", "-1".into())];
            let Some(l) = get_json(http, "https://music.163.com/api/song/lyric", &q, referer).await else { continue };
            let lrc = l.pointer("/lrc/lyric").and_then(Value::as_str).unwrap_or("");
            let lines = parse_lrc(lrc);
            // NetEase mette spesso crediti ("作词 : …") nelle prime righe con tempo 0
            let lines: Vec<_> = lines.into_iter().filter(|(_, t)| !(t.contains(" : ") || t.contains("：") && t.len() < 40)).collect();
            if lines.iter().filter(|(_, t)| !t.is_empty()).count() >= 4 {
                return Some(lines);
            }
            if plain_out.is_none() && !lrc.trim().is_empty() && !lrc.contains('[') {
                *plain_out = Some(lrc.to_string());
            }
        }
    }
    None
}

/// Cerca il testo con tutte le fonti. `subtitles`: sottotitoli del video già scaricati (VTT), se ci sono.
pub async fn find(http: &reqwest::Client, title: &str, raw_title: &str, artist: &str, duration: f64, subtitles: Option<String>) -> Option<Lyrics> {
    let titles = title_variants(title, raw_title);
    let mut plain = None;
    if titles.is_empty() {
        let lines = subtitles.as_deref().map(parse_vtt).unwrap_or_default();
        return (lines.len() >= 4).then(|| Lyrics { lines, source: "YouTube".into(), synced: true });
    }
    if let Some(lines) = lrclib(http, &titles, artist, duration, &mut plain).await {
        return Some(Lyrics { lines, source: "LRCLIB".into(), synced: true });
    }
    if let Some(lines) = netease(http, &titles, artist, duration, &mut plain).await {
        return Some(Lyrics { lines, source: "NetEase".into(), synced: true });
    }
    if let Some(vtt) = subtitles {
        let lines = parse_vtt(&vtt);
        if lines.len() >= 4 {
            return Some(Lyrics { lines, source: "YouTube".into(), synced: true });
        }
    }
    let lines = spread_plain(plain.as_deref().unwrap_or(""), duration);
    (!lines.is_empty()).then(|| Lyrics { lines, source: "LRCLIB (tempi stimati)".into(), synced: false })
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
        // offset positivo: il testo arriva prima
        let o = parse_lrc("[offset:+500]\n[00:02.00]a");
        assert_eq!(o[0].0, 1.5);
        // formato [mm:ss:xx]
        assert_eq!(parse_lrc("[00:02:50]b")[0].0, 2.5);
    }

    #[test]
    fn vtt() {
        let v = "WEBVTT\n\n00:00:01.000 --> 00:00:03.000\n<c>Zankoku na</c> tenshi\n\n00:00:03.000 --> 00:00:05.000\nZankoku na tenshi\n\n00:07.500 --> 00:09.000\n♪ Shounen yo ♪\n";
        let l = parse_vtt(v);
        assert_eq!(l, vec![(1.0, "Zankoku na tenshi".into()), (7.5, "Shounen yo".into())]);
    }

    #[test]
    fn plain_and_variants() {
        let p = spread_plain("a\n\nb\nc", 100.0);
        assert_eq!(p.len(), 3);
        assert!((p[0].0 - 8.0).abs() < 1e-9 && p[2].0 < 92.0);
        let v = title_variants("Idol feat. Someone / TV Size", "YOASOBI「アイドル」 Official Music Video");
        assert!(v.contains(&"Idol".to_string()));
        assert!(v.contains(&"Idol feat. Someone".to_string()));
        assert!(v.iter().any(|x| x.starts_with("YOASOBI")));
        let v = title_variants("「アイドル」", "");
        assert!(v.contains(&"アイドル".to_string()));
    }
}
