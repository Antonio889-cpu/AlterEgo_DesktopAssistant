//! Lingua dei testi del backend (menu della tray, stati e messaggi d'errore mostrati all'utente).
//! La sceglie il frontend (impostazione "Lingua dell'interfaccia", "auto" = lingua di Windows).
//! I testi per il modello AI restano in italiano: il modello risponde nella lingua scelta.

use std::sync::atomic::{AtomicBool, Ordering};

static EN: AtomicBool = AtomicBool::new(false);

pub fn en() -> bool {
    EN.load(Ordering::Relaxed)
}

/// Imposta la lingua; true se è cambiata.
pub fn set_en(on: bool) -> bool {
    EN.swap(on, Ordering::Relaxed) != on
}

/// `tr("italiano", "english")`
pub fn tr(it: &'static str, en_text: &'static str) -> &'static str {
    if en() { en_text } else { it }
}

/// `tr!("Ciao {nome}", "Hi {nome}")`: come `format!`, nella lingua dell'interfaccia.
#[macro_export]
macro_rules! tr {
    ($it:literal, $en:literal) => {
        if $crate::i18n::en() { format!($en) } else { format!($it) }
    };
    ($it:literal, $en:literal, $($arg:tt)*) => {
        if $crate::i18n::en() { format!($en, $($arg)*) } else { format!($it, $($arg)*) }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    // NB: i test girano in parallelo e altri controllano i messaggi italiani: qui niente set_en(true)
    #[test]
    fn italian_by_default() {
        let x = 3;
        assert!(!en());
        assert_eq!(crate::tr!("{x} brani", "{x} tracks"), "3 brani");
        assert_eq!(tr("Esci", "Quit"), "Esci");
    }
}
