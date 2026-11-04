//! Music Venue: brani ascoltati di recente e preferiti, in `music_library.json`.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Quanti brani recenti si tengono.
const MAX_RECENT: usize = 60;

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(default)]
pub struct Entry {
    pub id: String,
    pub title: String,
    pub artist: String,
    pub duration: f64,
    pub thumbnail: Option<String>,
    pub url: Option<String>,
    /// Ultimo ascolto (unix, s).
    pub played: i64,
    pub plays: u32,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(default)]
pub struct Library {
    pub recent: Vec<Entry>,
    pub favorites: Vec<Entry>,
}

pub fn path(data_dir: &Path) -> PathBuf {
    data_dir.join("music_library.json")
}

pub fn load(data_dir: &Path) -> Library {
    std::fs::read_to_string(path(data_dir)).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
}

pub fn save(data_dir: &Path, lib: &Library) {
    let p = path(data_dir);
    let tmp = p.with_extension("json.tmp");
    if let Ok(bytes) = serde_json::to_vec_pretty(lib) {
        if std::fs::write(&tmp, bytes).is_ok() {
            let _ = std::fs::rename(&tmp, &p);
        }
    }
}

fn now() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

impl Library {
    /// Un brano è partito: va in cima ai recenti.
    pub fn played(&mut self, mut e: Entry) {
        let plays = self.recent.iter().find(|r| r.id == e.id).map(|r| r.plays).unwrap_or(0);
        self.recent.retain(|r| r.id != e.id);
        e.played = now();
        e.plays = plays + 1;
        // i preferiti restano aggiornati (titolo, copertina)
        for f in self.favorites.iter_mut().filter(|f| f.id == e.id) {
            f.title = e.title.clone();
            f.artist = e.artist.clone();
            f.thumbnail = e.thumbnail.clone();
            f.plays = e.plays;
            f.played = e.played;
        }
        self.recent.insert(0, e);
        self.recent.truncate(MAX_RECENT);
    }

    pub fn is_favorite(&self, id: &str) -> bool {
        self.favorites.iter().any(|f| f.id == id)
    }

    /// Aggiunge o toglie dai preferiti. Restituisce il nuovo stato.
    pub fn toggle_favorite(&mut self, e: Entry) -> bool {
        if self.is_favorite(&e.id) {
            self.favorites.retain(|f| f.id != e.id);
            false
        } else {
            self.favorites.insert(0, e);
            true
        }
    }

    pub fn find(&self, id: &str) -> Option<&Entry> {
        self.favorites.iter().chain(self.recent.iter()).find(|e| e.id == id)
    }

    pub fn remove_recent(&mut self, id: &str) {
        self.recent.retain(|e| e.id != id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn e(id: &str) -> Entry {
        Entry { id: id.into(), title: id.into(), ..Default::default() }
    }

    #[test]
    fn recent_and_favorites() {
        let mut l = Library::default();
        l.played(e("a"));
        l.played(e("b"));
        l.played(e("a"));
        assert_eq!(l.recent.iter().map(|x| x.id.as_str()).collect::<Vec<_>>(), ["a", "b"]);
        assert_eq!(l.recent[0].plays, 2);
        assert!(l.toggle_favorite(e("b")));
        assert!(l.is_favorite("b"));
        assert!(!l.toggle_favorite(e("b")));
        for i in 0..100 {
            l.played(e(&i.to_string()));
        }
        assert_eq!(l.recent.len(), MAX_RECENT);
    }
}
