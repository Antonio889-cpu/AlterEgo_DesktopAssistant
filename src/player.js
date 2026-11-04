// Lettore musicale: stato dal backend (music-state), spettro e cambi di sprite (music-spectrum).
const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;
const win = window.__TAURI__.window.getCurrentWindow();
const $ = (id) => document.getElementById(id);

const BANDS = 24;
let st = null;
let stAt = 0; // performance.now() quando è arrivato lo stato
let seeking = false;

// ---------- barre dello spettro ----------
const bars = [];
for (let i = 0; i < BANDS; i++) {
  const b = document.createElement("span");
  $("viz").appendChild(b);
  bars.push(b);
}
function drawBands(bands) {
  bars.forEach((b, i) => { b.style.height = Math.max(2, (bands[i] || 0) * 0.52) + "px"; });
}

// ---------- sprite ----------
const face = $("face");
const imgs = {};
let current = null;
async function loadSprites() {
  let user = {};
  try { user = await invoke("load_user_sprites"); } catch {}
  const files = window.SPRITES.files;
  for (const name of Object.keys(files)) {
    const img = new Image();
    img.src = user[name] || `assets/sprites/${files[name]}`;
    img.alt = "";
    face.appendChild(img);
    imgs[name] = img;
  }
  showSprite("idle");
}
function showSprite(name) {
  if (!imgs[name] || name === current) return;
  current = name;
  for (const [n, img] of Object.entries(imgs)) img.classList.toggle("on", n === name);
}
let beatTimer = null;
function beat() {
  face.classList.add("beat");
  clearTimeout(beatTimer);
  beatTimer = setTimeout(() => face.classList.remove("beat"), 110);
}

