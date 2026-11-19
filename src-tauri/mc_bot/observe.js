// Impara dal giocatore guardandolo. Onesto sui limiti: da un altro client non si vedono il suo inventario né
// la griglia di crafting. Si vede: cosa ha in mano e addosso (equipaggiamento dell'entità), come si muove
// (velocità, salti, sprint, sneak, dove passa il tempo), i blocchi che compaiono o spariscono vicino a lui,
// il banco da lavoro davanti a cui si ferma (e l'oggetto nuovo in mano subito dopo ⇒ crafting dedotto),
// la fornace che si accende, l'elytra (flag di volo nei metadati) con decollo, planata, razzi e atterraggio.
// Ogni 30 s un riassunto va all'app (mclink/observe.rs), che lo unisce al profilo del giocatore e ne
// ricava abitudini; le abitudini tornano qui e cambiano la vita di tutti i giorni.
// Parte 1: logica pura (node:test). Parte 2 (`attach`): sensi veri con Mineflayer.
"use strict";

const I = require("./inventory");

// ---------- parte 1: logica pura ----------

const SLOTS = ["helmet", "chestplate", "leggings", "boots"];
const NATURAL = /(_log|_leaves|^stone$|^dirt$|grass_block|^sand$|gravel|_ore$|deepslate$|^netherrack$|^clay$|^snow|^ice$|^andesite$|^diorite$|^granite$|^tuff$|short_grass|tall_grass|^fern$|flower|^poppy$|^dandelion$|melon|pumpkin|sugar_cane|bamboo|^cactus$|wheat|carrots|potatoes|beetroots)/;

function newAcc() {
  return {
    t: 0, dist: 0, sprint: 0, sneak: 0, air: 0, jumps: 0, held: {}, armor: {}, placed: {}, broken: {}, crafted: {}, smelted: 0, places: {},
    fly: { n: 0, t: 0, pitch: 0, pitchN: 0, speed: 0, rockets: 0, takeoff: { jump: 0, drop: 0 }, land: { soft: 0, hard: 0 }, alt: 0 },
  };
}

/** Stato di inseguimento per un giocatore (tra un campione e l'altro). */
function newTrack() {
  return { prev: null, ground: true, jumpAt: -9, flying: false, flyStart: 0, flyY: 0, flyMinY: 0, held: null, knownHeld: new Set(), tableAt: -99, tableStill: 0 };
}

const add = (o, k, v = 1) => { if (k) o[k] = (o[k] || 0) + v; };

/**
 * Un campione (≈ 5 al secondo): s = {t (s), pos: {x,y,z}, onGround, sneaking, sprinting, elytra, held, armor: {helmet…}, pitch, nearTable, lookTable}
 * Restituisce gli eventi dedotti: [{kind: "jump"|"takeoff"|"land"|"crafted", …}].
 */
