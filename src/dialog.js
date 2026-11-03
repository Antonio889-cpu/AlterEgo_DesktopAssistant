// Box dialogo: typewriter, ▼ lampeggiante, testo spezzato in pagine da 2 righe.
// Uso: await Dialog.say("testo"); clic o Invio per proseguire.
// Se "auto_advance" è attivo, a pagina completa va avanti da solo dopo
// auto_advance_secs + il tempo di lettura (e aspetta che la voce finisca).
// "\n\n" nel testo forza una nuova pagina.

const Dialog = (() => {
  const box = document.getElementById("dialog");
  const tagEl = document.getElementById("dialog-tag");
  const textEl = document.getElementById("dialog-text");
  const measureEl = document.getElementById("dialog-measure");
  const measureTag = box.querySelector(".measure .tag");
  const measureText = measureEl.parentElement;

  const CHAR_MS = 32;
  const PAUSE = { ".": 220, "!": 220, "?": 220, "…": 260, "。": 220, "！": 220, "？": 220, ",": 90, "、": 90, ";": 120, ":": 120 };
  // CJK spezzabile a ogni carattere, il resto a ogni parola.
  const TOKEN = /\s+|[　-ヿ㐀-鿿＀-￯]|[^\s　-ヿ㐀-鿿＀-￯]+/g;

  let pages = [];
  let page = 0;
  let shown = 0;
  let timer = null;
  let resolveCurrent = null;
  let listeners = { char: [], talk: [] };

  function emit(type, value) { listeners[type].forEach((fn) => fn(value)); }

  // ---------- avanzamento automatico ----------
  let autoTimer = null;
  function clearAuto() { clearTimeout(autoTimer); autoTimer = null; }
  function scheduleAuto() {
    clearAuto();
    const c = window.appConfig;
    if (!c || !c.auto_advance) return;
    const ms = (c.auto_advance_secs || 3) * 1000 + (pages[page] || "").length * 25;
    let heldBySpeech = false;
    const tick = () => {
      // la voce sta ancora leggendo, o c'è una richiesta di conferma aperta: aspetta
      const confirmOpen = !document.getElementById("confirm")?.classList.contains("hidden");
      if (window.Audio2?.isSpeaking() || confirmOpen) {
        heldBySpeech = true;
        autoTimer = setTimeout(tick, 300);
      } else if (heldBySpeech) {
        heldBySpeech = false;
        autoTimer = setTimeout(tick, 900); // piccola pausa dopo la fine della voce
      } else {
        autoTimer = null;
        advance();
      }
    };
    autoTimer = setTimeout(tick, ms);
  }

  function fits(text) {
    measureEl.textContent = text;
    const lh = parseFloat(getComputedStyle(measureText).lineHeight);
    return measureText.scrollHeight <= lh * 2 + 1;
  }

  /** Spezza il testo in pagine che stanno in 2 righe (▼ compresa). */
  function paginate(text) {
    measureTag.textContent = tagEl.textContent;
    const out = [];
    for (const block of text.split(/\n{2,}/)) {
      let current = "";
      for (const tok of block.match(TOKEN) || []) {
        const candidate = current + tok;
        if (current && !fits(candidate)) {
          out.push(current.trimEnd());
          current = /^\s+$/.test(tok) ? "" : tok;
          // parola più lunga di una pagina: spezzala a caratteri
          while (current && !fits(current)) {
            let cut = current.length - 1;
            while (cut > 1 && !fits(current.slice(0, cut))) cut--;
            out.push(current.slice(0, cut));
            current = current.slice(cut);
          }
        } else {
          current = candidate;
        }
      }
      if (current.trim()) out.push(current.trim());
    }
    return out.length ? out : [""];
  }

  function stopTyping() {
    clearTimeout(timer);
    timer = null;
    emit("talk", false);
  }

  function typeNext() {
    const full = pages[page];
    if (shown >= full.length) {
      stopTyping();
      box.classList.add("waiting");
      scheduleAuto();
      return;
    }
    const ch = full[shown++];
    textEl.textContent = full.slice(0, shown);
    if (!/\s/.test(ch)) emit("char", ch);
    timer = setTimeout(typeNext, CHAR_MS + (PAUSE[ch] || 0));
  }

  function showPage(i) {
    clearAuto();
    page = i;
    shown = 0;
    textEl.textContent = "";
    box.classList.remove("waiting");
    emit("talk", true);
    typeNext();
  }

  function finish() {
    clearAuto();
    box.classList.add("hidden");
    box.classList.remove("waiting");
    const r = resolveCurrent;
    resolveCurrent = null;
    if (r) r();
  }

  /** Clic/Invio: completa la pagina, poi passa alla successiva, poi chiude. */
  function advance() {
    if (box.classList.contains("hidden") || box.classList.contains("pending")) return;
    if (timer) {
      stopTyping();
      shown = pages[page].length;
      textEl.textContent = pages[page];
      box.classList.add("waiting");
      scheduleAuto();
    } else if (page < pages.length - 1) {
      showPage(page + 1);
    } else {
      finish();
    }
  }

  /** Mostra un messaggio. Un nuovo messaggio interrompe quello in corso. */
  function say(text) {
    stopTyping();
    box.classList.remove("pending");
    if (resolveCurrent) resolveCurrent();
    box.classList.remove("hidden");
    pages = paginate(String(text));
    return new Promise((resolve) => {
      resolveCurrent = resolve;
      showPage(0);
    });
  }

  /** Box con "…" animati mentre Alter Ego pensa. Non si chiude col clic. */
  function pending(label = "") {
    stopTyping();
    clearAuto();
    if (resolveCurrent) { const r = resolveCurrent; resolveCurrent = null; r(); }
    box.classList.remove("hidden", "waiting");
    box.classList.add("pending");
    textEl.textContent = label;
    pages = [""];
    page = 0;
  }

  function setName(name) { tagEl.textContent = name; }

  box.addEventListener("click", advance);

  return {
    say,
    pending,
    advance,
    setName,
    isOpen: () => !box.classList.contains("hidden"),
    on: (type, fn) => listeners[type].push(fn),
  };
})();