// ---------- stato ----------
const fmt = (s) => { s = Math.max(0, Math.floor(s || 0)); return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, "0")}`; };
function position() {
  if (!st) return 0;
  let p = st.position;
  if (st.playing && !st.paused) p += (performance.now() - stAt) / 1000;
  return st.duration ? Math.min(p, st.duration) : p;
}

function applyState(s) {
  st = s;
  stAt = performance.now();
  const trk = s.track;
  const title = $("title");
  if (s.status) {
    title.textContent = s.status;
    title.classList.add("status");
    $("artist").textContent = trk ? `${trk.title} — ${trk.artist}` : "";
  } else {
    title.classList.remove("status");
    title.textContent = trk ? trk.title : "—";
    $("artist").textContent = trk ? trk.artist : "";
  }
  document.body.classList.toggle("paused", !s.playing || s.paused);
  $("b-shuffle").classList.toggle("on", s.shuffle);
  $("b-repeat").classList.toggle("on", s.repeat !== "off");
  $("b-repeat").classList.toggle("one", s.repeat === "one");
  $("b-repeat").title = t({ off: "Ripeti: no", all: "Ripeti: tutta la coda", one: "Ripeti: questo brano" }[s.repeat]);
  $("dur").textContent = fmt(s.duration);
  if (document.activeElement !== $("volume") && !volTimer) setVolumeUi(s.volume);
  $("b-fav").classList.toggle("on", !!s.favorite);
  $("b-fav").title = t(s.favorite ? "Togli dai preferiti" : "Aggiungi ai preferiti");
  $("b-venue").classList.toggle("on", !!s.venue_open);
  $("b-venue").title = t(s.venue_open ? "Chiudi la Music Venue" : "Apri la Music Venue");
  if (!s.reactive_sprite) showSprite("idle");
  render();
}

// il cursore va da 0 a 100; il guadagno vero segue una curva nel backend (metà corsa ≈ volume normale)
function setVolumeUi(v) {
  const pct = Math.round(v * 100);
  $("volume").value = pct;
  $("vol-val").textContent = pct;
  $("volume").style.setProperty("--v", `${pct}%`);
  $("vol-w1").style.opacity = v > 0.05 ? 1 : 0.2;
  $("vol-w2").style.opacity = v > 0.5 ? 1 : 0.2;
}

function lyricLines(pos) {
  const ly = st?.track?.lyrics;
  if (!ly || !ly.length) return null;
  let i = 0;
  while (i + 1 < ly.length && ly[i + 1][0] <= pos + 0.25) i++;
  if (ly[0][0] > pos + 0.25) i = -1; // prima della prima riga
  const at = (k) => (k < 0 ? "♪" : k < ly.length ? ly[k][1] || "♪" : "");
  return [at(i), at(i + 1), at(i + 2)];
}

let lastLyric = "";
function render() {
  if (!st) return;
  const pos = seeking ? seeking : position();
  const pct = st.duration ? (pos / st.duration) * 100 : 0;
  $("fill").style.width = pct + "%";
  $("knob").style.left = pct + "%";
  $("pos").textContent = fmt(pos);
  const lines = lyricLines(pos);
  const key = lines ? lines.join("|") : "none";
  if (key !== lastLyric) {
    lastLyric = key;
    const els = $("lines").children;
    const trk = st.track;
    const why = !trk ? "" : trk.lyrics_source == null ? t("cerco il testo…") : t("testo non trovato");
    const show = lines || [trk ? "♪ ♪ ♪" : "", why, ""];
    for (let k = 0; k < 3; k++) els[k].textContent = show[k];
    $("lyrics").classList.toggle("none", !lines);
    $("lyrics").title = st.track?.lyrics_source ? `${t("Testo")}: ${st.track.lyrics_source}${st.track.lyrics_synced ? "" : " — " + t("tempi stimati")}` : "";
  }
}
(function loop() { render(); requestAnimationFrame(loop); })();

// ---------- comandi ----------
$("b-play").onclick = () => invoke("music_toggle");
$("b-next").onclick = () => invoke("music_next");
$("b-prev").onclick = () => invoke("music_prev");
$("b-shuffle").onclick = () => invoke("music_shuffle");
$("b-repeat").onclick = () => invoke("music_repeat");
$("close").onclick = () => invoke("music_stop");
$("b-fav").onclick = async () => {
  try {
    const on = await invoke("venue_favorite", { id: "" });
    $("b-fav").classList.toggle("on", on);
    $("b-fav").classList.remove("pop"); void $("b-fav").offsetWidth; $("b-fav").classList.add("pop");
  } catch {}
};
$("b-venue").onclick = () => invoke(st?.venue_open ? "venue_close" : "venue_open");
let volTimer = null;
function setVolume(pct) {
  pct = Math.max(0, Math.min(100, Math.round(pct)));
  setVolumeUi(pct / 100);
  clearTimeout(volTimer);
  volTimer = setTimeout(() => { volTimer = null; invoke("music_volume", { volume: pct / 100 }); }, 60);
}
$("volume").oninput = (e) => setVolume(+e.target.value);
// rotella sul volume: 2 punti a scatto
$("vol").addEventListener("wheel", (e) => { e.preventDefault(); setVolume(+$("volume").value + (e.deltaY < 0 ? 2 : -2)); }, { passive: false });

// seek: clic o trascinamento sulla barra
const bar = $("bar");
function seekAt(e) {
  const r = bar.getBoundingClientRect();
  const f = Math.min(1, Math.max(0, (e.clientX - r.left) / r.width));
  return f * (st?.duration || 0);
}
bar.addEventListener("pointerdown", (e) => {
  if (!st?.duration) return;
  e.stopPropagation();
  bar.setPointerCapture(e.pointerId);
  seeking = seekAt(e) || 0.001;
});
bar.addEventListener("pointermove", (e) => { if (seeking) seeking = seekAt(e) || 0.001; });
bar.addEventListener("pointerup", (e) => {
  if (!seeking) return;
  const to = seekAt(e);
  seeking = false;
  st.position = to;
  stAt = performance.now();
  invoke("music_seek", { secs: to });
});

// ---------- dimensione: il contenuto è disegnato a 600 px e si scala con la finestra ----------
const BASE_W = 600;
function fit() { document.body.style.zoom = String(window.innerWidth / BASE_W); }
window.addEventListener("resize", fit);
fit();
let rs = null;
let rsWidth = null;
const grip = $("resize");
grip.addEventListener("pointerdown", (e) => {
  e.stopPropagation();
  grip.setPointerCapture(e.pointerId);
  rs = { x: e.screenX, w: window.innerWidth };
});
grip.addEventListener("pointermove", (e) => {
  if (!rs) return;
  const first = rsWidth === null;
  rsWidth = rs.w + (e.screenX - rs.x);
  if (first) requestAnimationFrame(() => { invoke("player_resize", { width: rsWidth }); rsWidth = null; });
});
grip.addEventListener("pointerup", () => { rs = null; });
grip.addEventListener("pointercancel", () => { rs = null; });
window.addEventListener("wheel", (e) => {
  if (!e.ctrlKey) return;
  e.preventDefault();
  invoke("player_resize", { width: window.innerWidth * (e.deltaY < 0 ? 1.06 : 1 / 1.06) });
}, { passive: false });

// spostamento della finestra (fuori dai comandi)
$("screen").addEventListener("mousedown", (e) => {
  if (e.button !== 0 || e.target.closest("button, input, #bar, #resize")) return;
  win.startDragging();
});
window.addEventListener("contextmenu", (e) => e.preventDefault());
window.addEventListener("keydown", (e) => {
  if (e.key === " ") { e.preventDefault(); invoke("music_toggle"); }
  else if (e.key === "ArrowUp") { e.preventDefault(); setVolume(+$("volume").value + 2); }
  else if (e.key === "ArrowDown") { e.preventDefault(); setVolume(+$("volume").value - 2); }
  else if (e.key === "ArrowRight") invoke("music_seek", { secs: position() + 5 });
  else if (e.key === "ArrowLeft") invoke("music_seek", { secs: Math.max(0, position() - 5) });
});

// ---------- accensione / spegnimento "TV" ----------
const panel = $("panel");
// accensione e spegnimento con effetto glitch (glitch.css)
function tvOn() {
  panel.classList.remove("tv-off", "glitch-out");
  void panel.offsetWidth;
  panel.classList.add("glitch-in");
  setTimeout(() => panel.classList.remove("glitch-in"), 800);
}
function tvOff() {
  panel.classList.remove("glitch-in");
  panel.classList.add("glitch-out");
  setTimeout(() => { panel.classList.add("tv-off"); invoke("player_closed"); }, 600);
}

// ---------- eventi ----------
listen("music-state", (e) => applyState(e.payload));
listen("music-spectrum", (e) => {
  const s = e.payload;
  drawBands(s.bands);
  if (!st?.reactive_sprite) return;
  if (s.beat) beat();
  if (s.sprite) showSprite(s.sprite);
});
// NB: listen() riceve anche gli eventi mandati ad altre finestre: il lettore ha i suoi nomi
listen("player-tv", (e) => (e.payload === "off" ? tvOff() : tvOn()));

(async () => {
  try { I18N.setLang(I18N.resolve((await invoke("get_config")).ui_language)); } catch {}
  await document.fonts.load('20px "DotGothic16"');
  await loadSprites();
  drawBands([]);
  try { applyState(await invoke("music_state")); } catch {}
  tvOn();
})();
