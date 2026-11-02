// Conversazione: input di testo, chiamata al backend (streaming + fallback), parsing [emozione].

const Chat = (() => {
  const { invoke, Channel } = window.__TAURI__.core;
  const prompt = document.getElementById("prompt");
  const input = document.getElementById("prompt-input");
  const toast = document.getElementById("toast");
  let busy = false;

  /** "[happy]\ntesto\n---EN---\ntext" → { emotion, it, en } */
  function parseReply(raw) {
    let text = raw.replace(/\r/g, "");
    const m = text.match(/^\s*\[([a-zA-Z_]+)\]\s*/);
    const emotion = m ? m[1].toLowerCase() : "idle";
    if (m) text = text.slice(m[0].length);
    const parts = text.split(/^\s*-{3}\s*EN\s*-{3}\s*$/m);
    const clean = (s) => (s || "").replace(/^\s*\[[a-z_]+\]\s*/gim, "").trim();
    return { emotion, it: clean(parts[0]), en: clean(parts[1]) };
  }

  let toastTimer = null;
  function notify(text) {
    toast.textContent = text;
    toast.classList.add("on");
    clearTimeout(toastTimer);
    toastTimer = setTimeout(() => toast.classList.remove("on"), 5000);
  }

  /** Testo da mostrare secondo la lingua scelta (solo IT / IT+EN / solo EN). */
  function displayText(reply) {
    const lang = window.appConfig?.language || "it";
    if (lang === "it_en" && reply.en) return `${reply.it}\n\n${reply.en}`;
    return reply.it || reply.en || "…";
  }

  async function ask(text, image = null) {
    text = text.trim();
    if (!text || busy) return;
    busy = true;
    State.setMode("thinking");
    Dialog.pending();

    const events = new Channel();
    events.onmessage = (ev) => {
      if (ev.kind === "fallback") notify(`${ev.from} non risponde (${ev.reason}) → provo il successivo`);
    };
    try {
      // contesto e memoria li gestisce il backend
      const res = await invoke("chat", { text, image, onEvent: events });
      const reply = parseReply(res.text);
      State.setEmotion(reply.emotion);
      State.setMode("speaking");
      busy = false;
      await Dialog.say(displayText(reply));
    } catch (err) {
      busy = false;
      State.setEmotion("error");
      State.setMode("speaking");
      const name = window.appConfig?.user_name?.trim();
      await Dialog.say(`Ah… scusa${name ? ", " + name : ""}, non riesco a collegarmi. Sono solo un programma, ma ce la sto mettendo tutta…\n\n(${err})`);
    }
    State.settle();
  }

  // ---------- push-to-talk (la registrazione la fa il backend) ----------
  function onStt(ev) {
    switch (ev.state) {
      case "recording":
        if (busy) return;
        State.setMode("listening");
        Dialog.pending("🎙 Ti ascolto");
        break;
      case "transcribing":
        State.setMode("thinking");
        Dialog.pending("✎ Trascrivo");
        break;
      case "done":
        notify(`Hai detto: “${ev.text}”`);
        ask(ev.text);
        break;
      case "empty":
        State.settle();
        Dialog.say("Ehm… non ho sentito niente. Tieni premuto il tasto mentre parli.");
        break;
      case "error":
        State.setEmotion("worried");
        State.setMode("speaking");
        Dialog.say(`Non riesco a sentirti… (${ev.message})`).then(() => State.settle());
        break;
    }
  }

  function openInput() {
    if (busy) return;
    invoke("focus_main");
    State.setMode("listening");
    prompt.classList.remove("hidden");
    input.value = "";
    setTimeout(() => input.focus(), 30);
  }

  function closeInput() {
    prompt.classList.add("hidden");
    input.blur();
    if (State.mode() === "listening") State.settle();
  }

  input.addEventListener("keydown", (e) => {
    e.stopPropagation(); // Invio/Spazio qui non devono far avanzare il dialogo
    if (e.key === "Enter" && !e.isComposing) {
      e.preventDefault();
      const text = input.value;
      closeInput();
      ask(text);
    } else if (e.key === "Escape") {
      closeInput();
    }
  });
  input.addEventListener("blur", () => setTimeout(() => { if (document.activeElement !== input) closeInput(); }, 150));

  return { ask, openInput, closeInput, parseReply, notify, onStt };
})();
