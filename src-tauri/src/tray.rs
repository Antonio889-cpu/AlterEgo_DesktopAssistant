//! Icona nella system tray (l'app non compare nella taskbar).

use crate::{window, AppState};
use std::sync::atomic::Ordering;
use tauri::{
    menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Emitter, Manager, Wry,
};

const OPACITIES: [u32; 5] = [100, 90, 75, 60, 45];

pub fn create(app: &AppHandle) -> tauri::Result<()> {
    let state = app.state::<AppState>();
    let current = (state.cfg.lock().unwrap().opacity * 100.0).round() as u32;

    let show = MenuItem::with_id(app, "toggle", "Mostra / nascondi", true, None::<&str>)?;
    let click = CheckMenuItem::with_id(app, "click", "Click-through", true, false, None::<&str>)?;
    let opacity_items: Vec<CheckMenuItem<Wry>> = OPACITIES
        .iter()
        .map(|p| {
            CheckMenuItem::with_id(app, format!("op_{p}"), format!("{p}%"), true, *p == current, None::<&str>)
        })
        .collect::<tauri::Result<_>>()?;
    let refs: Vec<&dyn tauri::menu::IsMenuItem<Wry>> =
        opacity_items.iter().map(|i| i as &dyn tauri::menu::IsMenuItem<Wry>).collect();
    let opacity = Submenu::with_items(app, "Opacità", true, &refs)?;
    let reset = MenuItem::with_id(app, "reset", "Reimposta posizione", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Esci", true, None::<&str>)?;
    let sep = PredefinedMenuItem::separator(app)?;
    let menu = Menu::with_items(app, &[&show, &click, &opacity, &reset, &sep, &quit])?;

    *state.tray_click_item.lock().unwrap() = Some(click);

    TrayIconBuilder::with_id("main")
        .icon(app.default_window_icon().cloned().expect("icona mancante"))
        .tooltip("Alter Ego")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(move |app, event| match event.id().as_ref() {
            "toggle" => window::toggle_visibility(app),
            "click" => {
                let on = !app.state::<AppState>().click_through.load(Ordering::Relaxed);
                window::set_click_through(app, on);
            }
            "reset" => window::reset_geometry(app),
            "quit" => {
                crate::flush_config(app);
                app.exit(0);
            }
            id if id.starts_with("op_") => {
                let p: u32 = id[3..].parse().unwrap_or(100);
                for (item, v) in opacity_items.iter().zip(OPACITIES) {
                    let _ = item.set_checked(v == p);
                }
                let value = p as f64 / 100.0;
                let state = app.state::<AppState>();
                state.cfg.lock().unwrap().opacity = value;
                state.dirty.store(true, Ordering::Relaxed);
                let _ = app.emit("opacity", value);
            }
            _ => {}
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
