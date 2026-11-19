// Test della logica pura delle parti nuove (inventario, crafting, osservazione, schematiche, costruzione, duelli).
// Niente rete: node --test test/
"use strict";
const test = require("node:test");
const assert = require("node:assert");
const fs = require("fs");
const path = require("path");
const zlib = require("zlib");
const I = require("../inventory");
const C = require("../craft");
const O = require("../observe");
const S = require("../schematic");
const B = require("../build");
const P = require("../pvp");

let mcData = null;
try { mcData = require("minecraft-data")("1.21.4"); } catch {}

// ---------- inventario ----------

test("inventario: categorie, attrezzo giusto, armatura migliore", () => {
  assert.strictEqual(I.classify("iron_pickaxe"), "tool");
  assert.strictEqual(I.classify("diamond_sword"), "weapon");
  assert.strictEqual(I.classify("rotten_flesh"), "junk");
  assert.strictEqual(I.classify("cooked_beef"), "food");
  assert.strictEqual(I.classify("oak_planks"), "block");
  assert.strictEqual(I.classify("diamond"), "valuable");
  assert.strictEqual(I.toolForBlock({ name: "stone", material: "mineable/pickaxe" }), "pickaxe");
  assert.strictEqual(I.toolForBlock("oak_log"), "axe");
  assert.strictEqual(I.toolForBlock("sand"), "shovel");
  const inv = [{ name: "wooden_pickaxe" }, { name: "iron_pickaxe" }, { name: "stone_shovel" }, { name: "diamond_shovel" }];
  assert.strictEqual(I.bestTool(inv, { name: "diamond_ore", material: "mineable/pickaxe" }), "iron_pickaxe");
  assert.strictEqual(I.bestTool(inv, { name: "dirt", material: "mineable/shovel" }), "stone_shovel", "sulla terra risparmia il diamante");
  assert.strictEqual(I.bestTool([{ name: "wooden_pickaxe" }], { name: "iron_ore", material: "mineable/pickaxe" }), null, "il legno non basta per il ferro");
  // quasi rotto: scartato
  assert.strictEqual(I.bestTool([{ name: "iron_pickaxe", durabilityUsed: 249 }, { name: "stone_pickaxe" }], { name: "stone" }, (n) => (n === "iron_pickaxe" ? 250 : 131)), "stone_pickaxe");
  assert.ok(I.canHarvest([{ name: "stone_pickaxe" }], "iron_ore") && !I.canHarvest([{ name: "stone_pickaxe" }], "diamond_ore"));
  assert.strictEqual(I.pickaxeFor("diamond_ore"), "iron_pickaxe");
  const a = I.bestArmor([{ name: "iron_chestplate" }, { name: "leather_helmet" }, { name: "golden_boots" }, { name: "elytra" }], { chestplate: "leather_chestplate", boots: "iron_boots" });
  assert.deepStrictEqual(a, { helmet: "leather_helmet", chestplate: "iron_chestplate", boots: "iron_boots" });
  assert.strictEqual(I.bestArmor([{ name: "elytra" }], { chestplate: "iron_chestplate" }, true).chestplate, "elytra");
  assert.deepStrictEqual(I.worn([{ name: "iron_sword", durabilityUsed: 240 }, { name: "stone_axe", durabilityUsed: 2 }], (n) => (n === "iron_sword" ? 250 : 131)), ["iron_sword"]);
  assert.deepStrictEqual(I.repairPairs([{ name: "iron_axe", durabilityUsed: 200 }, { name: "iron_axe", durabilityUsed: 150 }], () => 250), ["iron_axe"]);
});

