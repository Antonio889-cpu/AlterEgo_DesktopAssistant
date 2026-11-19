// Alter Ego in Minecraft: corpo, sensi e mani (Mineflayer). Il cervello resta nell'app (src-tauri/src/mclink).
//
// Protocollo: righe JSON.
//  stdin  (dall'app):  {"id":12,"op":"follow","who":"Steve"}  (comandi numerati, "connect" per primo)
//  stdout (all'app):   {"t":"ready"} {"t":"state",…} {"t":"ev","kind":"chat",…} {"t":"ack","id":12,"ok":true}
//                      {"t":"sense","s":{"pain":0.5,…}} (5 volte al secondo, per il cervello di moscerino)
// I log vanno su stderr: su stdout solo il protocollo.
//
// Il comportamento continuo (seguire, guardare chi parla, difendersi, raccogliere, esplorare) gira qui senza
// token; il modello si chiama nell'app solo quando qualcuno le parla o succede qualcosa di importante.
// Idee prese da Mindcraft (https://github.com/mindcraft-bots/mindcraft, MIT): abilità piccole e componibili
// (raccogli, crea, fondi, costruisci), modalità continue (autodifesa, mangiare, guardare i giocatori).
"use strict";

const L = require("./lib");
const Conn = require("./conn"); // rete: ping, messaggi del server, comandi "/", sottoserver

// niente console.log su stdout (alcune librerie scrivono lì): tutto su stderr
const log = (...a) => process.stderr.write(a.map(String).join(" ") + "\n");
console.log = log;
console.info = log;
console.warn = log;
const out = (obj) => process.stdout.write(L.encode(obj));
const ev = (kind, data = {}) => out({ t: "ev", kind, ...data });

// ---------- solo ricerca dei mondi LAN (node bot.js --scan-lan) ----------
function scanLan(ms) {
  return new Promise((resolve) => {
    const dgram = require("dgram");
    const found = new Map();
    let sock;
    try {
      sock = dgram.createSocket({ type: "udp4", reuseAddr: true });
    } catch (e) {
      return resolve([]);
    }
    sock.on("error", () => { try { sock.close(); } catch {} resolve([...found.values()]); });
    sock.on("message", (msg, rinfo) => {
      const ad = L.parseLanAd(msg.toString("utf8"));
      if (ad) found.set(`${rinfo.address}:${ad.port}`, { host: rinfo.address, port: ad.port, motd: ad.motd });
    });
    sock.bind(4445, () => {
      try { sock.addMembership("224.0.2.60"); } catch {}
    });
    setTimeout(() => { try { sock.close(); } catch {} resolve([...found.values()]); }, ms);
  });
}

if (process.argv.includes("--scan-lan")) {
  scanLan(3500).then((list) => { out({ t: "lan", worlds: list }); process.exit(0); });
  return;
}
// solo versione del server (node bot.js --ping host porta): nativo o ponte ViaProxy?
if (process.argv.includes("--ping")) {
  Conn.pingMain(process.argv, scanLan, out).catch((e) => { out({ t: "ping", ok: false, err: String(e && e.message || e) }); process.exit(0); });
  return;
}

const mineflayer = require("mineflayer");
const { pathfinder, Movements, goals } = require("mineflayer-pathfinder");
const pvp = require("mineflayer-pvp").plugin;
const collectBlock = require("mineflayer-collectblock").plugin;
const toolPlugin = require("mineflayer-tool").plugin;
const { Vec3 } = require("vec3");

let cfg = { lang: "it", initiative: 60, follow: true, gather: false, build: true, explore: false, fly: true, player: "", quips: {} };
let bot = null;
let mc = null; // minecraft-data
let mode = "idle"; // idle | follow | stay | guard
let target = ""; // giocatore seguito o protetto
let task = null; // nome dell'azione lunga in corso
let cancel = 0; // cresce a ogni "stop": le azioni lunghe controllano
const queue = [];
let running = false;
const used = new Set();
let lastSay = 0;
let lastAct = Date.now(); // ultimo comando o attività (per la noia)
let hpPrev = 20;
let hurtNow = 0;
let hitBy = "";
let hitAt = 0;
let boomAt = 0;
let fuseAt = 0;
let lightAvg = null;
let spawnAt = 0;
let soundAt = null;
const dist = new Map(); // distanza precedente delle minacce (per capire se si avvicinano)
const seenOres = new Set();
let wasNight = false;

const rand = (a, b) => a + Math.random() * (b - a);
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const en = () => cfg.lang === "en";

// ---------- parlare ----------
async function chat(text) {
  for (const line of L.splitChat(text)) {
    await sleep(L.typingDelay(line, Math.random()));
    try { bot && bot.chat(line); } catch (e) { log("chat:", e.message); }
  }
}

