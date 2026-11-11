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

  /** Lingua delle risposte: "auto" segue la lingua dell'interfaccia. */
  function answerLang() {
    const l = window.appConfig?.language || "it";
    return l === "auto" ? I18N.lang() : l;
  }

  /** Testo da mostrare secondo la lingua scelta (solo IT / IT+EN / solo EN). */
  function displayText(reply) {
    const lang = answerLang();
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
      if (ev.kind === "fallback") notify(`${ev.from}: ${ev.reason}`);
    };
    try {
      // contesto e memoria li gestisce il backend
      const res = await invoke("chat", { text, image, onEvent: events });
      const reply = parseReply(res.text);
      State.setEmotion(reply.emotion);
      State.setMode("speaking");
      busy = false;
      const shown = displayText(reply);
      Audio2.speak(reply.it || reply.en, answerLang() === "en" ? "en" : "it");
      if (res.model === "clip") Audio2.chime();
      await Dialog.say(shown);
    } catch (err) {
      busy = false;
      if (/^(interrotto|stopped)/.test(String(err))) {
        State.setEmotion("embarrassed");
        State.setMode("speaking");
        await Dialog.say(t("Va bene, mi fermo qui."));
        State.settle();
        return;
      }
      State.setEmotion("error");
      State.setMode("speaking");
      const name = window.appConfig?.user_name?.trim();
      await Dialog.say(t("Ah… scusa{name}, non riesco a collegarmi. Sono solo un programma, ma ce la sto mettendo tutta…\n\n({err})", { name: name ? ", " + name : "", err }));
    }
    State.settle();
  }

  // ---------- push-to-talk (la registrazione la fa il backend) ----------
  function onStt(ev) {
    switch (ev.state) {
      case "recording":
        if (busy) return;
        State.setMode("listening");
        Dialog.pending(t("🎙 Ti ascolto"));
        break;
      case "transcribing":
        State.setMode("thinking");
        Dialog.pending(t("✎ Trascrivo"));
        break;
      case "done":
        notify(t("Hai detto: “{text}”", { text: ev.text }));
        ask(ev.text);
        break;
      case "clip":
        State.setEmotion(ev.ok ? "happy" : "worried");
        State.setMode("speaking");
        if (ev.ok) Audio2.chime();
        Dialog.say(ev.ok ? ev.message : t("Non sono riuscita a fare la clip… ({msg})", { msg: ev.message })).then(() => State.settle());
        break;
      case "empty":
        State.settle(true);
        Dialog.say(t("Ehm… non ho sentito niente. Tieni premuto il tasto mentre parli."));
        break;
      case "error":
        State.setEmotion("worried");
        State.setMode("speaking");
        Dialog.say(t("Non riesco a sentirti… ({msg})", { msg: ev.message })).then(() => State.settle(true));
        break;
    }
  }

  function openInput() {
    if (busy) { notify(t("Sto ancora lavorando… premi Esc per interrompere.")); return; }
    invoke("focus_main");
    State.setMode("listening");
    Dialog.suspend(true);
    prompt.classList.remove("hidden");
    input.value = "";
    setTimeout(() => input.focus(), 30);
  }

  function closeInput() {
    prompt.classList.add("hidden");
    Dialog.suspend(false);
    input.blur();
    if (State.mode() === "listening") State.settle(true);
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

  /** Interrompe la risposta in corso (Esc). */
  function cancel() {
    if (!busy) return false;
    invoke("chat_cancel");
    notify(t("Interrompo…"));
    return true;
  }

  /** Alter Ego sta usando strumenti (azioni PC). `p`: { names, round, max } (o solo l'elenco dei nomi). */
  function onToolUse(p) {
    const names = Array.isArray(p) ? p : p.names;
    const step = Array.isArray(p) || !p.round ? "" : ` (${p.round}/${p.max})`;
    const labels = { open_target: "apro", list_dir: "guardo la cartella", read_file: "leggo il file", write_file: "scrivo il file",
      delete_file: "elimino il file", set_reminder: "imposto il promemoria", list_reminders: "controllo i promemoria",
      cancel_reminder: "annullo il promemoria", read_clipboard: "leggo gli appunti", write_clipboard: "copio negli appunti",
      take_screenshot: "guardo lo schermo", set_volume: "regolo il volume", set_brightness: "regolo la luminosità", run_command: "eseguo il comando",
      run_shell: "uso il terminale", web_search: "cerco sul web", web_fetch: "leggo la pagina", play_music: "cerco la musica", music_control: "controllo la musica",
      mouse_move: "muovo il mouse", mouse_click: "clicco", mouse_scroll: "scorro", type_text: "scrivo", press_keys: "premo i tasti", app_control: "sistemo l'app",
      zoom_screen: "guardo da vicino", ui_elements: "leggo i controlli della finestra", find_files: "cerco i file", make_dir: "creo la cartella",
      move_path: "sposto il file", sql_connect: "mi collego al database", sql_query: "interrogo il database", sql_disconnect: "chiudo il database",
      load_tools: "prendo altri strumenti", graph_build: "mappo il progetto", graph_ask: "consulto la mappa" };
    State.setMode("thinking");
    Dialog.pending("⚙ " + names.map((n) => t(labels[n] || n)).join(", ") + step);
  }

  return { onToolUse, cancel, isBusy: () => busy, ask, openInput, closeInput, parseReply, notify, onStt };
})();