test("inventario: riordino e barra veloce", () => {
  const items = [
    { name: "wooden_pickaxe", count: 1 }, { name: "stone_pickaxe", count: 1 }, { name: "rotten_flesh", count: 12 }, { name: "cobblestone", count: 300 },
    { name: "dirt", count: 64 }, { name: "bread", count: 5 }, { name: "diamond", count: 2 }, { name: "iron_sword", count: 1 }, { name: "andesite", count: 20 },
  ];
  const p = I.sortPlan(items, { free: 2, blockStacks: 4, favorites: ["andesite"] });
  const names = (l) => l.map((x) => x.name);
  assert.ok(names(p.keep).includes("stone_pickaxe") && names(p.store).includes("wooden_pickaxe"), "il piccone peggiore va in cassa");
  assert.ok(names(p.drop).includes("rotten_flesh"), "poco spazio: spazzatura via");
  assert.ok(names(p.keep).includes("andesite"), "i preferiti del giocatore restano");
  assert.ok(names(p.keep).includes("bread") && names(p.keep).includes("diamond"));
  const keptBlocks = p.keep.filter((x) => I.classify(x.name) === "block").reduce((a, x) => a + x.count, 0);
  assert.ok(keptBlocks <= 256, "al massimo 4 stack di blocchi");
  assert.ok(p.store.some((x) => x.name === "cobblestone" && x.why === "extra_blocks"));
  const roomy = I.sortPlan(items, { free: 20 });
  assert.ok(names(roomy.store).includes("rotten_flesh") && !names(roomy.drop).includes("rotten_flesh"), "con spazio la spazzatura va in cassa");
  const h = I.hotbarLayout(items);
  assert.strictEqual(h[0], "iron_sword");
  assert.strictEqual(h[1], "stone_pickaxe");
  assert.strictEqual(h[8], "bread");
  assert.strictEqual(h[4], "cobblestone");
  assert.deepStrictEqual(I.missingBasics([{ name: "cobblestone" }, { name: "stone_pickaxe" }]), ["stone_axe", "stone_sword", "stone_shovel"]);
});

// ---------- crafting ----------

test("crafting ricorsivo con il ricettario piccolo", () => {
  const p = C.plan("stone_pickaxe", 1, [], { book: C.miniBook() });
  assert.ok(p.ok, C.explain(p));
  const ops = p.steps.map((s) => `${s.op} ${s.block || s.item}`);
  // tronchi → assi → banco → bastoni → piccone di legno → pietra → piccone di pietra
  assert.ok(ops.indexOf("collect oak_log") < ops.indexOf("craft crafting_table"), ops.join(" | "));
  assert.ok(ops.indexOf("craft wooden_pickaxe") < ops.indexOf("collect stone"), "serve il piccone di legno per la pietra");
  assert.strictEqual(ops[ops.length - 1], "craft stone_pickaxe");
  // con il banco vicino e i materiali: un passo solo
  const q = C.plan("stone_pickaxe", 1, [{ name: "cobblestone", count: 3 }, { name: "stick", count: 2 }], { book: C.miniBook(), tableNear: true });
  assert.deepStrictEqual(q.steps, [{ op: "craft", item: "stone_pickaxe", n: 1, table: true }]);
  // quantità arrotondate alle ricette: 3 bastoni → una ricetta da 4
  const s = C.plan("stick", 3, [{ name: "oak_planks", count: 2 }], { book: C.miniBook() });
  assert.deepStrictEqual(s.steps, [{ op: "craft", item: "stick", n: 4, table: false }]);
  // ferro: fornace + piccone di pietra + fusione
  const f = C.plan("iron_pickaxe", 1, [{ name: "stone_pickaxe", count: 1 }, { name: "stick", count: 2 }, { name: "coal", count: 3 }], { book: C.miniBook(), tableNear: true });
  assert.ok(f.ok);
  const fo = f.steps.map((x) => x.op + " " + (x.block || x.item));
  assert.ok(fo.includes("craft furnace") && fo.includes("collect iron_ore") && fo.includes("smelt raw_iron"), fo.join(" | "));
  assert.ok(!fo.includes("craft stone_pickaxe"), "il piccone di pietra c'è già");
  // pezzi che non si fanno: errore chiaro
  const m = C.plan("leather_helmet", 1, [], { book: new Map([["leather_helmet", [{ ins: { leather: 5 }, out: 1, table: true }]]]), tableNear: true });
  assert.ok(!m.ok && m.missing[0].item === "leather" && m.missing[0].why === "hunt");
  assert.match(C.explain(m), /leather.*cow/);
  assert.match(C.explain(m, true), /^missing 5 leather/);
  assert.match(C.treeLine(f.tree), /iron_pickaxe ← .*iron_ingot \(smelt raw_iron\)/);
});

