// Volto di Alter Ego. Sorgenti, in ordine:
// 1. sprite importati in %APPDATA%\AlterEgo\sprites (Impostazioni → Personalità → Sprite);
// 2. sprite inclusi nella build (src/assets/sprites/);
// 3. volto segnaposto originale (kaomoji), così l'app funziona anche senza sprite.
// Parlato: talk_open/talk_closed se esistono, altrimenti un leggero sobbalzo.
// Blink: solo se esiste lo sprite "blink".

const Face = (() => {
  const box = document.getElementById("face");
  const { files: bundledFiles, alias } = window.SPRITES;
  const KAOMOJI = {
    idle: "(・ω・)", happy: "(＾▽＾)", embarrassed: "(〃ω〃)", worried: "(・_・;)", thinking: "(・ε・)…",
    sad: "(´・ω・`)", surprised: "(ﾟoﾟ;)", error: "(×_×)", sleep: "(－ω－) zzZ", listening: "(・ω・)♪", blink: "(－ω－)",
  };
  let imgs = {};
  let mode = "loading";
  let current = "idle";
  let talking = false;
  let talkTimer = null;
  let blinkTimer = null;
  let music = false;
  const placeholder = document.createElement("div");
  placeholder.className = "face-placeholder";
  placeholder.innerHTML = "<span></span><small>Volto segnaposto · importa gli sprite da Impostazioni → Personalità</small>";

  const has = (n) => !!imgs[n];
  function resolve(state) {
    if (has(state)) return state;
    const a = alias[state];
    return a && has(a) ? a : "idle";
  }

  function show(name) {
    if (mode === "loading") return;
    if (mode === "placeholder") {
      placeholder.firstChild.textContent = KAOMOJI[name] || KAOMOJI.idle;
      return;
    }
    for (const [n, img] of Object.entries(imgs)) img.classList.toggle("on", n === name);
  }

  function set(state) {
    current = state;
    box.dataset.state = state;
    // musica in corso e Alter Ego tranquilla: cuffie e leggero dondolio
    const groove = music && state === "idle" && !talking && has("music");
    box.classList.toggle("groove", groove);
    document.body.classList.toggle("grooving", groove);
    show(mode === "placeholder" ? state : groove ? "music" : resolve(state));
  }

  /** Musica in corso (decide app.js: solo quando Alter Ego non è occupata). */
  function setMusic(on) {
    if (music === on) return;
    music = on;
    // piccolo glitch nel cambio di sprite
    box.classList.remove("swap-glitch"); void box.offsetWidth; box.classList.add("swap-glitch");
    set(current);
  }

  function setTalking(on) {
    talking = on;
    clearInterval(talkTimer);
    const frames = has("talk_open") && has("talk_closed");
    box.classList.toggle("talking", on && !frames);
    if (on && frames) {
      let open = false;
      talkTimer = setInterval(() => show((open = !open) ? "talk_open" : "talk_closed"), 120);
    } else if (!on) {
      set(current);
    }
  }

  function scheduleBlink() {
    clearTimeout(blinkTimer);
    if (!has("blink")) return;
    blinkTimer = setTimeout(() => {
      if (current === "idle" && !talking) {
        show("blink");
        setTimeout(() => set(current), 130);
      }
      scheduleBlink();
    }, 2500 + Math.random() * 4000);
  }

  function canLoad(src) {
    return new Promise((ok) => { const i = new Image(); i.onload = () => ok(true); i.onerror = () => ok(false); i.src = src; });
  }

  function build(sources) {
    box.innerHTML = "";
    imgs = {};
    for (const [name, src] of Object.entries(sources)) {
      const img = new Image();
      img.src = src;
      img.alt = "";
      img.draggable = false;
      box.appendChild(img);
      imgs[name] = img;
    }
  }

  /** (Ri)carica gli sprite. Chiamata all'avvio e dopo un'importazione. */
  async function load() {
    let user = {};
    try { user = await window.__TAURI__.core.invoke("load_user_sprites"); } catch {}
    if (user.idle) {
      mode = "user";
      // gli sprite importati non hanno le cuffie: si usa quello incluso, se c'è
      if (!user.music && (await canLoad(`assets/sprites/${bundledFiles.music}`))) user.music = `assets/sprites/${bundledFiles.music}`;
      build(user);
    } else if (await canLoad(`assets/sprites/${bundledFiles.idle}`)) {
      mode = "bundled";
      build(Object.fromEntries(Object.entries(bundledFiles).map(([n, f]) => [n, `assets/sprites/${f}`])));
    } else {
      mode = "placeholder";
      box.innerHTML = "";
      imgs = {};
      box.appendChild(placeholder);
    }
    box.classList.toggle("placeholder", mode === "placeholder");
    set(current);
    scheduleBlink();
  }

  window.__TAURI__.event.listen("sprites-changed", () => load());
  load();

  return { set, setMusic, get: () => current, setTalking, isTalking: () => talking, mode: () => mode, reload: load };
})();
