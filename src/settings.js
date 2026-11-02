// Finestra impostazioni: generale + AI (provider, chiavi, modelli, ordine di fallback).
const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;
const $ = (id) => document.getElementById(id);

let ai = null;          // copia locale di config.ai
let infos = [];         // provider (id, label, default_base, needs_key, has_key)
let saveTimer = null;

function saveAi() {
  clearTimeout(saveTimer);
  saveTimer = setTimeout(() => invoke("set_ai_config", { ai }), 250);
}

function setResult(li, text, cls = "") {
  const r = li.querySelector(".result");
  r.textContent = text;
  r.className = "result " + cls;
}

function renderProviders() {
  const list = $("providers");
  list.innerHTML = "";
  const tpl = $("provider-tpl");
  ai.order.forEach((id, idx) => {
    const info = infos.find((p) => p.id === id);
    if (!info) return;
    const pc = ai.providers[id] || (ai.providers[id] = { enabled: false, model: "", base_url: "", vision: false });
    const li = tpl.content.firstElementChild.cloneNode(true);
    li.dataset.id = id;
    li.classList.toggle("on", pc.enabled);
    li.querySelector(".label").textContent = `${idx + 1}. ${info.label}`;

    const badge = li.querySelector(".badge");
    if (!info.needs_key) { badge.textContent = "locale, senza chiave"; badge.className = "badge ok"; }
    else if (info.has_key) { badge.textContent = "chiave salvata ✓"; badge.className = "badge ok"; }
    else { badge.textContent = "chiave mancante"; badge.className = "badge missing"; }

    const enabled = li.querySelector(".enabled");
    enabled.checked = pc.enabled;
    enabled.onchange = () => { pc.enabled = enabled.checked; li.classList.toggle("on", pc.enabled); saveAi(); };

    li.querySelector(".up").onclick = () => move(idx, -1);
    li.querySelector(".down").onclick = () => move(idx, +1);

    const keyRow = li.querySelector(".key-row");
    if (!info.needs_key) keyRow.remove();
    else {
      const keyInput = li.querySelector(".key");
      keyInput.placeholder = info.has_key ? "•••••••• (salvata: incollane una nuova per sostituirla)" : "incolla la chiave";
      li.querySelector(".save-key").onclick = async () => {
        try {
          await invoke("set_api_key", { provider: id, key: keyInput.value });
          keyInput.value = "";
          await reloadInfos();
        } catch (e) { setResult(li, String(e), "bad"); }
      };
      li.querySelector(".del-key").onclick = async () => {
        if (!confirm(`Eliminare la chiave di ${info.label}?`)) return;
        await invoke("set_api_key", { provider: id, key: "" });
        await reloadInfos();
      };
    }

    const model = li.querySelector(".model");
    const datalist = li.querySelector("datalist");
    datalist.id = `models-${id}`;
    model.setAttribute("list", datalist.id);
    model.value = pc.model;
    model.placeholder = id === "ollama" ? "vuoto = primo modello installato" : "nome modello";
    model.oninput = () => { pc.model = model.value.trim(); saveAi(); };
    li.querySelector(".load-models").onclick = async (ev) => {
      ev.target.disabled = true;
      setResult(li, "carico…");
      try {
        const models = await invoke("list_models", { provider: id });
        datalist.innerHTML = models.map((m) => `<option value="${m.replace(/"/g, "&quot;")}">`).join("");
        setResult(li, `${models.length} modelli trovati: scegli dal campo Modello`, "ok");
      } catch (e) { setResult(li, String(e), "bad"); }
      ev.target.disabled = false;
    };

    const base = li.querySelector(".base");
    base.value = pc.base_url;
    base.placeholder = info.default_base;
    base.oninput = () => { pc.base_url = base.value.trim(); saveAi(); };

    const vision = li.querySelector(".vision");
    vision.checked = pc.vision;
    vision.onchange = () => { pc.vision = vision.checked; saveAi(); };

    li.querySelector(".test").onclick = async (ev) => {
      ev.target.disabled = true;
      setResult(li, "provo…");
      clearTimeout(saveTimer);
      await invoke("set_ai_config", { ai });
      try { setResult(li, await invoke("test_provider", { provider: id }), "ok"); }
      catch (e) { setResult(li, String(e), "bad"); }
      ev.target.disabled = false;
    };

    list.appendChild(li);
  });
}

function move(idx, delta) {
  const j = idx + delta;
  if (j < 0 || j >= ai.order.length) return;
  [ai.order[idx], ai.order[j]] = [ai.order[j], ai.order[idx]];
  renderProviders();
  saveAi();
}

async function reloadInfos() {
  infos = await invoke("providers_info");
  renderProviders();
}

function fillGeneral(cfg) {
  $("user-name").value = cfg.user_name;
  $("name-tag").value = cfg.name_tag;
}

$("save-general").onclick = () => invoke("set_general", { userName: $("user-name").value, nameTag: $("name-tag").value });

$("temperature").oninput = (e) => { ai.temperature = +e.target.value; $("temperature-val").textContent = ai.temperature.toFixed(2); saveAi(); };
$("max-history").onchange = (e) => { ai.max_history = Math.max(2, Math.min(50, +e.target.value || 12)); saveAi(); };

