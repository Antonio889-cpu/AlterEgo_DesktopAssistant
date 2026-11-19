//! Memoria a strati e a grafo (stile vault di Obsidian): ricordi piccoli (nodi) collegati tra loro,
//! cercati solo quando servono invece di finire tutti nel prompt.
//!
//! - **nodi**: fatti sull'utente, persone e cose nominate (entità), riassunti delle conversazioni passate
//!   (episodi), ricordi dei giochi; ognuno con titolo, testo e uso.
//! - **collegamenti**: un fatto che nomina "Sayaka" è collegato al nodo "Sayaka", e da lì agli altri
//!   fatti su di lei: cercando una cosa arrivano anche quelle vicine (a un passo).
//! - **ricerca**: indice full-text SQLite (FTS5, bm25) + vicini nel grafo + uso e recentezza, entro un
//!   budget di caratteri deciso per ogni domanda (Rizzo Flow o parole chiave).
//! - **indice**: una riga con i titoli che restano fuori, così il modello sa cosa può chiedere con
//!   `[[RECALL: …]]` (vedi assistant): niente strumento in più, la lista strumenti resta in cache.
//!
//! I fatti e i riassunti restano anche nelle loro tabelle (pannello Memoria): qui se ne tiene una copia
//! sincronizzata. `export_vault` scrive tutto come cartella di note Markdown con [[link]] (Obsidian).

use super::{e, now, Memory, R};
use rusqlite::{params, Connection};
use std::collections::{HashMap, HashSet};
use std::path::Path;

pub const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS nodes (
    id INTEGER PRIMARY KEY, kind TEXT NOT NULL, title TEXT NOT NULL, body TEXT NOT NULL,
    updated INTEGER NOT NULL, uses INTEGER NOT NULL DEFAULT 0, UNIQUE(kind, title));
CREATE TABLE IF NOT EXISTS edges (
    src INTEGER NOT NULL, dst INTEGER NOT NULL, rel TEXT NOT NULL, PRIMARY KEY(src, dst, rel));
CREATE INDEX IF NOT EXISTS edges_dst ON edges(dst);
CREATE VIRTUAL TABLE IF NOT EXISTS nodes_fts USING fts5(
    title, body, content='nodes', content_rowid='id', tokenize='unicode61 remove_diacritics 2');
CREATE TRIGGER IF NOT EXISTS nodes_ai AFTER INSERT ON nodes BEGIN
    INSERT INTO nodes_fts(rowid, title, body) VALUES (new.id, new.title, new.body); END;
CREATE TRIGGER IF NOT EXISTS nodes_ad AFTER DELETE ON nodes BEGIN
    INSERT INTO nodes_fts(nodes_fts, rowid, title, body) VALUES ('delete', old.id, old.title, old.body);
    DELETE FROM edges WHERE src = old.id OR dst = old.id; END;
CREATE TRIGGER IF NOT EXISTS nodes_au AFTER UPDATE OF title, body ON nodes BEGIN
    INSERT INTO nodes_fts(nodes_fts, rowid, title, body) VALUES ('delete', old.id, old.title, old.body);
    INSERT INTO nodes_fts(rowid, title, body) VALUES (new.id, new.title, new.body); END;
";

/// Spazio fisso per il "nucleo" di fatti principali (vedi `recall`).
const CORE_CHARS: usize = 350;

/// Tipi di nodo.
pub const FACT: &str = "fact";
pub const ENTITY: &str = "entity";
pub const EPISODE: &str = "episode";
pub const GAME: &str = "game";

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct Node {
    pub id: i64,
    pub kind: String,
    pub title: String,
    pub body: String,
    pub updated: i64,
    pub uses: i64,
}

/// Parole che non dicono niente (non diventano termini di ricerca né entità).
const STOP: &[&str] = &[
    "alter", "ego", "the", "and", "you", "your", "what", "when", "where", "which", "with", "that", "this", "have", "about", "from",
    "che", "chi", "come", "cosa", "dove", "quando", "quale", "sono", "sei", "per", "con", "del", "della", "dei", "delle", "una", "uno",
    "gli", "nel", "nella", "sul", "sulla", "mio", "mia", "tuo", "tua", "suo", "sua", "anche", "ancora", "molto", "tutto", "tutti",
    "fatto", "fare", "detto", "dire", "ricordi", "ricordo", "remember", "sai", "know", "user", "utente", "piace", "likes",
];

