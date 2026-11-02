// Niente console in release su Windows.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod config;
mod hotkeys;
mod tray;
mod window;

use serde::Serialize;
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Mutex,
    },
};
use tauri::{menu::CheckMenuItem, AppHandle, Emitter, Manager, State, WindowEvent, Wry};

pub struct AppState {
    pub cfg: Mutex<config::Config>,
    pub cfg_path: PathBuf,
    pub dirty: AtomicBool,
    pub click_through: AtomicBool,
    pub handle_hover: AtomicBool,
    pub tray_click_item: Mutex<Option<CheckMenuItem<Wry>>>,
    pub hotkey_map: Mutex<HashMap<u32, hotkeys::Action>>,
    pub hotkey_errors: Mutex<Vec<String>>,
}

pub fn flush_config(app: &AppHandle) {
    let state = app.state::<AppState>();
    if state.dirty.swap(false, Ordering::Relaxed) {
        let cfg = state.cfg.lock().unwrap().clone();
        if let Err(e) = config::save(&state.cfg_path, &cfg) {
            eprintln!("salvataggio config fallito: {e}");
            state.dirty.store(true, Ordering::Relaxed);
        }
    }
}

// ---------- comandi per il frontend ----------

#[derive(Serialize, Clone)]
pub struct FrontConfig {
    opacity: f64,
    click_through: bool,
    hotkeys: config::Hotkeys,
    hotkey_errors: Vec<String>,
    user_name: String,
    name_tag: String,
    background: String,
    bg_intensity: f64,
    dialog_style: String,
    effects: bool,
}

pub fn front_config(app: &AppHandle) -> FrontConfig {
    let state = app.state::<AppState>();
    let hotkey_errors = state.hotkey_errors.lock().unwrap().clone();
    let cfg = state.cfg.lock().unwrap();
    FrontConfig {
        hotkey_errors,
        opacity: cfg.opacity,
        click_through: state.click_through.load(Ordering::Relaxed),
        hotkeys: cfg.hotkeys.clone(),
        user_name: cfg.user_name.clone(),
        name_tag: cfg.name_tag.clone(),
        background: cfg.background.clone(),
        bg_intensity: cfg.bg_intensity,
        dialog_style: cfg.dialog_style.clone(),
        effects: cfg.effects,
    }
}

/// Modifica la config, la segna da salvare e notifica il frontend.
pub fn update_config(app: &AppHandle, f: impl FnOnce(&mut config::Config)) {
    let state = app.state::<AppState>();
    f(&mut state.cfg.lock().unwrap());
    state.dirty.store(true, Ordering::Relaxed);
    let _ = app.emit("config", front_config(app));
}

#[tauri::command]
fn get_config(app: AppHandle) -> FrontConfig {
    front_config(&app)
}

#[tauri::command]
fn resize_window(app: AppHandle, width: f64) -> Result<(), String> {
    let win = app.get_webview_window(window::MAIN).ok_or("finestra assente")?;
    window::resize_keep_ratio(&win, width).map_err(|e| e.to_string())
}

#[tauri::command]
fn set_click_through(app: AppHandle, on: bool) {
    window::set_click_through(&app, on);
}

#[tauri::command]
fn set_opacity(app: AppHandle, value: f64) {
    update_config(&app, |c| c.opacity = value.clamp(0.2, 1.0));
}

/// Cambia le hotkey senza riavvio. Se una non si registra, restituisce gli errori.
#[tauri::command]
fn set_hotkeys(app: AppHandle, state: State<AppState>, hotkeys: config::Hotkeys) -> Vec<String> {
    let errors = hotkeys::register(&app, &hotkeys);
    state.cfg.lock().unwrap().hotkeys = hotkeys;
    state.dirty.store(true, Ordering::Relaxed);
    errors
}

fn main() {
    tauri::Builder::default()
        // Seconda istanza: mostra quella già aperta ed esce.
        .plugin(tauri_plugin_single_instance::init(|app, _, _| {
            if let Some(w) = app.get_webview_window(window::MAIN) {
                let _ = w.show();
            }
        }))
        .plugin(hotkeys::plugin())
        .setup(|app| {
            let cfg_path = config::config_path(&app.path().config_dir()?);
            let cfg = config::load(&cfg_path);
            let hk = cfg.hotkeys.clone();
            app.manage(AppState {
                cfg: Mutex::new(cfg),
                cfg_path,
                dirty: AtomicBool::new(false),
                click_through: AtomicBool::new(false),
                handle_hover: AtomicBool::new(false),
                tray_click_item: Mutex::new(None),
                hotkey_map: Mutex::new(HashMap::new()),
                hotkey_errors: Mutex::new(Vec::new()),
            });
            let handle = app.handle().clone();
            window::create_main(&handle)?;
            tray::create(&handle)?;
            for e in hotkeys::register(&handle, &hk) {
                eprintln!("hotkey non registrata: {e}");
            }
            window::spawn_saver(handle.clone());
            window::spawn_handle_poller(handle);
            Ok(())
        })
        .on_window_event(|win, event| match event {
            WindowEvent::Moved(_) | WindowEvent::Resized(_) => window::record_geometry(win),
            // Alt+F4 nasconde invece di chiudere: si esce solo dalla tray.
            WindowEvent::CloseRequested { api, .. } => {
                api.prevent_close();
                let _ = win.hide();
            }
            _ => {}
        })
        .invoke_handler(tauri::generate_handler![
            get_config,
            resize_window,
            set_click_through,
            set_opacity,
            set_hotkeys
        ])
        .run(tauri::generate_context!())
        .expect("errore all'avvio di Alter Ego");
}
