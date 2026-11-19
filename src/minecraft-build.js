// Impostazioni → Giochi → Minecraft: schematiche (file e costruzioni chieste a parole) con anteprima
// isometrica o per piani, e quello che Alter Ego ha imparato guardando i giocatori e nei duelli.
// Backend: src-tauri/src/mclink/plan.rs (minecraft_schematic_*, minecraft_plan).
(() => {
  const { invoke } = window.__TAURI__.core;
  const { listen } = window.__TAURI__.event;
  const el = (id) => document.getElementById(id);
  const tt = (s, v) => (typeof t === "function" ? t(s, v) : s);
  if (!el("mc-plan-view")) return;
  let view = null; // anteprima mostrata: {w, h, l, palette, blocks, name}
  let info = "";
  let planAt = 0;

  // ---------- colori dei blocchi ----------
  const COLORS = [
    [/grass/, "#5fa83a"], [/dirt|mud|podzol/, "#6b4a2b"], [/cobble|^stone$|andesite|tuff/, "#7d7d7d"], [/stone_brick|bricks?$/, "#8a8a8a"],
    [/oak_planks|oak_log|oak_door|ladder|oak_fence/, "#a27a45"], [/spruce/, "#6e4f2c"], [/birch/, "#d7c98c"], [/dark_oak/, "#4a3018"], [/jungle/, "#a87252"],
    [/acacia/, "#b05e33"], [/cherry/, "#e2a4a0"], [/mangrove/, "#773a32"], [/_log$|_wood$/, "#6d5332"], [/glass/, "#a8d8e8"], [/sand/, "#dcd29b"],
    [/gravel/, "#8c8480"], [/water/, "#3f6fd8"], [/lava/, "#e06a1d"], [/leaves/, "#3e7d2b"], [/wool|concrete|terracotta/, "#c9c9c9"], [/torch|lantern/, "#ffd75a"],
    [/deepslate/, "#4b4b52"], [/quartz/, "#ece6dc"], [/gold/, "#f2c94c"], [/iron/, "#d8d8d8"], [/diamond/, "#5fe0d0"], [/obsidian/, "#2a1f3c"], [/brick/, "#9c4f3c"],
  ];
  const cache = new Map();
  function color(name) {
    if (cache.has(name)) return cache.get(name);
    let c = (COLORS.find(([re]) => re.test(name)) || [])[1];
    if (!c) { let h = 0; for (const ch of name) h = (h * 31 + ch.charCodeAt(0)) >>> 0; c = `hsl(${h % 360} 35% 52%)`; }
    cache.set(name, c);
    return c;
  }
  function shade(hex, k) {
    const m = /^#(..)(..)(..)$/.exec(hex);
    if (!m) return hex;
    const f = (x) => Math.max(0, Math.min(255, Math.round(parseInt(x, 16) * k))).toString(16).padStart(2, "0");
    return `#${f(m[1])}${f(m[2])}${f(m[3])}`;
  }

  // ---------- disegno ----------
  function draw() {
    const c = el("mc-plan-view");
    const g = c.getContext("2d");
    g.clearRect(0, 0, c.width, c.height);
    if (!view || !view.blocks || !view.blocks.length) {
      g.fillStyle = "#2a5a2a"; g.font = "13px monospace"; g.textAlign = "center";
      g.fillText(tt("nessuna anteprima"), c.width / 2, c.height / 2);
      return;
    }
    const mode = el("mc-plan-mode").value;
    const layer = Number(el("mc-plan-layer").value) || 0;
    if (mode === "layer") {
      // piano per piano dall'alto: quello scelto pieno, quello sotto in trasparenza
      const s = Math.max(2, Math.floor(Math.min((c.width - 10) / view.w, (c.height - 10) / view.l)));
      const ox = (c.width - s * view.w) / 2, oy = (c.height - s * view.l) / 2;
      g.strokeStyle = "#163016";
      for (let x = 0; x <= view.w; x++) { g.beginPath(); g.moveTo(ox + x * s, oy); g.lineTo(ox + x * s, oy + view.l * s); g.stroke(); }
      for (let z = 0; z <= view.l; z++) { g.beginPath(); g.moveTo(ox, oy + z * s); g.lineTo(ox + view.w * s, oy + z * s); g.stroke(); }
      for (const [x, y, z, i] of view.blocks) {
        if (y !== layer && y !== layer - 1) continue;
        g.globalAlpha = y === layer ? 1 : 0.25;
        g.fillStyle = color(view.palette[i] || "");
        g.fillRect(ox + x * s + 1, oy + z * s + 1, s - 2, s - 2);
      }
      g.globalAlpha = 1;
      return;
    }
    // isometrica: fino al piano scelto (così si vede dentro)
    const maxY = Math.min(view.h - 1, layer);
    const span = view.w + view.l;
    const s = Math.max(1.5, Math.min((c.width - 16) / span, (c.height - 16) / (span / 2 + maxY + 1)));
    const ox = c.width / 2 - ((view.w - view.l) * s) / 2;
    const oy = (c.height - (span / 2 + maxY + 1) * s) / 2 + (maxY + 1) * s;
    const list = view.blocks.filter((b) => b[1] <= maxY).sort((a, b) => a[0] + a[2] - (b[0] + b[2]) || a[1] - b[1]);
    for (const [x, y, z, i] of list) {
      const base = color(view.palette[i] || "");
      const px = ox + (x - z) * s, py = oy + (x + z) * s / 2 - y * s;
      const top = [[px, py - s], [px + s, py - s / 2], [px, py], [px - s, py - s / 2]];
      const poly = (pts, fill) => { g.beginPath(); g.moveTo(pts[0][0], pts[0][1]); for (const p of pts.slice(1)) g.lineTo(p[0], p[1]); g.closePath(); g.fillStyle = fill; g.fill(); };
      poly(top, shade(base, 1.12));
      poly([[px - s, py - s / 2], [px, py], [px, py + s], [px - s, py + s / 2]], shade(base, 0.8));
      poly([[px + s, py - s / 2], [px, py], [px, py + s], [px + s, py + s / 2]], shade(base, 0.62));
    }
  }

  function show(prev, text) {
    view = prev && prev.blocks ? prev : null;
    info = text || "";
    const r = el("mc-plan-layer");
    r.max = view ? Math.max(0, view.h - 1) : 0;
    r.value = r.max; // nuova anteprima: tutta intera
    el("mc-plan-layer-val").textContent = view ? `${Number(r.value) + 1}/${view.h}` : "";
    el("mc-plan-info").textContent = info || tt("Nessuna anteprima: scegli un file o chiedile una costruzione.");
    draw();
  }

  const mats = (need, n = 8) => Object.entries(need || {}).sort((a, b) => b[1] - a[1]).slice(0, n).map(([k, v]) => `${v} ${k}`).join(", ");

  function fileInfo(v) {
    const lines = [tt("{n}: {w}×{h}×{l}, {b} blocchi", { n: v.name || v.file || "", w: v.size[0], h: v.size[1], l: v.size[2], b: v.blocks })];
    if (v.need) lines.push(tt("Servono: {m}", { m: mats(v.need) }));
    if (v.skipped && Object.keys(v.skipped).length) lines.push(tt("Non si piazzano: {m}", { m: mats(v.skipped, 4) }));
    if (v.preview && v.preview.truncated) lines.push(tt("Anteprima parziale (costruzione molto grande)"));
    return lines.join("\n");
  }

  function planInfo(p) {
    const lines = [p.line || p.name || ""];
    if (p.missing && Object.keys(p.missing).length) lines.push(tt("Mancano: {m}", { m: mats(p.missing) }));
    if (p.clear) lines.push(tt("Da togliere: {n} blocchi ({t} alberi)", { n: p.clear, t: p.trees || 0 }));
    if (p.at) lines.push(tt("Punto: {x} {y} {z}", { x: p.at.x, y: p.at.y, z: p.at.z }));
    return lines.filter(Boolean).join("\n");
  }

  // ---------- file ----------
  async function list(select) {
    let names = [];
    try { names = (await invoke("minecraft_schematics")) || []; } catch {}
    const s = el("mc-schem-list");
    s.innerHTML = "";
    for (const n of Array.isArray(names) ? names : []) { const o = document.createElement("option"); o.value = n; o.textContent = n; s.appendChild(o); }
    if (select) s.value = select;
    el("mc-schem-build").disabled = !s.value;
  }

  el("mc-schem-pick").onclick = () => el("mc-schem-file").click();
  el("mc-schem-file").onchange = (e) => {
    const f = e.target.files && e.target.files[0];
    e.target.value = "";
    if (!f) return;
    const msg = el("mc-schem-msg");
    msg.textContent = tt("Leggo la schematica…"); msg.className = "result";
    const r = new FileReader();
    r.onload = async () => {
      try {
        const v = await invoke("minecraft_schematic_load", { name: f.name, data: String(r.result) });
        msg.textContent = tt("Schematica {f} pronta ✓", { f: v.file || f.name }); msg.className = "result ok";
        await list(v.file);
        show(v.preview, fileInfo(v));
      } catch (err) { msg.textContent = String(err); msg.className = "result bad"; }
    };
    r.readAsDataURL(f);
  };
  el("mc-schem-list").onchange = async () => {
    const file = el("mc-schem-list").value;
    el("mc-schem-build").disabled = !file;
    if (!file) return;
    try { const v = await invoke("minecraft_schematic_preview", { file }); show(v.preview, fileInfo({ ...v, file })); }
    catch (err) { el("mc-schem-msg").textContent = String(err); el("mc-schem-msg").className = "result bad"; }
  };
  el("mc-schem-build").onclick = async () => {
    const msg = el("mc-schem-msg");
    try { await invoke("minecraft_schematic_build", { file: el("mc-schem-list").value }); msg.textContent = tt("Alter Ego controlla i blocchi e poi costruisce"); msg.className = "result ok"; }
    catch (err) { msg.textContent = String(err); msg.className = "result bad"; }
  };
  el("mc-plan-mode").onchange = draw;
  el("mc-plan-layer").oninput = () => { el("mc-plan-layer-val").textContent = view ? `${Number(el("mc-plan-layer").value) + 1}/${view.h}` : ""; draw(); };

  // ---------- ultimo piano e cose imparate ----------
  async function refresh() {
    let p;
    try { p = await invoke("minecraft_plan"); } catch { return; }
    if (!p || typeof p !== "object") return;
    if (p.at && p.at !== planAt && p.plan && p.plan.preview) { planAt = p.at; show(p.plan.preview, planInfo(p.plan)); }
    const ul = el("mc-people");
    ul.innerHTML = "";
    for (const line of [...(p.people || []), ...(p.duels || [])].slice(0, 12)) { const li = document.createElement("li"); li.textContent = line; ul.appendChild(li); }
  }

  listen("minecraft-plan", refresh);
  list();
  show(null, "");
  refresh();
  setInterval(refresh, 5000);
})();