/// Termini di ricerca di una frase (minuscoli, senza parole vuote).
pub fn terms(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for w in text.split(|c: char| !c.is_alphanumeric()) {
        let w = w.to_lowercase();
        if w.chars().count() < 3 || STOP.contains(&w.as_str()) || w.chars().all(|c| c.is_ascii_digit()) {
            continue;
        }
        if !out.contains(&w) {
            out.push(w);
        }
    }
    out.truncate(16);
    out
}

/// Nomi propri in un testo (parole con la maiuscola non a inizio frase, o sigle): diventano entità.
pub fn entities(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut sentence_start = true;
    for raw in text.split_whitespace() {
        let w: String = raw.trim_matches(|c: char| !c.is_alphanumeric()).to_string();
        let ends = raw.ends_with(['.', '!', '?', ':']);
        let mut chars = w.chars();
        let first_up = chars.next().is_some_and(|c| c.is_uppercase());
        let acronym = w.chars().count() >= 2 && w.chars().all(|c| c.is_uppercase() || c.is_ascii_digit()) && w.chars().any(|c| c.is_alphabetic());
        if (first_up && !sentence_start || acronym) && w.chars().count() >= 3 && !STOP.contains(&w.to_lowercase().as_str()) && !out.contains(&w) {
            out.push(w);
        }
        sentence_start = ends;
    }
    out.truncate(8);
    out
}

/// Query FTS5: termini a prefisso in OR (ogni termine tra virgolette: niente sintassi iniettata).
/// Le parole lunghe si accorciano a 5 lettere: "programmi" trova anche "programma", "programmare".
fn fts_query(terms: &[String]) -> String {
    terms
        .iter()
        .map(|t| {
            let t: String = t.replace('"', "");
            let stem: String = if t.chars().count() > 5 { t.chars().take(5).collect() } else { t };
            format!("\"{stem}\"*")
        })
        .collect::<Vec<_>>()
        .join(" OR ")
}

fn row_node(r: &rusqlite::Row) -> rusqlite::Result<Node> {
    Ok(Node { id: r.get(0)?, kind: r.get(1)?, title: r.get(2)?, body: r.get(3)?, updated: r.get(4)?, uses: r.get(5)? })
}

fn upsert(c: &Connection, kind: &str, title: &str, body: &str) -> rusqlite::Result<i64> {
    c.execute(
        "INSERT INTO nodes(kind, title, body, updated) VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(kind, title) DO UPDATE SET body = excluded.body, updated = excluded.updated
         WHERE nodes.body <> excluded.body",
        params![kind, title, body, now()],
    )?;
    c.query_row("SELECT id FROM nodes WHERE kind = ?1 AND title = ?2", params![kind, title], |r| r.get(0))
}

fn link(c: &Connection, a: i64, b: i64, rel: &str) -> rusqlite::Result<()> {
    if a != b {
        c.execute("INSERT OR IGNORE INTO edges(src, dst, rel) VALUES (?1, ?2, ?3)", params![a, b, rel])?;
    }
    Ok(())
}

/// Titolo di un nodo nato da un testo lungo (fatto, riassunto): l'inizio, tagliato a parola.
fn title_of(text: &str, max: usize) -> String {
    let t = text.trim();
    if t.chars().count() <= max {
        return t.to_string();
    }
    let cut: String = t.chars().take(max).collect();
    match cut.rfind(' ') {
        Some(i) if i > max / 2 => format!("{}…", &cut[..i]),
        _ => format!("{cut}…"),
    }
}

/// Collega un nodo alle entità che nomina (creandole se mancano).
fn link_entities(c: &Connection, id: i64, text: &str) -> rusqlite::Result<()> {
    for name in entities(text) {
        let ent = upsert(c, ENTITY, &name, &name)?;
        link(c, id, ent, "nomina")?;
    }
    Ok(())
}

