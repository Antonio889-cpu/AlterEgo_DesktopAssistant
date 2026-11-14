// Pet: lo sprite di Alter Ego che gira per il desktop. Fisica, finestre e mouse sono in Rust
// (src-tauri/src/pet/): qui si disegna l'animazione che arriva con "pet-anim".
//
// Foglio sprite: 2 righe di fotogrammi quadrati.
//   riga 1: fermo A, fermo B, salto, caduta
//   riga 2: corsa (runFrames fotogrammi)
// Il personaggio nel foglio guarda/corre verso sinistra: verso destra si specchia.

const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

const W = 128, H = 168, F = 128; // canvas e fotogramma (pixel dello sprite)
const cv = document.getElementById("pet");
const ctx = cv.getContext("2d");
const bubble = document.getElementById("bubble");
const sticker = document.getElementById("sticker");

let sheet = null;
let frame = 128;         // lato di un fotogramma nel foglio
let runFrames = 4;
let align = [];          // per fotogramma: spostamento verticale per avere i piedi sul fondo
let shiftX = 0;          // centratura orizzontale comune
let stickers = { 1: "", 2: "" };
let anim = "fall";
let right = false;
let since = performance.now();
let lastKey = "";

const img = (src) => new Promise((ok, ko) => {
  const i = new Image();
  i.onload = () => ok(i);
  i.onerror = ko;
  i.src = src;
});

/** Riga e colonna del fotogramma da disegnare adesso. */
function current(now) {
  const t = (now - since) / 1000;
  switch (anim) {
    case "run": return [1, Math.floor(t * 9) % runFrames];
    case "jump": return [0, 2];
    case "fall":
    case "held": return [0, 3];
    default: return [0, Math.floor(t / 0.6) % 2]; // fermo: dondola tra due pose
  }
}

/** Misura ogni fotogramma (pixel non trasparenti): piedi sul fondo, corpo centrato. */
function measure() {
  const c = document.createElement("canvas");
  c.width = sheet.naturalWidth;
  c.height = sheet.naturalHeight;
  const g = c.getContext("2d", { willReadFrequently: true });
  g.drawImage(sheet, 0, 0);
  const s = F / frame;
  const boxes = [];
  for (const [row, cols] of [[0, 4], [1, runFrames]]) {
    for (let col = 0; col < cols; col++) {
      const d = g.getImageData(col * frame, row * frame, frame, frame).data;
      let x0 = frame, y0 = frame, x1 = -1, y1 = -1;
      for (let y = 0; y < frame; y++) for (let x = 0; x < frame; x++) {
        if (d[(y * frame + x) * 4 + 3] > 24) { x0 = Math.min(x0, x); x1 = Math.max(x1, x); y0 = Math.min(y0, y); y1 = Math.max(y1, y); }
      }
      if (x1 < 0) { x0 = 0; y0 = 0; x1 = frame - 1; y1 = frame - 1; }
      boxes.push({ row, col, x0: x0 * s, y0: y0 * s, x1: (x1 + 1) * s, y1: (y1 + 1) * s });
    }
  }
  align = {};
  let ux0 = W, uy0 = H, ux1 = 0, uy1 = 0;
  // centro orizzontale: quello delle pose da fermo (le gambe della corsa non lo spostano)
  const idle = boxes.filter((b) => b.row === 0 && b.col < 2);
  shiftX = Math.round(W / 2 - idle.reduce((a, b) => a + (b.x0 + b.x1) / 2, 0) / idle.length);
  for (const b of boxes) {
    const dy = Math.round(H - b.y1);
    align[`${b.row}:${b.col}`] = dy;
    // corpo per le collisioni e il mouse: pose da terra e da fermo
    if (b.row === 1 || b.col < 2) {
      ux0 = Math.min(ux0, b.x0 + shiftX); ux1 = Math.max(ux1, b.x1 + shiftX);
      uy0 = Math.min(uy0, b.y0 + dy); uy1 = Math.max(uy1, b.y1 + dy);
    }
  }
  // un po' più stretto del disegno (capelli e braccia non sono "muri")
  const pad = (ux1 - ux0) * 0.12;
  body = [(ux0 + pad) / W, uy0 / H, (ux1 - pad) / W, Math.min(1, uy1 / H)];
  report();
}

