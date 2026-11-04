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

/// Una chiave API: qui solo id ed etichetta, il segreto sta nel Credential Manager.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(default)]
pub struct KeySlot {
    pub id: String,
    pub label: String,
}

/// Impostazioni di un provider AI. Le chiavi NON stanno qui (Credential Manager).
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(default)]
pub struct ProviderCfg {
    pub enabled: bool,
    /// Chiavi in ordine di prova: se una fallisce (401/403/429/quota) passa alla successiva.
    pub keys: Vec<KeySlot>,
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
                model("deepseek", "deepseek-flash", false),
                model("ollama", "", true),
            ]),
            temperature: 0.7,
            max_history: 12,
        }
    }
}

impl ProviderCfg {
    pub fn key_ids(&self) -> Vec<String> {
        self.keys.iter().map(|k| k.id.clone()).collect()
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
    /// "Senza filtri": fa ciò che le chiedi anche se non è d'accordo (permessi e conferme restano).
    pub unfiltered: bool,
}

impl Default for Personality {
    fn default() -> Self {
        Self { confidence: 0.3, warmth: 0.7, imitate: String::new(), unfiltered: false }
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

/// Voce di una lista consentita: nome mostrato al modello → programma/sito/comando.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(default)]
pub struct Named {
    pub name: String,
    pub target: String,
}

/// Permessi delle azioni sul PC: tutto spento di default (vedi docs/PERMESSI.md).
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default)]
pub struct Permissions {
    pub open: bool,
    pub open_allowlist: Vec<Named>,
    pub files: bool,
    pub folders: Vec<String>,
    pub reminders: bool,
    pub clipboard: bool,
    pub screen: bool,
    pub volume: bool,
    pub shell: bool,
    pub commands: Vec<Named>,
    /// Esegue senza chiedere le azioni che di solito chiedono conferma
    /// (restano valide le autorizzazioni per tipo di azione e le cartelle consentite).
    pub auto_approve: bool,
    /// Terminale: comandi PowerShell o cmd scritti dal modello.
    pub terminal: bool,
    /// Terminale senza conferma (i comandi distruttivi la chiedono comunque).
    pub terminal_autonomous: bool,
    /// Ricerca web e lettura di pagine (API Search/Fetch di DeepSeek).
    pub web: bool,
    /// Cerca musica su YouTube e la riproduce.
    pub music: bool,
    /// Mouse e tastiera: Alter Ego muove il cursore, clicca, scrive e preme tasti.
    pub input: bool,
    /// Mouse e tastiera senza conferma per ogni azione.
    pub input_autonomous: bool,
    /// Comandi dell'app a voce/testo ("apri Music Venue", "chiudi il lettore"…). Riguardano solo
    /// Alter Ego, non il PC: acceso di default.
    pub app_control: bool,
}

impl Default for Permissions {
    fn default() -> Self {
        Self {
            open: false,
            open_allowlist: Vec::new(),
            files: false,
            folders: Vec::new(),
            reminders: false,
            clipboard: false,
            screen: false,
            volume: false,
            shell: false,
            commands: Vec::new(),
            auto_approve: false,
            terminal: false,
            terminal_autonomous: false,
            web: false,
            music: false,
            input: false,
            input_autonomous: false,
            app_control: true,
        }
    }
}

/// Prezzo di un modello o servizio, in dollari. `model` vuoto = tutti i modelli del provider;
/// altrimenti vale per i modelli che iniziano così.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(default)]
pub struct Price {
    pub provider: String,
    pub model: String,
    /// $ per milione di token di input (non in cache).
    pub input: f64,
    /// $ per milione di token di input letti dalla cache.
    pub cached: f64,
    /// $ per milione di token di output.
    pub output: f64,
    /// $ per chiamata (ricerca/pagina web).
    pub per_call: f64,
    /// $ per ora di audio (trascrizione).
    pub per_hour: f64,
}

/// Prezzi iniziali (modificabili in Impostazioni → Costi). Vanno verificati sui siti dei provider.
pub fn default_prices() -> Vec<Price> {
    let p = |provider: &str, model: &str, input: f64, cached: f64, output: f64| Price {
        provider: provider.into(), model: model.into(), input, cached, output, ..Default::default()
    };
    vec![
        // DeepSeek V4.1 Flash, tariffa piena (fuori dalla fascia scontata), ottobre 2026
        p("deepseek", "deepseek-flash", 0.30, 0.006, 1.20),
        p("deepseek", "", 0.30, 0.006, 1.20),
        Price { provider: "deepseek".into(), model: "search".into(), per_call: 0.00075, ..Default::default() },
        Price { provider: "deepseek".into(), model: "fetch".into(), per_call: 0.00075, ..Default::default() },
        Price { provider: "groq".into(), model: "whisper".into(), per_hour: 0.04, ..Default::default() },
        p("ollama", "", 0.0, 0.0, 0.0),
    ]
}

/// Lettore musicale.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct MusicConfig {
    /// Posizione del cursore del volume (0–1); il guadagno vero segue una curva (vedi music::gain).
    pub volume: f64,
    pub shuffle: bool,
    /// "off", "one" (ripete il brano), "all" (ripete la coda).
    pub repeat: String,
    /// Testi sincronizzati da LRCLIB.
    pub lyrics: bool,
    /// Il volto di Alter Ego nel lettore cambia con la musica.
    pub reactive_sprite: bool,
    /// Posizione del lettore (pixel fisici).
    pub player_pos: Option<(i32, i32)>,
    /// Durante la musica la finestra di Alter Ego si rimpicciolisce (torna grande quando parla o scrivi).
    pub compact: bool,
    /// Larghezza del lettore in px logici (si ridimensiona mantenendo le proporzioni).
    pub player_width: f64,
    /// Posizione della Music Venue (pixel fisici).
    pub venue_pos: Option<(i32, i32)>,
    /// Sprite "con le cuffie" nella finestra principale mentre suona la musica.
    pub music_sprite: bool,
    /// La Music Venue si apre dentro il lettore (true) o in una finestra separata.
    pub venue_docked: bool,
}

