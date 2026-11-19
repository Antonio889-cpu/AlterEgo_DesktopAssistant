// Finto window.__TAURI__ per provare le finestre in un browser normale (stress test, screenshot).
// Si carica PRIMA degli script della pagina (vedi stress.mjs). Ogni invoke risponde con dati
// verosimili; i comandi sconosciuti rispondono null senza errori.
(() => {
  const params = new URLSearchParams(location.search);
  const LONG = params.has("long"); // testi lunghissimi per gli stress test
  const MANY = params.has("many"); // liste enormi
  const cfg = window.__AE_CONFIG__ || {};
  if (params.get("lang")) cfg.ui_language = params.get("lang");
  if (cfg.ai && cfg.ai.providers) {
    const p = cfg.ai.providers.deepseek || Object.values(cfg.ai.providers)[0];
    if (p) { p.enabled = true; p.model = LONG ? "deepseek-chat-v3.1-terminus-extra-long-model-name-for-testing" : "deepseek-chat"; }
  }
  const long = (s) => (LONG ? s + " " + "lunghissimo-".repeat(12) : s);
  const n = MANY ? 400 : 6;
  const now = Math.floor(Date.now() / 1000);
  const models = Array.from({ length: n }, (_, i) => ({
    name: long(["lfm3:1.7b", "gemma3:4b", "qwen3:8b", "llama3.2:3b", "mistral-small3.1:24b", "deepseek-r1:8b"][i % 6] + (i >= 6 ? "-" + i : "")),
    size: (1 + (i % 9)) * 1.1e9, params: ["1.7B", "4B", "8B", "3B", "24B", "8B"][i % 6], quant: "Q4_K_M", family: "llama",
    gpu_percent: i === 0 ? 100 : null,
  }));
  const R = {
    get_config: () => structuredClone(cfg),
    app_version: () => "1.14.0",
    local_status: () => ({ installed: true, running: true, version: "0.11.10", lfm3_ok: true, managed: true, binary: "ollama", base: "http://127.0.0.1:11434", models, busy: false }),
    gmodlink_status: () => ({ dir: "C:/Steam/steamapps/common/GarrysMod/garrysmod", installed: true, game_running: true, linked: true, in_game: true, lambda: true, body: "lambda", games: 12, persona: { habits: [] }, fly: null }),
    memory_overview: () => ({ messages: MANY ? 98231 : 1240, facts: Array.from({ length: n }, (_, i) => ({ id: i, ts: now - i * 3600, content: long("Ama il caffè e programma in Rust " + i) })), summaries: [], db_path: "C:/Users/makoto/AppData/Roaming/AlterEgo/memory.db" }),
    costs_list: () => Array.from({ length: MANY ? 5000 : 40 }, (_, i) => ({ ts: now - i * 900, kind: "chat", provider: "deepseek", model: "deepseek-chat", input: 1800 + i, cached: 12000, output: 120, ms: 900, cost: 0.0004 })),
    ai_usage: () => ({ deepseek: { requests: 42, input: 51234, cached: 40100 } }),
    providers_info: () => [
      { id: "gemini", label: "Google Gemini", default_base: "https://generativelanguage.googleapis.com/v1beta/openai", needs_key: true },
      { id: "openrouter", label: "OpenRouter", default_base: "https://openrouter.ai/api/v1", needs_key: true },
      { id: "groq", label: "Groq", default_base: "https://api.groq.com/openai/v1", needs_key: true },
      { id: "deepseek", label: "DeepSeek", default_base: "https://api.deepseek.com/v1", needs_key: true },
      { id: "ollama", label: "Ollama (locale)", default_base: "http://localhost:11434/v1", needs_key: false },
    ],
    list_models: () => ["deepseek-chat", "deepseek-reasoner"],
    tts_voices: () => [{ id: "it_IT-1", name: long("Elsa") }],
    voicestudio_status: () => ({ installed: false, running: false }),
    local_gpu: () => ({ gpus: [{ name: long("AMD Radeon RX 6600"), vendor: "amd", vram_mb: 8192, integrated: false }], devices: [], advice: long("Usa Vulkan"), suggested_backend: "vulkan", log: "", settings: cfg.local || {}, user_env: false }),
    audio_devices: () => ({ outputs: ["Altoparlanti (Realtek)", long("Cuffie USB")], inputs: ["Microfono (Realtek)"], default_output: "Altoparlanti (Realtek)", default_input: "Microfono (Realtek)", cable_render: null, cable_capture: null }),
    audio_status: () => ({}),
    pet_active: () => false,
    memory_graph_count: () => (MANY ? 4210 : 37),
    pet_assets: () => ({ name: "chihiro", longName: "Chihiro", author: "", sprites: {} }),
    load_user_sprites: () => ({}),
    log_list: () => [],
    applog_list: () => [],
  };
  async function invoke(cmd, args) {
    const f = R[cmd];
    // sconosciuti: elenco vuoto per liste e registri, oggetto vuoto per il resto
    try { return f ? f(args) : /(_list|_log|_voices)$/.test(cmd) ? [] : {}; } catch (e) { console.warn("mock", cmd, e); return null; }
  }
  const win = { listen: async () => () => {}, onResized: async () => () => {}, onMoved: async () => () => {}, setSize: async () => {}, innerSize: async () => ({ width: innerWidth, height: innerHeight }), scaleFactor: async () => 1, close: async () => {}, hide: async () => {}, show: async () => {}, setFocus: async () => {}, isMaximized: async () => false, startDragging: async () => {} };
  window.__TAURI__ = {
    core: { invoke, Channel: class { constructor() { this.onmessage = null; } } },
    event: { listen: async () => () => {}, emit: async () => {}, once: async () => () => {} },
    window: { getCurrentWindow: () => win },
  };
})();
