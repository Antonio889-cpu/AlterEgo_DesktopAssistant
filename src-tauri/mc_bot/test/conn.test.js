// Test della logica pura di conn.js (niente rete): node --test test/
"use strict";
const test = require("node:test");
const assert = require("node:assert");
const C = require("../conn");

// pezzo di minecraft-data .versions.pc (dal più nuovo)
const PC = [
  { minecraftVersion: "26.4-snapshot-1", version: 1073742163, releaseType: "snapshot" },
  { minecraftVersion: "26.3", version: 777, releaseType: "release" },
  { minecraftVersion: "26.2", version: 776, releaseType: "release" },
  { minecraftVersion: "26.1.2", version: 775, releaseType: "release" },
  { minecraftVersion: "26.1", version: 775, releaseType: "release" },
  { minecraftVersion: "1.21.10", version: 773, releaseType: "release" },
  { minecraftVersion: "1.21.9", version: 773, releaseType: "release" },
  { minecraftVersion: "1.21.8", version: 772, releaseType: "release" },
  { minecraftVersion: "1.21.7", version: 772, releaseType: "release" },
  { minecraftVersion: "1.21.4", version: 769, releaseType: "release" },
  { minecraftVersion: "1.8.9", version: 47, releaseType: "release" },
  { minecraftVersion: "1.8.8", version: 47, releaseType: "release" },
  { minecraftVersion: "1.7.10", version: 5, releaseType: "release" },
];

test("versione nativa o ponte", () => {
  assert.strictEqual(C.nativeVersion(769, PC, "1.8.8", "26.1"), "1.21.4");
  assert.strictEqual(C.nativeVersion(775, PC, "1.8.8", "26.1"), "26.1.2", "la più nuova con quel protocollo");
  assert.strictEqual(C.nativeVersion(47, PC, "1.8.8", "26.1"), "1.8.9");
  // più nuove o più vecchie di Mineflayer: serve il ponte
  assert.strictEqual(C.nativeVersion(777, PC, "1.8.8", "26.1"), null);
  assert.strictEqual(C.nativeVersion(776, PC, "1.8.8", "26.1"), null);
  assert.strictEqual(C.nativeVersion(5, PC, "1.8.8", "26.1"), null);
  assert.strictEqual(C.nativeVersion(1073742163, PC, "1.8.8", "26.1"), null, "snapshot");
  assert.strictEqual(C.nativeVersion(770, PC, "1.8.8", "26.1"), null, "protocollo sconosciuto");
  assert.strictEqual(C.nativeVersion("x", PC, "1.8.8", "26.1"), null);
  assert.strictEqual(C.nativeVersion(769, PC, "1.8.8", "99.9"), null, "estremi sconosciuti");
  // minecraft-data senza dati veri per 1.21.8 / 1.21.7: niente versione nativa
  assert.strictEqual(C.nativeVersion(772, PC, "1.8.8", "26.1", (n) => n === "1.21.7"), "1.21.7");
  assert.strictEqual(C.nativeVersion(772, PC, "1.8.8", "26.1", () => false), null);
});

test("risposta al ping", () => {
  const ok = C.pingResult({ version: { name: "Paper 26.3", protocol: 777 }, description: { text: "§aCiao ", extra: [{ text: "mondo" }] } }, null, "mc.example.net", 25565, PC, "1.8.8", "26.1");
  assert.deepStrictEqual(ok, { t: "ping", ok: true, host: "mc.example.net", port: 25565, protocol: 777, name: "Paper 26.3", motd: "Ciao mondo", native: null, latest: "26.1" });
  const nat = C.pingResult({ version: { name: "1.21.4", protocol: 769 }, description: "A Minecraft Server" }, null, "h", 1, PC, "1.8.8", "26.1");
  assert.strictEqual(nat.native, "1.21.4");
  const bad = C.pingResult(null, new Error("connect ECONNREFUSED"), "h", 1, PC, "1.8.8", "26.1");
  assert.strictEqual(bad.ok, false);
  assert.match(bad.err, /ECONNREFUSED/);
  assert.strictEqual(bad.latest, "26.1");
  assert.strictEqual(C.flat([{ text: "a" }, "b", { translate: "c" }]), "abc");
});

test("solo comandi, una riga", () => {
  assert.ok(C.isCommand("/login segreta"));
  assert.ok(C.isCommand("/warp spawn"));
  assert.ok(!C.isCommand("login segreta"), "chat pubblica: no");
  assert.ok(!C.isCommand("/ ciao"));
  assert.ok(!C.isCommand("//wand"), "niente doppia barra");
  assert.ok(!C.isCommand("/a\n/b"));
  assert.ok(!C.isCommand("/" + "x".repeat(300)));
  assert.ok(!C.isCommand(null));
});

test("messaggi, marchio, argomenti", () => {
  assert.strictEqual(C.classify("chat"), "chat");
  assert.strictEqual(C.classify("system"), "sys");
  assert.strictEqual(C.classify("game_info"), null, "barra delle azioni: no");
  assert.strictEqual(C.cleanBrand("§eBungeeCord §7<- Paper"), "BungeeCord <- Paper");
  assert.strictEqual(C.sendsTickEnd({ packet_tick_end: ["container", []] }), true);
  assert.strictEqual(C.sendsTickEnd({ packet_position: [] }), false);
  assert.strictEqual(C.sendsTickEnd(null), false);
  assert.deepStrictEqual(C.pingArgs(["node", "bot.js", "--ping", "mc.example.net", "25566"]), { host: "mc.example.net", port: 25566 });
  assert.deepStrictEqual(C.pingArgs(["node", "bot.js", "--ping"]), { host: "127.0.0.1", port: 0 });
  assert.deepStrictEqual(C.pingArgs(["node", "bot.js", "--ping", "h", "99999"]), { host: "h", port: 65535 });
});

test("comandi dall'app", () => {
  const acks = [];
  const ack = (cmd, ok, err, info) => acks.push({ id: cmd.id, ok, err, info });
  const said = [];
  const bot = { entity: {}, chat: (t) => said.push(t), game: { serverBrand: "Paper", dimension: "minecraft:overworld" }, version: "26.1" };
  C.handle({ id: 1, op: "cmd", text: "/login x" }, bot, ack);
  C.handle({ id: 2, op: "cmd", text: "ciao a tutti" }, bot, ack);
  C.handle({ id: 3, op: "net" }, bot, ack);
  C.handle({ id: 4, op: "cmd", text: "/x" }, null, ack);
  assert.deepStrictEqual(said, ["/login x"]);
  assert.strictEqual(acks[0].ok, true);
  assert.strictEqual(acks[0].err, undefined, "la risposta non ripete il comando");
  assert.strictEqual(acks[1].ok, false);
  assert.deepStrictEqual(acks[2].info, { brand: "Paper", dim: "minecraft:overworld", version: "26.1" });
  assert.strictEqual(acks[3].ok, false);
  assert.ok(C.OPS.has("cmd") && !C.OPS.has("say"));
});
