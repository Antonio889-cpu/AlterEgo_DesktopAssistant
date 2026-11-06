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
    else if (nKeys) { badge.textContent = nKeys === 1 ? "1 chiave ✓" : t("{n} chiavi ✓", { n: nKeys }); badge.className = "badge ok"; }
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
        label.textContent = t("{i}. {label} (salvata ✓)", { i: i + 1, label: k.label });
        const up = Object.assign(document.createElement("button"), { textContent: "▲", title: "Prova prima" });
        const down = Object.assign(document.createElement("button"), { textContent: "▼", title: "Prova dopo" });
        const del = Object.assign(document.createElement("button"), { textContent: "Elimina" });
        up.disabled = i === 0;
        down.disabled = i === nKeys - 1;
        up.onclick = async () => { await invoke("move_api_key", { provider: id, id: k.id, delta: -1 }); await syncKeys(); };
        down.onclick = async () => { await invoke("move_api_key", { provider: id, id: k.id, delta: 1 }); await syncKeys(); };
        del.onclick = async () => {
          if (!confirm(t("Eliminare la chiave \"{label}\" di {provider}?", { label: k.label, provider: info.label }))) return;
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
        setResult(li, t("{n} modelli trovati: scegli dal campo Modello", { n: models.length }), "ok");
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
  $("unfiltered").checked = !!cfg.personality.unfiltered;
  $("memory-enabled").checked = cfg.memory_enabled;
}
$("language").onchange = (e) => { persona.language = e.target.value; savePersona(); };
$("confidence").oninput = (e) => { persona.personality.confidence = +e.target.value; savePersona(); };
$("warmth").oninput = (e) => { persona.personality.warmth = +e.target.value; savePersona(); };
$("imitate").oninput = (e) => { persona.personality.imitate = e.target.value; savePersona(); };
$("unfiltered").onchange = (e) => { persona.personality.unfiltered = e.target.checked; savePersona(); };
$("memory-enabled").onchange = (e) => { persona.memoryEnabled = e.target.checked; savePersona(); };
$("open-data").onclick = () => invoke("open_data_dir").catch((e) => alert(e));

// ---------- memoria ----------
const fmtDate = (ts) => new Date(ts * 1000).toLocaleString(I18N.lang(), { dateStyle: "short", timeStyle: "short" });
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
    $("memory-stats").textContent = t("{n} messaggi salvati · file: {path}", { n: o.messages, path: o.db_path });
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
  try { memResult(t("Esportata in {path}", { path: await invoke("memory_export") }), "ok"); }
  catch (e) { memResult(String(e), "bad"); }
};
$("forget-all").onclick = async () => {
  if (!confirm(t("Cancellare TUTTA la memoria di Alter Ego (conversazioni, riassunti, fatti)? Non si può annullare."))) return;
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
  $("stt-fallback").value = sttCfg.fallback || "none";
  $("stt-gemini-model").value = sttCfg.gemini_model || "";
  sttRows();
}
function sttRows() {
  $("stt-url-row").style.display = sttCfg.provider === "local" || sttCfg.fallback === "local" ? "" : "none";
  $("stt-gemini-row").style.display = sttCfg.provider === "gemini" || sttCfg.fallback === "gemini" ? "" : "none";
}
$("stt-provider").onchange = (e) => { sttCfg.provider = e.target.value; if (sttCfg.fallback === sttCfg.provider) { sttCfg.fallback = "none"; $("stt-fallback").value = "none"; } sttRows(); saveStt(); };
$("stt-fallback").onchange = (e) => { sttCfg.fallback = e.target.value; sttRows(); saveStt(); };
$("stt-gemini-model").oninput = (e) => { sttCfg.gemini_model = e.target.value.trim(); saveStt(); };
$("stt-model").oninput = (e) => { sttCfg.model = e.target.value.trim(); saveStt(); };
$("stt-url").oninput = (e) => { sttCfg.base_url = e.target.value.trim(); saveStt(); };
$("stt-language").oninput = (e) => { sttCfg.language = e.target.value.trim(); saveStt(); };
$("stt-test").onclick = async (ev) => {
  const r = $("stt-result");
  ev.target.disabled = true;
  clearTimeout(sttTimer);
  await invoke("set_stt", { stt: sttCfg });
  r.className = "result"; r.textContent = "🎙 parla adesso… (4 secondi)";
  try { r.textContent = t("Ho capito: {text}", { text: await invoke("stt_test") }); r.className = "result ok"; }
  catch (e) { r.textContent = String(e); r.className = "result bad"; }
  ev.target.disabled = false;
};


