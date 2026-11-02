//! Memoria locale in SQLite: `%APPDATA%\AlterEgo\memory.db`.
//! Cronologia, riassunti periodici, fatti sull'utente, promemoria.
//! Punto di innesto per una memoria esterna (es. CyberAI): sostituisci questo modulo
//! mantenendo gli stessi metodi pubblici.

pub mod summarize;

use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use std::{
    path::{Path, PathBuf},
    sync::Mutex,
    time::{SystemTime, UNIX_EPOCH},
};

pub fn now() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

#[derive(Serialize, Clone, Debug)]
pub struct Message {
    pub id: i64,
    pub ts: i64,
    pub role: String,
    pub content: String,
}

#[derive(Serialize, Clone, Debug)]
pub struct Item {
    pub id: i64,
    pub ts: i64,
    pub content: String,
}

#[derive(Serialize)]
pub struct Overview {
    pub messages: i64,
    pub facts: Vec<Item>,
    pub summaries: Vec<Item>,
    pub db_path: String,
}

pub struct Memory {
    conn: Mutex<Connection>,
    pub path: PathBuf,
}

type R<T> = Result<T, String>;
fn e(err: rusqlite::Error) -> String {
    format!("errore memoria: {err}")
}

impl Memory {
    pub fn open(path: &Path) -> R<Self> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|err| err.to_string())?;
        }
        let conn = Connection::open(path).map_err(e)?;
        Self::init(conn, path)
    }

    #[cfg(test)]
    pub fn in_memory() -> Self {
        Self::init(Connection::open_in_memory().unwrap(), Path::new(":memory:")).unwrap()
    }

    fn init(conn: Connection, path: &Path) -> R<Self> {
        conn.execute_batch(
            "PRAGMA journal_mode=WAL;
             CREATE TABLE IF NOT EXISTS messages (
                id INTEGER PRIMARY KEY, ts INTEGER NOT NULL, role TEXT NOT NULL, content TEXT NOT NULL);
             CREATE TABLE IF NOT EXISTS summaries (
                id INTEGER PRIMARY KEY, ts INTEGER NOT NULL, upto_id INTEGER NOT NULL, content TEXT NOT NULL);
             CREATE TABLE IF NOT EXISTS facts (
                id INTEGER PRIMARY KEY, ts INTEGER NOT NULL, content TEXT NOT NULL UNIQUE COLLATE NOCASE);
             CREATE TABLE IF NOT EXISTS reminders (
                id INTEGER PRIMARY KEY, ts INTEGER NOT NULL, due INTEGER NOT NULL, text TEXT NOT NULL,
                done INTEGER NOT NULL DEFAULT 0);",
        )
        .map_err(e)?;
        Ok(Self { conn: Mutex::new(conn), path: path.to_path_buf() })
    }

    pub fn add_message(&self, role: &str, content: &str) -> R<i64> {
        let c = self.conn.lock().unwrap();
        c.execute("INSERT INTO messages (ts, role, content) VALUES (?1, ?2, ?3)", params![now(), role, content])
            .map_err(e)?;
        Ok(c.last_insert_rowid())
    }

    pub fn delete_message(&self, id: i64) -> R<()> {
        self.conn.lock().unwrap().execute("DELETE FROM messages WHERE id = ?1", [id]).map_err(e)?;
        Ok(())
    }

    /// Ultimi `n` messaggi, in ordine cronologico.
    pub fn recent(&self, n: usize) -> R<Vec<Message>> {
        let c = self.conn.lock().unwrap();
        let mut st = c
            .prepare("SELECT id, ts, role, content FROM messages ORDER BY id DESC LIMIT ?1")
            .map_err(e)?;
        let mut v: Vec<Message> = st
            .query_map([n as i64], |r| Ok(Message { id: r.get(0)?, ts: r.get(1)?, role: r.get(2)?, content: r.get(3)? }))
            .map_err(e)?
            .collect::<Result<_, _>>()
            .map_err(e)?;
        v.reverse();
        Ok(v)
    }

    pub fn latest_summary(&self) -> R<Option<(i64, String)>> {
        let c = self.conn.lock().unwrap();
        c.query_row("SELECT upto_id, content FROM summaries ORDER BY id DESC LIMIT 1", [], |r| Ok((r.get(0)?, r.get(1)?)))
            .optional()
            .map_err(e)
    }

    /// Messaggi non ancora riassunti e già usciti dalla finestra degli ultimi `window`.
    pub fn pending_for_summary(&self, window: usize) -> R<Vec<Message>> {
        let upto = self.latest_summary()?.map(|s| s.0).unwrap_or(0);
        let c = self.conn.lock().unwrap();
        let mut st = c
            .prepare(
                "SELECT id, ts, role, content FROM messages
                 WHERE id > ?1 AND id NOT IN (SELECT id FROM messages ORDER BY id DESC LIMIT ?2)
                 ORDER BY id",
            )
            .map_err(e)?;
        let v = st
            .query_map(params![upto, window as i64], |r| {
                Ok(Message { id: r.get(0)?, ts: r.get(1)?, role: r.get(2)?, content: r.get(3)? })
            })
            .map_err(e)?
            .collect::<Result<_, _>>()
            .map_err(e)?;
        Ok(v)
    }

    pub fn add_summary(&self, upto_id: i64, content: &str) -> R<()> {
        self.conn
            .lock()
            .unwrap()
            .execute("INSERT INTO summaries (ts, upto_id, content) VALUES (?1, ?2, ?3)", params![now(), upto_id, content])
            .map_err(e)?;
        Ok(())
    }

    pub fn facts(&self) -> R<Vec<Item>> {
        self.items("SELECT id, ts, content FROM facts ORDER BY id")
    }

    pub fn summaries(&self) -> R<Vec<Item>> {
        self.items("SELECT id, ts, content FROM summaries ORDER BY id DESC LIMIT 20")
    }

    fn items(&self, sql: &str) -> R<Vec<Item>> {
        let c = self.conn.lock().unwrap();
        let mut st = c.prepare(sql).map_err(e)?;
        let v = st
            .query_map([], |r| Ok(Item { id: r.get(0)?, ts: r.get(1)?, content: r.get(2)? }))
            .map_err(e)?
            .collect::<Result<_, _>>()
            .map_err(e)?;
        Ok(v)
    }

    /// Aggiunge un fatto; i duplicati (maiuscole ignorate) vengono scartati.
    pub fn add_fact(&self, content: &str) -> R<()> {
        let content = content.trim().trim_start_matches(['-', '*', '•']).trim();
        if content.len() < 3 {
            return Ok(());
        }
        self.conn
            .lock()
            .unwrap()
            .execute("INSERT OR IGNORE INTO facts (ts, content) VALUES (?1, ?2)", params![now(), content])
            .map_err(e)?;
        Ok(())
    }

    pub fn delete_fact(&self, id: i64) -> R<()> {
        self.conn.lock().unwrap().execute("DELETE FROM facts WHERE id = ?1", [id]).map_err(e)?;
        Ok(())
    }

    pub fn overview(&self) -> R<Overview> {
        let messages = self
            .conn
            .lock()
            .unwrap()
            .query_row("SELECT COUNT(*) FROM messages", [], |r| r.get(0))
            .map_err(e)?;
        Ok(Overview {
            messages,
            facts: self.facts()?,
            summaries: self.summaries()?,
            db_path: self.path.display().to_string(),
        })
    }

    /// "Dimentica tutto": svuota ogni tabella e compatta il file.
    pub fn forget_all(&self) -> R<()> {
        self.conn
            .lock()
            .unwrap()
            .execute_batch(
                "DELETE FROM messages; DELETE FROM summaries; DELETE FROM facts; DELETE FROM reminders; VACUUM;",
            )
            .map_err(e)
    }

    pub fn export(&self) -> R<serde_json::Value> {
        let all = self.recent(usize::MAX >> 1)?;
        Ok(serde_json::json!({
            "esportato_il": now(),
            "messaggi": all,
            "riassunti": self.items("SELECT id, ts, content FROM summaries ORDER BY id")?,
            "fatti": self.facts()?,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn window_and_summary() {
        let m = Memory::in_memory();
        for i in 0..10 {
            m.add_message(if i % 2 == 0 { "user" } else { "assistant" }, &format!("msg {i}")).unwrap();
        }
        let r = m.recent(4).unwrap();
        assert_eq!(r.iter().map(|x| x.content.as_str()).collect::<Vec<_>>(), ["msg 6", "msg 7", "msg 8", "msg 9"]);
        let p = m.pending_for_summary(4).unwrap();
        assert_eq!(p.len(), 6);
        m.add_summary(p.last().unwrap().id, "riassunto").unwrap();
        assert!(m.pending_for_summary(4).unwrap().is_empty());
        m.add_message("user", "nuovo").unwrap();
        assert_eq!(m.pending_for_summary(4).unwrap().len(), 1); // "msg 6" esce dalla finestra
    }

    #[test]
    fn facts_dedupe_and_forget() {
        let m = Memory::in_memory();
        m.add_fact("- Si chiama Makoto").unwrap();
        m.add_fact("si chiama makoto").unwrap();
        m.add_fact("x").unwrap();
        assert_eq!(m.facts().unwrap().len(), 1);
        assert_eq!(m.facts().unwrap()[0].content, "Si chiama Makoto");
        m.add_message("user", "ciao").unwrap();
        m.forget_all().unwrap();
        let o = m.overview().unwrap();
        assert_eq!((o.messages, o.facts.len()), (0, 0));
    }
}
