// Inventario in ordine: cosa tenere, cosa mettere in una cassa, cosa buttare; attrezzo giusto per ogni blocco;
// armatura migliore; attrezzi consumati da riparare o rifare.
// Parte 1: logica pura (si prova con node:test). Parte 2 (`attach`): le mani, con Mineflayer.
"use strict";

// ---------- parte 1: logica pura ----------

/** Materiali dal peggiore al migliore (l'oro è debole e si consuma subito: vale poco). */
const TIERS = ["wooden", "golden", "stone", "iron", "diamond", "netherite"];
const TOOL_KINDS = ["pickaxe", "axe", "shovel", "hoe", "sword"];
const ARMOR_SLOTS = { helmet: "head", chestplate: "torso", leggings: "legs", boots: "feet" };
/** Punti armatura (wiki): elmo, corpetto, gambali, stivali. */
const ARMOR_POINTS = {
  leather: [1, 3, 2, 1], golden: [2, 5, 3, 1], chainmail: [2, 5, 4, 1], iron: [2, 6, 5, 2], diamond: [3, 8, 6, 3], netherite: [3, 8, 6, 3], turtle: [2, 0, 0, 0],
};
const ARMOR_ORDER = ["helmet", "chestplate", "leggings", "boots"];

/** Spazzatura: si butta (o si mette in una cassa) quando serve posto. */
const JUNK = new Set([
  "rotten_flesh", "poisonous_potato", "spider_eye", "dead_bush", "wheat_seeds", "beetroot_seeds", "pumpkin_seeds", "melon_seeds", "stick",
  "string", "feather", "bone", "gunpowder", "flint", "andesite", "diorite", "granite", "tuff", "gravel", "netherrack", "kelp", "seagrass",
  "lily_pad", "leather_boots", "leather_helmet", "golden_helmet", "golden_boots", "egg", "ink_sac", "glow_ink_sac", "short_grass", "fern",
]);
/** Cose preziose: non si buttano mai. */
const VALUABLE = /diamond|emerald|netherite|ancient_debris|gold_ingot|iron_ingot|raw_iron|raw_gold|raw_copper|copper_ingot|lapis|redstone|enchanted|totem|elytra|ender_pearl|blaze_rod|shulker|nether_star|trident|golden_apple|experience_bottle|book|saddle|name_tag|firework_rocket|shield|bow|crossbow|arrow|bucket|map|compass|clock|bed$/;
/** Blocchi buoni per costruire (quelli che tiene di più). */
const BUILD = /(_planks|cobblestone|cobbled_deepslate|stone_bricks|^stone$|^dirt$|sandstone|bricks$|_log$|glass$|^deepslate_bricks|terracotta|concrete$|_wool$|smooth_stone|blackstone|^andesite$|^polished_)/;
const FOOD = new Set([
  "golden_carrot", "cooked_beef", "cooked_porkchop", "cooked_mutton", "cooked_salmon", "cooked_chicken", "cooked_cod", "cooked_rabbit", "bread",
  "baked_potato", "pumpkin_pie", "mushroom_stew", "beetroot_soup", "rabbit_stew", "apple", "golden_apple", "enchanted_golden_apple", "carrot",
  "melon_slice", "sweet_berries", "glow_berries", "cookie", "beetroot", "potato", "dried_kelp", "beef", "porkchop", "mutton", "chicken", "rabbit",
  "cod", "salmon", "honey_bottle",
]);

function tierOf(name) {
  const n = String(name || "");
  if (/^chainmail_/.test(n)) return 2.5;
  if (/^leather_/.test(n)) return 0.5;
  if (/^turtle_/.test(n)) return 2.5;
  for (let i = TIERS.length - 1; i >= 0; i--) if (n.startsWith(TIERS[i] + "_")) return i;
  return -1;
}

function toolKind(name) {
  const m = /_(pickaxe|axe|shovel|hoe|sword)$/.exec(String(name || ""));
  return m ? m[1] : null;
}

function armorSlot(name) {
  const m = /_(helmet|chestplate|leggings|boots)$/.exec(String(name || ""));
  return m ? m[1] : null;
}

