// Libreria LLM (finestra separata, si apre dalle Impostazioni → Modelli locali):
// consigliati, installati, ricerca (Ollama e Hugging Face), download da un link, coda dei download.
// Prima di tutto l'hardware: per ogni modello un pallino dice se entra nella VRAM (verde), se va in parte
// su CPU e RAM (giallo) o se è troppo grande (rosso), con una stima della velocità.
// Alter Ego compare ogni tanto per un commento, come nella dashboard larga delle Impostazioni (wide.js).
const { invoke } = window.__TAURI__.core;
const { listen, emit } = window.__TAURI__.event;
const $ = (id) => document.getElementById(id);
const REDUCED = matchMedia("(prefers-reduced-motion: reduce)");

/** Crea un elemento: el("div.cls", { title }, figli…). */
function el(spec, props, ...kids) {
  const [tag, ...cls] = spec.split(".");
  const e = document.createElement(tag || "div");
  if (cls.length) e.className = cls.join(" ");
  for (const [k, v] of Object.entries(props || {})) {
    if (v == null || v === false) continue;
    if (k.startsWith("on")) e[k] = v;
    else if (k === "text") e.textContent = v;
    else if (k === "html") e.innerHTML = v;
    else if (k in e && typeof v !== "string") e[k] = v;
    else e.setAttribute(k, v === true ? "" : v);
  }
  for (const k of kids.flat()) if (k != null && k !== false) e.append(k);
  return e;
}

// ---------- icone pixel 8×8 (una per famiglia, sempre la stessa) ----------
function hash(s) {
  let h = 2166136261;
  for (const c of s) h = Math.imul(h ^ c.charCodeAt(0), 16777619);
  return h >>> 0;
}
const PALETTES = [["#9cff3a", "#4f9a1f"], ["#6fdc8c", "#2f7a46"], ["#c8ff7a", "#6b8f2a"], ["#3fd17a", "#1d6a3a"], ["#e4ffd4", "#6f9a62"], ["#b6ff5a", "#3d7a2c"]];
function pixelIcon(key) {
  let s = hash(key || "?") || 1;
  const rnd = () => ((s = Math.imul(s ^ (s >>> 15), 2246822507) ^ Math.imul(s ^ (s >>> 13), 3266489909)), (s >>>= 0) / 4294967296);
  const [hi, lo] = PALETTES[hash("p" + key) % PALETTES.length];
  let rects = "";
  let on = 0;
  for (let y = 1; y < 7; y++) {
    for (let x = 1; x < 4; x++) {
      const r = rnd();
      if (r < 0.42) continue;
      on++;
      const fill = r > 0.78 ? hi : lo;
      rects += `<rect x="${x}" y="${y}" width="1" height="1" fill="${fill}"/><rect x="${7 - x}" y="${y}" width="1" height="1" fill="${fill}"/>`;
    }
  }
  if (on < 4) rects += `<rect x="3" y="3" width="2" height="2" fill="${hi}"/>`;
  return `<svg viewBox="0 0 8 8" aria-hidden="true"><rect width="8" height="8" fill="#061206"/><rect x=".5" y=".5" width="7" height="7" fill="none" stroke="${lo}" stroke-width=".25"/>${rects}</svg>`;
}
const ico = (key, cls = "ico") => el("span." + cls, { html: pixelIcon(key) });
$("logo").innerHTML = pixelIcon("alter-ego-llm");

// ---------- nomi: famiglia e produttore ----------
/** "gemma3:4b" → "gemma", "hf.co/LiquidAI/LFM3-VL-2B-GGUF" → "lfm", "qwen2.5-coder:7b" → "qwen". */
function familyOf(name) {
  const base = String(name).split(":")[0].split("/").pop().toLowerCase();
  return (base.match(/^[a-z]+/) || [base])[0] || base;
}
const MAKERS = { gemma: "Google", codegemma: "Google", llama: "Meta", codellama: "Meta", qwen: "Alibaba", qwq: "Alibaba", mistral: "Mistral AI", mixtral: "Mistral AI",
  devstral: "Mistral AI", magistral: "Mistral AI", codestral: "Mistral AI", ministral: "Mistral AI", phi: "Microsoft", deepseek: "DeepSeek", lfm: "Liquid AI",
  granite: "IBM", gpt: "OpenAI", command: "Cohere", aya: "Cohere", smollm: "Hugging Face", olmo: "Ai2", nemotron: "NVIDIA", falcon: "TII", starcoder: "BigCode",
  llava: "LLaVA", moondream: "Vikhyat", nomic: "Nomic", minicpm: "OpenBMB", glm: "Zhipu AI", kimi: "Moonshot AI", hermes: "Nous Research", exaone: "LG AI" };