impl Default for MusicConfig {
    fn default() -> Self {
        Self {
            volume: 0.6,
            shuffle: false,
            repeat: "off".into(),
            lyrics: true,
            reactive_sprite: true,
            player_pos: None,
            compact: true,
            player_width: 600.0,
            venue_pos: None,
            music_sprite: true,
            venue_docked: true,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(default)]
pub struct ClipTarget {
    pub name: String,
    pub hotkey: String,
    pub enabled: bool,
}

/// Clip vocali: frasi riconosciute in locale (senza modello) → scorciatoia del programma di registrazione.
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default)]
pub struct ClipConfig {
    pub enabled: bool,
    pub phrases: Vec<String>,
    pub targets: Vec<ClipTarget>,
    /// Risposta di Alter Ego dopo la clip.
    pub reply: String,
}

impl Default for ClipConfig {
    fn default() -> Self {
        let t = |name: &str, hotkey: &str| ClipTarget { name: name.into(), hotkey: hotkey.into(), enabled: false };
        Self {
            enabled: true,
            phrases: ["alter ego clip that", "alter ego clippa", "alter ego clip", "alter ego salva la clip", "clip that", "clippa"]
                .map(String::from)
                .to_vec(),
            targets: vec![t("AMD Adrenalin (Instant Replay)", "Ctrl+Shift+S"), t("NVIDIA App / ShadowPlay", "Alt+F10"), t("Medal", "F8")],
            reply: "Clip salvata!".into(),
        }
    }
}

/// Mic-Link: microfono + voce di Alter Ego mixati in un cavo audio virtuale (VB-CABLE),
/// che le altre app (Discord, OBS, giochi…) vedono come microfono "AlterEgo Link Audio".
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct MicLink {
    pub enabled: bool,
    /// Microfono da mixare ("" = predefinito di Windows).
    pub mic_device: String,
    /// Lato "riproduzione" del cavo virtuale ("" = trova da solo CABLE Input).
    pub cable_device: String,
    pub mic_volume: f64,
    pub voice_volume: f64,
    /// Anche i blip del dialogo finiscono nel microfono virtuale.
    pub include_blips: bool,
    /// Anche la musica del lettore finisce nel microfono virtuale.
    pub include_music: bool,
    /// Riduzione del rumore del microfono (RNNoise), solo sul microfono di Mic-Link.
    pub noise_suppression: bool,
    /// Quanto rumore togliere: 0 = niente, 1 = tutto quello che il modello trova.
    pub ns_strength: f64,
    /// Soglia voce (0–1): sotto questa probabilità di voce il microfono si abbassa (gate).
    pub ns_gate: f64,
}