/** Categoria di un oggetto: tool, weapon, armor, food, valuable, block, junk, misc. */
function classify(name) {
  const n = String(name || "");
  if (n === "elytra") return "armor";
  const k = toolKind(n);
  if (k === "sword") return "weapon";
  if (k) return "tool";
  if (["bow", "crossbow", "trident", "shield"].includes(n)) return "weapon";
  if (armorSlot(n)) return "armor";
  if (FOOD.has(n) && n !== "rotten_flesh") return "food";
  if (JUNK.has(n)) return "junk";
  if (VALUABLE.test(n)) return "valuable";
  if (BUILD.test(n)) return "block";
  return "misc";
}

/** Usura: 0 = nuovo, 1 = rotto. `durabilityUsed` come in prismarine-item, `max` da minecraft-data. */
function wear(item, max) {
  const used = Number(item && item.durabilityUsed) || 0;
  if (!(max > 0)) return 0;
  return Math.max(0, Math.min(1, used / max));
}

/** Che attrezzo serve per un blocco: dal campo `material` di minecraft-data ("mineable/pickaxe") o dal nome. */
function toolForBlock(block) {
  const mat = String((block && block.material) || "");
  const name = String((block && block.name) || block || "");
  if (mat.includes("pickaxe")) return "pickaxe";
  if (mat.includes("mineable/axe")) return "axe";
  if (mat.includes("shovel")) return "shovel";
  if (mat.includes("hoe") && !mat.includes("leaves")) return "hoe";
  if (/(_log|_wood|_planks|chest|crafting_table|barrel|fence|door|sign|bookshelf)$/.test(name)) return "axe";
  if (/(stone|ore|cobble|deepslate|brick|andesite|diorite|granite|furnace|obsidian|netherrack|terracotta|basalt|tuff|calcite)/.test(name)) return "pickaxe";
  if (/(dirt|grass_block|sand|gravel|clay|soul_|snow|mud|farmland|podzol|mycelium)/.test(name)) return "shovel";
  if (/leaves|cobweb/.test(name)) return name.includes("cobweb") ? "sword" : null;
  return null;
}

/** Livello minimo di piccone per raccogliere qualcosa (null = qualunque cosa va bene). */
function minTier(blockName) {
  const n = String(blockName || "");
  if (/obsidian|ancient_debris|crying_obsidian|respawn_anchor|netherite/.test(n)) return "diamond";
  if (/diamond_ore|emerald_ore|gold_ore|redstone_ore|diamond_block|emerald_block|gold_block/.test(n) && !/nether_gold/.test(n)) return "iron";
  if (/iron_ore|lapis_ore|copper_ore|iron_block|lapis_block|copper_block|raw_iron_block/.test(n)) return "stone";
  if (/stone|ore|cobble|deepslate|brick|andesite|diorite|granite|furnace|netherrack|terracotta|basalt|tuff|calcite|sandstone|concrete$/.test(n)) return "wooden";
  return null;
}

/**
 * Attrezzo migliore per `block` tra `items` ([{name, durabilityUsed}], `maxDur(name)` → durata massima):
 * l'attrezzo giusto del livello più alto che basta e che non sta per rompersi (si risparmiano i buoni sui
 * blocchi facili: per la terra va bene una pala di pietra, il diamante si tiene per i minerali).
 */
function bestTool(items, block, maxDur = () => 0) {
  const kind = toolForBlock(block);
  if (!kind) return null;
  const need = kind === "pickaxe" ? minTier(block && block.name ? block.name : block) : null;
  const needIdx = need ? TIERS.indexOf(need) : -1;
  const ok = (items || []).filter((i) => toolKind(i.name) === kind && tierOf(i.name) >= needIdx && wear(i, maxDur(i.name)) < 0.97);
  if (!ok.length) return null;
  // minerali duri: il migliore; blocchi facili: il più modesto che c'è (il diamante non si consuma sulla terra)
  const hard = needIdx >= 2;
  const dir = hard ? -1 : 1;
  ok.sort((a, b) => dir * (tierOf(a.name) - tierOf(b.name)) || wear(a, maxDur(a.name)) - wear(b, maxDur(b.name)));
  return ok[0].name;
}

