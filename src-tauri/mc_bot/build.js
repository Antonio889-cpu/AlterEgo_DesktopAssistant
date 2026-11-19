// Costruire da una schematica (fatta a parole o letta da file): sceglie il punto, conta i blocchi contro
// l'inventario e si procura quelli che mancano, elenca e toglie quello che ostruisce (alberi, terra, blocchi
// nel volume e davanti alla porta), riempie i buchi sotto le fondamenta, poi costruisce in ordine
// (fondamenta → pareti → tetto, i dettagli come porte e torce alla fine), con impalcature dove non arriva,
// tolte alla fine.
// Parte 1: logica pura (node:test), con il mondo passato come funzioni. Parte 2 (`attach`): le mani.
"use strict";

const S = require("./schematic");
const I = require("./inventory");

// ---------- parte 1: logica pura ----------

const REPLACEABLE = /^(air|cave_air|void_air|short_grass|tall_grass|fern|large_fern|dead_bush|snow|seagrass|vine|glow_lichen|dandelion|poppy|.*_tulip|oxeye_daisy|cornflower|azure_bluet|allium|blue_orchid|lily_of_the_valley|torchflower|pink_petals|leaf_litter|bush|firefly_bush|short_dry_grass|tall_dry_grass)$/;
const isReplaceable = (n) => !n || REPLACEABLE.test(n);
const NATURAL = /(_log|_leaves|^stone$|^dirt$|grass_block|^sand$|^red_sand$|gravel|_ore$|^deepslate$|^netherrack$|^clay$|^andesite$|^diorite$|^granite$|^tuff$|^coarse_dirt$|^podzol$|^mud$|^moss_block$|^snow_block$|^ice$|^cactus$|^sugar_cane$|^bamboo$|^pumpkin$|^melon$|mushroom|^calcite$|^terracotta$|_terracotta$|^sandstone$|^red_sandstone$)/;
const isTree = (n) => /(_log|_leaves|_wood)$/.test(n) && !/^stripped_/.test(n);
const DECOR = /(torch|door|ladder|_sign|_banner|lantern|flower_pot|_button|lever|_carpet|_pressure_plate|bed$|glass_pane|_bars$|_trapdoor|rail)/;

/**
 * Punto migliore dove costruire w × l davanti al giocatore.
 *  world.ground(x, z) → {y: quota del blocco solido più alto, name, water}
 *  world.block(x, y, z) → nome del blocco
 *  from: {x, y, z} (il giocatore), dir: {x, z} (dove guarda), opts: {h, minDist, maxDist}
 * Restituisce {x, y, z (angolo con y = sopra il terreno), score, flat, obstacles} o null.
 * Punteggio: dislivello (×3), ostacoli nel volume (×1, alberi ×0.6), acqua (×40), distanza (×0.3).
 */
function chooseSite(world, from, dir, w, l, opts = {}) {
  const h = opts.h || 4;
  const minD = opts.minDist == null ? 2 : opts.minDist;
  const maxD = opts.maxDist == null ? 10 : opts.maxDist;
  const d = Math.hypot(dir.x, dir.z) || 1;
  const fx = dir.x / d, fz = dir.z / d;
  const cands = [];
  for (let dist = minD; dist <= maxD; dist += 1) {
    for (let side = -4; side <= 4; side += 2) {
      // centro del lotto davanti (e un po' di lato)
      const cx = from.x + fx * (dist + l / 2) - fz * side;
      const cz = from.z + fz * (dist + l / 2) + fx * side;
      cands.push({ x: Math.round(cx - w / 2), z: Math.round(cz - l / 2), dist: Math.hypot(cx - from.x, cz - from.z) });
    }
  }
  let best = null;
  for (const c of cands) {
    const hs = [];
    let water = 0;
    for (let z = 0; z < l; z++) for (let x = 0; x < w; x++) {
      const g = world.ground(c.x + x, c.z + z);
      if (!g) { water += 1; continue; }
      if (g.water) water++;
      hs.push(g.y);
    }
    if (!hs.length) continue;
    const sorted = hs.slice().sort((a, b) => a - b);
    const med = sorted[Math.floor(sorted.length / 2)];
    const flat = hs.reduce((a, y) => a + Math.abs(y - med), 0);
    let obs = 0;
    for (let y = 1; y <= h; y++) for (let z = 0; z < l; z++) for (let x = 0; x < w; x++) {
      const n = world.block(c.x + x, med + y, c.z + z);
      if (!isReplaceable(n)) obs += isTree(n) ? 0.6 : 1;
    }
    // il giocatore non deve stare dentro il lotto
    const inLot = from.x >= c.x - 0.5 && from.x <= c.x + w + 0.5 && from.z >= c.z - 0.5 && from.z <= c.z + l + 0.5;
    const score = flat * 3 + obs + water * 40 + c.dist * 0.3 + (inLot ? 100 : 0);
    if (!best || score < best.score) best = { x: c.x, y: med + 1, z: c.z, score, flat, obstacles: obs };
  }
  return best;
}

