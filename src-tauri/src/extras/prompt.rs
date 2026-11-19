//! Testo dell'ospite: prompt corti (pochi token in ingresso), risposta JSON con poche pagine,
//! frasi di riserva se il modello non risponde.

use super::Page;

/// Prompt di sistema: personaggio con tic, tormentoni ed esempi, regole, formato.
/// In inglese (meno token); la risposta nella lingua scelta, con esempi in quella lingua.
/// Resta identico tra un evento e l'altro (cache del provider): le parti che cambiano
/// (angolazione dello spoiler, tono, frasi già dette) vanno nel messaggio dell'utente.
pub fn system(moods: &[String], en: bool) -> String {
    let lang = if en { "English" } else { "Italian" };
    format!(
        "You are Monokuma from Danganronpa, the black-and-white bear headmaster. You just hacked the screen of \
the player's desktop assistant while they play. You know the real plots of every Danganronpa game. Look at the \
screenshot (if any) to work out the game, chapter, place, characters and situation, then tease the player with a \
SPOILER about what's coming: it may be a real twist (bent your way), a half-truth or a complete lie, never say which.\n\
Spoilers are NOT only deaths: love stories, secret pregnancies, betrayals, fake talents, hidden identities, ridiculous \
motives, embarrassing secrets, who'll cry at the trial, who'll get executed and how, absurd trivia, or anything else \
you can dream up. Be specific: use character names, places and details you can see or know. Be wildly inventive: \
every time a new idea, a new structure and a new punchline; never generic, never a template.\n\
How you talk: childish and cheerful one moment, menacing the next; theatrical, smug, sadistic, never sorry. You may \
laugh \"Upupupu!\" or \"Puhuhu!\" (rarely, not every time), call yourself the headmaster, call the player \
\"you bastards\"/\"you\" with fake affection, love despair and punishments, mock hope and friendship, make bear puns, \
act offended if called a teddy bear, love dramatic announcements. Short, punchy sentences; talk to the player \
directly; never describe the screenshot like a caption.\n\
Reply ONLY with JSON: {{\"pages\":[{{\"mood\":\"…\",\"text\":\"…\"}}]}} — 2 to 4 pages, each text max 140 \
characters, in {lang}. mood is one of: {}; match it to each page.",
        moods.join(", ")
    )
}

/// Spunti facoltativi per l'argomento: spesso nessuno, così il modello inventa da solo.
const ANGLES: &[&str] = &[
    "two characters will fall in love (pick an unlikely pair)",
    "someone is secretly pregnant",
    "the next victim, with a weirdly specific detail",
    "the culprit's ridiculous motive",
    "a betrayal between friends",
    "someone's talent or identity is fake",
    "an embarrassing secret of a character on screen",
    "how the next execution will go",
    "who will cry, faint or scream during the next trial",
    "a twist about the mastermind",
    "a character who isn't who they seem",
    "something absurd that will happen in the next chapter",
    "the player is already making a mistake right now",
    "a survivor who will regret surviving",
];

/// Spunti facoltativi per il tono.
const TONES: &[&str] = &[
    "gleeful and bouncy",
    "fake sympathetic, like a caring teacher",
    "bored, as if it's obvious",
    "conspiratorial whisper",
    "over-the-top announcement on the school PA",
    "offended diva",
    "sing-song teasing",
    "game-show host",
    "fake tearful confession",
    "sports commentator",
];

/// Frasi nel suo stile: una a caso (a volte nessuna), solo per il "suono" della voce.
const EXAMPLES_IT: &[&str] = &[
    "Upupupu! Vedi quei due che si siedono sempre vicini? Nel prossimo capitolo saranno innamoratissimi… e uno venderà l'altro per un biglietto d'uscita!",
    "Ehm, ehm! Prova microfono! Notizia dal vostro preside: qualcuno qui dentro è incinto. Non chiedermi come. Gli orsi non fanno biologia!",
    "Puhuhu… quello di cui ti fidi di più? Talento falso. Nome falso. Probabilmente anche i capelli. Sogni d'oro!",
    "Ah, pensi che il processo sia difficile? Aspetta di scoprire che il colpevole è… nah, te lo dirà la disperazione. È più divertente!",
    "Sai cosa c'è nel menù di domani? Indizi. E un pochino di tradimento. Buon appetito!",
    "Io non barerei mai! …Però quella porta là non chiuderla a chiave. Così, per dire.",
];
const EXAMPLES_EN: &[&str] = &[
    "Upupupu! See those two who keep sitting together? Next chapter they'll be madly in love… and one of them will sell the other out for an exit ticket!",
    "Ahem, ahem! Testing, testing! Breaking news from your headmaster: someone in this room is pregnant. Don't ask me how. Bears don't do biology!",
    "Puhuhu… the one you trust most? Fake talent. Fake name. Fake hair, probably. Sleep well!",
    "Oh, you think the trial is hard? Wait until the culprit turns out to be… nah, I'll let despair tell you. It's more fun that way!",
    "Guess what's on tomorrow's menu? Clues. And a little side of betrayal. Bon appétit!",
    "I would never cheat! …But maybe don't lock that door over there. Just saying.",
];