test("crafting con le ricette vere (minecraft-data)", { skip: !mcData }, () => {
  const book = C.makeBook(mcData);
  assert.ok(book.get("crafting_table").length >= 8, "un banco per ogni tipo di asse");
  assert.ok(book.get("oak_planks").some((r) => !r.table), "le assi si fanno a mano");
  assert.ok(book.get("iron_pickaxe")[0].table, "il piccone vuole il banco");
  // con tronchi di betulla usa le assi di betulla
  const p = C.plan("wooden_pickaxe", 1, [{ name: "birch_log", count: 3 }], { book });
  assert.ok(p.ok && p.steps.every((s) => s.op === "craft") && p.steps.some((s) => s.item === "birch_planks"), C.explain(p));
  // da zero: un piano completo per il piccone di ferro, senza blocchi di ferro grezzo
  const q = C.plan("iron_pickaxe", 1, [], { book });
  assert.ok(q.ok, C.explain(q));
  assert.ok(!q.steps.some((s) => /raw_iron_block|iron_block/.test(s.item || "")));
  // il pane vuole il grano coltivato
  const b = C.plan("bread", 1, [], { book });
  assert.ok(!b.ok && b.missing.some((x) => x.item === "wheat" && x.why === "farm"));
  // con il blocco di ferro in tasca lo usa (9 lingotti)
  const c = C.plan("iron_chestplate", 1, [{ name: "iron_block", count: 1 }], { book, tableNear: true });
  assert.deepStrictEqual(c.steps.map((s) => s.item), ["iron_ingot", "iron_chestplate"]);
});

// ---------- osservazione ----------

function feed(acc, tr, list) {
  const out = [];
  for (const s of list) out.push(...O.sample(acc, tr, s));
  return out;
}

test("osservazione: movimenti, salti, elytra", () => {
  const acc = O.newAcc(), tr = O.newTrack();
  const base = { onGround: true, sneaking: false, sprinting: false, elytra: false, held: "iron_sword", armor: { chestplate: "elytra", helmet: "iron_helmet" } };
  const samples = [];
  let t = 0, y = 64;
  for (let i = 0; i < 10; i++) samples.push({ ...base, t: (t += 0.2), pos: { x: i * 1.1, y, z: 0 }, sprinting: true });
  // salto e poi elytra (decollo dal salto)
  samples.push({ ...base, t: (t += 0.2), pos: { x: 11, y: 64.8, z: 0 }, onGround: false });
  samples.push({ ...base, t: (t += 0.2), pos: { x: 12, y: 65.5, z: 0 }, onGround: false, elytra: true, pitch: -0.2 });
  for (let i = 0; i < 20; i++) samples.push({ ...base, t: (t += 0.2), pos: { x: 13 + i * 5, y: 65 - i * 0.3, z: 0 }, onGround: false, elytra: true, pitch: -0.25 });
  const ev = feed(acc, tr, samples);
  O.rocket(acc, tr);
  // atterraggio morbido
  ev.push(...O.sample(acc, tr, { ...base, t: (t += 0.2), pos: { x: 115, y: 59, z: 0 }, onGround: true, elytra: false }));
  assert.ok(ev.some((e) => e.kind === "jump"));
  assert.deepStrictEqual(ev.find((e) => e.kind === "takeoff"), { kind: "takeoff", how: "jump" });
  assert.ok(ev.some((e) => e.kind === "land" && !e.hard));
  const s = O.summary(acc);
  assert.ok(s.sprint > 1.5 && s.jumps === 1 && s.armor.chestplate === "elytra" && s.armor.helmet === "iron_helmet");
  assert.ok(s.fly.n === 1 && s.fly.rockets === 1 && s.fly.pitch < 0 && s.fly.t > 3, JSON.stringify(s.fly));
  assert.ok(Object.keys(s.places).length >= 1, "dove passa il tempo");
  const st = O.flightStyle(s.fly);
  assert.ok(st.takeoff === "jump" && st.pitch < 0 && st.rocketEvery >= 2);
  // senza dati: valori sicuri
  assert.deepStrictEqual(O.flightStyle(null), { pitch: -0.15, rocketEvery: 6, takeoff: "jump", cruise: "low" });
  assert.strictEqual(O.summary(O.newAcc()), null, "niente visto: niente riassunto");
});

