// Modulo aperto dal backend (form.rs) quando Alter Ego chiede un'informazione all'utente.
// I valori tornano solo al backend: il modello AI non vede le password.
const { invoke } = window.__TAURI__.core;
const $ = (id) => document.getElementById(id);
let spec = null;

function build() {
  $("title").textContent = " · " + spec.title;
  document.title = spec.title;
  $("intro").textContent = spec.intro || "";
  const form = $("form");
  form.innerHTML = "";
  for (const f of spec.fields) {
    const label = document.createElement("label");
    let input;
    if (f.type === "select") {
      input = document.createElement("select");
      for (const o of f.options || []) {
        const opt = document.createElement("option");
        opt.value = o.value; opt.textContent = o.label;
        input.append(opt);
      }
      input.value = f.value ?? "";
    } else {
      input = document.createElement("input");
      input.type = f.type === "checkbox" ? "checkbox" : f.type === "password" ? "password" : f.type === "number" ? "number" : "text";
      if (f.type === "checkbox") input.checked = !!f.value; else input.value = f.value ?? "";
      if (f.placeholder) input.placeholder = f.placeholder;
    }
    input.name = f.name;
    input.dataset.kind = f.type;
    if (f.type === "checkbox") { label.className = "inline"; label.append(input, Object.assign(document.createElement("span"), { textContent: f.label })); }
    else { const span = document.createElement("span"); span.textContent = f.label; label.append(span, input); }
    form.append(label);
  }
  // l'utente con l'accesso di Windows non deve scrivere utente e password
  const auth = form.querySelector("[name=auth]");
  const sync = () => {
    const win = auth && auth.value === "windows";
    for (const n of ["user", "password", "remember"]) {
      const el = form.querySelector(`[name=${n}]`);
      if (el) el.closest("label").style.display = win ? "none" : "";
    }
  };
  if (auth) { auth.onchange = sync; sync(); }
  const first = [...form.querySelectorAll("input:not([type=checkbox])")].find((i) => !i.value) || form.querySelector("input");
  setTimeout(() => first?.focus(), 50);
}

$("form").onsubmit = (e) => {
  e.preventDefault();
  const values = {};
  for (const el of $("form").elements) {
    if (!el.name) continue;
    values[el.name] = el.dataset.kind === "checkbox" ? el.checked : el.dataset.kind === "number" ? Number(el.value) : el.value;
  }
  invoke("form_submit", { id: spec.id, values });
};
$("cancel").onclick = () => invoke("form_submit", { id: spec.id, values: null });
window.addEventListener("keydown", (e) => { if (e.key === "Escape") $("cancel").click(); });

(async () => {
  try { const cfg = await invoke("get_config"); I18N.setLang(I18N.resolve(cfg.ui_language)); } catch {}
  spec = await invoke("form_spec");
  if (!spec) { window.__TAURI__.window.getCurrentWindow().close(); return; }
  build();
})();