/** Corpo in frazioni della finestra (l'area dello sprite può essere più piccola della finestra). */
let body = null;
function report() {
  if (!body) return;
  const r = document.getElementById("stage").getBoundingClientRect();
  const vw = window.innerWidth, vh = window.innerHeight;
  if (!vw || !vh || !r.width) return;
  const fx = (f) => (r.left + f * r.width) / vw;
  const fy = (f) => (r.top + f * r.height) / vh;
  invoke("pet_ready", { hit: [fx(body[0]), fy(body[1]), fx(body[2]), Math.min(1, fy(body[3]))] });
}
window.addEventListener("resize", report);

function draw(now) {
  requestAnimationFrame(draw);
  if (!sheet) return;
  const [row, col] = current(now);
  const key = `${row}:${col}:${right}`;
  if (key === lastKey) return;
  lastKey = key;
  ctx.clearRect(0, 0, W, H);
  ctx.save();
  if (right) { ctx.translate(W, 0); ctx.scale(-1, 1); }
  ctx.imageSmoothingEnabled = false;
  ctx.drawImage(sheet, col * frame, row * frame, frame, frame, shiftX, align[`${row}:${col}`] ?? H - F, F, F);
  ctx.restore();
}

async function load() {
  try {
    I18N.setLang(I18N.resolve((await invoke("get_config")).ui_language));
  } catch {}
  const a = await invoke("pet_assets");
  runFrames = Math.max(1, a.runFrames || 4);
  try {
    sheet = await img(a.sprites);
  } catch {
    sheet = null;
    return;
  }
  frame = sheet.naturalHeight / 2;
  stickers = { 1: a.sticker1, 2: a.sticker2 };
  measure();
  lastKey = "";
}

// ---------- sticker ----------
let bubbleTimer = null;
function showSticker(n) {
  const src = stickers[n];
  if (!src) return;
  clearTimeout(bubbleTimer);
  sticker.src = src;
  bubble.classList.remove("hidden", "out");
  void bubble.offsetWidth;
  bubbleTimer = setTimeout(() => {
    bubble.classList.add("out");
    bubbleTimer = setTimeout(() => bubble.classList.add("hidden"), 300);
  }, n === 1 ? 1600 : 2200);
}

// ---------- eventi dal backend ----------
function flash(cls, ms) {
  document.body.classList.remove(cls);
  void document.body.offsetWidth;
  document.body.classList.add(cls);
  setTimeout(() => document.body.classList.remove(cls), ms);
}
listen("pet-anim", (e) => {
  const prev = anim;
  if (e.payload.anim !== anim) since = performance.now();
  anim = e.payload.anim;
  right = e.payload.right;
  document.body.classList.toggle("held", anim === "held");
  if (prev === "fall" && (anim === "idle" || anim === "run")) flash("land", 220);
});
listen("pet-sticker", (e) => showSticker(e.payload));
listen("pet-spawn", () => { anim = "jump"; since = performance.now(); flash("spawn", 450); });
listen("pet-assets", () => load());
listen("config", (e) => I18N.setLang(I18N.resolve(e.payload.ui_language)));

// ---------- mouse: prendi, lancia, click, doppio click ----------
cv.addEventListener("pointerdown", (e) => {
  if (e.button !== 0) return;
  cv.setPointerCapture(e.pointerId);
  invoke("pet_grab");
});
const up = (e) => {
  if (e.button !== 0 && e.type === "pointerup") return;
  invoke("pet_release");
};
cv.addEventListener("pointerup", up);
cv.addEventListener("pointercancel", up);
window.addEventListener("contextmenu", (e) => e.preventDefault());

load();
requestAnimationFrame(draw);
