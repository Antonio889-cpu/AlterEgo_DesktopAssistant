// Test della logica pura del bot (niente rete): node --test test/
"use strict";
const test = require("node:test");
const assert = require("node:assert");
const L = require("../lib");

test("righe del protocollo", () => {
  assert.deepStrictEqual(L.parseLine('{"id":3,"op":"follow","who":"Steve"}'), { id: 3, op: "follow", who: "Steve" });
  assert.strictEqual(L.parseLine("non json"), null);
  assert.strictEqual(L.parseLine('{"id":3}'), null, "serve op");
  assert.strictEqual(L.parseLine("{rotto"), null);
  const e = L.encode({ t: "ev", kind: "chat", text: "a\nb" });
  assert.ok(e.endsWith("\n") && e.split("\n").length === 2, "una riga sola");
  assert.deepStrictEqual(JSON.parse(e), { t: "ev", kind: "chat", text: "a\nb" });
});

test("annuncio LAN", () => {
  assert.deepStrictEqual(L.parseLanAd("[MOTD]Makoto - Nuovo mondo[/MOTD][AD]51234[/AD]"), { motd: "Makoto - Nuovo mondo", port: 51234 });
  assert.deepStrictEqual(L.parseLanAd("[MOTD]§aColori[/MOTD][AD]192.168.1.5:25565[/AD]"), { motd: "Colori", port: 25565 });
  assert.strictEqual(L.parseLanAd("[MOTD]x[/MOTD][AD]0[/AD]"), null);
  assert.strictEqual(L.parseLanAd("ciao"), null);
});

test("chat: niente comandi, righe corte", () => {
  assert.deepStrictEqual(L.splitChat("/op Alter\nciao"), ["op Alter ciao"]);
  const long = L.splitChat("parola ".repeat(80), 100);
  assert.ok(long.length >= 2 && long.every((l) => l.length <= 100));
  assert.deepStrictEqual(L.splitChat("   "), []);
  assert.ok(L.typingDelay("ciao", 0) < L.typingDelay("una frase molto più lunga da scrivere", 0));
  assert.ok(L.typingDelay("x".repeat(1000), 1) <= 3500);
});

test("lato delle cose (yaw di Mineflayer)", () => {
  const me = { x: 0, z: 0 };
  // yaw 0 = guarda verso -Z (nord): est (+X) è a destra
  assert.strictEqual(L.side(me, 0, { x: 0, z: -5 }), "front");
  assert.strictEqual(L.side(me, 0, { x: 0, z: 5 }), "back");
  assert.strictEqual(L.side(me, 0, { x: 5, z: 0 }), "right");
  assert.strictEqual(L.side(me, 0, { x: -5, z: 0 }), "left");
  // girata verso est (yaw = -π/2): sud (+Z) è a destra
  assert.strictEqual(L.side(me, -Math.PI / 2, { x: 5, z: 0 }), "front");
  assert.strictEqual(L.side(me, -Math.PI / 2, { x: 0, z: 5 }), "right");
});

test("sensi per il cervello di moscerino", () => {
  assert.deepStrictEqual(L.senses({}), {});
  const s = L.senses({ hurt: 3, threats: [{ d: 3, side: "left", closing: true }, { d: 20, side: "right", closing: true }], creeper: 2, fuse: true, boom: 0.2, hitBy: true, water: true, fall: -1.5, light: 14, lightBefore: 2 });
  assert.ok(s.pain > 0.4 && s.pain <= 1);
  assert.ok(s.loom_l > 0.8 && !s.loom_r, "la minaccia lontana non conta");
  assert.strictEqual(s.smell_danger, 1);
  assert.ok(s.sound > 0.8 && s.touch === 1 && s.water === 1 && s.wind > 0.5 && s.light > 0.9);
  const f = L.senses({ food: 0.7, hungry: true, threats: [{ d: 5, side: "front", closing: false }] });
  assert.ok(f.sweet === 0.7 && f.smell_food > 0 && f.loom_l > 0 && f.loom_r > 0);
  for (const v of Object.values(s)) assert.ok(v >= 0 && v <= 1);
});

test("riflessi, cibo, giocatori, inventario", () => {
  assert.deepStrictEqual(L.reflexPlan("strafe_l"), [["left", 450]]);
  assert.strictEqual(L.reflexPlan("boh").length, 0);
  assert.strictEqual(L.bestFood(["dirt", "apple", "cooked_beef"]), "cooked_beef");
  assert.strictEqual(L.bestFood(["dirt"]), null);
  assert.strictEqual(L.findPlayer(["Makoto_N", "Kyoko"], "makoto"), "Makoto_N");
  assert.strictEqual(L.findPlayer(["Makoto_N", "Kyoko"], "yok"), "Kyoko");
  assert.strictEqual(L.findPlayer(["Makoto_N"], ""), null);
  assert.strictEqual(L.invLine([{ name: "dirt", count: 3 }, { name: "oak_log", count: 12 }, { name: "dirt", count: 2 }, { name: "torch", count: 1 }]), "oak_log x12, dirt x5, torch");
});

test("motivo dell'espulsione in testo semplice", () => {
  assert.strictEqual(L.flatten('{"translate":"multiplayer.disconnect.unverified_username"}'), "multiplayer.disconnect.unverified_username");
  assert.strictEqual(L.flatten({ text: "Failed to verify ", extra: [{ text: "username!" }] }), "Failed to verify username!");
  assert.strictEqual(L.flatten("Server closed"), "Server closed");
  assert.strictEqual(L.flatten(null), "");
});

test("battute senza ripetizioni e forme", () => {
  const used = new Set();
  const a = L.pickLine(["a", "b"], used, 0);
  const b = L.pickLine(["a", "b"], used, 0);
  assert.notStrictEqual(a, b);
  assert.strictEqual(L.pickLine([], used), null);
  for (const lang of ["it", "en"]) assert.ok(Object.keys(L.FALLBACK[lang]).length >= 10);
  assert.strictEqual(L.shape("tower", 4).length, 4);
  const hut = L.shape("hut", 5);
  assert.ok(hut.length > 40 && hut.every(([x, y, z]) => x >= 0 && x < 5 && z >= 0 && z < 5 && y >= 0 && y <= 3));
  assert.ok(!hut.some(([x, y, z]) => x === 2 && z === 0 && y < 2), "c'è la porta");
  assert.deepStrictEqual(L.shape("boh"), []);
});
