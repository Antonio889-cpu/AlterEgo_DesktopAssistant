//! Comandi dell'app a voce o per iscritto ("apri music venue", "chiudi il lettore audio"…).
//! Riguardano solo Alter Ego (finestre, lettore, aspetto, link, aggiornamento), non il PC.
//! Le frasi più comuni si riconoscono in locale (senza modello, come le clip);
//! il resto passa dal modello con lo strumento `app_control`.

use crate::{music, update, update_config, window, AppState};
use tauri::{AppHandle, Manager};

pub const ACTIONS: &[&str] = &[
    "open_music_venue",
    "close_music_venue",
    "open_player",
    "close_player",
    "player_bigger",
    "player_smaller",
    "player_size",
    "open_settings",
    "hide_alter_ego",
    "click_through_on",
    "click_through_off",
    "opacity",
    "compact_on",
    "compact_off",
    "music_sprite_on",
    "music_sprite_off",
    "noise_suppression_on",
    "noise_suppression_off",
    "unfiltered_on",
    "unfiltered_off",
    "open_website",
    "open_github",
    "check_update",
    "install_update",
    "version",
];

use crate::i18n::tr;

pub async fn run(app: &AppHandle, action: &str, value: Option<f64>) -> Result<String, String> {
    let ok = |s: &str| Ok(s.to_string());
    match action {
        "open_music_venue" => {
            music::open_venue(app);
            ok(tr("Music Venue aperta.", "Music Venue opened."))
        }
        "close_music_venue" => {
            music::close_venue(app);
            ok(tr("Music Venue chiusa.", "Music Venue closed."))
        }
        "open_player" => {
            if !music::state(app).playing {
                return Err(tr("non sta suonando niente: usa play_music per mettere un brano", "nothing is playing: use play_music to start a song").into());
            }
            music::show_player(app);
            ok(tr("Lettore aperto.", "Player opened."))
        }
        "close_player" => {
            music::stop_music(app);
            ok(tr("Lettore chiuso, musica fermata.", "Player closed, music stopped."))
        }
        "player_bigger" | "player_smaller" | "player_size" => {
            let cur = app.state::<AppState>().cfg.lock().unwrap().music.player_width;
            let w = match action {
                "player_bigger" => cur * 1.2,
                "player_smaller" => cur / 1.2,
                _ => value.unwrap_or(cur),
            };
            music::resize_player(app, w);
            let w = app.state::<AppState>().cfg.lock().unwrap().music.player_width;
            Ok(crate::tr!("Lettore largo {:.0} px.", "Player is {:.0} px wide.", w))
        }
        "open_settings" => window::open_settings(app).map(|_| tr("Impostazioni aperte.", "Settings opened.").into()).map_err(|e| e.to_string()),
        "hide_alter_ego" => {
            window::hide_main(app, false);
            ok(tr("Mi nascondo: richiamami con Ctrl+Alt+A.", "Hiding: call me back with Ctrl+Alt+A."))
        }
        "click_through_on" | "click_through_off" => {
            window::set_click_through(app, action.ends_with("on"));
            ok(if action.ends_with("on") { tr("Click-through attivo.", "Click-through on.") } else { tr("Click-through spento.", "Click-through off.") })
        }
        "opacity" => {
            let v = (value.unwrap_or(100.0) / 100.0).clamp(0.2, 1.0);
            update_config(app, |c| c.opacity = v);
            Ok(crate::tr!("Opacità al {:.0}%.", "Opacity at {:.0}%.", v * 100.0))
        }
        "compact_on" | "compact_off" => {
            let on = action.ends_with("on");
            update_config(app, |c| c.music.compact = on);
            if !on {
                window::set_compact(app, false);
            }
            ok(if on { tr("Modalità compatta attiva.", "Compact mode on.") } else { tr("Modalità compatta spenta.", "Compact mode off.") })
        }
        "music_sprite_on" | "music_sprite_off" => {
            let on = action.ends_with("on");
            update_config(app, |c| c.music.music_sprite = on);
            ok(if on { tr("Metto le cuffie quando c'è musica.", "I'll wear headphones while music plays.") } else { tr("Niente cuffie.", "No headphones.") })
        }
        "noise_suppression_on" | "noise_suppression_off" => {
            let on = action.ends_with("on");
            update_config(app, |c| c.audio.mic_link.noise_suppression = on);
            crate::audio::commands::apply(app);
            ok(if on { tr("Riduzione del rumore attiva sul microfono Mic-Link.", "Noise suppression on for the Mic-Link microphone.") } else { tr("Riduzione del rumore spenta.", "Noise suppression off.") })
        }
        "unfiltered_on" | "unfiltered_off" => {
            let on = action.ends_with("on");
            update_config(app, |c| c.personality.unfiltered = on);
            ok(if on { tr("Modalità Senza filtri attiva (dal prossimo messaggio).", "Unfiltered mode on (from the next message).") } else { tr("Modalità Senza filtri spenta.", "Unfiltered mode off.") })
        }
        "open_website" => update::open_link("site").map(|_| tr("Sito aperto.", "Website opened.").into()),
        "open_github" => update::open_link("github").map(|_| tr("GitHub aperto.", "GitHub opened.").into()),
        "check_update" => {
            let i = update::check(app).await?;
            Ok(if i.newer {
                crate::tr!("C'è la versione {} (hai la {}). Dimmi \"aggiorna\" per installarla.", "Version {} is out (you have {}). Say \"update\" to install it.", i.latest, i.current)
            } else {
                crate::tr!("Sei all'ultima versione ({}).", "You're on the latest version ({}).", i.current)
            })
        }
        "install_update" => update::install(app).await,
        "version" => Ok(crate::tr!("Versione {}.", "Version {}.", update::VERSION)),
        other => Err(format!("azione sconosciuta: {other}")),
    }
}

