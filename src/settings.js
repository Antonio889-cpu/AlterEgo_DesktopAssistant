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


// ---------- sprite ----------
async function spriteStatus() {
  const sprites = await invoke("load_user_sprites");
  const n = Object.keys(sprites || {}).length;
  $("sprite-status").textContent = n ? `Sprite importati: ${Object.keys(sprites).join(", ")}` : "Nessuno sprite importato: Alter Ego usa il volto segnaposto (o quelli inclusi in una build privata).";
}
function spriteResult(t, cls = "") { const r = $("sprite-result"); r.textContent = t; r.className = "result " + cls; }
$("sprite-file").onchange = async (e) => {
  const f = e.target.files[0];
  if (!f) return;
  spriteResult("elaboro…");
  const data = await new Promise((ok, ko) => { const r = new FileReader(); r.onload = () => ok(r.result); r.onerror = ko; r.readAsDataURL(f); });
  try { const saved = await invoke("import_sprite_sheet", { data }); spriteResult(`importati ${saved.length} sprite ✓`, "ok"); }
  catch (err) { spriteResult(String(err), "bad"); }
  e.target.value = "";
  spriteStatus();
};
$("sprite-dir").onclick = () => invoke("open_sprite_dir").catch((e) => spriteResult(String(e), "bad"));
$("sprite-reload").onclick = async () => { await invoke("reload_sprites"); spriteStatus(); spriteResult("ricaricati", "ok"); };
$("sprite-remove").onclick = async () => {
  if (!confirm("Rimuovere gli sprite importati?")) return;
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
}

/** Token e cache dei provider da quando l'app è aperta (DeepSeek riporta la cache). */
async function refreshUsage() {
  const el = $("ai-usage");
  let u = {};
  try { u = await invoke("ai_usage"); } catch { return; }
  const parts = Object.entries(u).filter(([, v]) => v.requests).map(([id, v]) => {
    const pct = v.input ? Math.round((v.cached / v.input) * 100) : 0;
    return `${id}: ${v.requests} richieste, ${v.input.toLocaleString("it-IT")} token in ingresso` + (v.cached || id === "deepseek" ? ` (${pct}% dalla cache)` : "");
  });
  el.textContent = parts.length ? "Da quando Alter Ego è aperto — " + parts.join(" · ") : "";
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
  if (value && !list.includes(value)) opts.push(`<option value="${esc(value)}">${esc(value)} (non collegato)</option>`);
  sel.innerHTML = opts.join("");
  sel.value = value || "";
}
async function refreshDevices() {
  try { devices = await invoke("audio_devices"); } catch (e) { $("dev-status").textContent = String(e); return; }
  const outs = devices.outputs.filter((n) => n !== devices.cable_render);
  const ins = devices.inputs.filter((n) => n !== devices.cable_capture);
  fillDeviceSelect($("out-device"), outs, audio.output_device, `Predefinito di Windows${devices.default_output ? " (" + devices.default_output + ")" : ""}`);
  fillDeviceSelect($("in-device"), ins, audio.input_device, `Predefinito di Windows${devices.default_input ? " (" + devices.default_input + ")" : ""}`);
  fillDeviceSelect($("link-mic"), ins, audio.mic_link.mic_device, "Microfono predefinito di Windows");
  fillDeviceSelect($("link-cable"), devices.outputs, audio.mic_link.cable_device, `Automatico${devices.cable_render ? " (" + devices.cable_render + ")" : " (nessun cavo trovato)"}`);
  const d = $("link-driver");
  if (!devices.cable_render) { d.textContent = "VB-CABLE non trovato: installalo (punto 1) e riavvia il PC."; d.className = "result bad"; }
  else if (!devices.link_named) { d.textContent = `Cavo trovato: ${devices.cable_capture || devices.cable_render}. Rinominalo (punto 2) per vederlo come «AlterEgo Link Audio».`; d.className = "result"; }
  else { d.textContent = `Pronto: le altre app vedono «${devices.cable_capture}».`; d.className = "result ok"; }
  if (devices.default_output && devices.default_output === devices.cable_render) {
    d.textContent += " ⚠ Il cavo è l'uscita predefinita di Windows: rimetti i tuoi altoparlanti.";
    d.className = "result bad";
  }
}
async function refreshAudioStatus() {
  try {
    const st = await invoke("audio_status");
    const ds = $("dev-status");
    ds.textContent = st.output_error ? "Uscita: " + st.output_error : st.output ? "In uso: " + st.output : "";
    ds.className = st.output_error ? "result bad" : "result ok";
    const ls = $("link-status");
    if (!audio.mic_link.enabled) { ls.textContent = "Mic-Link spento"; ls.className = "result"; }
    else if (st.link_active) {
      ls.textContent = `Attivo: ${st.link_mic || "solo voce"} + voce → ${st.link_cable}` + (st.link_error ? ` (${st.link_error})` : "");
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
    else { el.textContent = st.running ? "Server avviato: " + st.message : "Installato, server fermo (" + st.message + ")"; el.className = "result"; }
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
  $("auto-approve").checked = !!perms.auto_approve;
  $("auto-approve").onchange = (e) => {
    if (e.target.checked && !confirm("Alter Ego eseguirà le azioni attivate senza chiederti conferma. Continuare?")) { e.target.checked = false; return; }
    perms.auto_approve = e.target.checked; savePerms();
  };
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
