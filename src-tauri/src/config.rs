//! Configurazione locale: `%APPDATA%\AlterEgo\config.json`.
//! Nessun segreto qui dentro: le chiavi API andranno nel Credential Manager (fase 4).

use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fs, path::{Path, PathBuf}};

/// Geometria della finestra in pixel fisici.
#[derive(Serialize, Deserialize, Clone, Copy, Debug)]
pub struct Geometry {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default)]
pub struct Hotkeys {
    pub toggle_visibility: String,
    pub push_to_talk: String,
    pub text_input: String,
    pub toggle_click_through: String,
}

impl Default for Hotkeys {
    fn default() -> Self {
        Self {
            toggle_visibility: "Ctrl+Alt+A".into(),
            push_to_talk: "Ctrl+Alt+Space".into(),
            text_input: "Ctrl+Alt+T".into(),
            toggle_click_through: "Ctrl+Alt+C".into(),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default)]
pub struct Config {
    /// Opacità della finestra, 0.2–1.0.
    pub opacity: f64,
    pub hotkeys: Hotkeys,
    /// Avvio con Windows: disattivato di default (non ancora implementato).
    pub autostart: bool,
    /// Ultimo monitor usato (nome di sistema, es. `\\.\DISPLAY1`).
    pub last_monitor: Option<String>,
    /// Posizione/dimensione salvate per ogni monitor.
    pub geometry: HashMap<String, Geometry>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            opacity: 1.0,
            hotkeys: Hotkeys::default(),
            autostart: false,
            last_monitor: None,
            geometry: HashMap::new(),
        }
    }
}

pub fn config_path(base_config_dir: &Path) -> PathBuf {
    base_config_dir.join("AlterEgo").join("config.json")
}

/// Carica la config. Se il file è corrotto lo rinomina in `.bak` e riparte dai default.
pub fn load(path: &Path) -> Config {
    let Ok(text) = fs::read_to_string(path) else {
        return Config::default();
    };
    match serde_json::from_str::<Config>(&text) {
        Ok(mut cfg) => {
            cfg.opacity = cfg.opacity.clamp(0.2, 1.0);
            cfg
        }
        Err(e) => {
            eprintln!("config.json non valido ({e}), uso i default");
            let _ = fs::rename(path, path.with_extension("json.bak"));
            Config::default()
        }
    }
}

/// Scrittura atomica: file temporaneo + rename.
pub fn save(path: &Path, cfg: &Config) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, serde_json::to_vec_pretty(cfg)?)?;
    fs::rename(&tmp, path)
}