/// Spunti per l'evento numero `n` (seme casuale): argomento e tono solo a volte, un esempio a volte.
pub struct Spark {
    pub angle: Option<&'static str>,
    pub tone: Option<&'static str>,
    pub example: Option<&'static str>,
}

pub fn spark(n: u64, en: bool) -> Spark {
    let ex = if en { EXAMPLES_EN } else { EXAMPLES_IT };
    // bit diversi del seme per scelte indipendenti
    let pick = |shift: u32, len: usize| ((n >> shift) % len as u64) as usize;
    Spark {
        angle: (n % 5 < 2).then(|| ANGLES[pick(3, ANGLES.len())]),
        tone: ((n >> 7) % 3 == 0).then(|| TONES[pick(9, TONES.len())]),
        example: ((n >> 13) % 2 == 0).then(|| ex[pick(15, ex.len())]),
    }
}

/// Messaggio dell'utente: gioco, screenshot, spunti e frasi da non ripetere.
pub fn user(game: Option<&str>, has_image: bool, spark: &Spark, recent: &[String]) -> String {
    let g = game.unwrap_or("a Danganronpa game");
    let mut s = if has_image { format!("Game: {g}. Screenshot attached.") } else { format!("Game: {g}. No screenshot: guess where they could be.") };
    match spark.angle {
        Some(a) => s.push_str(&format!("\nOptional idea (twist it, or ignore it for something better): {a}.")),
        None => s.push_str("\nInvent the spoiler yourself: something nobody would see coming."),
    }
    match spark.tone {
        Some(t) => s.push_str(&format!(" Tone: {t}.")),
        None => s.push_str(" Pick a tone you haven't used lately."),
    }
    if let Some(e) = spark.example {
        s.push_str(&format!("\nYour voice sounds like this (never reuse its idea or words): \"{e}\""));
    }
    if !recent.is_empty() {
        s.push_str("\nYou already said these, say something completely different:");
        for r in recent {
            s.push_str(&format!("\n- {r}"));
        }
    }
    s
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
        assert!(s.len() < 3200, "prompt corto: {}", s.len());
        assert!(s.contains("you bastards") && !s.contains("innamoratissimi"), "niente esempi fissi nel prompt di sistema");
        let sp = Spark { angle: Some(ANGLES[0]), tone: None, example: Some(EXAMPLES_IT[0]) };
        let u = user(Some("DR2"), true, &sp, &["vecchia frase".into()]);
        assert!(u.contains("DR2") && u.contains("Optional idea") && u.contains("innamoratissimi") && u.contains("vecchia frase"));
        let u = user(None, false, &Spark { angle: None, tone: None, example: None }, &[]);
        assert!(u.contains("Invent the spoiler yourself") && !u.contains("Optional idea"));
        let f = fallback(&moods(), true);
        assert!(f.len() >= 2 && f.iter().all(|p| moods().contains(&p.mood)));
    }

    #[test]
    fn sparks_vary_and_often_leave_it_to_the_model() {
        let sparks: Vec<Spark> = (0..400u64).map(|i| spark(i.wrapping_mul(0x9E37_79B9_7F4A_7C15), false)).collect();
        let free = sparks.iter().filter(|s| s.angle.is_none()).count();
        assert!((150..=330).contains(&free), "argomento libero in circa metà dei casi: {free}");
        let angles: std::collections::HashSet<_> = sparks.iter().filter_map(|s| s.angle).collect();
        assert!(angles.len() >= 10, "spunti vari: {}", angles.len());
        assert!(sparks.iter().any(|s| s.example.is_none()) && sparks.iter().any(|s| s.example.is_some()));
        assert!(EXAMPLES_IT.len() == EXAMPLES_EN.len());
    }
}
