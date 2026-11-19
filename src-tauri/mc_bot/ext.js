// Collega a bot.js le parti nuove: inventario, crafting ricorsivo, osservazione del giocatore, schematiche e
// costruzione, duelli. bot.js passa le sue funzioni interne (`core`), qui si costruisce il contesto comune e
// ogni modulo aggiunge le sue abilità (SKILLS) e i suoi giri periodici.
// Comandi immediati in più: "habits" (abitudini dall'app), "answer" (risposta a una domanda), "duel_tactic".
"use strict";

const Inv = require("./inventory");
const Craft = require("./craft");
const Obs = require("./observe");
const Build = require("./build");
const Pvp = require("./pvp");

module.exports = function ext(core) {
  const ticks = [];
  const fast = [];
  const spawns = [];
  const ops = new Map();
  const waiting = new Map(); // domande in attesa di risposta: chiave → resolve
  let timers = [];

  // impostazioni nuove: accese finché l'app non dice altro
  for (const k of ["observe", "tidy", "duels", "ask"]) if (core.cfg[k] === undefined) core.cfg[k] = true;

  const ctx = {
    ...core,
    Vec3: require("vec3").Vec3,
    cfg: core.cfg,
    habits: null,
    favorites: () => (ctx.habits && ctx.habits.favorites) || [],
    onTick: (f) => ticks.push(f),
    onFastTick: (f) => fast.push(f),
    onSpawn: (f) => spawns.push(f),
    onOp: (name, f) => ops.set(name, f),
    isFood: (n) => Inv.FOOD.has(n),
    /**
     * Chiede all'app (che chiede al giocatore in chat, o risponde da quello che ricorda). Restituisce la
     * risposta (true/false o testo) o `def` se nessuno risponde entro `ms`.
     */
    ask(key, info, ms = 45000, def = null) {
      return new Promise((resolve) => {
        const t = setTimeout(() => { waiting.delete(key); resolve(def); }, ms);
        waiting.set(key, (v) => { clearTimeout(t); waiting.delete(key); resolve(v); });
        core.ev("doubt", { key, ...info });
      });
    },
  };

  const inv = Inv.attach(ctx);
  ctx.equipFor = inv.equipFor;
  Craft.attach(ctx);
  Obs.attach(ctx);
  Build.attach(ctx);
  Pvp.attach(ctx);

  ops.set("habits", (o) => { ctx.habits = o.h || null; });
  ops.set("answer", (o) => { const f = waiting.get(String(o.key || "")); if (f) f(o.value); });

  return {
    ctx,
    /** Comando immediato di questi moduli? (true = gestito) */
    op(cmd) {
      const f = ops.get(cmd.op);
      if (!f) return false;
      try { f(cmd); core.ack(cmd, true); } catch (e) { core.ack(cmd, false, e.message); }
      return true;
    },
    spawned(bot) {
      for (const f of spawns) { try { f(bot); } catch (e) { core.log("ext spawn:", e.message); } }
      timers.forEach(clearInterval);
      timers = [
        setInterval(() => { for (const f of fast) { try { f(); } catch (e) { core.log("ext:", e.message); } } }, 200),
        setInterval(() => { for (const f of ticks) { Promise.resolve().then(f).catch((e) => core.log("ext:", e.message)); } }, 1000),
      ];
    },
    stopped() { timers.forEach(clearInterval); timers = []; },
    /** Idee in più per quando si annoia: copia le abitudini del giocatore. */
    ideas() {
      const b = core.bot();
      if (!b || !b.entity || !ctx.cfg.observe) return [];
      return Obs.ideas(ctx.habits, b.inventory.items()).slice(0, 1);
    },
  };
};
