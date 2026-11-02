//! Hotkey globali configurabili.

use crate::{config::Hotkeys, window, AppState};
use std::collections::HashMap;
use std::sync::atomic::Ordering;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutEvent, ShortcutState};

#[derive(Clone, Copy, Debug)]
pub enum Action {
    ToggleVisibility,
    PushToTalk,
    TextInput,
    ToggleClickThrough,
}

pub fn plugin() -> tauri::plugin::TauriPlugin<tauri::Wry> {
    tauri_plugin_global_shortcut::Builder::new()
        .with_handler(handle)
        .build()
}

fn handle(app: &AppHandle, shortcut: &Shortcut, event: ShortcutEvent) {
    let state = app.state::<AppState>();
    let Some(action) = state.hotkey_map.lock().unwrap().get(&shortcut.id()).copied() else {
        return;
    };
    let pressed = event.state() == ShortcutState::Pressed;
    match action {
        // Push-to-talk usa pressione E rilascio; il resto solo la pressione.
        Action::PushToTalk => crate::assistant::ptt(app, pressed),
        _ if !pressed => {}
        Action::ToggleVisibility => window::toggle_visibility(app),
        Action::TextInput => {
            let _ = app.emit("text-input", ());
        }
        Action::ToggleClickThrough => {
            let on = !state.click_through.load(Ordering::Relaxed);
            window::set_click_through(app, on);
        }
    }
}

/// Registra le hotkey. Restituisce gli errori (es. combinazione già usata da un'altra app).
pub fn register(app: &AppHandle, hk: &Hotkeys) -> Vec<String> {
    let gs = app.global_shortcut();
    let _ = gs.unregister_all();
    let mut map = HashMap::new();
    let mut errors = Vec::new();
    for (combo, action, label) in [
        (&hk.toggle_visibility, Action::ToggleVisibility, "mostra/nascondi"),
        (&hk.push_to_talk, Action::PushToTalk, "parla"),
        (&hk.text_input, Action::TextInput, "input testo"),
        (&hk.toggle_click_through, Action::ToggleClickThrough, "click-through"),
    ] {
        if combo.trim().is_empty() {
            continue; // hotkey disattivata
        }
        match combo.parse::<Shortcut>() {
            Ok(sc) => match gs.register(sc) {
                Ok(()) => {
                    map.insert(sc.id(), action);
                }
                Err(e) => errors.push(format!("{label} ({combo}): {e}")),
            },
            Err(e) => errors.push(format!("{label} ({combo}): combinazione non valida ({e})")),
        }
    }
    let state = app.state::<AppState>();
    *state.hotkey_map.lock().unwrap() = map;
    *state.hotkey_errors.lock().unwrap() = errors.clone();
    errors
}
