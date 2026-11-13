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
    /// Ragionamento dei modelli "thinking": "auto" (spento per le domande semplici), "hide" (si usa ma
    /// non si vede) o "off" (spento, meno token).
    pub thinking: String,
    /// Token massimi di una risposta (0 = decide il provider).
    pub max_tokens: u32,
    /// Nucleus sampling (1.0 = non inviato).
    pub top_p: f64,
    /// Secondi di attesa per la prima parte della risposta (Ollama aspetta almeno 120 s: carica il modello).
    pub timeout_secs: u64,
    /// Passaggi massimi con gli strumenti per una sola richiesta (richieste complesse).
    pub tool_rounds: usize,
    pub ollama: OllamaOpts,
}

/// Opzioni di Ollama che l'endpoint compatibile OpenAI non accetta: con almeno una impostata
/// si usa l'API nativa `/api/chat` (vedi providers::ollama).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct OllamaOpts {
    /// Finestra di contesto in token (0 = quella del modello).
    pub num_ctx: u32,
    /// Quanto resta caricato il modello dopo una richiesta ("" = default di Ollama, es. "30m", "-1" = sempre).
    pub keep_alive: String,
    /// Livelli del modello sulla GPU (-1 = decide Ollama, 0 = solo CPU).
    pub num_gpu: i32,
    /// Campionamento consigliato da alcuni modelli (es. LFM3: top_k 50, repeat penalty 1.1). 0 = default.
    pub top_k: u32,
    pub repeat_penalty: f64,
}

impl Default for OllamaOpts {
    fn default() -> Self {
        Self { num_ctx: 0, keep_alive: String::new(), num_gpu: -1, top_k: 0, repeat_penalty: 0.0 }
    }
}

impl OllamaOpts {
    /// Serve l'API nativa?
    pub fn native(&self) -> bool {
        self.num_ctx > 0 || !self.keep_alive.trim().is_empty() || self.num_gpu >= 0 || self.top_k > 0 || self.repeat_penalty > 0.0
    }
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
            thinking: "auto".into(),
            max_tokens: 0,
            top_p: 1.0,
            timeout_secs: 45,
            tool_rounds: 12,
            ollama: OllamaOpts::default(),
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
        if !["auto", "hide", "off"].contains(&self.thinking.as_str()) {
            self.thinking = "auto".into();
        }
        self.max_tokens = self.max_tokens.min(200_000);
        self.top_p = if self.top_p <= 0.0 { 1.0 } else { self.top_p.clamp(0.05, 1.0) };
        self.timeout_secs = self.timeout_secs.clamp(10, 600);
        self.tool_rounds = self.tool_rounds.clamp(2, 40);
        self.ollama.num_ctx = if self.ollama.num_ctx == 0 { 0 } else { self.ollama.num_ctx.clamp(512, 1_048_576) };
        self.ollama.keep_alive = self.ollama.keep_alive.trim().chars().filter(|c| c.is_ascii_alphanumeric() || *c == '-').take(8).collect();
        self.ollama.num_gpu = self.ollama.num_gpu.clamp(-1, 999);
        self.ollama.top_k = self.ollama.top_k.min(500);
        self.ollama.repeat_penalty = if self.ollama.repeat_penalty <= 0.0 { 0.0 } else { self.ollama.repeat_penalty.clamp(0.5, 2.0) };
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
    /// Se il primo fallisce: "none", "groq", "gemini" o "local".
    pub fallback: String,
    /// Modello Gemini per la trascrizione (chiavi Gemini).
    pub gemini_model: String,
}

impl Default for SttConfig {
    fn default() -> Self {
        Self {
            provider: "groq".into(),
            model: "whisper-large-v3-turbo".into(),
            base_url: "http://localhost:8000/v1".into(),
            language: "it".into(),
            fallback: "gemini".into(),
            gemini_model: "gemini-2.5-flash".into(),
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
    /// Gestione file su tutto il disco, non solo nelle cartelle in lista
    /// (le eliminazioni e le scritture nelle cartelle di sistema chiedono sempre conferma).
    pub files_all: bool,
    /// Dopo un clic manda al modello uno screenshot di controllo (serve anche "Lettura schermo").
    pub input_feedback: bool,
    /// Righelli con le coordinate e posizione del cursore disegnati negli screenshot.
    pub input_grid: bool,
    /// Secondi massimi per un comando del terminale.
    pub terminal_timeout: u64,
    /// Database SQL Server su questo PC: lettura (SELECT).
    pub sql: bool,
    /// SQL Server: anche comandi che modificano dati o struttura (chiedono conferma).
    pub sql_write: bool,
    /// Mappe dei progetti con Graphify (cartelle della Gestione file).
    pub graph: bool,
    /// Strumento `hermes_task`: delega a Hermes Agent (solo se installato; conferma a ogni uso).
    pub hermes: bool,
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
            files_all: false,
            input_feedback: true,
            input_grid: true,
            terminal_timeout: 120,
            sql: false,
            sql_write: false,
            graph: false,
            hermes: true,
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
    /// Avvia VoiceStudio insieme ad Alter Ego quando è la voce in uso (URL sulla porta 3900).
    pub voicestudio_autostart: bool,
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
            voicestudio_autostart: true,
            output_device: String::new(),
            input_device: String::new(),
            mic_link: MicLink::default(),
        }
    }
}

