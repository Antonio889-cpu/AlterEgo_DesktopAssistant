// Contenuti aggiuntivi: ogni tanto, durante un gioco riconosciuto, un "ospite" prende il
// controllo della TV di Alter Ego (vedi src-tauri/src/extras/). Interferenza, la finestra si
// ingrandisce al centro dello schermo, l'ospite parla (testo + Line vocali a caso per umore,
// musica in loop), poi se ne va e Alter Ego si scusa per le "interferenze".
// Anche la prima domanda all'avvio ("scarico i contenuti aggiuntivi?").

const Extras = (() => {
  const box = $("hijack");
  const screen = box.querySelector(".hj-screen");
  const bg = box.querySelector(".hj-bg");
  const sprite = box.querySelector(".hj-sprite");
  const nameEl = box.querySelector(".hj-name");
  const textEl = box.querySelector(".hj-text");
  const nextEl = box.querySelector(".hj-next");
  const dlg = box.querySelector(".hj-box");
  const noise = box.querySelector(".hj-static");
  const nctx = noise.getContext("2d");

  const OFFICES = ["assets/extras/office1.jpg", "assets/extras/office2.jpg"];
  let active = false;
  let staticLevel = 0;
  let sprites = {};
  let advance = null;   // risolve l'attesa del click / Invio
  let skipTyping = null;

  const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

  // ---------- statica (canvas a bassa risoluzione, ingrandita) ----------
  let noiseRunning = false;
  function drawNoise() {
    if (staticLevel <= 0.01) { noise.style.opacity = 0; noiseRunning = false; return; }
    noiseRunning = true;
    const img = nctx.createImageData(noise.width, noise.height);
    const d = img.data;
    for (let i = 0; i < d.length; i += 4) {
      const v = Math.random() * 255;
      d[i] = v * 0.8; d[i + 1] = v; d[i + 2] = v * 0.8; d[i + 3] = 255;
    }
    // righe "strappate"
    for (let k = 0; k < 3; k++) {
      const y = Math.floor(Math.random() * noise.height);
      for (let x = 0; x < noise.width; x++) { const i = (y * noise.width + x) * 4; d[i] = d[i + 1] = d[i + 2] = 230; }
    }
    nctx.putImageData(img, 0, 0);
    noise.style.opacity = staticLevel;
    requestAnimationFrame(drawNoise);
  }
  function setStatic(level) {
    staticLevel = level;
    if (level > 0.01 && !noiseRunning) requestAnimationFrame(drawNoise);
  }

  // ---------- risorse del pacchetto ----------
  let fontsDone = false;
  function applyAssets(a) {
    sprites = a.sprites || {};
    // uffici inclusi nell'app (a caso a ogni evento); lo sfondo del pacchetto solo se mancano
    const office = OFFICES[Math.floor(Math.random() * OFFICES.length)] || a.background;
    bg.style.backgroundImage = office ? `url("${office}")` : "";
    nameEl.textContent = a.nameTag || "";
    if (!fontsDone) {
      const css = [];
      if (a.fonts?.name) css.push(`@font-face { font-family: "HJName"; src: url("${a.fonts.name}"); font-weight: 100 900; }`);
      if (a.fonts?.text) css.push(`@font-face { font-family: "HJText"; src: url("${a.fonts.text}"); font-weight: 200 900; }`);
      const st = document.createElement("style");
      st.textContent = css.join("\n");
      document.head.appendChild(st);
      fontsDone = true;
    }
  }

  function setSprite(mood) {
    const src = sprites[mood] || sprites.idle || Object.values(sprites)[0];
    if (!src) return;
    if (sprite.getAttribute("src") !== src) {
      sprite.src = src;
      sprite.classList.remove("pop");
      void sprite.offsetWidth;
      sprite.classList.add("pop");
      // un attimo di segnale disturbato a ogni cambio
      setStatic(0.35);
      setTimeout(() => active && setStatic(0.06), 160);
    }
  }

  // ---------- testo ----------
  function waitAdvance(ms) {
    return new Promise((resolve) => {
      const timer = setTimeout(done, ms);
      function done() { clearTimeout(timer); advance = null; resolve(); }
      advance = done;
    });
  }

  async function type(text) {
    textEl.textContent = "";
    nextEl.classList.remove("on");
    let skip = false;
    skipTyping = () => { skip = true; };
    for (const ch of text) {
      if (skip) break;
      textEl.textContent += ch;
      await sleep(/[.!?…]/.test(ch) ? 160 : /[,;:]/.test(ch) ? 80 : 26);
    }
    textEl.textContent = text;
    skipTyping = null;
    nextEl.classList.add("on");
    await waitAdvance(2200 + text.length * 45);
    nextEl.classList.remove("on");
  }

  function onAdvance(e) {
    if (!active) return;
    if (e.type === "keydown") {
      if (e.key !== "Enter" && e.key !== " ") return;
      e.preventDefault();
      e.stopImmediatePropagation(); // il box di Alter Ego non deve andare avanti
    }
    if (skipTyping) skipTyping();
    else if (advance) advance();
  }
  box.addEventListener("click", onAdvance);
  window.addEventListener("keydown", onAdvance, true);

  // ---------- l'evento ----------
  const FALLBACK = () => [
    { mood: "laugh", text: t("Upupupu! Pensavi di giocare in pace? Che ingenuo!") },
    { mood: "sinister", text: t("Ho visto come va a finire… e non ti piacerà per niente.") },
  ];
  const SORRY = [
    "Scusa… c'è stata un'interferenza. Sto cercando di capire da dove arriva…",
    "Ah… ci sono state delle interferenze sul segnale. Sto cercando di risolvere, scusami…",
    "Hai visto anche tu? Qualcosa ha preso il controllo del monitor per un attimo… Ci sto lavorando.",
    "Scusami… il segnale è saltato. Non so cosa sia successo, ma sto cercando di sistemarlo.",
    "Interferenze… di nuovo. Mi dispiace, sto controllando i miei circuiti.",
  ];

  async function run({ game, manual }) {
    if (active) return;
    if (!manual && (alterEgoBusy() || petStarting)) { invoke("extras_done", { shown: false }); return; }
    active = true;
    let shown = false;
    try {
      const prep = invoke("extras_prepare", { game: game || null });
      applyAssets(await invoke("extras_assets"));
      Audio2.stop();
      // 1. interferenza sulla TV di Alter Ego, poi la finestra si ingrandisce
      document.body.classList.add("hijacking");
      root.classList.add("interference");
      invoke("extras_sfx", { name: "intro" });
      await invoke("extras_window", { on: true });
      await sleep(700);
      // 2. l'ospite prende il monitor
      box.classList.remove("hidden", "out");
      dlg.classList.add("off");
      sprite.removeAttribute("src");
      setStatic(1);
      shown = true;
      invoke("extras_music", { on: true });
      await sleep(650);
      root.classList.remove("interference");
      setStatic(0.06);
      setSprite("idle");
      await sleep(250);
      invoke("extras_line", { mood: "intro" });
      // 3. parla (il testo può arrivare dopo: lo screenshot va al modello AI)
      let pages = await Promise.race([prep, sleep(25000).then(() => null)]);
      if (!pages?.length) pages = FALLBACK();
      dlg.classList.remove("off");
      for (const p of pages) {
        setSprite(p.mood);
        invoke("extras_line", { mood: p.mood });
        await type(p.text);
      }
      // 4. se ne va
      dlg.classList.add("off");
      setSprite(sprites.leave ? "leave" : "idle");
      invoke("extras_line", { mood: "outro" });
      await sleep(1700);
      invoke("extras_sfx", { name: "outro" });
      setStatic(1);
      invoke("extras_music", { on: false });
      await sleep(700);
    } catch (err) {
      console.warn("extras:", err);
    } finally {
      box.classList.add("hidden");
      setStatic(0);
      root.classList.remove("interference");
      document.body.classList.remove("hijacking");
      invoke("extras_music", { on: false });
      await invoke("extras_window", { on: false }).catch(() => {});
      active = false;
      invoke("extras_done", { shown });
    }
    if (!shown) return;
    // 5. Alter Ego si riprende il monitor
    await sleep(500);
    const line = t(SORRY[Math.floor(Math.random() * SORRY.length)]);
    State.setEmotion("worried");
    State.setMode("speaking");
    Audio2.speak(line);
    await Dialog.say(line);
    State.settle();
  }

  listen("extras-go", (e) => run(e.payload || {}));

  // ---------- primo avvio: scaricare i contenuti aggiuntivi? ----------
  const setup = $("setup");
  async function install() {
    Chat.notify(t("Scarico i contenuti aggiuntivi…"));
    try {
      await invoke("extras_install", {});
      Chat.notify(t("Contenuti aggiuntivi installati ✓"));
    } catch (err) {
      Chat.notify(t("Download non riuscito: {e}", { e: String(err) }));
    }
  }
  listen("extras-progress", (e) => {
    const { done, total } = e.payload;
    if (total > 0 && done < total) Chat.notify(t("Scarico i contenuti aggiuntivi… {p}%", { p: Math.floor((done / total) * 100) }));
  });
  $("setup-yes").onclick = () => { setup.classList.add("hidden"); install(); };
  $("setup-no").onclick = () => { setup.classList.add("hidden"); invoke("extras_asked"); };
  async function askFirstRun() {
    try {
      const s = await invoke("extras_status");
      if (s.installed || s.asked || !s.url) return;
      setup.classList.remove("hidden");
      setTimeout(() => $("setup-yes").focus(), 50);
    } catch {}
  }

  return { active: () => active, askFirstRun };
})();