test("osservazione: crafting dedotto e blocchi", () => {
  const acc = O.newAcc(), tr = O.newTrack();
  const st = (t, extra) => ({ t, pos: { x: 0, y: 64, z: 0 }, onGround: true, armor: {}, held: "oak_planks", ...extra });
  // fermo davanti al banco per un secondo, poi in mano un piccone nuovo ⇒ l'ha fatto lui
  const ev = feed(acc, tr, [st(0), st(0.3, { lookTable: true }), st(0.7, { lookTable: true }), st(1.2, { lookTable: true }), st(2, { held: "stone_pickaxe" })]);
  assert.deepStrictEqual(ev.filter((e) => e.kind === "crafted"), [{ kind: "crafted", item: "stone_pickaxe" }]);
  // oggetto già visto o raccolto (non si fa al banco): niente
  const ev2 = feed(acc, tr, [st(3, { held: "oak_planks", lookTable: true }), st(4, { held: "oak_planks", lookTable: true }), st(5, { held: "diamond" })]);
  assert.ok(!ev2.some((e) => e.kind === "crafted"));
  // lontano dal banco: niente
  const acc2 = O.newAcc(), tr2 = O.newTrack();
  assert.ok(!feed(acc2, tr2, [st(0), st(1), st(2, { held: "iron_sword" })]).some((e) => e.kind === "crafted"));
  assert.strictEqual(O.blockChange(acc, "air", "oak_planks"), "placed");
  assert.strictEqual(O.blockChange(acc, "oak_log", "air"), "broken");
  assert.strictEqual(O.blockChange(acc, "stone", "stone"), null);
  assert.deepStrictEqual(O.summary(acc).crafted, { stone_pickaxe: 1 });
  // idee dalle abitudini: armatura come la sua, raccoglie quello che usa
  const ideas = O.ideas({ armor: "iron", gather: ["oak_log"], tools: [] }, [{ name: "iron_ingot", count: 9 }]);
  assert.deepStrictEqual(ideas[0], { op: "craft", item: "iron_chestplate", n: 1, why: "copy_armor" });
  assert.ok(ideas.some((i) => i.op === "collect" && i.block === "oak_log"));
  assert.ok(!O.ideas({ armor: "iron" }, [{ name: "iron_chestplate", count: 1 }, { name: "iron_ingot", count: 2 }]).length, "senza materiale non ci prova");
});

// ---------- schematiche ----------

test("schematica a parole: casetta 5x5 con porta, finestre e tetto", () => {
  const s = S.generate({ kind: "house", w: 5, l: 5, wall: "oak_planks" });
  assert.deepStrictEqual([s.w, s.l], [5, 5]);
  assert.strictEqual(S.get(s, 2, 1, 0), "oak_door", "porta in mezzo davanti");
  assert.strictEqual(S.get(s, 0, 2, 2), "glass_pane", "finestra di lato");
  assert.strictEqual(S.get(s, 0, 1, 0), "oak_log", "angoli di tronco");
  assert.strictEqual(S.get(s, 2, 0, 2), "cobblestone", "pavimento");
  assert.strictEqual(S.get(s, 2, 4, 2), "oak_planks", "tetto");
  assert.ok(S.isAir(S.get(s, 2, 1, 2)) && S.isAir(S.get(s, 2, 2, 2)), "dentro è vuoto");
  const m = S.materials(s);
  assert.strictEqual(m.need.oak_door, 1, "la porta conta una volta");
  assert.strictEqual(m.need.cobblestone, 25);
  assert.strictEqual(m.need.torch, 1);
  assert.ok(m.need.oak_planks > 30);
  const d = S.diff(m.need, [{ name: "oak_planks", count: 10 }, { name: "cobblestone", count: 64 }]);
  assert.ok(!d.ok && d.missing.oak_planks === m.need.oak_planks - 10 && !d.missing.cobblestone);
  assert.strictEqual(S.pickMaterial("oak_planks", [{ name: "cobblestone", count: 99 }], 40), "cobblestone", "non ha le assi: usa la pietra");
  assert.strictEqual(S.pickMaterial(null, [{ name: "spruce_planks", count: 3 }, { name: "dirt", count: 50 }]), "dirt");
  const tower = S.generate({ kind: "tower", h: 6 });
  assert.ok(S.blocks(tower).some((b) => b.name === "ladder"));
  const peaked = S.generate({ kind: "house", w: 7, l: 7, peaked: true });
  assert.ok(peaked.h > 5);
  const pr = S.preview(s);
  assert.ok(pr.blocks.length > 0 && pr.palette.includes("oak_planks") && !pr.truncated);
  // misure assurde: tagliate
  const big = S.generate({ kind: "house", w: 99, l: -3 });
  assert.ok(big.w === 15 && big.l === 15);
});