/** Battuta veloce (zero token): righe del modello, altrimenti di riserva. Tiene conto dell'iniziativa. */
function quip(kind, emotion = "happy", force = false) {
  const now = Date.now();
  if (!force && now - lastSay < 9000) return;
  if (!force && Math.random() * 100 > 30 + cfg.initiative * 0.7) return;
  const lines = (cfg.quips && cfg.quips[kind]) || (L.FALLBACK[en() ? "en" : "it"] || {})[kind];
  const line = L.pickLine(lines, used);
  if (!line) return;
  lastSay = now;
  chat(line);
  ev("said", { text: line, emotion, cat: kind });
}

// ---------- utilità ----------
function players() {
  return Object.values(bot.players).filter((p) => p.username !== bot.username);
}
function playerEntity(who) {
  const names = players().map((p) => p.username);
  const name = L.findPlayer(names, who) || (who ? null : owner());
  const p = name && bot.players[name];
  return p && p.entity ? p.entity : null;
}
/** Con chi gioca: il nome nelle impostazioni, altrimenti il giocatore più vicino. */
function owner() {
  const names = players().map((p) => p.username);
  const set = cfg.player && L.findPlayer(names, cfg.player);
  if (set) return set;
  let best = null, bd = Infinity;
  for (const p of players()) {
    if (!p.entity) continue;
    const d = p.entity.position.distanceTo(bot.entity.position);
    if (d < bd) { bd = d; best = p.username; }
  }
  return best || names[0] || "";
}
const isHostile = (e) => e && e !== bot.entity && (e.type === "hostile" || L.HOSTILE.has(e.name));
function invNames() {
  return bot.inventory.items().map((i) => i.name);
}
function countItem(name) {
  return bot.inventory.items().filter((i) => i.name === name).reduce((a, i) => a + i.count, 0);
}
function movements(dig) {
  const m = new Movements(bot);
  m.canDig = !!dig;
  m.allowSprinting = true;
  m.allowParkour = true;
  return m;
}
function checkCancel(c) {
  if (c !== cancel) throw new Error(en() ? "stopped" : "fermata");
}
async function goNear(pos, range, c, dig = false) {
  bot.pathfinder.setMovements(movements(dig));
  const p = bot.pathfinder.goto(new goals.GoalNear(pos.x, pos.y, pos.z, range));
  const t = sleep(45000).then(() => { throw new Error(en() ? "too far, couldn't get there" : "troppo lontano, non ci arrivo"); });
  await Promise.race([p, t]);
  checkCancel(c);
}

// nomi comodi → blocchi veri
const ALIAS = {
  wood: "_log", log: "_log", logs: "_log", legno: "_log", tronco: "_log", tronchi: "_log",
  stone: "stone", pietra: "stone", cobble: "stone", cobblestone: "stone", coal: "coal_ore", carbone: "coal_ore",
  iron: "iron_ore", ferro: "iron_ore", diamond: "diamond_ore", diamante: "diamond_ore", diamanti: "diamond_ore", sand: "sand", sabbia: "sand",
  dirt: "dirt", terra: "dirt", gold: "gold_ore", oro: "gold_ore", copper: "copper_ore", rame: "copper_ore",
};
function blockIds(name) {
  const n = String(name || "").toLowerCase().trim().replace(/\s+/g, "_");
  const a = ALIAS[n] || n;
  const ids = [];
  for (const b of Object.values(mc.blocksByName)) {
    if (a.startsWith("_") ? b.name.endsWith(a) && !b.name.startsWith("stripped") : b.name === a || (a.endsWith("_ore") && b.name === "deepslate_" + a)) ids.push(b.id);
  }
  if (a === "stone") { const c = mc.blocksByName.cobblestone; if (c) ids.push(c.id); }
  return ids;
}
const DROPS = { stone: "cobblestone", coal_ore: "coal", iron_ore: "raw_iron", gold_ore: "raw_gold", diamond_ore: "diamond", copper_ore: "raw_copper" };

