// Volto di Alter Ego: sprite precaricati, cambio istantaneo (come nel gioco).
// Parlato: talk_open/talk_closed se esistono, altrimenti un leggero sobbalzo.
// Blink: solo se esiste lo sprite "blink".

const Face = (() => {
  const box = document.getElementById("face");
  const { files, alias } = window.SPRITES;
  const imgs = {};
  let current = null;
  let talking = false;
  let talkTimer = null;
  let blinkTimer = null;

  for (const [name, file] of Object.entries(files)) {
    const img = new Image();
    img.src = `assets/sprites/${file}`;
    img.alt = "";
    img.draggable = false;
    img.dataset.state = name;
    box.appendChild(img);
    imgs[name] = img;
  }

  /** Stato → nome sprite esistente (segue gli alias, fallback idle). */
  function resolve(state) {
    if (files[state]) return state;
    const a = alias[state];
    return a && files[a] ? a : "idle";
  }

  function show(spriteName) {
    for (const [n, img] of Object.entries(imgs)) img.classList.toggle("on", n === spriteName);
  }

  function set(state) {
    current = state;
    box.dataset.state = state;
    show(resolve(state));
  }

  const hasTalkFrames = files.talk_open && files.talk_closed;
  function setTalking(on) {
    talking = on;
    clearInterval(talkTimer);
    box.classList.toggle("talking", on && !hasTalkFrames);
    if (on && hasTalkFrames) {
      let open = false;
      talkTimer = setInterval(() => show((open = !open) ? "talk_open" : "talk_closed"), 120);
    } else if (!on) {
      show(resolve(current));
    }
  }

  function scheduleBlink() {
    if (!files.blink) return;
    blinkTimer = setTimeout(() => {
      if (current === "idle" && !talking) {
        show("blink");
        setTimeout(() => show(resolve(current)), 130);
      }
      scheduleBlink();
    }, 2500 + Math.random() * 4000);
  }

  set("idle");
  scheduleBlink();

  return {
    set,
    get: () => current,
    setTalking,
    isTalking: () => talking,
  };
})();
