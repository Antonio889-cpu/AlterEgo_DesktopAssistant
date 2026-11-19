// Crafting ricorsivo: albero delle ricette (minecraft-data), materiali mancanti da raccogliere, banco da
// lavoro e fornace quando servono, quantità calcolate; errori chiari ("mi manca …: lo trovo scavando …").
// Parte 1: logica pura (node:test). Parte 2 (`attach`): esegue il piano un passo alla volta e ripianifica
// dopo ogni passo con l'inventario vero (i drop non sono mai esattamente quelli previsti).
"use strict";

const I = require("./inventory");

// ---------- parte 1: logica pura ----------

/** Fornace: oggetto fuso → ingredienti possibili (il primo è quello da raccogliere se manca). */
const SMELT = {
  iron_ingot: ["raw_iron"], gold_ingot: ["raw_gold"], copper_ingot: ["raw_copper"], glass: ["sand", "red_sand"], stone: ["cobblestone"],
  smooth_stone: ["stone"], deepslate: ["cobbled_deepslate"], brick: ["clay_ball"], charcoal: ["oak_log", "spruce_log", "birch_log", "jungle_log", "acacia_log", "dark_oak_log", "mangrove_log", "cherry_log"],
  cooked_beef: ["beef"], cooked_porkchop: ["porkchop"], cooked_mutton: ["mutton"], cooked_chicken: ["chicken"], cooked_cod: ["cod"],
  cooked_salmon: ["salmon"], cooked_rabbit: ["rabbit"], baked_potato: ["potato"], dried_kelp: ["kelp"], netherite_scrap: ["ancient_debris"],
  smooth_sandstone: ["sandstone"], cracked_stone_bricks: ["stone_bricks"], lime_dye: ["sea_pickle"], green_dye: ["cactus"],
};

/** Combustibile: quanti oggetti fonde un pezzo. */
const FUEL = { coal: 8, charcoal: 8, coal_block: 80, blaze_rod: 12, lava_bucket: 100 };
const fuelValue = (n) => FUEL[n] || (/_planks$/.test(n) ? 1.5 : /_log$|_wood$/.test(n) ? 1.5 : n === "stick" ? 0.5 : 0);

const LOGS = ["oak_log", "spruce_log", "birch_log", "jungle_log", "acacia_log", "dark_oak_log", "mangrove_log", "cherry_log", "pale_oak_log"];

/**
 * Da dove viene un materiale grezzo (che non si fa al banco): blocco da scavare, quanti ne dà uno, piccone
 * minimo. "hunt" = da animali o mob, "farm" = coltivato: non lo fa da sola, lo dice.
 */
const RAW = {
  cobblestone: { block: "stone", tool: "wooden_pickaxe" }, cobbled_deepslate: { block: "deepslate", tool: "wooden_pickaxe", cost: 4 },
  coal: { block: "coal_ore", tool: "wooden_pickaxe" }, raw_iron: { block: "iron_ore", tool: "stone_pickaxe" }, raw_copper: { block: "copper_ore", tool: "stone_pickaxe", per: 2 },
  raw_gold: { block: "gold_ore", tool: "iron_pickaxe" }, diamond: { block: "diamond_ore", tool: "iron_pickaxe" }, redstone: { block: "redstone_ore", tool: "iron_pickaxe", per: 4 },
  lapis_lazuli: { block: "lapis_ore", tool: "stone_pickaxe", per: 4 }, emerald: { block: "emerald_ore", tool: "iron_pickaxe" },
  sand: { block: "sand" }, red_sand: { block: "red_sand" }, gravel: { block: "gravel" }, dirt: { block: "dirt" }, clay_ball: { block: "clay", per: 4 },
  flint: { block: "gravel", per: 0.2 }, obsidian: { block: "obsidian", tool: "diamond_pickaxe" }, ancient_debris: { block: "ancient_debris", tool: "diamond_pickaxe" },
  snowball: { block: "snow_block", per: 4 }, ice: { block: "ice" }, netherrack: { block: "netherrack", tool: "wooden_pickaxe" },
  andesite: { block: "andesite", tool: "wooden_pickaxe" }, diorite: { block: "diorite", tool: "wooden_pickaxe" }, granite: { block: "granite", tool: "wooden_pickaxe" },
  string: { hunt: "spider" }, leather: { hunt: "cow" }, feather: { hunt: "chicken" }, bone: { hunt: "skeleton" }, gunpowder: { hunt: "creeper" },
  white_wool: { hunt: "sheep" }, beef: { hunt: "cow" }, porkchop: { hunt: "pig" }, chicken: { hunt: "chicken" }, mutton: { hunt: "sheep" },
  ender_pearl: { hunt: "enderman" }, blaze_rod: { hunt: "blaze" }, slime_ball: { hunt: "slime" }, spider_eye: { hunt: "spider" },
  wheat: { farm: "wheat" }, sugar_cane: { block: "sugar_cane" }, apple: { block: "oak_leaves", per: 0.05 }, egg: { hunt: "chicken" },
  bamboo: { block: "bamboo", cost: 5 }, cactus: { block: "cactus" }, pumpkin: { block: "pumpkin" }, melon_slice: { block: "melon", per: 5 },
};
// i tronchi comuni prima (a parità di tutto): quercia, abete, betulla…
LOGS.forEach((l, i) => { RAW[l] = { block: l, log: true, cost: i * 0.05 + (i >= 6 ? 1 : 0) }; });

