// Catalogo dei modelli locali (finestra separata): consigliati, ricerca su Ollama e Hugging Face, download.
const { invoke } = window.__TAURI__.core;
const { listen, emit } = window.__TAURI__.event;
const $ = (id) => document.getElementById(id);

(async () => {
  try { I18N.setLang(I18N.resolve((await invoke("get_config")).ui_language)); } catch {}
})();

const RECOMMENDED = [
  ["LFM3"],
  ["lfm3", "LFM3 1.7B · Q4, ~1,1 GB (consigliato)"],
  ["lfm3:1.7b-qad-q4_0", "LFM3 1.7B · Q4_0 QAD, ~1 GB (addestrato per la quantizzazione)"],
  ["lfm3:1.7b-q8_0", "LFM3 1.7B · Q8_0, ~1,8 GB (più preciso)"],
  ["hf.co/LiquidAI/LFM3-VL-2B-GGUF:Q4_K_M", "LFM3-VL 2B · vede gli screenshot, ~1,3 GB (sperimentale)"],
  ["Gemma 4"],
  ["gemma4:e2b", "Gemma 4 E2B · strumenti, visione, audio"],
  ["gemma4:e4b", "Gemma 4 E4B · strumenti, visione, audio"],
  ["gemma4:12b", "Gemma 4 12B · ~8 GB"],
  ["gemma4:26b-a4b", "Gemma 4 26B MoE · ~16 GB"],
  ["Gemma 3"],
  ["gemma3:1b", "Gemma 3 1B · solo chat, ~0,8 GB"],
  ["gemma3:4b", "Gemma 3 4B · chat e visione, niente strumenti, ~3,3 GB"],
  ["gemma3:12b", "Gemma 3 12B · chat e visione, niente strumenti, ~8 GB"],
  ["gemma3:27b", "Gemma 3 27B · chat e visione, niente strumenti, ~17 GB"],
];
{
  let group = null;
  for (const [v, l] of RECOMMENDED) {
    if (!l) { group = Object.assign(document.createElement("optgroup"), { label: v }); $("variant").append(group); continue; }
    group.append(Object.assign(document.createElement("option"), { value: v, textContent: t(l) }));
  }
}

const fmtSize = (b) => (b >= 1e9 ? (b / 1e9).toFixed(1) + " GB" : (b / 1e6).toFixed(0) + " MB");
function say(text, bad = false) { $("state").textContent = text; $("state").className = "status" + (bad ? " bad" : ""); }

let activePull = null;
listen("local-progress", ({ payload: p }) => {
  $("progress").hidden = false;
  const pct = p.total ? Math.round((p.completed / p.total) * 100) : 0;
  $("pbar").style.width = (p.total ? pct : 100) + "%";
  $("ptext").textContent = p.error ? "⚠ " + p.error : p.done ? "✓ " + (p.status === "success" ? t("{name} pronto", { name: p.name || "Ollama" }) : p.status)
    : `${p.name ? p.name + " · " : ""}${p.status}${p.total ? ` · ${fmtSize(p.completed)} / ${fmtSize(p.total)} (${pct}%)` : ""}`;
  $("ptext").className = p.error ? "bad" : "";
  activePull = p.task === "pull" && !p.done ? p.name : null;
  $("cancel").hidden = !activePull;
});
$("cancel").onclick = () => activePull && invoke("local_cancel", { name: activePull });

async function busy(btn, fn) {
  btn.disabled = true;
  try { await fn(); } catch (e) { say(String(e), true); }
  btn.disabled = false;
}

$("rec-go").onclick = (e) => busy(e.target, async () => {
  say(t("Preparo il modello… (installa Ollama se manca, poi lo scarica)"));
  say(await invoke("local_lfm3", { variant: $("variant").value, primary: $("primary").checked }));
  emit("local-used");
});
$("url-go").onclick = (e) => busy(e.target, async () => {
  const name = await invoke("local_pull", { name: $("url").value });
  say(t("Scaricato: {name}. Premi «Usa» nella tabella per usarlo.", { name }));
});

async function search() {
  const box = $("results");
  box.innerHTML = '<div class="empty">' + t("Cerco…") + "</div>";
  const chips = [...document.querySelectorAll('#chips input[data-kind="word"]:checked')].map((c) => c.value);
  const caps = [...document.querySelectorAll('#chips input[data-kind="cap"]:checked')].map((c) => c.value);
  let list;
  try {
    list = await invoke("local_search", { source: $("source").value, query: $("query").value, chips, caps, maxB: +$("size").value });
  } catch (e) { box.innerHTML = ""; box.append(Object.assign(document.createElement("div"), { className: "empty", textContent: String(e) })); return; }
  box.innerHTML = "";
  if (!list.length) { box.innerHTML = '<div class="empty">' + t("Nessun modello trovato con questi filtri.") + "</div>"; return; }
  for (const m of list) box.append(item(m));
}

function item(m) {
  const d = Object.assign(document.createElement("div"), { className: "item" });
  d.append(Object.assign(document.createElement("div"), { className: "nm", textContent: m.name, title: m.name }));
  const act = Object.assign(document.createElement("div"), { className: "act" });
  const sel = document.createElement("select");
  if (m.source === "huggingface") {
    // i file .gguf del repository: il backend ne ricava la quantizzazione (es. Q4_K_M)
    sel.append(Object.assign(document.createElement("option"), { value: "", textContent: t("quantizzazione predefinita (Q4_K_M)") }));
    sel.onfocus = async () => {
      if (sel.dataset.loaded) return;
      sel.dataset.loaded = "1";
      try {
        for (const [f, size] of await invoke("local_hf_files", { repo: m.name })) sel.append(Object.assign(document.createElement("option"), { value: f, textContent: `${f.split("/").pop()} · ${fmtSize(size)}`, title: f }));
      } catch {}
    };
    act.append(sel);
  } else if (m.sizes.length > 1) {
    for (const s of m.sizes) sel.append(Object.assign(document.createElement("option"), { value: s, textContent: s }));
    act.append(sel);
  }
  const b = Object.assign(document.createElement("button"), { textContent: t("⬇ Scarica") });
  b.onclick = () => busy(b, async () => {
    const name = await invoke("local_pull", { name: m.name + (sel.value ? ":" + sel.value : "") });
    say(t("Scaricato: {name}. Premi «Usa» nella tabella per usarlo.", { name }));
  });
  act.append(b);
  d.append(act);
  if (m.description) d.append(Object.assign(document.createElement("div"), { className: "ds", textContent: m.description }));
  const tg = Object.assign(document.createElement("div"), { className: "tg" });
  for (const x of [...m.capabilities, ...m.sizes]) tg.append(Object.assign(document.createElement("span"), { textContent: x }));
  if (m.downloads) tg.append(Object.assign(document.createElement("span"), { textContent: "⬇ " + m.downloads }));
  if (m.updated) tg.append(Object.assign(document.createElement("span"), { textContent: m.updated }));
  d.append(tg);
  return d;
}
$("search").onclick = search;
$("query").addEventListener("keydown", (e) => { if (e.key === "Enter") search(); });
$("source").onchange = search;
document.querySelectorAll("#chips input, #size").forEach((c) => (c.onchange = search));