function sample(acc, tr, s) {
  const out = [];
  const p = tr.prev;
  const dt = p ? Math.max(0, Math.min(1, s.t - p.t)) : 0;
  if (p && dt > 0) {
    const dx = s.pos.x - p.pos.x, dz = s.pos.z - p.pos.z, dy = s.pos.y - p.pos.y;
    const d = Math.hypot(dx, dz);
    if (d < 40 * dt) acc.dist += d; // niente teletrasporti
    acc.t += dt;
    if (s.sprinting) acc.sprint += dt;
    if (s.sneaking) acc.sneak += dt;
    if (!s.onGround) acc.air += dt;
    // salto: era a terra, adesso sale
    if (tr.ground && !s.onGround && dy > 0.2 && !s.elytra) { acc.jumps++; tr.jumpAt = s.t; out.push({ kind: "jump" }); }
    if (s.held) add(acc.held, s.held, dt);
    if (s.elytra) {
      acc.fly.t += dt;
      acc.fly.pitch += Number(s.pitch) || 0;
      acc.fly.pitchN++;
      acc.fly.speed += Math.hypot(dx, dy, dz) / dt;
      tr.flyMinY = Math.min(tr.flyMinY, s.pos.y);
    }
  }
  // dove passa il tempo (celle da 16 blocchi): la base è dove sta più spesso
  if (dt > 0) add(acc.places, `${Math.floor(s.pos.x / 16)},${Math.floor(s.pos.z / 16)}`, dt);
  for (const sl of SLOTS) if (s.armor && s.armor[sl]) acc.armor[sl] = s.armor[sl];
  if (s.armor && s.armor.chestplate === "elytra") acc.armor.chestplate = "elytra";
  // elytra: decollo e atterraggio
  if (s.elytra && !tr.flying) {
    tr.flying = true;
    tr.flyStart = s.t;
    tr.flyY = s.pos.y;
    tr.flyMinY = s.pos.y;
    const how = s.t - tr.jumpAt < 1.2 ? "jump" : "drop";
    acc.fly.n++;
    add(acc.fly.takeoff, how);
    out.push({ kind: "takeoff", how });
  } else if (!s.elytra && tr.flying) {
    tr.flying = false;
    const fall = p ? p.pos.y - s.pos.y : 0;
    const hard = fall / Math.max(dt, 0.05) > 8; // più di 8 blocchi al secondo in giù: atterraggio duro
    add(acc.fly.land, hard ? "hard" : "soft");
    acc.fly.alt += Math.max(0, tr.flyY - tr.flyMinY);
    out.push({ kind: "land", secs: s.t - tr.flyStart, hard });
  }
  // banco da lavoro: fermo davanti, che lo guarda
  const still = p && Math.hypot(s.pos.x - p.pos.x, s.pos.z - p.pos.z) < 0.15;
  if (s.lookTable && still) { tr.tableStill += dt; if (tr.tableStill >= 0.8) tr.tableAt = s.t; } else if (!s.lookTable) tr.tableStill = 0;
  // oggetto nuovo in mano poco dopo il banco ⇒ l'ha appena creato
  if (s.held && s.held !== tr.held) {
    const fresh = !tr.knownHeld.has(s.held);
    if (fresh && s.t - tr.tableAt < 12 && craftable(s.held)) { add(acc.crafted, s.held); out.push({ kind: "crafted", item: s.held }); }
    tr.knownHeld.add(s.held);
    tr.held = s.held;
  }
  tr.ground = !!s.onGround;
  tr.prev = { t: s.t, pos: { ...s.pos } };
  return out;
}

/** Cose che si fanno al banco (attrezzi, armi, armature e qualche blocco lavorato), non quelle raccolte. */
function craftable(name) {
  const c = I.classify(name);
  return c === "tool" || c === "weapon" || (c === "armor" && name !== "elytra") || /(_planks|_slab|_stairs|stick|torch|crafting_table|furnace|chest|bed$|_door|fence|ladder|bucket|shield|bow|boat|bread)/.test(name);
}

/** Un razzo vicino a chi vola: un razzo usato. */
function rocket(acc, tr) {
  if (tr.flying) acc.fly.rockets++;
}

/** Blocco cambiato vicino a un giocatore: piazzato o rotto (chi è stato lo decide `attach`). */
function blockChange(acc, oldName, newName) {
  const air = (n) => !n || n === "air" || n === "cave_air" || n === "water" || n === "lava" || n === "short_grass" || n === "tall_grass" || n === "fire";
  if (air(oldName) && !air(newName)) { add(acc.placed, newName); return "placed"; }
  if (!air(oldName) && air(newName)) { add(acc.broken, oldName); return "broken"; }
  if (oldName === "furnace" && newName === "furnace") return null;
  return null;
}

/** Riassunto compatto da mandare all'app (vuoto se non ha visto niente di utile). */
function summary(acc) {
  if (acc.t < 2) return null;
  const r = (x) => Math.round(x * 10) / 10;
  const top = (o, n = 6) => Object.fromEntries(Object.entries(o).sort((a, b) => b[1] - a[1]).slice(0, n).map(([k, v]) => [k, r(v)]));
  const s = { t: r(acc.t), dist: r(acc.dist), sprint: r(acc.sprint), sneak: r(acc.sneak), air: r(acc.air), jumps: acc.jumps, held: top(acc.held), armor: acc.armor, placed: top(acc.placed, 8), broken: top(acc.broken, 8), crafted: acc.crafted, smelted: acc.smelted, places: top(acc.places, 3) };
  if (acc.fly.n || acc.fly.t > 0) {
    s.fly = { n: acc.fly.n, t: r(acc.fly.t), pitch: acc.fly.pitchN ? r(acc.fly.pitch / acc.fly.pitchN) : 0, speed: acc.fly.pitchN ? r(acc.fly.speed / acc.fly.pitchN) : 0, rockets: acc.fly.rockets, takeoff: acc.fly.takeoff, land: acc.fly.land, alt: r(acc.fly.alt) };
  }
  return s;
}

/**
 * Stile di volo da copiare (dalle abitudini ricevute dall'app): beccheggio medio, ogni quanti secondi un razzo,
 * decollo preferito, quota. Valori sicuri se non l'ha mai visto volare.
 */