/// Collega i nodi `ids` alle entità già note che nominano (anche a inizio frase, dove la maiuscola
/// non basta a riconoscere un nome).
fn link_known(c: &Connection, ids: &[i64]) -> rusqlite::Result<()> {
    let ents: Vec<(i64, String)> = {
        let mut st = c.prepare("SELECT id, title FROM nodes WHERE kind = 'entity'")?;
        let rows = st.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?;
        rows.filter_map(Result::ok).collect()
    };
    if ents.is_empty() {
        return Ok(());
    }
    let mut st = c.prepare("SELECT title || ' ' || body FROM nodes WHERE id = ?1")?;
    for id in ids {
        let text: String = st.query_row([id], |r| r.get(0))?;
        let words: HashSet<&str> = text.split(|ch: char| !ch.is_alphanumeric()).collect();
        for (ent, name) in &ents {
            if words.contains(name.as_str()) {
                link(c, *id, *ent, "nomina")?;
            }
        }
    }
    Ok(())
}

/// Ricordo trovato, con il punteggio usato per sceglierlo.
#[derive(Debug, Clone)]
pub struct Hit {
    pub node: Node,
    pub score: f64,
}

impl Memory {
    /// Rende la tabella `kind` uguale a `items` (testo per nodo): aggiunge, aggiorna, toglie.
    fn sync_kind(&self, kind: &str, items: &[String], title_len: usize) -> R<()> {
        let mut c = self.conn.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        let tx = c.transaction().map_err(e)?;
        let mut keep: HashSet<i64> = HashSet::new();
        for it in items {
            let it = it.trim();
            if it.is_empty() {
                continue;
            }
            // titolo = inizio del testo; due testi con lo stesso inizio si distinguono col numero
            let mut title = title_of(it, title_len);
            let mut n = 2;
            while let Ok(id) = tx.query_row("SELECT id FROM nodes WHERE kind = ?1 AND title = ?2 AND body <> ?3", params![kind, title, it], |r| r.get::<_, i64>(0)) {
                if keep.contains(&id) {
                    title = format!("{} ({n})", title_of(it, title_len));
                    n += 1;
                } else {
                    break;
                }
            }
            let id = upsert(&tx, kind, &title, it).map_err(e)?;
            link_entities(&tx, id, it).map_err(e)?;
            keep.insert(id);
        }
        let ids: Vec<i64> = {
            let mut st = tx.prepare("SELECT id FROM nodes WHERE kind = ?1").map_err(e)?;
            let rows = st.query_map([kind], |r| r.get::<_, i64>(0)).map_err(e)?;
            rows.filter_map(Result::ok).collect()
        };
        for id in ids.into_iter().filter(|id| !keep.contains(id)) {
            tx.execute("DELETE FROM nodes WHERE id = ?1", [id]).map_err(e)?;
        }
        let kept: Vec<i64> = keep.iter().copied().collect();
        link_known(&tx, &kept).map_err(e)?;
        // entità rimaste senza collegamenti
        tx.execute("DELETE FROM nodes WHERE kind = 'entity' AND id NOT IN (SELECT dst FROM edges) AND id NOT IN (SELECT src FROM edges)", []).map_err(e)?;
        tx.commit().map_err(e)
    }

    /// Risincronizza il grafo se fatti o riassunti sono cambiati (o al primo uso: migrazione).
    pub fn sync_if_needed(&self) -> R<()> {
        if self.graph_dirty.swap(false, std::sync::atomic::Ordering::Relaxed) {
            if let Err(err) = self.sync_graph() {
                self.graph_dirty.store(true, std::sync::atomic::Ordering::Relaxed);
                return Err(err);
            }
        }
        Ok(())
    }

    /// Copia nel grafo i fatti e i riassunti delle tabelle normali (dopo ogni cambiamento).
    pub fn sync_graph(&self) -> R<()> {
        let facts: Vec<String> = self.facts()?.into_iter().map(|f| f.content).collect();
        self.sync_kind(FACT, &facts, 60)?;
        let eps: Vec<String> = self.summaries()?.into_iter().map(|s| s.content).collect();
        // l'ultimo riassunto va già nel prompt: nel grafo servono quelli vecchi
        let old: Vec<String> = eps.iter().skip(1).cloned().collect();
        self.sync_kind(EPISODE, &old, 50)
    }

    /// Aggiunge o aggiorna un ricordo di un altro tipo (es. dai giochi), collegato alle entità che nomina.
    pub fn remember_node(&self, kind: &str, title: &str, body: &str) -> R<i64> {
        let c = self.conn.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        let id = upsert(&c, kind, title.trim(), body.trim()).map_err(e)?;
        link_entities(&c, id, &format!("{title} {body}")).map_err(e)?;
        link_known(&c, &[id]).map_err(e)?;
        Ok(id)
    }