/**
 * Cosa ostruisce: blocchi nel volume (e nel corridoio davanti alla porta) che non sono quelli voluti.
 *  Restituisce {clear: [{x,y,z,name,kind: tree|natural|foreign}], fill: [{x,y,z}] buchi sotto le fondamenta, foreign: {nome: n}, trees}
 *  I tronchi e le foglie attaccati agli alberi che entrano nel volume vengono tolti tutti (niente alberi a metà).
 */
function obstacles(schem, origin, world, opts = {}) {
  const clear = [];
  const seen = new Set();
  const key = (x, y, z) => `${x},${y},${z}`;
  const push = (x, y, z, name) => {
    const k = key(x, y, z);
    if (seen.has(k)) return;
    seen.add(k);
    clear.push({ x, y, z, name, kind: isTree(name) ? "tree" : NATURAL.test(name) ? "natural" : "foreign" });
  };
  // volume della costruzione (anche dove la schematica ha aria: dentro la casa deve essere vuoto)
  for (let y = 0; y < schem.h + (opts.headroom || 0); y++) for (let z = 0; z < schem.l; z++) for (let x = 0; x < schem.w; x++) {
    const wx = origin.x + x, wy = origin.y + y, wz = origin.z + z;
    const have = world.block(wx, wy, wz);
    const want = S.get(schem, x, y, z);
    if (isReplaceable(have)) continue;
    if (have === want) continue;
    push(wx, wy, wz, have);
  }
  // accesso: corridoio di 2 blocchi davanti alle porte (z = -1, -2) alto 2
  for (const b of S.blocks(schem)) {
    if (!/door$/.test(b.name) || b.y > 1) continue;
    for (const dz of [-1, -2]) for (const dy of [0, 1]) {
      const wx = origin.x + b.x, wy = origin.y + b.y + dy, wz = origin.z + b.z + dz;
      const n = world.block(wx, wy, wz);
      if (!isReplaceable(n)) push(wx, wy, wz, n);
    }
  }
  // alberi interi: tronchi e foglie collegati (fino a 6 blocchi di distanza dal primo pezzo)
  const starts = clear.filter((c) => c.kind === "tree");
  for (const s of starts) {
    const q = [[s.x, s.y, s.z]];
    let n = 0;
    while (q.length && n < 200) {
      const [x, y, z] = q.shift();
      for (const [dx, dy, dz] of [[1, 0, 0], [-1, 0, 0], [0, 1, 0], [0, -1, 0], [0, 0, 1], [0, 0, -1]]) {
        const nx = x + dx, ny = y + dy, nz = z + dz;
        if (Math.abs(nx - s.x) > 6 || Math.abs(nz - s.z) > 6 || ny < origin.y) continue;
        if (seen.has(key(nx, ny, nz))) continue;
        const nm = world.block(nx, ny, nz);
        if (!isTree(nm)) continue;
        push(nx, ny, nz, nm);
        q.push([nx, ny, nz]);
        n++;
      }
    }
  }
  // fondamenta: sotto il pavimento ci deve essere qualcosa di pieno
  const fill = [];
  for (let z = 0; z < schem.l; z++) for (let x = 0; x < schem.w; x++) {
    if (S.isAir(S.get(schem, x, 0, z))) continue;
    for (let dy = 1; dy <= 4; dy++) {
      const wy = origin.y - dy;
      const n = world.block(origin.x + x, wy, origin.z + z);
      if (!isReplaceable(n) && !/^(water|lava)$/.test(n)) break;
      fill.push({ x: origin.x + x, y: wy, z: origin.z + z });
    }
  }
  // dall'alto in basso (gli alberi si tolgono dalla cima? no: dal basso si arriva meglio; le foglie restano a terra)
  clear.sort((a, b) => (a.kind === "tree") - (b.kind === "tree") || b.y - a.y);
  const foreign = {};
  for (const c of clear) if (c.kind === "foreign") foreign[c.name] = (foreign[c.name] || 0) + 1;
  fill.sort((a, b) => a.y - b.y);
  // alberi = tronchi distinti (colonne di tronco), non i blocchi
  const trunks = new Set(clear.filter((c) => /_log$|_wood$/.test(c.name)).map((c) => `${c.x},${c.z}`));
  return { clear, fill, foreign, trees: trunks.size || (starts.length ? 1 : 0) };
}