/// Frase normalizzata: minuscole, senza punteggiatura, senza "alter ego", "per favore", "please".
fn norm(text: &str) -> String {
    let t: String = text.to_lowercase().chars().map(|c| if c.is_alphanumeric() || c == '\'' { c } else { ' ' }).collect();
    let t = format!(" {} ", t.split_whitespace().collect::<Vec<_>>().join(" "));
    let mut t = t;
    for w in [" alter ego ", " alterego ", " per favore ", " perfavore ", " please ", " hey ", " ehi ", " ok "] {
        t = t.replace(w, " ");
    }
    t.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Comando locale: (azione dell'app o "music:<azione>"). Solo frasi brevi e inequivocabili.
pub fn quick_match(text: &str) -> Option<&'static str> {
    let t = norm(text);
    if t.split(' ').count() > 7 {
        return None;
    }
    const TABLE: &[(&str, &[&str])] = &[
        ("open_music_venue", &["apri music venue", "apri la music venue", "apri il music venue", "mostra la music venue", "open music venue", "open the music venue", "show the music venue"]),
        ("close_music_venue", &["chiudi music venue", "chiudi la music venue", "chiudi il music venue", "close music venue", "close the music venue"]),
        ("close_player", &["chiudi il lettore", "chiudi il lettore audio", "chiudi il lettore musicale", "chiudi il player", "chiudi il music player", "close the player", "close the music player", "close the audio player", "close player"]),
        ("open_player", &["apri il lettore", "apri il lettore audio", "apri il lettore musicale", "mostra il lettore", "open the player", "open the music player", "show the player"]),
        ("player_bigger", &["ingrandisci il lettore", "lettore più grande", "make the player bigger", "bigger player"]),
        ("player_smaller", &["rimpicciolisci il lettore", "lettore più piccolo", "make the player smaller", "smaller player"]),
        ("open_settings", &["apri le impostazioni", "apri impostazioni", "open settings", "open the settings"]),
        ("open_website", &["apri il sito", "apri il sito web", "apri il tuo sito", "open the website", "open your website", "open the site"]),
        ("open_github", &["apri github", "apri il github", "apri la pagina github", "open github", "open the github page"]),
        ("check_update", &["controlla gli aggiornamenti", "controlla aggiornamenti", "ci sono aggiornamenti", "check for updates", "check updates", "any updates"]),
        ("music:pause", &["pausa", "metti in pausa", "metti pausa", "pause", "pause the music"]),
        ("music:resume", &["riprendi", "riprendi la musica", "play", "resume", "resume the music"]),
        ("music:next", &["prossimo brano", "prossima canzone", "brano successivo", "salta", "next song", "next track", "skip"]),
        ("music:previous", &["brano precedente", "canzone precedente", "previous song", "previous track"]),
        ("music:favorite", &["aggiungi ai preferiti", "metti nei preferiti", "add to favorites", "favorite this song"]),
    ];
    TABLE.iter().find(|(_, phrases)| phrases.contains(&t.as_str())).map(|(a, _)| *a)
}

/// Esegue una frase locale. None = non è un comando: va al modello.
pub async fn quick(app: &AppHandle, text: &str) -> Option<Result<String, String>> {
    let action = quick_match(text)?;
    if let Some(m) = action.strip_prefix("music:") {
        // senza musica in corso la frase può voler dire altro: decide il modello
        if !music::state(app).playing {
            return None;
        }
        return Some(music::control(app, m, None).await);
    }
    Some(run(app, action, None).await)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_phrases() {
        assert_eq!(quick_match("Apri Music Venue!"), Some("open_music_venue"));
        assert_eq!(quick_match("Alter Ego, chiudi il lettore audio per favore"), Some("close_player"));
        assert_eq!(quick_match("close the music player please"), Some("close_player"));
        assert_eq!(quick_match("pausa"), Some("music:pause"));
        assert_eq!(quick_match("apri il sito di repubblica"), None);
        assert_eq!(quick_match("cosa pensi della music venue di ieri sera"), None);
    }
}
