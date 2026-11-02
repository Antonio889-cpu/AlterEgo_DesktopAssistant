// Niente console in release su Windows.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod assistant;
mod config;
mod hotkeys;
mod keys;
mod memory;
mod providers;
mod stt;
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
    pub ai: providers::Ai,
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
    ai: config::AiConfig,
    language: String,
    personality: config::Personality,
    memory_enabled: bool,
    stt: config::SttConfig,
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
        ai: cfg.ai.clone(),
        language: cfg.language.clone(),
        personality: cfg.personality.clone(),
        memory_enabled: cfg.memory_enabled,
        stt: cfg.stt.clone(),
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

// ---------- impostazioni ----------

#[tauri::command]
fn set_general(app: AppHandle, user_name: String, name_tag: String) {
    update_config(&app, |c| {
        c.user_name = user_name.trim().chars().take(40).collect();
        let tag = name_tag.trim();
        c.name_tag = if tag.is_empty() { "Alter Ego".into() } else { tag.chars().take(24).collect() };
    });
}

#[tauri::command]
fn set_ai_config(app: AppHandle, mut ai: config::AiConfig) {
    ai.normalize();
    update_config(&app, |c| c.ai = ai);
}

#[tauri::command]
fn open_settings(app: AppHandle) -> Result<(), String> {
    window::open_settings(&app).map_err(|e| e.to_string())
}

/// Dà il focus alla finestra principale (serve per scrivere nell'input di testo).
#[tauri::command]
fn focus_main(app: AppHandle) {
    if let Some(w) = app.get_webview_window(window::MAIN) {
        let _ = w.show();
        let _ = w.set_focus();
    }
}

// ---------- AI ----------

#[derive(Serialize)]
struct ProviderView {
    id: &'static str,
    label: &'static str,
    default_base: &'static str,
    needs_key: bool,
    has_key: bool,
}

#[tauri::command]
fn providers_info() -> Vec<ProviderView> {
    providers::PROVIDERS
        .iter()
        .map(|p| ProviderView {
            id: p.id,
            label: p.label,
            default_base: p.default_base,
            needs_key: p.needs_key,
            has_key: p.needs_key && keys::has(p.id),
        })
        .collect()
}

/// Stato delle chiavi: solo sì/no, la chiave non esce mai dal backend.
#[tauri::command]
fn key_status() -> HashMap<String, bool> {
    providers::PROVIDERS
        .iter()
        .filter(|p| p.needs_key)
        .map(|p| (p.id.to_string(), keys::has(p.id)))
        .collect()
}

/// Salva la chiave nel Credential Manager. Stringa vuota = cancella.
#[tauri::command]
fn set_api_key(provider: String, key: String) -> Result<(), String> {
    if providers::info(&provider).map(|p| p.needs_key) != Some(true) {
        return Err("provider senza chiave".into());
    }
    keys::set(&provider, &key)
}

#[tauri::command]
async fn list_models(app: AppHandle, provider: String) -> Result<Vec<String>, String> {
    let state = app.state::<AppState>();
    let cfg = state.cfg.lock().unwrap().ai.clone();
    providers::list_models(&state.ai, &cfg, &provider).await
}

#[tauri::command]
async fn test_provider(app: AppHandle, provider: String) -> Result<String, String> {
    let state = app.state::<AppState>();
    let cfg = state.cfg.lock().unwrap().ai.clone();
    providers::test(&state.ai, &cfg, &provider).await
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
            let data_dir = cfg_path.parent().map(|p| p.to_path_buf()).unwrap_or_default();
            app.manage(assistant::Conversation::new(data_dir));
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
                ai: providers::Ai::new(),
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
            WindowEvent::Moved(_) | WindowEvent::Resized(_) if win.label() == window::MAIN => {
                window::record_geometry(win)
            }
            // Alt+F4 nasconde invece di chiudere: si esce solo dalla tray.
            WindowEvent::CloseRequested { api, .. } if win.label() == window::MAIN => {
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
            set_hotkeys,
            set_general,
            set_ai_config,
            open_settings,
            focus_main,
            key_status,
            providers_info,
            set_api_key,
            list_models,
            test_provider,
            assistant::chat,
            assistant::set_persona_options,
            assistant::set_stt,
            assistant::memory_overview,
            assistant::memory_add_fact,
            assistant::memory_delete_fact,
            assistant::memory_forget_all,
            assistant::memory_export,
            assistant::open_data_dir,
            assistant::stt_test
        ])
        .run(tauri::generate_context!())
        .expect("errore all'avvio di Alter Ego");
}