// ---------- azioni (abilità) ----------
const SKILLS = {
  async follow(o) {
    const e = playerEntity(o.who);
    if (!e) throw new Error(en() ? "I can't see you" : "non ti vedo");
    mode = "follow";
    target = e.username;
    resume();
  },
  async stay() {
    mode = "stay";
    bot.pathfinder.setGoal(null);
    bot.pvp.stop();
  },
  async come(o, c) {
    const e = playerEntity(o.who);
    if (!e) throw new Error(en() ? "I can't see you" : "non ti vedo");
    mode = mode === "follow" ? "follow" : "idle";
    await goNear(e.position, 2, c);
    await bot.lookAt(e.position.offset(0, 1.6, 0));
  },
  async goto(o, c) {
    const x = Number(o.x), y = Number(o.y), z = Number(o.z);
    if (![x, y, z].every(Number.isFinite)) throw new Error("x y z?");
    mode = "idle";
    await goNear(new Vec3(x, y, z), 1, c, true);
  },
  async look(o) {
    const e = playerEntity(o.who);
    if (e) await bot.lookAt(e.position.offset(0, 1.6, 0));
  },
  async jump() {
    bot.setControlState("jump", true);
    await sleep(300);
    bot.setControlState("jump", false);
  },
  async guard(o) {
    const e = playerEntity(o.who);
    if (!e) throw new Error(en() ? "I can't see you" : "non ti vedo");
    mode = "guard";
    target = e.username;
    resume();
  },
  async attack(o, c) {
    const w = String(o.what || "").toLowerCase();
    const p = playerEntity(w);
    const e = p || bot.nearestEntity((x) => x !== bot.entity && x.name && (x.name === w || x.name.includes(w) || (w === "mob" && isHostile(x))) && x.position.distanceTo(bot.entity.position) < 32);
    if (!e) throw new Error(en() ? `no ${w} around` : `nessun ${w} qui vicino`);
    await equipWeapon();
    bot.pvp.attack(e);
    const t0 = Date.now();
    while (e.isValid && Date.now() - t0 < 30000) { await sleep(300); checkCancel(c); }
    bot.pvp.stop();
  },
  async collect(o, c) {
    const ids = blockIds(o.block);
    if (!ids.length) throw new Error(en() ? `unknown block ${o.block}` : `blocco sconosciuto ${o.block}`);
    const n = Math.max(1, Math.min(64, Number(o.n) || 1));
    let got = 0;
    let misses = 0;
    while (got < n) {
      checkCancel(c);
      const pos = bot.findBlocks({ matching: ids, maxDistance: 64, count: 1 })[0];
      if (!pos) {
        if (got) break;
        throw new Error(en() ? `no ${o.block} nearby` : `niente ${o.block} qui vicino`);
      }
      const b = bot.blockAt(pos);
      bot.pathfinder.setMovements(movements(true));
      try {
        await Promise.race([bot.collectBlock.collect(b, { ignoreNoPath: false }), sleep(40000).then(() => { throw new Error("timeout"); })]);
        got++;
        lastAct = Date.now();
      } catch (e) {
        if (++misses > 3) throw new Error((en() ? "can't collect: " : "non riesco a raccogliere: ") + e.message);
      }
    }
    return { got };
  },
  async craft(o, c) {
    const it = mc.itemsByName[String(o.item || "").toLowerCase()];
    if (!it) throw new Error(en() ? `unknown item ${o.item}` : `oggetto sconosciuto ${o.item}`);
    const n = Math.max(1, Math.min(64, Number(o.n) || 1));
    let recipe = bot.recipesFor(it.id, null, 1, null)[0];
    let table = null;
    if (!recipe) {
      table = bot.findBlock({ matching: mc.blocksByName.crafting_table.id, maxDistance: 32 });
      if (!table && countItem("crafting_table")) table = await placeNear("crafting_table", c);
      if (table) {
        await goNear(table.position, 3, c);
        recipe = bot.recipesFor(it.id, null, 1, table)[0];
      }
    }
    if (!recipe) {
      const any = bot.recipesAll ? bot.recipesAll(it.id, null, true) : [];
      const need = any[0] ? [...new Set((any[0].delta || []).filter((d) => d.count < 0).map((d) => mc.items[d.id] && mc.items[d.id].name))].join(", ") : "";
      throw new Error((en() ? `missing materials for ${it.name}` : `mancano i materiali per ${it.name}`) + (need ? ` (${need})` : "") + (table ? "" : en() ? ", maybe a crafting table" : ", forse un banco da lavoro"));
    }
    const times = Math.ceil(n / (recipe.result.count || 1));
    await bot.craft(recipe, times, table);
    lastAct = Date.now();
    return { have: countItem(it.name) };
  },
  async smelt(o, c) {
    const input = String(o.item || "").toLowerCase();
    if (!countItem(input)) throw new Error(en() ? `I have no ${input}` : `non ho ${input}`);
    let fb = bot.findBlock({ matching: mc.blocksByName.furnace.id, maxDistance: 32 });
    if (!fb && countItem("furnace")) fb = await placeNear("furnace", c);
    if (!fb) throw new Error(en() ? "no furnace" : "nessuna fornace");
    await goNear(fb.position, 3, c);
    const fuel = ["coal", "charcoal", "oak_planks", "spruce_planks", "birch_planks", "oak_log", "spruce_log", "birch_log"].find((f) => countItem(f));
    if (!fuel) throw new Error(en() ? "no fuel" : "niente combustibile");
    const n = Math.max(1, Math.min(16, Number(o.n) || 1, countItem(input)));
    const f = await bot.openFurnace(fb);
    try {
      await f.putFuel(mc.itemsByName[fuel].id, null, Math.min(countItem(fuel), Math.ceil(n / (fuel.includes("coal") ? 8 : 1.5))));
      await f.putInput(mc.itemsByName[input].id, null, n);
      const t0 = Date.now();
      while (Date.now() - t0 < n * 10500 + 2000) { await sleep(1000); checkCancel(c); }
      if (f.outputItem()) await f.takeOutput();
    } finally {
      f.close();
    }
  },
  async place(o, c) {
    await placeNear(String(o.block || "").toLowerCase(), c);
  },
  async build(o, c) {
    const kind = String(o.shape || "tower").toLowerCase();
    const blocks = L.shape(kind, Number(o.size) || 0);
    if (!blocks.length) throw new Error(en() ? `I don't know how to build ${kind}` : `non so costruire ${kind}`);
    const name = String(o.block || "").toLowerCase() || pickBuildBlock();
    if (!name || !countItem(name)) throw new Error(en() ? `I need blocks (${name || "cobblestone, planks…"})` : `mi servono blocchi (${name || "pietrisco, assi…"})`);
    const fwd = new Vec3(-Math.sin(bot.entity.yaw), 0, -Math.cos(bot.entity.yaw));
    let base = bot.entity.position.floored().plus(new Vec3(Math.round(fwd.x * 3), 0, Math.round(fwd.z * 3)));
    // la base va sul terreno: sopra i blocchi pieni, giù se sotto c'è il vuoto
    const solid = (p) => { const b = bot.blockAt(p); return !!b && b.boundingBox === "block"; };
    for (let i = 0; i < 4 && solid(base); i++) base = base.offset(0, 1, 0);
    for (let i = 0; i < 4 && !solid(base.offset(0, -1, 0)); i++) base = base.offset(0, -1, 0);
    let placed = 0;
    let already = 0;
    for (const [x, y, z] of blocks) {
      checkCancel(c);
      if (!countItem(name)) break;
      const p = base.offset(x, y, z);
      if (solid(p)) { already++; continue; }
      try {
        if (bot.entity.position.distanceTo(p) > 4) await goNear(p, 3, c);
        if (await placeAt(p, name)) placed++;
      } catch (e) {
        if (String(e.message).includes(en() ? "stopped" : "fermata")) throw e;
      }
    }
    if (!placed && !already) throw new Error(en() ? "couldn't place any block" : "nessun blocco messo");
    lastAct = Date.now();
    return { placed, of: blocks.length, missing: blocks.length - placed - already };
  },
  async give(o, c) {
    const e = playerEntity(o.who);
    if (!e) throw new Error(en() ? "I can't see you" : "non ti vedo");
    const name = String(o.item || "").toLowerCase();
    const have = countItem(name);
    if (!have) throw new Error(en() ? `I have no ${name}` : `non ho ${name}`);
    await goNear(e.position, 2, c);
    await bot.lookAt(e.position.offset(0, 1.2, 0));
    await bot.toss(mc.itemsByName[name].id, null, Math.min(have, Math.max(1, Number(o.n) || have)));
  },
  async equip(o) {
    const it = bot.inventory.items().find((i) => i.name === String(o.item || "").toLowerCase());
    if (!it) throw new Error(en() ? `I have no ${o.item}` : `non ho ${o.item}`);
    await bot.equip(it, "hand");
  },
  async eat() {
    if (!(await eat(true))) throw new Error(en() ? "nothing to eat" : "niente da mangiare");
  },
  async sleep(_o, c) {
    const bed = bot.findBlock({ matching: (b) => b.name.endsWith("_bed"), maxDistance: 32 });
    if (!bed) throw new Error(en() ? "no bed nearby" : "nessun letto qui vicino");
    await goNear(bed.position, 2, c);
    await bot.sleep(bed);
  },
  async drop(o) {
    const name = String(o.item || "").toLowerCase();
    if (!name) return;
    const have = countItem(name);
    if (!have) throw new Error(en() ? `I have no ${name}` : `non ho ${name}`);
    await bot.toss(mc.itemsByName[name].id, null, Math.min(have, Math.max(1, Number(o.n) || have)));
  },
  async explore(o, c) {
    const d = Math.max(10, Math.min(120, Number(o.dist) || 40));
    const a = Math.random() * Math.PI * 2;
    const p = bot.entity.position;
    mode = "idle";
    bot.pathfinder.setMovements(movements(false));
    await Promise.race([bot.pathfinder.goto(new goals.GoalXZ(Math.round(p.x + Math.cos(a) * d), Math.round(p.z + Math.sin(a) * d))), sleep(60000)]);
    checkCancel(c);
  },
  async say(o) {
    await chat(o.text);
  },
  async quip(o) {
    quip(String(o.kind || ""), o.emotion || "happy", true);
  },
  async skin(o) {
    const cmd = String(o.cmd || "").trim();
    if (!/^\/skin(\s|$)/i.test(cmd)) throw new Error("only /skin");
    skinWatch = Date.now();
    bot.chat(cmd.slice(0, 250));
  },
};

