// Rete del bot: ping di stato prima di entrare (versione del server: Mineflayer la conosce o serve il
// ponte ViaProxy?), tutti i messaggi del server per la console e per il login (AuthMe), comandi "/"
// mandati dall'app, cambi di mondo e di sottoserver (marchio del server: BungeeCord, Velocity…).
// Il cervello che decide (login, inviti, warp) è nell'app: src-tauri/src/mclink/{via,auth,servers}.rs.
"use strict";

// ---------- logica pura (node --test test/) ----------

/**
 * Versione che Mineflayer sa parlare con il protocollo `protocol` del server, o null.
 * `versions`: minecraft-data .versions.pc ({ minecraftVersion, version, releaseType }), dal più nuovo.
 * `oldest`/`latest`: versioni estreme di Mineflayer ("1.8.8", "26.1").
 * `speaks(nome, protocollo)`: minecraft-data ha davvero i dati di quella versione con quel protocollo
 * (per alcune, come 1.21.7, ricade sulla 1.21 con un protocollo diverso).
 */
function nativeVersion(protocol, versions, oldest, latest, speaks = () => true) {
  const p = Number(protocol);
  if (!Number.isInteger(p) || p <= 0 || !Array.isArray(versions)) return null;
  const proto = (name) => { const v = versions.find((x) => x.minecraftVersion === name); return v ? v.version : null; };
  const lo = proto(oldest), hi = proto(latest);
  if (lo == null || hi == null || p < lo || p > hi) return null;
  // stesso protocollo per più versioni (1.21.9 e 1.21.10): la più nuova pubblicata
  const rel = versions.filter((v) => v.version === p && (!v.releaseType || v.releaseType === "release"));
  const any = (rel.length ? rel : versions.filter((v) => v.version === p)).filter((v) => speaks(v.minecraftVersion, p));
  return any.length ? any[0].minecraftVersion : null;
}

/** Testo semplice da un MOTD (stringa o componente di chat). */
function flat(c) {
  if (c == null) return "";
  if (typeof c === "string") return c;
  if (Array.isArray(c)) return c.map(flat).join("");
  let s = typeof c.text === "string" ? c.text : "";
  if (Array.isArray(c.extra)) s += c.extra.map(flat).join("");
  if (!s && typeof c.translate === "string") s = c.translate;
  return s;
}

/** Riga del protocollo con la risposta al ping (o l'errore). */
function pingResult(data, err, host, port, versions, oldest, latest, speaks) {
  if (err || !data) return { t: "ping", ok: false, host, port, err: String((err && err.message) || err || "no answer").slice(0, 200), latest };
  const v = data.version || {};
  const protocol = Number(v.protocol) || 0;
  return {
    t: "ping", ok: true, host, port, protocol,
    name: String(v.name || "").slice(0, 80),
    motd: flat(data.description).replace(/§./g, "").replace(/\s+/g, " ").trim().slice(0, 160),
    native: nativeVersion(protocol, versions, oldest, latest, speaks),
    latest,
  };
}

/** Un comando di gioco da mandare: inizia con "/", una riga, al massimo 256 caratteri. */
function isCommand(text) {
  const s = String(text || "");
  return /^\/[^\s/]/.test(s) && !/[\r\n]/.test(s) && s.length <= 256;
}

/** Posizione del messaggio di Mineflayer → tipo per la console ("chat", "sys") o null (barra delle azioni). */
function classify(position) {
  if (position === "chat") return "chat";
  if (position === "game_info") return null;
  return "sys";
}

/** Marchio del server più leggibile (senza colori). */
function cleanBrand(b) {
  return String(b || "").replace(/§./g, "").replace(/\s+/g, " ").trim().slice(0, 120);
}

/**
 * Il client vero manda "tick_end" a ogni tick dalla 1.21.2; Mineflayer no. I server 26.2+ (anche
 * attraverso ViaProxy) buttano fuori per "invalid_player_movement" chi non lo manda: lo mandiamo noi
 * se il protocollo della versione lo prevede. `types`: minecraft-data .protocol.play.toServer.types.
 */
function sendsTickEnd(types) {
  return !!(types && types.packet_tick_end);
}

/** Argomenti di `node bot.js --ping host porta`. */
function pingArgs(argv) {
  const i = argv.indexOf("--ping");
  const host = String(argv[i + 1] || "127.0.0.1");
  const port = Math.max(0, Math.min(65535, Number(argv[i + 2]) || 0));
  return { host, port };
}