impl Default for MicLink {
    fn default() -> Self {
        Self {
            enabled: false,
            mic_device: String::new(),
            cable_device: String::new(),
            mic_volume: 1.0,
            voice_volume: 0.9,
            include_blips: false,
            include_music: false,
            noise_suppression: false,
            ns_strength: 0.9,
            ns_gate: 0.3,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default)]
pub struct AudioConfig {
    /// Suoni del dialogo: "dr" (stile Danganronpa, sintetizzato), "soft" (originale), "off".
    pub blips: String,
    pub blip_volume: f64,
    /// Voce: "off", "windows" (voci di sistema), "kokoro" (Kokoro-82M locale)
    /// o "openai" (qualsiasi endpoint compatibile /audio/speech).
    pub tts: String,
    pub tts_voice: String,
    pub tts_rate: f64,
    pub tts_volume: f64,
    pub tts_url: String,
    pub tts_model: String,
    /// Durante la voce i blip tacciono.
    pub mute_blips_with_tts: bool,
    /// Kokoro-82M: server locale (installato e avviato dall'app, oppure Kokoro-FastAPI).
    pub kokoro_url: String,
    pub kokoro_voice: String,
    pub kokoro_voice_en: String,
    pub kokoro_speed: f64,
    /// Avvia il server Kokoro insieme ad Alter Ego (se installato).
    pub kokoro_autostart: bool,
    /// Altoparlanti/cuffie per voce e suoni ("" = predefinito di Windows).
    pub output_device: String,
    /// Microfono per il push-to-talk ("" = predefinito di Windows).
    pub input_device: String,
    pub mic_link: MicLink,
}

impl Default for AudioConfig {
    fn default() -> Self {
        Self {
            blips: "dr".into(),
            blip_volume: 0.35,
            tts: "off".into(),
            tts_voice: String::new(),
            tts_rate: 1.05,
            tts_volume: 0.9,
            tts_url: "http://localhost:8880/v1".into(),
            tts_model: "tts-1".into(),
            mute_blips_with_tts: true,
            kokoro_url: "http://127.0.0.1:8880/v1".into(),
            kokoro_voice: "if_sara".into(),
            kokoro_voice_en: "af_heart".into(),
            kokoro_speed: 1.0,
            kokoro_autostart: true,
            output_device: String::new(),
            input_device: String::new(),
            mic_link: MicLink::default(),
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
    /// Lingua dell'interfaccia: "auto" (lingua di Windows), "it" o "en".
    pub ui_language: String,
    /// Sfondo: "scanline", "circuit" o "both".
    pub background: String,
    /// Intensità degli effetti di sfondo, 0.0–1.0.
    pub bg_intensity: f64,
    /// Stile box dialogo: "chiaro" (come lo screenshot) o "scuro".
    pub dialog_style: String,
    /// Flicker e rumore animati (spegnili per CPU minima).
    pub effects: bool,
    /// Animazione da vecchia TV quando la finestra appare/scompare.
    pub tv_animation: bool,
    /// Il dialogo va avanti da solo se non si clicca.
    pub auto_advance: bool,
    /// Attesa (secondi) a pagina completa prima di andare avanti; si aggiunge il tempo di lettura.
    pub auto_advance_secs: f64,
    pub ai: AiConfig,
    /// Lingua risposte: "auto" (come l'interfaccia), "it", "it_en" (italiano + traduzione) o "en".
    pub language: String,
    pub personality: Personality,
    /// Memoria persistente (SQLite). Spenta = si ricorda solo la sessione corrente.
    pub memory_enabled: bool,
    pub stt: SttConfig,
    pub permissions: Permissions,
    pub clip: ClipConfig,
    pub audio: AudioConfig,
    pub prices: Vec<Price>,
    pub music: MusicConfig,
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
            ui_language: "auto".into(),
            background: "both".into(),
            bg_intensity: 0.6,
            dialog_style: "chiaro".into(),
            effects: true,
            tv_animation: true,
            auto_advance: true,
            auto_advance_secs: 3.0,
            ai: AiConfig::default(),
            language: "auto".into(),
            personality: Personality::default(),
            memory_enabled: true,
            stt: SttConfig::default(),
            permissions: Permissions::default(),
            prices: default_prices(),
            music: MusicConfig::default(),
            clip: ClipConfig::default(),
            audio: AudioConfig::default(),
            last_monitor: None,
            geometry: HashMap::new(),
        }
    }
}

impl Config {
    pub fn normalize_voice(&mut self) {
        if !["auto", "it", "it_en", "en"].contains(&self.language.as_str()) {
            self.language = "it".into();
        }
        self.personality.confidence = self.personality.confidence.clamp(0.0, 1.0);
        self.personality.warmth = self.personality.warmth.clamp(0.0, 1.0);
        self.personality.imitate = self.personality.imitate.trim().chars().take(60).collect();
        if !["groq", "local", "off"].contains(&self.stt.provider.as_str()) {
            self.stt.provider = "groq".into();
        }
        if !["dr", "soft", "off"].contains(&self.audio.blips.as_str()) {
            self.audio.blips = "dr".into();
        }
        if !["off", "windows", "kokoro", "openai"].contains(&self.audio.tts.as_str()) {
            self.audio.tts = "off".into();
        }
        self.audio.kokoro_speed = self.audio.kokoro_speed.clamp(0.5, 2.0);
        let ml = &mut self.audio.mic_link;
        ml.mic_volume = ml.mic_volume.clamp(0.0, 2.0);
        ml.voice_volume = ml.voice_volume.clamp(0.0, 1.5);
        ml.ns_strength = ml.ns_strength.clamp(0.0, 1.0);
        ml.ns_gate = ml.ns_gate.clamp(0.0, 0.95);
        self.audio.blip_volume = self.audio.blip_volume.clamp(0.0, 1.0);
        self.audio.tts_volume = self.audio.tts_volume.clamp(0.0, 1.0);
        self.audio.tts_rate = self.audio.tts_rate.clamp(0.5, 2.0);
        self.clip.phrases.retain(|p| !p.trim().is_empty());
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
            cfg.auto_advance_secs = cfg.auto_advance_secs.clamp(0.5, 30.0);
            cfg.music.player_width = cfg.music.player_width.clamp(crate::music::PLAYER_MIN_W, crate::music::PLAYER_MAX_W);
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