/** Ricettario da minecraft-data: nome → [{ins: {nome: n}, out, table}]. */
function makeBook(mc) {
  const book = new Map();
  const name = (id) => (mc.items[id] ? mc.items[id].name : null);
  for (const [id, list] of Object.entries(mc.recipes || {})) {
    const res = name(Number(id));
    if (!res) continue;
    for (const r of list) {
      const ins = {};
      let cells = 0, w = 0, h = 0;
      if (r.inShape) {
        h = r.inShape.length;
        for (const row of r.inShape) {
          w = Math.max(w, row.length);
          for (const x of row) if (x != null) { const n = name(Array.isArray(x) ? x[0] : x); if (n) { ins[n] = (ins[n] || 0) + 1; cells++; } }
        }
      } else if (r.ingredients) {
        for (const x of r.ingredients) { const n = name(Array.isArray(x) ? x[0] : x); if (n) { ins[n] = (ins[n] || 0) + 1; cells++; } }
        w = h = cells > 4 ? 3 : 2;
      }
      if (!cells) continue;
      const table = w > 2 || h > 2 || cells > 4;
      if (!book.has(res)) book.set(res, []);
      book.get(res).push({ ins, out: (r.result && r.result.count) || 1, table });
    }
  }
  return book;
}

/** Ricettario piccolo scritto a mano (per i test e come riserva). */
function miniBook() {
  const b = new Map();
  const add = (res, ins, out = 1, table = true) => { if (!b.has(res)) b.set(res, []); b.get(res).push({ ins, out, table }); };
  for (const l of LOGS) add(l.replace("_log", "_planks"), { [l]: 1 }, 4, false);
  for (const p of LOGS.map((l) => l.replace("_log", "_planks"))) { add("stick", { [p]: 2 }, 4, false); add("crafting_table", { [p]: 4 }, 1, false); }
  for (const p of LOGS.map((l) => l.replace("_log", "_planks"))) { add("wooden_pickaxe", { [p]: 3, stick: 2 }); add("wooden_sword", { [p]: 2, stick: 1 }); add("wooden_axe", { [p]: 3, stick: 2 }); }
  add("stone_pickaxe", { cobblestone: 3, stick: 2 }); add("stone_sword", { cobblestone: 2, stick: 1 }); add("stone_axe", { cobblestone: 3, stick: 2 });
  add("furnace", { cobblestone: 8 }); add("iron_pickaxe", { iron_ingot: 3, stick: 2 }); add("iron_sword", { iron_ingot: 2, stick: 1 });
  add("iron_helmet", { iron_ingot: 5 }); add("iron_chestplate", { iron_ingot: 8 }); add("iron_leggings", { iron_ingot: 7 }); add("iron_boots", { iron_ingot: 4 });
  add("iron_ingot", { iron_block: 1 }, 9, false); add("iron_block", { iron_ingot: 9 }); add("iron_ingot", { iron_nugget: 9 }); add("iron_nugget", { iron_ingot: 1 }, 9, false);
  add("torch", { coal: 1, stick: 1 }, 4, false); add("shield", { oak_planks: 6, iron_ingot: 1 }); add("diamond_pickaxe", { diamond: 3, stick: 2 });
  return b;
}