/**
 * Ordine di costruzione: fondamenta (y = 0) → pareti → tetto, strato per strato; in ogni strato prima i
 * blocchi appoggiati a qualcosa (terreno o già messi), poi i loro vicini; i dettagli (porte, torce, scale,
 * vetri) alla fine dello strato. I blocchi che restano sospesi hanno `support: true` (serve un appoggio provvisorio).
 *  solidBelow(x, y, z) → true se nel mondo c'è già un blocco pieno lì (coordinate relative alla schematica).
 */
function order(schem, solidAt = () => false) {
  const all = S.blocks(schem);
  const placed = new Set();
  const key = (x, y, z) => `${x},${y},${z}`;
  const out = [];
  const byY = new Map();
  for (const b of all) { if (!byY.has(b.y)) byY.set(b.y, []); byY.get(b.y).push(b); }
  const ys = [...byY.keys()].sort((a, b) => a - b);
  const topY = ys[ys.length - 1];
  const nb = (b) => [[0, -1, 0], [1, 0, 0], [-1, 0, 0], [0, 0, 1], [0, 0, -1], [0, 1, 0]].some(([dx, dy, dz]) => placed.has(key(b.x + dx, b.y + dy, b.z + dz)) || solidAt(b.x + dx, b.y + dy, b.z + dz));
  for (const y of ys) {
    const layer = byY.get(y);
    const main = layer.filter((b) => !DECOR.test(b.name));
    const decor = layer.filter((b) => DECOR.test(b.name));
    // perimetro prima (le pareti reggono il resto)
    const edge = (b) => b.x === 0 || b.z === 0 || b.x === schem.w - 1 || b.z === schem.l - 1;
    let todo = main.slice().sort((a, b) => Number(edge(b)) - Number(edge(a)) || a.z - b.z || a.x - b.x);
    let guard = 0;
    while (todo.length && guard++ < 10000) {
      const i = todo.findIndex((b) => nb(b));
      const b = i >= 0 ? todo.splice(i, 1)[0] : todo.shift();
      const phase = y === 0 ? "foundation" : y === topY && ys.length > 2 ? "roof" : "walls";
      out.push({ ...b, phase, support: i < 0 });
      placed.add(key(b.x, b.y, b.z));
    }
    for (const b of decor) { out.push({ ...b, phase: "details", support: false }); placed.add(key(b.x, b.y, b.z)); }
  }
  return out;
}

/**
 * Impalcature: per i blocchi troppo alti da raggiungere dal terreno (portata ~4,3 blocchi dagli occhi)
 * sceglie poche colonne su cui salire, fuori dal perimetro o dentro la stanza vuota.
 *  cells: [{x,y,z}] relative (y = 0 è il pavimento, si sta in piedi a y = 1); walkable(x, z) → si può stare lì.
 *  Restituisce {columns: [{x, z, top}], reach: Map "x,y,z" → indice della colonna}.
 */
