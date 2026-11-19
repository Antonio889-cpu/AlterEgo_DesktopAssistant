// Logica pura del corpo di Alter Ego in Minecraft (niente rete, niente Mineflayer): si prova con node:test.
// Alcune idee (abilità come funzioni piccole, comportamenti continui senza modello) vengono da
// Mindcraft (https://github.com/mindcraft-bots/mindcraft, MIT).
"use strict";

/** Riga JSON da Rust (stdin) → comando, o null se non va bene. */
function parseLine(line) {
  const s = String(line || "").trim();
  if (!s || s[0] !== "{") return null;
  try {
    const v = JSON.parse(s);
    return v && typeof v === "object" && typeof v.op === "string" ? v : null;
  } catch {
    return null;
  }
}

/** Messaggio per Rust (stdout): una riga JSON. */
function encode(obj) {
  return JSON.stringify(obj).replace(/\n/g, " ") + "\n";
}

/** Annuncio LAN di Minecraft (UDP 224.0.2.60:4445): "[MOTD]mondo[/MOTD][AD]51234[/AD]". */
function parseLanAd(msg) {
  const s = String(msg || "");
  const m = /\[MOTD\]([\s\S]*?)\[\/MOTD\]\s*\[AD\]([^\[]*)\[\/AD\]/.exec(s);
  if (!m) return null;
  const ad = m[2].trim();
  // a volte "host:porta", di solito solo la porta
  const port = Number(ad.includes(":") ? ad.slice(ad.lastIndexOf(":") + 1) : ad);
  if (!Number.isInteger(port) || port < 1 || port > 65535) return null;
  return { motd: m[1].replace(/§./g, "").trim(), port };
}

/** Testo per la chat: una riga, niente comandi ("/"), al massimo `max` caratteri per messaggio. */
function splitChat(text, max = 240) {
  let t = String(text || "").replace(/[\r\n]+/g, " ").replace(/\s+/g, " ").trim();
  while (t.startsWith("/")) t = t.slice(1).trim();
  if (!t) return [];
  const out = [];
  while (t.length > max) {
    let cut = t.lastIndexOf(" ", max);
    if (cut < max / 2) cut = max;
    out.push(t.slice(0, cut).trim());
    t = t.slice(cut).trim();
  }
  if (t) out.push(t);
  return out.slice(0, 3);
}

/** Tempo per "scrivere" una riga, come una persona (ms). `r` in 0..1. */
function typingDelay(text, r = 0.5) {
  const n = String(text || "").length;
  return Math.round(Math.min(3500, 300 + n * 28 + r * 500));
}

/** Da che lato sta `to` rispetto a chi guarda con `yaw` (Mineflayer: avanti = (-sin yaw, -cos yaw)). */
function side(from, yaw, to) {
  const dx = to.x - from.x;
  const dz = to.z - from.z;
  const fx = -Math.sin(yaw);
  const fz = -Math.cos(yaw);
  const front = dx * fx + dz * fz;
  const right = dx * -fz + dz * fx;
  if (Math.abs(front) >= Math.abs(right)) return front >= 0 ? "front" : "back";
  return right > 0 ? "right" : "left";
}

const clamp01 = (x) => (Number.isFinite(x) ? Math.max(0, Math.min(1, x)) : 0);

/**
 * Sensi per il cervello di moscerino (stessi nomi di gmodlink/fly.rs), da una fotografia del mondo:
 *  hurt: danno appena preso (cuori, 0..20); threats: [{d, side, closing}] mob ostili e frecce;
 *  creeper: distanza del creeper più vicino (o null) e se sta per esplodere (fuse);
 *  boom: secondi dall'ultima esplosione; food: cibo per terra vicino o in mano con fame;
 *  hitBy: colpita da un giocatore; water, fall: velocità verso il basso; light: luce adesso e prima.
 */
function senses(w) {
  const s = {};
  const hurt = Number(w.hurt) || 0;
  if (hurt > 0) s.pain = clamp01(hurt / 6);
  let l = 0, r = 0;
  for (const t of w.threats || []) {
    if (!(t.d > 0) || t.d > 10) continue;
    const k = clamp01((10 - t.d) / 8) * (t.closing ? 1 : 0.5);
    if (t.side === "left") l = Math.max(l, k);
    else if (t.side === "right") r = Math.max(r, k);
    else { l = Math.max(l, k * 0.8); r = Math.max(r, k * 0.8); }
  }
  if (l > 0.05) s.loom_l = l;
  if (r > 0.05) s.loom_r = r;
  if (w.creeper != null && w.creeper < 7) s.smell_danger = clamp01((7 - w.creeper) / 5 + (w.fuse ? 0.5 : 0));
  if (w.boom != null && w.boom < 1.5) s.sound = clamp01(1 - w.boom / 1.5);
  if (w.food) s.sweet = clamp01(w.food);
  if (w.food && w.hungry) s.smell_food = 0.6;
  if (w.hitBy) s.touch = 1;
  if (w.water) { s.water = 1; s.wet = 1; }
  if (w.fall != null && w.fall < -0.6) s.wind = clamp01((-w.fall - 0.6) / 1.2);
  // dal buio vero alla luce piena (uscendo da una grotta): abbagliata
  if (w.light != null && w.lightBefore != null && w.lightBefore <= 4 && w.light >= 12) s.light = clamp01((w.light - w.lightBefore) / 12);
  return s;
}

/** Movimenti di un riflesso: [[controllo, ms], …] (eseguiti uno dopo l'altro). */
function reflexPlan(kind, threat) {
  switch (kind) {
    case "flee": return [["sprint+forward", 1400]]; // prima si gira dall'altra parte
    case "strafe_l": return [["left", 450]];
    case "strafe_r": return [["right", 450]];
    case "back": return [["back", 500]];
    case "jump": return [["jump", 250]];
    case "flinch": return [["sneak", 350]];
    case "swim_up": return [["jump", 900]];
    default: return [];
  }
}

const HOSTILE = new Set([
  "zombie", "skeleton", "creeper", "spider", "cave_spider", "enderman", "witch", "slime", "magma_cube", "drowned", "husk", "stray",
  "phantom", "pillager", "vindicator", "evoker", "ravager", "vex", "blaze", "ghast", "hoglin", "zoglin", "piglin_brute", "wither_skeleton",
  "guardian", "elder_guardian", "silverfish", "endermite", "shulker", "zombie_villager", "bogged", "breeze", "warden",
]);

const FOODS = [
  "golden_carrot", "cooked_beef", "cooked_porkchop", "cooked_mutton", "cooked_salmon", "cooked_chicken", "cooked_cod", "cooked_rabbit",
  "bread", "baked_potato", "pumpkin_pie", "mushroom_stew", "beetroot_soup", "rabbit_stew", "apple", "golden_apple", "carrot", "melon_slice",
  "sweet_berries", "glow_berries", "cookie", "beetroot", "potato", "dried_kelp", "beef", "porkchop", "mutton", "chicken", "rabbit", "cod", "salmon",
];
const SWEET = new Set(["apple", "golden_apple", "cookie", "pumpkin_pie", "melon_slice", "sweet_berries", "glow_berries", "cake", "honey_bottle"]);

/** Il cibo migliore nell'inventario (nomi degli oggetti), o null. */
function bestFood(names) {
  for (const f of FOODS) if (names.includes(f)) return f;
  return null;
}

/** Giocatore per nome, anche scritto a metà o in minuscolo. */
function findPlayer(names, who) {
  const w = String(who || "").trim().toLowerCase();
  if (!w) return null;
  return names.find((n) => n.toLowerCase() === w) || names.find((n) => n.toLowerCase().startsWith(w)) || names.find((n) => n.toLowerCase().includes(w)) || null;
}

/** Testo semplice da un messaggio di Minecraft (stringa, JSON o componente di chat). */
function flatten(msg) {
  if (msg == null) return "";
  if (typeof msg === "string") {
    const s = msg.trim();
    if (s.startsWith("{") || s.startsWith("[") || s.startsWith('"')) {
      try { return flatten(JSON.parse(s)); } catch { return s; }
    }
    return s;
  }
  if (Array.isArray(msg)) return msg.map(flatten).join("");
  if (typeof msg === "object") {
    if (typeof msg.toString === "function" && msg.toString !== Object.prototype.toString) {
      const t = msg.toString();
      if (t && t !== "[object Object]") return t;
    }
    let s = msg.text != null ? String(msg.text) : msg.translate != null ? String(msg.translate) : "";
    if (Array.isArray(msg.with)) s += " " + msg.with.map(flatten).join(" ");
    if (Array.isArray(msg.extra)) s += msg.extra.map(flatten).join("");
    return s.trim();
  }
  return String(msg);
}

/** Inventario compatto: "oak_log x12, stone x30" (i più numerosi prima). */
function invLine(items, max = 10) {
  const sum = new Map();
  for (const it of items || []) sum.set(it.name, (sum.get(it.name) || 0) + (it.count || 1));
  return [...sum.entries()].sort((a, b) => b[1] - a[1]).slice(0, max).map(([n, c]) => (c > 1 ? `${n} x${c}` : n)).join(", ");
}

/** Battute di riserva (senza modello). Le categorie sono le stesse di mclink/quips.rs. */
const FALLBACK = {
  it: {
    hurt: ["ahia", "ehi!", "ouch", "ma dai"], flee: ["scappo!", "via via via", "nooo"], died: ["noo... game over", "ok... respawn", "rip"],
    creeper: ["creeper!!", "sssh... no", "attento al creeper"], greet: ["ciaoo", "eccomi!", "ehi!"], found: ["guarda qua!", "trovato!", "ooh"],
    done: ["fatto!", "ecco qua", "finito :)"], night: ["si fa buio...", "notte, occhio ai mob"], hungry: ["ho fame...", "mi serve cibo"],
    oops: ["ops", "non ci arrivo...", "uff"], bored: ["che facciamo?", "hmm", "noia..."], eat: ["gnam", "buono"], dodge: ["mancato!", "eheh"],
  },
  en: {
    hurt: ["ow", "hey!", "ouch", "come on"], flee: ["nope!", "run run run", "nooo"], died: ["noo i died", "ok... respawn", "rip"],
    creeper: ["creeper!!", "nope nope", "creeper behind you"], greet: ["hii", "here i am!", "hey!"], found: ["look at this!", "found it!", "ooh"],
    done: ["done!", "there you go", "finished :)"], night: ["it's getting dark...", "night, watch out for mobs"], hungry: ["i'm hungry...", "need food"],
    oops: ["oops", "can't reach it...", "ugh"], bored: ["what do we do?", "hmm", "bored..."], eat: ["nom", "yum"], dodge: ["missed!", "hehe"],
  },
};

/** Sceglie una riga senza ripetere le ultime usate (`used`: Set condiviso). */
function pickLine(lines, used, r = Math.random()) {
  const list = (lines || []).filter(Boolean);
  if (!list.length) return null;
  const fresh = list.filter((l) => !used.has(l));
  const pool = fresh.length ? fresh : list;
  const line = pool[Math.floor(r * pool.length) % pool.length];
  used.add(line);
  if (used.size > 40) used.delete(used.values().next().value);
  return line;
}

/** Forme da costruire: posizioni relative (x, y, z) dei blocchi, dal basso verso l'alto. */
function shape(kind, size) {
  const n = Math.max(2, Math.min(9, Math.round(size || 0) || 0));
  const out = [];
  if (kind === "tower" || kind === "pillar") {
    for (let y = 0; y < (size ? n : 5); y++) out.push([0, y, 0]);
  } else if (kind === "wall") {
    const w = size ? n : 5;
    for (let y = 0; y < 3; y++) for (let x = 0; x < w; x++) out.push([x, y, 0]);
  } else if (kind === "hut" || kind === "house" || kind === "shelter") {
    const w = size ? Math.max(3, n) : 5;
    for (let y = 0; y < 3; y++) {
      for (let x = 0; x < w; x++) {
        for (let z = 0; z < w; z++) {
          const edge = x === 0 || z === 0 || x === w - 1 || z === w - 1;
          const door = z === 0 && x === Math.floor(w / 2) && y < 2;
          if (edge && !door) out.push([x, y, z]);
        }
      }
    }
    for (let x = 0; x < w; x++) for (let z = 0; z < w; z++) out.push([x, 3, z]);
  }
  return out;
}

module.exports = {
  parseLine, encode, parseLanAd, splitChat, typingDelay, side, senses, reflexPlan, HOSTILE, FOODS, SWEET, bestFood,
  findPlayer, flatten, invLine, FALLBACK, pickLine, shape,
};