/// Risparmio di token sulle API (stessa qualità di risposta; vedi README → Risparmio token).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct SavingConfig {
    /// Manda le definizioni degli strumenti solo quando la domanda sembra chiederne uno
    /// (risparmia ~1–3 mila token a messaggio di chiacchiera).
    pub tools_on_demand: bool,
    /// Descrizioni degli strumenti accorciate (solo la prima frase).
    pub compact_tools: bool,
    /// Accorcia le risposte vecchie di Alter Ego nella cronologia (le ultime restano intere).
    pub trim_history: bool,
    /// Fatti sull'utente inviati: solo i più pertinenti alla domanda (0 = tutti).
    pub facts_limit: usize,
    /// Larghezza massima degli screenshot mandati al modello (meno pixel = meno token).
    pub screenshot_width: u32,
    /// Caratteri massimi di una pagina web letta.
    pub page_chars: usize,
    /// Router locale: "off", "rizzo" (Rizzo Flow) o "heuristic" (parole chiave, gratis).
    pub router: String,
    pub router_url: String,
    /// Modello economico per le domande semplici (provider e modello; vuoto = spento).
    pub economy_provider: String,
    pub economy_model: String,
    /// Strumenti a gruppi: si mandano solo quelli che servono alla domanda, gli altri il modello
    /// li carica da solo con `load_tools`.
    pub tool_groups: bool,
    /// Nelle richieste lunghe i risultati vecchi degli strumenti (già usati) vengono accorciati.
    pub loop_compact: bool,
}

impl Default for SavingConfig {
    fn default() -> Self {
        Self {
            tools_on_demand: false,
            compact_tools: false,
            trim_history: true,
            facts_limit: 0,
            screenshot_width: 1280,
            page_chars: 8000,
            router: "heuristic".into(),
            router_url: "http://127.0.0.1:8017".into(),
            economy_provider: String::new(),
            economy_model: String::new(),
            tool_groups: true,
            loop_compact: true,
        }
    }
}

impl SavingConfig {
    pub fn normalize(&mut self) {
        self.screenshot_width = self.screenshot_width.clamp(512, 1920);
        self.page_chars = self.page_chars.clamp(1000, 30000);
        self.facts_limit = self.facts_limit.min(100);
        if !["off", "rizzo", "heuristic"].contains(&self.router.as_str()) {
            self.router = "heuristic".into();
        }
    }
}

/// Rizzo Flow installato da Alter Ego (vedi rizzo.rs).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct RizzoConfig {
    /// Avvia `rizzo serve` all'avvio di Alter Ego.
    pub autostart: bool,
    /// Taglia del modello: "4b" (più preciso) o "1.7b" (più leggero).
    pub size: String,
    /// Cartella di un Rizzo Flow già scaricato ("" = quella gestita da Alter Ego).
    pub dir: String,
}

impl Default for RizzoConfig {
    fn default() -> Self {
        Self { autostart: false, size: "4b".into(), dir: String::new() }
    }
}

/// Spotify Web API (serve Spotify Premium per controllare la riproduzione).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
#[serde(default)]
pub struct SpotifyConfig {
    /// "Metti …" suona su Spotify invece che su YouTube.
    pub enabled: bool,
    /// Client ID dell'app creata su developer.spotify.com (non è un segreto: si usa PKCE).
    pub client_id: String,
    /// Account collegato (il refresh token sta nel Credential Manager).
    pub connected: bool,
    /// Nome dell'account collegato.
    pub account: String,
}

/// Ricerca web: provider, fallback e lettura delle pagine.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct WebConfig {
    /// "deepseek", "gemini" (Google Search), "tavily", "brave", "duckduckgo".
    pub provider: String,
    /// Se il provider fallisce prova gli altri disponibili (DuckDuckGo per ultimo, gratis).
    pub fallback: bool,
    /// Lettura delle pagine: "auto", "deepseek" o "local" (gratis, direttamente dal PC).
    pub fetch: String,
    /// Modello Gemini per la ricerca con Google.
    pub gemini_model: String,
}

impl Default for WebConfig {
    fn default() -> Self {
        Self { provider: "deepseek".into(), fallback: true, fetch: "auto".into(), gemini_model: "gemini-2.5-flash".into() }
    }
}