function scaffolds(cells, w, l, opts = {}) {
  const reach = opts.reach || 4.3;
  const walkable = opts.walkable || ((x, z) => x < 0 || z < 0 || x >= w || z >= l);
  const eyeAt = (x, z, standY) => ({ x: x + 0.5, y: standY + 1.62, z: z + 0.5 });
  const dist = (e, c) => Math.hypot(e.x - (c.x + 0.5), e.y - (c.y + 0.5), e.z - (c.z + 0.5));
  // posti da terra: anello intorno (piedi a y = 0) e interno libero (sul pavimento, piedi a y = 1)
  const out = (x, z) => x < 0 || z < 0 || x >= w || z >= l;
  const ground = [];
  for (let z = -2; z <= l + 1; z++) for (let x = -2; x <= w + 1; x++) if (walkable(x, z)) ground.push({ x, z, y: out(x, z) ? 0 : 1 });
  const high = cells.filter((c) => !ground.some((g) => dist(eyeAt(g.x, g.z, g.y), c) <= reach));
  const columns = [];
  const at = new Map();
  let left = high.slice();
  let guard = 0;
  // le colonne solo fuori dal perimetro: si sale saltando e mettendo un blocco sotto i piedi
  const ring = ground.filter((g) => g.y === 0 && (g.x === -1 || g.z === -1 || g.x === w || g.z === l));
  while (left.length && guard++ < 50) {
    let best = null;
    const minY = Math.min(...left.map((c) => c.y));
    const top = Math.max(1, minY - 1);
    for (const g of ring) {
      const cov = left.filter((c) => dist(eyeAt(g.x, g.z, top), c) <= reach);
      if (!best || cov.length > best.cov.length) best = { g, top, cov };
    }
    if (!best || !best.cov.length) break;
    const ci = columns.length;
    columns.push({ x: best.g.x, z: best.g.z, top: best.top });
    for (const c of best.cov) at.set(`${c.x},${c.y},${c.z}`, ci);
    left = left.filter((c) => !best.cov.includes(c));
  }
  return { columns, reach: at, unreachable: left };
}

/** Riassunto del piano per la chat e la UI. */
function summary(schem, mats, inv, obs, sc) {
  const d = S.diff(mats.need, inv);
  return {
    name: schem.name, size: [schem.w, schem.h, schem.l], blocks: Object.values(mats.need).reduce((a, n) => a + n, 0),
    need: mats.need, missing: d.missing, clear: obs ? obs.clear.length : 0, trees: obs ? obs.trees : 0, fill: obs ? obs.fill.length : 0,
    foreign: obs ? obs.foreign : {}, scaffolds: sc ? sc.columns.length : 0,
  };
}

/** Testo corto del piano ("casetta 5x5: 74 blocchi, mi mancano 20 oak_planks, tolgo 1 albero"). */
function planLine(sum, en = false) {
  const miss = Object.entries(sum.missing || {}).map(([k, n]) => `${n} ${k}`);
  const parts = [`${sum.name}: ${sum.blocks} ${en ? "blocks" : "blocchi"}`];
  if (miss.length) parts.push((en ? "missing " : "mi mancano ") + miss.slice(0, 4).join(", "));
  if (sum.trees) parts.push(en ? `cutting ${sum.trees} tree(s)` : `taglio ${sum.trees} albero/i`);
  else if (sum.clear) parts.push(en ? `clearing ${sum.clear} blocks` : `tolgo ${sum.clear} blocchi`);
  if (sum.scaffolds) parts.push(en ? `${sum.scaffolds} scaffold(s)` : `${sum.scaffolds} impalcatura/e`);
  return parts.join(", ");
}

// ---------- parte 2: le mani (Mineflayer) ----------

