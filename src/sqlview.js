// Finestra SQL: risultati delle query di Alter Ego e console per le query dell'utente.
const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;
const $ = (id) => document.getElementById(id);
let last = null;

function render(r) {
  if (last && JSON.stringify(last) === JSON.stringify(r)) return; // già mostrato
  last = r;
  $("error").textContent = r.error ? r.error : "";
  const by = r.by === "tu" ? t("tu") : "Alter Ego";
  $("meta").innerHTML = "";
  const who = Object.assign(document.createElement("span"), { className: "by", textContent: by });
  const info = r.error ? "" : r.columns.length
    ? t("{n} righe{more} · {ms} ms", { n: r.rows.length, more: r.truncated ? "+" : "", ms: r.ms })
    : t("{n} righe modificate · {ms} ms", { n: r.affected, ms: r.ms });
  $("meta").append(who, " · " + info);
  $("empty").hidden = r.columns.length > 0;
  if (!r.columns.length && !r.error) { $("empty").hidden = false; $("empty").textContent = t("{n} righe modificate.", { n: r.affected }); }
  const head = $("grid").querySelector("thead");
  const body = $("grid").querySelector("tbody");
  head.innerHTML = ""; body.innerHTML = "";
  if (r.columns.length) {
    const tr = document.createElement("tr");
    for (const c of r.columns) tr.append(Object.assign(document.createElement("th"), { textContent: c }));
    head.append(tr);
  }
  const frag = document.createDocumentFragment();
  for (const row of r.rows) {
    const tr = document.createElement("tr");
    for (const v of row) {
      const td = document.createElement("td");
      td.textContent = v;
      td.title = v.length > 60 ? v : "";
      if (v === "NULL") td.className = "null";
      tr.append(td);
    }
    frag.append(tr);
  }
  body.append(frag);
  addHistory(r);
}

function addHistory(r) {
  const d = document.createElement("div");
  d.textContent = (r.by === "tu" ? "› " : "◆ ") + r.query.replace(/\s+/g, " ");
  d.title = r.query;
  d.onclick = () => { $("query").value = r.query; $("query").focus(); };
  $("history").prepend(d);
  while ($("history").children.length > 50) $("history").lastChild.remove();
}

async function refresh() {
  const s = await invoke("sql_status");
  $("status").textContent = s.connected ? t("Connesso: {label}", { label: s.label }) : (s.read ? t("Non connesso") : t("Permesso «Database SQL Server» spento"));
  $("disconnect").disabled = !s.connected;
  $("run").disabled = !s.connected;
}

async function run() {
  const q = $("query").value.trim();
  if (!q) return;
  $("run").disabled = true;
  $("error").textContent = "";
  try { render(await invoke("sql_run_user", { query: q })); } catch (e) { $("error").textContent = String(e); }
  refresh();
}

$("run").onclick = run;
$("query").addEventListener("keydown", (e) => { if (e.key === "Enter" && e.ctrlKey) { e.preventDefault(); run(); } });
$("connect").onclick = async () => {
  $("error").textContent = "";
  try { await invoke("sql_connect_ui"); } catch (e) { $("error").textContent = String(e); }
  refresh();
};
$("disconnect").onclick = async () => { await invoke("sql_disconnect_ui"); refresh(); };
$("export").onclick = async () => {
  if (!last || !last.columns.length) return;
  const esc = (v) => /[;"\n]/.test(v) ? '"' + v.replace(/"/g, '""') + '"' : v;
  const csv = [last.columns, ...last.rows].map((r) => r.map(esc).join(";")).join("\n");
  try { await navigator.clipboard.writeText(csv); $("meta").append(" · " + t("copiato")); } catch (e) { $("error").textContent = String(e); }
};

listen("sql-result", (e) => { render(e.payload); refresh(); });
listen("sql-status", () => refresh());

(async () => {
  try { const cfg = await invoke("get_config"); I18N.setLang(I18N.resolve(cfg.ui_language)); } catch {}
  const prev = await invoke("sql_last");
  if (prev) render(prev);
  refresh();
})();
