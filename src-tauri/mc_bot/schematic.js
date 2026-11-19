// Schematica mentale: una griglia di voxel con i materiali, fatta PRIMA di costruire (anche per le richieste a
// parole: "fammi una casetta 5x5") oppure letta da un file (.schem Sponge v1-3, .schematic MCEdit, .litematic,
// .nbt strutture). Da qui: blocchi necessari contro inventario, anteprima per la UI, ordine di costruzione (build.js).
// Tutto puro: si prova con node:test. Da riga di comando: `node schematic.js file.schem` → JSON con anteprima
// e materiali (l'app lo usa per l'anteprima anche quando Alter Ego non è nel mondo).
"use strict";

// ---------- griglia ----------

/** Schematica vuota w × h × l (x, y, z). palette[0] = "air". */
function create(w, h, l) {
  w = Math.max(1, w | 0); h = Math.max(1, h | 0); l = Math.max(1, l | 0);
  return { w, h, l, palette: ["air"], states: ["air"], data: new Uint16Array(w * h * l), name: "" };
}
const idx = (s, x, y, z) => (y * s.l + z) * s.w + x;
const inside = (s, x, y, z) => x >= 0 && y >= 0 && z >= 0 && x < s.w && y < s.h && z < s.l;

/** Nome base del blocco: "minecraft:oak_stairs[facing=north]" → "oak_stairs". */
function baseName(state) {
  return String(state || "air").replace(/^minecraft:/, "").replace(/\[.*$/, "").trim() || "air";
}
function propsOf(state) {
  const m = /\[(.*)\]/.exec(String(state || ""));
  const out = {};
  if (m) for (const kv of m[1].split(",")) { const [k, v] = kv.split("="); if (k && v != null) out[k.trim()] = v.trim(); }
  return out;
}

function paletteIndex(s, state) {
  const st = String(state || "air").replace(/^minecraft:/, "");
  let i = s.states.indexOf(st);
  if (i < 0) { i = s.states.length; s.states.push(st); s.palette.push(baseName(st)); }
  return i;
}

function set(s, x, y, z, state) {
  if (!inside(s, x, y, z)) return;
  s.data[idx(s, x, y, z)] = paletteIndex(s, state);
}
function get(s, x, y, z) {
  if (!inside(s, x, y, z)) return "air";
  return s.palette[s.data[idx(s, x, y, z)]] || "air";
}
function getState(s, x, y, z) {
  if (!inside(s, x, y, z)) return "air";
  return s.states[s.data[idx(s, x, y, z)]] || "air";
}

const AIRS = new Set(["air", "cave_air", "void_air", "structure_void"]);
const isAir = (n) => AIRS.has(n);

/** Tutti i blocchi non d'aria: [{x, y, z, name, state}], dal basso. */
function blocks(s) {
  const out = [];
  for (let y = 0; y < s.h; y++) for (let z = 0; z < s.l; z++) for (let x = 0; x < s.w; x++) {
    const i = s.data[idx(s, x, y, z)];
    const n = s.palette[i];
    if (!isAir(n)) out.push({ x, y, z, name: n, state: s.states[i] });
  }
  return out;
}

// ---------- materiali ----------

/** Oggetto che serve per piazzare un blocco (null = non si piazza: aria, fluidi, portali, pezzi di altri blocchi). */
function itemFor(name, props = {}) {
  const n = baseName(name);
  if (isAir(n) || /^(water|lava|bubble_column|fire|soul_fire|nether_portal|end_portal|end_gateway|moving_piston|piston_head|frosted_ice|light|barrier|structure_block|jigsaw|command_block)$/.test(n)) return null;
  // metà superiore di porte, letti e piante alte: non conta (si piazza con l'altra metà)
  if (props.half === "upper" || props.part === "head") return null;
  const map = {
    wall_torch: "torch", soul_wall_torch: "soul_torch", redstone_wall_torch: "redstone_torch", redstone_wire: "redstone", tripwire: "string",
    carrots: "carrot", potatoes: "potato", beetroots: "beetroot_seeds", wheat: "wheat_seeds", cocoa: "cocoa_beans", sweet_berry_bush: "sweet_berries",
    bamboo_sapling: "bamboo", kelp_plant: "kelp", tall_seagrass: "seagrass", cave_vines: "glow_berries", cave_vines_plant: "glow_berries",
    twisting_vines_plant: "twisting_vines", weeping_vines_plant: "weeping_vines", melon_stem: "melon_seeds", pumpkin_stem: "pumpkin_seeds",
    attached_melon_stem: "melon_seeds", attached_pumpkin_stem: "pumpkin_seeds", farmland: "dirt", dirt_path: "dirt", big_dripleaf_stem: null,
    powder_snow: "powder_snow_bucket", tall_grass: "short_grass", large_fern: "fern", water_cauldron: "cauldron", lava_cauldron: "cauldron",
  };
  if (n in map) return map[n];
  if (/_wall_sign$/.test(n)) return n.replace("_wall_sign", "_sign");
  if (/_wall_hanging_sign$/.test(n)) return n.replace("_wall_hanging_sign", "_hanging_sign");
  if (/_wall_banner$/.test(n)) return n.replace("_wall_banner", "_banner");
  if (/_wall_(head|skull)$/.test(n)) return n.replace("_wall_", "_");
  if (/^potted_/.test(n)) return "flower_pot";
  return n;
}

/** Quanti oggetti servono: {oggetto: n} (le lastre doppie contano 2). `skipped`: blocchi che non si piazzano. */
function materials(s) {
  const need = {};
  const skipped = {};
  for (const b of blocks(s)) {
    const props = propsOf(b.state);
    const it = itemFor(b.name, props);
    if (!it) { if (props.half !== "upper" && props.part !== "head") skipped[b.name] = (skipped[b.name] || 0) + 1; continue; }
    const k = /_slab$/.test(b.name) && props.type === "double" ? 2 : 1;
    need[it] = (need[it] || 0) + k;
  }
  return { need, skipped };
}

/** Confronto con l'inventario: {have: {n}, missing: {n}, ok}. */
function diff(need, items) {
  const inv = {};
  for (const i of items || []) inv[i.name] = (inv[i.name] || 0) + (i.count || 1);
  const missing = {}, have = {};
  for (const [k, n] of Object.entries(need)) {
    const h = Math.min(n, inv[k] || 0);
    if (h) have[k] = h;
    if (n - h > 0) missing[k] = n - h;
  }
  return { have, missing, ok: Object.keys(missing).length === 0 };
}

// ---------- costruzioni a parole ----------

/** Materiali di ripiego: se non ha quello scelto, usa quello di cui ha di più tra questi. */
const WALLS = ["oak_planks", "spruce_planks", "birch_planks", "cobblestone", "stone_bricks", "cobbled_deepslate", "dark_oak_planks", "jungle_planks", "acacia_planks", "cherry_planks", "mangrove_planks", "bricks", "sandstone", "dirt"];

/**
 * Schematica da una richiesta: {kind: house|hut|tower|wall|platform|pillar|bridge, w, l, h, wall, roof, floor, door, windows, peaked}.
 * Misure ragionevoli (3-15 di lato, 1-20 di altezza). La casa ha pavimento, pareti, porta, finestre, tetto.
 */
function generate(spec) {
  const k = String(spec.kind || "house").toLowerCase();
  const clampI = (v, a, b, d) => { const n = Math.round(Number(v)); return Number.isFinite(n) && n > 0 ? Math.max(a, Math.min(b, n)) : d; };
  const wall = spec.wall || "oak_planks";
  const roof = spec.roof || wall;
  const floor = spec.floor || (wall.endsWith("_planks") ? "cobblestone" : wall);
  if (k === "pillar") {
    const h = clampI(spec.h, 1, 20, 5);
    const s = create(1, h, 1);
    for (let y = 0; y < h; y++) set(s, 0, y, 0, wall);
    s.name = `pillar ${h}`;
    return s;
  }
  if (k === "wall") {
    const w = clampI(spec.w, 2, 15, 5), h = clampI(spec.h, 1, 8, 3);
    const s = create(w, h, 1);
    for (let y = 0; y < h; y++) for (let x = 0; x < w; x++) set(s, x, y, 0, wall);
    s.name = `wall ${w}x${h}`;
    return s;
  }
  if (k === "platform" || k === "floor") {
    const w = clampI(spec.w, 2, 15, 5), l = clampI(spec.l || spec.w, 2, 15, w);
    const s = create(w, 1, l);
    for (let z = 0; z < l; z++) for (let x = 0; x < w; x++) set(s, x, 0, z, floor === "cobblestone" && wall.endsWith("_planks") ? wall : floor);
    s.name = `platform ${w}x${l}`;
    return s;
  }
  if (k === "bridge") {
    const l = clampI(spec.l || spec.w, 3, 30, 10);
    const s = create(3, 2, l);
    for (let z = 0; z < l; z++) {
      for (let x = 0; x < 3; x++) set(s, x, 0, z, wall);
      set(s, 0, 1, z, "oak_fence"); set(s, 2, 1, z, "oak_fence");
    }
    s.name = `bridge ${l}`;
    return s;
  }
  if (k === "tower") {
    const w = clampI(spec.w, 3, 7, 3), h = clampI(spec.h, 3, 20, 8);
    const s = create(w, h + 1, w);
    for (let y = 0; y < h; y++) for (let z = 0; z < w; z++) for (let x = 0; x < w; x++) {
      const edge = x === 0 || z === 0 || x === w - 1 || z === w - 1;
      const door = z === 0 && x === Math.floor(w / 2) && y >= 1 && y <= 2;
      if (y === 0) set(s, x, y, z, floor);
      else if (edge && !door) set(s, x, y, z, wall);
    }
    // scala a pioli dentro, sulla parete di fondo, e tetto
    if (w >= 3) for (let y = 1; y < h; y++) set(s, Math.floor(w / 2), y, w - 2, "ladder[facing=north]");
    for (let z = 0; z < w; z++) for (let x = 0; x < w; x++) if (!(x === Math.floor(w / 2) && z === w - 2)) set(s, x, h, z, roof);
    s.name = `tower ${w}x${w} h${h}`;
    return s;
  }
  // casa / casetta / capanna
  const w = clampI(spec.w, 3, 15, 5), l = clampI(spec.l || spec.w, 3, 15, w);
  const h = clampI(spec.h, 2, 6, k === "hut" ? 2 : 3);
  const peaked = !!spec.peaked && Math.min(w, l) >= 5;
  const roofH = peaked ? Math.ceil(Math.min(w, l) / 2) : 1;
  const s = create(w, 1 + h + roofH, l);
  const midX = Math.floor(w / 2);
  for (let z = 0; z < l; z++) for (let x = 0; x < w; x++) set(s, x, 0, z, floor);
  for (let y = 1; y <= h; y++) {
    for (let z = 0; z < l; z++) for (let x = 0; x < w; x++) {
      const edge = x === 0 || z === 0 || x === w - 1 || z === l - 1;
      if (!edge) continue;
      const corner = (x === 0 || x === w - 1) && (z === 0 || z === l - 1);
      if (spec.door !== false && z === 0 && x === midX && y <= 2) {
        set(s, x, y, z, y === 1 ? "oak_door[facing=south,half=lower]" : "oak_door[facing=south,half=upper]");
        continue;
      }
      // finestre: in mezzo alle pareti laterali e di fondo, all'altezza degli occhi
      const win = spec.windows !== false && y === 2 && !corner && ((x === 0 || x === w - 1) && z === Math.floor(l / 2) || (z === l - 1 && x === midX));
      set(s, x, y, z, win ? "glass_pane" : corner && wall.endsWith("_planks") ? wall.replace("_planks", "_log") : wall);
    }
  }
  if (!peaked) {
    for (let z = 0; z < l; z++) for (let x = 0; x < w; x++) set(s, x, h + 1, z, roof);
  } else {
    for (let i = 0; i < roofH; i++) {
      for (let z = i; z < l - i; z++) for (let x = i; x < w - i; x++) {
        const edge = x === i || z === i || x === w - 1 - i || z === l - 1 - i;
        if (edge || i === roofH - 1) set(s, x, h + 1 + i, z, roof);
      }
    }
  }
  // una torcia dentro (se c'è posto)
  if (w >= 4 && l >= 4) set(s, 1, 1, l - 2, "torch");
  s.name = `${k === "hut" ? "hut" : "house"} ${w}x${l}`;
  return s;
}

/**
 * Sceglie il materiale delle pareti: `want` se ne ha abbastanza (o se il giocatore l'ha chiesto: `keep`),
 * altrimenti quello di cui ha di più; se non ha niente, `want` o assi di quercia (da procurarsi).
 */
function pickMaterial(want, items, need = 0, keep = false) {
  if (want && keep) return want;
  const inv = {};
  for (const i of items || []) inv[i.name] = (inv[i.name] || 0) + (i.count || 1);
  if (want && (inv[want] || 0) >= need) return want;
  const best = WALLS.filter((m) => inv[m]).sort((a, b) => inv[b] - inv[a])[0];
  return best || want || "oak_planks";
}

// ---------- file ----------

const toU = (b) => b & 0xff;

/** long di prismarine-nbt ([alto, basso] con segno) o BigInt → BigInt senza segno a 64 bit. */
function toBig(v) {
  if (typeof v === "bigint") return BigInt.asUintN(64, v);
  if (Array.isArray(v)) return BigInt.asUintN(64, (BigInt(v[0] | 0) << 32n) | BigInt(v[1] >>> 0));
  return BigInt.asUintN(64, BigInt(v || 0));
}

function stateString(entry) {
  if (typeof entry === "string") return entry;
  const name = String(entry.Name || entry.name || "air").replace(/^minecraft:/, "");
  const p = entry.Properties || entry.properties;
  if (p && Object.keys(p).length) return `${name}[${Object.entries(p).map(([k, v]) => `${k}=${v}`).join(",")}]`;
  return name;
}

/** Varint senza segno (Sponge BlockData). */
function readVarints(bytes, n) {
  const out = new Array(n);
  let i = 0, p = 0;
  while (i < n && p < bytes.length) {
    let v = 0, shift = 0, b;
    do { b = toU(bytes[p++]); v |= (b & 0x7f) << shift; shift += 7; } while (b & 0x80 && p < bytes.length);
    out[i++] = v;
  }
  for (; i < n; i++) out[i] = 0;
  return out;
}

/** Dall'NBT già semplificato (prismarine-nbt `simplify`) alla schematica. `legacy`: mappa "id:meta" → stato (MCEdit). */
function fromNbt(root, opts = {}) {
  let r = root || {};
  if (r.Schematic && typeof r.Schematic === "object") r = r.Schematic; // Sponge v3
  const ext = String(opts.ext || "").toLowerCase();
  // Litematica
  if (r.Regions && typeof r.Regions === "object") return fromLitematic(r);
  // struttura del gioco (.nbt)
  if (Array.isArray(r.size) && (Array.isArray(r.palette) || Array.isArray(r.palettes)) && Array.isArray(r.blocks)) return fromStructure(r);
  // Sponge (.schem): Palette + BlockData (v1/v2) oppure Blocks.Palette + Blocks.Data (v3)
  const pal = r.Palette || (r.Blocks && r.Blocks.Palette);
  const bdata = r.BlockData || (r.Blocks && r.Blocks.Data);
  if (pal && bdata) {
    const s = create(r.Width, r.Height, r.Length);
    const byIdx = [];
    for (const [state, i] of Object.entries(pal)) byIdx[i] = state;
    const vals = readVarints(bdata, s.w * s.h * s.l);
    // ordine Sponge: x + z*W + y*W*L (uguale al nostro)
    for (let y = 0; y < s.h; y++) for (let z = 0; z < s.l; z++) for (let x = 0; x < s.w; x++) {
      const st = byIdx[vals[x + z * s.w + y * s.w * s.l]];
      if (st && !isAir(baseName(st))) set(s, x, y, z, st);
    }
    s.name = (r.Metadata && r.Metadata.Name) || opts.name || "schematic";
    return s;
  }
  // MCEdit (.schematic): Blocks + Data, id numerici (vecchie versioni)
  if (r.Blocks && r.Data && r.Width != null) {
    const s = create(r.Width, r.Height, r.Length);
    const legacy = opts.legacy || {};
    const add = r.AddBlocks || r.Add || null;
    for (let y = 0; y < s.h; y++) for (let z = 0; z < s.l; z++) for (let x = 0; x < s.w; x++) {
      const i = (y * s.l + z) * s.w + x;
      let id = toU(r.Blocks[i]);
      if (add) id |= ((toU(add[i >> 1]) >> ((i & 1) ? 0 : 4)) & 0xf) << 8;
      if (!id) continue;
      const meta = toU(r.Data[i]) & 0xf;
      const st = legacy[`${id}:${meta}`] || legacy[`${id}:0`];
      if (st) set(s, x, y, z, st);
      else set(s, x, y, z, "stone"); // id sconosciuto: segnaposto pieno
    }
    s.name = opts.name || "schematic";
    return s;
  }
  throw new Error(ext ? `formato ${ext} non riconosciuto` : "schematica non riconosciuta");
}

function fromStructure(r) {
  const [w, h, l] = r.size.map((x) => Number(x) | 0);
  const s = create(w, h, l);
  const pal = (Array.isArray(r.palette) ? r.palette : r.palettes[0]).map(stateString);
  for (const b of r.blocks) {
    const st = pal[b.state];
    if (!st || isAir(baseName(st))) continue;
    const [x, y, z] = b.pos;
    set(s, x, y, z, st);
  }
  s.name = "structure";
  return s;
}

function fromLitematic(r) {
  const regs = Object.entries(r.Regions);
  // ingombro totale (le regioni possono avere dimensioni negative)
  let min = [Infinity, Infinity, Infinity], max = [-Infinity, -Infinity, -Infinity];
  const parts = regs.map(([name, g]) => {
    const pos = [g.Position.x, g.Position.y, g.Position.z].map(Number);
    const size = [g.Size.x, g.Size.y, g.Size.z].map(Number);
    const lo = pos.map((p, i) => (size[i] < 0 ? p + size[i] + 1 : p));
    const abs = size.map(Math.abs);
    for (let i = 0; i < 3; i++) { min[i] = Math.min(min[i], lo[i]); max[i] = Math.max(max[i], lo[i] + abs[i] - 1); }
    return { name, g, lo, abs };
  });
  const s = create(max[0] - min[0] + 1, max[1] - min[1] + 1, max[2] - min[2] + 1);
  for (const { g, lo, abs } of parts) {
    const pal = (g.BlockStatePalette || []).map(stateString);
    const bits = Math.max(2, Math.ceil(Math.log2(Math.max(1, pal.length))));
    const longs = (g.BlockStates || []).map(toBig);
    const mask = (1n << BigInt(bits)) - 1n;
    const [sx, sy, sz] = abs;
    const total = sx * sy * sz;
    for (let i = 0; i < total; i++) {
      const bit = i * bits;
      const li = Math.floor(bit / 64), off = bit % 64;
      if (li >= longs.length) break;
      let v = (longs[li] >> BigInt(off)) & mask;
      if (off + bits > 64 && li + 1 < longs.length) v |= (longs[li + 1] << BigInt(64 - off)) & mask;
      const st = pal[Number(v)];
      if (!st || isAir(baseName(st))) continue;
      const x = i % sx, z = Math.floor(i / sx) % sz, y = Math.floor(i / (sx * sz));
      set(s, lo[0] - min[0] + x, lo[1] - min[1] + y, lo[2] - min[2] + z, st);
    }
  }
  s.name = (r.Metadata && r.Metadata.Name) || "litematic";
  return s;
}

/** Legge un file (Buffer) con prismarine-nbt (gzip o no). */
async function parseFile(buf, ext = "", legacy = null) {
  const nbt = require("prismarine-nbt");
  const { parsed } = await nbt.parse(buf);
  const root = nbt.simplify(parsed);
  if (!legacy && /schematic$/i.test(ext)) {
    try { legacy = require("minecraft-data").legacy.pc.blocks; } catch {}
  }
  return fromNbt(root, { ext, legacy });
}

// ---------- anteprima ----------

/** Anteprima compatta per la UI: {w, h, l, name, palette, blocks: [[x,y,z,i]…] (al massimo `max`), truncated}. */
function preview(s, max = 6000) {
  const out = [];
  const used = new Map();
  let truncated = false;
  // solo i blocchi visibili (almeno una faccia verso l'aria): l'interno pieno non si vede
  const air = (x, y, z) => isAir(get(s, x, y, z));
  for (const b of blocks(s)) {
    if (!(air(b.x + 1, b.y, b.z) || air(b.x - 1, b.y, b.z) || air(b.x, b.y + 1, b.z) || air(b.x, b.y - 1, b.z) || air(b.x, b.y, b.z + 1) || air(b.x, b.y, b.z - 1))) continue;
    if (out.length >= max) { truncated = true; break; }
    if (!used.has(b.name)) used.set(b.name, used.size);
    out.push([b.x, b.y, b.z, used.get(b.name)]);
  }
  return { w: s.w, h: s.h, l: s.l, name: s.name, palette: [...used.keys()], blocks: out, truncated };
}

module.exports = { create, set, get, getState, blocks, baseName, propsOf, isAir, itemFor, materials, diff, generate, pickMaterial, WALLS, fromNbt, parseFile, preview, toBig, readVarints };

// riga di comando: anteprima e materiali di un file (per l'app)
if (require.main === module) {
  const file = process.argv[2];
  const fs = require("fs");
  const path = require("path");
  (async () => {
    try {
      const s = await parseFile(fs.readFileSync(file), path.extname(file));
      const m = materials(s);
      process.stdout.write(JSON.stringify({ ok: true, size: [s.w, s.h, s.l], name: s.name || path.basename(file), blocks: blocks(s).length, need: m.need, skipped: m.skipped, preview: preview(s) }) + "\n");
    } catch (e) {
      process.stdout.write(JSON.stringify({ ok: false, err: String(e && e.message ? e.message : e) }) + "\n");
    }
  })();
}
