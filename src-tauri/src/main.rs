// Niente console in release su Windows.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod actions;
mod alternet;
mod appctl;
mod assistant;
mod audio;
mod config;
mod costs;
mod hotkeys;
mod i18n;
mod keys;
mod memory;
mod music;
mod providers;
mod rizzo;
mod saving;
mod sprites;
mod spotify;
mod stt;
mod tray;
mod update;
mod web;
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
use tauri::{menu::CheckMenuItem, AppHandle, Emitter, Manager, WindowEvent, Wry};

pub struct AppState {
    pub cfg: Mutex<config::Config>,
    pub cfg_path: PathBuf,
    pub dirty: AtomicBool,
    pub click_through: AtomicBool,
    pub handle_hover: AtomicBool,
    /// Finestra di conferma aperta: il mouse deve funzionare anche in click-through.
    pub modal: AtomicBool,
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
    ui_language: String,
    background: String,
    bg_intensity: f64,
    dialog_style: String,
    effects: bool,
    tv_animation: bool,
    auto_advance: bool,
    auto_advance_secs: f64,
    ai: config::AiConfig,
    language: String,
    personality: config::Personality,
    memory_enabled: bool,
    stt: config::SttConfig,
    permissions: config::Permissions,
    clip: config::ClipConfig,
    audio: config::AudioConfig,
    autostart: bool,
    prices: Vec<config::Price>,
    music: config::MusicConfig,
    web: config::WebConfig,
    saving: config::SavingConfig,
    spotify: config::SpotifyConfig,
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
        ui_language: cfg.ui_language.clone(),
        background: cfg.background.clone(),
        bg_intensity: cfg.bg_intensity,
        dialog_style: cfg.dialog_style.clone(),
        effects: cfg.effects,
        tv_animation: cfg.tv_animation,
        auto_advance: cfg.auto_advance,
        auto_advance_secs: cfg.auto_advance_secs,
        ai: cfg.ai.clone(),
        language: cfg.language.clone(),
        personality: cfg.personality.clone(),
        memory_enabled: cfg.memory_enabled,
        stt: cfg.stt.clone(),
        permissions: cfg.permissions.clone(),
        clip: cfg.clip.clone(),
        audio: cfg.audio.clone(),
        autostart: cfg.autostart,
        prices: cfg.prices.clone(),
        music: cfg.music.clone(),
        web: cfg.web.clone(),
        saving: cfg.saving.clone(),
        spotify: cfg.spotify.clone(),
    }
}

/// Modifica la config, la segna da salvare e notifica il frontend.
pub fn update_config(app: &AppHandle, f: impl FnOnce(&mut config::Config)) {
    let state = app.state::<AppState>();
    f(&mut state.cfg.lock().unwrap());
    state.dirty.store(true, Ordering::Relaxed);
    let _ = app.emit("config", front_config(app));
    tray::refresh(app);
}

#[tauri::command]
fn get_config(app: AppHandle) -> FrontConfig {
    front_config(&app)
}

#[tauri::command]
fn resize_window(app: AppHandle, width: f64) -> Result<(), String> {
    if window::is_compact() {
        return Ok(()); // durante la musica la dimensione la decide la modalità compatta
    }
    let win = app.get_webview_window(window::MAIN).ok_or("finestra assente")?;
    window::resize_keep_ratio(&win, width).map_err(|e| e.to_string())
}

/// Lingua dell'interfaccia scelta nelle impostazioni ("auto", "it", "en").
#[tauri::command]
fn set_ui_language(app: AppHandle, lang: String) {
    let lang = if ["it", "en"].contains(&lang.as_str()) { lang } else { "auto".into() };
    update_config(&app, |c| c.ui_language = lang);
}

/// Lingua effettiva (risolta dal frontend per "auto"): menu della tray e messaggi del backend.
#[tauri::command]
fn set_resolved_language(app: AppHandle, lang: String) {
    if i18n::set_en(lang == "en") {
        tray::refresh(&app);
    }
}