/** Un piccone abbastanza buono per `blockName`? */
function canHarvest(items, blockName) {
  const need = minTier(blockName);
  if (!need) return true;
  const idx = TIERS.indexOf(need);
  return (items || []).some((i) => toolKind(i.name) === "pickaxe" && tierOf(i.name) >= idx);
}

/** Piccone che serve per `blockName` (null se va bene la mano). */
function pickaxeFor(blockName) {
  const need = minTier(blockName);
  return need ? `${need}_pickaxe` : null;
}

function armorValue(name) {
  const slot = armorSlot(name);
  if (!slot) return name === "elytra" ? 0.1 : 0;
  const mat = name.slice(0, name.lastIndexOf("_"));
  const pts = ARMOR_POINTS[mat];
  return pts ? pts[ARMOR_ORDER.indexOf(slot)] + (mat === "netherite" ? 0.5 : 0) : 0;
}

/**
 * Armatura migliore per ogni pezzo: { helmet: "iron_helmet", … } tra indossata (`worn`) e inventario.
 * `preferElytra`: al posto del corpetto, se deve volare.
 */
function bestArmor(items, worn = {}, preferElytra = false) {
  const out = {};
  for (const slot of ARMOR_ORDER) {
    const cands = [...(items || []).map((i) => i.name), worn[slot]].filter((n) => n && armorSlot(n) === slot);
    if (slot === "chestplate" && preferElytra && [...(items || []).map((i) => i.name), worn[slot]].includes("elytra")) { out[slot] = "elytra"; continue; }
    cands.sort((a, b) => armorValue(b) - armorValue(a));
    if (cands[0]) out[slot] = cands[0];
  }
  return out;
}

/** Attrezzi e armi consumati (usura ≥ soglia): da rifare prima che si rompano. */
function worn(items, maxDur = () => 0, limit = 0.85) {
  return (items || []).filter((i) => (toolKind(i.name) || armorSlot(i.name)) && wear(i, maxDur(i.name)) >= limit).map((i) => i.name);
}

/**
 * Due attrezzi uguali consumati si uniscono nella griglia (riparazione del crafting, +5%).
 * Restituisce i nomi che si possono riparare così.
 */
function repairPairs(items, maxDur = () => 0) {
  const by = new Map();
  for (const i of items || []) {
    if (!(toolKind(i.name) || armorSlot(i.name))) continue;
    if (wear(i, maxDur(i.name)) <= 0.3) continue;
    by.set(i.name, (by.get(i.name) || 0) + 1);
  }
  return [...by.entries()].filter(([, n]) => n >= 2).map(([k]) => k);
}

/**
 * Riordino: cosa tenere, cosa mettere in cassa, cosa buttare.
 *  items: [{name, count, durabilityUsed}]; opts: {free: slot liberi, favorites: [nomi], blockStacks: quanti stack di blocchi tenere, maxDur}
 * Regole: attrezzi → il migliore per tipo (gli altri in cassa, i rotti via); armi e armatura → la migliore;
 * cibo e preziosi → sempre; blocchi → i più numerosi fino a `blockStacks` stack (il resto in cassa);
 * spazzatura → via solo se lo spazio è poco (altrimenti in cassa), ma tiene i preferiti del giocatore.
 */