/// Ollama gestito da Alter Ego: GPU e avvio (vedi localai.rs).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct LocalConfig {
    /// "auto", "vulkan", "rocm" o "cpu".
    pub backend: String,
    /// GPU Vulkan da usare (GGML_VK_VISIBLE_DEVICES, "" = decide Ollama). Serve sui PC con CPU Intel
    /// e grafica integrata: senza, Ollama può scegliere l'iGPU invece della scheda AMD.
    pub vk_device: String,
    pub flash_attention: bool,
    /// Cache K/V: "f16", "q8_0" (metà memoria) o "q4_0".
    pub kv_cache: String,
    /// Linux + ROCm su schede non in elenco (RX 6600/6700: "10.3.0").
    pub hsa_override: String,
    /// Se Ollama è spento, Alter Ego lo avvia insieme a sé.
    pub autostart: bool,
}

impl Default for LocalConfig {
    fn default() -> Self {
        Self { backend: "auto".into(), vk_device: String::new(), flash_attention: true, kv_cache: "q8_0".into(), hsa_override: String::new(), autostart: true }
    }
}

impl LocalConfig {
    pub fn normalize(&mut self) {
        if !["auto", "vulkan", "rocm", "cpu"].contains(&self.backend.as_str()) {
            self.backend = "auto".into();
        }
        if !["f16", "q8_0", "q4_0"].contains(&self.kv_cache.as_str()) {
            self.kv_cache = "q8_0".into();
        }
        self.vk_device = self.vk_device.chars().filter(|c| c.is_ascii_digit() || *c == ',' || *c == '-').take(16).collect();
        self.hsa_override = self.hsa_override.chars().filter(|c| c.is_ascii_digit() || *c == '.').take(10).collect();
    }
}

/// Ultima connessione a SQL Server (la password, se scelto "ricorda", sta nel Credential Manager).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct SqlConfig {
    /// "localhost", "localhost\\SQLEXPRESS", "127.0.0.1,1433"…
    pub server: String,
    pub database: String,
    /// "windows" (accesso integrato, solo Windows) o "sql" (utente e password).
    pub auth: String,
    pub user: String,
    /// Certificato del server considerato attendibile (normale per i server locali).
    pub trust_cert: bool,
    /// Password salvata nel Credential Manager.
    pub remember: bool,
    /// I risultati delle query si aprono anche nella finestra SQL.
    pub show_window: bool,
    /// Righe massime restituite al modello per una query.
    pub max_rows: usize,
}

impl Default for SqlConfig {
    fn default() -> Self {
        Self {
            server: "localhost".into(),
            database: String::new(),
            auth: if cfg!(windows) { "windows".into() } else { "sql".into() },
            user: String::new(),
            trust_cert: true,
            remember: false,
            show_window: true,
            max_rows: 50,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default)]
pub struct Config {
    /// Opacità della finestra, 0.2–1.0.
    pub opacity: f64,
    pub hotkeys: Hotkeys,
    /// Avvio con il sistema (spento di default).
    pub autostart: bool,
    /// Con l'avvio automatico parte nascosta: solo l'icona nella tray.
    pub start_hidden: bool,
    /// Finestra sempre in primo piano.
    pub always_on_top: bool,
    /// Alter Ego sa quali applicazioni principali sono aperte (solo i nomi, in fondo alla domanda).
    pub apps_context: bool,
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
    pub web: WebConfig,
    /// Risparmio di token (vedi providers::saving).
    pub saving: SavingConfig,
    /// Spotify (utenti Premium): controllo della riproduzione via Web API.
    pub spotify: SpotifyConfig,
    /// Reagisce ai suoni di Garry's Mod (Shinri Trial) ascoltando l'audio del PC (spento di default).
    pub gmod: bool,
    pub rizzo: RizzoConfig,
    /// Hermes Agent come "cervello": ogni messaggio va a Hermes (spento di default).
    pub hermes_brain: bool,
    pub sql: SqlConfig,
    pub local: LocalConfig,
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
            start_hidden: true,
            always_on_top: true,
            apps_context: true,
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
            web: WebConfig::default(),
            saving: SavingConfig::default(),
            spotify: SpotifyConfig::default(),
            gmod: false,
            rizzo: RizzoConfig::default(),
            hermes_brain: false,
            sql: SqlConfig::default(),
            local: LocalConfig::default(),
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
        if !["groq", "gemini", "local", "off"].contains(&self.stt.provider.as_str()) {
            self.stt.provider = "groq".into();
        }
        if !["none", "groq", "gemini", "local"].contains(&self.stt.fallback.as_str()) || self.stt.fallback == self.stt.provider {
            self.stt.fallback = "none".into();
        }
        if !["deepseek", "gemini", "tavily", "brave", "duckduckgo"].contains(&self.web.provider.as_str()) {
            self.web.provider = "deepseek".into();
        }
        if !["auto", "deepseek", "local"].contains(&self.web.fetch.as_str()) {
            self.web.fetch = "auto".into();
        }
        self.saving.normalize();
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
        self.permissions.terminal_timeout = self.permissions.terminal_timeout.clamp(10, 1800);
        if !["windows", "sql"].contains(&self.sql.auth.as_str()) || (!cfg!(windows) && self.sql.auth == "windows") {
            self.sql.auth = "sql".into();
        }
        self.sql.max_rows = self.sql.max_rows.clamp(1, 1000);
        self.local.normalize();
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