// ---------- con la rete ----------

/** node bot.js --ping host porta: stampa una riga {"t":"ping",…} ed esce. Porta 0: cerca prima il mondo in LAN. */
async function pingMain(argv, scanLan, out) {
  const data = require("minecraft-data");
  const mineflayer = require("mineflayer");
  const versions = data.versions.pc;
  const oldest = mineflayer.oldestSupportedVersion, latest = mineflayer.latestSupportedVersion;
  const speaks = (name, p) => { try { const d = data(name); return !!d && d.version.version === p; } catch { return false; } };
  let { host, port } = pingArgs(argv);
  if (!port) {
    const list = await scanLan(3500);
    const w = list.find((x) => x.host === host || host === "127.0.0.1" || host === "localhost") || list[0];
    if (!w) { out(pingResult(null, "no LAN world found", host, 0, versions, oldest, latest, speaks)); return process.exit(0); }
    port = w.port;
    if (host === "localhost" || host === "127.0.0.1") host = w.host === "127.0.0.1" ? host : w.host;
  }
  const mc = require("minecraft-protocol");
  let res, err;
  try {
    res = await Promise.race([
      mc.ping({ host, port, closeTimeout: 8000, noPongTimeout: 1500 }),
      new Promise((_, rej) => setTimeout(() => rej(new Error("timeout")), 9000)),
    ]);
  } catch (e) {
    err = e;
  }
  out(pingResult(res, err, host, port, versions, oldest, latest, speaks));
  process.exit(0);
}

/** Collega al bot i messaggi del server, i cambi di mondo e il marchio del server. */
function attach(bot, { ev, log }) {
  let brand = "";
  const checkBrand = () => {
    const b = cleanBrand(bot.game && bot.game.serverBrand);
    if (b && b !== brand) { brand = b; ev("net", { brand: b }); }
  };
  bot.on("messagestr", (msg, position) => {
    const k = classify(position);
    if (!k) return;
    const text = String(msg || "").slice(0, 500);
    if (text.trim()) ev("sys", { text, pos: k });
  });
  let first = true;
  bot.on("spawn", () => {
    setTimeout(checkBrand, 1500);
    if (first) { first = false; return; }
    ev("world", { dim: String((bot.game && bot.game.dimension) || "").replace("minecraft:", "") });
  });
  bot.on("respawn", () => {
    ev("world", { dim: String((bot.game && bot.game.dimension) || "").replace("minecraft:", "") });
    setTimeout(checkBrand, 1500);
  });
  bot.on("login", () => setTimeout(checkBrand, 2000));
  // fine del tick come il client vero (vedi sendsTickEnd)
  let tickEnd = null;
  bot.once("login", () => {
    try { tickEnd = sendsTickEnd(require("minecraft-data")(bot.version).protocol.play.toServer.types); } catch { tickEnd = false; }
    if (tickEnd) log("tick_end attivo per " + bot.version);
  });
  bot.on("physicsTick", () => {
    if (!tickEnd) return;
    try { bot._client.write("tick_end", {}); } catch (e) { tickEnd = false; log("tick_end:", e.message); }
  });
}

const OPS = new Set(["cmd", "net"]);

/** Comandi dall'app gestiti qui: "cmd" (un comando "/…", anche con la password: mai nei log), "net". */
function handle(cmd, bot, ack) {
  if (!bot || !bot.entity) return ack(cmd, false, "not connected");
  if (cmd.op === "cmd") {
    const text = String(cmd.text || "").trim();
    if (!isCommand(text)) return ack(cmd, false, "only /commands");
    try { bot.chat(text); } catch (e) { return ack(cmd, false, "chat: " + (e && e.message ? e.message : e)); }
    return ack(cmd, true);
  }
  if (cmd.op === "net") {
    return ack(cmd, true, null, { brand: cleanBrand(bot.game && bot.game.serverBrand), dim: String((bot.game && bot.game.dimension) || ""), version: bot.version });
  }
  return ack(cmd, false, "unknown op " + cmd.op);
}

module.exports = { nativeVersion, flat, pingResult, isCommand, classify, cleanBrand, sendsTickEnd, pingArgs, pingMain, attach, handle, OPS };
