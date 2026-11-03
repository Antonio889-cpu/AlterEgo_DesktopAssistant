//! Icona nella system tray (l'app non compare nella taskbar).
//! Il menu viene ricostruito dalla config a ogni modifica, così le spunte restano allineate
//! anche quando si cambia qualcosa dalla finestra Impostazioni.

use crate::{update_config, window, AppState};
use std::sync::atomic::Ordering;
use tauri::{
    menu::{CheckMenuItem, IsMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Emitter, Manager, Wry,
};

fn pct(v: f64) -> String {
    ((v * 100.0).round() as u32).to_string()
}

/// Sottomenu a scelta singola: id delle voci = "<prefisso>:<valore>".
fn radio(app: &AppHandle, title: &str, prefix: &str, options: &[(&str, &str)], current: &str) -> tauri::Result<Submenu<Wry>> {
    let items: Vec<CheckMenuItem<Wry>> = options
        .iter()
        .map(|(v, label)| CheckMenuItem::with_id(app, format!("{prefix}:{v}"), *label, true, *v == current, None::<&str>))
        .collect::<tauri::Result<_>>()?;
    let refs: Vec<&dyn IsMenuItem<Wry>> = items.iter().map(|i| i as &dyn IsMenuItem<Wry>).collect();
    Submenu::with_items(app, title, true, &refs)
}

fn build_menu(app: &AppHandle) -> tauri::Result<Menu<Wry>> {
    let state = app.state::<AppState>();
    let cfg = state.cfg.lock().unwrap().clone();
    let click_on = state.click_through.load(Ordering::Relaxed);

    let show = MenuItem::with_id(app, "toggle", "Mostra / nascondi", true, None::<&str>)?;
    let talk = MenuItem::with_id(app, "talk", "Scrivi ad Alter Ego…", true, None::<&str>)?;
    let settings = MenuItem::with_id(app, "settings", "Impostazioni…", true, None::<&str>)?;
    let demo = MenuItem::with_id(app, "demo", "Messaggio di prova", true, None::<&str>)?;
    let click = CheckMenuItem::with_id(app, "click", "Click-through", true, click_on, None::<&str>)?;
    let opacity = radio(app, "Opacità", "op", &[("100", "100%"), ("90", "90%"), ("75", "75%"), ("60", "60%"), ("45", "45%")], &pct(cfg.opacity))?;
    let look = Submenu::with_items(
        app,
        "Aspetto",
        true,
        &[
            &radio(app, "Sfondo", "bg", &[("scanline", "Scanline"), ("circuit", "Circuiti"), ("both", "Entrambi")], &cfg.background)?,
            &radio(app, "Intensità sfondo", "int", &[("100", "100%"), ("75", "75%"), ("60", "60%"), ("50", "50%"), ("25", "25%")], &pct(cfg.bg_intensity))?,
            &radio(app, "Box dialogo", "dlg", &[("chiaro", "Chiaro (come screenshot)"), ("scuro", "Scuro")], &cfg.dialog_style)?,
            &CheckMenuItem::with_id(app, "fx", "Effetti animati (flicker)", true, cfg.effects, None::<&str>)?,
        ],
    )?;
    let sound = radio(app, "Suoni dialogo", "blip", &[("dr", "Blip stile Danganronpa"), ("soft", "Blip morbidi"), ("off", "Muto")], &cfg.audio.blips)?;
    let voice = CheckMenuItem::with_id(app, "tts", "Voce (TTS)", true, cfg.audio.tts != "off", None::<&str>)?;
    let reset = MenuItem::with_id(app, "reset", "Reimposta posizione", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Esci", true, None::<&str>)?;
    let sep = PredefinedMenuItem::separator(app)?;
    let sep2 = PredefinedMenuItem::separator(app)?;
    let menu = Menu::with_items(
        app,
        &[&show, &talk, &settings, &demo, &sep, &click, &opacity, &look, &sound, &voice, &reset, &sep2, &quit],
    )?;
    *state.tray_click_item.lock().unwrap() = Some(click);
    Ok(menu)
}

/// Ricostruisce il menu con i valori attuali.
pub fn refresh(app: &AppHandle) {
    if let (Some(tray), Ok(menu)) = (app.tray_by_id("main"), build_menu(app)) {
        let _ = tray.set_menu(Some(menu));
    }
}

fn on_menu(app: &AppHandle, id: &str) {
    match id {
        "toggle" => window::toggle_visibility(app),
        "settings" => {
            let _ = window::open_settings(app);
        }
        "talk" => {
            let _ = app.emit("text-input", ());
        }
        "demo" => {
            let _ = app.emit("demo", ());
        }
        "click" => {
            let on = !app.state::<AppState>().click_through.load(Ordering::Relaxed);
            window::set_click_through(app, on);
        }
        "fx" => update_config(app, |c| c.effects = !c.effects),
        "tts" => update_config(app, |c| {
            // riaccende la voce di Windows se era spenta
            c.audio.tts = if c.audio.tts == "off" { "windows".into() } else { "off".into() };
        }),
        "reset" => window::reset_geometry(app),
        "quit" => window::hide_main(app, true),
        _ => {
            let Some((prefix, value)) = id.split_once(':') else { return };
            let num = value.parse::<f64>().unwrap_or(100.0) / 100.0;
            let value = value.to_string();
            match prefix {
                "op" => update_config(app, |c| c.opacity = num.clamp(0.2, 1.0)),
                "bg" => update_config(app, |c| c.background = value),
                "int" => update_config(app, |c| c.bg_intensity = num.clamp(0.0, 1.0)),
                "dlg" => update_config(app, |c| c.dialog_style = value),
                "blip" => update_config(app, |c| c.audio.blips = value),
                _ => {}
            }
        }
    }
}

pub fn create(app: &AppHandle) -> tauri::Result<()> {
    let menu = build_menu(app)?;
    TrayIconBuilder::with_id("main")
        .icon(app.default_window_icon().cloned().expect("icona mancante"))
        .tooltip("Alter Ego")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| on_menu(app, event.id().as_ref()))
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
                window::toggle_visibility(tray.app_handle());
            }
        })
        .build(app)?;
    Ok(())
}