// ---------- sprite ----------
async function spriteStatus() {
  const sprites = await invoke("load_user_sprites");
  const n = Object.keys(sprites || {}).length;
  $("sprite-status").textContent = n ? t("Sprite importati: {list}", { list: Object.keys(sprites).join(", ") }) : t("Nessuno sprite importato: Alter Ego usa quelli inclusi.");
}
function spriteResult(t, cls = "") { const r = $("sprite-result"); r.textContent = t; r.className = "result " + cls; }
$("sprite-file").onchange = async (e) => {
  const f = e.target.files[0];
  if (!f) return;
  spriteResult("elaboro…");
  const data = await new Promise((ok, ko) => { const r = new FileReader(); r.onload = () => ok(r.result); r.onerror = ko; r.readAsDataURL(f); });
  try { const saved = await invoke("import_sprite_sheet", { data }); spriteResult(t("importati {n} sprite ✓", { n: saved.length }), "ok"); }
  catch (err) { spriteResult(String(err), "bad"); }
  e.target.value = "";
  spriteStatus();
};
$("sprite-dir").onclick = () => invoke("open_sprite_dir").catch((e) => spriteResult(String(e), "bad"));
$("sprite-reload").onclick = async () => { await invoke("reload_sprites"); spriteStatus(); spriteResult("ricaricati", "ok"); };
$("sprite-remove").onclick = async () => {
  if (!confirm(t("Rimuovere gli sprite importati?"))) return;
  try { await invoke("remove_user_sprites"); spriteResult("rimossi", "ok"); } catch (err) { spriteResult(String(err), "bad"); }
  spriteStatus();
};

// ---------- schede ----------
function showTab(name) {
  document.querySelectorAll("nav#tabs button").forEach((b) => b.classList.toggle("on", b.dataset.tab === name));
  document.querySelectorAll("main section").forEach((sec) => { sec.hidden = sec.dataset.tab !== name; });
  try { localStorage.setItem("tab", name); } catch {}
  if (name === "log") refreshLog();
  if (name === "memory") refreshMemory();
  if (name === "persona") spriteStatus();
  if (name === "voice" && audio) { refreshDevices().then(refreshAudioStatus); if (audio.tts === "kokoro") refreshKokoro(); }
  if (name === "ai") refreshUsage();
  if (name === "costs") refreshCosts();
}

/** Token e cache dei provider da quando l'app è aperta (DeepSeek riporta la cache). */
async function refreshUsage() {
  const el = $("ai-usage");
  let u = {};
  try { u = await invoke("ai_usage"); } catch { return; }
  const parts = Object.entries(u).filter(([, v]) => v.requests).map(([id, v]) => {
    const pct = v.input ? Math.round((v.cached / v.input) * 100) : 0;
    return t("{id}: {n} richieste, {tok} token in ingresso", { id, n: v.requests, tok: v.input.toLocaleString(I18N.lang()) }) + (v.cached || id === "deepseek" ? t(" ({pct}% dalla cache)", { pct }) : "");
  });
  el.textContent = parts.length ? t("Da quando Alter Ego è aperto — {list}", { list: parts.join(" · ") }) : "";
}
document.querySelectorAll("nav#tabs button").forEach((b) => (b.onclick = () => showTab(b.dataset.tab)));

// ---------- aspetto e avvio ----------
let lookTimer = null;
function saveLook() {
  clearTimeout(lookTimer);
  lookTimer = setTimeout(() => invoke("set_look", {
    opacity: +$("look-opacity").value, background: $("look-bg").value, bgIntensity: +$("look-int").value,
    dialogStyle: $("look-dlg").value, effects: $("look-fx").checked, tvAnimation: $("look-tv").checked,
    autoAdvance: $("auto-advance").checked, autoAdvanceSecs: +$("auto-advance-secs").value,
  }), 200);
  $("auto-advance-val").textContent = (+$("auto-advance-secs").value).toFixed(1) + " s";
}
function applyLang(cfg) {
  I18N.setLang(I18N.resolve(cfg.ui_language));
  $("ui-language").value = cfg.ui_language || "auto";
}
$("ui-language").onchange = (e) => invoke("set_ui_language", { lang: e.target.value });

function fillLook(cfg) {
  $("look-opacity").value = cfg.opacity;
  $("look-bg").value = cfg.background;
  $("look-int").value = cfg.bg_intensity;
  $("look-dlg").value = cfg.dialog_style;
  $("look-fx").checked = cfg.effects;
  $("look-tv").checked = cfg.tv_animation;
  $("auto-advance").checked = cfg.auto_advance;
  $("auto-advance-secs").value = cfg.auto_advance_secs;
  $("auto-advance-val").textContent = cfg.auto_advance_secs.toFixed(1) + " s";
  $("autostart").checked = cfg.autostart;
}
["look-opacity", "look-int", "auto-advance-secs"].forEach((id) => ($(id).oninput = saveLook));
["look-bg", "look-dlg", "look-fx", "look-tv", "auto-advance"].forEach((id) => ($(id).onchange = saveLook));
$("autostart").onchange = async (e) => {
  const r = $("autostart-result");
  try { await invoke("set_autostart", { on: e.target.checked }); r.textContent = e.target.checked ? "attivo" : "disattivato"; r.className = "result ok"; }
  catch (err) { r.textContent = String(err); r.className = "result bad"; e.target.checked = !e.target.checked; }
};

