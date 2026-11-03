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
  const lang = I18N.resolve(cfg.ui_language);
  if (lang !== I18N.lang() || !applyConfig.done) {
    I18N.setLang(lang);
    invoke("set_resolved_language", { lang }); // tray e messaggi del backend
    applyConfig.done = true;
  }
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
    ? t("Hotkey non registrate: {list}", { list: cfg.hotkey_errors.join("; ") })
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
  await Dialog.say(t("Ciao{name}! Sono Alter Ego... sono solo un programma di intelligenza artificiale, però farò del mio meglio.", { name: name ? ", " + name : "" }));
  await Dialog.say(t("Questo è un messaggio lungo per provare le pagine del box dialogo. Quando il testo non entra in due righe continua nella pagina successiva: clicca qui sopra, oppure premi Invio, per andare avanti."));
  await Dialog.say("あなたは何者？");
}

// ---------- spostamento ----------
$("screen").addEventListener("mousedown", (e) => {
  State.touch();
  if (e.button !== 0 || e.target.closest("#dialog, #prompt, #confirm")) return;
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
  if (confirmId !== null) {
    if (e.key === "Escape") answerConfirm(false);
    return; // Invio/Spazio agiscono sul pulsante con il focus
  }
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

// il volto "parla" mentre scorre il testo o mentre suona la voce
let typing = false;
const syncTalk = () => Face.setTalking(typing || Audio2.isSpeaking());
Dialog.on("talk", (on) => { typing = on; syncTalk(); });
Dialog.on("char", (ch) => Audio2.blip(ch));
Audio2.onSpeaking(syncTalk);

// ---------- conferme delle azioni PC ----------
let confirmId = null;
function answerConfirm(allow) {
  if (confirmId === null) return;
  invoke("confirm_action", { id: confirmId, allow });
  confirmId = null;
  $("confirm").classList.add("hidden");
}
$("confirm-yes").onclick = () => answerConfirm(true);
$("confirm-no").onclick = () => answerConfirm(false);
listen("confirm", (e) => {
  confirmId = e.payload.id;
  $("confirm-text").textContent = e.payload.text;
  $("confirm").classList.remove("hidden");
  State.setEmotion("worried");
  invoke("focus_main");
  setTimeout(() => $("confirm-no").focus(), 50); // default sicuro: Nega
});
listen("confirm-closed", (e) => {
  if (confirmId === e.payload) { confirmId = null; $("confirm").classList.add("hidden"); }
});
listen("tool-use", (e) => Chat.onToolUse(e.payload));
listen("reminder", async (e) => {
  Audio2.chime();
  State.setEmotion("surprised");
  State.setMode("speaking");
  Audio2.speak(e.payload.text);
  await Dialog.say(t("⏰ Promemoria: {text}", { text: e.payload.text }));
  State.settle();
});

// ---------- accensione / spegnimento "vecchia TV" (vedi window.rs) ----------
let tvTimer = null;
function tvOn() {
  clearTimeout(tvTimer);
  root.classList.remove("tv-off", "tv-off-anim", "tv-on-anim");
  if (!config?.tv_animation) return;
  void root.offsetWidth; // riavvia l'animazione
  root.classList.add("tv-on-anim");
  tvTimer = setTimeout(() => root.classList.remove("tv-on-anim"), 700);
}
function tvOff() {
  clearTimeout(tvTimer);
  Audio2.stop();
  root.classList.remove("tv-on-anim");
  root.classList.add("tv-off-anim");
  tvTimer = setTimeout(() => {
    root.classList.add("tv-off");
    root.classList.remove("tv-off-anim");
    invoke("tv_done");
  }, 470);
}
listen("tv", (e) => (e.payload === "on" ? tvOn() : tvOff()));

// ---------- modalità compatta durante la musica ----------
// Mentre suona un brano la finestra si rimpicciolisce; torna normale quando Alter Ego parla,
// pensa, ascolta o quando apri il box di testo / una conferma.
let musicOn = false;
let compact = false;
let wantSince = 0;
listen("music-state", (e) => { musicOn = !!(e.payload.playing && !e.payload.paused); });
function alterEgoBusy() {
  return Dialog.isOpen() || !$("prompt").classList.contains("hidden") || !$("confirm").classList.contains("hidden")
    || ["thinking", "listening", "speaking"].includes(State.mode()) || Audio2.isSpeaking();
}
setInterval(() => {
  const want = musicOn && config?.music?.compact !== false && !alterEgoBusy();
  if (!want) {
    wantSince = 0;
    if (compact) { compact = false; invoke("set_compact", { on: false }); }
    return;
  }
  // si rimpicciolisce dopo un attimo di calma, così non "salta" tra una frase e l'altra
  if (!wantSince) wantSince = Date.now();
  if (!compact && Date.now() - wantSince > 1500) { compact = true; invoke("set_compact", { on: true }); }
}, 250);

window.addEventListener("contextmenu", (e) => e.preventDefault());

(async () => {
  applyConfig(await invoke("get_config"));
  await document.fonts.load('5vh "DotGothic16"');
  tvOn();
  const name = config.user_name.trim();
  State.setEmotion("happy");
  State.setMode("speaking");
  await Dialog.say(t("Ciao{name}! Scrivimi con Ctrl+Alt+T, oppure tieni premuto Ctrl+Alt+Spazio e parla.", { name: name ? ", " + name : "" }));
  State.settle();
})();
