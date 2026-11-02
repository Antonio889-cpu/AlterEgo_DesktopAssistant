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
    const nKeys = (pc.keys || []).length;
    if (!info.needs_key) { badge.textContent = "locale, senza chiave"; badge.className = "badge ok"; }
    else if (nKeys) { badge.textContent = nKeys === 1 ? "1 chiave ✓" : `${nKeys} chiavi ✓`; badge.className = "badge ok"; }
    else { badge.textContent = "chiave mancante"; badge.className = "badge missing"; }

    const enabled = li.querySelector(".enabled");
    enabled.checked = pc.enabled;
    enabled.onchange = () => { pc.enabled = enabled.checked; li.classList.toggle("on", pc.enabled); saveAi(); };

    li.querySelector(".up").onclick = () => move(idx, -1);
    li.querySelector(".down").onclick = () => move(idx, +1);

    const keyRow = li.querySelector(".key-row");
    if (!info.needs_key) keyRow.remove();
    else {
      const ul = li.querySelector(".keys");
      if (!nKeys) ul.innerHTML = '<li class="empty">nessuna chiave salvata</li>';
      (pc.keys || []).forEach((k, i) => {
        const row = document.createElement("li");
        const label = document.createElement("span");
        label.textContent = `${i + 1}. ${k.label} (salvata ✓)`;
        const up = Object.assign(document.createElement("button"), { textContent: "▲", title: "Prova prima" });
        const down = Object.assign(document.createElement("button"), { textContent: "▼", title: "Prova dopo" });
        const del = Object.assign(document.createElement("button"), { textContent: "Elimina" });
        up.disabled = i === 0;
        down.disabled = i === nKeys - 1;
        up.onclick = async () => { await invoke("move_api_key", { provider: id, id: k.id, delta: -1 }); await syncKeys(); };
        down.onclick = async () => { await invoke("move_api_key", { provider: id, id: k.id, delta: 1 }); await syncKeys(); };
        del.onclick = async () => {
          if (!confirm(`Eliminare la chiave "${k.label}" di ${info.label}?`)) return;
          try { await invoke("delete_api_key", { provider: id, id: k.id }); } catch (e) { setResult(li, String(e), "bad"); }
          await syncKeys();
        };
        row.append(label, up, down, del);
        ul.append(row);
      });
      li.querySelector(".add-key").onclick = async () => {
        const keyInput = li.querySelector(".key");
        const labelInput = li.querySelector(".key-label");
        if (!keyInput.value.trim()) return;
        try {
          clearTimeout(saveTimer);
          await invoke("set_ai_config", { ai });
          await invoke("add_api_key", { provider: id, label: labelInput.value, key: keyInput.value });
          keyInput.value = "";
          labelInput.value = "";
          await syncKeys();
        } catch (e) { setResult(li, String(e), "bad"); }
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

/** Dopo aggiunta/eliminazione/spostamento: riprende l'elenco chiavi dalla config. */
async function syncKeys() {
  const cfg = await invoke("get_config");
  for (const [id, pc] of Object.entries(cfg.ai.providers)) {
    if (ai.providers[id]) ai.providers[id].keys = pc.keys;
  }
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


// ---------- schede ----------
function showTab(name) {
  document.querySelectorAll("nav#tabs button").forEach((b) => b.classList.toggle("on", b.dataset.tab === name));
  document.querySelectorAll("main section").forEach((sec) => { sec.hidden = sec.dataset.tab !== name; });
  try { localStorage.setItem("tab", name); } catch {}
  if (name === "log") refreshLog();
  if (name === "memory") refreshMemory();
}
document.querySelectorAll("nav#tabs button").forEach((b) => (b.onclick = () => showTab(b.dataset.tab)));

// ---------- aspetto e avvio ----------
let lookTimer = null;
function saveLook() {
  clearTimeout(lookTimer);
  lookTimer = setTimeout(() => invoke("set_look", {
    opacity: +$("look-opacity").value, background: $("look-bg").value, bgIntensity: +$("look-int").value,
    dialogStyle: $("look-dlg").value, effects: $("look-fx").checked,
  }), 200);
}
function fillLook(cfg) {
  $("look-opacity").value = cfg.opacity;
  $("look-bg").value = cfg.background;
  $("look-int").value = cfg.bg_intensity;
  $("look-dlg").value = cfg.dialog_style;
  $("look-fx").checked = cfg.effects;
  $("autostart").checked = cfg.autostart;
}
["look-opacity", "look-int"].forEach((id) => ($(id).oninput = saveLook));
["look-bg", "look-dlg", "look-fx"].forEach((id) => ($(id).onchange = saveLook));
$("autostart").onchange = async (e) => {
  const r = $("autostart-result");
  try { await invoke("set_autostart", { on: e.target.checked }); r.textContent = e.target.checked ? "attivo" : "disattivato"; r.className = "result ok"; }
  catch (err) { r.textContent = String(err); r.className = "result bad"; e.target.checked = !e.target.checked; }
};

// ---------- audio ----------
let audio = null;
let audioTimer = null;
function saveAudio() { clearTimeout(audioTimer); audioTimer = setTimeout(() => invoke("set_audio", { audio }), 250); }
function fillVoices() {
  const sel = $("tts-voice");
  const voices = window.speechSynthesis?.getVoices() || [];
  sel.innerHTML = '<option value="">Automatica (italiana se presente)</option>' +
    voices.map((v) => `<option value="${v.name.replace(/"/g, "&quot;")}">${v.name} (${v.lang})</option>`).join("");
  if (audio) sel.value = audio.tts_voice;
}
window.speechSynthesis?.addEventListener?.("voiceschanged", fillVoices);
function ttsRows() {
  $("tts-voice-row").style.display = audio.tts === "windows" ? "" : "none";
  $("tts-voice-note").style.display = audio.tts === "windows" ? "" : "none";
  $("tts-openai").style.display = audio.tts === "openai" ? "" : "none";
}
function fillAudio(cfg) {
  audio = structuredClone(cfg.audio);
  $("blips").value = audio.blips;
  $("blip-volume").value = audio.blip_volume;
  $("tts").value = audio.tts;
  $("tts-url").value = audio.tts_url;
  $("tts-model").value = audio.tts_model;
  $("tts-remote-voice").value = audio.tts === "openai" ? audio.tts_voice : "";
  $("tts-rate").value = audio.tts_rate;
  $("tts-volume").value = audio.tts_volume;
  $("mute-blips").checked = audio.mute_blips_with_tts;
  fillVoices();
  ttsRows();
}
$("blips").onchange = (e) => { audio.blips = e.target.value; saveAudio(); };
$("blip-volume").oninput = (e) => { audio.blip_volume = +e.target.value; saveAudio(); };
$("tts").onchange = (e) => { audio.tts = e.target.value; audio.tts_voice = ""; ttsRows(); fillVoices(); saveAudio(); };
$("tts-voice").onchange = (e) => { audio.tts_voice = e.target.value; saveAudio(); };
$("tts-remote-voice").oninput = (e) => { audio.tts_voice = e.target.value.trim(); saveAudio(); };
$("tts-url").oninput = (e) => { audio.tts_url = e.target.value.trim(); saveAudio(); };
$("tts-model").oninput = (e) => { audio.tts_model = e.target.value.trim(); saveAudio(); };
$("tts-rate").oninput = (e) => { audio.tts_rate = +e.target.value; saveAudio(); };
$("tts-volume").oninput = (e) => { audio.tts_volume = +e.target.value; saveAudio(); };
$("mute-blips").onchange = (e) => { audio.mute_blips_with_tts = e.target.checked; saveAudio(); };
$("tts-key-save").onclick = async () => {
  try { await invoke("set_tts_key", { key: $("tts-key").value }); $("tts-key").value = ""; $("tts-result").textContent = "chiave salvata"; $("tts-result").className = "result ok"; }
  catch (e) { $("tts-result").textContent = String(e); $("tts-result").className = "result bad"; }
};
$("tts-test").onclick = async () => {
  const r = $("tts-result");
  const text = "Ciao! Sono Alter Ego. Così suona la mia voce.";
  clearTimeout(audioTimer);
  await invoke("set_audio", { audio });
  r.className = "result"; r.textContent = "";
  if (audio.tts === "windows") {
    const u = new SpeechSynthesisUtterance(text);
    const vs = speechSynthesis.getVoices();
    u.voice = vs.find((v) => v.name === audio.tts_voice) || vs.find((v) => v.lang?.startsWith("it")) || null;
    u.lang = u.voice?.lang || "it-IT"; u.rate = audio.tts_rate; u.volume = audio.tts_volume;
    speechSynthesis.cancel(); speechSynthesis.speak(u);
    if (!u.voice) { r.textContent = "nessuna voce italiana installata: uso quella predefinita"; r.className = "result bad"; }
  } else if (audio.tts === "openai") {
    try {
      const b64 = await invoke("tts_speak", { text });
      const el = new Audio("data:audio/mpeg;base64," + b64); el.volume = audio.tts_volume; await el.play();
    } catch (e) { r.textContent = String(e); r.className = "result bad"; }
  } else { r.textContent = "la voce è spenta"; }
};

// ---------- tasti ----------
let hotkeys = null;
const CODE_NAMES = { Space: "Space", Enter: "Enter", Tab: "Tab", Escape: "Escape", ArrowUp: "Up", ArrowDown: "Down", ArrowLeft: "Left", ArrowRight: "Right",
  PageUp: "PageUp", PageDown: "PageDown", Home: "Home", End: "End", Insert: "Insert", Pause: "Pause", PrintScreen: "PrintScreen" };
function comboFromEvent(e) {
  const code = e.code;
  if (["ControlLeft", "ControlRight", "AltLeft", "AltRight", "ShiftLeft", "ShiftRight", "MetaLeft", "MetaRight"].includes(code)) return null;
  let key = CODE_NAMES[code];
  if (!key && /^Key[A-Z]$/.test(code)) key = code.slice(3);
  if (!key && /^Digit\d$/.test(code)) key = code.slice(5);
  if (!key && /^F\d{1,2}$/.test(code)) key = code;
  if (!key && /^Numpad\d$/.test(code)) key = code;
  if (!key) key = code;
  const mods = [];
  if (e.ctrlKey) mods.push("Ctrl");
  if (e.altKey) mods.push("Alt");
  if (e.shiftKey) mods.push("Shift");
  if (e.metaKey) mods.push("Super");
  return [...mods, key].join("+");
}
/** Campo che registra una combinazione di tasti (e, se allowMouse, un tasto del mouse). */
function captureInput(input, onSet, allowMouse) {
  input.onfocus = () => { input.classList.add("capturing"); input.dataset.old = input.value; input.value = "premi i tasti…"; };
  input.onblur = () => { input.classList.remove("capturing"); if (input.value === "premi i tasti…") input.value = input.dataset.old || ""; };
  input.onkeydown = (e) => {
    e.preventDefault();
    if (e.key === "Delete" || e.key === "Backspace") { input.value = ""; onSet(""); input.blur(); return; }
    if (e.key === "Escape" && !e.ctrlKey && !e.altKey) { input.blur(); return; }
    const c = comboFromEvent(e);
    if (c) { input.value = c; onSet(c); input.blur(); }
  };
  input.onmousedown = (e) => {
    if (!allowMouse || document.activeElement !== input) return;
    const name = { 1: "MouseMiddle", 3: "Mouse4", 4: "Mouse5" }[e.button];
    if (name) { e.preventDefault(); input.value = name; onSet(name); input.blur(); }
  };
  input.onauxclick = (e) => e.preventDefault();
}
function fillHotkeys(cfg) {
  hotkeys = structuredClone(cfg.hotkeys);
  document.querySelectorAll("input.hotkey[data-key]").forEach((inp) => {
    inp.value = hotkeys[inp.dataset.key] || "";
    captureInput(inp, (v) => { hotkeys[inp.dataset.key] = v; }, !!inp.dataset.mouse);
  });
  const r = $("hotkey-result");
  if (cfg.hotkey_errors?.length) { r.textContent = cfg.hotkey_errors.join("; "); r.className = "result bad"; }
}
$("save-hotkeys").onclick = async () => {
  const errors = await invoke("set_hotkeys", { hotkeys });
  const r = $("hotkey-result");
  r.textContent = errors.length ? errors.join("; ") : "tasti salvati ✓";
  r.className = "result " + (errors.length ? "bad" : "ok");
};

// ---------- clip ----------
let clip = null;
let clipTimer = null;
function saveClip() { clearTimeout(clipTimer); clipTimer = setTimeout(() => invoke("set_clip", { clip }), 300); }
function renderClipTargets() {
  const box = $("clip-targets");
  box.innerHTML = "";
  clip.targets.forEach((t, i) => {
    const row = document.createElement("div");
    row.className = "clip-target";
    const en = Object.assign(document.createElement("input"), { type: "checkbox", checked: t.enabled });
    const name = Object.assign(document.createElement("input"), { className: "name", value: t.name });
    const hk = Object.assign(document.createElement("input"), { className: "hotkey", value: t.hotkey, readOnly: true });
    const del = Object.assign(document.createElement("button"), { textContent: "✕", title: "Rimuovi" });
    en.onchange = () => { t.enabled = en.checked; saveClip(); };
    name.oninput = () => { t.name = name.value; saveClip(); };
    captureInput(hk, (v) => { t.hotkey = v; saveClip(); }, false);
    del.onclick = () => { clip.targets.splice(i, 1); renderClipTargets(); saveClip(); };
    row.append(en, name, hk, del);
    box.append(row);
  });
  const add = Object.assign(document.createElement("button"), { textContent: "+ Aggiungi programma" });
  add.onclick = () => { clip.targets.push({ name: "Altro programma", hotkey: "", enabled: true }); renderClipTargets(); saveClip(); };
  box.append(add);
}
function fillClip(cfg) {
  clip = structuredClone(cfg.clip);
  $("clip-enabled").checked = clip.enabled;
  $("clip-phrases").value = clip.phrases.join("\n");
  $("clip-reply").value = clip.reply;
  renderClipTargets();
}
$("clip-enabled").onchange = (e) => { clip.enabled = e.target.checked; saveClip(); };
$("clip-phrases").oninput = (e) => { clip.phrases = e.target.value.split("\n").map((x) => x.trim()).filter(Boolean); saveClip(); };
$("clip-reply").oninput = (e) => { clip.reply = e.target.value; saveClip(); };
$("clip-test").onclick = async (ev) => {
  const r = $("clip-result");
  ev.target.disabled = true;
  clearTimeout(clipTimer);
  await invoke("set_clip", { clip });
  for (let i = 3; i > 0; i--) { r.textContent = `torna al gioco… ${i}`; r.className = "result"; await new Promise((ok) => setTimeout(ok, 1000)); }
  try { r.textContent = await invoke("test_clip"); r.className = "result ok"; }
  catch (e) { r.textContent = String(e); r.className = "result bad"; }
  ev.target.disabled = false;
};

// ---------- permessi ----------
let perms = null;
let permsTimer = null;
function savePerms() { clearTimeout(permsTimer); permsTimer = setTimeout(() => invoke("set_permissions", { permissions: perms }), 300); }
function renderList(div) {
  const key = div.dataset.list;
  const cols = div.dataset.cols.split("|");
  const items = perms[key];
  div.innerHTML = "";
  items.forEach((item, i) => {
    const row = document.createElement("div");
    row.className = "row";
    if (cols.length === 1) {
      const inp = Object.assign(document.createElement("input"), { value: item, placeholder: cols[0] });
      inp.oninput = () => { items[i] = inp.value.trim(); savePerms(); };
      row.append(inp);
    } else {
      const n = Object.assign(document.createElement("input"), { value: item.name, placeholder: cols[0] });
      const t = Object.assign(document.createElement("input"), { value: item.target, placeholder: cols[1] });
      n.style.flex = "1"; t.style.flex = "2";
      n.oninput = () => { item.name = n.value.trim(); savePerms(); };
      t.oninput = () => { item.target = t.value.trim(); savePerms(); };
      row.append(n, t);
    }
    const del = Object.assign(document.createElement("button"), { textContent: "✕" });
    del.onclick = () => { items.splice(i, 1); renderList(div); savePerms(); };
    row.append(del);
    div.append(row);
  });
  const add = Object.assign(document.createElement("button"), { textContent: "+ Aggiungi" });
  add.onclick = () => { items.push(cols.length === 1 ? "" : { name: "", target: "" }); renderList(div); savePerms(); };
  div.append(add);
}
function fillPerms(cfg) {
  perms = structuredClone(cfg.permissions);
  document.querySelectorAll("input[data-perm]").forEach((c) => {
    c.checked = !!perms[c.dataset.perm];
    c.onchange = () => { perms[c.dataset.perm] = c.checked; savePerms(); };
  });
  document.querySelectorAll(".list[data-list]").forEach(renderList);
}

// ---------- registro ----------
async function refreshLog() {
  const rows = await invoke("actions_log");
  const tb = document.querySelector("#log-table tbody");
  tb.innerHTML = "";
  if (!rows.length) { tb.innerHTML = '<tr><td colspan="4" class="note">nessuna azione registrata</td></tr>'; return; }
  for (const r of rows) {
    const tr = document.createElement("tr");
    [fmtDate(r.ts), r.azione, r.dettaglio, r.esito].forEach((v, i) => {
      const td = document.createElement("td");
      td.textContent = v ?? "";
      if (i === 3) td.className = v === "ok" || v === "notificato" ? "ok" : "bad";
      tr.append(td);
    });
    tb.append(tr);
  }
}
$("log-refresh").onclick = refreshLog;
$("log-clear").onclick = async () => { if (confirm("Svuotare il registro delle azioni?")) { await invoke("clear_actions_log"); refreshLog(); } };

(async () => {
  const cfg = await invoke("get_config");
  fillLook(cfg);
  fillAudio(cfg);
  fillHotkeys(cfg);
  fillClip(cfg);
  fillPerms(cfg);
  let tab = "general";
  try { tab = localStorage.getItem("tab") || "general"; } catch {}
  showTab(tab);
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
  listen("config", (e) => {
    if (["INPUT", "TEXTAREA", "SELECT"].includes(document.activeElement?.tagName)) return;
    fillGeneral(e.payload);
    fillLook(e.payload);
  });
})();
