//! Icona nella system tray (l'app non compare nella taskbar).

use crate::{update_config, window, AppState};
use std::sync::atomic::Ordering;
use tauri::{
    menu::{CheckMenuItem, IsMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Emitter, Manager, Wry,
};

/// Gruppo di voci a scelta singola: id = "<prefisso>:<valore>".
struct Radio {
    prefix: &'static str,
    items: Vec<(String, CheckMenuItem<Wry>)>,
}

impl Radio {
    fn new(app: &AppHandle, prefix: &'static str, options: &[(&str, &str)], current: &str) -> tauri::Result<Self> {
        let items = options
            .iter()
            .map(|(value, label)| {
                let item = CheckMenuItem::with_id(
                    app, format!("{prefix}:{value}"), *label, true, *value == current, None::<&str>,
                )?;
                Ok((value.to_string(), item))
            })
            .collect::<tauri::Result<_>>()?;
        Ok(Self { prefix, items })
    }

    fn submenu(&self, app: &AppHandle, title: &str) -> tauri::Result<Submenu<Wry>> {
        let refs: Vec<&dyn IsMenuItem<Wry>> = self.items.iter().map(|(_, i)| i as &dyn IsMenuItem<Wry>).collect();
        Submenu::with_items(app, title, true, &refs)
    }

    /// Se l'id appartiene al gruppo, aggiorna le spunte e restituisce il valore scelto.
    fn select<'a>(&self, id: &'a str) -> Option<&'a str> {
        let value = id.strip_prefix(self.prefix)?.strip_prefix(':')?;
        for (v, item) in &self.items {
            let _ = item.set_checked(v == value);
        }
        Some(value)
    }
}

fn pct(v: f64) -> String {
    ((v * 100.0).round() as u32).to_string()
}

pub fn create(app: &AppHandle) -> tauri::Result<()> {
    let state = app.state::<AppState>();
    let cfg = state.cfg.lock().unwrap().clone();

    let show = MenuItem::with_id(app, "toggle", "Mostra / nascondi", true, None::<&str>)?;
    let demo = MenuItem::with_id(app, "demo", "Messaggio di prova", true, None::<&str>)?;
    let talk = MenuItem::with_id(app, "talk", "Scrivi ad Alter Ego…", true, None::<&str>)?;
    let settings = MenuItem::with_id(app, "settings", "Impostazioni…", true, None::<&str>)?;
    let click = CheckMenuItem::with_id(app, "click", "Click-through", true, false, None::<&str>)?;

    let opacity = Radio::new(app, "op", &[("100", "100%"), ("90", "90%"), ("75", "75%"), ("60", "60%"), ("45", "45%")], &pct(cfg.opacity))?;
    let background = Radio::new(app, "bg", &[("scanline", "Scanline"), ("circuit", "Circuiti"), ("both", "Entrambi")], &cfg.background)?;
    let intensity = Radio::new(app, "int", &[("100", "100%"), ("75", "75%"), ("50", "50%"), ("25", "25%")], &pct(cfg.bg_intensity))?;
    let dialog = Radio::new(app, "dlg", &[("chiaro", "Chiaro (come screenshot)"), ("scuro", "Scuro")], &cfg.dialog_style)?;
    let effects = CheckMenuItem::with_id(app, "fx", "Effetti animati (flicker)", true, cfg.effects, None::<&str>)?;

    let look = Submenu::with_items(
        app,
        "Aspetto",
        true,
        &[
            &background.submenu(app, "Sfondo")?,
            &intensity.submenu(app, "Intensità sfondo")?,
            &dialog.submenu(app, "Box dialogo")?,
            &effects,
        ],
    )?;
    let reset = MenuItem::with_id(app, "reset", "Reimposta posizione", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Esci", true, None::<&str>)?;
    let sep = PredefinedMenuItem::separator(app)?;
    let sep2 = PredefinedMenuItem::separator(app)?;
    let menu = Menu::with_items(
        app,
        &[&show, &talk, &settings, &demo, &sep, &click, &opacity.submenu(app, "Opacità")?, &look, &reset, &sep2, &quit],
    )?;

    *state.tray_click_item.lock().unwrap() = Some(click);

    TrayIconBuilder::with_id("main")
        .icon(app.default_window_icon().cloned().expect("icona mancante"))
        .tooltip("Alter Ego")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(move |app, event| {
            let id = event.id().as_ref();
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
                "fx" => {
                    let on = effects.is_checked().unwrap_or(true);
                    update_config(app, |c| c.effects = on);
                }
                "reset" => window::reset_geometry(app),
                "quit" => {
                    crate::flush_config(app);
                    app.exit(0);
                }
                _ => {
                    if let Some(v) = opacity.select(id) {
                        let v = v.parse::<f64>().unwrap_or(100.0) / 100.0;
                        update_config(app, |c| c.opacity = v);
                    } else if let Some(v) = background.select(id) {
                        let v = v.to_string();
                        update_config(app, |c| c.background = v);
                    } else if let Some(v) = intensity.select(id) {
                        let v = v.parse::<f64>().unwrap_or(75.0) / 100.0;
                        update_config(app, |c| c.bg_intensity = v);
                    } else if let Some(v) = dialog.select(id) {
                        let v = v.to_string();
                        update_config(app, |c| c.dialog_style = v);
                    }
                }
            }
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                window::toggle_visibility(tray.app_handle());
            }
        })
        .build(app)?;
    Ok(())
}
