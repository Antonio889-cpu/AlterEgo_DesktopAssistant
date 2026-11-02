// Avvio: finestra (drag, resize 16:9, click-through), aspetto da config, eventi backend.
const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;
const appWindow = window.__TAURI__.window.getCurrentWindow();

const $ = (id) => document.getElementById(id);
const root = $("root");
let config = null;

// ---------- aspetto ----------
function applyConfig(cfg) {
  config = cfg;
  window.appConfig = cfg;
  root.style.opacity = cfg.opacity;
  document.documentElement.style.setProperty("--intensity", cfg.bg_intensity);
  const body = document.body;
  body.classList.remove("bg-scanline", "bg-circuit", "bg-both", "dialog-chiaro", "dialog-scuro");
  body.classList.add(`bg-${cfg.background}`, `dialog-${cfg.dialog_style}`);
  body.classList.toggle("fx", cfg.effects);
  body.classList.toggle("click-through", cfg.click_through);
  Dialog.setName(cfg.name_tag);
  $("errors").textContent = cfg.hotkey_errors.length
    ? "Hotkey non registrate: " + cfg.hotkey_errors.join("; ")
    : "";
}

function applyClickThrough(on) {
  document.body.classList.toggle("click-through", on);
  if (!on) document.body.classList.remove("handle-hover");
}

// ---------- messaggio di prova ----------
async function demo() {
  const name = config?.user_name?.trim();
  for (const e of ["happy", "embarrassed", "thinking", "worried", "surprised", "sad", "error", "idle"]) {
    State.setEmotion(e);
    await new Promise((r) => setTimeout(r, 450));
  }
  await Dialog.say(`Ciao${name ? ", " + name : ""}! Sono Alter Ego... sono solo un programma di intelligenza artificiale, però farò del mio meglio.`);
  await Dialog.say("Questo è un messaggio lungo per provare le pagine del box dialogo. Quando il testo non entra in due righe continua nella pagina successiva: clicca qui sopra, oppure premi Invio, per andare avanti.");
  await Dialog.say("あなたは何者？");
}

// ---------- spostamento ----------
$("screen").addEventListener("mousedown", (e) => {
  State.touch();
  if (e.button !== 0 || e.target.closest("#dialog, #prompt")) return;
  appWindow.startDragging();
});

// ---------- ridimensionamento (16:9 garantito da Rust) ----------
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
  const first = pendingWidth === null;
  pendingWidth = resize.startW + (e.screenX - resize.startX) * window.devicePixelRatio;
  if (first) requestAnimationFrame(() => {
    invoke("resize_window", { width: pendingWidth });
    pendingWidth = null;
  });
});
const endResize = () => { resize = null; };
handle.addEventListener("pointerup", endResize);
handle.addEventListener("pointercancel", endResize);

window.addEventListener("wheel", async (e) => {
  if (!e.ctrlKey) return;
  e.preventDefault();
  const size = await appWindow.innerSize();
  invoke("resize_window", { width: size.width * (e.deltaY < 0 ? 1.06 : 1 / 1.06) });
}, { passive: false });

// ---------- tastiera ----------
window.addEventListener("keydown", (e) => {
  State.touch();
  if (e.key === "Enter" || e.key === " ") {
    e.preventDefault();
    Dialog.advance();
  }
});

// ---------- click-through ----------
$("ct-handle").addEventListener("click", () => invoke("set_click_through", { on: false }));

// ---------- eventi dal backend ----------
listen("config", (e) => applyConfig(e.payload));
listen("click-through", (e) => applyClickThrough(e.payload));
listen("handle-hover", (e) => document.body.classList.toggle("handle-hover", e.payload));
listen("demo", () => demo());
listen("stt", (e) => Chat.onStt(e.payload));
listen("text-input", () => Chat.openInput());

Dialog.on("talk", (on) => Face.setTalking(on));

window.addEventListener("contextmenu", (e) => e.preventDefault());

(async () => {
  applyConfig(await invoke("get_config"));
  await document.fonts.load('5vh "DotGothic16"');
  const name = config.user_name.trim();
  State.setEmotion("happy");
  State.setMode("speaking");
  await Dialog.say(`Ciao${name ? ", " + name : ""}! Scrivimi con Ctrl+Alt+T, oppure tieni premuto Ctrl+Alt+Spazio e parla.`);
  State.settle();
})();