function sortPlan(items, opts = {}) {
  const maxDur = opts.maxDur || (() => 0);
  const fav = new Set(opts.favorites || []);
  const free = opts.free == null ? 36 : opts.free;
  const blockStacks = opts.blockStacks == null ? 4 : opts.blockStacks;
  const keep = [], store = [], drop = [];
  const sum = new Map();
  for (const i of items || []) sum.set(i.name, (sum.get(i.name) || 0) + (i.count || 1));
  // attrezzi: il migliore per tipo
  const bestBy = {};
  for (const i of items || []) {
    const k = toolKind(i.name);
    if (!k) continue;
    if (wear(i, maxDur(i.name)) >= 0.99) { drop.push({ name: i.name, count: 1, why: "broken" }); continue; }
    if (!bestBy[k] || tierOf(i.name) > tierOf(bestBy[k])) bestBy[k] = i.name;
  }
  const seen = new Set();
  let blocks = [];
  for (const [name, count] of sum) {
    if (seen.has(name)) continue;
    seen.add(name);
    const c = classify(name);
    const k = toolKind(name);
    if (k) {
      if (drop.some((d) => d.name === name) && name !== bestBy[k]) continue;
      if (name === bestBy[k] || fav.has(name)) keep.push({ name, count });
      else store.push({ name, count, why: "worse_tool" });
      continue;
    }
    if (c === "food" || c === "valuable" || c === "weapon" || c === "armor" || fav.has(name) || name === "crafting_table" || name === "furnace" || name === "torch" || name === "coal" || name === "charcoal") {
      keep.push({ name, count });
    } else if (c === "block") {
      blocks.push({ name, count });
    } else if (c === "junk") {
      if (free <= 3) drop.push({ name, count, why: "junk" });
      else store.push({ name, count, why: "junk" });
    } else {
      keep.push({ name, count });
    }
  }
  // blocchi: i preferiti e i più numerosi, fino a `blockStacks` stack in tutto
  blocks.sort((a, b) => Number(fav.has(b.name)) - Number(fav.has(a.name)) || b.count - a.count);
  let room = blockStacks * 64;
  for (const b of blocks) {
    const k = Math.min(b.count, room);
    if (k > 0) keep.push({ name: b.name, count: k });
    if (b.count - k > 0) store.push({ name: b.name, count: b.count - k, why: "extra_blocks" });
    room -= k;
  }
  return { keep, store, drop };
}

/** Barra veloce: spada 0, piccone 1, ascia 2, pala 3, blocchi 4-5, cibo 8. Restituisce {slot: nome}. */
function hotbarLayout(items) {
  const names = [...new Set((items || []).map((i) => i.name))];
  const best = (pred) => names.filter(pred).sort((a, b) => tierOf(b) - tierOf(a))[0];
  const out = {};
  const put = (slot, n) => { if (n && !Object.values(out).includes(n)) out[slot] = n; };
  put(0, best((n) => toolKind(n) === "sword") || best((n) => toolKind(n) === "axe"));
  put(1, best((n) => toolKind(n) === "pickaxe"));
  put(2, best((n) => toolKind(n) === "axe"));
  put(3, best((n) => toolKind(n) === "shovel"));
  const sums = new Map();
  for (const i of items || []) sums.set(i.name, (sums.get(i.name) || 0) + (i.count || 1));
  const blocks = names.filter((n) => classify(n) === "block").sort((a, b) => sums.get(b) - sums.get(a));
  put(4, blocks[0]);
  put(5, blocks[1]);
  if (names.includes("bow")) put(6, "bow");
  if (names.includes("shield")) put(7, null);
  const foods = names.filter((n) => FOOD.has(n));
  put(8, foods.sort((a, b) => [...FOOD].indexOf(a) - [...FOOD].indexOf(b))[0]);
  return out;
}

/** Attrezzi base che le mancano, in ordine di priorità (secondo il materiale migliore che ha in mano). */
function missingBasics(items) {
  const names = (items || []).map((i) => i.name);
  const has = (k) => names.some((n) => toolKind(n) === k);
  const mats = names.includes("iron_ingot") ? "iron" : names.includes("cobblestone") ? "stone" : "wooden";
  const out = [];
  for (const k of ["pickaxe", "axe", "sword", "shovel"]) if (!has(k)) out.push(`${mats}_${k}`);
  return out;
}

// ---------- parte 2: le mani (Mineflayer) ----------

const EQUIP_SLOT = { helmet: "head", chestplate: "torso", leggings: "legs", boots: "feet" };

