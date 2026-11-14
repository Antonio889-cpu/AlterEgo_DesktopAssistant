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
  if (e.key === "Escape" && Chat.cancel()) return; // ferma la risposta in corso
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
// frasi dal backend (es. aggiornamento avviato dalla tray): "[emozione]\ntesto"
listen("app-say", async (e) => {
  const r = Chat.parseReply(e.payload);
  State.setEmotion(r.emotion);
  State.setMode("speaking");
  Audio2.speak(r.it || r.en);
  await Dialog.say(r.it || r.en);
  State.settle();
});
listen("update-status", (e) => { if (e.payload) Chat.notify(e.payload); });
listen("reminder", async (e) => {
  Audio2.chime();
  State.setEmotion("surprised");
  State.setMode("speaking");
  Audio2.speak(e.payload.text);
  await Dialog.say(t("⏰ Promemoria: {text}", { text: e.payload.text }));
  State.settle();
});

// ---------- Garry's Mod / Shinri Trial: reazioni ai suoni del gioco (vedi gmod.rs) ----------
const GMOD_LINES = {
  body: [["worried", "Un corpo è stato scoperto… Fai attenzione, per favore. Da adesso ogni dettaglio conta."],
         ["sad", "Un altro corpo… Non doveva succedere. Aiutami a scoprire la verità."]],
  check_body: [["thinking", "Stai esaminando il corpo… Guarda le ferite, la posizione e cosa c'è intorno."]],
  investigation: [["thinking", "È iniziata l'indagine. Raccogli più prove che puoi, e se hai le mani sporche… lavale subito."]],
  game_start: [["worried", "La partita è cominciata… Spero che nessuno ceda alla disperazione."]],
  trial: [["thinking", "Il processo di classe sta per iniziare. Usa le prove che hai raccolto… io credo in te!"]],
  voting: [["worried", "È il momento di votare… Pensaci bene. Una scelta sbagliata condanna tutti."]],
  verdict_true: [["sad", "Avete trovato il colpevole… È la verità, anche se fa male."]],
  verdict_false: [["surprised", "Il verdetto era sbagliato…! No… il colpevole l'ha fatta franca."]],
  end_peaceful: [["happy", "È finita senza vittime… Sono così sollevata! Ce l'avete fatta tutti insieme."]],
  end_failure: [["sad", "È finita male… Mi dispiace tanto. La prossima volta andrà meglio, vero?"]],
  blackout: [["worried", "Blackout…! Resta vicino a qualcuno di cui ti fidi."]],
  disguise: [["worried", "Travestimenti… adesso nessuno è chi sembra. Non fidarti dell'aspetto."]],
  teleport: [["surprised", "Teletrasporto?! Guardati intorno, potresti essere ovunque…"]],
  killer: [["sad", "Anche tu… alla fine hai ceduto alla disperazione…"],
           ["sad", "Perché…? Speravo che la speranza vincesse… e invece anche tu hai ceduto alla disperazione."],
           ["sad", "Non posso crederci… Hai davvero scelto la disperazione? Io… non so cosa dire."]],
  hurt: [["surprised", "Ti hanno colpito! Stai bene? Allontanati, presto!"]],
  death: [["sad", "No… non può essere… Ti prego, dimmi che è solo un gioco…"]],
  low_sleep: [["worried", "Hai bisogno di dormire… Riposa un po', per favore."]],
};
listen("gmod", async (e) => {
  const opts = GMOD_LINES[e.payload.event];
  if (!opts) return;
  const [emotion, text] = opts[Math.floor(Math.random() * opts.length)];
  const line = t(text);
  State.setEmotion(emotion);
  State.setMode("speaking");
  Audio2.speak(line);
  await Dialog.say(line);
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
// otturatore: si chiude prima dello screenshot e si riapre dopo
listen("shutter", (e) => {
  const s = document.getElementById("shutter");
  s.className = "";
  void s.offsetWidth;
  s.className = e.payload;
  if (e.payload === "open") setTimeout(() => (s.className = ""), 320);
});

// ---------- modalità compatta durante la musica ----------
// Mentre suona un brano la finestra si rimpicciolisce; torna normale quando Alter Ego parla,
// pensa, ascolta o quando apri il box di testo / una conferma.
let musicOn = false;
let compact = false;
let wantSince = 0;
listen("music-state", (e) => { musicOn = !!(e.payload.playing && !e.payload.paused); });
function alterEgoBusy() {
  return (typeof Extras !== "undefined" && Extras.active()) || Dialog.isOpen() || !$("prompt").classList.contains("hidden") || !$("confirm").classList.contains("hidden")
    || ["thinking", "listening", "speaking"].includes(State.mode()) || Audio2.isSpeaking();
}
setInterval(() => {
  // cuffie: musica in corso e nessuna domanda/risposta in corso
  Face.setMusic(musicOn && config?.music?.music_sprite !== false && !alterEgoBusy());
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

// ---------- pet: Alter Ego esce dalla TV e gira per il desktop (vedi pet.js e pet/mod.rs) ----------
// Esce nei momenti di calma, a caso; rientra quando le parli, quando deve parlare lei, con
// Mostra/nascondi, con un doppio click sul pet o dopo un po' (Impostazioni → Pet).
const PET_CHANCE = { rare: 0.15, normal: 0.35, often: 0.7 }; // probabilità a ogni minuto di calma
let petOut = false;
let petStarting = false;
let lastActive = Date.now();
let lastRoll = Date.now();
const markActive = () => { lastActive = Date.now(); };
window.addEventListener("pointerdown", markActive, true);
window.addEventListener("keydown", markActive, true);
listen("pet", (e) => { petOut = !!e.payload.active; markActive(); });
function petHome() {
  if (!petOut) return;
  petOut = false;
  invoke("pet_home");
}
// le parli (push-to-talk, box di testo) o parla lei (promemoria, frasi dal backend): rientra subito
document.addEventListener("ae-mode", (e) => {
  if (["listening", "thinking", "speaking"].includes(e.detail)) { markActive(); petHome(); }
  // parla (promemoria, avvisi, frasi dal backend): se era nascosta o ridotta a icona ricompare
  if (e.detail === "speaking") invoke("reveal_main").catch(() => {});
});
async function petExplore(manual = false) {
  if (petOut || petStarting) return;
  if (!manual && (!config?.pet?.enabled || alterEgoBusy() || musicOn)) return;
  petStarting = true;
  try {
    if (!(await invoke("pet_can_explore"))) return;
    // la TV si spegne ed esce lei
    petOut = await invoke("pet_explore");
  } catch (err) {
    console.warn("pet:", err);
  } finally {
    petStarting = false;
  }
}
listen("pet-go", () => petExplore(true));
setInterval(async () => {
  if (alterEgoBusy()) { markActive(); petHome(); return; }
  const p = config?.pet;
  if (!p?.enabled || petOut || petStarting) return;
  const calm = Date.now() - lastActive;
  if (calm < p.idle_minutes * 60000 || Date.now() - lastRoll < 60000) return;
  lastRoll = Date.now();
  let visible = false;
  try { visible = await appWindow.isVisible(); } catch {}
  if (visible && Math.random() < (PET_CHANCE[p.frequency] ?? 0.35)) petExplore();
}, 5000);

window.addEventListener("contextmenu", (e) => e.preventDefault());

(async () => {
  applyConfig(await invoke("get_config"));
  await document.fonts.load('5vh "DotGothic16"');
  // avvio silenzioso: finestra nascosta, niente saluto (né blip) finché non la apri
  let visible = true;
  try { visible = await appWindow.isVisible(); } catch {}
  if (!visible) return;
  tvOn();
  const name = config.user_name.trim();
  State.setEmotion("happy");
  State.setMode("speaking");
  await Dialog.say(t("Ciao{name}! Scrivimi con Ctrl+Alt+T, oppure tieni premuto Ctrl+Alt+Spazio e parla.", { name: name ? ", " + name : "" }));
  State.settle();
  // primo avvio: proposta di scaricare i contenuti aggiuntivi (una volta sola)
  Extras.askFirstRun();
})();
