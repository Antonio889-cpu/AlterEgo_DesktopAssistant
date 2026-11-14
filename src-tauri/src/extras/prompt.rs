//! Testo dell'ospite: prompt corti (pochi token in ingresso), risposta JSON con poche pagine,
//! frasi di riserva se il modello non risponde.

use super::Page;

/// Prompt di sistema: personaggio, regole, formato. In inglese (meno token), risposta nella lingua scelta.
pub fn system(moods: &[String], en: bool) -> String {
    let lang = if en { "English" } else { "Italian" };
    format!(
        "You are Monokuma from Danganronpa: the sadistic, theatrical bear headmaster. You just hacked the \
screen of the player's desktop assistant while they play. Look at the screenshot (if any) to guess the game, \
chapter, place, characters and situation, then tease the player with a SPOILER about what will happen next: \
it may be true or completely made up, mix them freely and never say which. Be smug, cruel, playful, with \
\"upupupu\" laughs and his catchphrases; talk to the player directly. Don't describe the screenshot.\n\
Reply ONLY with JSON: {{\"pages\":[{{\"mood\":\"…\",\"text\":\"…\"}}]}} — 2 to 4 pages, each text max 140 \
characters, in {lang}. mood is one of: {}.",
        moods.join(", ")
    )
}

pub fn user(game: Option<&str>, has_image: bool) -> String {
    let g = game.unwrap_or("a Danganronpa game");
    if has_image { format!("Game: {g}. Screenshot attached.") } else { format!("Game: {g}. No screenshot: guess where they could be.") }
}

/// Estrae le pagine dalla risposta (anche con testo o ``` intorno al JSON).
pub fn parse(text: &str, moods: &[String]) -> Vec<Page> {
    let (Some(a), Some(b)) = (text.find('{'), text.rfind('}')) else {
        return plain(text, moods);
    };
    #[derive(serde::Deserialize)]
    struct R {
        pages: Vec<Page>,
    }
    let Ok(r) = serde_json::from_str::<R>(&text[a..=b]) else {
        return plain(text, moods);
    };
    r.pages
        .into_iter()
        .filter(|p| !p.text.trim().is_empty())
        .take(5)
        .map(|p| Page {
            mood: if moods.contains(&p.mood) { p.mood } else { default_mood(moods) },
            text: p.text.trim().chars().take(260).collect(),
        })
        .collect()
}

/// Risposta senza JSON: si usa il testo così com'è, in una pagina.
fn plain(text: &str, moods: &[String]) -> Vec<Page> {
    let t = text.trim().trim_matches('`').trim();
    if t.is_empty() {
        return Vec::new();
    }
    vec![Page { mood: default_mood(moods), text: t.chars().take(260).collect() }]
}

fn default_mood(moods: &[String]) -> String {
    ["smug", "laugh", "idle"].iter().find(|m| moods.iter().any(|x| x == *m)).map(|m| m.to_string()).or_else(|| moods.first().cloned()).unwrap_or_else(|| "idle".into())
}

/// Frasi di riserva (niente rete o nessun provider): niente spoiler, solo minacce.
pub fn fallback(moods: &[String], en: bool) -> Vec<Page> {
    let sets_it: &[&[(&str, &str)]] = &[
        &[("laugh", "Upupupu! Pensavi di giocare in pace? Che ingenuo!"), ("sinister", "Ho visto come va a finire… e credimi, non ti piacerà per niente.")],
        &[("smug", "Ehi, ehi! Lo sai che il colpevole è già davanti ai tuoi occhi?"), ("giggle", "Ma tanto non te ne accorgerai. Puhuhu!")],
        &[("excited", "Che emozione! Tra poco qualcuno sparirà…"), ("tease", "Indovina chi? No, non te lo dico. Non ancora!")],
        &[("sinister", "La disperazione ti aspetta proprio dietro l'angolo."), ("laugh", "Continua pure a giocare… upupupu!")],
    ];
    let sets_en: &[&[(&str, &str)]] = &[
        &[("laugh", "Upupupu! Did you really think you could play in peace? How naive!"), ("sinister", "I've seen how this ends… and trust me, you won't like it one bit.")],
        &[("smug", "Hey, hey! Did you know the culprit is already right in front of you?"), ("giggle", "Not that you'll ever notice. Puhuhu!")],
        &[("excited", "How exciting! Someone is about to disappear…"), ("tease", "Guess who? Nope, not telling. Not yet!")],
        &[("sinister", "Despair is waiting for you just around the corner."), ("laugh", "Go on, keep playing… upupupu!")],
    ];
    let sets = if en { sets_en } else { sets_it };
    let n = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as usize).unwrap_or(0) % sets.len();
    sets[n]
        .iter()
        .map(|(m, t)| Page { mood: if moods.is_empty() || moods.iter().any(|x| x == m) { m.to_string() } else { default_mood(moods) }, text: t.to_string() })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn moods() -> Vec<String> {
        ["idle", "laugh", "smug", "sinister"].map(String::from).to_vec()
    }

    #[test]
    fn parses_json_with_noise() {
        let r = "Ecco:\n```json\n{\"pages\":[{\"mood\":\"laugh\",\"text\":\"Upupupu!\"},{\"mood\":\"boh\",\"text\":\" Ciao \"},{\"mood\":\"smug\",\"text\":\"\"}]}\n```";
        let p = parse(r, &moods());
        assert_eq!(p.len(), 2);
        assert_eq!(p[0], Page { mood: "laugh".into(), text: "Upupupu!".into() });
        assert_eq!(p[1].mood, "smug"); // umore sconosciuto → predefinito
        assert_eq!(p[1].text, "Ciao");
        let p = parse("solo testo, niente json", &moods());
        assert_eq!(p.len(), 1);
        assert!(parse("  ", &moods()).is_empty());
    }

    #[test]
    fn prompt_and_fallback() {
        let s = system(&moods(), false);
        assert!(s.contains("Italian") && s.contains("idle, laugh, smug, sinister"));
        assert!(s.len() < 1200, "prompt corto: {}", s.len());
        let f = fallback(&moods(), true);
        assert!(f.len() >= 2 && f.iter().all(|p| moods().contains(&p.mood)));
    }
}
