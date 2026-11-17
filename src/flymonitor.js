// FruitFly Monitor: umore e cervello di moscerino di Alter Ego mentre gioca a Garry's Mod.
// Dati da gmodlink_fly_view (5 volte al secondo); il cervello è un disegno a fil di ferro con le
// regioni che si accendono quando lavorano (occhi, antenne, corpi a fungo, centro, gusto, cordone).
const { invoke } = window.__TAURI__.core;
const W = window.__TAURI__.window && window.__TAURI__.window.getCurrentWindow ? window.__TAURI__.window.getCurrentWindow() : null;
const $ = (id) => document.getElementById(id);

let EN = false;
const TXT = {
  mood: ["Umore:", "Mood:"], status: ["Stato:", "Status:"],
  curious: ["Curiosa", "Curious"], calm: ["Calma", "Calm"], anxious: ["Ansiosa", "Anxious"], playful: ["Giocosa", "Playful"], resting: ["A riposo", "Resting"],
  active: ["Attivo", "Active"], brain_off: ["Cervello spento", "Brain off"], waiting: ["In attesa", "Waiting"], offline: ["Non in gioco", "Offline"],
  neurons: ["{n} neuroni · {s} sinapsi", "{n} neurons · {s} synapses"], spikes: ["{a}% attività", "{a}% activity"],
  goal: ["Obiettivo: {g} (passo {n})", "Goal: {g} (step {n})"], fav: ["Preferito: {h} ♥", "Favorite: {h} ♥"],
};
const tx = (k, v = {}) => (TXT[k] ? TXT[k][EN ? 1 : 0] : k).replace(/\{(\w+)\}/g, (_, x) => v[x] ?? "");

(async () => {
  try {
    const c = await invoke("get_config");
    EN = I18N.resolve(c.ui_language) === "en";
  } catch {}
  $("l-mood").textContent = tx("mood");
  $("l-status").textContent = tx("status");
})();

if (W) {
  $("b-min").onclick = () => W.minimize();
  $("b-max").onclick = () => W.toggleMaximize();
  $("b-close").onclick = () => W.close();
}

// ---------- barre ----------
const ORDER = ["curious", "calm", "anxious", "playful", "resting"];
const rows = {};
for (const k of ORDER) {
  const row = document.createElement("div");
  row.className = "bar";
  const name = document.createElement("span");
  const track = document.createElement("span");
  track.className = "track";
  const fill = document.createElement("span");
  fill.className = "fill";
  fill.style.width = "0%";
  track.appendChild(fill);
  const pct = document.createElement("span");
  pct.className = "pct";
  row.append(name, track, pct);
  $("bars").appendChild(row);
  rows[k] = { name, fill, pct };
}

let regions = {};
let activity = 0;
function render(v) {
  const m = v.mood || {};
  const bars = Object.fromEntries(m.bars || []);
  for (const k of ORDER) {
    const p = bars[k] ?? 0;
    rows[k].name.textContent = tx(k);
    rows[k].fill.style.width = `calc(${p}% - 2px)`;
    rows[k].pct.textContent = p + "%";
  }
  $("mood").textContent = v.status === "active" ? tx(m.label || "calm") : "—";
  $("status").textContent = tx(v.status || "offline");
  regions = v.status === "active" ? m.regions || {} : {};
  activity = v.status === "active" ? m.activity || 0 : 0;
  document.querySelector(".fly").classList.toggle("alert", (bars.anxious || 0) > 45);
  $("who").textContent = v.name || "";
  const info = [tx("neurons", { n: (v.neurons || 0).toLocaleString(), s: (v.synapses || 0).toLocaleString() })];
  if (v.status === "active") info.push(tx("spikes", { a: Math.round(activity * 100) }));
  const p = v.persona || {};
  if (p.goal) info.push(tx("goal", { g: p.goal.text, n: p.goal.steps }));
  const fav = (p.habits || []).find((h) => h.pref > 0.5);
  if (fav) info.push(tx("fav", { h: fav.id }));
  if (v.instinct) info.push(v.instinct);
  $("info").replaceChildren(...info.map((s) => Object.assign(document.createElement("div"), { textContent: s })));
}

async function poll() {
  if (!document.hidden) {
    try { render(await invoke("gmodlink_fly_view")); } catch {}
  }
  setTimeout(poll, 200);
}
poll();