    pub fn node_count(&self) -> R<i64> {
        self.sync_if_needed()?;
        let c = self.conn.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        c.query_row("SELECT COUNT(*) FROM nodes", [], |r| r.get(0)).map_err(e)
    }

    /// I ricordi più utili per `query` entro `budget` caratteri: ricerca full-text, poi i vicini nel
    /// grafo (a un passo, con mezzo punteggio), poi uso e recentezza. Le entità servono da ponte:
    /// non vengono restituite da sole. Senza termini utili: i fatti più usati e recenti.
    pub fn recall(&self, query: &str, budget: usize) -> R<Vec<Hit>> {
        if budget == 0 {
            return Ok(Vec::new());
        }
        self.sync_if_needed()?;
        let c = self.conn.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        let t = terms(query);
        let mut score: HashMap<i64, f64> = HashMap::new();
        if !t.is_empty() {
            let mut st = c
                .prepare("SELECT rowid, bm25(nodes_fts, 3.0, 1.0) FROM nodes_fts WHERE nodes_fts MATCH ?1 ORDER BY 2 LIMIT 40")
                .map_err(e)?;
            let rows = st.query_map([fts_query(&t)], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, f64>(1)?))).map_err(e)?;
            for (id, bm) in rows.filter_map(Result::ok) {
                // bm25 di SQLite è negativo: più è basso, meglio è
                score.insert(id, -bm);
            }
            // vicini: un passo, più un passo in più attraverso le entità (fatto → Sayaka → altri fatti)
            let mut near_st = c.prepare("SELECT dst FROM edges WHERE src = ?1 UNION SELECT src FROM edges WHERE dst = ?1").map_err(e)?;
            let mut kind_st = c.prepare("SELECT kind FROM nodes WHERE id = ?1").map_err(e)?;
            let mut frontier: Vec<(i64, f64)> = score.iter().map(|(k, v)| (*k, *v)).collect();
            for hop in 0..2 {
                let mut next = Vec::new();
                for (id, s) in frontier {
                    if hop == 1 && kind_st.query_row([id], |r| r.get::<_, String>(0)).map_err(e)? != ENTITY {
                        continue;
                    }
                    let near: Vec<i64> = near_st.query_map([id], |r| r.get(0)).map_err(e)?.filter_map(Result::ok).collect();
                    for n in near {
                        let v = score.entry(n).or_insert(0.0);
                        if s * 0.5 > *v {
                            *v = s * 0.5;
                            next.push((n, *v));
                        }
                    }
                }
                frontier = next;
            }
        }
        let mut nodes: Vec<Node> = Vec::new();
        if t.is_empty() {
            let mut st = c
                .prepare("SELECT id, kind, title, body, updated, uses FROM nodes WHERE kind = 'fact' ORDER BY uses DESC, updated DESC LIMIT 12")
                .map_err(e)?;
            nodes = st.query_map([], row_node).map_err(e)?.filter_map(Result::ok).collect();
            let n = nodes.len() as f64;
            for (i, nd) in nodes.iter().enumerate() {
                score.insert(nd.id, (n - i as f64) / n.max(1.0) * 0.1);
            }
        } else {
            let mut st = c.prepare("SELECT id, kind, title, body, updated, uses FROM nodes WHERE id = ?1").map_err(e)?;
            for id in score.keys() {
                if let Ok(n) = st.query_row([id], row_node) {
                    nodes.push(n);
                }
            }
        }
        let t_now = now();
        let mut hits: Vec<Hit> = nodes
            .into_iter()
            .filter(|n| n.kind != ENTITY)
            .map(|n| {
                let days = ((t_now - n.updated).max(0) as f64) / 86_400.0;
                let s = score.get(&n.id).copied().unwrap_or(0.0) * (1.0 + (n.uses as f64).ln_1p() * 0.15) / (1.0 + days / 180.0);
                Hit { node: n, score: s }
            })
            .collect();
        hits.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal).then(b.node.updated.cmp(&a.node.updated)));
        let mut used = 0;
        let mut out: Vec<Hit> = Vec::new();
        for h in hits {
            let len = h.node.body.chars().count() + 3;
            if used + len > budget {
                continue;
            }
            used += len;
            out.push(h);
        }
        // nucleo: i fatti più usati e recenti riempiono un piccolo spazio fisso, così anche nelle
        // chiacchiere Alter Ego non dimentica le cose principali
        let core = CORE_CHARS.min(budget.saturating_sub(used));
        if core > 0 && !t.is_empty() {
            let mut st = c
                .prepare("SELECT id, kind, title, body, updated, uses FROM nodes WHERE kind = 'fact' ORDER BY uses DESC, updated DESC LIMIT 12")
                .map_err(e)?;
            let top: Vec<Node> = st.query_map([], row_node).map_err(e)?.filter_map(Result::ok).collect();
            let mut left = core;
            for n in top {
                let len = n.body.chars().count() + 3;
                if out.iter().any(|h: &Hit| h.node.id == n.id) || len > left {
                    continue;
                }
                left -= len;
                out.push(Hit { node: n, score: 0.0 });
            }
        }
        // ordine stabile nel prompt: per tipo e poi per id (meno variazioni da un messaggio all'altro)
        out.sort_by(|a, b| a.node.kind.cmp(&b.node.kind).then(a.node.id.cmp(&b.node.id)));
        let ids: Vec<i64> = out.iter().map(|h| h.node.id).collect();
        for id in ids {
            let _ = c.execute("UPDATE nodes SET uses = uses + 1 WHERE id = ?1", [id]);
        }
        Ok(out)
    }

    /// Titoli di ciò che è rimasto fuori (entità più collegate, poi episodi): la riga "indice" del prompt.
    pub fn index(&self, exclude: &[i64], max: usize) -> R<Vec<String>> {
        self.sync_if_needed()?;
        let c = self.conn.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut st = c
            .prepare(
                "SELECT n.id, n.title FROM nodes n LEFT JOIN edges ed ON ed.dst = n.id
                 WHERE n.kind IN ('entity', 'game') GROUP BY n.id ORDER BY COUNT(ed.src) DESC, n.updated DESC LIMIT ?1",
            )
            .map_err(e)?;
        let rows = st.query_map([(max + exclude.len()) as i64], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?))).map_err(e)?;
        Ok(rows.filter_map(Result::ok).filter(|(id, _)| !exclude.contains(id)).map(|(_, t)| t).take(max).collect())
    }

    /// Scrive la memoria come vault di note Markdown con [[link]] (si apre con Obsidian).
    pub fn export_vault(&self, dir: &Path) -> R<usize> {
        self.sync_if_needed()?;
        let c = self.conn.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut st = c.prepare("SELECT id, kind, title, body, updated, uses FROM nodes").map_err(e)?;
        let nodes: Vec<Node> = st.query_map([], row_node).map_err(e)?.filter_map(Result::ok).collect();
        let names: HashMap<i64, String> = nodes.iter().map(|n| (n.id, file_name(&n.kind, &n.title))).collect();
        let mut es = c.prepare("SELECT dst FROM edges WHERE src = ?1 UNION SELECT src FROM edges WHERE dst = ?1").map_err(e)?;
        std::fs::create_dir_all(dir).map_err(|x| x.to_string())?;
        for n in &nodes {
            let near: Vec<i64> = es.query_map([n.id], |r| r.get(0)).map_err(e)?.filter_map(Result::ok).collect();
            let mut md = format!("---\ntipo: {}\naggiornato: {}\n---\n\n{}\n", n.kind, n.updated, n.body);
            if !near.is_empty() {
                md.push_str("\n## Collegamenti\n");
                for id in near {
                    if let Some(f) = names.get(&id) {
                        md.push_str(&format!("- [[{f}]]\n"));
                    }
                }
            }
            std::fs::write(dir.join(format!("{}.md", names[&n.id])), md).map_err(|x| x.to_string())?;
        }
        Ok(nodes.len())
    }
}

