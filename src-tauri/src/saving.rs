//! Risparmio di token sulle API, a parità di risposta (Impostazioni → AI e chiavi → Risparmio token).
//!
//! Tecniche (tutte facoltative):
//! - **strumenti su richiesta**: le definizioni degli strumenti (1–3 mila token) si mandano solo se la
//!   domanda sembra chiedere un'azione. Se il modello ne avrebbe avuto bisogno risponde con [[TOOLS]]
//!   e la domanda si ripete subito con gli strumenti (nessuna azione persa);
//! - **router locale** che decide "serve un'azione?" e "è una domanda semplice?" senza spendere token:
//!   parole chiave (gratis) oppure [Rizzo Flow](https://github.com/Rizzo-AI-Academy/rizzo-flow) in locale;
//! - **modello economico** per le domande semplici (model routing);
//! - **descrizioni compatte** degli strumenti;
//! - **cronologia accorciata**: le risposte vecchie di Alter Ego vengono troncate in modo deterministico,
//!   così il prefisso resta identico e continua a essere letto dalla cache del provider;
//! - **fatti pertinenti**: solo i fatti sull'utente che c'entrano con la domanda;
//! - screenshot più piccoli e pagine web più corte (in actions e web).

use crate::{config::SavingConfig, providers::ChatMessage};
use serde_json::{json, Value};
use std::time::Duration;

/// Segnale che il modello usa quando gli mancano gli strumenti.
pub const NEED_TOOLS: &str = "[[TOOLS]]";

/// Riga aggiunta in fondo al prompt quando gli strumenti non vengono inviati.
pub const NO_TOOLS_NOTE: &str = "\n\n## Strumenti\nIn questo messaggio gli strumenti non sono stati inviati. Se per rispondere ti serve \
un'azione (PC, app, web, musica, file, schermo, promemoria, mouse o tastiera) rispondi SOLO con [[TOOLS]] e nient'altro.";

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Route {
    /// La domanda probabilmente richiede uno strumento.
    pub needs_tools: bool,
    /// Domanda semplice: va bene il modello economico.
    pub simple: bool,
}

const ACTION_WORDS: &[&str] = &[
    // italiano
    "apri", "aprire", "chiudi", "cerca", "cercami", "trova", "metti", "suona", "riproduci", "musica", "canzone", "brano", "playlist",
    "pausa", "riprendi", "prossim", "precedente", "volume", "alza", "abbassa", "muto", "luminosità", "screenshot", "schermo",
    "file", "cartella", "documento", "leggi", "scrivi nel", "scrivi in", "salva", "elimina", "cancella", "promemoria", "ricordami",
    "timer", "sveglia", "appunti", "copia", "incolla", "clicca", "digita", "premi", "mouse", "tastiera", "terminale", "comando",
    "powershell", "esegui", "installa", "internet", "web", "online", "notizie", "meteo", "oggi", "stasera", "domani", "ultim",
    "attuale", "prezzo", "quanto costa", "risultat", "aggiorna", "impostazioni", "lettore", "venue", "preferiti", "sito", "link",
    "spotify", "youtube", "github", "click-through", "opacità", "ingrandisci", "rimpicciolisci", "nasconditi", "clip",
    // inglese
    "open", "close", "search", "look up", "find", "play", "music", "song", "track", "pause", "resume", "next", "previous",
    "skip", "louder", "quieter", "mute", "brightness", "screen", "folder", "read the", "write to", "save", "delete", "remind",
    "alarm", "clipboard", "copy", "paste", "click", "type", "press", "keyboard", "terminal", "command", "run ", "install",
    "news", "weather", "today", "tonight", "tomorrow", "latest", "current", "price", "how much", "update", "settings",
    "player", "favorite", "website", "url", "http",
];

const COMPLEX_WORDS: &[&str] = &[
    "spiega", "perché", "perche", "come funziona", "differenza", "confronta", "analizza", "codice", "programma", "funzione",
    "errore", "bug", "calcola", "dimostra", "riassumi", "traduci", "scrivi un", "scrivimi un", "elenca", "passaggi", "piano",
    "explain", "why", "how does", "difference", "compare", "analy", "code", "function", "error", "calculate", "prove",
    "summar", "translate", "write a", "list", "steps", "plan",
];

