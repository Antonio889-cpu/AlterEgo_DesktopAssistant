//! Chatbox di VRChat: al massimo 144 caratteri per messaggio. I testi lunghi si dividono in pagine
//! (a parole intere, con "…" quando continua) e si mostrano una dopo l'altra con il tempo di leggerle.

use std::time::Duration;

pub const MAX: usize = 144;
/// Pagine al massimo per una risposta (oltre si taglia: è una chat, non un libro).
pub const MAX_PAGES: usize = 4;

fn len(s: &str) -> usize {
    s.chars().count()
}

/// Divide `text` in pagine di al massimo `max` caratteri.
pub fn pages(text: &str, max: usize) -> Vec<String> {
    let max = max.max(8);
    let words: Vec<&str> = text.split_whitespace().collect();
    if words.is_empty() {
        return Vec::new();
    }
    let joined = words.join(" ");
    if len(&joined) <= max {
        return vec![joined];
    }
    // pagine non ultime finiscono con "…": c'è un carattere in meno
    let room = max - 1;
    let mut out: Vec<String> = Vec::new();
    let mut cur = String::new();
    for w in words {
        // parola più lunga di una pagina: si spezza
        let mut w = w.to_string();
        while len(&w) > room {
            if !cur.is_empty() {
                out.push(std::mem::take(&mut cur));
            }
            let head: String = w.chars().take(room).collect();
            w = w.chars().skip(room).collect();
            out.push(head);
        }
        let extra = if cur.is_empty() { len(&w) } else { len(&w) + 1 };
        if len(&cur) + extra > room {
            out.push(std::mem::take(&mut cur));
        }
        if !cur.is_empty() {
            cur.push(' ');
        }
        cur.push_str(&w);
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    let n = out.len();
    for (i, p) in out.iter_mut().enumerate() {
        if i + 1 < n {
            p.push('…');
        }
    }
    if out.len() > MAX_PAGES {
        out.truncate(MAX_PAGES);
        if let Some(last) = out.last_mut() {
            if !last.ends_with('…') {
                last.push('…');
            }
        }
    }
    out
}

/// Quanto lasciare una pagina a schermo: abbastanza per leggerla, e mai sotto i 3 s
/// (VRChat blocca la chatbox se riceve troppi messaggi di fila).
pub fn dwell(page: &str) -> Duration {
    Duration::from_millis((len(page) as u64 * 55).clamp(3000, 9000))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paginates_at_word_boundaries() {
        assert!(pages("   ", MAX).is_empty());
        assert_eq!(pages("Ciao!  come   stai?", MAX), vec!["Ciao! come stai?"]);
        let exact = "a".repeat(MAX);
        assert_eq!(pages(&exact, MAX), vec![exact.clone()], "144 caratteri esatti: una pagina");
        let long = "Questa è una frase abbastanza lunga che si ripete. ".repeat(6);
        let p = pages(&long, MAX);
        assert!(p.len() >= 2 && p.len() <= MAX_PAGES, "{p:?}");
        for (i, x) in p.iter().enumerate() {
            assert!(x.chars().count() <= MAX, "pagina {i} troppo lunga: {}", x.chars().count());
            assert!(!x.starts_with(' ') && !x.contains("  "));
            if i + 1 < p.len() {
                assert!(x.ends_with('…'));
            }
        }
        // niente parole perse (a parte i "…")
        let back: String = p.join(" ").replace('…', "");
        assert_eq!(back.split_whitespace().collect::<Vec<_>>(), long.split_whitespace().collect::<Vec<_>>());
        // caratteri multibyte contati come caratteri
        let jp = "こんにちは ".repeat(40);
        assert!(pages(&jp, MAX).iter().all(|x| x.chars().count() <= MAX));
        // parola enorme spezzata
        let huge = "x".repeat(300);
        let h = pages(&huge, MAX);
        assert_eq!(h.len(), 3);
        assert!(h.iter().all(|x| x.chars().count() <= MAX));
        // troppo lungo: al massimo MAX_PAGES, l'ultima con "…"
        let p = pages(&"parola ".repeat(400), MAX);
        assert_eq!(p.len(), MAX_PAGES);
        assert!(p.last().unwrap().ends_with('…'));
        assert!(dwell("ciao") >= Duration::from_secs(3) && dwell(&"a".repeat(144)) > dwell("ciao"));
    }
}