/// Nome di file valido su Windows e Linux per una nota.
fn file_name(kind: &str, title: &str) -> String {
    let clean: String = title.chars().map(|c| if "\\/:*?\"<>|[]#^".contains(c) || c.is_control() { ' ' } else { c }).collect();
    let clean = clean.split_whitespace().collect::<Vec<_>>().join(" ");
    let short: String = clean.trim_end_matches('…').chars().take(60).collect();
    format!("{kind} - {}", short.trim())
}

/// Testo dei ricordi per il prompt (una riga per ricordo, poche parole di contorno).
pub fn render(hits: &[Hit]) -> String {
    let mut s = String::new();
    for h in hits {
        let tag = match h.node.kind.as_str() {
            FACT => "",
            EPISODE => "(past chat) ",
            GAME => "(game) ",
            _ => "",
        };
        s.push_str(&format!("- {tag}{}\n", h.node.body.replace('\n', " ")));
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Solo i ricordi trovati dalla ricerca (senza il nucleo di fatti principali).
    fn found(h: Vec<Hit>) -> Vec<Hit> {
        h.into_iter().filter(|x| x.score > 0.0).collect()
    }

    fn mem_with(facts: &[&str]) -> Memory {
        let m = Memory::in_memory();
        for f in facts {
            m.add_fact(f).unwrap();
        }
        m
    }

    #[test]
    fn terms_and_entities() {
        assert_eq!(terms("Come sta Sayaka? E il progetto Rust?"), vec!["sta", "sayaka", "progetto", "rust"]);
        let e = entities("Il fratello si chiama Makoto e lavora a Tokyo. Ama la NASA.");
        assert!(e.contains(&"Makoto".to_string()) && e.contains(&"Tokyo".to_string()) && e.contains(&"NASA".to_string()));
        assert!(!e.contains(&"Il".to_string()) && !e.contains(&"Ama".to_string()), "{e:?}");
    }

    #[test]
    fn fts5_is_available_and_finds_facts() {
        let m = mem_with(&["Ama il caffè senza zucchero", "Programma in Rust e TypeScript", "La sorella si chiama Sayaka e suona il pianoforte"]);
        let h = found(m.recall("che linguaggi programmi?", 500).unwrap());
        assert_eq!(h.len(), 1);
        assert_eq!(h[0].node.body, "Programma in Rust e TypeScript");
        // accenti e prefissi
        assert!(m.recall("caffe", 500).unwrap().iter().any(|x| x.node.body.contains("caffè")));
        assert!(m.recall("pianofor", 500).unwrap().iter().any(|x| x.node.body.contains("Sayaka")));
    }

    #[test]
    fn graph_brings_neighbours_through_entities() {
        let m = mem_with(&[
            "La sorella si chiama Sayaka",
            "Sayaka studia medicina a Bologna",
            "Sayaka odia i ragni",
            "Gioca a Minecraft il sabato",
        ]);
        // "sorella" trova il primo fatto; Sayaka (entità) porta gli altri due, non quello su Minecraft
        let h = found(m.recall("come sta mia sorella?", 1000).unwrap());
        let bodies: Vec<&str> = h.iter().map(|x| x.node.body.as_str()).collect();
        assert!(bodies.contains(&"Sayaka studia medicina a Bologna") && bodies.contains(&"Sayaka odia i ragni"), "{bodies:?}");
        assert!(!bodies.iter().any(|b| b.contains("Minecraft")));
        assert!(h.iter().all(|x| x.node.kind != ENTITY), "le entità non vanno nel prompt");
    }

    #[test]
    fn budget_and_fallback() {
        let facts: Vec<String> = (0..60).map(|i| format!("Fatto numero {i} sulle abitudini dell'utente")).collect();
        let refs: Vec<&str> = facts.iter().map(String::as_str).collect();
        let m = mem_with(&refs);
        let h = m.recall("abitudini", 200).unwrap();
        assert!(!h.is_empty() && render(&h).chars().count() <= 220, "rispetta il budget");
        // domanda senza termini utili: i fatti più usati/recenti, entro il budget
        let h = m.recall("ok!", 300).unwrap();
        assert!(!h.is_empty() && render(&h).chars().count() <= 320);
        assert!(m.recall("qualsiasi", 0).unwrap().is_empty());
    }

    #[test]
    fn sync_follows_the_tables() {
        let m = mem_with(&["Lavora come infermiere a Napoli", "Ha un gatto di nome Byakuya"]);
        let n0 = m.node_count().unwrap();
        assert!(n0 >= 4, "2 fatti + entità: {n0}");
        let ids: Vec<i64> = m.facts().unwrap().iter().map(|f| f.id).collect();
        m.delete_fact(ids[1]).unwrap(); // il grafo si aggiorna da solo alla prossima ricerca
        assert!(found(m.recall("gatto", 500).unwrap()).is_empty(), "fatto cancellato anche dal grafo (e dall'indice FTS)");
        assert!(!render(&m.recall("gatto", 500).unwrap()).contains("Byakuya"));
        assert!(!m.index(&[], 10).unwrap().contains(&"Byakuya".to_string()), "entità orfana tolta");
        // risincronizzare non duplica
        m.sync_graph().unwrap();
        m.sync_graph().unwrap();
        assert!(m.node_count().unwrap() < n0);
    }

    #[test]
    fn index_lists_what_was_left_out() {
        let m = mem_with(&["Il migliore amico è Leon", "Leon gioca a basket", "Il capo si chiama Celestia"]);
        let idx = m.index(&[], 10).unwrap();
        assert_eq!(idx.first().map(String::as_str), Some("Leon"), "le entità più collegate prima: {idx:?}");
        assert!(idx.contains(&"Celestia".to_string()));
    }

    #[test]
    fn game_nodes_and_vault_export() {
        let m = mem_with(&["Il nickname su Steam è Hope"]);
        m.remember_node(GAME, "Garry's Mod: torre di divani", "Abbiamo costruito una torre di divani con Hope e Mondo").unwrap();
        assert!(m.recall("divani", 500).unwrap().iter().any(|h| h.node.kind == GAME));
        let dir = std::env::temp_dir().join(format!("ae-vault-{}", std::process::id()));
        let n = m.export_vault(&dir).unwrap();
        assert!(n >= 3);
        let note = std::fs::read_to_string(dir.join("game - Garry's Mod torre di divani.md")).unwrap();
        assert!(note.contains("[[entity - Mondo]]") && note.contains("tipo: game"), "{note}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Misura: prompt con tutti i fatti (com'era) contro memoria a grafo, su domande diverse.
    /// `cargo test token_savings -- --nocapture` stampa i numeri.
    #[test]
    fn token_savings() {
        let mut facts: Vec<String> = vec![
            "La sorella si chiama Sayaka e studia medicina a Bologna".into(),
            "Sayaka ha paura dei ragni".into(),
            "Lavora come sviluppatore backend in Rust".into(),
            "Il gatto si chiama Byakuya ed è nero".into(),
            "Il migliore amico è Leon, gioca a basket".into(),
            "Sta preparando un viaggio in Giappone ad aprile".into(),
        ];
        let topics = ["caffè", "serie TV", "palestra", "colazione", "musica lo-fi", "tastiere meccaniche", "giardinaggio", "fotografia",
            "cucina", "scacchi", "film horror", "podcast", "bicicletta", "anime", "manga", "giochi da tavolo"];
        for (i, tpc) in topics.iter().cycle().take(74).enumerate() {
            facts.push(format!("Preferenza {i}: gli piace parlare di {tpc} la sera dopo il lavoro"));
        }
        let refs: Vec<&str> = facts.iter().map(String::as_str).collect();
        let m = mem_with(&refs);
        let all: usize = facts.iter().map(|f| f.chars().count() + 3).sum();
        // (domanda, quanto serve la memoria, parola che deve esserci nei ricordi o almeno nell'indice)
        let cases = [
            ("ciao, come va?", 0.2, None),
            ("come sta mia sorella?", 0.7, Some("Sayaka")),
            ("ho un dubbio su un errore del borrow checker", 0.4, Some("Rust")),
            ("cosa mi consigli di vedere in Giappone?", 0.7, Some("Giappone")),
        ];
        for (q, need, must) in cases {
            let h = m.recall(q, crate::saving::memory_budget(need)).unwrap();
            let txt = render(&h);
            let idx = m.index(&[], 10).unwrap().join(", ");
            let n = txt.chars().count() + idx.chars().count();
            eprintln!("«{q}»: {n} caratteri invece di {all} ({}% in meno)", 100 - n * 100 / all);
            assert!(n * 3 < all, "almeno 3 volte meno: {n} vs {all}");
            if let Some(w) = must {
                assert!(txt.contains(w) || idx.contains(w), "manca il ricordo giusto per «{q}»: {txt} | {idx}");
            }
        }
        assert!(!render(&m.recall("ciao, come va?", crate::saving::memory_budget(0.2)).unwrap()).is_empty(), "nucleo anche nelle chiacchiere");
    }

    #[test]
    fn fts_query_is_safe() {
        assert_eq!(fts_query(&["ab\"c".into(), "programmi".into()]), "\"abc\"* OR \"progr\"*");
        let m = mem_with(&["Usa NEAR e OR come parole normali"]);
        assert!(m.recall("NEAR OR \"AND\" *", 500).is_ok(), "niente errori di sintassi FTS");
    }
}