const isTool = (n) => !!I.toolKind(n);

/** Quanti ne ha (inventario simulato: Map nome → n). */
const have = (inv, n) => inv.get(n) || 0;
function take(inv, n, k) { inv.set(n, have(inv, n) - k); if (inv.get(n) <= 0) inv.delete(n); }
function give(inv, n, k) { inv.set(n, have(inv, n) + k); }

/**
 * Piano per avere `n` × `item` partendo da `inventory` ([{name, count}] o Map).
 *  opts: {book, tableNear, furnaceNear, prefer: [blocchi visti vicino, es. "birch_log"], maxDepth}
 * Restituisce {ok, steps: [{op, …}], missing: [{item, n, why, block?, mob?}], tree}.
 *  passi: {op:"collect", block, n, item} · {op:"craft", item, n, table} · {op:"smelt", item: ingrediente, n, out}
 * Le quantità sono già arrotondate alle ricette (3 bastoni → una ricetta da 4).
 */
function plan(target, n, inventory, opts = {}) {
  const book = opts.book || miniBook();
  const inv = new Map();
  if (inventory instanceof Map) for (const [k, v] of inventory) inv.set(k, v);
  else for (const it of inventory || []) give(inv, it.name, it.count || 1);
  const steps = [];
  const missing = [];
  const st = { table: !!opts.tableNear, furnace: !!opts.furnaceNear };
  const prefer = opts.prefer || [];
  const maxDepth = opts.maxDepth || 12;
  const tree = need(target, Math.max(1, n | 0 || 1), 0, new Set());
  // passi uguali uno dopo l'altro: uniti
  const merged = [];
  for (const s of steps) {
    const last = merged[merged.length - 1];
    if (last && last.op === s.op && (last.block || last.item) === (s.block || s.item)) last.n += s.n;
    else merged.push(s);
  }
  return { ok: missing.length === 0, steps: merged, missing, tree, inv };

  // costo stimato (in passi) per procurarsi un oggetto: serve a scegliere tra ricette alternative
  function cost(item, depth, seen) {
    if (have(inv, item) > 0) return 0;
    if (depth > 5 || seen.has(item)) return 1000;
    const raw = RAW[item];
    if (raw && raw.block) return (raw.tool && !hasPick(raw.tool) ? 3 : 1) + (raw.cost || 0) + (raw.log && !prefer.includes(item) && prefer.some((p) => p.endsWith("_log")) ? 3 : 0);
    if (raw) return 400; // caccia o coltivazione: da evitare
    if (SMELT[item]) return 2 + furnaceCost() + Math.min(...SMELT[item].map((x) => cost(x, depth + 1, new Set([...seen, item]))));
    const rs = book.get(item);
    if (!rs) return 1000;
    let best = 1000;
    for (const r of rs) {
      let c = 1;
      for (const [x, k] of Object.entries(r.ins)) c += have(inv, x) >= k ? 0 : cost(x, depth + 1, new Set([...seen, item]));
      best = Math.min(best, c);
    }
    return best;
  }

  function furnaceCost() { return st.furnace || have(inv, "furnace") > 0 ? 0 : 6; }

  function hasPick(tool) {
    const want = I.TIERS.indexOf(tool.replace("_pickaxe", ""));
    for (const [k, v] of inv) if (v > 0 && I.toolKind(k) === "pickaxe" && I.tierOf(k) >= want) return true;
    return false;
  }

  function need(item, k, depth, seen) {
    const node = { item, n: k, how: "have", kids: [] };
    // l'oggetto chiesto si fa comunque (quelli che ha già non contano)
    const got = depth === 0 ? 0 : have(inv, item);
    if (got >= k) { take(inv, item, k); return node; }
    if (got > 0) { take(inv, item, got); k -= got; node.from = got; }
    node.n = k;
    if (depth > maxDepth || seen.has(item)) { node.how = "missing"; missing.push({ item, n: k, why: "loop" }); return node; }
    const seen2 = new Set([...seen, item]);
    const raw = RAW[item];
    const rs = book.get(item) || [];
    // grezzo (da scavare, cacciare o coltivare): la ricetta si usa solo se ha già gli ingredienti
    // (raw_iron ← raw_iron_block solo se il blocco ce l'ha)
    const ready = rs.some((r) => Object.entries(r.ins).every(([x, c]) => have(inv, x) >= c));
    if (raw && raw.block && !ready) {
      if (raw.tool && !hasPick(raw.tool)) {
        node.kids.push(need(raw.tool, 1, depth + 1, seen2));
        give(inv, raw.tool, 1); // l'attrezzo resta (non si consuma nel piano)
      }
      const blocks = Math.ceil(k / (raw.per || 1));
      steps.push({ op: "collect", block: raw.block, n: blocks, item, any: raw.log ? "log" : undefined });
      node.how = "collect";
      node.block = raw.block;
      give(inv, item, Math.floor(blocks * (raw.per || 1)) - k);
      return node;
    }
    if (raw && !raw.block && !ready && !SMELT[item]) {
      node.how = "missing";
      missing.push({ item, n: k, why: raw.hunt ? "hunt" : raw.farm ? "farm" : "unknown", mob: raw.hunt, block: raw.block });
      return node;
    }
    // fornace
    if (SMELT[item] && (!rs.length || Math.min(...SMELT[item].map((x) => cost(x, 0, seen2))) + 2 + furnaceCost() <= recipeCost(rs, depth, seen2))) {
      const input = SMELT[item].slice().sort((a, b) => cost(a, 0, seen2) - cost(b, 0, seen2))[0];
      if (!st.furnace && have(inv, "furnace") < 1) { node.kids.push(need("furnace", 1, depth + 1, seen2)); give(inv, "furnace", 1); }
      st.furnace = true;
      node.kids.push(need(input, k, depth + 1, seen2));
      node.kids.push(fuel(k, depth + 1, seen2));
      steps.push({ op: "smelt", item: input, n: k, out: item });
      node.how = "smelt";
      node.input = input;
      give(inv, item, 0);
      return node;
    }
    if (!rs.length) { node.how = "missing"; missing.push({ item, n: k, why: "unknown" }); return node; }
    // ricetta più economica con quello che c'è
    const r = rs.slice().sort((a, b) => rcost(a, depth, seen2) - rcost(b, depth, seen2))[0];
    const times = Math.ceil(k / r.out);
    if (r.table && !st.table && have(inv, "crafting_table") < 1) { node.kids.push(need("crafting_table", 1, depth + 1, seen2)); give(inv, "crafting_table", 1); }
    if (r.table) st.table = true;
    for (const [x, c] of Object.entries(r.ins)) node.kids.push(need(x, c * times, depth + 1, seen2));
    steps.push({ op: "craft", item, n: times * r.out, table: r.table });
    node.how = "craft";
    node.times = times;
    give(inv, item, times * r.out - k);
    return node;
  }

  function rcost(r, depth, seen) {
    let c = 0;
    for (const [x, k] of Object.entries(r.ins)) c += have(inv, x) >= k ? 0 : cost(x, 0, seen);
    return c + (r.table && !st.table && have(inv, "crafting_table") < 1 ? 2 : 0);
  }
  function recipeCost(rs, depth, seen) { return Math.min(...rs.map((r) => rcost(r, depth, seen) + 1)); }

  function fuel(k, depth, seen) {
    // combustibile che c'è già
    let left = k;
    const node = { item: "fuel", n: k, how: "have", kids: [] };
    for (const [name, v] of [...inv]) {
      const fv = fuelValue(name);
      if (!fv || left <= 0 || isTool(name) || name === "crafting_table") continue;
      const use = Math.min(v, Math.ceil(left / fv));
      take(inv, name, use);
      left -= use * fv;
    }
    if (left > 0) {
      const log = [...inv.keys()].find((x) => LOGS.includes(x)) || prefer.find((x) => LOGS.includes(x)) || "oak_log";
      node.kids.push(need(log.replace("_log", "_planks"), Math.ceil(left / 1.5), depth, seen));
    }
    return node;
  }
}