function flightStyle(fly) {
  const f = fly || {};
  const t = Math.max(1, f.t || 0);
  const pitch = Number.isFinite(f.pitch) && f.n ? Math.max(-0.6, Math.min(0.5, f.pitch)) : -0.15; // Mineflayer: pitch > 0 = muso in su, < 0 = in giù (prende velocità)
  const rocketEvery = f.rockets > 0 ? Math.max(2, Math.min(15, t / f.rockets)) : 6;
  const takeoff = f.takeoff && (f.takeoff.drop || 0) > (f.takeoff.jump || 0) ? "drop" : "jump";
  return { pitch, rocketEvery, takeoff, cruise: f.alt > 30 ? "high" : "low" };
}

/** Idee per la vita di tutti i giorni dalle abitudini del giocatore (h = {armor, gather, build, crafted, tools}). */
function ideas(h, inv) {
  const out = [];
  if (!h) return out;
  const names = (inv || []).map((i) => i.name);
  // si fa l'armatura che usa il giocatore, se ha il materiale
  const mat = h.armor;
  if (mat && mat !== "leather" && mat !== "elytra") {
    const ingot = mat === "iron" ? "iron_ingot" : mat === "diamond" ? "diamond" : mat === "golden" ? "gold_ingot" : null;
    for (const [sl, k] of [["chestplate", 8], ["leggings", 7], ["helmet", 5], ["boots", 4]]) {
      const piece = `${mat}_${sl}`;
      if (names.includes(piece)) continue;
      if (ingot && (inv || []).filter((i) => i.name === ingot).reduce((a, i) => a + i.count, 0) >= k) { out.push({ op: "craft", item: piece, n: 1, why: "copy_armor" }); break; }
    }
  }
  // attrezzi del livello che usa lui
  if (h.tools) for (const t of h.tools) if (!names.includes(t)) { out.push({ op: "craft", item: t, n: 1, why: "copy_tool" }); break; }
  // raccoglie quello che il giocatore usa spesso
  if (h.gather && h.gather.length) out.push({ op: "collect", block: h.gather[0], n: 6, why: "copy_gather" });
  return out;
}

// ---------- parte 2: sensi veri (Mineflayer) ----------