test("materiali: blocchi che non si piazzano e oggetti diversi dal blocco", () => {
  assert.strictEqual(S.itemFor("wall_torch"), "torch");
  assert.strictEqual(S.itemFor("oak_wall_sign"), "oak_sign");
  assert.strictEqual(S.itemFor("redstone_wire"), "redstone");
  assert.strictEqual(S.itemFor("water"), null);
  assert.strictEqual(S.itemFor("oak_door", { half: "upper" }), null);
  assert.strictEqual(S.itemFor("red_bed", { part: "head" }), null);
  assert.strictEqual(S.itemFor("potted_poppy"), "flower_pot");
  const s = S.create(2, 1, 1);
  S.set(s, 0, 0, 0, "minecraft:stone_slab[type=double]");
  S.set(s, 1, 0, 0, "minecraft:water[level=0]");
  const m = S.materials(s);
  assert.deepStrictEqual(m.need, { stone_slab: 2 });
  assert.deepStrictEqual(m.skipped, { water: 1 });
  assert.deepStrictEqual(S.propsOf("oak_stairs[facing=north,half=top]"), { facing: "north", half: "top" });
});

test("file di schematica: .schem (fixture), Sponge v3, .litematic, .nbt, .schematic", async () => {
  const nbt = require("prismarine-nbt");
  // fixture piccola nel repository (Sponge v2, gzip)
  const fix = await S.parseFile(fs.readFileSync(path.join(__dirname, "fixtures", "tiny.schem")), ".schem");
  assert.deepStrictEqual([fix.w, fix.h, fix.l], [3, 2, 3]);
  assert.strictEqual(S.get(fix, 1, 1, 1), "oak_log");
  assert.strictEqual(S.get(fix, 0, 1, 0), "wall_torch");
  assert.deepStrictEqual(S.materials(fix).need, { cobblestone: 9, torch: 1, oak_log: 1 });
  // Sponge v3 (Blocks.Palette / Blocks.Data), con varint a due byte (indice 130)
  const pal = { "minecraft:air": 0, "minecraft:stone": 130 };
  const v3 = { Schematic: { Version: 3, Width: 2, Height: 1, Length: 1, Blocks: { Palette: pal, Data: [0x82, 0x01, 0] } } };
  const s3 = S.fromNbt(v3);
  assert.deepStrictEqual([S.get(s3, 0, 0, 0), S.get(s3, 1, 0, 0)], ["stone", "air"]);
  // Litematica: 2 bit per voce, palette [air, glass, dirt], 2x1x2, dimensione negativa su x
  // valori (x,z): (0,0)=1 (1,0)=2 (0,1)=0 (1,1)=1 → bit: 01 | 10<<2 | 00<<4 | 01<<6 = 0b01001001 = 73
  const lit = { Version: 6, Metadata: { Name: "prova" }, Regions: { r: { Position: { x: 1, y: 0, z: 0 }, Size: { x: -2, y: 1, z: 2 }, BlockStatePalette: [{ Name: "minecraft:air" }, { Name: "minecraft:glass" }, { Name: "minecraft:dirt" }], BlockStates: [[0, 73]] } } };
  const sl = S.fromNbt(lit);
  assert.deepStrictEqual([sl.w, sl.h, sl.l, sl.name], [2, 1, 2, "prova"]);
  assert.deepStrictEqual([S.get(sl, 0, 0, 0), S.get(sl, 1, 0, 0), S.get(sl, 0, 0, 1), S.get(sl, 1, 0, 1)], ["glass", "dirt", "air", "glass"]);
  // valori che attraversano due long (bit 63-64)
  const big = S.toBig([-1, -1]);
  assert.strictEqual(big, (1n << 64n) - 1n);
  // struttura del gioco (.nbt)
  const st = S.fromNbt({ size: [2, 2, 1], palette: [{ Name: "minecraft:oak_planks" }, { Name: "minecraft:oak_stairs", Properties: { facing: "east" } }], blocks: [{ pos: [0, 0, 0], state: 0 }, { pos: [1, 1, 0], state: 1 }] });
  assert.deepStrictEqual([S.get(st, 0, 0, 0), S.getState(st, 1, 1, 0)], ["oak_planks", "oak_stairs[facing=east]"]);
  // MCEdit con id numerici (mappa delle vecchie versioni)
  const mce = S.fromNbt({ Width: 2, Height: 1, Length: 1, Blocks: [1, 17], Data: [0, 1] }, { legacy: { "1:0": "minecraft:stone", "17:1": "minecraft:spruce_log[axis=y]" } });
  assert.deepStrictEqual([S.get(mce, 0, 0, 0), S.get(mce, 1, 0, 0)], ["stone", "spruce_log"]);
  // file vero scritto e riletto (gzip) per la struttura .nbt
  const root = nbt.comp({ size: nbt.list(nbt.int([1, 1, 1])), palette: nbt.list(nbt.comp([{ Name: nbt.string("minecraft:gold_block") }])), blocks: nbt.list(nbt.comp([{ pos: nbt.list(nbt.int([0, 0, 0])), state: nbt.int(0) }])) });
  const buf = zlib.gzipSync(nbt.writeUncompressed(root));
  const back = await S.parseFile(buf, ".nbt");
  assert.strictEqual(S.get(back, 0, 0, 0), "gold_block");
  assert.throws(() => S.fromNbt({ boh: 1 }), /non riconosciuta/);
});