/// Parole chiave → gruppi di strumenti (vedi actions::GROUPS). Nessuna corrispondenza = tutti.
const GROUP_WORDS: &[(&str, &[&str])] = &[
    ("sistema", &["apri", "aprire", "avvia", "lancia", "terminale", "comando", "esegui", "powershell", "bash", "cmd", "installa", "disinstalla",
        "processo", "programma", "volume", "luminosità", "appunti", "copia", "incolla", "sistema", "rete", "ip ", "disco", "spazio", "ram", "cpu",
        "open", "launch", "run ", "command", "terminal", "install", "process", "program", "brightness", "clipboard", "network", "disk"]),
    ("file", &["file", "cartell", "documento", "percorso", "salva", "leggi il", "leggi la", "scrivi nel", "scrivi in un", "sposta", "rinomina",
        "desktop", "download", ".txt", ".pdf", ".csv", ".json", "folder", "directory", "path", "save", "read the", "write to", "move", "rename"]),
    ("web", &["cerca", "cercami", "internet", "web", "online", "notizie", "meteo", "sito", "pagina", "link", "http", "prezzo", "quanto costa",
        "oggi", "stasera", "domani", "ultim", "attuale", "search", "look up", "news", "weather", "website", "url", "latest", "current", "price"]),
    ("schermo", &["schermo", "screenshot", "clicca", "click", "mouse", "tastiera", "digita", "premi", "finestra", "pulsante", "bottone", "vedi",
        "guarda", "screen", "keyboard", "type ", "press", "window", "button", "look at", "see "]),
    ("musica", &["metti", "suona", "musica", "canzone", "brano", "playlist", "pausa", "riprendi", "prossim", "precedente", "spotify", "youtube",
        "play", "music", "song", "track", "pause", "resume", "skip"]),
    ("promemoria", &["promemoria", "ricordami", "timer", "sveglia", "remind", "alarm"]),
    ("database", &["database", "sql", "tabella", "tabelle", "query", "select", "db ", "table"]),
    ("grafo", &["codice", "progetto", "repository", "repo", "funzione", "classe", "modulo", "architettura", "grafo", "dipendenz",
        "code", "project", "function", "class", "module", "graph", "dependenc"]),
];

/// Gruppi di strumenti che la domanda sembra chiedere ("app" sempre). Vuoto = nessuna parola chiave.
pub fn pick_groups(text: &str) -> Vec<&'static str> {
    let t = format!(" {} ", text.to_lowercase());
    let mut out: Vec<&'static str> = GROUP_WORDS.iter().filter(|(_, words)| words.iter().any(|w| t.contains(w))).map(|(g, _)| *g).collect();
    if out.is_empty() {
        return out;
    }
    out.insert(0, "app");
    out
}

/// Router a parole chiave (gratis, istantaneo).
pub fn heuristic(text: &str, prev_used_tools: bool) -> Route {
    let t = format!(" {} ", text.to_lowercase());
    let needs_tools = prev_used_tools || ACTION_WORDS.iter().any(|w| t.contains(w));
    let words = text.split_whitespace().count();
    let simple = !needs_tools && words <= 25 && !COMPLEX_WORDS.iter().any(|w| t.contains(w)) && !text.contains('\n');
    Route { needs_tools, simple }
}

/// Router con Rizzo Flow (API compatibile Jev, `POST /v1/systemone`): decisioni tipizzate in ~50 ms,
/// zero token generati. Se il server non risponde si usa il router a parole chiave.
pub async fn rizzo(http: &reqwest::Client, url: &str, text: &str, prev_reply: &str, prev_used_tools: bool) -> Option<Route> {
    let body = json!({
        "model": "rizzo-latest",
        "state": { "user_message": text, "previous_assistant_reply": prev_reply.chars().take(300).collect::<String>() },
        "questions": {
            "needs_action": {
                "type": "noul",
                "instructions": "Is the user asking the desktop assistant to DO something with a tool: open a program or website, search the web or get current/real-time information, play or control music, read/write files, take a screenshot, set a reminder, use the mouse or keyboard, run a command, or change the assistant app (windows, player, settings, updates)? Small talk, opinions, explanations and questions answerable from general knowledge are NO."
            },
            "complexity": {
                "type": "score",
                "instructions": "How demanding is the answer the user expects?",
                "criteria": ["Greeting, small talk or a very short simple answer", "A normal everyday question", "Complex: reasoning, code, long explanation or several steps"]
            }
        }
    });
    let endpoint = format!("{}/v1/systemone", url.trim().trim_end_matches('/'));
    let resp = http.post(endpoint).timeout(Duration::from_millis(1800)).json(&body).send().await.ok()?;
    if !resp.status().is_success() {
        return None;
    }
    let v: Value = resp.json().await.ok()?;
    parse_rizzo(&v, prev_used_tools)
}

