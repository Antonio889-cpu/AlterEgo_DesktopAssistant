// Fase 1: drag, resize 16:9, opacità, click-through, eventi hotkey.
const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;
const appWindow = window.__TAURI__.window.getCurrentWindow();

const $ = (id) => document.getElementById(id);
const root = $("root");
const statusEl = $("status");

function setStatus(text) { statusEl.textContent = text; }
function applyOpacity(v) { root.style.opacity = v; }
function applyClickThrough(on) {
  document.body.classList.toggle("click-through", on);
  if (!on) document.body.classList.remove("handle-hover");
  setStatus(on ? "Click-through attivo – passa sul lucchetto ↗ per disattivarlo" : "Click-through disattivato");
}

// --- spostamento ---
$("screen").addEventListener("mousedown", (e) => {
  if (e.button === 0) appWindow.startDragging();
});

// --- ridimensionamento con maniglia (16:9 garantito da Rust) ---
const handle = $("resize-handle");
let resize = null;
let pendingWidth = null;
handle.addEventListener("pointerdown", async (e) => {
  e.stopPropagation();
  handle.setPointerCapture(e.pointerId);
  const size = await appWindow.innerSize();
  resize = { startX: e.screenX, startW: size.width };
});
handle.addEventListener("pointermove", (e) => {
  if (!resize) return;
  const dx = (e.screenX - resize.startX) * window.devicePixelRatio;
  const first = pendingWidth === null;
  pendingWidth = resize.startW + dx;
  if (first) requestAnimationFrame(() => {
    invoke("resize_window", { width: pendingWidth });
    pendingWidth = null;
  });
});
const endResize = () => { resize = null; };
handle.addEventListener("pointerup", endResize);
handle.addEventListener("pointercancel", endResize);

// --- Ctrl + rotella ---
window.addEventListener("wheel", async (e) => {
  if (!e.ctrlKey) return;
  e.preventDefault();
  const size = await appWindow.innerSize();
  invoke("resize_window", { width: size.width * (e.deltaY < 0 ? 1.06 : 1 / 1.06) });
}, { passive: false });

// --- maniglia click-through ---
$("ct-handle").addEventListener("click", () => invoke("set_click_through", { on: false }));

// --- eventi dal backend ---
listen("opacity", (e) => applyOpacity(e.payload));
listen("click-through", (e) => applyClickThrough(e.payload));
listen("handle-hover", (e) => document.body.classList.toggle("handle-hover", e.payload));
listen("ptt", (e) => setStatus(e.payload ? "🎙 Parla… (push-to-talk premuto)" : "Push-to-talk rilasciato"));
listen("text-input", () => setStatus("⌨ Input testo richiesto (arriva in fase 6)"));

// Blocca il menu contestuale del browser.
window.addEventListener("contextmenu", (e) => e.preventDefault());

(async () => {
  const cfg = await invoke("get_config");
  applyOpacity(cfg.opacity);
  document.body.classList.toggle("click-through", cfg.click_through);
  if (cfg.hotkey_errors.length) {
    $("errors").textContent = "Hotkey non registrate: " + cfg.hotkey_errors.join("; ");
  }
})();