// ---------- costruzione ----------

/** Mondo finto: terreno a y = 63 (erba), un albero in (12, 64..67, 2) con foglie, un muro di assi in (9, 64, 0). */
function fakeWorld(extra = {}) {
  const blocks = new Map(Object.entries(extra));
  const k = (x, y, z) => `${x},${y},${z}`;
  for (let y = 64; y <= 67; y++) blocks.set(k(12, y, 2), "oak_log");
  for (const [dx, dz] of [[1, 0], [-1, 0], [0, 1], [0, -1]]) blocks.set(k(12 + dx, 67, 2 + dz), "oak_leaves");
  return {
    block(x, y, z) { const b = blocks.get(k(x, y, z)); if (b) return b; return y <= 63 ? (y === 63 ? "grass_block" : "dirt") : "air"; },
    ground(x, z) { return { y: 63, name: "grass_block", water: false }; },
    set(x, y, z, n) { blocks.set(k(x, y, z), n); },
  };
}

test("costruzione: punto, ostacoli (albero intero), fondamenta", () => {
  const w = fakeWorld();
  const s = S.generate({ kind: "house", w: 5, l: 5 });
  const site = B.chooseSite(w, { x: 0, y: 64, z: 0 }, { x: 1, z: 0 }, 5, 5, { h: s.h });
  assert.ok(site && site.y === 64 && site.flat === 0);
  assert.ok(!(0 >= site.x && 0 <= site.x + 5 && 0 >= site.z && 0 <= site.z + 5), "il giocatore non sta nel lotto");
  // punto fisso sopra l'albero: va tagliato tutto (anche le foglie in alto fuori dal volume)
  const at = { x: 10, y: 64, z: 0 };
  const obs = B.obstacles(s, at, w);
  assert.strictEqual(obs.trees >= 1, true);
  const logs = obs.clear.filter((c) => c.name === "oak_log");
  assert.strictEqual(logs.length, 4, "tutto il tronco");
  assert.ok(obs.clear.some((c) => c.name === "oak_leaves"));
  assert.deepStrictEqual(obs.foreign, {});
  // un muro costruito da qualcuno dentro il volume: "foreign" (prima di romperlo chiede)
  const w2 = fakeWorld({ "11,65,1": "stone_bricks" });
  assert.deepStrictEqual(B.obstacles(s, at, w2).foreign, { stone_bricks: 1 });
  // buco sotto il pavimento: da riempire
  const w3 = fakeWorld();
  w3.set(11, 63, 1, "air");
  w3.set(11, 62, 1, "air");
  const o3 = B.obstacles(s, at, w3);
  assert.deepStrictEqual(o3.fill.map((f) => [f.x, f.y, f.z]), [[11, 62, 1], [11, 63, 1]]);
  // davanti alla porta deve restare libero
  const w4 = fakeWorld({ "12,65,-1": "dirt" });
  assert.ok(B.obstacles(s, at, w4).clear.some((c) => c.x === 12 && c.z === -1));
  const sum = B.summary(s, S.materials(s), [{ name: "oak_planks", count: 64 }], obs, null);
  assert.ok(sum.blocks > 50 && sum.missing.cobblestone === 25 && sum.trees >= 1);
  assert.match(B.planLine(sum), /house 5x5: \d+ blocchi, mi mancano .*25 cobblestone.*taglio/);
});

