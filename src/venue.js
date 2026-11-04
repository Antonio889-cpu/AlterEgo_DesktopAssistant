// Music Venue: libreria con brani recenti e preferiti, ricerca, e il brano in riproduzione.
const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;
const win = window.__TAURI__.window.getCurrentWindow();
const $ = (id) => document.getElementById(id);

let lib = { recent: [], favorites: [] };
let st = null;
let stAt = 0;
let tab = "library";

const fmt = (s) => { s = Math.max(0, Math.floor(s || 0)); return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, "0")}`; };
const favIds = () => new Set(lib.favorites.map((f) => f.id));

// ---------- righe ----------
const ICON_FAV = '<svg viewBox="0 0 24 24"><path d="M12 20s-7.5-4.6-7.5-10.2A4.3 4.3 0 0 1 12 7a4.3 4.3 0 0 1 7.5 2.8C19.5 15.4 12 20 12 20z"/></svg>';
const ICON_QUEUE = '<svg viewBox="0 0 24 24"><path d="M4 7h11M4 12h11M4 17h7M18 14v6M15 17h6"/></svg>';
const ICON_DEL = '<svg viewBox="0 0 24 24"><path d="M7 7l10 10M17 7L7 17"/></svg>';

function row(e, { removable = false, i = 0 } = {}) {
  const li = document.createElement("li");
  li.className = "row";
  li.dataset.id = e.id;
  li.style.animationDelay = `${Math.min(i, 12) * 35}ms`;
  const th = document.createElement("div");
  th.className = "thumb";
  if (e.thumbnail) {
    const img = new Image();
    img.src = e.thumbnail;
    img.alt = "";
    img.draggable = false;
    img.onerror = () => { img.remove(); th.innerHTML = '<span class="ph">♪</span>'; };
    th.append(img);
  } else th.innerHTML = '<span class="ph">♪</span>';
  const meta = document.createElement("div");
  meta.className = "meta";
  meta.innerHTML = '<div class="t"></div><div class="a"></div>';
  meta.children[0].textContent = e.title;
  meta.children[1].textContent = e.artist;
  const acts = document.createElement("div");
  acts.className = "acts";
  const eq = document.createElement("span");
  eq.className = "eq";
  eq.innerHTML = "<i></i><i></i><i></i>";
  const fav = Object.assign(document.createElement("button"), { className: "fav" + (favIds().has(e.id) ? " on" : ""), innerHTML: ICON_FAV, title: t("Preferito") });
  fav.onclick = async (ev) => { ev.stopPropagation(); try { await invoke("venue_favorite", { id: e.id }); } catch {} };
  const q = Object.assign(document.createElement("button"), { innerHTML: ICON_QUEUE, title: t("Aggiungi in coda") });
  q.onclick = (ev) => { ev.stopPropagation(); playEntry(e, true, li); };
  acts.append(eq, fav, q);
  if (removable) {
    const del = Object.assign(document.createElement("button"), { innerHTML: ICON_DEL, title: t("Togli dai recenti") });
    del.onclick = (ev) => { ev.stopPropagation(); invoke("venue_remove", { id: e.id }); };
    acts.append(del);
  }
  const dur = Object.assign(document.createElement("div"), { className: "dur", textContent: e.duration ? fmt(e.duration) : "" });
  li.append(th, meta, acts, dur);
  li.onclick = () => playEntry(e, false, li);
  return li;
}

function fill(ul, items, opts = {}) {
  ul.innerHTML = "";
  if (!items.length) {
    ul.innerHTML = `<li class="empty"></li>`;
    ul.firstChild.textContent = opts.empty || "";
    return;
  }
  items.forEach((e, i) => ul.append(row(e, { ...opts, i })));
  markCurrent();
}

function render() {
  const q = $("q").value.trim().toLowerCase();
  const searching = q.length > 0;
  $("box-search").hidden = !searching;
  $("box-recent").hidden = searching || tab === "favorites";
  $("box-fav").hidden = searching || tab === "recent";
  document.querySelectorAll(".more").forEach((b) => { b.hidden = tab !== "library"; });
  const few = tab === "library" ? 5 : 999;
  fill($("rows-recent"), lib.recent.slice(0, few), { removable: true, empty: t("Ancora niente: chiedi ad Alter Ego di mettere un brano.") });
  fill($("rows-fav"), lib.favorites.slice(0, few), { empty: t("Nessun preferito: premi il cuore nel lettore o qui.") });
  if (searching) {
    const seen = new Set();
    const hits = [...lib.favorites, ...lib.recent].filter((e) => {
      if (seen.has(e.id)) return false;
      seen.add(e.id);
      return `${e.title} ${e.artist}`.toLowerCase().includes(q);
    });
    $("search-title").textContent = t("Nella libreria");
    fill($("rows-search"), hits, { empty: t("Nella libreria non c'è: cercalo su YouTube qui sotto.") });
  }
}

function markCurrent() {
  const id = st?.playing ? st.track?.id : null;
  document.querySelectorAll(".row").forEach((r) => r.classList.toggle("current", r.dataset.id === id));
}

async function playEntry(e, enqueue, li) {
  li?.classList.remove("flash"); void li?.offsetWidth; li?.classList.add("flash");
  setStatus(enqueue ? t("Aggiungo in coda…") : t("Carico il brano…"));
  try { setStatus(await invoke("venue_play", { id: e.id, query: null, enqueue })); }
  catch (err) { setStatus(String(err)); }
}

async function searchYoutube(enqueue) {
  const q = $("q").value.trim();
  if (!q) return;
  setStatus(t("Cerco su YouTube…"));
  try { setStatus(await invoke("venue_play", { id: "", query: q, enqueue })); }
  catch (err) { setStatus(String(err)); }
}

let statusTimer = null;
function setStatus(s) {
  $("status").textContent = s || "";
  clearTimeout(statusTimer);
  statusTimer = setTimeout(() => { $("status").textContent = st?.status || ""; }, 6000);
}

// ---------- in riproduzione ----------
let lastThumb = null;
function applyState(s) {
  st = s;
  stAt = performance.now();
  const trk = s.playing ? s.track : null;
  $("now-title").textContent = trk ? trk.title : t("Niente in riproduzione");
  $("now-artist").textContent = trk ? trk.artist : "";
  const thumb = trk?.thumbnail || "";
  if (thumb !== lastThumb) {
    lastThumb = thumb;
    $("now-img").src = thumb;
    $("now-thumb").classList.remove("swap"); void $("now-thumb").offsetWidth; $("now-thumb").classList.add("swap");
  }
  $("now-fav").classList.toggle("on", !!s.favorite);
  $("now-fav").disabled = !trk;
  $("now-dur").textContent = fmt(s.duration);
  document.body.classList.toggle("paused", !s.playing || s.paused);
  if (s.status) $("status").textContent = s.status;
  markCurrent();
}
function position() {
  if (!st) return 0;
  let p = st.position;
  if (st.playing && !st.paused) p += (performance.now() - stAt) / 1000;
  return st.duration ? Math.min(p, st.duration) : p;
}
(function loop() {
  if (st) {
    const p = position();
    $("now-fill").style.width = (st.duration ? (p / st.duration) * 100 : 0) + "%";
    $("now-pos").textContent = fmt(p);
  }
  requestAnimationFrame(loop);
})();

// ---------- comandi ----------
document.querySelectorAll("#tabs button[data-tab]").forEach((b) => (b.onclick = () => showTab(b.dataset.tab)));
document.querySelectorAll(".more").forEach((b) => (b.onclick = () => showTab(b.dataset.go)));
function showTab(name) {
  tab = name;
  $("q").value = "";
  document.querySelectorAll("#tabs button[data-tab]").forEach((b) => b.classList.toggle("on", b.dataset.tab === name));
  render();
}
let searchTimer = null;
$("q").oninput = () => { clearTimeout(searchTimer); searchTimer = setTimeout(render, 120); };
$("q").onkeydown = (e) => { if (e.key === "Enter") searchYoutube(false); if (e.key === "Escape") { $("q").value = ""; render(); } };
$("yt-play").onclick = () => searchYoutube(false);
$("yt-queue").onclick = () => searchYoutube(true);
$("n-play").onclick = () => invoke("music_toggle");
$("n-next").onclick = () => invoke("music_next");
$("n-prev").onclick = () => invoke("music_prev");
$("now-fav").onclick = async () => {
  try { await invoke("venue_favorite", { id: "" }); } catch {}
  $("now-fav").classList.remove("pop"); void $("now-fav").offsetWidth; $("now-fav").classList.add("pop");
};
$("close").onclick = () => invoke("venue_close");

// spostamento e ridimensionamento (finestra senza bordi)
$("screen").addEventListener("mousedown", (e) => {
  if (e.button !== 0 || e.target.closest("button, input, .row, #grip, .rows")) return;
  win.startDragging();
});
$("grip").addEventListener("mousedown", (e) => { if (e.button === 0) { e.preventDefault(); win.startResizeDragging("SouthEast"); } });
window.addEventListener("contextmenu", (e) => e.preventDefault());
window.addEventListener("keydown", (e) => {
  if (e.key === "Escape" && document.activeElement !== $("q")) invoke("venue_close");
});

// ---------- apertura / chiusura glitch ----------
const panel = $("panel");
function tvOn() {
  panel.classList.remove("tv-off", "glitch-out");
  void panel.offsetWidth;
  panel.classList.add("glitch-in");
  setTimeout(() => panel.classList.remove("glitch-in"), 800);
}
function tvOff() {
  panel.classList.remove("glitch-in");
  panel.classList.add("glitch-out");
  setTimeout(() => { panel.classList.add("tv-off"); invoke("venue_closed"); }, 600);
}

// ---------- eventi ----------
listen("music-state", (e) => applyState(e.payload));
listen("venue-library", (e) => { lib = e.payload; render(); });
// NB: listen() riceve anche gli eventi di altre finestre: la Venue ha il suo nome
listen("venue-tv", (e) => (e.payload === "off" ? tvOff() : tvOn()));

(async () => {
  try { I18N.setLang(I18N.resolve((await invoke("get_config")).ui_language)); } catch {}
  try { document.fonts && (await document.fonts.load('20px "DotGothic16"')); } catch {}
  try { lib = await invoke("venue_library"); } catch {}
  render();
  try { applyState(await invoke("music_state")); } catch {}
  tvOn();
})();