// ---------- personalità e lingua ----------
let persona = null;
function savePersona() {
  clearTimeout(personaTimer);
  personaTimer = setTimeout(() => invoke("set_persona_options", persona), 300);
}
let personaTimer = null;
function fillPersona(cfg) {
  persona = { language: cfg.language, personality: structuredClone(cfg.personality), memoryEnabled: cfg.memory_enabled };
  $("language").value = cfg.language;
  $("confidence").value = cfg.personality.confidence;
  $("warmth").value = cfg.personality.warmth;
  $("imitate").value = cfg.personality.imitate;
  $("memory-enabled").checked = cfg.memory_enabled;
}
$("language").onchange = (e) => { persona.language = e.target.value; savePersona(); };
$("confidence").oninput = (e) => { persona.personality.confidence = +e.target.value; savePersona(); };
$("warmth").oninput = (e) => { persona.personality.warmth = +e.target.value; savePersona(); };
$("imitate").oninput = (e) => { persona.personality.imitate = e.target.value; savePersona(); };
$("memory-enabled").onchange = (e) => { persona.memoryEnabled = e.target.checked; savePersona(); };
$("open-data").onclick = () => invoke("open_data_dir").catch((e) => alert(e));

// ---------- memoria ----------
const fmtDate = (ts) => new Date(ts * 1000).toLocaleString("it-IT", { dateStyle: "short", timeStyle: "short" });
function listItems(ul, items, onDelete) {
  ul.innerHTML = "";
  if (!items.length) { ul.innerHTML = '<li class="empty">nessuno</li>'; return; }
  for (const it of items) {
    const li = document.createElement("li");
    const span = document.createElement("span");
    span.textContent = it.content;
    const small = document.createElement("small");
    small.textContent = fmtDate(it.ts);
    li.append(span, small);
    if (onDelete) {
      const del = document.createElement("button");
      del.textContent = "✕";
      del.title = "Dimentica questo fatto";
      del.onclick = () => onDelete(it);
      li.append(del);
    }
    ul.append(li);
  }
}
function memResult(text, cls = "") { const r = $("memory-result"); r.textContent = text; r.className = "result " + cls; }
async function refreshMemory() {
  try {
    const o = await invoke("memory_overview");
    $("memory-stats").textContent = `${o.messages} messaggi salvati · file: ${o.db_path}`;
    listItems($("facts"), o.facts, async (it) => { await invoke("memory_delete_fact", { id: it.id }); refreshMemory(); });
    listItems($("summaries"), o.summaries);
  } catch (e) { $("memory-stats").textContent = String(e); }
}
$("refresh-memory").onclick = refreshMemory;
$("add-fact").onclick = async () => {
  const v = $("new-fact").value.trim();
  if (!v) return;
  await invoke("memory_add_fact", { content: v });
  $("new-fact").value = "";
  refreshMemory();
};
$("export-memory").onclick = async () => {
  try { memResult("Esportata in " + (await invoke("memory_export")), "ok"); }
  catch (e) { memResult(String(e), "bad"); }
};
$("forget-all").onclick = async () => {
  if (!confirm("Cancellare TUTTA la memoria di Alter Ego (conversazioni, riassunti, fatti)? Non si può annullare.")) return;
  try { await invoke("memory_forget_all"); memResult("Memoria cancellata.", "ok"); refreshMemory(); }
  catch (e) { memResult(String(e), "bad"); }
};

// ---------- voce ----------
let sttCfg = null;
let sttTimer = null;
function saveStt() { clearTimeout(sttTimer); sttTimer = setTimeout(() => invoke("set_stt", { stt: sttCfg }), 300); }
function fillStt(cfg) {
  sttCfg = structuredClone(cfg.stt);
  $("stt-provider").value = sttCfg.provider;
  $("stt-model").value = sttCfg.model;
  $("stt-url").value = sttCfg.base_url;
  $("stt-language").value = sttCfg.language;
  $("stt-url-row").style.display = sttCfg.provider === "local" ? "" : "none";
}
$("stt-provider").onchange = (e) => { sttCfg.provider = e.target.value; $("stt-url-row").style.display = sttCfg.provider === "local" ? "" : "none"; saveStt(); };
$("stt-model").oninput = (e) => { sttCfg.model = e.target.value.trim(); saveStt(); };
$("stt-url").oninput = (e) => { sttCfg.base_url = e.target.value.trim(); saveStt(); };
$("stt-language").oninput = (e) => { sttCfg.language = e.target.value.trim(); saveStt(); };
$("stt-test").onclick = async (ev) => {
  const r = $("stt-result");
  ev.target.disabled = true;
  clearTimeout(sttTimer);
  await invoke("set_stt", { stt: sttCfg });
  r.className = "result"; r.textContent = "🎙 parla adesso… (4 secondi)";
  try { r.textContent = "Ho capito: " + (await invoke("stt_test")); r.className = "result ok"; }
  catch (e) { r.textContent = String(e); r.className = "result bad"; }
  ev.target.disabled = false;
};

(async () => {
  const cfg = await invoke("get_config");
  fillPersona(cfg);
  fillStt(cfg);
  refreshMemory();
  ai = structuredClone(cfg.ai);
  fillGeneral(cfg);
  $("temperature").value = ai.temperature;
  $("temperature-val").textContent = ai.temperature.toFixed(2);
  $("max-history").value = ai.max_history;
  await reloadInfos();
  // aggiornamenti da altre parti dell'app (non sovrascrive ciò che si sta scrivendo)
  listen("config", (e) => { if (document.activeElement?.tagName !== "INPUT") fillGeneral(e.payload); });
})();