test("costruzione: ordine fondamenta → pareti → tetto, dettagli alla fine, impalcature", () => {
  const s = S.generate({ kind: "house", w: 5, l: 5 });
  const ord = B.order(s, (x, y, z) => y < 0);
  const phase = ord.map((o) => o.phase);
  assert.strictEqual(phase[0], "foundation");
  assert.ok(phase.indexOf("walls") > phase.lastIndexOf("foundation"));
  assert.ok(phase.indexOf("roof") > phase.lastIndexOf("walls") - 10);
  assert.strictEqual(ord[ord.length - 1].phase, "roof");
  // la porta e la torcia dopo le pareti dello stesso strato
  const door = ord.findIndex((o) => o.name === "oak_door");
  const wall1 = ord.filter((o) => o.y === 1 && o.phase === "walls").map((o) => ord.indexOf(o));
  assert.ok(door > Math.max(...wall1));
  assert.ok(ord.filter((o) => o.y === 0).every((o) => !o.support), "le fondamenta poggiano sul terreno");
  // tetto in aria (niente pareti sotto): sospeso → serve un appoggio
  const roofOnly = S.create(3, 3, 1);
  S.set(roofOnly, 0, 2, 0, "stone");
  assert.ok(B.order(roofOnly, () => false)[0].support);
  // torre alta: impalcature per i blocchi in cima
  const t = S.generate({ kind: "tower", h: 10, w: 3 });
  const cells = B.order(t, (x, y, z) => y < 0).filter((c) => c.y >= 1);
  const sc = B.scaffolds(cells, t.w, t.l, { walkable: (x, z) => x < 0 || z < 0 || x >= t.w || z >= t.l });
  assert.ok(sc.columns.length >= 1 && sc.columns.length <= 4, JSON.stringify(sc.columns));
  assert.ok(sc.columns.every((c) => c.x === -1 || c.z === -1 || c.x === t.w || c.z === t.l), "colonne fuori dal perimetro");
  assert.strictEqual(sc.unreachable.length, 0);
  assert.ok(sc.reach.size > 0 && !sc.reach.has("1,1,0"), "quelli bassi si fanno da terra");
  // casetta bassa: niente impalcature
  const low = B.scaffolds(B.order(s, (x, y) => y < 0).filter((c) => c.y >= 1), 5, 5, {});
  assert.strictEqual(low.columns.length, 0);
});

// ---------- duelli ----------

