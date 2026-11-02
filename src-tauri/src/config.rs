//! Configurazione locale: `%APPDATA%\AlterEgo\config.json`.
//! Nessun segreto qui dentro: le chiavi API andranno nel Credential Manager (fase 4).

use serde::{Deserialize, Serialize};
use std::{collections::{BTreeMap, HashMap}, fs, path::{Path, PathBuf}};

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

/// Impostazioni di un provider AI. La chiave NON sta qui (Credential Manager).
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(default)]
pub struct ProviderCfg {
    pub enabled: bool,
    /// Vuoto = default (Ollama: primo modello installato).
    pub model: String,
    /// Vuoto = URL predefinito del provider.
    pub base_url: String,
    /// Il modello accetta immagini (screenshot).
    pub vision: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default)]
pub struct AiConfig {
    /// Ordine di fallback: il primo attivo è il provider principale.
    pub order: Vec<String>,
    pub providers: BTreeMap<String, ProviderCfg>,
    pub temperature: f64,
    /// Messaggi recenti inviati come contesto.
    pub max_history: usize,
}

impl Default for AiConfig {
    fn default() -> Self {
        let model = |id: &str, m: &str, enabled: bool| {
            (id.to_string(), ProviderCfg { enabled, model: m.into(), ..Default::default() })
        };
        Self {
            order: ["gemini", "openrouter", "groq", "deepseek", "ollama"].map(String::from).to_vec(),
            providers: BTreeMap::from([
                model("gemini", "gemini-2.5-flash", false),
                model("openrouter", "openrouter/auto", false),
                model("groq", "llama-3.3-70b-versatile", false),
                model("deepseek", "deepseek-chat", false),
                model("ollama", "", true),
            ]),
            temperature: 0.7,
            max_history: 12,
        }
    }
}

impl AiConfig {
    /// Ripara config incomplete o modificate a mano.
    pub fn normalize(&mut self) {
        let all = ["gemini", "openrouter", "groq", "deepseek", "ollama"];
        self.order.retain(|id| all.contains(&id.as_str()));
        self.order.dedup();
        for id in all {
            if !self.order.iter().any(|o| o == id) {
                self.order.push(id.into());
            }
            self.providers.entry(id.into()).or_default();
        }
        self.temperature = self.temperature.clamp(0.0, 2.0);
        self.max_history = self.max_history.clamp(2, 50);
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default)]
pub struct Personality {
    /// 0 = timida, 1 = sicura.
    pub confidence: f64,
    /// 0 = formale, 1 = affettuosa.
    pub warmth: f64,
    /// Modalità imitazione: nome della persona da imitare (vuoto = spenta).
    pub imitate: String,
}

impl Default for Personality {
    fn default() -> Self {
        Self { confidence: 0.3, warmth: 0.7, imitate: String::new() }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default)]
pub struct SttConfig {
    /// "groq", "local" o "off".
    pub provider: String,
    pub model: String,
    /// Server Whisper locale compatibile OpenAI (solo provider "local").
    pub base_url: String,
    pub language: String,
}

impl Default for SttConfig {
    fn default() -> Self {
        Self {
            provider: "groq".into(),
            model: "whisper-large-v3-turbo".into(),
            base_url: "http://localhost:8000/v1".into(),
            language: "it".into(),
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
    /// Nome dell'utente, usato da Alter Ego nelle risposte.
    pub user_name: String,
    /// Tag nome mostrato nel box dialogo.
    pub name_tag: String,
    /// Sfondo: "scanline", "circuit" o "both".
    pub background: String,
    /// Intensità degli effetti di sfondo, 0.0–1.0.
    pub bg_intensity: f64,
    /// Stile box dialogo: "chiaro" (come lo screenshot) o "scuro".
    pub dialog_style: String,
    /// Flicker e rumore animati (spegnili per CPU minima).
    pub effects: bool,
    pub ai: AiConfig,
    /// Lingua risposte: "it", "it_en" (italiano + traduzione) o "en".
    pub language: String,
    pub personality: Personality,
    /// Memoria persistente (SQLite). Spenta = si ricorda solo la sessione corrente.
    pub memory_enabled: bool,
    pub stt: SttConfig,
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
            user_name: String::new(),
            name_tag: "Alter Ego".into(),
            background: "both".into(),
            bg_intensity: 0.6,
            dialog_style: "chiaro".into(),
            effects: true,
            ai: AiConfig::default(),
            language: "it".into(),
            personality: Personality::default(),
            memory_enabled: true,
            stt: SttConfig::default(),
            last_monitor: None,
            geometry: HashMap::new(),
        }
    }
}

impl Config {
    pub fn normalize_voice(&mut self) {
        if !["it", "it_en", "en"].contains(&self.language.as_str()) {
            self.language = "it".into();
        }
        self.personality.confidence = self.personality.confidence.clamp(0.0, 1.0);
        self.personality.warmth = self.personality.warmth.clamp(0.0, 1.0);
        self.personality.imitate = self.personality.imitate.trim().chars().take(60).collect();
        if !["groq", "local", "off"].contains(&self.stt.provider.as_str()) {
            self.stt.provider = "groq".into();
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
            cfg.bg_intensity = cfg.bg_intensity.clamp(0.0, 1.0);
            cfg.ai.normalize();
            cfg.normalize_voice();
            if !["scanline", "circuit", "both"].contains(&cfg.background.as_str()) {
                cfg.background = "both".into();
            }
            if !["chiaro", "scuro"].contains(&cfg.dialog_style.as_str()) {
                cfg.dialog_style = "chiaro".into();
            }
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
