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
  // catalogo finto della Libreria LLM (Ollama, community, Hugging Face)
  const LIB = [
    ["llama3.1", "Llama 3.1 is a new state-of-the-art model from Meta available in 8B, 70B and 405B parameter sizes.", ["tools"], ["8b", "70b", "405b"], "108.2M"],
    ["deepseek-r1", "DeepSeek-R1 is a family of open reasoning models with performance approaching that of leading models.", ["tools", "thinking"], ["1.5b", "7b", "8b", "14b", "32b", "70b", "671b"], "68.5M"],
    ["gemma3", "The current, most capable model that runs on a single GPU.", ["vision"], ["270m", "1b", "4b", "12b", "27b"], "24.1M"],
    ["qwen3", "Qwen3 is the latest generation of large language models in Qwen series.", ["tools", "thinking"], ["0.6b", "1.7b", "4b", "8b", "14b", "30b", "32b", "235b"], "12.9M"],
    ["llama3.2", "Meta's Llama 3.2 goes small with 1B and 3B models.", ["tools"], ["1b", "3b"], "41.3M"],
    ["qwen2.5-coder", "The latest series of Code-Specific Qwen models.", ["tools"], ["0.5b", "1.5b", "3b", "7b", "14b", "32b"], "9.1M"],
    ["mistral", "The 7B model released by Mistral AI, updated to version 0.3.", ["tools"], ["7b"], "19.4M"],
    ["gpt-oss", "OpenAI's open-weight models designed for powerful reasoning, agentic tasks, and versatile developer use cases.", ["tools", "thinking"], ["20b", "120b"], "3.2M"],
    ["phi4-mini", "Phi-4-mini brings significant enhancements in multilingual support, reasoning, and mathematics.", ["tools"], ["3.8b"], "820K"],
    ["llava", "LLaVA is a novel end-to-end trained large multimodal model.", ["vision"], ["7b", "13b", "34b"], "8.6M"],
    ["nomic-embed-text", "A high-performing open embedding model with a large token context window.", ["embedding"], [], "35.0M"],
    ["lfm3", "LFM3-1.7B is a tri-hybrid model for local agents.", ["tools", "thinking"], ["1.7b"], "73.2K"],
    ["gemma4", "Gemma 4: tools, vision, audio and reasoning in sizes for every device.", ["tools", "vision", "thinking", "audio"], ["e2b", "e4b", "12b", "26b-a4b"], "1.4M"],
    ["mistral-small3.2", "An update to Mistral Small 3.1 that improves instruction following and function calling.", ["tools", "vision"], ["24b"], "410K"],
    ["granite3.3", "IBM Granite 2B and 8B models are 128K context length language models.", ["tools"], ["2b", "8b"], "390K"],
    ["devstral", "Devstral: the best open source model for coding agents.", ["tools"], ["24b"], "520K"],
    ["smollm2", "SmolLM2 is a family of compact language models available in three size: 135M, 360M, and 1.7B parameters.", ["tools"], ["135m", "360m", "1.7b"], "2.1M"],
    ["qwen2.5vl", "Flagship vision-language model of Qwen.", ["vision"], ["3b", "7b", "32b", "72b"], "1.1M"],
    ["command-r7b", "The smallest model in Cohere's R series delivers top-tier speed.", ["tools"], ["7b"], "120K"],
    ["olmo2", "OLMo 2 is a new family of 7B and 13B models trained on up to 5T tokens.", [], ["7b", "13b"], "300K"],
    ["deepseek-v3", "A strong Mixture-of-Experts (MoE) language model with 671B total parameters.", [], ["671b"], "2.6M"],
    ["mixtral", "A set of Mixture of Experts (MoE) model with open weights by Mistral AI.", ["tools"], ["8x7b", "8x22b"], "1.4M"],
  ];
  const lib = (MANY ? Array.from({ length: 80 }, (_, i) => { const m = LIB[i % LIB.length]; return [i < LIB.length ? m[0] : m[0] + "-variant-" + i, m[1], m[2], m[3], m[4]]; }) : LIB)
    .map(([name, d, caps, sizes, pulls], i) => ({ name: long(name), description: long(d), capabilities: caps, sizes, downloads: pulls, updated: "", source: "library" }));
  const hf = Array.from({ length: MANY ? 60 : 14 }, (_, i) => ({
    name: long("hf.co/" + ["bartowski", "unsloth", "mlabonne", "TheBloke", "LiquidAI"][i % 5] + "/" + ["Qwen3-8B", "gemma-3-12b-it", "Llama-3.2-3B-Instruct-abliterated", "Mistral-Small-3.2-24B-Instruct-2506", "LFM3-VL-2B"][i % 5] + "-GGUF" + (i >= 5 ? "-v" + i : "")),
    description: "text-generation", capabilities: i % 3 === 1 ? ["vision"] : [], sizes: [["8b", "12b", "3b", "24b", "2b"][i % 5]], downloads: String(120000 - i * 7000), updated: "2026-10-0" + (1 + (i % 9)), source: "huggingface",
  }));
  const community = Array.from({ length: MANY ? 40 : 8 }, (_, i) => ({ name: long(["orcarouter/LFM3-1.7B-abliterated", "huihui_ai/qwen3-abliterated", "sam860/deepseek-r1-uncensored-8b", "mannix/llama3.1-8b-heretic"][i % 4] + (i >= 4 ? "-" + i : "")), description: "", capabilities: [], sizes: [], downloads: String(3400 - i * 300), updated: "November 11, 2026", source: "community" }));
  const evl = {};
  const fire = (ev, payload) => (evl[ev] || []).forEach((f) => f({ event: ev, payload }));
  // la Libreria mostra la coda con un download già in corso (avviato dalle Impostazioni)
  if (/models\.html/.test(location.pathname)) {
    let done = 0;
    setInterval(() => { done = Math.min(9.3e9, done + 3.1e8); fire("local-progress", { task: "pull", name: long("qwen3:14b"), status: "pulling a8cc1361f314", completed: 3.9e9 + done * 0.1, total: 9.3e9, done: false }); }, 250);
  }
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
    local_search: ({ source, query }) => (source === "huggingface" ? hf : source === "community" ? community : lib).filter((m) => !query || m.name.toLowerCase().includes(String(query).toLowerCase())),
    local_hf_files: ({ repo }) => ["Q2_K", "Q3_K_M", "Q4_K_M", "Q5_K_M", "Q6_K", "Q8_0"].map((q, i) => [repo.split("/").pop() + "-" + q + ".gguf", Math.round((2.4 + i * 1.3) * 1e9)]).concat([["mmproj-model-f16.gguf", 8.5e8]]).filter(([f]) => !f.startsWith("mmproj")),
    local_show: ({ name }) => ({ family: "qwen3", params: "8.2B", quant: "Q4_K_M", format: "gguf", context: 40960, license: long("Apache License"), capabilities: ["completion", "tools", "thinking"] }),
    local_pull: ({ name }) => new Promise((res) => { let c = 0; const id = setInterval(() => { c += 2e8; fire("local-progress", { task: "pull", name, status: "pulling", completed: c, total: 2e9, done: false }); if (c >= 2e9) { clearInterval(id); fire("local-progress", { task: "pull", name, status: "success", done: true }); res(name); } }, 200); }),
    local_use: () => "In uso.",
    local_delete: () => null,
    local_cancel: () => null,
    local_open_page: () => null,
    local_gpu: () => ({ gpus: [{ name: long("AMD Radeon RX 6600"), vendor: "amd", vram_mb: 8192, integrated: false }], devices: [], advice: long("Usa Vulkan"), suggested_backend: "vulkan", log: "", settings: cfg.local || {}, user_env: false, ram_mb: 32768, cpu_threads: 16 }),
    audio_devices: () => ({ outputs: ["Altoparlanti (Realtek)", long("Cuffie USB")], inputs: ["Microfono (Realtek)"], default_output: "Altoparlanti (Realtek)", default_input: "Microfono (Realtek)", cable_render: null, cable_capture: null }),
    audio_status: () => ({}),
    vrchat_status: () => ({ running: true, connected: true, osc: "127.0.0.1:9001 → 9000", avatar: { id: "avtr_c38a1615-5bf5-42b4-84eb-a8b6c37cbd11", name: long("Alter Ego (Chihiro)"), params: 13 }, mode: "follow", emotion: "happy", inputs: ["Vertical"],
      hearing: { on: true, device: "Altoparlanti (Realtek)", error: null, talking: false, voices: 3, last: long("ciao a tutti") }, vision: { mode: "local", looking: false, error: null, last: { text: long("Two avatars near a mirror, one waving"), secs: 4 } },
      fly: { out: [["escape", 0.1]], stress: 0.2, mood: "" }, account: "", logged_in: false, mic_link: true, error: null }),
    vrchat_avatars: () => Array.from({ length: MANY ? 300 : 3 }, (_, i) => ({ id: "avtr_" + String(i).padStart(8, "0") + "-0000-0000-0000-000000000000", name: long("Avatar " + i), user: "usr_1", mtime: now - i,
      params: Array.from({ length: MANY ? 120 : 8 }, (_, j) => ({ name: long(["VelocityZ", "HeadPat", "Contact_Boop", "Proximity_L", "VRCEmote", "Expression", "Tail_IsGrabbed", "GestureLeft"][j % 8]) + (j >= 8 ? j : ""), ty: "Float", input: j % 2 === 0, output: true })) })),
    vrchat_vision_models: () => ({ installed: [long("qwen2.5vl:3b")], recommended: ["qwen2.5vl:3b", "gemma3:4b", "moondream:1.8b"] }),
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
    event: {
      listen: async (ev, f) => { (evl[ev] = evl[ev] || []).push(f); return () => { evl[ev] = evl[ev].filter((x) => x !== f); }; },
      emit: async (ev, payload) => fire(ev, payload),
      once: async () => () => {},
    },
    window: { getCurrentWindow: () => win },
  };
})();