/// Modalità compatta della finestra principale (la decide il frontend: musica in corso e Alter Ego inattiva).
#[tauri::command]
fn set_compact(app: AppHandle, on: bool) {
    let enabled = app.state::<AppState>().cfg.lock().unwrap().music.compact;
    window::set_compact(&app, on && enabled);
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
fn set_hotkeys(app: AppHandle, hotkeys: config::Hotkeys) -> Vec<String> {
    let errors = hotkeys::register(&app, &hotkeys);
    update_config(&app, |c| c.hotkeys = hotkeys);
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
async fn open_settings(app: AppHandle) -> Result<(), String> {
    window::open_settings(&app).map_err(|e| e.to_string())
}

/// Dà il focus alla finestra principale (serve per scrivere nell'input di testo).
#[tauri::command]
fn focus_main(app: AppHandle) {
    window::show_main(&app);
    if let Some(w) = app.get_webview_window(window::MAIN) {
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
}

#[tauri::command]
fn providers_info() -> Vec<ProviderView> {
    providers::PROVIDERS
        .iter()
        .map(|p| ProviderView { id: p.id, label: p.label, default_base: p.default_base, needs_key: p.needs_key })
        .collect()
}

/// Aggiunge una chiave al provider (in coda all'ordine di prova).
#[tauri::command]
fn add_api_key(app: AppHandle, provider: String, label: String, key: String) -> Result<(), String> {
    if providers::info(&provider).map(|p| p.needs_key) != Some(true) {
        return Err("provider senza chiave".into());
    }
    let id = format!("{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0));
    keys::set_slot(&provider, &id, &key)?;
    update_config(&app, |c| {
        let pc = c.ai.providers.entry(provider.clone()).or_default();
        let n = pc.keys.len() + 1;
        let label = label.trim();
        pc.keys.push(config::KeySlot { id, label: if label.is_empty() { format!("Chiave {n}") } else { label.chars().take(40).collect() } });
    });
    Ok(())
}

#[tauri::command]
fn delete_api_key(app: AppHandle, provider: String, id: String) -> Result<(), String> {
    keys::delete_slot(&provider, &id)?;
    update_config(&app, |c| {
        if let Some(pc) = c.ai.providers.get_mut(&provider) {
            pc.keys.retain(|k| k.id != id);
        }
    });
    Ok(())
}

/// Sposta una chiave su (-1) o giù (+1) nell'ordine di prova.
#[tauri::command]
fn move_api_key(app: AppHandle, provider: String, id: String, delta: i32) {
    update_config(&app, |c| {
        if let Some(pc) = c.ai.providers.get_mut(&provider) {
            if let Some(i) = pc.keys.iter().position(|k| k.id == id) {
                let j = i as i32 + delta;
                if j >= 0 && (j as usize) < pc.keys.len() {
                    pc.keys.swap(i, j as usize);
                }
            }
        }
    });
}

#[tauri::command]
async fn list_models(app: AppHandle, provider: String) -> Result<Vec<String>, String> {
    let state = app.state::<AppState>();
    let cfg = state.cfg.lock().unwrap().ai.clone();
    providers::list_models(&state.ai, &cfg, &provider).await
}

/// Registro dei costi: ultime righe (dalla più recente) con il costo calcolato dai prezzi attuali.
#[derive(Serialize)]
struct CostRow {
    #[serde(flatten)]
    entry: costs::Entry,
    cost: f64,
}

#[tauri::command]
fn costs_list(app: AppHandle, limit: Option<usize>) -> Vec<CostRow> {
    let path = app.state::<assistant::Conversation>().data_dir.join("costs.jsonl");
    let prices = app.state::<AppState>().cfg.lock().unwrap().prices.clone();
    costs::read(&path, limit.unwrap_or(5000).min(50_000))
        .into_iter()
        .map(|e| CostRow { cost: costs::cost(&prices, &e), entry: e })
        .collect()
}

#[tauri::command]
fn costs_clear(app: AppHandle) -> Result<(), String> {
    let path = app.state::<assistant::Conversation>().data_dir.join("costs.jsonl");
    std::fs::write(path, b"").map_err(|e| e.to_string())
}

/// Esporta il registro dei costi in CSV nella cartella Documenti. Restituisce il percorso.
#[tauri::command]
fn costs_export(app: AppHandle) -> Result<String, String> {
    let rows = costs_list(app.clone(), Some(50_000));
    let mut csv = String::from("data_unix;tipo;provider;modello;input;input_cache;output;chiamate;audio_s;ms;costo_usd\n");
    for r in rows.iter().rev() {
        let e = &r.entry;
        csv.push_str(&format!(
            "{};{};{};{};{};{};{};{};{:.1};{};{:.6}\n",
            e.ts, e.kind, e.provider, e.model, e.input, e.cached, e.output, e.calls, e.audio_secs, e.ms, r.cost
        ));
    }
    let dir = app.path().document_dir().unwrap_or_else(|_| app.state::<assistant::Conversation>().data_dir.clone());
    let path = dir.join(format!("AlterEgo-costi-{}.csv", costs::now()));
    std::fs::write(&path, csv).map_err(|e| e.to_string())?;
    Ok(path.display().to_string())
}

/// Impostazioni di risparmio token.
#[tauri::command]
fn set_saving(app: AppHandle, mut saving: config::SavingConfig) {
    saving.normalize();
    update_config(&app, |c| c.saving = saving);
}

#[tauri::command]
fn set_prices(app: AppHandle, prices: Vec<config::Price>) {
    update_config(&app, |c| c.prices = prices);
}

#[tauri::command]
fn reset_prices(app: AppHandle) {
    update_config(&app, |c| c.prices = config::default_prices());
}

/// Token usati per provider da quando l'app è aperta (con la parte letta dalla cache).
#[tauri::command]
fn ai_usage(app: AppHandle) -> HashMap<String, providers::Usage> {
    app.state::<AppState>().ai.usage()
}

#[tauri::command]
async fn test_provider(app: AppHandle, provider: String) -> Result<String, String> {
    let state = app.state::<AppState>();
    let cfg = state.cfg.lock().unwrap().ai.clone();
    providers::test(&state.ai, &cfg, &provider).await
}

/// Aspetto dalla finestra Impostazioni (lo stesso della tray).
#[tauri::command]
#[allow(clippy::too_many_arguments)]
fn set_look(
    app: AppHandle,
    opacity: f64,
    background: String,
    bg_intensity: f64,
    dialog_style: String,
    effects: bool,
    tv_animation: Option<bool>,
    auto_advance: Option<bool>,
    auto_advance_secs: Option<f64>,
) {
    update_config(&app, |c| {
        c.opacity = opacity.clamp(0.2, 1.0);
        if ["scanline", "circuit", "both"].contains(&background.as_str()) {
            c.background = background;
        }
        c.bg_intensity = bg_intensity.clamp(0.0, 1.0);
        if ["chiaro", "scuro"].contains(&dialog_style.as_str()) {
            c.dialog_style = dialog_style;
        }
        c.effects = effects;
        if let Some(v) = tv_animation {
            c.tv_animation = v;
        }
        if let Some(v) = auto_advance {
            c.auto_advance = v;
        }
        if let Some(v) = auto_advance_secs {
            c.auto_advance_secs = v.clamp(0.5, 30.0);
        }
    });
}

/// Avvio con Windows (spento di default).
#[tauri::command]
fn set_autostart(app: AppHandle, on: bool) -> Result<(), String> {
    use tauri_plugin_autostart::ManagerExt;
    let al = app.autolaunch();
    if on { al.enable() } else { al.disable() }.map_err(|e| e.to_string())?;
    update_config(&app, |c| c.autostart = on);
    Ok(())
}

fn main() {
    // processo elevato avviato da Impostazioni → Voce → Mic-Link: rinomina il cavo ed esce
    #[cfg(windows)]
    if std::env::args().any(|a| a == audio::rename::ARG) {
        std::process::exit(audio::rename::run_elevated());
    }
    tauri::Builder::default()
        // Seconda istanza: mostra quella già aperta ed esce.
        .plugin(tauri_plugin_single_instance::init(|app, _, _| window::show_main(app)))
        .plugin(hotkeys::plugin())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_autostart::init(tauri_plugin_autostart::MacosLauncher::LaunchAgent, None))
        .setup(|app| {
            let cfg_path = config::config_path(&app.path().config_dir()?);
            let data_dir = cfg_path.parent().map(|p| p.to_path_buf()).unwrap_or_default();
            app.manage(actions::Actions::new(&data_dir));
            // cartella degli sprite importati dall'utente (vedi sprites.rs)
            let _ = std::fs::create_dir_all(data_dir.join("sprites"));
            app.manage(audio::kokoro::Kokoro::new(&data_dir));
            app.manage(audio::AudioEngine::new());
            app.manage(music::Music::new(&data_dir));
            let cost_log = data_dir.join("costs.jsonl");
            app.manage(assistant::Conversation::new(data_dir));
            let mut cfg = config::load(&cfg_path);
            // migrazione: chiave singola delle versioni precedenti → slot "1"
            let mut migrated = false;
            for p in providers::PROVIDERS.iter().filter(|p| p.needs_key) {
                let pc = cfg.ai.providers.entry(p.id.to_string()).or_default();
                if pc.keys.is_empty() && keys::migrate_legacy(p.id) {
                    pc.keys.push(config::KeySlot { id: "1".into(), label: "Chiave 1".into() });
                    migrated = true;
                }
            }
            if migrated {
                let _ = config::save(&cfg_path, &cfg);
            }
            let hk = cfg.hotkeys.clone();
            app.manage(AppState {
                cfg: Mutex::new(cfg),
                cfg_path,
                dirty: AtomicBool::new(false),
                click_through: AtomicBool::new(false),
                handle_hover: AtomicBool::new(false),
                modal: AtomicBool::new(false),
                tray_click_item: Mutex::new(None),
                hotkey_map: Mutex::new(HashMap::new()),
                hotkey_errors: Mutex::new(Vec::new()),
                ai: providers::Ai::new(),
            });
            *app.state::<AppState>().ai.cost_log.lock().unwrap() = Some(cost_log.clone());
            *app.state::<assistant::Conversation>().ai_bg.cost_log.lock().unwrap() = Some(cost_log);
            let handle = app.handle().clone();
            audio::commands::apply(&handle);
            rizzo::autostart(&handle);
            window::create_main(&handle)?;
            tray::create(&handle)?;
            for e in hotkeys::register(&handle, &hk) {
                eprintln!("hotkey non registrata: {e}");
            }
            window::spawn_saver(handle.clone());
            actions::spawn_reminder_loop(handle.clone());
            music::spawn_analyzer(handle.clone());
            window::spawn_handle_poller(handle);
            Ok(())
        })
        .on_window_event(|win, event| match event {
            WindowEvent::Moved(_) | WindowEvent::Resized(_) if win.label() == window::MAIN => {
                window::record_geometry(win)
            }
            WindowEvent::Moved(_) if win.label() == music::PLAYER => music::record_player_position(win),
            WindowEvent::Moved(_) if win.label() == music::VENUE => music::record_venue_position(win),
            WindowEvent::Destroyed if win.label() == music::PLAYER || win.label() == music::VENUE => {
                music::window_destroyed(win.app_handle(), win.label())
            }
            // Alt+F4 nasconde invece di chiudere: si esce solo dalla tray.
            WindowEvent::CloseRequested { api, .. } if win.label() == window::MAIN => {
                api.prevent_close();
                window::hide_main(win.app_handle(), false);
            }
            _ => {}
        })
        .invoke_handler(tauri::generate_handler![
            get_config,
            resize_window,
            set_click_through,
            set_compact,
            set_ui_language,
            set_resolved_language,
            set_opacity,
            set_hotkeys,
            set_general,
            set_ai_config,
            open_settings,
            focus_main,
            providers_info,
            add_api_key,
            delete_api_key,
            move_api_key,
            list_models,
            test_provider,
            ai_usage,
            costs_list,
            costs_clear,
            costs_export,
            set_prices,
            reset_prices,
            music::music_state,
            music::music_toggle,
            music::music_next,
            music::music_prev,
            music::music_seek,
            music::music_volume,
            music::music_shuffle,
            music::music_repeat,
            music::music_stop,
            music::music_play,
            music::music_settings,
            music::player_closed,
            music::player_resize,
            music::venue_open,
            music::player_expand,
            music::venue_detach,
            music::venue_dock,
            music::venue_close,
            music::venue_closed,
            music::venue_library,
            music::venue_favorite,
            music::venue_remove,
            music::venue_play,
            update::update_check,
            update::update_install,
            update::open_link_cmd,
            update::app_version,
            alternet::alternet_open,
            alternet::alternet_info,
            rizzo::rizzo_status,
            rizzo::rizzo_install,
            rizzo::rizzo_start,
            rizzo::rizzo_stop,
            rizzo::rizzo_settings,
            rizzo::rizzo_open_dir,
            web::set_web_key,
            web::web_keys,
            web::set_web_config,
            spotify::spotify_connect,
            spotify::spotify_disconnect,
            spotify::spotify_settings,
            spotify::spotify_status,
            set_saving,
            saving::rizzo_test,
            assistant::chat,
            assistant::set_persona_options,
            assistant::set_stt,
            assistant::memory_overview,
            assistant::memory_add_fact,
            assistant::memory_delete_fact,
            assistant::memory_forget_all,
            assistant::memory_export,
            assistant::open_data_dir,
            assistant::stt_test,
            assistant::set_permissions,
            assistant::set_clip,
            assistant::test_clip,
            assistant::set_audio,
            assistant::set_tts_key,
            assistant::has_tts_key,
            audio::commands::audio_devices,
            audio::commands::audio_status,
            audio::commands::audio_sfx,
            audio::commands::audio_stop,
            audio::commands::tts_say,
            audio::commands::tts_voices,
            audio::commands::kokoro_status,
            audio::commands::kokoro_install,
            audio::commands::kokoro_start,
            audio::commands::kokoro_stop,
            audio::commands::mic_link_rename,
            audio::commands::open_vbcable_page,
            actions::confirm_action,
            actions::actions_log,
            actions::clear_actions_log,
            set_autostart,
            set_look,
            window::tv_done,
            assistant::load_user_sprites,
            assistant::import_sprite_sheet,
            assistant::remove_user_sprites,
            assistant::open_sprite_dir,
            assistant::reload_sprites
        ])
        .build(tauri::generate_context!())
        .expect("errore all'avvio di Alter Ego")
        .run(|app, event| {
            if let tauri::RunEvent::Exit = event {
                // il server Kokoro avviato da noi si chiude con l'app
                app.state::<audio::kokoro::Kokoro>().stop();
                rizzo::stop(app);
            }
        });
}