function attach(ctx) {
  const { SKILLS } = ctx;
  const bot = () => ctx.bot();
  const maxDur = (n) => (ctx.mc().itemsByName[n] && ctx.mc().itemsByName[n].maxDurability) || 0;
  const items = () => bot().inventory.items();
  const wornNow = () => {
    const b = bot();
    const s = (i) => { const it = b.inventory.slots[i]; return it ? it.name : null; };
    return { helmet: s(5), chestplate: s(6), leggings: s(7), boots: s(8) };
  };

  /** Si mette l'armatura migliore che ha. */
  async function equipArmor(preferElytra = false) {
    const want = bestArmor(items(), wornNow(), preferElytra);
    const now = wornNow();
    const put = [];
    for (const [slot, name] of Object.entries(want)) {
      if (now[slot] === name) continue;
      const it = items().find((i) => i.name === name);
      if (!it) continue;
      try { await bot().equip(it, EQUIP_SLOT[slot]); put.push(name); } catch (e) { ctx.log("armatura:", e.message); }
    }
    return put;
  }

  /** Prende in mano l'attrezzo giusto per il blocco (chiamato prima di scavare). */
  async function equipFor(block) {
    const name = bestTool(items(), block, maxDur);
    if (!name) return null;
    if (bot().heldItem && bot().heldItem.name === name) return name;
    const it = items().find((i) => i.name === name);
    if (it) await bot().equip(it, "hand");
    return name;
  }

  /** Una cassa vicina (o null). */
  function nearChest() {
    const mc = ctx.mc();
    const ids = ["chest", "barrel", "trapped_chest"].map((n) => mc.blocksByName[n] && mc.blocksByName[n].id).filter((x) => x != null);
    return bot().findBlock({ matching: ids, maxDistance: 24 });
  }

  SKILLS.armor = async () => {
    const put = await equipArmor(false);
    return { put };
  };

  SKILLS.tool = async (o) => {
    const b = bot().blockAt(new ctx.Vec3(Number(o.x), Number(o.y), Number(o.z)));
    return { held: b ? await equipFor(b) : null };
  };

  /** Riordino: butta la spazzatura se serve posto, deposita il resto in una cassa vicina (se c'è). */
  SKILLS.sort = async (o, c) => {
    const free = bot().inventory.emptySlotCount();
    const plan = sortPlan(items(), { free, favorites: ctx.favorites(), maxDur });
    let dropped = 0, stored = 0;
    for (const d of plan.drop) {
      ctx.checkCancel(c);
      const it = ctx.mc().itemsByName[d.name];
      try { await bot().toss(it.id, null, Math.min(d.count, ctx.countItem(d.name))); dropped += d.count; } catch {}
    }
    const chest = plan.store.length && o.store !== false ? nearChest() : null;
    if (chest) {
      await ctx.goNear(chest.position, 2, c);
      const w = await bot().openContainer(chest);
      try {
        for (const s of plan.store) {
          ctx.checkCancel(c);
          const it = ctx.mc().itemsByName[s.name];
          const have = ctx.countItem(s.name);
          if (!it || !have) continue;
          try { await w.deposit(it.id, null, Math.min(s.count, have)); stored += s.count; } catch {}
        }
      } finally { w.close(); }
    }
    // barra veloce in ordine
    const lay = hotbarLayout(items());
    for (const [slot, name] of Object.entries(lay)) {
      const it = items().find((i) => i.name === name);
      if (!it || it.slot === 36 + Number(slot)) continue;
      try { await bot().moveSlotItem(it.slot, 36 + Number(slot)); } catch {}
    }
    await equipArmor(false);
    return { dropped, stored, chest: !!chest };
  };

  /** Ogni tanto (senza token): armatura migliore, attrezzi consumati da rifare, spazzatura se l'inventario è pieno. */
  let lastTidy = 0;
  ctx.onTick(async () => {
    const b = bot();
    if (!b || !b.entity || ctx.busy() || !ctx.cfg.tidy || Date.now() - lastTidy < 20000) return;
    lastTidy = Date.now();
    try {
      await equipArmor(false);
      const w = worn(items(), maxDur, 0.9);
      if (w.length) ctx.ev("worn", { items: w.slice(0, 4) });
      if (b.inventory.emptySlotCount() <= 2) ctx.queue({ op: "sort", store: false });
    } catch (e) { ctx.log("ordine:", e.message); }
  });

  return { equipArmor, equipFor, nearChest };
}

module.exports = {
  TIERS, TOOL_KINDS, ARMOR_SLOTS, JUNK, FOOD, tierOf, toolKind, armorSlot, classify, wear, toolForBlock, minTier, bestTool, canHarvest, pickaxeFor,
  armorValue, bestArmor, worn, repairPairs, sortPlan, hotbarLayout, missingBasics, attach,
};