/** Spiegazione corta del piano o di cosa manca (per la chat e le lezioni). */
function explain(p, en = false) {
  if (!p) return "";
  if (!p.ok) {
    const parts = p.missing.map((m) => {
      if (m.why === "hunt") return en ? `${m.n} ${m.item} (from ${m.mob}s)` : `${m.n} ${m.item} (dai ${m.mob})`;
      if (m.why === "farm") return en ? `${m.n} ${m.item} (needs farming)` : `${m.n} ${m.item} (va coltivato)`;
      if (m.why === "loop") return en ? `${m.item} (recipe loop)` : `${m.item} (ricetta circolare)`;
      return en ? `${m.n} ${m.item} (don't know how to get it)` : `${m.n} ${m.item} (non so come procurarlo)`;
    });
    return (en ? "missing " : "mi manca ") + parts.join(", ");
  }
  const s = p.steps.map((x) => (x.op === "collect" ? `${en ? "mine" : "scavo"} ${x.n} ${x.block}` : x.op === "smelt" ? `${en ? "smelt" : "fondo"} ${x.n} ${x.item}` : `${en ? "craft" : "creo"} ${x.n} ${x.item}`));
  return s.length ? s.join(" → ") : en ? "already have it" : "ce l'ho già";
}

/** Albero in testo ("iron_pickaxe ← 3 iron_ingot (smelt raw_iron ← mine iron_ore) + 2 stick"), per il log e la UI. */
function treeLine(node, depth = 0) {
  if (!node) return "";
  const kids = (node.kids || []).filter((k) => k.how !== "have" || (k.kids && k.kids.length)).map((k) => treeLine(k, depth + 1)).filter(Boolean);
  const how = node.how === "collect" ? `mine ${node.block}` : node.how === "smelt" ? `smelt ${node.input}` : node.how === "missing" ? "MISSING" : node.how === "craft" ? "" : "have";
  const head = `${node.n} ${node.item}${how ? ` (${how})` : ""}`;
  return kids.length && depth < 4 ? `${head} ← ${kids.join(" + ")}` : head;
}