/** Blocco da costruzione che ha di più nell'inventario. */
function pickBuildBlock() {
  const good = ["cobblestone", "oak_planks", "spruce_planks", "birch_planks", "stone", "dirt", "stone_bricks", "cobbled_deepslate", "sandstone"];
  let best = null, bn = 0;
  for (const g of good) { const n = countItem(g); if (n > bn) { bn = n; best = g; } }
  return best;
}

async function placeAt(p, name) {
  const it = bot.inventory.items().find((i) => i.name === name);
  if (!it) return false;
  const faces = [new Vec3(0, -1, 0), new Vec3(1, 0, 0), new Vec3(-1, 0, 0), new Vec3(0, 0, 1), new Vec3(0, 0, -1), new Vec3(0, 1, 0)];
  for (const f of faces) {
    const ref = bot.blockAt(p.plus(f));
    if (ref && ref.boundingBox === "block") {
      await bot.equip(it, "hand");
      try {
        await bot.placeBlock(ref, f.scaled(-1));
        return true;
      } catch {}
    }
  }
  return false;
}

/** Mette un blocco dell'inventario accanto a sé e lo restituisce. */
async function placeNear(name, c) {
  if (!countItem(name)) throw new Error(en() ? `I have no ${name}` : `non ho ${name}`);
  const me = bot.entity.position.floored();
  for (const [x, z] of [[1, 0], [-1, 0], [0, 1], [0, -1], [1, 1], [-1, -1], [2, 0], [0, 2]]) {
    checkCancel(c);
    const p = me.offset(x, 0, z);
    const here = bot.blockAt(p);
    if (here && here.boundingBox === "empty" && (await placeAt(p, name))) return bot.blockAt(p);
  }
  throw new Error(en() ? `no room to place ${name}` : `non c'è posto per ${name}`);
}