pub fn parse_rizzo(v: &Value, prev_used_tools: bool) -> Option<Route> {
    let p_action = v.pointer("/answers/needs_action/noul").and_then(Value::as_f64)?;
    let score = v.pointer("/answers/complexity/score").and_then(Value::as_f64).unwrap_or(1.0);
    // si sbaglia volentieri dalla parte degli strumenti: un'azione persa è peggio di qualche token
    let needs_tools = prev_used_tools || p_action >= 0.3;
    Some(Route { needs_tools, simple: !needs_tools && score < 0.6 })
}

/// Descrizioni degli strumenti ridotte alla prima frase, senza descrizioni dei parametri.
pub fn compact_tools(tools: &Value) -> Value {
    let mut t = tools.clone();
    for f in t.as_array_mut().into_iter().flatten() {
        if let Some(d) = f.pointer_mut("/function/description") {
            if let Some(s) = d.as_str() {
                let first = s.split(". ").next().unwrap_or(s).trim_end_matches('.').to_string();
                *d = Value::String(first);
            }
        }
        if let Some(props) = f.pointer_mut("/function/parameters/properties").and_then(Value::as_object_mut) {
            for p in props.values_mut() {
                if let Some(o) = p.as_object_mut() {
                    o.remove("description");
                }
            }
        }
    }
    t
}

/// Accorcia le risposte vecchie di Alter Ego (tutte tranne le ultime `keep` voci della cronologia).
pub fn trim_history(history: &mut [ChatMessage], keep: usize, max_chars: usize) {
    let n = history.len();
    for m in history.iter_mut().take(n.saturating_sub(keep)) {
        if m.role == "assistant" && m.content.chars().count() > max_chars {
            let cut: String = m.content.chars().take(max_chars).collect();
            m.content = format!("{cut}…");
        }
    }
}

/// Risultati degli strumenti tra `from` e `to` (già letti dal modello) accorciati a inizio + fine,
/// solo se insieme superano `threshold` caratteri. Restituisce true se ha accorciato qualcosa.
pub fn compact_tool_results(history: &mut [ChatMessage], from: usize, to: usize, threshold: usize) -> bool {
    let to = to.min(history.len());
    if from >= to {
        return false;
    }
    let total: usize = history[from..to].iter().filter(|m| m.role == "tool").map(|m| m.content.chars().count()).sum();
    if total <= threshold {
        return false;
    }
    for m in history[from..to].iter_mut().filter(|m| m.role == "tool") {
        let n = m.content.chars().count();
        if n > 1200 {
            let head: String = m.content.chars().take(700).collect();
            let tail: String = m.content.chars().skip(n - 300).collect();
            m.content = format!("{head}\n[… {} caratteri tolti: risultato già usato nei passaggi precedenti …]\n{tail}", n - 1000);
        }
    }
    true
}

/// I `n` fatti più pertinenti alla domanda (parole in comune), in ordine originale. 0 = tutti.
pub fn pick_facts(facts: Vec<String>, question: &str, n: usize) -> Vec<String> {
    if n == 0 || facts.len() <= n {
        return facts;
    }
    let words = |s: &str| -> Vec<String> {
        s.to_lowercase().split(|c: char| !c.is_alphanumeric()).filter(|w| w.chars().count() > 3).map(|w| w.chars().take(6).collect()).collect()
    };
    let q = words(question);
    let mut scored: Vec<(usize, usize)> = facts
        .iter()
        .enumerate()
        .map(|(i, f)| (words(f).iter().filter(|w| q.contains(w)).count(), i))
        .collect();
    // pertinenza, poi i più recenti (in fondo alla lista)
    scored.sort_by(|a, b| b.0.cmp(&a.0).then(b.1.cmp(&a.1)));
    let mut keep: Vec<usize> = scored.into_iter().take(n).map(|(_, i)| i).collect();
    keep.sort();
    keep.into_iter().map(|i| facts[i].clone()).collect()
}

/// Configurazione AI per una domanda semplice: il provider economico in testa, con il suo modello.
pub fn economy_ai(ai: &crate::config::AiConfig, s: &SavingConfig) -> Option<crate::config::AiConfig> {
    let id = s.economy_provider.trim();
    if id.is_empty() || crate::providers::info(id).is_none() {
        return None;
    }
    let mut a = ai.clone();
    let pc = a.providers.entry(id.to_string()).or_default();
    if !s.economy_model.trim().is_empty() {
        pc.model = s.economy_model.trim().to_string();
    }
    pc.enabled = true;
    a.order.retain(|o| o != id);
    a.order.insert(0, id.to_string());
    Some(a)
}