// ---------- parte 2: le mani (Mineflayer) ----------

function attach(ctx) {
  const { SKILLS } = ctx;
  const base = { craft: SKILLS.craft, collect: SKILLS.collect };
  const bot = () => ctx.bot();
  let book = null;
  const getBook = () => (book ||= makeBook(ctx.mc()));
  const near = (name) => { const b = ctx.mc().blocksByName[name]; return !!(b && bot().findBlock({ matching: b.id, maxDistance: 24 })); };
  const nearLogs = () => {
    const mc = ctx.mc();
    const ids = LOGS.map((l) => mc.blocksByName[l] && mc.blocksByName[l].id).filter((x) => x != null);
    return [...new Set(bot().findBlocks({ matching: ids, maxDistance: 48, count: 8 }).map((p) => bot().blockAt(p).name))];
  };

  function current(target, n) {
    return plan(target, n, bot().inventory.items(), { book: getBook(), tableNear: near("crafting_table"), furnaceNear: near("furnace") || near("blast_furnace") && /ingot/.test(target), prefer: nearLogs() });
  }

  /** Fonde con qualunque combustibile buono (carbone, assi, tronchi). */
  async function smelt(input, n, c) {
    const mc = ctx.mc();
    let fb = bot().findBlock({ matching: mc.blocksByName.furnace.id, maxDistance: 32 });
    if (!fb && ctx.countItem("furnace")) fb = await ctx.placeNear("furnace", c);
    if (!fb) throw new Error(ctx.en() ? "no furnace" : "nessuna fornace");
    await ctx.goNear(fb.position, 3, c);
    const f = await bot().openFurnace(fb);
    try {
      let left = n;
      const fuels = bot().inventory.items().filter((i) => fuelValue(i.name) && !I.toolKind(i.name)).sort((a, b) => fuelValue(b.name) - fuelValue(a.name));
      let fuelNeed = n;
      for (const it of fuels) {
        if (fuelNeed <= 0) break;
        const k = Math.min(it.count, Math.ceil(fuelNeed / fuelValue(it.name)));
        await f.putFuel(it.type, null, k);
        fuelNeed -= k * fuelValue(it.name);
      }
      if (fuelNeed > 0 && !f.fuelItem()) throw new Error(ctx.en() ? "no fuel" : "niente combustibile");
      await f.putInput(mc.itemsByName[input].id, null, Math.min(n, ctx.countItem(input)));
      const t0 = Date.now();
      while (left > 0 && Date.now() - t0 < n * 10500 + 4000) {
        await ctx.sleep(1000);
        ctx.checkCancel(c);
        if (f.outputItem()) { const k = f.outputItem().count; await f.takeOutput(); left -= k; }
      }
      if (f.outputItem()) await f.takeOutput();
    } finally {
      f.close();
    }
  }

  /** "craft X n" ricorsivo: raccoglie, fonde e crea tutto quello che serve. */
  SKILLS.craft = async (o, c) => {
    const target = String(o.item || "").toLowerCase().replace("minecraft:", "");
    if (!ctx.mc().itemsByName[target]) throw new Error(ctx.en() ? `unknown item ${o.item}` : `oggetto sconosciuto ${o.item}`);
    const n = Math.max(1, Math.min(64, Number(o.n) || 1));
    const startHave = ctx.countItem(target);
    const goal = startHave + n;
    let first = null;
    for (let i = 0; i < 40; i++) {
      ctx.checkCancel(c);
      const left = goal - ctx.countItem(target);
      if (left <= 0) break;
      const p = current(target, left);
      if (!first) {
        first = p;
        ctx.ev("craft_plan", { item: target, n, ok: p.ok, steps: p.steps.length, tree: treeLine(p.tree).slice(0, 400), missing: p.missing.map((m) => m.item) });
      }
      if (!p.ok) throw new Error(explain(p, ctx.en()));
      const s = p.steps[0];
      if (!s) break;
      if (s.op === "collect") {
        const blk = s.any === "log" ? (nearLogs()[0] || "wood") : s.block;
        await base.collect({ block: blk, n: Math.min(s.n, 16) }, c);
      } else if (s.op === "smelt") {
        await smelt(s.item, s.n, c);
      } else {
        await base.craft({ item: s.item, n: s.n }, c);
      }
    }
    const got = ctx.countItem(target) - startHave;
    if (got <= 0) throw new Error(ctx.en() ? `couldn't make ${target}` : `non sono riuscita a fare ${target}`);
    return { have: ctx.countItem(target), steps: first ? first.steps.length : 0 };
  };

  SKILLS.smelt = async (o, c) => {
    const input = String(o.item || "").toLowerCase();
    const n = Math.max(1, Math.min(64, Number(o.n) || 1));
    // "smelt iron_ingot" (il risultato) va bene lo stesso
    const real = SMELT[input] ? SMELT[input].find((x) => ctx.countItem(x)) || SMELT[input][0] : input;
    if (!ctx.countItem(real)) {
      const p = current(real, n);
      if (!p.ok) throw new Error(explain(p, ctx.en()));
      await SKILLS.craft({ item: SMELT[input] ? input : real, n }, c);
      return;
    }
    await smelt(real, Math.min(n, ctx.countItem(real)), c);
  };

  /** Solo il piano, senza fare niente (per "cosa ti serve per…?"). */
  SKILLS.craft_plan = async (o) => {
    const p = current(String(o.item || "").toLowerCase(), Math.max(1, Number(o.n) || 1));
    return { ok: p.ok, plan: explain(p, ctx.en()), tree: treeLine(p.tree).slice(0, 500) };
  };

  return { current, smelt, getBook };
}

module.exports = { SMELT, FUEL, RAW, LOGS, fuelValue, makeBook, miniBook, plan, explain, treeLine, attach };