async function equipWeapon() {
  const w = ["netherite_sword", "diamond_sword", "iron_sword", "stone_sword", "wooden_sword", "diamond_axe", "iron_axe", "stone_axe", "wooden_axe"].find((n) => countItem(n));
  if (w) { try { await bot.equip(bot.inventory.items().find((i) => i.name === w), "hand"); } catch {} }
}

let eating = false;
async function eat(force) {
  if (eating || (!force && bot.food >= 14)) return false;
  const name = L.bestFood(invNames());
  if (!name) return false;
  eating = true;
  try {
    await bot.equip(bot.inventory.items().find((i) => i.name === name), "hand");
    await bot.consume();
    ev("ate", { what: name });
    quip("eat");
    return true;
  } catch {
    return false;
  } finally {
    eating = false;
  }
}

/** Riprende il comportamento continuo (dopo un'azione o un riflesso). */
function resume() {
  if (!bot || !bot.entity || task) return;
  bot.pathfinder.setMovements(movements(false));
  if (mode === "follow" || mode === "guard") {
    const e = playerEntity(target);
    bot.pathfinder.setGoal(e ? new goals.GoalFollow(e, mode === "guard" ? 3 : 2) : null, true);
  } else {
    bot.pathfinder.setGoal(null);
  }
}

// ---------- riflessi (decisi dal cervello di moscerino nell'app) ----------
let threat = null; // posizione dell'ultima minaccia
let reflexBusy = false;
async function reflex(kind) {
  if (!bot || !bot.entity || reflexBusy) return;
  reflexBusy = true;
  try {
    // tempo di reazione umano
    await sleep(rand(90, 240));
    if (kind === "eat") { await eat(false); return; }
    if (kind === "look_sound" && soundAt) { await bot.lookAt(soundAt); return; }
    if (kind === "look_away") { await bot.look(bot.entity.yaw + Math.PI * 0.6, -0.3); return; }
    const plan = L.reflexPlan(kind);
    if (!plan.length) return;
    const goal = bot.pathfinder.goal;
    bot.pathfinder.setGoal(null);
    if (kind === "flee" && threat) {
      const d = bot.entity.position.minus(threat);
      await bot.look(Math.atan2(-d.x, -d.z), 0, true);
    }
    for (const [ctl, ms] of plan) {
      const keys = ctl.split("+");
      keys.forEach((k) => bot.setControlState(k, true));
      if (kind === "flee") bot.setControlState("jump", Math.random() < 0.4);
      await sleep(ms * rand(0.85, 1.2));
      keys.forEach((k) => bot.setControlState(k, false));
      bot.setControlState("jump", false);
    }
    if (kind === "flee") quip("flee", "worried");
    if (goal && !task) resume();
  } finally {
    reflexBusy = false;
  }
}

// ---------- coda delle azioni ----------
const NOW = new Set(["stop", "stay", "reflex", "config", "quips", "quip", "look", "jump", "say", "follow", "guard", "leave", "skin"]);

function handle(cmd) {
  lastAct = Date.now();
  if (cmd.op === "connect") return connect(cmd);
  if (Conn.OPS.has(cmd.op)) return Conn.handle(cmd, bot, ack);
  if (!bot || !bot.entity) {
    if (cmd.op === "config") { Object.assign(cfg, cmd.cfg || {}); return ack(cmd, true); }
    return ack(cmd, false, "not connected");
  }
  if (cmd.op === "stop") {
    cancel++;
    queue.length = 0;
    task = null;
    mode = "idle";
    try { bot.pathfinder.setGoal(null); bot.pvp.stop(); bot.collectBlock.cancelTask && bot.collectBlock.cancelTask(); bot.clearControlStates(); } catch {}
    return ack(cmd, true);
  }
  if (cmd.op === "config") { Object.assign(cfg, cmd.cfg || {}); return ack(cmd, true); }
  if (cmd.op === "quips") { cfg.quips = cmd.lines || {}; return ack(cmd, true); }
  if (cmd.op === "reflex") { reflex(cmd.kind); return; }
  if (cmd.op === "leave") { ack(cmd, true); setTimeout(() => bot && bot.quit(), 300); return; }
  if (NOW.has(cmd.op)) return run(cmd);
  queue.push(cmd);
  pump();
}