// ---------- audio ----------
let audio = null;
let audioTimer = null;
let devices = null;
function saveAudio() {
  clearTimeout(audioTimer);
  audioTimer = setTimeout(async () => {
    await invoke("set_audio", { audio });
    setTimeout(refreshAudioStatus, 600);
  }, 250);
}
const esc = (t) => String(t).replace(/&/g, "&amp;").replace(/"/g, "&quot;").replace(/</g, "&lt;");
/** Riempie un <select> di dispositivi: "" = predefinito; un dispositivo scollegato resta visibile. */
function fillDeviceSelect(sel, list, value, defLabel) {
  const opts = [`<option value="">${esc(defLabel)}</option>`].concat(list.map((n) => `<option value="${esc(n)}">${esc(n)}</option>`));
  if (value && !list.includes(value)) opts.push(`<option value="${esc(value)}">${esc(t("{name} (non collegato)", { name: value }))}</option>`);
  sel.innerHTML = opts.join("");
  sel.value = value || "";
}
async function refreshDevices() {
  try { devices = await invoke("audio_devices"); } catch (e) { $("dev-status").textContent = String(e); return; }
  const outs = devices.outputs.filter((n) => n !== devices.cable_render);
  const ins = devices.inputs.filter((n) => n !== devices.cable_capture);
  fillDeviceSelect($("out-device"), outs, audio.output_device, t("Predefinito di Windows{dev}", { dev: devices.default_output ? " (" + devices.default_output + ")" : "" }));
  fillDeviceSelect($("in-device"), ins, audio.input_device, t("Predefinito di Windows{dev}", { dev: devices.default_input ? " (" + devices.default_input + ")" : "" }));
  fillDeviceSelect($("link-mic"), ins, audio.mic_link.mic_device, "Microfono predefinito di Windows");
  fillDeviceSelect($("link-cable"), devices.outputs, audio.mic_link.cable_device, t("Automatico{dev}", { dev: devices.cable_render ? " (" + devices.cable_render + ")" : t(" (nessun cavo trovato)") }));
  const d = $("link-driver");
  if (!devices.cable_render) { d.textContent = "VB-CABLE non trovato: installalo (punto 1) e riavvia il PC."; d.className = "result bad"; }
  else if (!devices.link_named) { d.textContent = t("Cavo trovato: {name}. Rinominalo (punto 2) per vederlo come «AlterEgo Link Audio».", { name: devices.cable_capture || devices.cable_render }); d.className = "result"; }
  else { d.textContent = t("Pronto: le altre app vedono «{name}».", { name: devices.cable_capture }); d.className = "result ok"; }
  if (devices.default_output && devices.default_output === devices.cable_render) {
    d.textContent += t(" ⚠ Il cavo è l'uscita predefinita di Windows: rimetti i tuoi altoparlanti.");
    d.className = "result bad";
  }
}
async function refreshAudioStatus() {
  try {
    const st = await invoke("audio_status");
    const ds = $("dev-status");
    ds.textContent = st.output_error ? t("Uscita: {err}", { err: st.output_error }) : st.output ? t("In uso: {name}", { name: st.output }) : "";
    ds.className = st.output_error ? "result bad" : "result ok";
    const ls = $("link-status");
    if (!audio.mic_link.enabled) { ls.textContent = "Mic-Link spento"; ls.className = "result"; }
    else if (st.link_active) {
      ls.textContent = t("Attivo: {mic} + voce → {cable}", { mic: st.link_mic || t("solo voce"), cable: st.link_cable }) + (st.link_error ? ` (${st.link_error})` : "");
      ls.className = st.link_error ? "result bad" : "result ok";
    } else { ls.textContent = st.link_error || "non attivo"; ls.className = "result bad"; }
  } catch {}
}
async function fillVoices() {
  const sel = $("tts-voice");
  let voices = [];
  try { voices = await invoke("tts_voices", { engine: "windows" }); } catch {}
  sel.innerHTML = '<option value="">Automatica (italiana se presente)</option>' +
    voices.map((v) => `<option value="${esc(v.name)}">${esc(v.name)} (${esc(v.lang)})</option>`).join("");
  if (audio) sel.value = voices.some((v) => v.name === audio.tts_voice) ? audio.tts_voice : "";
  const ko = await invoke("tts_voices", { engine: "kokoro" });
  const opt = (list) => list.map((v) => `<option value="${v.id}">${esc(v.name)} — ${v.id}</option>`).join("");
  $("kokoro-voice").innerHTML = opt(ko);
  $("kokoro-voice-en").innerHTML = opt(ko.filter((v) => v.lang === "en"));
  $("kokoro-voice").value = audio.kokoro_voice;
  $("kokoro-voice-en").value = audio.kokoro_voice_en;
}
function ttsRows() {
  $("tts-voice-row").style.display = audio.tts === "windows" ? "" : "none";
  $("tts-voice-note").style.display = audio.tts === "windows" ? "" : "none";
  $("tts-openai").style.display = audio.tts === "openai" ? "" : "none";
  $("tts-kokoro").style.display = audio.tts === "kokoro" ? "" : "none";
  $("tts-rate-row").style.display = audio.tts === "kokoro" ? "none" : "";
  if (audio.tts === "kokoro") refreshKokoro();
}
let kokoroTimer = null;
async function refreshKokoro() {
  clearTimeout(kokoroTimer);
  const el = $("kokoro-state");
  try {
    const st = await invoke("kokoro_status");
    $("kokoro-install").textContent = st.installed ? "Reinstalla / aggiorna" : "Installa Kokoro";
    $("kokoro-start").disabled = !st.installed || st.running;
    $("kokoro-stop").disabled = !st.running;
    if (st.ready) { el.textContent = "Kokoro pronto ✓"; el.className = "result ok"; }
    else if (!st.installed) { el.textContent = "Non installato (o installazione in corso nella finestra di PowerShell)."; el.className = "result bad"; }
    else { el.textContent = st.running ? t("Server avviato: {msg}", { msg: st.message }) : t("Installato, server fermo ({msg})", { msg: st.message }); el.className = "result"; }
  } catch (e) { el.textContent = String(e); el.className = "result bad"; }
  // aggiorna da solo mentre la scheda è aperta e la voce scelta è Kokoro
  if (audio.tts === "kokoro" && !document.hidden) kokoroTimer = setTimeout(refreshKokoro, 4000);
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
  $("kokoro-url").value = audio.kokoro_url;
  $("kokoro-speed").value = audio.kokoro_speed;
  $("kokoro-autostart").checked = audio.kokoro_autostart;
  const ml = audio.mic_link;
  $("link-enabled").checked = ml.enabled;
  $("link-mic-vol").value = ml.mic_volume;
  $("link-voice-vol").value = ml.voice_volume;
  $("link-blips").checked = ml.include_blips;
  $("link-music").checked = !!ml.include_music;
  $("ns-enabled").checked = !!ml.noise_suppression;
  $("ns-strength").value = ml.ns_strength ?? 0.9;
  $("ns-gate").value = ml.ns_gate ?? 0.3;
  nsLabels();
  fillVoices();
  ttsRows();
  refreshDevices().then(refreshAudioStatus);
}
$("blips").onchange = (e) => { audio.blips = e.target.value; saveAudio(); };
$("blip-volume").oninput = (e) => { audio.blip_volume = +e.target.value; saveAudio(); };
$("tts").onchange = (e) => {
  // la voce di Windows e quella del servizio remoto usano lo stesso campo
  if (audio.tts === "windows" || e.target.value === "windows") audio.tts_voice = "";
  audio.tts = e.target.value; ttsRows(); saveAudio();
};
$("tts-voice").onchange = (e) => { audio.tts_voice = e.target.value; saveAudio(); };
$("tts-remote-voice").oninput = (e) => { audio.tts_voice = e.target.value.trim(); saveAudio(); };
$("tts-url").oninput = (e) => { audio.tts_url = e.target.value.trim(); saveAudio(); };
$("tts-model").oninput = (e) => { audio.tts_model = e.target.value.trim(); saveAudio(); };
$("tts-rate").oninput = (e) => { audio.tts_rate = +e.target.value; saveAudio(); };
$("tts-volume").oninput = (e) => { audio.tts_volume = +e.target.value; saveAudio(); };
$("mute-blips").onchange = (e) => { audio.mute_blips_with_tts = e.target.checked; saveAudio(); };
$("kokoro-url").oninput = (e) => { audio.kokoro_url = e.target.value.trim(); saveAudio(); };
$("kokoro-speed").oninput = (e) => { audio.kokoro_speed = +e.target.value; saveAudio(); };
$("kokoro-voice").onchange = (e) => { audio.kokoro_voice = e.target.value; saveAudio(); };
$("kokoro-voice-en").onchange = (e) => { audio.kokoro_voice_en = e.target.value; saveAudio(); };
$("kokoro-autostart").onchange = (e) => { audio.kokoro_autostart = e.target.checked; saveAudio(); };
$("kokoro-install").onclick = async () => {
  const el = $("kokoro-state");
  try {
    await invoke("kokoro_install", { cuda: $("kokoro-cuda").checked });
    el.textContent = "Installazione avviata nella finestra di PowerShell: segui lì l'avanzamento (può volerci un po').";
    el.className = "result";
    kokoroTimer = setTimeout(refreshKokoro, 8000);
  } catch (e) { el.textContent = String(e); el.className = "result bad"; }
};
$("kokoro-start").onclick = async () => {
  try { await invoke("kokoro_start"); } catch (e) { $("kokoro-state").textContent = String(e); $("kokoro-state").className = "result bad"; return; }
  setTimeout(refreshKokoro, 1500);
};
$("kokoro-stop").onclick = async () => { await invoke("kokoro_stop"); setTimeout(refreshKokoro, 500); };
$("out-device").onchange = (e) => { audio.output_device = e.target.value; saveAudio(); };
$("in-device").onchange = (e) => { audio.input_device = e.target.value; saveAudio(); };
$("dev-refresh").onclick = () => refreshDevices().then(refreshAudioStatus);
$("link-enabled").onchange = (e) => { audio.mic_link.enabled = e.target.checked; saveAudio(); };
$("link-mic").onchange = (e) => { audio.mic_link.mic_device = e.target.value; saveAudio(); };
$("link-cable").onchange = (e) => { audio.mic_link.cable_device = e.target.value; saveAudio(); };
$("link-mic-vol").oninput = (e) => { audio.mic_link.mic_volume = +e.target.value; saveAudio(); };
$("link-voice-vol").oninput = (e) => { audio.mic_link.voice_volume = +e.target.value; saveAudio(); };
$("link-blips").onchange = (e) => { audio.mic_link.include_blips = e.target.checked; saveAudio(); };
$("link-music").onchange = (e) => { audio.mic_link.include_music = e.target.checked; saveAudio(); };
function nsLabels() {
  $("ns-strength-val").textContent = Math.round($("ns-strength").value * 100) + "%";
  $("ns-gate-val").textContent = +$("ns-gate").value === 0 ? t("spenta") : Math.round($("ns-gate").value * 100) + "%";
}
$("ns-enabled").onchange = (e) => { audio.mic_link.noise_suppression = e.target.checked; saveAudio(); };
$("ns-strength").oninput = (e) => { audio.mic_link.ns_strength = +e.target.value; nsLabels(); saveAudio(); };
$("ns-gate").oninput = (e) => { audio.mic_link.ns_gate = +e.target.value; nsLabels(); saveAudio(); };

// ---------- musica ----------
function fillMusic(cfg) {
  $("music-lyrics").checked = cfg.music.lyrics;
  $("music-reactive").checked = cfg.music.reactive_sprite;
  $("music-compact").checked = cfg.music.compact !== false;
  $("music-sprite").checked = cfg.music.music_sprite !== false;
}
const saveMusic = () => invoke("music_settings", { lyrics: $("music-lyrics").checked, reactiveSprite: $("music-reactive").checked, compact: $("music-compact").checked, musicSprite: $("music-sprite").checked });
$("music-sprite").onchange = saveMusic;
$("open-venue").onclick = () => invoke("venue_open");
$("music-lyrics").onchange = saveMusic;
$("music-compact").onchange = saveMusic;
$("music-reactive").onchange = saveMusic;
$("music-go").onclick = async () => {
  const q = $("music-query").value.trim();
  const r = $("music-result");
  if (!q) return;
  r.textContent = "cerco… (la prima volta scarica yt-dlp)"; r.className = "result";
  try { r.textContent = await invoke("music_play", { query: q }); r.className = "result ok"; }
  catch (e) { r.textContent = String(e); r.className = "result bad"; }
};

// ---------- costi ----------
let prices = [];
let costRows = [];
const usd = (v) => (v === 0 ? "$0" : v < 0.01 ? "$" + v.toFixed(5) : "$" + v.toFixed(v < 1 ? 4 : 2));
const num = (n) => (n || 0).toLocaleString(I18N.lang());
function rowModel(r) {
  if (r.kind === "ricerca web" || r.kind === "pagina web") return "DeepSeek " + r.model;
  return `${r.provider}${r.model ? " / " + r.model : ""}`;
}
function renderCosts() {
  const filter = $("cost-filter").value;
  const now = Date.now() / 1000;
  const day = new Date(); day.setHours(0, 0, 0, 0);
  const month = new Date(day.getFullYear(), day.getMonth(), 1);
  const sum = (pred) => costRows.filter(pred).reduce((a, r) => a + r.cost, 0);
  const count = (pred) => costRows.filter(pred).length;
  const cards = [
    ["Oggi", sum((r) => r.ts * 1000 >= day), count((r) => r.ts * 1000 >= day)],
    ["Ultimi 7 giorni", sum((r) => r.ts >= now - 7 * 86400), count((r) => r.ts >= now - 7 * 86400)],
    ["Questo mese", sum((r) => r.ts * 1000 >= month), count((r) => r.ts * 1000 >= month)],
    ["Totale registrato", sum(() => true), costRows.length],
  ];
  const chat = costRows.filter((r) => r.input + r.cached > 0);
  const inTok = chat.reduce((a, r) => a + r.input + r.cached, 0);
  const cached = chat.reduce((a, r) => a + r.cached, 0);
  cards.push(["Input dalla cache", null, inTok ? Math.round((cached / inTok) * 100) + "%" : "—"]);
  $("cost-cards").innerHTML = cards.map(([label, v, n]) => v === null
    ? `<div class="card"><small>${t(label)}</small><b>${n}</b><small>${t("{a} di {b} token", { a: num(cached), b: num(inTok) })}</small></div>`
    : `<div class="card"><small>${t(label)}</small><b>${usd(v)}</b><small>${t("{n} richieste", { n: num(n) })}</small></div>`).join("");

  // per servizio e modello
  const groups = new Map();
  for (const r of costRows) {
    const k = r.provider + "\u0000" + (r.kind.includes("web") ? r.model : r.model);
    const g = groups.get(k) || { provider: r.provider, model: r.model, n: 0, input: 0, cached: 0, output: 0, cost: 0 };
    g.n++; g.input += r.input; g.cached += r.cached; g.output += r.output; g.cost += r.cost;
    groups.set(k, g);
  }
  const tb = document.querySelector("#cost-by tbody");
  const gs = [...groups.values()].sort((a, b) => b.cost - a.cost);
  tb.innerHTML = gs.length ? gs.map((g) => `<tr><td>${esc(g.provider)}</td><td>${esc(g.model || "—")}</td><td class="num">${num(g.n)}</td><td class="num">${num(g.input)}</td><td class="num">${num(g.cached)}</td><td class="num">${num(g.output)}</td><td class="num">${usd(g.cost)}</td></tr>`).join("")
    : '<tr><td colspan="7" class="note">nessuna richiesta registrata</td></tr>';

  // elenco richieste
  const rows = costRows.filter((r) => !filter || r.kind === filter).slice(0, 500);
  const body = document.querySelector("#cost-table tbody");
  body.innerHTML = rows.map((r) => {
    const tokIn = r.kind === "trascrizione" ? t("{s} s audio", { s: r.audio_secs.toFixed(0) }) : r.calls ? `${r.calls}×` : num(r.input);
    const d = new Date(r.ts * 1000);
    const when = d.toLocaleString(I18N.lang(), { day: "2-digit", month: "2-digit", hour: "2-digit", minute: "2-digit" });
    return `<tr><td>${when}</td><td>${esc(r.kind)}</td><td>${esc(rowModel(r))}</td><td class="num">${tokIn}</td><td class="num">${r.cached ? num(r.cached) : ""}</td><td class="num">${r.output ? num(r.output) : ""}</td><td class="num">${r.ms ? (r.ms / 1000).toFixed(1) + " s" : ""}</td><td class="num">${usd(r.cost)}${r.reported_cost != null ? " ✓" : ""}</td></tr>`;
  }).join("") || '<tr><td colspan="8" class="note">nessuna richiesta</td></tr>';
}
async function refreshCosts() {
  try { costRows = await invoke("costs_list", { limit: 20000 }); } catch (e) { $("cost-result").textContent = String(e); return; }
  renderCosts();
}
function fillPrices(cfg) {
  prices = structuredClone(cfg.prices || []);
  const tb = document.querySelector("#price-table tbody");
  tb.innerHTML = "";
  prices.forEach((p, i) => {
    const tr = document.createElement("tr");
    const cell = (key, type = "number") => {
      const td = document.createElement("td");
      const inp = document.createElement("input");
      inp.type = type; inp.value = p[key]; if (type === "number") { inp.step = "any"; inp.min = "0"; }
      inp.oninput = () => { p[key] = type === "number" ? +inp.value || 0 : inp.value.trim(); savePrices(); };
      td.append(inp); return td;
    };
    tr.append(cell("provider", "text"), cell("model", "text"), cell("input"), cell("cached"), cell("output"), cell("per_call"), cell("per_hour"));
    const td = document.createElement("td");
    const del = document.createElement("button"); del.textContent = "✕";
    del.onclick = () => { prices.splice(i, 1); savePrices(true); };
    td.append(del); tr.append(td);
    tb.append(tr);
  });
}
let priceTimer = null;
function savePrices(redraw = false) {
  clearTimeout(priceTimer);
  priceTimer = setTimeout(async () => { await invoke("set_prices", { prices }); refreshCosts(); }, 400);
  if (redraw) fillPrices({ prices });
}
$("price-add").onclick = () => { prices.push({ provider: "", model: "", input: 0, cached: 0, output: 0, per_call: 0, per_hour: 0 }); fillPrices({ prices }); };
$("price-reset").onclick = async () => { if (confirm(t("Ripristinare i prezzi predefiniti?"))) { await invoke("reset_prices"); fillPrices(await invoke("get_config")); refreshCosts(); } };
$("cost-filter").onchange = renderCosts;
$("cost-refresh").onclick = refreshCosts;
$("cost-export").onclick = async () => {
  const r = $("cost-result");
  try { r.textContent = t("Salvato: {path}", { path: await invoke("costs_export") }); r.className = "result ok"; } catch (e) { r.textContent = String(e); r.className = "result bad"; }
};
$("cost-clear").onclick = async () => { if (confirm(t("Svuotare il registro dei costi?"))) { await invoke("costs_clear"); refreshCosts(); } };
setInterval(() => { if (!document.hidden && localStorage.getItem("tab") === "costs") refreshCosts(); }, 5000);
$("vb-download").onclick = () => invoke("open_vbcable_page").catch((e) => alert(e));
$("link-rename").onclick = async () => {
  const d = $("link-driver");
  d.textContent = "Conferma la richiesta di Windows (amministratore)…"; d.className = "result";
  try { d.textContent = await invoke("mic_link_rename"); d.className = "result ok"; setTimeout(refreshDevices, 1000); }
  catch (e) { d.textContent = String(e); d.className = "result bad"; }
};
$("tts-key-save").onclick = async () => {
  try { await invoke("set_tts_key", { key: $("tts-key").value }); $("tts-key").value = ""; $("tts-result").textContent = "chiave salvata"; $("tts-result").className = "result ok"; }
  catch (e) { $("tts-result").textContent = String(e); $("tts-result").className = "result bad"; }
};
$("tts-test").onclick = async () => {
  const r = $("tts-result");
  clearTimeout(audioTimer);
  await invoke("set_audio", { audio });
  if (audio.tts === "off") { r.textContent = "la voce è spenta"; r.className = "result"; return; }
  r.textContent = audio.tts === "kokoro" ? "sintetizzo… (la prima volta Kokoro carica il modello)" : "sintetizzo…";
  r.className = "result";
  try {
    await invoke("tts_say", { text: "Ciao! Sono Alter Ego. Così suona la mia voce.", lang: "it" });
    r.textContent = "ok"; r.className = "result ok";
  } catch (e) { r.textContent = String(e); r.className = "result bad"; }
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
  for (let i = 3; i > 0; i--) { r.textContent = t("torna al gioco… {i}", { i }); r.className = "result"; await new Promise((ok) => setTimeout(ok, 1000)); }
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
      const tgt = Object.assign(document.createElement("input"), { value: item.target, placeholder: cols[1] });
      n.style.flex = "1"; tgt.style.flex = "2";
      n.oninput = () => { item.name = n.value.trim(); savePerms(); };
      tgt.oninput = () => { item.target = tgt.value.trim(); savePerms(); };
      row.append(n, tgt);
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
  $("auto-approve").checked = !!perms.auto_approve;
  $("auto-approve").onchange = (e) => {
    if (e.target.checked && !confirm(t("Alter Ego eseguirà le azioni attivate senza chiederti conferma. Continuare?"))) { e.target.checked = false; return; }
    perms.auto_approve = e.target.checked; savePerms();
  };
  document.querySelectorAll("input[data-perm]").forEach((c) => {
    c.checked = !!perms[c.dataset.perm];
    c.onchange = () => { perms[c.dataset.perm] = c.checked; savePerms(); };
  });
  document.querySelectorAll(".list[data-list]").forEach(renderList);
}

// ---------- ragionamento e risparmio token ----------
$("thinking").onchange = (e) => { ai.thinking = e.target.value; saveAi(); };
let saving = null;
let savingTimer = null;
function saveSaving() { clearTimeout(savingTimer); savingTimer = setTimeout(() => invoke("set_saving", { saving }), 250); }
const SAVING_DEFAULT = { tools_on_demand: false, compact_tools: false, trim_history: true, facts_limit: 0, screenshot_width: 1280, page_chars: 8000 };
const SAVING_MAX = { tools_on_demand: true, compact_tools: true, trim_history: true, facts_limit: 10, screenshot_width: 1024, page_chars: 5000 };
function fillSaving(cfg) {
  saving = structuredClone(cfg.saving);
  const eco = $("sv-eco-provider");
  if (eco.options.length <= 1) {
    for (const [id, label] of [["gemini", "Google Gemini"], ["openrouter", "OpenRouter"], ["groq", "Groq"], ["deepseek", "DeepSeek"], ["ollama", "Ollama (locale)"]]) {
      eco.append(Object.assign(document.createElement("option"), { value: id, textContent: label }));
    }
  }
  $("sv-tools").checked = saving.tools_on_demand;
  $("sv-compact").checked = saving.compact_tools;
  $("sv-trim").checked = saving.trim_history;
  $("sv-facts").value = String(saving.facts_limit);
  $("sv-shot").value = String(saving.screenshot_width);
  $("sv-page").value = String(saving.page_chars);
  $("sv-router").value = saving.router;
  $("sv-rizzo-url").value = saving.router_url;
  $("sv-eco-provider").value = saving.economy_provider;
  $("sv-eco-model").value = saving.economy_model;
  $("sv-rizzo-row").style.display = $("sv-rizzo-note").style.display = saving.router === "rizzo" ? "" : "none";
}
$("sv-tools").onchange = (e) => { saving.tools_on_demand = e.target.checked; saveSaving(); };
$("sv-compact").onchange = (e) => { saving.compact_tools = e.target.checked; saveSaving(); };
$("sv-trim").onchange = (e) => { saving.trim_history = e.target.checked; saveSaving(); };
$("sv-facts").onchange = (e) => { saving.facts_limit = +e.target.value; saveSaving(); };
$("sv-shot").onchange = (e) => { saving.screenshot_width = +e.target.value; saveSaving(); };
$("sv-page").onchange = (e) => { saving.page_chars = +e.target.value; saveSaving(); };
$("sv-router").onchange = (e) => { saving.router = e.target.value; $("sv-rizzo-row").style.display = $("sv-rizzo-note").style.display = saving.router === "rizzo" ? "" : "none"; saveSaving(); };
$("sv-rizzo-url").oninput = (e) => { saving.router_url = e.target.value.trim(); saveSaving(); };
$("sv-eco-provider").onchange = (e) => { saving.economy_provider = e.target.value; saveSaving(); };
$("sv-eco-model").oninput = (e) => { saving.economy_model = e.target.value.trim(); saveSaving(); };
$("sv-rizzo-link").onclick = (e) => { e.preventDefault(); invoke("open_link_cmd", { which: "rizzo" }); };
$("sv-rizzo-test").onclick = async (e) => {
  e.preventDefault();
  const r = $("sv-rizzo-result");
  r.className = "result"; r.textContent = t("Controllo…");
  try { r.textContent = await invoke("rizzo_test", { url: saving.router_url }); r.className = "result ok"; }
  catch { r.textContent = t("Non raggiungibile: avvialo con «uv run rizzo serve»"); r.className = "result bad"; }
};
function applySavingPreset(p, thinking) {
  Object.assign(saving, p);
  ai.thinking = thinking; $("thinking").value = thinking; saveAi();
  fillSaving({ saving }); saveSaving();
}
$("saving-max").onclick = () => applySavingPreset(SAVING_MAX, "off");
$("saving-default").onclick = () => applySavingPreset(SAVING_DEFAULT, "hide");

// ---------- ricerca web ----------
let web = null;
let webTimer = null;
function saveWeb() { clearTimeout(webTimer); webTimer = setTimeout(() => invoke("set_web_config", { web }), 250); }
async function webKeyStatus() {
  let k = {};
  try { k = (await invoke("web_keys")) || {}; } catch {}
  for (const p of ["tavily", "brave"]) { const el = $(`web-${p}-st`); el.textContent = k[p] ? t("chiave salvata ✓") : ""; el.className = "result ok"; }
}
function webRows() {
  $("web-gemini-row").style.display = web.provider === "gemini" ? "" : "none";
  $("web-tavily-row").style.display = web.provider === "tavily" ? "" : "none";
  $("web-brave-row").style.display = web.provider === "brave" ? "" : "none";
}
function fillWeb(cfg) {
  web = structuredClone(cfg.web);
  $("web-provider").value = web.provider;
  $("web-fallback").checked = web.fallback;
  $("web-fetch").value = web.fetch;
  $("web-gemini-model").value = web.gemini_model;
  webRows();
  webKeyStatus();
}
$("web-provider").onchange = (e) => { web.provider = e.target.value; webRows(); saveWeb(); };
$("web-fallback").onchange = (e) => { web.fallback = e.target.checked; saveWeb(); };
$("web-fetch").onchange = (e) => { web.fetch = e.target.value; saveWeb(); };
$("web-gemini-model").oninput = (e) => { web.gemini_model = e.target.value.trim(); saveWeb(); };
for (const p of ["tavily", "brave"]) {
  $(`web-${p}-save`).onclick = async (e) => {
    e.preventDefault();
    try { await invoke("set_web_key", { provider: p, key: $(`web-${p}-key`).value }); $(`web-${p}-key`).value = ""; webKeyStatus(); }
    catch (err) { const el = $(`web-${p}-st`); el.textContent = String(err); el.className = "result bad"; }
  };
}

// ---------- Spotify ----------
async function spotifyStatus() {
  const el = $("sp-status");
  try {
    const s = await invoke("spotify_status");
    el.textContent = s.connected ? t("Collegato: {name}", { name: s.account }) : t("Non collegato");
    el.className = s.connected ? "result ok" : "result";
    $("sp-redirect").textContent = s.redirect;
  } catch (e) { el.textContent = String(e); el.className = "result bad"; }
}
function fillSpotify(cfg) {
  $("sp-client").value = cfg.spotify.client_id || "";
  $("sp-enabled").checked = !!cfg.spotify.enabled;
  spotifyStatus();
}
$("sp-dev").onclick = (e) => { e.preventDefault(); invoke("open_link_cmd", { which: "spotify_dev" }); };
$("sp-connect").onclick = async (e) => {
  const el = $("sp-status");
  e.target.disabled = true;
  el.className = "result"; el.textContent = t("Si apre il browser: accedi a Spotify e consenti l'accesso…");
  try { const name = await invoke("spotify_connect", { clientId: $("sp-client").value }); el.textContent = t("Collegato: {name}", { name }); el.className = "result ok"; }
  catch (err) { el.textContent = String(err); el.className = "result bad"; }
  e.target.disabled = false;
};
$("sp-disconnect").onclick = async () => { await invoke("spotify_disconnect"); spotifyStatus(); };
$("sp-enabled").onchange = (e) => invoke("spotify_settings", { enabled: e.target.checked });

// ---------- alter net ----------
async function alterNetInfo() {
  try {
    const i = (await invoke("alternet_info")) || {};
    $("an-server").textContent = i.server || "";
    $("an-offline").textContent = i.offline ? t("copia offline presente") : t("copia offline non installata");
  } catch (_) {}
}
alterNetInfo();
$("an-open").onclick = () => invoke("alternet_open", { change: false }).catch(() => {});
$("an-change").onclick = () => invoke("alternet_open", { change: true }).catch(() => {});

// ---------- info e aggiornamenti ----------
invoke("app_version").then((v) => { $("app-version").textContent = v; }).catch(() => {});
$("link-site").onclick = () => invoke("open_link_cmd", { which: "site" });
$("link-github").onclick = () => invoke("open_link_cmd", { which: "github" });
function updResult(text, bad = false) { $("update-result").textContent = text; $("update-result").className = bad ? "result bad" : "result"; }
$("update-check").onclick = async () => {
  updResult(t("Controllo…"));
  try {
    const i = await invoke("update_check");
    $("update-install").hidden = !i.newer;
    updResult(i.newer ? t("Disponibile la versione {v} (hai la {c}).", { v: i.latest, c: i.current }) : t("Sei all'ultima versione ({c}).", { c: i.current }));
  } catch (e) { updResult(String(e), true); }
};
$("update-install").onclick = async () => {
  try { updResult(await invoke("update_install")); } catch (e) { updResult(String(e), true); }
};
listen("update-status", (e) => { if (e.payload) updResult(e.payload); });

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
$("log-clear").onclick = async () => { if (confirm(t("Svuotare il registro delle azioni?"))) { await invoke("clear_actions_log"); refreshLog(); } };

(async () => {
  const cfg = await invoke("get_config");
  applyLang(cfg);
  fillLook(cfg);
  fillAudio(cfg);
  fillHotkeys(cfg);
  fillClip(cfg);
  fillPerms(cfg);
  fillMusic(cfg);
  fillPrices(cfg);
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
  $("thinking").value = ai.thinking || "hide";
  fillSaving(cfg);
  fillWeb(cfg);
  fillSpotify(cfg);
  await reloadInfos();
  // aggiornamenti da altre parti dell'app (non sovrascrive ciò che si sta scrivendo)
  listen("config", (e) => {
    if (["INPUT", "TEXTAREA", "SELECT"].includes(document.activeElement?.tagName)) return;
    applyLang(e.payload);
    fillGeneral(e.payload);
    fillLook(e.payload);
  });
})();