// ---------- cervello a fil di ferro ----------
// regioni (coordinate normalizzate: x -0.5..0.5, y -0.4..0.5), a coppie simmetriche
const LOBES = [
  { r: "optic_l", x: -0.37, y: 0.02, a: 0.14, b: 0.21 },
  { r: "optic_r", x: 0.37, y: 0.02, a: 0.14, b: 0.21 },
  { r: "central", x: -0.17, y: -0.02, a: 0.18, b: 0.17 },
  { r: "central", x: 0.17, y: -0.02, a: 0.18, b: 0.17 },
  { r: "mushroom", x: -0.13, y: -0.17, a: 0.12, b: 0.1 },
  { r: "mushroom", x: 0.13, y: -0.17, a: 0.12, b: 0.1 },
  { r: "antenna", x: -0.095, y: 0.17, a: 0.085, b: 0.085 },
  { r: "antenna", x: 0.095, y: 0.17, a: 0.085, b: 0.085 },
  { r: "central", x: 0, y: -0.02, a: 0.032, b: 0.2 },
  { r: "sez", x: 0, y: 0.24, a: 0.075, b: 0.065 },
  { r: "ventral", x: 0, y: 0.38, a: 0.03, b: 0.11 },
];
const cv = $("brain");
const ctx = cv.getContext("2d");
const glow = {};
const sparks = [];
let last = performance.now();

function fit() {
  const r = cv.getBoundingClientRect();
  const d = window.devicePixelRatio || 1;
  cv.width = Math.max(1, Math.round(r.width * d));
  cv.height = Math.max(1, Math.round(r.height * d));
}
new ResizeObserver(fit).observe(cv);
fit();

function frame(now) {
  const dt = Math.min(0.1, (now - last) / 1000);
  last = now;
  const w = cv.width, h = cv.height;
  const s = Math.min(w, h * 1.25);
  const cx = w / 2, cy = h / 2;
  ctx.clearRect(0, 0, w, h);
  ctx.globalCompositeOperation = "lighter";
  ctx.lineJoin = "round";
  const t = now / 1000;
  const lw = Math.max(1, s / 420);
  for (const L of LOBES) {
    // bagliore che segue l'attività (sale svelto, scende piano)
    const want = regions[L.r] || 0;
    const key = L.r + L.x;
    glow[key] = (glow[key] || 0) + (want - (glow[key] || 0)) * (want > (glow[key] || 0) ? 0.5 : 0.08);
    const g = glow[key];
    const X = cx + L.x * s, Y = cy + L.y * s, A = L.a * s, B = L.b * s;
    const base = 0.32 + g * 0.6;
    ctx.shadowColor = "rgba(93,255,106,0.9)";
    ctx.shadowBlur = 6 + g * 18;
    // pieno leggero quando lavora
    if (g > 0.02) {
      const grd = ctx.createRadialGradient(X, Y, 0, X, Y, Math.max(A, B));
      grd.addColorStop(0, `rgba(93,255,106,${0.35 * g})`);
      grd.addColorStop(1, "rgba(93,255,106,0)");
      ctx.fillStyle = grd;
      ctx.beginPath();
      ctx.ellipse(X, Y, A, B, 0, 0, Math.PI * 2);
      ctx.fill();
    }
    ctx.strokeStyle = `rgba(93,255,106,${base})`;
    ctx.lineWidth = lw * 1.6;
    ctx.beginPath();
    ctx.ellipse(X, Y, A, B, 0, 0, Math.PI * 2);
    ctx.stroke();
    // meridiani che "girano" piano (effetto 3D)
    ctx.lineWidth = lw * 0.8;
    ctx.strokeStyle = `rgba(93,255,106,${base * 0.55})`;
    for (let k = 0; k < 5; k++) {
      const ph = Math.cos(t * 0.35 + k * 0.63 + L.x * 3);
      ctx.beginPath();
      ctx.ellipse(X, Y, Math.abs(A * ph), B, 0, 0, Math.PI * 2);
      ctx.stroke();
    }
    // paralleli
    for (let k = 1; k < 6; k++) {
      const yy = -B + (2 * B * k) / 6;
      const half = A * Math.sqrt(1 - (yy * yy) / (B * B));
      ctx.beginPath();
      ctx.ellipse(X, Y + yy, half, Math.max(1, half * 0.12), 0, 0, Math.PI * 2);
      ctx.stroke();
    }
    // scintille = neuroni che sparano
    const n = (g * 26 + activity * 3) * dt * 10;
    for (let i = 0; i < n; i++) {
      const ang = Math.random() * Math.PI * 2, rr = Math.sqrt(Math.random());
      sparks.push({ x: X + Math.cos(ang) * A * rr, y: Y + Math.sin(ang) * B * rr, life: 0.35 + Math.random() * 0.4 });
    }
  }
  ctx.shadowBlur = 8;
  for (let i = sparks.length - 1; i >= 0; i--) {
    const p = sparks[i];
    p.life -= dt;
    if (p.life <= 0) { sparks.splice(i, 1); continue; }
    ctx.fillStyle = `rgba(180,255,180,${Math.min(1, p.life * 2)})`;
    ctx.fillRect(p.x - lw, p.y - lw, lw * 2.2, lw * 2.2);
  }
  if (sparks.length > 1500) sparks.splice(0, sparks.length - 1500);
  ctx.globalCompositeOperation = "source-over";
  ctx.shadowBlur = 0;
  requestAnimationFrame(frame);
}
requestAnimationFrame(frame);