function ack(cmd, ok, err, info) {
  if (cmd.id == null) return;
  const a = { t: "ack", id: cmd.id, op: cmd.op, ok };
  if (err) a.err = String(err).slice(0, 200);
  if (info) a.info = info;
  out(a);
}

async function run(cmd) {
  const f = SKILLS[cmd.op];
  if (!f) return ack(cmd, false, "unknown op " + cmd.op);
  const c = cancel;
  try {
    // una persona non reagisce all'istante
    if (!NOW.has(cmd.op)) await sleep(rand(150, 450));
    const info = await f(cmd, c);
    ack(cmd, true, null, info);
  } catch (e) {
    ack(cmd, false, e && e.message ? e.message : String(e));
  }
}

async function pump() {
  if (running) return;
  running = true;
  while (queue.length) {
    const cmd = queue.shift();
    task = cmd.op;
    await run(cmd);
    task = null;
  }
  running = false;
  resume();
}

// ---------- connessione ----------
let skinWatch = 0;
let pendingSkin = "";

async function connect(o) {
  if (bot) return ack(o, false, "already connected");
  Object.assign(cfg, o.cfg || {});
  let host = o.host || "127.0.0.1";
  let port = Number(o.port) || 0;
  if (!port) {
    const list = await scanLan(3500);
    const w = list.find((x) => x.host === host || host === "127.0.0.1" || host === "localhost") || list[0];
    if (!w) {
      ev("error", { code: "no_lan", msg: "no LAN world found" });
      return ack(o, false, "no LAN world found");
    }
    port = w.port;
    if (host === "localhost" || host === "127.0.0.1") host = w.host === "127.0.0.1" ? host : w.host;
    ev("lan", w);
  }
  pendingSkin = String(o.skin || "");
  log(`connessione a ${host}:${port} come ${o.username}`);
  try {
    bot = mineflayer.createBot({ host, port, username: o.username || "AlterEgo", auth: "offline", version: o.version || false, hideErrors: true, checkTimeoutInterval: 60000 });
  } catch (e) {
    bot = null;
    ev("error", { msg: e.message });
    return ack(o, false, e.message);
  }
  bot.loadPlugin(pathfinder);
  bot.loadPlugin(toolPlugin);
  bot.loadPlugin(pvp);
  bot.loadPlugin(collectBlock);
  Conn.attach(bot, { ev, log });
  let joined = false;
  bot.once("spawn", () => {
    joined = true;
    mc = require("minecraft-data")(bot.version);
    hpPrev = bot.health;
    spawnAt = Date.now();
    ack(o, true, null, { version: bot.version, host, port });
    ev("joined", { version: bot.version, host, port, name: bot.username });
    if (cfg.follow) {
      const who = owner();
      if (who) { mode = "follow"; target = who; }
    }
    setTimeout(() => { resume(); quip("greet", "happy", true); }, 1500);
    if (pendingSkin) setTimeout(() => SKILLS.skin({ cmd: pendingSkin }).catch(() => {}), 4000);
    timers();
  });
  bot.on("respawn", () => { hpPrev = 20; setTimeout(resume, 1000); });
  bot.on("chat", (username, message) => {
    if (username === bot.username) return;
    ev("chat", { from: username, text: message, owner: username === owner() });
  });
  bot.on("whisper", (username, message) => {
    if (username === bot.username) return;
    ev("chat", { from: username, text: message, whisper: true, owner: username === owner() });
  });
  bot.on("messagestr", (msg, position) => {
    if (position === "chat") return;
    const m = String(msg);
    if (skinWatch && Date.now() - skinWatch < 10000 && /skin|unknown|incomplete|permission|sconosciuto|permesso/i.test(m)) {
      const ok = /skin/i.test(m) && !/unknown|incomplete|permission|error|errore|sconosciuto/i.test(m);
      ev("skin", { ok, msg: m.slice(0, 200) });
      skinWatch = 0;
    }
  });
  bot.on("kicked", (reason) => ev("kicked", { reason: L.flatten(reason).slice(0, 300), joined }));
  bot.on("error", (e) => ev("error", { msg: String(e && e.message ? e.message : e).slice(0, 300) }));
  bot.on("end", (reason) => {
    ev("left", { reason: String(reason || "") });
    bot = null;
    stopTimers();
    // senza mondo il processo non serve più: l'app ne avvia uno nuovo per rientrare
    setTimeout(() => process.exit(0), 200);
  });
  bot.on("death", () => { ev("died", {}); quip("died", "sad", true); });
  bot.on("health", () => {
    if (bot.health < hpPrev) {
      hurtNow = Math.max(hurtNow, hpPrev - bot.health);
      if (Date.now() - hitAt < 600 && hitBy) ev("hurt", { by: hitBy, dmg: Math.round(hpPrev - bot.health) });
      else ev("hurt", { dmg: Math.round(hpPrev - bot.health) });
      if (bot.health > 0) quip("hurt", "worried");
    }
    hpPrev = bot.health;
    if (bot.food <= 6) quip("hungry", "sad");
  });
  bot.on("entitySwingArm", (e) => {
    if (e.type === "player" && e.username !== bot.username && e.position.distanceTo(bot.entity.position) < 4.5) { hitBy = e.username; hitAt = Date.now(); }
  });
  bot.on("playerJoined", (p) => { if (joined && p.username !== bot.username) ev("player_join", { name: p.username }); });
  bot.on("playerLeft", (p) => { if (joined && p.username !== bot.username) ev("player_leave", { name: p.username }); });
  bot.on("soundEffectHeard", (name, pos) => {
    const n = String(name);
    if (n.includes("creeper.primed")) fuseAt = Date.now();
    if (/explode|thunder|tnt/.test(n)) { boomAt = Date.now(); soundAt = pos; }
  });
  bot._client.on("explosion", (p) => {
    boomAt = Date.now();
    const x = p.x ?? (p.center && p.center.x), y = p.y ?? (p.center && p.center.y), z = p.z ?? (p.center && p.center.z);
    if (Number.isFinite(x)) soundAt = new Vec3(x, y, z);
  });
}