function makerOf(name) {
  const fam = familyOf(name);
  if (MAKERS[fam]) return MAKERS[fam];
  const parts = String(name).replace(/^hf\.co\//, "").split("/");
  return parts.length > 1 ? parts[0] : "";
}

// ---------- hardware e stime ----------
const S = {
  tab: "rec", status: null, cfg: null, hw: null, filters: new Set(), page: 0, sel: null,
  search: { source: "library", query: null, list: null, error: null, loading: false, seq: 0 },
  shows: new Map(), hfFiles: new Map(), varSel: new Map(),
};

const BPP = [[/IQ1|Q1/, 0.25], [/IQ2|Q2/, 0.36], [/IQ3|Q3/, 0.47], [/IQ4|Q4/, 0.6], [/Q5/, 0.72], [/Q6/, 0.83], [/Q8/, 1.07], [/BF16|F16/, 2.0], [/F32/, 4.0]];
const bpp = (q) => (BPP.find(([re]) => re.test(String(q || "").toUpperCase())) || [0, 0.6])[1];

/** Miliardi di parametri da "8b", "1.7B", "135m", "e4b" (Gemma: effettivi), "26b-a4b" (MoE), "8x7b". */
function paramsOf(str) {
  const s = String(str || "").toLowerCase();
  let m;
  if ((m = s.match(/(\d+(?:\.\d+)?)b-a(\d+(?:\.\d+)?)b/))) return { total: +m[1], active: +m[2] };
  if ((m = s.match(/(\d+)x(\d+(?:\.\d+)?)b/))) return { total: m[1] * m[2] * 0.85, active: m[2] * 2 };
  if ((m = s.match(/(?:^|[^a-z0-9.])e(\d+(?:\.\d+)?)b(?![a-z])/))) return { total: m[1] * 2.6, active: +m[1] };
  if ((m = s.match(/(\d+(?:\.\d+)?)b(?![a-z])/))) return { total: +m[1], active: +m[1] };
  if ((m = s.match(/(\d+(?:\.\d+)?)m(?![a-z])/))) return { total: m[1] / 1000, active: m[1] / 1000 };
  return null;
}
const estSize = (p, quant) => (p ? p.total * bpp(quant) + 0.1 : 0);

/** Banda di memoria indicativa: GPU dedicata in base alla VRAM, CPU con la RAM di sistema. */
const gpuBw = (v) => (v <= 4 ? 110 : v <= 8 ? 240 : v <= 12 ? 360 : v <= 16 ? 450 : 700);
const CPU_BW = 28;

/** Dove gira un modello di `size` GB su questo PC: g (VRAM), y (in parte CPU/RAM), r (troppo grande), ? (non si sa). */
function fitOf(size, activeRatio = 1) {
  const hw = S.hw;
  if (!hw || !hw.known || !size) return { lv: "?", need: 0, tps: null };
  const need = size * 1.08 + 0.8; // pesi + cache del contesto (~8K) e buffer
  const vram = hw.vram * 0.92;
  const ram = (hw.ram || 8) * 0.6;
  const lv = need <= vram ? "g" : need <= vram + ram ? "y" : "r";
  const act = Math.max(0.05, size * activeRatio);
  let tps = null;
  if (lv === "g") tps = (gpuBw(hw.vram) * 0.55) / act;
  else if (lv === "y") {
    const f = Math.max(0, Math.min(1, vram / need));
    tps = 1 / (act * (f / (gpuBw(hw.vram) * 0.55) + (1 - f) / CPU_BW));
  }
  return { lv, need, tps };
}
const FIT_LABEL = { g: "Entra nella VRAM", y: "In parte su CPU e RAM", r: "Troppo grande per questo PC", "?": "Hardware non rilevato" };
function fitLabel(f) {
  if (f.lv === "y" && !S.hw?.vram) return t("Gira sulla CPU");
  return t(FIT_LABEL[f.lv]);
}
function speedLabel(tps) {
  if (!tps) return "";
  const word = tps >= 30 ? t("veloce") : tps >= 12 ? t("scorrevole") : tps >= 4 ? t("lento") : t("molto lento");
  return t("~{n} tok/s · {w}", { n: tps >= 10 ? Math.round(tps) : tps.toFixed(1), w: word });
}
const gb = (x) => (x >= 10 ? Math.round(x) : x >= 1 ? x.toFixed(1) : x.toFixed(2)).toString().replace(/\.0+$/, "") + " GB";
const fmtSize = (b) => (b >= 1e9 ? (b / 1e9).toFixed(1) + " GB" : (b / 1e6).toFixed(0) + " MB");
const fmtCtx = (n) => (n >= 1024 ? Math.round(n / 1024) + "K" : String(n));
function popularity(s) {
  const m = String(s || "").replace(/,/g, "").match(/([\d.]+)\s*([kmb])?/i);
  if (!m) return 0;
  return +m[1] * ({ k: 1e3, m: 1e6, b: 1e9 }[(m[2] || "").toLowerCase()] || 1);
}

async function loadHw() {
  let g = null;
  try { g = await invoke("local_gpu"); } catch {}
  const gpus = g?.gpus || [];
  const ded = gpus.filter((x) => !x.integrated).sort((a, b) => (b.vram_mb || 0) - (a.vram_mb || 0))[0];
  let vram = ded?.vram_mb ? ded.vram_mb / 1024 : 0;
  // memoria della scheda non dichiarata: quella che ha visto Ollama all'avvio ("8.0 GiB")
  if (!vram && ded) {
    const d = (g?.devices || []).find((d) => d.kind !== "iGPU" && /GiB/i.test(d.total));
    if (d) vram = parseFloat(d.total) || 0;
  }
  S.hw = { known: !!g, gpu: ded || gpus[0] || null, vram, ram: (g?.ram_mb || 0) / 1024, threads: g?.cpu_threads || 0 };
  const hw = S.hw;
  $("hw-gpu").textContent = hw.gpu ? hw.gpu.name + (hw.gpu.integrated ? " · " + t("integrata") : "") : t("nessuna scheda dedicata");
  $("hw-gpu").title = gpus.map((x) => x.name).join("\n");
  $("hw-vram").textContent = hw.vram ? gb(hw.vram) : "—";
  $("hw-ram").textContent = hw.ram ? gb(hw.ram) + (hw.threads ? " · " + t("{n} thread", { n: hw.threads }) : "") : "—";
  // parametri massimi in Q4 che entrano in VRAM, e con la RAM
  const maxB = (mem) => Math.max(0, (mem - 0.8) / 1.08 / 0.6);
  const inV = maxB(hw.vram * 0.92);
  const withRam = maxB(hw.vram * 0.92 + (hw.ram || 8) * 0.6);
  $("hw-max").textContent = !hw.known ? "—" : inV >= 1
    ? t("fino a ~{v}B in VRAM · ~{r}B con la RAM", { v: Math.floor(inV), r: Math.floor(withRam) })
    : t("fino a ~{r}B sulla CPU", { r: Math.floor(withRam) });
}

// ---------- modelli consigliati (scelti a mano, funzionano anche senza rete) ----------
// [nome per Ollama, parametri, GB su disco, quantizzazione, parametri attivi (MoE)]
const REC = [
  { id: "lfm3", title: "LFM3", maker: "Liquid AI", caps: ["tools", "chat"], ctx: 32768, license: "LFM Open License", best: true, pick: 0,
    desc: "Piccolo e veloce, pensato per agenti locali: usa bene gli strumenti e va anche senza scheda video.",
    v: [["lfm3", "1.7B", 1.1, "Q4_K_M"], ["lfm3:1.7b-qad-q4_0", "1.7B", 1.0, "Q4_0 QAD"], ["lfm3:1.7b-q8_0", "1.7B", 1.8, "Q8_0"]] },
  { id: "lfm3-vl", title: "LFM3-VL", maker: "Liquid AI", caps: ["vision", "chat"], ctx: 32768, license: "LFM Open License",
    desc: "La versione di LFM3 che vede le immagini: legge gli screenshot dello schermo. Sperimentale.",
    v: [["hf.co/LiquidAI/LFM3-VL-2B-GGUF:Q4_K_M", "2B", 1.3, "Q4_K_M"]] },
  { id: "gemma4", title: "Gemma 4", maker: "Google", caps: ["tools", "vision", "thinking", "audio"], ctx: 131072, license: "Gemma", best: true,
    desc: "Strumenti, visione e ragionamento: il più completo dei consigliati, ma vuole più memoria.",
    v: [["gemma4:e2b", "E2B", 5.6, "Q4_K_M", 0.4], ["gemma4:e4b", "E4B", 7.5, "Q4_K_M", 0.5], ["gemma4:12b", "12B", 8.1, "Q4_K_M"], ["gemma4:26b-a4b", "26B-A4B", 16, "Q4_K_M", 0.16]] },
  { id: "gemma3", title: "Gemma 3", maker: "Google", caps: ["vision", "chat"], ctx: 131072, license: "Gemma",
    desc: "Ottimo in chat e visione, ma su Ollama non usa gli strumenti (niente azioni sul PC).",
    v: [["gemma3:1b", "1B", 0.8, "Q4_K_M"], ["gemma3:4b", "4B", 3.3, "Q4_K_M"], ["gemma3:12b", "12B", 8.1, "Q4_K_M"], ["gemma3:27b", "27B", 17, "Q4_K_M"]] },
  { id: "qwen3", title: "Qwen3", maker: "Alibaba", caps: ["tools", "thinking"], ctx: 40960, license: "Apache 2.0",
    desc: "Ragiona prima di rispondere e usa gli strumenti; capisce bene l'italiano.",
    v: [["qwen3:4b", "4B", 2.5, "Q4_K_M"], ["qwen3:8b", "8B", 5.2, "Q4_K_M"], ["qwen3:14b", "14B", 9.3, "Q4_K_M"]] },
  { id: "llama3.2", title: "Llama 3.2", maker: "Meta", caps: ["tools", "chat"], ctx: 131072, license: "Llama 3.2 Community",
    desc: "Leggero e affidabile per la chat di tutti i giorni.",
    v: [["llama3.2:1b", "1B", 1.3, "Q8_0"], ["llama3.2:3b", "3B", 2.0, "Q4_K_M"]] },
  { id: "phi4-mini", title: "Phi-4 mini", maker: "Microsoft", caps: ["tools", "chat"], ctx: 131072, license: "MIT",
    desc: "Piccolo ma bravo in logica e matematica.",
    v: [["phi4-mini:3.8b", "3.8B", 2.5, "Q4_K_M"]] },
  { id: "deepseek-r1", title: "DeepSeek-R1", maker: "DeepSeek", caps: ["thinking"], ctx: 131072, license: "MIT",
    desc: "Specialista del ragionamento passo passo: lento a rispondere, ma pensa.",
    v: [["deepseek-r1:8b", "8B", 5.2, "Q4_K_M"], ["deepseek-r1:14b", "14B", 9.0, "Q4_K_M"]] },
  { id: "qwen2.5-coder", title: "Qwen2.5-Coder", maker: "Alibaba", caps: ["code", "tools"], ctx: 32768, license: "Apache 2.0",
    desc: "Scrive, corregge e spiega codice.",
    v: [["qwen2.5-coder:7b", "7B", 4.7, "Q4_K_M"], ["qwen2.5-coder:14b", "14B", 9.0, "Q4_K_M"]] },
  { id: "gpt-oss", title: "gpt-oss", maker: "OpenAI", caps: ["tools", "thinking"], ctx: 131072, license: "Apache 2.0",
    desc: "Ragionamento e strumenti; è un MoE, quindi più veloce di quanto dica la dimensione.",
    v: [["gpt-oss:20b", "20B", 14, "MXFP4", 0.18]] },
  { id: "mistral-small3.2", title: "Mistral Small 3.2", maker: "Mistral AI", caps: ["tools", "vision"], ctx: 131072, license: "Apache 2.0",
    desc: "Grande e preciso, con visione e strumenti: per PC con molta memoria.",
    v: [["mistral-small3.2:24b", "24B", 15, "Q4_K_M"]] },
];
/** Varianti che il backend sa installare da zero con «Installa e usa» (local_lfm3: installa anche Ollama). */
const ONE_CLICK = new Set(["lfm3", "lfm3:1.7b-qad-q4_0", "lfm3:1.7b-q8_0", "hf.co/LiquidAI/LFM3-VL-2B-GGUF:Q4_K_M", "gemma4:e2b", "gemma4:e4b", "gemma4:12b", "gemma4:26b-a4b", "gemma3:1b", "gemma3:4b", "gemma3:12b", "gemma3:27b"]);

function recEntries() {
  return REC.map((f, i) => ({
    key: "rec:" + f.id, title: f.title, family: familyOf(f.id), maker: f.maker, desc: t(f.desc), caps: f.caps, ctx: f.ctx, license: f.license,
    source: "rec", pop: 1000 - i, best: f.best, pick: f.pick,
    variants: f.v.map(([tag, params, size, quant, ar]) => ({ tag, params, size, quant, ar: ar || 1, exact: false })),
  }));
}

function searchEntry(m) {
  const fam = familyOf(m.name);
  const base = { key: m.source + ":" + m.name, title: m.name, family: fam, maker: makerOf(m.name), desc: m.description, caps: m.capabilities || [],
    source: m.source, pop: popularity(m.downloads), downloads: m.downloads, updated: m.updated, ctx: 0, license: "" };
  const sizes = m.sizes || [];
  if (m.source === "library" && sizes.length) {
    base.variants = sizes.map((s) => {
      const p = paramsOf(s);
      return { tag: m.name + ":" + s, params: s.toUpperCase(), size: estSize(p, "Q4_K_M"), quant: "Q4_K_M", ar: p ? p.active / p.total : 1, exact: false };
    });
  } else {
    const p = paramsOf(sizes[0] || m.name);
    base.variants = [{ tag: m.name, params: p ? (sizes[0] || "").toUpperCase() || p.total + "B" : "", size: estSize(p, "Q4_K_M"), quant: m.source === "huggingface" ? "Q4_K_M" : "", ar: p ? p.active / p.total : 1, exact: false }];
  }
  return base;
}

function installedEntries() {
  return (S.status?.models || []).map((m) => {
    const p = paramsOf(m.params);
    return { key: "inst:" + m.name, title: m.name, family: familyOf(m.name), maker: makerOf(m.name), desc: "", caps: [], source: "installed",
      installedModel: m, ctx: 0, license: "", pop: 0,
      variants: [{ tag: m.name, params: m.params, size: m.size / 1e9, quant: m.quant, ar: p ? p.active / p.total : 1, exact: true }] };
  });
}

// ---------- stato di Ollama e configurazione ----------
const installedNames = () => new Set((S.status?.models || []).map((m) => m.name));
function isInstalled(tag) {
  const set = installedNames();
  return set.has(tag) || set.has(tag + ":latest");
}
function mainModel() {
  const ai = S.cfg?.ai;
  const o = ai?.providers?.ollama;
  return o?.enabled && o.model ? { name: o.model, primary: ai.order?.find((id) => ai.providers[id]?.enabled) === "ollama" } : null;
}
const isMain = (name) => { const m = mainModel(); return !!m && (m.name === name || m.name + ":latest" === name); };

let statusSig = "";
async function refreshStatus(quiet) {
  try { S.status = await invoke("local_status"); } catch { S.status = null; }
  try { S.cfg = await invoke("get_config"); } catch {}
  const sig = JSON.stringify([S.status, S.cfg?.ai?.providers?.ollama, S.cfg?.ai?.order]);
  const same = sig === statusSig;
  statusSig = sig;
  const s = S.status;
  const pill = $("pill");
  pill.className = "pill " + (s?.running ? "ok" : s?.installed ? "off" : "bad");
  $("pill-text").textContent = s?.running ? t("Ollama: online · v{v}", { v: s.version }) : s?.installed ? t("Ollama: spento") : t("Ollama: non installato");
  $("n-inst").textContent = s?.models?.length ? "(" + s.models.length + ")" : "";
  const banner = $("banner");
  banner.hidden = !!s?.running;
  if (!s?.running) {
    $("banner-text").textContent = !s?.installed
      ? t("Ollama non è installato: serve per far girare i modelli sul tuo PC. Alter Ego lo installa per te.")
      : t("Ollama è spento: avvialo per vedere e usare i modelli installati. I download lo avviano da soli.");
    $("banner-go").textContent = !s?.installed ? t("⬇ Installa Ollama") : t("▶ Avvia Ollama");
    $("banner-go").onclick = (e) => busy(e.target, async () => {
      await invoke(s?.installed ? "local_start" : "local_install");
      await refreshStatus();
    });
    if (!quiet) say(!s?.installed ? "Senza Ollama non posso pensare in locale… Lo installiamo?" : "Ollama dorme. Lo sveglio io, se vuoi!", "worried");
  }
  if (same && quiet) return;
  if (!quiet || S.tab === "installed") render();
  else { refreshCardsState(); renderDetail(); }
}

async function busy(btn, fn) {
  if (btn) btn.disabled = true;
  try { await fn(); } catch (e) { toast(String(e), true); }
  if (btn) btn.disabled = false;
}
let toastT = null;
function toast(text, bad) {
  const el2 = $("toast");
  el2.textContent = text;
  el2.title = text;
  el2.className = "toast" + (bad ? " bad" : "");
  el2.hidden = false;
  clearTimeout(toastT);
  toastT = setTimeout(() => (el2.hidden = true), bad ? 9000 : 4500);
  if (bad) say("Uh… qualcosa non è andato.", "worried");
}

// ---------- schede ----------
const TABS = ["rec", "installed", "search", "link"];
function showTab(name, focus) {
  if (!TABS.includes(name)) name = "rec";
  const changed = S.tab !== name;
  S.tab = name;
  S.page = 0;
  try { localStorage.setItem("ml-tab", name); } catch {}
  document.querySelectorAll("#tabs [data-tab]").forEach((b) => {
    const on = b.dataset.tab === name;
    b.setAttribute("aria-selected", on);
    b.tabIndex = on ? 0 : -1;
    if (on && focus) b.focus();
  });
  const grid = name === "rec" || name === "search";
  $("toolbar").hidden = name === "link";
  $("source").hidden = name !== "search";
  $("f-size").hidden = !grid;
  $("pills").hidden = !grid;
  $("q").placeholder = name === "installed" ? t("Filtra i modelli installati…") : name === "search" ? t("Cerca su Ollama o Hugging Face… (Invio)") : t("Cerca un modello… (premi /)");
  if (name === "search" && S.search.query === null) runSearch();
  if (changed && !REDUCED.matches) {
    const v = $("view");
    v.classList.remove("tune");
    void v.offsetWidth;
    v.classList.add("tune");
  }
  S.sel = null;
  render();
  if (changed && name === "search") say("Ci sono un sacco di modelli là fuori… Cerchiamo qualcosa di adatto a te.", "thinking");
  if (changed && name === "link") say("Hai un link? Dammelo pure, ci penso io.", "happy");
}
window.showTab = showTab;
document.querySelectorAll("#tabs [data-tab]").forEach((b) => (b.onclick = () => showTab(b.dataset.tab)));
$("tabs").addEventListener("keydown", (e) => {
  if (e.key !== "ArrowRight" && e.key !== "ArrowLeft") return;
  const i = TABS.indexOf(S.tab) + (e.key === "ArrowRight" ? 1 : -1);
  showTab(TABS[(i + TABS.length) % TABS.length], true);
  e.preventDefault();
});

// ---------- filtri ----------
document.querySelectorAll(".pf").forEach((b) => (b.onclick = () => {
  const f = b.dataset.f;
  if (S.filters.has(f)) S.filters.delete(f); else S.filters.add(f);
  b.setAttribute("aria-pressed", S.filters.has(f));
  S.page = 0;
  if (f === "fit" && S.filters.has(f)) say("Ti mostro solo quelli che il tuo PC regge. Niente drammi.", "happy");
  render();
}));
$("f-size").onchange = $("sort").onchange = () => { S.page = 0; render(); };
let qTimer = null;
$("q").addEventListener("input", () => {
  clearTimeout(qTimer);
  qTimer = setTimeout(() => { S.page = 0; if (S.tab === "search") runSearch(); else render(); }, S.tab === "search" ? 450 : 120);
});
$("q").addEventListener("keydown", (e) => {
  if (e.key === "Enter" && S.tab === "search") { clearTimeout(qTimer); runSearch(); }
  if (e.key === "ArrowDown") { document.querySelector("#view .card, #view .row")?.focus(); e.preventDefault(); }
});
$("source").onchange = () => { S.page = 0; runSearch(); };
$("refresh").onclick = async (e) => busy(e.target, async () => {
  if (S.tab === "search") await runSearch();
  await Promise.all([refreshStatus(true), loadHw()]);
  render();
});
$("pill").onclick = () => refreshStatus();

const CODE_RE = /cod(e|er|ing|stral)|devstral|starcoder|program/i;
const ABLIT_RE = /ablit|uncensor|heretic/i;
function variantOf(e) {
  const vs = e.variants;
  if (S.varSel.has(e.key)) return vs[Math.min(S.varSel.get(e.key), vs.length - 1)];
  if (e.pick != null && vs[e.pick] && fitOf(vs[e.pick].size, vs[e.pick].ar).lv !== "r") return vs[e.pick];
  // la più grande che entra nella VRAM, altrimenti la più piccola
  let best = null;
  for (const v of vs) if (fitOf(v.size, v.ar).lv === "g" && (!best || v.size > best.size)) best = v;
  return best || vs.reduce((a, b) => (b.size && (!a.size || b.size < a.size) ? b : a), vs[0]);
}
function passes(e) {
  const v = variantOf(e);
  const hay = `${e.title} ${e.maker} ${e.desc} ${e.variants.map((x) => x.tag).join(" ")}`.toLowerCase();
  const q = S.tab === "search" ? "" : $("q").value.trim().toLowerCase();
  if (q && !q.split(/\s+/).every((w) => hay.includes(w))) return false;
  for (const f of S.filters) {
    if (f === "fit") { if (!e.variants.some((x) => ["g", "y"].includes(fitOf(x.size, x.ar).lv))) return false; }
    else if (f === "code") { if (!e.caps.includes("code") && !CODE_RE.test(hay)) return false; }
    else if (f === "ablit") { if (!ABLIT_RE.test(hay)) return false; }
    else if (f === "installed") { if (!e.variants.some((x) => isInstalled(x.tag))) return false; }
    else if (!e.caps.includes(f)) return false;
  }
  const max = +$("f-size").value;
  if (max) {
    const b = paramsOf(v.params || v.tag)?.total;
    if (b == null) return max !== 999;
    if (max === 999 ? b <= 35 : b > max) return false;
  }
  return true;
}
function sorted(list) {
  const by = $("sort").value;
  const rank = { g: 0, y: 1, r: 2, "?": 3 };
  const f = (e) => { const v = variantOf(e); return fitOf(v.size, v.ar); };
  const cmp = {
    pop: (a, b) => b.pop - a.pop,
    fit: (a, b) => rank[f(a).lv] - rank[f(b).lv] || (f(b).tps || 0) - (f(a).tps || 0),
    size: (a, b) => (variantOf(a).size || 1e9) - (variantOf(b).size || 1e9),
    name: (a, b) => a.title.localeCompare(b.title),
  }[by] || (() => 0);
  return list.slice().sort(cmp);
}

// ---------- ricerca online ----------
async function runSearch() {
  const seq = ++S.search.seq;
  S.search.query = $("q").value.trim();
  S.search.source = $("source").value;
  S.search.loading = true;
  S.search.error = null;
  if (S.tab === "search") render();
  try {
    const list = await invoke("local_search", { source: S.search.source, query: S.search.query, chips: [], caps: [], maxB: 0 });
    if (seq !== S.search.seq) return;
    S.search.list = (list || []).map(searchEntry);
  } catch (e) {
    if (seq !== S.search.seq) return;
    S.search.list = null;
    S.search.error = String(e);
    say("Il catalogo online non risponde… Forse manca la connessione?", "worried");
  }
  S.search.loading = false;
  if (S.tab === "search") render();
}

// ---------- disegno ----------
function pageSize() {
  const w = $("main").clientWidth - 36;
  const cols = Math.max(1, Math.floor((w + 12) / (232 + 12)));
  const rows = $("main").clientHeight > 700 ? 4 : 3;
  return Math.max(6, cols * rows);
}
let current = []; // voci della pagina visibile (per la tastiera)

function render() {
  const view = $("view");
  view.innerHTML = "";
  $("pager").innerHTML = "";
  if (S.tab === "link") { renderLink(view); renderDetail(); return; }
  if (S.tab === "installed") { renderInstalled(view); return; }
  let list;
  if (S.tab === "rec") list = recEntries();
  else {
    if (S.search.loading) { view.append(el("div.loading", { text: t("Cerco nel catalogo…") })); renderDetail(); return; }
    if (S.search.error) {
      view.append(el("div.empty", {}, el("b", { text: t("Catalogo non raggiungibile") }), el("span", { text: t("Controlla la connessione a Internet, poi riprova. I modelli consigliati funzionano anche offline.") }),
        el("br"), el("button", { text: t("Riprova"), onclick: runSearch })));
      renderDetail();
      return;
    }
    list = S.search.list || [];
  }
  const shown = sorted(list.filter(passes));
  if (!shown.length) {
    view.append(el("div.empty", {}, el("b", { text: t("Nessun modello trovato") }), el("span", { text: S.filters.size || +$("f-size").value ? t("Prova a togliere qualche filtro.") : t("Prova con un altro nome.") })));
    S.sel = null;
    renderDetail();
    return;
  }
  const per = pageSize();
  const pages = Math.ceil(shown.length / per);
  S.page = Math.min(S.page, pages - 1);
  current = shown.slice(S.page * per, (S.page + 1) * per);
  if (!S.sel || !shown.some((e) => e.key === S.sel.key)) S.sel = current[0];
  const grid = el("div.grid", { role: "list" });
  current.forEach((e, i) => grid.append(card(e, i)));
  view.append(grid);
  renderPager(pages);
  renderDetail();
}

function card(e, i) {
  const v = variantOf(e);
  const f = fitOf(v.size, v.ar);
  const inst = isInstalled(v.tag);
  const name = e.title;
  const c = el("article.card" + (S.sel?.key === e.key ? ".sel" : ""), { tabindex: 0, role: "listitem", "aria-label": name, "data-key": e.key, "data-no-i18n": true });
  c.style.setProperty("--i", i);
  c.append(
    el("div.c-top", {}, ico(e.family), el("div.c-id", {}, el("span.c-name", { text: name, title: name }),
      el("span.c-by", { text: [e.maker, v.params].filter(Boolean).join(" · "), title: [e.maker, v.params].filter(Boolean).join(" · ") })),
      el("i.fit." + (f.lv === "?" ? "u" : f.lv), { title: fitLabel(f) })),
    el("p.c-desc", { text: e.desc || t("Nessuna descrizione."), title: e.desc || "" }),
    tagsOf(e, inst),
    el("div.c-foot", {},
      el("span.c-size", { text: v.size ? (v.exact ? "" : "≈ ") + gb(v.size) : "—", title: v.exact ? t("Dimensione su disco") : t("Dimensione stimata su disco") }),
      el("span.c-speed", { text: speedLabel(f.tps) || fitLabel(f), title: t("Stima indicativa per questo PC") }),
      inst ? el("button.small", { text: t("Usa"), title: t("Usa come modello principale"), onclick: (ev) => { ev.stopPropagation(); useModel(installedName(v.tag)); } })
        : el("button.small.main", { text: t("⬇ Scarica"), onclick: (ev) => { ev.stopPropagation(); download(e, v); } })));
  c.onclick = () => select(e);
  c.onkeydown = (ev) => gridKeys(ev, e);
  return c;
}
function installedName(tag) { const set = installedNames(); return set.has(tag) ? tag : tag + ":latest"; }

const CAP_LABEL = { tools: "strumenti", vision: "visione", thinking: "ragionamento", code: "codice", chat: "chat", audio: "audio", embedding: "embedding", completion: "testo", insert: "completamento" };
function tagsOf(e, inst) {
  const box = el("div.tags");
  if (e.best) box.append(el("span.tag.rec", { text: t("consigliato") }));
  if (inst) box.append(el("span.tag.inst", { text: t("installato") }));
  const caps = e.caps.filter((c) => c !== "completion");
  for (const c of caps.slice(0, 4)) box.append(el("span.tag", { text: t(CAP_LABEL[c] || c) }));
  if (e.variants.length > 1) box.append(el("span.tag", { text: t("{n} versioni", { n: e.variants.length }) }));
  if (e.downloads) box.append(el("span.tag", { text: "⬇ " + e.downloads }));
  return box;
}

function renderPager(pages) {
  const p = $("pager");
  p.hidden = pages < 2;
  if (pages < 2) return;
  const go = (n) => { S.page = n; render(); $("main").scrollTop = 0; document.querySelector("#view .card")?.focus(); };
  p.append(el("button", { text: "‹", "aria-label": t("Pagina precedente"), disabled: S.page === 0, onclick: () => go(S.page - 1) }));
  const nums = new Set([0, pages - 1, S.page - 1, S.page, S.page + 1].filter((n) => n >= 0 && n < pages));
  let last = -1;
  for (const n of [...nums].sort((a, b) => a - b)) {
    if (n - last > 1) p.append(el("span.gap", { text: "…" }));
    p.append(el("button", { text: String(n + 1), "aria-current": n === S.page ? "page" : null, onclick: () => go(n) }));
    last = n;
  }
  p.append(el("button", { text: "›", "aria-label": t("Pagina successiva"), disabled: S.page === pages - 1, onclick: () => go(S.page + 1) }));
}

function gridKeys(ev, e) {
  const items = [...document.querySelectorAll("#view .card, #view .row")];
  const i = items.indexOf(ev.currentTarget);
  const cols = ev.currentTarget.classList.contains("row") ? 1 : Math.max(1, getComputedStyle(ev.currentTarget.parentElement).gridTemplateColumns.split(" ").length);
  const move = { ArrowRight: 1, ArrowLeft: -1, ArrowDown: cols, ArrowUp: -cols }[ev.key];
  if (move) {
    const n = i + move;
    if (n >= 0 && n < items.length) items[n].focus();
    else if (ev.key === "ArrowUp" && n < 0) $("q").focus();
    ev.preventDefault();
  } else if (ev.key === "Enter" || ev.key === " ") {
    if (ev.target !== ev.currentTarget) return;
    select(e, true);
    ev.preventDefault();
  } else if (ev.key === "PageDown" || ev.key === "PageUp") {
    const b = document.querySelector(`#pager button[aria-label="${t(ev.key === "PageDown" ? "Pagina successiva" : "Pagina precedente")}"]`);
    if (b && !b.disabled) b.click();
    ev.preventDefault();
  }
}

function select(e, focusDetail) {
  S.sel = e;
  document.querySelectorAll("#view .sel").forEach((x) => x.classList.remove("sel"));
  document.querySelector(`#view [data-key="${CSS.escape(e.key)}"]`)?.classList.add("sel");
  renderDetail(true);
  if (focusDetail || $("detail").classList.contains("open")) $("detail").querySelector("button.main, button")?.focus();
  const v = variantOf(e);
  const f = fitOf(v.size, v.ar);
  if (f.lv === "r") say("Questo è un po' troppo grande per il tuo PC… potrei metterci un'eternità a rispondere.", "embarrassed");
}

// ---------- dettagli ----------
function renderDetail(open) {
  const d = $("detail");
  d.innerHTML = "";
  const e = S.sel;
  if (open) d.classList.add("open");
  if (!e) {
    d.classList.remove("open");
    d.append(el("div.d-empty", {}, ico("alter-ego-llm"), el("p", { text: S.tab === "link" ? t("Incolla un link a sinistra: il modello finisce nella coda dei download.") : t("Scegli un modello per vedere i dettagli.") })));
    return;
  }
  const v = variantOf(e);
  const f = fitOf(v.size, v.ar);
  const inst = isInstalled(v.tag);
  const instName = inst ? installedName(v.tag) : null;
  const show = instName && S.shows.get(instName);
  if (instName && !S.shows.has(instName) && S.status?.running) {
    S.shows.set(instName, null);
    invoke("local_show", { name: instName }).then((x) => { S.shows.set(instName, x); if (S.sel?.key === e.key) renderDetail(); }).catch(() => {});
  }
  if (e.source === "huggingface" && !S.hfFiles.has(e.key)) loadHfFiles(e);
  const by = [e.maker, show?.params || v.params].filter(Boolean).join(" · ");
  d.append(el("div.d-head", { "data-no-i18n": true }, ico(e.family),
    el("div", {}, el("h2", { text: e.title, title: e.title }), el("p", { text: by, title: by })),
    el("button.d-close.icon", { text: "×", "aria-label": t("Chiudi"), title: t("Chiudi"), onclick: closeDetail })));
  if (e.desc) d.append(el("p.d-desc", { text: e.desc }));
  const why = [];
  if (f.lv !== "?") why.push(t("serve ~{n} di memoria", { n: gb(f.need) }) + (S.hw?.vram ? t(" su {v} di VRAM", { v: gb(S.hw.vram) }) : ""));
  if (f.tps) why.push(speedLabel(f.tps) + " " + t("(stima)"));
  d.append(el("div.d-fit." + f.lv, {}, el("i.fit." + (f.lv === "?" ? "u" : f.lv)), el("div", {}, el("b", { text: fitLabel(f) }), el("small", { text: why.join(" · ") }))));
  if (e.variants.length > 1) {
    const sel = el("select", { "aria-label": t("Versione") });
    e.variants.forEach((x, i) => {
      const fx = fitOf(x.size, x.ar);
      const where = { g: t("in VRAM"), y: t("in parte su CPU"), r: t("troppo grande"), "?": "" }[fx.lv];
      sel.append(el("option", { value: i, text: [x.tag.split("/").pop(), x.size ? (x.exact ? "" : "≈ ") + gb(x.size) : "", where, isInstalled(x.tag) ? "✓" : ""].filter(Boolean).join(" · "), selected: x === v }));
    });
    sel.onchange = () => { S.varSel.set(e.key, +sel.value); refreshCardsState(); renderDetail(); };
    d.append(el("label.d-var", {}, el("span", { text: t("Versione") }), sel));
  }
  const info = el("dl.d-info", { "data-no-i18n": true });
  const add = (k, val) => { if (val) info.append(el("dt", { text: t(k) }), el("dd", { text: val, title: val })); };
  add("Nome", v.tag);
  add("Dimensione", v.size ? (v.exact ? "" : "≈ ") + gb(v.size) : "—");
  add("Contesto", show?.context ? t("{n} token", { n: fmtCtx(show.context) }) : e.ctx ? t("{n} token", { n: fmtCtx(e.ctx) }) : "—");
  add("Formato", [(show?.format || "gguf").toUpperCase(), show?.quant || v.quant].filter(Boolean).join(" · "));
  add("Licenza", show?.license || e.license || "—");
  add("Fonte", t({ rec: "Libreria di Ollama", library: "Libreria di Ollama", community: "Community di Ollama", huggingface: "Hugging Face", installed: "Installato su questo PC" }[e.source] || e.source));
  if (e.updated) add("Aggiornato", e.updated);
  if (e.installedModel) {
    const g = e.installedModel.gpu_percent;
    add("In memoria", g == null ? t("no") : g >= 99 ? t("sì · GPU 100%") : g <= 0 ? t("sì · solo CPU") : t("sì · GPU {p}% · CPU {c}%", { p: g, c: 100 - g }));
  }
  d.append(el("h3", { text: t("Informazioni") }), info);
  const caps = [...new Set([...(e.caps || []), ...(show?.capabilities || [])])].filter((c) => c !== "completion");
  if (caps.length) {
    const tg = el("div.tags");
    caps.forEach((c) => tg.append(el("span.tag", { text: t(CAP_LABEL[c] || c) })));
    d.append(el("h3", { text: t("Capacità") }), tg);
  }
  if (e.source === "huggingface") {
    const files = S.hfFiles.get(e.key);
    if (files?.length) {
      const sel = el("select", { "aria-label": t("Quantizzazione") });
      e.variants.forEach((x, i) => sel.append(el("option", { value: i, text: `${x.quant || t("predefinita (Q4_K_M)")} · ${x.size ? (x.exact ? "" : "≈ ") + gb(x.size) : "?"}`, selected: x === v })));
      sel.onchange = () => { S.varSel.set(e.key, +sel.value); refreshCardsState(); renderDetail(); };
      d.append(el("label.d-var", {}, el("span", { text: t("Quantizzazione (file GGUF del repository)") }), sel));
    }
  }
  const act = el("div.d-act");
  if (inst) {
    const main = isMain(instName);
    act.append(el("button.main", { text: main ? t("★ Già il modello principale") : t("★ Usa come modello principale"), disabled: main, onclick: () => useModel(instName) }));
    act.append(el("button", { text: t("⟳ Aggiorna (scarica di nuovo)"), onclick: () => enqueue(instName, e.title) }));
    act.append(el("button.danger", { text: t("Elimina dal PC"), onclick: () => removeModel(instName) }));
  } else {
    act.append(el("button.main", { text: f.lv === "r" ? t("⬇ Scarica comunque") : t("⬇ Scarica il modello"), onclick: () => download(e, v) }));
    if (ONE_CLICK.has(v.tag)) act.append(el("button", { text: t("⬇ Scarica e usa subito"), onclick: () => download(e, v, true) }));
  }
  if (e.source !== "installed" || e.maker) act.append(el("button", { text: t("Vedi dettagli ↗"), onclick: () => invoke("local_open_page", { name: v.tag }).catch((x) => toast(String(x), true)) }));
  d.append(act);
}
function closeDetail() {
  const d = $("detail");
  d.classList.remove("open");
  document.querySelector(`#view [data-key="${CSS.escape(S.sel?.key || "")}"]`)?.focus();
}

async function loadHfFiles(e) {
  S.hfFiles.set(e.key, null);
  try {
    const files = await invoke("local_hf_files", { repo: e.title });
    const seen = new Set();
    const vs = [];
    for (const [path, size] of files || []) {
      const q = (path.toUpperCase().match(/(I?Q\d[A-Z0-9_]*|BF16|F16|F32)(?=[.-]|$)/g) || []).pop();
      if (!q || seen.has(q)) continue;
      seen.add(q);
      vs.push({ tag: e.title + ":" + q, params: e.variants[0].params, size: size / 1e9, quant: q, ar: e.variants[0].ar, exact: true });
    }
    if (vs.length) {
      vs.sort((a, b) => a.size - b.size);
      const def = vs.findIndex((x) => x.quant === "Q4_K_M");
      e.variants = vs;
      if (!S.varSel.has(e.key) && def >= 0) S.varSel.set(e.key, def);
    }
    S.hfFiles.set(e.key, vs);
  } catch { S.hfFiles.set(e.key, []); }
  if (S.sel?.key === e.key) { refreshCardsState(); renderDetail(); }
}

/** Aggiorna solo lo stato delle carte (installato, versione scelta) senza ridisegnare la pagina. */
function refreshCardsState() {
  if (S.tab !== "rec" && S.tab !== "search") return;
  for (const c of document.querySelectorAll("#view .card")) {
    const e = current.find((x) => x.key === c.dataset.key);
    if (!e) continue;
    const n = card(e, 0);
    n.style.animation = "none";
    const had = document.activeElement === c;
    c.replaceWith(n);
    if (had) n.focus();
  }
}

// ---------- installati ----------
function renderInstalled(view) {
  const s = S.status;
  if (!s?.running) {
    view.append(el("div.empty", {}, el("b", { text: s?.installed ? t("Ollama è spento") : t("Ollama non è installato") }),
      el("span", { text: s?.installed ? t("Avvialo per vedere i modelli installati.") : t("Installalo per scaricare e usare i modelli sul tuo PC.") })));
    S.sel = null;
    renderDetail();
    return;
  }
  const q = $("q").value.trim().toLowerCase();
  let list = installedEntries().filter((e) => !q || e.title.toLowerCase().includes(q));
  const by = $("sort").value;
  list.sort(by === "size" ? (a, b) => b.variants[0].size - a.variants[0].size : by === "fit" ? (a, b) => (fitOf(b.variants[0].size).tps || 0) - (fitOf(a.variants[0].size).tps || 0)
    : by === "pop" ? (a, b) => (isMain(b.title) - isMain(a.title)) || ((b.installedModel.gpu_percent ?? -1) - (a.installedModel.gpu_percent ?? -1)) || a.title.localeCompare(b.title)
    : (a, b) => a.title.localeCompare(b.title));
  if (!list.length) {
    view.append(el("div.empty", {}, el("b", { text: q ? t("Nessun modello trovato") : t("Nessun modello installato") }),
      el("span", { text: q ? t("Prova con un altro nome.") : t("Scegline uno tra i consigliati: il primo download si fa in un clic.") }),
      q ? null : el("br"), q ? null : el("button.main", { text: t("Vai ai consigliati"), onclick: () => showTab("rec") })));
    S.sel = null;
    renderDetail();
    return;
  }
  const total = list.reduce((a, e) => a + e.installedModel.size, 0);
  view.append(el("p.note", { text: t("{n} modelli · {size} su disco", { n: list.length, size: fmtSize(total) }) }));
  current = list;
  if (!S.sel || !list.some((e) => e.key === S.sel.key)) S.sel = list[0];
  const rows = el("div.rows", { role: "list" });
  for (const e of list) {
    const m = e.installedModel;
    const v = e.variants[0];
    const f = fitOf(v.size, v.ar);
    const g = m.gpu_percent;
    const mem = g == null ? null : el("span", { title: t("Caricato in memoria") }, el("span.mem", {}, el("i", { style: `width:${Math.max(4, g)}%` })),
      document.createTextNode(g >= 99 ? t("GPU 100%") : g <= 0 ? t("solo CPU") : t("GPU {p}% · CPU {c}%", { p: g, c: 100 - g })));
    const r = el("div.row" + (S.sel?.key === e.key ? ".sel" : ""), { tabindex: 0, role: "listitem", "aria-label": m.name, "data-key": e.key, "data-no-i18n": true },
      ico(e.family),
      el("div.r-name", {}, el("b", { text: m.name, title: m.name }), isMain(m.name) ? el("span.main-badge", { text: t("★ principale") }) : null),
      el("div.r-meta", {},
        el("span", { text: [m.params, m.quant].filter(Boolean).join(" · ") || "—" }),
        el("span", { text: fmtSize(m.size) }),
        el("span", { title: fitLabel(f) }, el("i.fit." + (f.lv === "?" ? "u" : f.lv)), document.createTextNode(speedLabel(f.tps) || fitLabel(f))),
        mem),
      el("div.r-act", {},
        el("button.small" + (isMain(m.name) ? "" : ".main"), { text: isMain(m.name) ? t("In uso") : t("Usa"), disabled: isMain(m.name), title: t("Usa come modello principale"), onclick: (ev) => { ev.stopPropagation(); useModel(m.name); } }),
        el("button.small", { text: "⟳", title: t("Aggiorna (scarica di nuovo)"), "aria-label": t("Aggiorna (scarica di nuovo)"), onclick: (ev) => { ev.stopPropagation(); enqueue(m.name, m.name); } }),
        el("button.small.danger", { text: t("Elimina"), onclick: (ev) => { ev.stopPropagation(); removeModel(m.name); } })));
    r.onclick = () => select(e);
    r.onkeydown = (ev) => gridKeys(ev, e);
    rows.append(r);
  }
  view.append(rows);
  renderDetail();
}

async function useModel(name) {
  try {
    const msg = await invoke("local_use", { name, primary: true });
    toast(msg);
    emit("local-used");
    say(t("D'ora in poi penso con {name}. Che emozione…", { name }), "happy", true);
  } catch (e) { toast(String(e), true); }
  await refreshStatus(true);
  render();
}

function confirmBox(text, yes) {
  const dlg = $("dlg");
  $("dlg-text").textContent = text;
  $("dlg-yes").textContent = yes;
  return new Promise((res) => {
    const done = (v) => { dlg.close(); res(v); };
    $("dlg-yes").onclick = () => done(true);
    $("dlg-no").onclick = () => done(false);
    dlg.oncancel = () => res(false);
    if (dlg.showModal) { dlg.showModal(); $("dlg-no").focus(); } else res(confirm(text));
  });
}
async function removeModel(name) {
  if (!(await confirmBox(t("Eliminare {name} dal PC? Lo spazio su disco si libera subito; per riaverlo va scaricato di nuovo.", { name }), t("Elimina")))) return;
  try {
    await invoke("local_delete", { name });
    say(t("Addio, {name}… era solo un modello, vero?", { name }), "sad", true);
    emit("local-used");
  } catch (e) { toast(String(e), true); }
  S.shows.delete(name);
  await refreshStatus(true);
  render();
}

// ---------- da link ----------
const EXAMPLES = ["hf.co/LiquidAI/LFM3-VL-2B-GGUF:Q4_K_M", "https://huggingface.co/bartowski/Llama-3.2-3B-Instruct-GGUF", "https://ollama.com/library/qwen3:8b", "https://example.com/modelli/mio-modello.Q4_K_M.gguf"];
function renderLink(view) {
  const input = el("input", { id: "url", placeholder: "hf.co/utente/modello-GGUF:Q4_K_M · link di Hugging Face · ollama.com/… · https://…/file.gguf", autocomplete: "off", spellcheck: "false", "aria-label": t("Indirizzo del modello") });
  const go = () => { const u = input.value.trim(); if (u) { enqueue(u, u); input.value = ""; } else input.focus(); };
  input.onkeydown = (e) => { if (e.key === "Enter") go(); };
  const ex = el("ul.examples");
  for (const x of EXAMPLES) ex.append(el("li", {}, el("button", { text: x, title: x, "data-no-i18n": true, onclick: () => { input.value = x; input.focus(); } })));
  view.append(el("div.linkbox", {},
    el("h2", { text: t("Scarica da un indirizzo") }),
    el("p.note", { text: t("Nomi di Ollama, repository GGUF di Hugging Face (anche il link a un singolo file .gguf), pagine di ollama.com o un link diretto a un file .gguf.") }),
    el("div.link-row", {}, input, el("button.main", { text: t("⬇ Aggiungi alla coda"), onclick: go })),
    el("p.note", { text: t("Esempi (clic per copiarli nel campo):") }), ex,
    el("p.note", { text: t("Se annulli un download, o si interrompe, riprende da dove era rimasto. Abliterated e uncensored sono versioni modificate da terzi senza i rifiuti del modello originale: usale con giudizio.") })));
  S.sel = null;
}

// ---------- coda dei download ----------
// Un download alla volta, nell'ordine. Ollama tiene i pezzi già scaricati: «Riprendi» riparte da lì.
// La coda resta salvata nella finestra: chiudendola e riaprendola i download non finiti tornano «in pausa».
const Q = [];
function saveQueue() {
  try { localStorage.setItem("ml-queue", JSON.stringify(Q.filter((x) => x.state !== "done").map(({ name, label, use }) => ({ name, label, use })))); } catch {}
}
function loadQueue() {
  try { for (const x of JSON.parse(localStorage.getItem("ml-queue") || "[]")) Q.push({ ...x, state: "paused", completed: 0, total: 0, status: "" }); } catch {}
}
function download(e, v, use) {
  if (use && ONE_CLICK.has(v.tag)) return enqueue(v.tag, e.title + (e.variants.length > 1 ? " · " + v.params : ""), true);
  enqueue(v.tag, e.title + (e.variants.length > 1 || e.source === "huggingface" ? " · " + (e.source === "huggingface" ? v.quant || "Q4_K_M" : v.params) : ""));
}
function enqueue(name, label, use) {
  const ex = Q.find((x) => x.name === name);
  if (ex && ["queued", "running"].includes(ex.state)) { openQueue(); return; }
  if (ex) Q.splice(Q.indexOf(ex), 1);
  Q.push({ name, label: label || name, use: !!use, state: "queued", completed: 0, total: 0, status: "" });
  saveQueue();
  openQueue(false, $("app").clientHeight >= 640);
  renderQueue();
  pump();
}
let running = null;
async function pump() {
  if (running || Q.some((x) => x.state === "running" && x.external)) return;
  const it = Q.find((x) => x.state === "queued");
  if (!it) return;
  running = it;
  it.state = "running";
  it.status = t("avvio…");
  renderQueue();
  try {
    if (it.use) {
      // installa anche Ollama se manca, scarica e imposta come principale
      toast(await invoke("local_lfm3", { variant: it.name, primary: true }));
      emit("local-used");
    } else {
      if (S.status && !S.status.installed) await invoke("local_install");
      it.pulled = await invoke("local_pull", { name: it.name });
    }
    it.state = "done";
    it.status = t("pronto ✓");
    say(t("Fatto! {name} è pronto. Lo proviamo?", { name: it.label }), "happy", true);
  } catch (e) {
    it.state = it.cancel ? "paused" : "error";
    it.status = it.cancel ? t("in pausa: «Riprendi» continua da dove era rimasto") : String(e);
    if (!it.cancel) say("Il download non è andato… Riproviamo tra poco?", "sad", true);
  }
  it.cancel = false;
  running = null;
  saveQueue();
  renderQueue();
  await refreshStatus(true);
  pump();
}
function cancelItem(it) {
  if (it.state === "running") { it.cancel = true; invoke("local_cancel", { name: it.pullName || it.name }); it.status = t("annullo…"); renderQueue(); }
  else { Q.splice(Q.indexOf(it), 1); saveQueue(); renderQueue(); }
}
function resumeItem(it) { it.state = "queued"; it.status = ""; saveQueue(); renderQueue(); pump(); }

listen("local-progress", ({ payload: p }) => {
  if (p.task !== "pull" || !p.name) return;
  // il nome arriva normalizzato dal backend (es. un link diventa hf.co/…): si lega al download in corso
  let it = Q.find((x) => x.name === p.name || x.pullName === p.name || x.pulled === p.name);
  if (!it && running && !running.pullName) { it = running; it.pullName = p.name; }
  if (!it) {
    if (p.done) return;
    // download avviato altrove (Impostazioni, Alter Ego in chat)
    it = { name: p.name, label: p.name, state: "running", external: true, completed: 0, total: 0, status: "" };
    Q.push(it);
    openQueue();
  }
  if (p.done) {
    if (it.external) { it.state = p.error ? "error" : "done"; it.status = p.error || t("pronto ✓"); refreshStatus(true).then(pump); }
  } else {
    if (it.state === "paused") { it.state = "running"; it.external = true; }
    it.completed = p.completed;
    it.total = p.total;
    it.status = p.status;
  }
  renderQueue();
});

let queueSig = "";
function renderQueue() {
  const list = $("q-list");
  // con lo stesso elenco si aggiornano solo testi e barre: i pulsanti restano cliccabili durante l'avanzamento
  const sig = Q.map((x) => x.name + "|" + x.state).join("\n");
  const rebuild = sig !== queueSig || list.children.length !== Q.length;
  queueSig = sig;
  if (rebuild) list.innerHTML = "";
  Q.forEach((it, i) => {
    const pct = it.state === "done" ? 100 : it.total ? Math.round((it.completed / it.total) * 100) : 0;
    const st = it.state === "running"
      ? `${it.status || t("scarico…")}${it.total ? ` · ${fmtSize(it.completed)} / ${fmtSize(it.total)} (${pct}%)` : ""}`
      : it.state === "queued" ? t("in coda") : it.state === "paused" ? (it.status || t("in pausa")) : it.status;
    if (!rebuild) {
      const li = list.children[i];
      li.querySelector(".q-bar").setAttribute("aria-valuenow", pct);
      li.querySelector(".q-bar > i").style.width = (it.state === "queued" ? 0 : pct) + "%";
      const s2 = li.querySelector(".q-st");
      if (s2.textContent !== st) { s2.textContent = st; s2.title = st; }
      return;
    }
    const btns = el("div.q-btns");
    if (it.state === "running" || it.state === "queued") btns.append(el("button.small", { text: t("Annulla"), onclick: () => cancelItem(it) }));
    if (it.state === "paused" || it.state === "error") btns.append(el("button.small.main", { text: t("Riprendi"), onclick: () => resumeItem(it) }), el("button.small", { text: "×", title: t("Togli dalla coda"), "aria-label": t("Togli dalla coda"), onclick: () => cancelItem(it) }));
    if (it.state === "done") btns.append(el("button.small", { text: "×", title: t("Togli dalla coda"), "aria-label": t("Togli dalla coda"), onclick: () => cancelItem(it) }));
    list.append(el("li.q-item." + it.state, { "data-no-i18n": true },
      el("span.q-name", { text: it.label, title: it.name }), btns,
      el("div.q-bar", { role: "progressbar", "aria-valuenow": pct, "aria-valuemin": 0, "aria-valuemax": 100, "aria-label": it.label }, el("i", { style: `width:${it.state === "queued" ? 0 : pct}%` })),
      el("span.q-st", { text: st, title: st })));
  });
  const act = Q.filter((x) => x.state === "running").length;
  const wait = Q.filter((x) => x.state === "queued").length;
  $("n-queue").textContent = act + wait ? "(" + (act + wait) + ")" : "";
  $("q-toggle").classList.toggle("busy", !!act);
  // riga riassuntiva (si vede anche con l'elenco chiuso)
  const run = Q.find((x) => x.state === "running");
  const pct = run?.total ? Math.round((run.completed / run.total) * 100) : 0;
  const sum = run ? `${run.label}${run.total ? " · " + pct + "%" : ""}${wait ? " · " + t("{w} in coda", { w: wait }) : ""}`
    : Q.length ? t("{a} in corso · {w} in coda", { a: act, w: wait }) : t("Nessun download");
  $("q-sum").textContent = sum;
  $("q-sum").title = sum;
  $("q-mini").firstElementChild.style.width = (run ? pct : 0) + "%";
  $("q-mini").hidden = !run;
  if (!Q.length) $("queue").hidden = true;
}
function openQueue(force, expand) {
  if (!Q.length && !force) return;
  $("queue").hidden = false;
  // elenco aperto solo se c'è spazio (finestre basse: solo la riga riassuntiva)
  if (expand !== undefined) setQueueOpen(expand);
}
function setQueueOpen(on) {
  $("queue").classList.toggle("open", on);
  $("q-more").setAttribute("aria-expanded", on);
  $("q-more").textContent = on ? "▾" : "▴";
}
$("q-more").onclick = () => setQueueOpen(!$("queue").classList.contains("open"));
$("q-toggle").onclick = () => {
  const show = $("queue").hidden && Q.length > 0;
  $("queue").hidden = !show;
  if (show) setQueueOpen(true);
  if (!Q.length) toast(t("Nessun download: la coda è vuota."));
};
$("q-close").onclick = () => ($("queue").hidden = true);
$("q-clear").onclick = () => { for (let i = Q.length - 1; i >= 0; i--) if (Q[i].state === "done") Q.splice(i, 1); saveQueue(); renderQueue(); };

// ---------- Alter Ego che commenta ogni tanto (accensione da TV, scrive, si spegne) ----------
const AE_GAP_MS = 40_000;
let aeTyping = null, aeHide = null, aeLast = 0;
const ae = $("ae");
function aeOff() {
  clearTimeout(aeHide);
  clearInterval(aeTyping);
  if (ae.classList.contains("ae-gone")) return;
  ae.classList.remove("ae-on");
  ae.classList.add("ae-off");
  setTimeout(() => { ae.classList.add("ae-gone"); ae.classList.remove("ae-off"); }, REDUCED.matches ? 0 : 420);
}
function say(text, face, force) {
  const now = Date.now();
  if (!force && now - aeLast < AE_GAP_MS) return;
  aeLast = now;
  const msg = t(text);
  if (face) $("ae-face").src = `assets/sprites/${face}.png`;
  clearInterval(aeTyping);
  clearTimeout(aeHide);
  ae.classList.remove("ae-gone", "ae-off");
  ae.style.bottom = ($("queue").hidden ? 14 : $("queue").offsetHeight + 10) + "px";
  void ae.offsetWidth;
  ae.classList.add("ae-on");
  const el2 = $("ae-text");
  if (REDUCED.matches) el2.textContent = msg;
  else {
    let i = 0;
    el2.textContent = "";
    aeTyping = setInterval(() => { el2.textContent = msg.slice(0, ++i); if (i >= msg.length) clearInterval(aeTyping); }, 26);
  }
  aeHide = setTimeout(aeOff, 2500 + msg.length * 70 + (REDUCED.matches ? 0 : msg.length * 26));
}
ae.onclick = aeOff;

// ---------- tastiera ----------
document.addEventListener("keydown", (e) => {
  const typing = /INPUT|SELECT|TEXTAREA/.test(document.activeElement?.tagName);
  if (e.key === "/" && !typing) { $("q").focus(); e.preventDefault(); }
  else if (e.key === "Escape") {
    if ($("detail").classList.contains("open")) closeDetail();
    else if (!$("queue").hidden) $("queue").hidden = true;
    else aeOff();
  }
});

// ---------- scala sugli schermi grandi (2K, 4K): proporzioni come a ~1500×880, come wide.js ----------
function scale() {
  let z = Math.max(1, Math.min(2.2, Math.min(innerWidth / 1500, innerHeight / 880)));
  if (z < 1.05) z = 1;
  document.body.style.zoom = z === 1 ? "" : String(Math.round(z * 100) / 100);
  document.body.style.setProperty("--vh", z === 1 ? "100vh" : `${innerHeight / z}px`);
}
let resizeT = null;
addEventListener("resize", () => {
  scale();
  clearTimeout(resizeT);
  resizeT = setTimeout(() => { if (S.tab === "rec" || S.tab === "search") render(); }, 200);
});

// ---------- avvio ----------
listen("local-used", () => refreshStatus(true));
(async () => {
  scale();
  try { I18N.setLang(I18N.resolve((await invoke("get_config")).ui_language)); } catch {}
  loadQueue();
  renderQueue();
  let tab = "rec";
  try { tab = localStorage.getItem("ml-tab") || "rec"; } catch {}
  await Promise.all([loadHw(), refreshStatus(true)]);
  showTab(tab);
  if (!S.status?.running) refreshStatus();
  else if (S.hw?.known) {
    say(S.hw.vram >= 4
      ? t("Ho dato un'occhiata al tuo PC: con {gpu} i modelli col pallino verde girano tutti sulla scheda video!", { gpu: S.hw.gpu?.name || "" })
      : "Niente scheda video dedicata… ma i modelli piccoli girano bene anche sulla CPU!", S.hw.vram >= 4 ? "happy" : "thinking", true);
  }
  if (Q.some((x) => x.state === "paused")) openQueue();
  // stato dei modelli caricati (GPU %) ogni tanto
  setInterval(() => { if (!document.hidden) refreshStatus(true); }, 15000);
})();