function attach(ctx) {
  const { SKILLS } = ctx;
  const bot = () => ctx.bot();
  const V = (x, y, z) => new ctx.Vec3(x, y, z);
  const SCAFFOLD = ["dirt", "cobblestone", "netherrack", "cobbled_deepslate", "oak_planks", "spruce_planks", "birch_planks"];
  let last = null; // ultima schematica (per "costruiscila")

  const world = {
    block(x, y, z) { const b = bot().blockAt(V(x, y, z)); return b ? b.name : "air"; },
    ground(x, z) {
      const top = Math.floor(bot().entity.position.y) + 8;
      for (let y = top; y > top - 20; y--) {
        const b = bot().blockAt(V(x, y, z));
        if (!b) return null;
        if (b.name === "water" || b.name === "lava") return { y, name: b.name, water: true };
        if (b.boundingBox === "block" && !/_leaves$|_log$/.test(b.name)) return { y, name: b.name, water: false };
      }
      return null;
    },
  };
  const solid = (p) => { const b = bot().blockAt(p); return !!b && b.boundingBox === "block"; };

  async function dig(p, c) {
    const b = bot().blockAt(p);
    if (!b || isReplaceable(b.name) || b.name === "bedrock") return;
    if (bot().entity.position.distanceTo(p) > 4.2) await ctx.goNear(p, 3, c, true);
    if (ctx.equipFor) await ctx.equipFor(b).catch(() => {});
    if (!bot().canDigBlock(b)) return;
    await bot().dig(b, true);
  }

  async function placeHere(p, item, c) {
    if (solid(p)) return true;
    if (bot().entity.position.distanceTo(p.offset(0.5, 0.5, 0.5)) > 4.3) await ctx.goNear(p, 3, c);
    // non dentro se stessa
    const me = bot().entity.position.floored();
    if (me.equals(p) || me.offset(0, 1, 0).equals(p)) { await ctx.goNear(p.offset(2, 0, 0), 1, c); }
    return ctx.placeAt(p, item);
  }

  /** Sale su una colonna (salta e mette un blocco sotto i piedi) fino a `top` blocchi sopra la base. */
  async function pillarUp(base, top, c) {
    const placed = [];
    await ctx.goNear(base.offset(0, 1, 0), 0, c);
    const item = SCAFFOLD.find((n) => ctx.countItem(n));
    if (!item) throw new Error(ctx.en() ? "no blocks for the scaffold" : "niente blocchi per l'impalcatura");
    await bot().look(bot().entity.yaw, -Math.PI / 2, true);
    for (let i = 0; i < top; i++) {
      ctx.checkCancel(c);
      const y0 = bot().entity.position.y;
      bot().setControlState("jump", true);
      for (let t = 0; t < 20 && bot().entity.position.y < y0 + 1.05; t++) await ctx.sleep(50);
      const under = bot().entity.position.floored().offset(0, -1, 0);
      const ok = await ctx.placeAt(under, item).catch(() => false);
      bot().setControlState("jump", false);
      if (!ok) break;
      placed.push(under);
      await ctx.sleep(200);
    }
    return placed;
  }

  async function pillarDown(placed, c) {
    for (let i = placed.length - 1; i >= 0; i--) {
      ctx.checkCancel(c);
      try { await bot().dig(bot().blockAt(placed[i]), true); } catch {}
      await ctx.sleep(250);
    }
  }

  /** Si procura i blocchi mancanti (craft ricorsivo: tronchi → assi, scava pietra…). */
  async function procure(missing, c) {
    const failed = {};
    for (const [item, n] of Object.entries(missing)) {
      ctx.checkCancel(c);
      try { await SKILLS.craft({ item, n }, c); } catch (e) { if (/fermata|stopped/.test(e.message)) throw e; failed[item] = e.message; }
    }
    return failed;
  }

  /** Costruisce una schematica `s` (o.at: angolo già scelto). */
  async function run(s, o, c) {
    const b = bot();
    const yaw = b.entity.yaw;
    const from = b.entity.position;
    const dir = { x: -Math.sin(yaw), z: -Math.cos(yaw) };
    let origin = o.at && Number.isFinite(o.at.x) ? { x: Math.round(o.at.x), y: Math.round(o.at.y), z: Math.round(o.at.z) } : chooseSite(world, from, dir, s.w, s.l, { h: s.h });
    if (!origin) throw new Error(ctx.en() ? "no good spot here" : "non trovo un posto adatto qui");
    const mats = S.materials(s);
    const obs = obstacles(s, origin, world);
    const ord = order(s, (x, y, z) => { const n = world.block(origin.x + x, origin.y + y, origin.z + z); return !isReplaceable(n) && n !== "water"; });
    const sc = scaffolds(ord.filter((x) => x.y >= 1), s.w, s.l, { walkable: (x, z) => x < 0 || z < 0 || x >= s.w || z >= s.l || S.isAir(S.get(s, x, 1, z)) });
    const sum = summary(s, mats, b.inventory.items(), obs, sc);
    ctx.ev("plan", { ...sum, at: origin, line: planLine(sum, ctx.en()), preview: S.preview(s, 4000) });
    if (o.dry) return sum;
    // cose costruite da qualcuno in mezzo: chiede prima di romperle
    if (Object.keys(obs.foreign).length) {
      const ok = await ctx.ask("can_break", { blocks: Object.keys(obs.foreign).slice(0, 4) }, 45000, false);
      if (!ok) throw new Error(ctx.en() ? "there's something built in the way and I won't break it" : "c'è qualcosa di costruito in mezzo e non lo rompo");
    }
    // materiali
    let miss = S.diff(mats.need, b.inventory.items()).missing;
    if (Object.keys(miss).length) {
      if (o.gather === false) throw new Error((ctx.en() ? "missing " : "mi mancano ") + Object.entries(miss).map(([k, n]) => `${n} ${k}`).join(", "));
      // si procura quello che manca; rifà il conto (il crafting consuma assi e bastoni) fino a tre volte
      for (let round = 0; round < 3 && Object.keys(miss).length; round++) {
        ctx.ev("build_status", { phase: "gather", missing: miss });
        const failed = await procure(miss, c);
        miss = S.diff(mats.need, bot().inventory.items()).missing;
        if (Object.keys(failed).length && Object.keys(miss).every((k) => failed[k])) break;
      }
    }
    // sgombero: prima gli alberi, poi il resto (dall'alto)
    ctx.ev("build_status", { phase: "clear", n: obs.clear.length });
    for (const x of obs.clear) { ctx.checkCancel(c); await dig(V(x.x, x.y, x.z), c).catch((e) => { if (/fermata|stopped/.test(e.message)) throw e; }); }
    // fondamenta: buchi sotto il pavimento
    const filler = () => ["dirt", "cobblestone", ...S.WALLS].find((n) => ctx.countItem(n));
    for (const f of obs.fill) { ctx.checkCancel(c); const it = filler(); if (it) await placeHere(V(f.x, f.y, f.z), it, c).catch(() => false); }
    // costruzione
    let skipped = 0;
    const done = new Set();
    const mark = (cell) => done.add(`${cell.x},${cell.y},${cell.z}`);
    const temp = [];
    const missingNow = {};
    const colDone = new Set();
    ctx.ev("build_status", { phase: "build", n: ord.length });
    for (const cell of ord) {
      ctx.checkCancel(c);
      const item = S.itemFor(cell.name, S.propsOf(cell.state));
      if (!item) continue;
      const p = V(origin.x + cell.x, origin.y + cell.y, origin.z + cell.z);
      const cur = bot().blockAt(p);
      if (done.has(`${cell.x},${cell.y},${cell.z}`)) continue;
      if (cur && cur.name === cell.name) { mark(cell); continue; }
      if (!ctx.countItem(item)) { missingNow[item] = (missingNow[item] || 0) + 1; skipped++; continue; }
      // troppo alto: sale sull'impalcatura della sua colonna
      const ci = sc.reach.get(`${cell.x},${cell.y},${cell.z}`);
      let pillar = null;
      if (ci != null && !colDone.has(ci)) {
        const col = sc.columns[ci];
        pillar = await pillarUp(V(origin.x + col.x, origin.y - 1, origin.z + col.z), col.top, c).catch(() => []);
        temp.push(...pillar);
      }
      if (cell.support) {
        const under = p.offset(0, -1, 0);
        if (!solid(under) && filler()) { if (await placeHere(under, filler(), c).catch(() => false)) temp.push(under); }
      }
      try {
        if (await placeHere(p, item, c)) mark(cell);
        else skipped++;
      } catch (e) {
        if (/fermata|stopped/.test(e.message)) throw e;
        skipped++;
      }
      if (pillar && pillar.length) {
        // finché è lassù mette tutti i blocchi della stessa colonna
        for (const other of ord) {
          if (sc.reach.get(`${other.x},${other.y},${other.z}`) !== ci || done.has(`${other.x},${other.y},${other.z}`)) continue;
          const it2 = S.itemFor(other.name, S.propsOf(other.state));
          const p2 = V(origin.x + other.x, origin.y + other.y, origin.z + other.z);
          if (it2 && ctx.countItem(it2) && !solid(p2) && bot().entity.position.offset(0, 1.62, 0).distanceTo(p2.offset(0.5, 0.5, 0.5)) <= 4.5) {
            if (await ctx.placeAt(p2, it2).catch(() => false)) mark(other);
          }
        }
        colDone.add(ci);
        await pillarDown(pillar, c);
      }
    }
    // via le impalcature e gli appoggi provvisori rimasti
    for (const t of temp) { const bb = bot().blockAt(t); if (bb && SCAFFOLD.includes(bb.name) && !isWanted(s, origin, t)) await dig(t, c).catch(() => {}); }
    ctx.lastAct();
    const placed = done.size;
    const res = { placed, skipped, total: ord.length, missing: missingNow, at: origin };
    ctx.ev("build_status", { phase: "done", ...res });
    if (!placed && ord.length) throw new Error(ctx.en() ? "couldn't place any block" : "nessun blocco messo");
    return res;
  }

  function isWanted(s, origin, p) {
    const x = p.x - origin.x, y = p.y - origin.y, z = p.z - origin.z;
    return !S.isAir(S.get(s, x, y, z));
  }

  // "build house 5x5 [materiale]": prima la schematica mentale, poi la costruzione
  SKILLS.build = async (o, c) => {
    const kind = String(o.shape || o.kind || "house").toLowerCase().replace("shelter", "hut");
    const w = Number(o.w || o.size) || 0, l = Number(o.l) || w;
    const blocks = (n) => Math.max(10, n);
    const wall = S.pickMaterial(String(o.block || "").toLowerCase() || null, bot().inventory.items(), blocks((w || 5) * 10), !!o.block);
    const s = S.generate({ kind: kind === "house" || kind === "hut" || kind === "tower" || kind === "wall" || kind === "platform" || kind === "pillar" || kind === "bridge" ? kind : "house", w, l, h: Number(o.h) || 0, wall, roof: o.roof, peaked: !!o.peaked });
    last = s;
    return run(s, o, c);
  };

  /** Costruisce da un file di schematica (percorso scelto nell'app). */
  SKILLS.build_file = async (o, c) => {
    const fs = require("fs");
    const path = require("path");
    let p = String(o.path || "");
    // per nome (dal modello): il file della cartella delle schematiche con più parole in comune
    if (!p && o.name && ctx.cfg.schem_dir && fs.existsSync(ctx.cfg.schem_dir)) {
      const words = String(o.name).toLowerCase().split(/[^a-z0-9àèéìòù]+/).filter((w) => w.length >= 2);
      let best = 0;
      for (const f of fs.readdirSync(ctx.cfg.schem_dir)) {
        if (!/\.(schem|schematic|litematic|nbt)$/i.test(f)) continue;
        const n = f.toLowerCase().replace(/[_-]/g, " ");
        const sc = words.filter((w) => n.includes(w)).length;
        if (sc > best) { best = sc; p = path.join(ctx.cfg.schem_dir, f); }
      }
    }
    if (!p || !fs.existsSync(p)) throw new Error(ctx.en() ? "schematic file not found" : "file della schematica non trovato");
    const s = await S.parseFile(fs.readFileSync(p), path.extname(p));
    s.name = s.name && s.name !== "schematic" ? s.name : path.basename(p, path.extname(p));
    if (s.w * s.h * s.l > 64 * 64 * 64) throw new Error(ctx.en() ? "too big for me (max 64×64×64)" : "troppo grande per me (massimo 64×64×64)");
    last = s;
    return run(s, o, c);
  };

  /** Solo il piano: punto, blocchi, mancanti, ostacoli (anteprima nella UI). */
  SKILLS.build_plan = async (o, c) => {
    if (o.path) return SKILLS.build_file({ ...o, dry: true }, c);
    return SKILLS.build({ ...o, dry: true }, c);
  };

  return { run, last: () => last };
}

module.exports = { chooseSite, obstacles, order, scaffolds, summary, planLine, isReplaceable, isTree, attach };