/// Prova del router: Rizzo Flow risponde? Restituisce il modello caricato.
#[tauri::command]
pub async fn rizzo_test(app: tauri::AppHandle, url: String) -> Result<String, String> {
    use tauri::Manager;
    let ai = &app.state::<crate::AppState>().ai;
    let endpoint = format!("{}/v1/models", url.trim().trim_end_matches('/'));
    let v: Value = ai
        .client_for(&endpoint)
        .get(&endpoint)
        .timeout(Duration::from_secs(4))
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;
    let model = v.pointer("/data/0/id").and_then(Value::as_str).unwrap_or("rizzo");
    Ok(crate::tr!("Rizzo Flow risponde ✓ ({model})", "Rizzo Flow is answering ✓ ({model})"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn heuristic_router() {
        assert!(heuristic("metti A Cruel Angel's Thesis", false).needs_tools);
        assert!(heuristic("What's the weather today in Rome?", false).needs_tools);
        let r = heuristic("ciao! come stai?", false);
        assert!(!r.needs_tools && r.simple);
        assert!(!heuristic("spiegami perché il cielo è blu", false).simple);
        assert!(heuristic("grazie", true).needs_tools); // il turno prima usava strumenti
    }

    #[test]
    fn loop_compaction() {
        let mut h = vec![
            ChatMessage::new("user", "domanda"),
            ChatMessage { role: "tool".into(), content: "x".repeat(9000), ..Default::default() },
            ChatMessage { role: "tool".into(), content: "y".repeat(5000), ..Default::default() },
            ChatMessage { role: "tool".into(), content: "z".repeat(9000), ..Default::default() },
        ];
        assert!(!compact_tool_results(&mut h, 1, 2, 12_000)); // sotto la soglia
        assert!(compact_tool_results(&mut h, 1, 3, 12_000));
        assert!(h[1].content.len() < 1200 && h[1].content.contains("caratteri tolti"));
        assert_eq!(h[3].content.len(), 9000); // l'ultimo passaggio resta intero
    }

    #[test]
    fn group_words() {
        assert_eq!(pick_groups("cerca le notizie di oggi"), ["app", "web"]);
        assert_eq!(pick_groups("apri il terminale e controlla lo spazio su disco"), ["app", "sistema"]);
        assert_eq!(pick_groups("quanti clienti ci sono nella tabella del database?"), ["app", "database"]);
        assert!(pick_groups("grazie mille").is_empty());
    }

    #[test]
    fn rizzo_answer() {
        let v = json!({"answers": {"needs_action": {"type": "noul", "noul": 0.92}, "complexity": {"type": "score", "score": 0.1}}});
        assert_eq!(parse_rizzo(&v, false), Some(Route { needs_tools: true, simple: false }));
        let v = json!({"answers": {"needs_action": {"type": "noul", "noul": 0.04}, "complexity": {"type": "score", "score": 0.2}}});
        assert_eq!(parse_rizzo(&v, false), Some(Route { needs_tools: false, simple: true }));
        assert_eq!(parse_rizzo(&json!({}), false), None);
    }

    #[test]
    fn compact_trim_facts() {
        let tools = json!([{ "type": "function", "function": { "name": "x", "description": "Fa una cosa. Dettagli lunghi.",
            "parameters": { "type": "object", "properties": { "q": { "type": "string", "description": "lunga" } } } } }]);
        let c = compact_tools(&tools);
        assert_eq!(c[0]["function"]["description"], "Fa una cosa");
        assert!(c[0]["function"]["parameters"]["properties"]["q"].get("description").is_none());
        let mut h = vec![ChatMessage::new("assistant", &"a".repeat(1000)), ChatMessage::new("user", "x"), ChatMessage::new("assistant", &"b".repeat(1000))];
        trim_history(&mut h, 2, 100);
        assert_eq!(h[0].content.chars().count(), 101);
        assert_eq!(h[2].content.len(), 1000);
        let facts = vec!["Ama il caffè".to_string(), "Gioca a Danganronpa".into(), "Vive a Milano".into()];
        assert_eq!(pick_facts(facts.clone(), "che tempo fa a Milano?", 1), vec!["Vive a Milano".to_string()]);
        assert_eq!(pick_facts(facts.clone(), "x", 0).len(), 3);
    }
}
