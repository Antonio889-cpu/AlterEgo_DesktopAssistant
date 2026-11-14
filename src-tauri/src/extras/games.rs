//! Riconosce un gioco della serie aperto dal titolo della finestra o dal nome del programma.
//! I titoli restano sul PC: non vengono mandati al modello AI.

/// Programmi che mostrano titoli qualsiasi (pagine web, video, chat): mai un gioco.
const NOT_GAMES: &[&str] = &[
    "chrome", "msedge", "firefox", "brave", "opera", "opera_gx", "vivaldi", "discord", "telegram", "whatsapp", "slack",
    "teams", "ms-teams", "obs64", "obs", "vlc", "mpc-hc64", "spotify", "explorer", "code", "notepad", "notepad++", "steam",
    "steamwebhelper", "alter-ego", "alter ego", "youtube", "twitch", "chromium", "librewolf", "zen", "thorium", "arc",
];

/// Eseguibili noti (senza estensione, minuscolo) → nome del gioco.
const EXES: &[(&str, &str)] = &[
    ("dr1_us", "Danganronpa: Trigger Happy Havoc"),
    ("dr2_us", "Danganronpa 2: Goodbye Despair"),
    ("dangan3win", "Danganronpa V3: Killing Harmony"),
    ("dr3", "Danganronpa V3: Killing Harmony"),
    ("drae", "Danganronpa Another Episode: Ultra Despair Girls"),
    ("dangan_s", "Danganronpa S: Ultimate Summer Camp"),
    ("danganronpas", "Danganronpa S: Ultimate Summer Camp"),
];

/// Nome dell'eseguibile senza cartella né estensione (separatori di Windows e Linux).
fn stem(path: &str) -> String {
    let name = path.rsplit(['/', '\\']).next().unwrap_or("").to_lowercase();
    match name.rsplit_once('.') {
        Some((s, ext)) if ext.len() <= 4 => s.to_string(),
        _ => name,
    }
}

/// Nome del gioco dal titolo (None se il titolo non è di un gioco della serie).
pub fn from_title(title: &str) -> Option<String> {
    let t = title.to_lowercase();
    let known: &[(&str, &str)] = &[
        ("ultra despair girls", "Danganronpa Another Episode: Ultra Despair Girls"),
        ("another episode", "Danganronpa Another Episode: Ultra Despair Girls"),
        ("summer camp", "Danganronpa S: Ultimate Summer Camp"),
        ("killing harmony", "Danganronpa V3: Killing Harmony"),
        ("danganronpa v3", "Danganronpa V3: Killing Harmony"),
        ("goodbye despair", "Danganronpa 2: Goodbye Despair"),
        ("danganronpa 2", "Danganronpa 2: Goodbye Despair"),
        ("trigger happy havoc", "Danganronpa: Trigger Happy Havoc"),
        ("decadence", "Danganronpa Decadence"),
        ("ニューダンガンロンパv3", "Danganronpa V3: Killing Harmony"),
        ("スーパーダンガンロンパ2", "Danganronpa 2: Goodbye Despair"),
        ("絶対絶望少女", "Danganronpa Another Episode: Ultra Despair Girls"),
    ];
    if let Some((_, g)) = known.iter().find(|(k, _)| t.contains(k)) {
        return Some(g.to_string());
    }
    (t.starts_with("danganronpa") || t.contains("ダンガンロンパ")).then(|| "Danganronpa".to_string())
}

/// Sceglie tra le finestre aperte (eseguibile, titolo).
pub fn pick(windows: &[(String, String)]) -> Option<String> {
    for (exe, title) in windows {
        let e = stem(exe);
        if NOT_GAMES.contains(&e.as_str()) {
            continue;
        }
        if let Some((_, g)) = EXES.iter().find(|(x, _)| *x == e) {
            return Some(g.to_string());
        }
        if let Some(g) = from_title(title) {
            return Some(g);
        }
    }
    None
}

/// Gioco della serie aperto adesso.
pub fn detect() -> Option<String> {
    pick(&crate::apps::titled())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognizes_games_not_browsers() {
        let w = |e: &str, t: &str| (e.to_string(), t.to_string());
        assert_eq!(pick(&[w("C:\\Games\\DR1_us.exe", "")]).as_deref(), Some("Danganronpa: Trigger Happy Havoc"));
        assert_eq!(pick(&[w("C:\\x\\game.exe", "Danganronpa Another Episode: Ultra Despair Girls")]).as_deref(), Some("Danganronpa Another Episode: Ultra Despair Girls"));
        assert_eq!(pick(&[w("/usr/bin/wine64-preloader", "Danganronpa V3: Killing Harmony")]).as_deref(), Some("Danganronpa V3: Killing Harmony"));
        assert_eq!(pick(&[w("C:\\x\\Dangan3Win.exe", "ニューダンガンロンパV3")]).as_deref(), Some("Danganronpa V3: Killing Harmony"));
        // pagine web e video con "Danganronpa" nel titolo: no
        assert_eq!(pick(&[w("C:\\x\\chrome.exe", "Danganronpa 2 walkthrough - YouTube - Google Chrome")]), None);
        assert_eq!(pick(&[w("C:\\x\\Discord.exe", "#danganronpa - Discord")]), None);
        assert_eq!(pick(&[w("C:\\x\\notepad.exe", "appunti.txt")]), None);
        assert_eq!(from_title("Danganronpa Decadence").as_deref(), Some("Danganronpa Decadence"));
    }
}