// ---------- giri periodici ----------
let tState = null, tSense = null, tLife = null;
function stopTimers() { [tState, tSense, tLife].forEach((t) => t && clearInterval(t)); }
function timers() {
  tState = setInterval(sendState, 1000);
  tSense = setInterval(sendSenses, 200);
  tLife = setInterval(life, 1000);
}

function dayPart() {
  const t = bot.time.timeOfDay;
  return t < 12000 ? "day" : t < 13800 ? "dusk" : t < 23000 ? "night" : "dawn";
}

function sendState() {
  if (!bot || !bot.entity) return;
  const me = bot.entity.position;
  const ps = players().map((p) => {
    const o = { name: p.username };
    if (p.entity) { o.d = Math.round(p.entity.position.distanceTo(me)); o.dir = L.side(me, bot.entity.yaw, p.entity.position); }
    else o.seen = false;
    return o;
  });
  const near = Object.values(bot.entities)
    .filter((e) => e !== bot.entity && e.type !== "player" && e.name && e.position.distanceTo(me) < 16 && (e.type === "mob" || e.type === "hostile" || e.type === "animal" || isHostile(e)))
    .sort((a, b) => a.position.distanceTo(me) - b.position.distanceTo(me))
    .slice(0, 6)
    .map((e) => ({ name: e.name, d: Math.round(e.position.distanceTo(me)), dir: L.side(me, bot.entity.yaw, e.position), hostile: isHostile(e) || undefined }));
  const held = bot.heldItem ? bot.heldItem.name : null;
  let light = null;
  try { const b = bot.blockAt(me.offset(0, 1, 0)); if (b) light = Math.max(b.light || 0, bot.time.isDay ? b.skyLight || 0 : (b.skyLight || 0) - 11); } catch {}
  out({
    t: "state",
    hp: Math.round(bot.health), food: Math.round(bot.food), pos: [Math.round(me.x), Math.round(me.y), Math.round(me.z)],
    dim: String(bot.game.dimension || "").replace("minecraft:", ""), time: dayPart(), rain: !!bot.isRaining,
    mode, target, task, held, inv: L.invLine(bot.inventory.items()), players: ps, near, light, water: !!bot.entity.isInWater,
    owner: owner(), version: bot.version,
  });
}

function sendSenses() {
  if (!bot || !bot.entity) return;
  const me = bot.entity.position;
  const threats = [];
  let creeper = null;
  let closest = null, cd = Infinity;
  for (const e of Object.values(bot.entities)) {
    if (e === bot.entity || !e.position) continue;
    const d = e.position.distanceTo(me);
    if (d > 12) continue;
    const proj = /arrow|trident|fireball|snowball|egg|potion|wind_charge/.test(e.name || "");
    if (!isHostile(e) && !proj) continue;
    const prev = dist.get(e.id);
    dist.set(e.id, d);
    let closing = prev != null && d < prev - 0.05;
    if (proj && e.velocity) closing = e.velocity.x * (me.x - e.position.x) + e.velocity.z * (me.z - e.position.z) > 0;
    threats.push({ d, side: L.side(me, bot.entity.yaw, e.position), closing });
    if (e.name === "creeper" && (creeper == null || d < creeper)) creeper = d;
    if (d < cd) { cd = d; closest = e.position; }
  }
  if (dist.size > 200) dist.clear();
  if (closest) threat = closest;
  const names = invNames();
  const sweetHeld = names.some((n) => L.SWEET.has(n));
  const drop = bot.nearestEntity((e) => {
    if (e.name !== "item" || e.position.distanceTo(me) > 3) return false;
    const it = typeof e.getDroppedItem === "function" ? e.getDroppedItem() : null;
    return !!it && (L.SWEET.has(it.name) || L.FOODS.includes(it.name));
  });
  let light = null;
  try { const b = bot.blockAt(me.offset(0, 1, 0)); if (b) light = Math.max(b.light || 0, bot.time.isDay ? b.skyLight || 0 : 0); } catch {}
  const s = L.senses({
    hurt: hurtNow, threats, creeper, fuse: Date.now() - fuseAt < 1600,
    boom: boomAt ? (Date.now() - boomAt) / 1000 : null,
    food: (bot.food < 18 && (sweetHeld || L.bestFood(names)) ? 0.7 : 0) || (drop ? 0.4 : 0), hungry: bot.food < 14,
    hitBy: Date.now() - hitAt < 500, water: !!bot.entity.isInWater,
    fall: bot.entity.onGround ? 0 : bot.entity.velocity.y, light, lightBefore: Date.now() - spawnAt > 5000 ? lightAvg : null,
  });
  hurtNow = 0;
  if (light != null) lightAvg = lightAvg == null ? light : lightAvg * 0.9 + light * 0.1;
  if (s.smell_danger > 0.6) quip("creeper", "worried");
  // senza cervello di moscerino: riflessi semplici fatti qui
  if (!cfg.fly) {
    if (s.smell_danger > 0.7 || (s.pain > 0.5 && creeper != null)) reflex("flee");
    else if (s.loom_l > 0.6) reflex("strafe_r");
    else if (s.loom_r > 0.6) reflex("strafe_l");
  }
  out({ t: "sense", s });
}