test("duelli: regole dalla chat", () => {
  assert.deepStrictEqual(P.parseRules("duelliamo"), { endHp: 6, bestOf: 1, noBow: false, noRod: false, noPearls: false });
  assert.strictEqual(P.parseRules("duello fino a 2 cuori").endHp, 4);
  assert.strictEqual(P.parseRules("al primo ko").endHp, 0);
  assert.strictEqual(P.parseRules("1v1 best of 3, no bow").bestOf, 3);
  assert.ok(P.parseRules("1v1 best of 3, no bow").noBow);
  assert.ok(P.parseRules("al meglio di 5 senza perle").noPearls && P.parseRules("al meglio di 5 senza perle").bestOf === 5);
  assert.strictEqual(P.rulesLine(P.parseRules("fino a 3 cuori al meglio di 3")), "fino a 3 cuori, al meglio di 3");
  assert.strictEqual(P.rulesLine(P.parseRules("first ko"), true), "first KO");
});

test("duelli: stile dell'avversario, counter, scelta della tattica", () => {
  const p = P.newPattern();
  let t = 0;
  // avversario vicino (2 blocchi), strafe che cambia lato, critici in salto, scudo a volte
  for (let i = 0; i < 100; i++) {
    t += 0.1;
    P.observe(p, { t, d: 2 + (i % 3) * 0.1, lat: i % 10 < 5 ? 0.3 : -0.3, vr: 0.2, air: i % 4 === 0, falling: i % 4 === 0, swing: i % 4 === 0, blocking: i % 20 === 0, hp: 20 });
  }
  const st = P.style(p);
  assert.ok(st.prefDist > 1.9 && st.prefDist < 2.3, String(st.prefDist));
  assert.ok(st.strafe > 0.9 && st.flips > 0.5);
  assert.ok(st.crit > 0.9 && st.cps > 2);
  const c = P.counters(st, { shield: true });
  assert.ok(c.shield > 0.3 && c.spacing > 0.3, "contro critici e chi sta addosso: scudo e distanza");
  assert.strictEqual(c.kite, -9, "niente arco: niente kite");
  // senza dati sceglie per i counter
  const first = P.choose({}, st, { shield: true, eps: 0 });
  assert.ok(["shield", "spacing"].includes(first), first);
  // impara: la tattica che vince sale
  const stats = { spacing: { n: 6, reward: 0.6 }, shield: { n: 6, reward: 5.5 }, rush: { n: 6, reward: 0.5 }, strafe: { n: 6, reward: 0.4 } };
  assert.strictEqual(P.choose(stats, null, { shield: true, eps: 0 }), "shield");
  // esplorazione: ogni tanto prova altro
  let r = 0;
  const seq = [0.01, 0.99];
  const other = P.choose(stats, null, { shield: true, eps: 0.5 }, () => seq[r++ % 2]);
  assert.ok(P.NAMES.includes(other));
  assert.ok(P.reward(true, 10, 2) > P.reward(true, 2, 10) && P.reward(true, 2, 10) > P.reward(false, 2, 10));
  // copia dal giocatore distanza e strafe
  const pr = P.params("spacing", { dist: 2.4, strafe: 0.9 });
  assert.ok(pr.dist < 3.1 && pr.strafe === 0.9 && pr.wtap);
  assert.deepStrictEqual(P.roundOver({ endHp: 6 }, 5, 15, 10, 3), { over: true, won: false, why: "hearts" });
  assert.deepStrictEqual(P.roundOver({ endHp: 6 }, 12, 4, 10, 3), { over: true, won: true, why: "hearts" });
  assert.deepStrictEqual(P.roundOver({ endHp: 0 }, 5, 15, 10, 3), { over: false });
  // partito già basso: deve perdere almeno 4 punti
  assert.deepStrictEqual(P.roundOver({ endHp: 6 }, 20, 5, 1, 3, { me: 20, opp: 6 }), { over: false });
  assert.strictEqual(P.roundOver({ endHp: 6 }, 20, 2, 1, 3, { me: 20, opp: 6 }).won, true);
  assert.strictEqual(P.roundOver({ endHp: 6 }, 20, null, 10, 50).why, "left");
});