function attach(ctx) {
  const tracks = new Map();
  const accs = new Map();
  let lastSend = Date.now();
  let lastSwing = new Map();
  let t0 = Date.now();
  const now = () => (Date.now() - t0) / 1000;
  const acc = (n) => { if (!accs.has(n)) accs.set(n, newAcc()); return accs.get(n); };
  const track = (n) => { if (!tracks.has(n)) tracks.set(n, newTrack()); return tracks.get(n); };

  function others() {
    const b = ctx.bot();
    return Object.values(b.players).filter((p) => p.username !== b.username && p.entity && p.entity.position.distanceTo(b.entity.position) < 96);
  }

  function nearest(pos, maxD) {
    let best = null, bd = maxD;
    for (const p of others()) { const d = p.entity.position.distanceTo(pos); if (d < bd) { bd = d; best = p; } }
    return best;
  }

  function lookingAtTable(e) {
    const b = ctx.bot();
    const mc = ctx.mc();
    const ct = mc.blocksByName.crafting_table;
    if (!ct) return false;
    const tb = b.findBlock({ point: e.position, matching: ct.id, maxDistance: 4 });
    if (!tb) return false;
    const eye = e.position.offset(0, 1.62, 0);
    const to = tb.position.offset(0.5, 0.5, 0.5).minus(eye);
    const dir = { x: -Math.sin(e.yaw) * Math.cos(e.pitch), y: Math.sin(e.pitch), z: -Math.cos(e.yaw) * Math.cos(e.pitch) };
    const len = Math.hypot(to.x, to.y, to.z) || 1;
    return (dir.x * to.x + dir.y * to.y + dir.z * to.z) / len > 0.8;
  }

  ctx.onSpawn((b) => {
    t0 = Date.now();
    b.on("entitySwingArm", (e) => { if (e.type === "player" && e.username !== b.username) lastSwing.set(e.username, Date.now()); });
    b.on("blockUpdate", (oldB, newB) => {
      if (!ctx.cfg.observe || !oldB || !newB || oldB.name === newB.name) {
        // la fornace si accende: qualcuno sta fondendo
        if (oldB && newB && oldB.name === "furnace" && newB.getProperties && newB.getProperties().lit && !(oldB.getProperties && oldB.getProperties().lit)) {
          const p = nearest(newB.position, 6);
          if (p) acc(p.username).smelted++;
        }
        return;
      }
      const p = nearest(newB.position, 6);
      if (!p) return;
      // le cose fatte da lei non contano
      if (b.entity.position.distanceTo(newB.position) < 5 && ctx.busy() && Date.now() - (lastSwing.get(p.username) || 0) > 1500) return;
      if (Date.now() - (lastSwing.get(p.username) || 0) > 1500) return;
      const k = blockChange(acc(p.username), oldB.name, newB.name);
      if (k) ctx.lastObserved = { who: p.username, kind: k, block: k === "placed" ? newB.name : oldB.name };
    });
    b.on("entitySpawn", (e) => {
      if (!ctx.cfg.observe || !e || !/firework/.test(e.name || "")) return;
      const p = nearest(e.position, 3);
      if (p) rocket(acc(p.username), track(p.username));
    });
  });

  ctx.onFastTick(() => {
    const b = ctx.bot();
    if (!b || !b.entity || !ctx.cfg.observe) return;
    const t = now();
    for (const p of others()) {
      const e = p.entity;
      const eq = e.equipment || [];
      const nm = (i) => (eq[i] ? eq[i].name : null);
      const flags = (e.metadata && typeof e.metadata[0] === "number") ? e.metadata[0] : 0;
      const ev = sample(acc(p.username), track(p.username), {
        t, pos: { x: e.position.x, y: e.position.y, z: e.position.z }, onGround: ctx.onGround(e), sneaking: !!e.crouching, sprinting: !!(flags & 0x08),
        elytra: !!e.elytraFlying, held: nm(0), armor: { helmet: nm(5), chestplate: nm(4), leggings: nm(3), boots: nm(2) }, pitch: e.pitch, lookTable: lookingAtTable(e),
      });
      for (const x of ev) if (x.kind === "crafted" || x.kind === "takeoff") ctx.ev("observed", { who: p.username, what: x.kind, item: x.item, how: x.how });
    }
    if (Date.now() - lastSend > 30000) {
      lastSend = Date.now();
      for (const [who, a] of accs) {
        const s = summary(a);
        if (s) ctx.ev("observe", { who, s });
      }
      accs.clear();
    }
  });

  /** Vola con l'elytra nello stile del giocatore (se ha elytra e razzi). Verso x z, o avanti di `dist`. */
  ctx.SKILLS.fly = async (o, c) => {
    const b = ctx.bot();
    const st = flightStyle(ctx.habits && ctx.habits.fly);
    const ely = b.inventory.items().find((i) => i.name === "elytra") || (b.inventory.slots[6] && b.inventory.slots[6].name === "elytra" ? b.inventory.slots[6] : null);
    if (!ely) throw new Error(ctx.en() ? "I have no elytra" : "non ho l'elytra");
    if (b.inventory.slots[6] !== ely) await b.equip(ely, "torso");
    const rockets = () => b.inventory.items().find((i) => i.name === "firework_rocket");
    const tx = Number.isFinite(Number(o.x)) ? Number(o.x) : b.entity.position.x - Math.sin(b.entity.yaw) * (Number(o.dist) || 60);
    const tz = Number.isFinite(Number(o.z)) ? Number(o.z) : b.entity.position.z - Math.cos(b.entity.yaw) * (Number(o.dist) || 60);
    const yawTo = () => Math.atan2(-(tx - b.entity.position.x), -(tz - b.entity.position.z));
    // decollo: salto (oppure da un punto alto, come fa il giocatore, se c'è un dislivello qui davanti: non lo cerca)
    b.setControlState("jump", true);
    await ctx.sleep(300);
    b.setControlState("jump", false);
    await ctx.sleep(250);
    await b.elytraFly();
    let lastRocket = 0;
    const t0f = Date.now();
    try {
      while (Date.now() - t0f < 60000) {
        ctx.checkCancel(c);
        const d = Math.hypot(tx - b.entity.position.x, tz - b.entity.position.z);
        if (d < 6 || !b.entity.elytraFlying) break;
        await b.look(yawTo(), d < 20 ? 0.35 : st.pitch, true);
        if (rockets() && Date.now() - lastRocket > st.rocketEvery * 1000) {
          await b.equip(rockets(), "hand");
          b.activateItem();
          lastRocket = Date.now();
        }
        await ctx.sleep(250);
      }
    } finally {
      // atterraggio morbido: muso un po' in su
      try { await b.look(b.entity.yaw, 0.5, true); } catch {}
    }
    return { style: st };
  };

  return { accs, tracks };
}

module.exports = { newAcc, newTrack, sample, craftable, rocket, blockChange, summary, flightStyle, ideas, NATURAL, attach };