/** Vita di tutti i giorni (ogni secondo, zero token): mangiare, difendersi, guardare, annoiarsi. */
let lastIdle = Date.now();
function life() {
  if (!bot || !bot.entity) return;
  const me = bot.entity.position;
  if (!task && !reflexBusy) eat(false);
  // autodifesa: un mob ostile addosso (non i creeper: da quelli si scappa)
  const foe = bot.nearestEntity((e) => isHostile(e) && e.name !== "creeper" && e.position.distanceTo(me) < 3.5);
  if (foe && !bot.pvp.target && bot.health > 6 && mode !== "stay") { equipWeapon().then(() => bot && bot.pvp.attack(foe)); }
  if (mode === "guard" && !bot.pvp.target) {
    const p = playerEntity(target);
    const m = p && bot.nearestEntity((e) => isHostile(e) && e.name !== "creeper" && e.position.distanceTo(p.position) < 8);
    if (m) bot.pvp.attack(m);
  }
  if (bot.pvp.target && !bot.pvp.target.isValid) { bot.pvp.stop(); resume(); }
  // minerali preziosi visti da vicino
  try {
    const ore = bot.findBlock({ matching: (b) => /diamond_ore|emerald_ore|ancient_debris/.test(b.name), maxDistance: 8 });
    if (ore && !seenOres.has(ore.position.toString())) {
      seenOres.add(ore.position.toString());
      ev("found", { block: ore.name, d: Math.round(ore.position.distanceTo(me)) });
      quip("found", "surprised");
    }
  } catch {}
  const night = dayPart() === "night";
  if (night && !wasNight) { ev("night", {}); quip("night", "worried"); }
  wasNight = night;
  // guarda chi le sta vicino (come una persona), ogni tanto
  if (!task && !bot.pathfinder.isMoving() && Math.random() < 0.25) {
    const p = playerEntity(owner());
    if (p && p.position.distanceTo(me) < 12) bot.lookAt(p.position.offset(0, 1.6, 0)).catch(() => {});
  }
  // iniziativa: quando non succede niente fa qualcosa da sola
  const idle = Date.now() - Math.max(lastAct, lastIdle);
  const wait = 60000 - cfg.initiative * 450; // da 60 s (iniziativa 0) a 15 s (iniziativa 100)
  if (task || queue.length || mode === "stay" || cfg.initiative <= 0 || idle < wait) return;
  lastIdle = Date.now();
  if (Math.random() * 100 > cfg.initiative) return;
  const ideas = [];
  if (cfg.gather) ideas.push({ op: "collect", block: Math.random() < 0.6 ? "wood" : "stone", n: 3 + Math.floor(Math.random() * 4) });
  if (cfg.explore && mode !== "follow") ideas.push({ op: "explore", dist: 25 });
  if (!ideas.length) { quip("bored", "thinking"); return; }
  const idea = ideas[Math.floor(Math.random() * ideas.length)];
  ev("idea", { op: idea.op });
  queue.push(idea);
  pump();
}

// ---------- ingresso ----------
const rl = require("readline").createInterface({ input: process.stdin });
rl.on("line", (line) => {
  const cmd = L.parseLine(line);
  if (cmd) {
    try { handle(cmd); } catch (e) { ack(cmd, false, e.message); }
  }
});
// l'app si è chiusa: si esce anche dal mondo
rl.on("close", () => { try { bot && bot.quit(); } catch {} setTimeout(() => process.exit(0), 300); });
process.on("uncaughtException", (e) => { log("errore:", e && e.stack ? e.stack : e); ev("error", { msg: String(e && e.message ? e.message : e).slice(0, 300) }); });
process.on("unhandledRejection", (e) => log("promessa:", e && e.message ? e.message : e));
out({ t: "ready", v: 1 });
