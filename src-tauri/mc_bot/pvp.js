// Duelli PvP che imparano. Solo su richiesta ("duelliamo"), con regole chiare: conto alla rovescia, fine a
// pochi cuori (o al primo KO, se lo chiede il giocatore), niente danni fuori dal duello.
// Durante il duello registra come combatte il giocatore (distanza preferita, strafe, critici in salto,
// w-tap, scudo, arco, canna da pesca, mangiare, perle, fuga); dopo ogni round l'app (mclink/pvp.rs) aggiorna
// il profilo di combattimento e sceglie la tattica del round dopo (bandit + counter, Rizzo Flow se c'è).
// Il cervello di moscerino resta per i riflessi (scarti, sobbalzi) anche durante il duello.
// Parte 1: logica pura (node:test). Parte 2 (`attach`): il duello vero con Mineflayer.
"use strict";

// ---------- parte 1: logica pura ----------

/** Tattiche: distanza da tenere, strafe, critici in salto, w-tap, scudo, arco, ascia contro lo scudo. */
const TACTICS = {
  rush: { dist: 2.2, strafe: 0, crit: true, wtap: false, shield: false, bow: false, desc: "stay close and spam jump crits" },
  spacing: { dist: 3.1, strafe: 0.4, crit: false, wtap: true, shield: false, bow: false, desc: "keep max reach, hit as they step in, w-tap" },
  strafe: { dist: 2.7, strafe: 1, crit: false, wtap: true, shield: false, bow: false, desc: "circle strafe, hard to hit" },
  shield: { dist: 2.8, strafe: 0.3, crit: false, wtap: false, shield: true, bow: false, desc: "block with the shield, counter-hit" },
  kite: { dist: 7, strafe: 0.6, crit: false, wtap: false, shield: false, bow: true, desc: "keep away and shoot with the bow" },
  axe: { dist: 2.6, strafe: 0.2, crit: true, wtap: false, shield: false, bow: false, axe: true, desc: "axe to disable the shield, then crits" },
};
const NAMES = Object.keys(TACTICS);

/** Regole da una frase: {endHp: vita a cui si ferma (0 = KO), bestOf, noBow, noRod, noPearls}. */
function parseRules(text, base = {}) {
  const t = String(text || "").toLowerCase();
  const r = { endHp: 6, bestOf: 1, noBow: false, noRod: false, noPearls: false, ...base };
  const h = /(\d+(?:[.,]5)?)\s*(cuori|cuore|hearts?)/.exec(t);
  if (h) r.endHp = Math.max(1, Math.min(18, Math.round(parseFloat(h[1].replace(",", ".")) * 2)));
  if (/(primo|al)\s*k\.?o\.?|first\s*k\.?o|to the death|alla morte|fino alla morte|\bko\b/.test(t)) r.endHp = 0;
  if (/primo colpo|first hit/.test(t)) r.firstHit = true;
  const b = /(?:al meglio di|meglio di|best of|bo)\s*(\d+)/.exec(t);
  if (b) r.bestOf = Math.max(1, Math.min(7, Number(b[1]) | 1));
  const rounds = /(\d+)\s*(round|manche|partite)/.exec(t);
  if (rounds && !b) r.bestOf = Math.max(1, Math.min(7, Number(rounds[1])));
  if (/(senza|niente|no)\s*(arc|bow)/.test(t)) r.noBow = true;
  if (/(senza|niente|no)\s*(cann|rod)/.test(t)) r.noRod = true;
  if (/(senza|niente|no)\s*(perl|pearl)/.test(t)) r.noPearls = true;
  return r;
}

/** Testo delle regole per la chat. */
function rulesLine(r, en = false) {
  const end = r.endHp === 0 ? (en ? "first KO" : "al primo KO") : en ? `until ${r.endHp / 2} hearts` : `fino a ${r.endHp / 2} cuori`;
  const extra = [r.noBow && (en ? "no bow" : "niente arco"), r.noRod && (en ? "no rod" : "niente canna"), r.noPearls && (en ? "no pearls" : "niente perle")].filter(Boolean);
  const bo = r.bestOf > 1 ? (en ? `, best of ${r.bestOf}` : `, al meglio di ${r.bestOf}`) : "";
  return `${end}${bo}${extra.length ? ", " + extra.join(", ") : ""}`;
}

function newPattern() {
  return { n: 0, dist: 0, lateral: 0, strafeFlips: 0, lastSide: 0, swings: 0, crits: 0, wtaps: 0, shield: 0, bow: 0, rod: 0, eat: 0, pearls: 0, flee: 0, t: 0, approach: 0, hitsOnMe: 0 };
}

/**
 * Un campione dell'avversario (≈ 10 al secondo), relativo a lei:
 *  s = {t, d (distanza), lat (velocità di lato, + = sua destra), vr (velocità verso di lei, + = si avvicina), air, falling,
 *       swing (ha colpito adesso), sprintStart (ha appena ripreso lo sprint), blocking, bow, rod, eating, pearl, hp}
 */
function observe(p, s) {
  const dt = p.prevT != null ? Math.max(0, Math.min(0.5, s.t - p.prevT)) : 0;
  p.prevT = s.t;
  p.n++;
  p.t += dt;
  p.dist += s.d;
  p.lateral += Math.abs(s.lat || 0);
  const side = Math.sign(s.lat || 0);
  if (side && p.lastSide && side !== p.lastSide && Math.abs(s.lat) > 0.08) p.strafeFlips++;
  if (side) p.lastSide = side;
  if (s.vr > 0.1) p.approach += dt;
  if (s.swing && s.d < 4.5) {
    p.swings++;
    if (s.air && s.falling) p.crits++;
    if (p.sprintAt != null && s.t - p.sprintAt < 0.6) p.wtaps++;
  }
  if (s.sprintStart) p.sprintAt = s.t;
  if (s.blocking) p.shield += dt;
  if (s.bow) p.bow += dt;
  if (s.rod) p.rod++;
  if (s.eating) p.eat += dt;
  if (s.pearl) p.pearls++;
  if (s.vr < -0.15 && s.hp != null && s.hp <= 8) p.flee += dt;
  if (s.hitMe) p.hitsOnMe++;
  return p;
}

/** Stile dell'avversario in numeri: distanza preferita, quanto fa strafe, quota di critici e w-tap… */
function style(p) {
  const n = Math.max(1, p.n);
  const t = Math.max(0.5, p.t);
  const sw = Math.max(1, p.swings);
  const r2 = (x) => Math.round(x * 100) / 100;
  return {
    secs: r2(p.t), prefDist: r2(p.dist / n), strafe: r2(Math.min(1, p.lateral / n / 0.25)), flips: r2(p.strafeFlips / t * 10),
    cps: r2(p.swings / t), crit: r2(p.crits / sw), wtap: r2(p.wtaps / sw), shield: r2(p.shield / t), bow: r2(p.bow / t),
    rod: p.rod, eat: r2(p.eat / t), pearls: p.pearls, flee: r2(p.flee / t), aggro: r2(p.approach / t), hitsOnMe: p.hitsOnMe,
  };
}

/** Counter: quanto conviene ogni tattica contro questo stile (0..1, prima di quello che ha imparato). */
function counters(st, opts = {}) {
  const b = Object.fromEntries(NAMES.map((k) => [k, 0]));
  if (!st) return b;
  if (st.crit > 0.3) { b.shield += 0.35; b.spacing += 0.2; }
  if (st.prefDist && st.prefDist < 2.6 || st.cps > 4) { b.spacing += 0.3; b.strafe += 0.15; }
  if (st.bow > 0.15) { b.rush += 0.35; b.strafe += 0.15; }
  if (st.shield > 0.15) { b.axe += 0.4; b.rush -= 0.1; }
  if (st.strafe > 0.6) { b.strafe += 0.15; b.spacing += 0.1; }
  if (st.flee > 0.1 || st.aggro < 0.2) b.rush += 0.25;
  if (st.wtap > 0.3) b.spacing += 0.1;
  if (!opts.shield) b.shield = -9;
  if (!opts.bow) b.kite = -9;
  if (!opts.axe) b.axe = -9;
  return b;
}

/**
 * Sceglie la tattica: UCB1 sui risultati passati (vittorie e danno) + counter sullo stile + un po' di
 * esplorazione (ε) per provare cose nuove. `stats`: {tattica: {n, reward}}; rng in 0..1.
 */
function choose(stats, st, opts = {}, rng = Math.random) {
  const c = counters(st, opts);
  const avail = NAMES.filter((k) => c[k] > -5);
  const N = avail.reduce((a, k) => a + ((stats && stats[k] && stats[k].n) || 0), 0);
  const eps = opts.eps == null ? 0.12 : opts.eps;
  if (rng() < eps) return avail[Math.floor(rng() * avail.length) % avail.length];
  let best = null, bv = -Infinity;
  for (const k of avail) {
    const s = (stats && stats[k]) || { n: 0, reward: 0 };
    const mean = s.n ? s.reward / s.n : 0.5;
    const bonus = s.n ? Math.sqrt((2 * Math.log(N + 1)) / s.n) * 0.5 : 0.6;
    const v = mean + bonus + c[k];
    if (v > bv) { bv = v; best = k; }
  }
  return best;
}

/** Premio di un round (0..1): vittoria conta di più, poi la differenza di danno. */
function reward(won, dealt, taken) {
  const diff = (dealt - taken) / Math.max(4, dealt + taken);
  return Math.max(0, Math.min(1, (won ? 0.65 : 0.1) + diff * 0.35));
}

/** Parametri per le mani: tattica + quello che copia dal giocatore (distanza e strafe, se gli funzionano). */
function params(tactic, mimic) {
  const t = { ...(TACTICS[tactic] || TACTICS.spacing), name: tactic };
  if (mimic && mimic.dist && tactic !== "kite") t.dist = Math.max(2, Math.min(3.4, (t.dist + mimic.dist) / 2));
  if (mimic && mimic.strafe != null && tactic !== "rush") t.strafe = Math.max(t.strafe, Math.min(1, mimic.strafe));
  return t;
}

/** Il round è finito? {over, won, why}. hp = vita sua, opp = vita dell'avversario (null se non si vede). */
function roundOver(r, me, opp, secs, d) {
  if (me <= 0) return { over: true, won: false, why: "ko" };
  if (opp != null && opp <= 0) return { over: true, won: true, why: "ko" };
  if (r.endHp > 0 && me <= r.endHp) return { over: true, won: false, why: "hearts" };
  if (r.endHp > 0 && opp != null && opp <= r.endHp) return { over: true, won: true, why: "hearts" };
  if (d != null && d > 40) return { over: true, won: null, why: "left" };
  if (secs > (r.maxSecs || 150)) return { over: true, won: null, why: "time" };
  return { over: false };
}

// ---------- parte 2: il duello vero (Mineflayer) ----------

function attach(ctx) {
  const { SKILLS } = ctx;
  let duel = null; // {who, rules, round, wins, losses}
  let tacticFromApp = null;
  ctx.isDueling = () => !!duel;

  ctx.onOp("duel_tactic", (o) => { tacticFromApp = { name: o.tactic, mimic: o.mimic || null, at: Date.now() }; });

  const bot = () => ctx.bot();
  const items = () => bot().inventory.items();
  const has = (n) => items().some((i) => i.name === n);

  async function equipBest(t) {
    const b = bot();
    const order = t.axe ? ["netherite_axe", "diamond_axe", "iron_axe", "stone_axe", "wooden_axe"] : [];
    const swords = ["netherite_sword", "diamond_sword", "iron_sword", "stone_sword", "wooden_sword", "netherite_axe", "diamond_axe", "iron_axe", "stone_axe"];
    const pick = [...order, ...swords].find(has);
    if (pick) { try { await b.equip(items().find((i) => i.name === pick), "hand"); } catch {} }
    if (t.shield && has("shield") && !(b.inventory.slots[45] && b.inventory.slots[45].name === "shield")) {
      try { await b.equip(items().find((i) => i.name === "shield"), "off-hand"); } catch {}
    }
  }

  // indici dei metadati dei giocatori (da minecraft-data: cambiano tra le versioni)
  const metaIdx = (name, def) => { const k = ctx.mc().entitiesByName.player && ctx.mc().entitiesByName.player.metadataKeys; const i = k ? k.indexOf(name) : -1; return i >= 0 ? i : def; };
  function oppHp(e) {
    const v = e && e.metadata ? e.metadata[metaIdx("health", 9)] : null;
    return typeof v === "number" && v >= 0 && v <= 1024 ? v : null;
  }

  async function waitTactic(st, stats) {
    tacticFromApp = null;
    ctx.ev("duel_need_tactic", { who: duel.who, round: duel.round, style: st });
    const t0 = Date.now();
    while (Date.now() - t0 < 1800 && !tacticFromApp) await ctx.sleep(100);
    if (tacticFromApp && TACTICS[tacticFromApp.name]) return params(tacticFromApp.name, tacticFromApp.mimic);
    return params(choose(stats, st, { shield: has("shield"), bow: has("bow") && !duel.rules.noBow && has("arrow"), axe: items().some((i) => /_axe$/.test(i.name)) }), null);
  }

  /** Un round: restituisce {won, dealt, taken, style}. */
  async function round(e0, t, c) {
    let e = e0;
    const b = bot();
    const p = newPattern();
    const r = duel.rules;
    const hp0 = b.health;
    let dealt = 0;
    let lastHit = 0, lastSwingSeen = 0, strafeDir = 1, strafeUntil = 0, prevSprint = false, prevPos = e.position.clone();
    let oppPrev = oppHp(e);
    // il colpo si registra nel momento in cui arriva (a terra o in caduta: critico)
    let swingNow = null, lastY = e.position.y, myHp = b.health;
    const onSwing = (x) => {
      if (x !== e && !(x && x.username === duel.who)) return;
      lastSwingSeen = Date.now();
      swingNow = { air: !x.onGround, falling: x.position.y < lastY - 0.02 };
    };
    b.on("entitySwingArm", onSwing);
    const t0 = Date.now();
    let res = { over: false };
    try {
      await equipBest(t);
      while (!res.over) {
        ctx.checkCancel(c);
        if (!e.isValid) {
          // dopo un respawn le entità si rifanno: lo ricerca per nome
          e = ctx.playerEntity(duel.who);
          if (!e) { res = { over: true, won: null, why: "left" }; break; }
          prevPos = e.position.clone();
        }
        const now = Date.now();
        const me = b.entity.position;
        const d = me.distanceTo(e.position);
        // dati dell'avversario
        const vel = e.position.minus(prevPos).scaled(10);
        prevPos = e.position.clone();
        const to = me.minus(e.position);
        const len = Math.hypot(to.x, to.z) || 1;
        const vr = (vel.x * to.x + vel.z * to.z) / len;
        const lat = (vel.x * -to.z + vel.z * to.x) / len;
        const flags = e.metadata && typeof e.metadata[0] === "number" ? e.metadata[0] : 0;
        const sprint = !!(flags & 0x08);
        const lf = e.metadata ? e.metadata[metaIdx("living_entity_flags", 8)] : 0;
        const using = typeof lf === "number" ? lf & 1 : 0;
        const held = e.equipment && e.equipment[0] ? e.equipment[0].name : "";
        const off = e.equipment && e.equipment[1] ? e.equipment[1].name : "";
        const hpo = oppHp(e);
        if (hpo != null && oppPrev != null && hpo < oppPrev) dealt += oppPrev - hpo;
        oppPrev = hpo;
        const sw = swingNow;
        swingNow = null;
        const hitMe = b.health < myHp - 0.1 && d < 5;
        myHp = b.health;
        lastY = e.position.y;
        observe(p, {
          t: (now - t0) / 1000, d, lat, vr, air: sw ? sw.air : !e.onGround, falling: sw ? sw.falling : vel.y < -0.5, swing: !!sw, sprintStart: sprint && !prevSprint, hitMe,
          blocking: !!using && (held === "shield" || off === "shield"), bow: !!using && /bow/.test(held), rod: held === "fishing_rod" && now - lastSwingSeen < 120,
          eating: !!using && ctx.isFood(held), pearl: false, hp: hpo,
        });
        prevSprint = sprint;
        // movimento: tiene la distanza della tattica
        await b.lookAt(e.position.offset(0, 1.5, 0), true);
        const far = d > t.dist + 0.4, near = d < t.dist - 0.6;
        b.setControlState("forward", far);
        b.setControlState("sprint", far);
        b.setControlState("back", near && !far);
        if (t.strafe > 0 && now > strafeUntil) { strafeDir = Math.random() < 0.5 ? 1 : -1; strafeUntil = now + 500 + Math.random() * 900 * (1.2 - t.strafe); }
        b.setControlState("left", t.strafe > 0 && strafeDir > 0 && d < 5);
        b.setControlState("right", t.strafe > 0 && strafeDir < 0 && d < 5);
        // scudo: alzato quando l'altro sta per colpire e lei non attacca
        const cd = now - lastHit;
        if (t.shield && (b.inventory.slots[45] || {}).name === "shield") {
          const threat = d < 3.6 && now - lastSwingSeen < 400 && cd < 450;
          if (threat) b.activateItem(true); else b.deactivateItem();
        }
        // arco da lontano
        if (t.bow && has("bow") && has("arrow") && d > 5) {
          await b.equip(items().find((i) => i.name === "bow"), "hand");
          b.activateItem();
          await ctx.sleep(900);
          await b.lookAt(e.position.offset(0, 1.6 + d * 0.02, 0), true);
          b.deactivateItem();
          await equipBest(t);
          continue;
        }
        // colpo: quando la spada è carica (~0,6 s) e a portata
        if (d < 3.0 && cd > 600) {
          if (t.crit && b.entity.onGround) {
            b.setControlState("jump", true);
            await ctx.sleep(260); // in caduta: critico
            b.setControlState("jump", false);
          }
          if (b.entity.position.distanceTo(e.position) < 3.1) {
            b.attack(e);
            lastHit = Date.now();
            if (hpo == null) dealt += t.crit ? 6 : 4; // stima se la vita non si vede
            if (t.wtap) { b.setControlState("sprint", false); b.setControlState("forward", false); await ctx.sleep(90); }
          }
        }
        if (b.food < 8 && d > 6) await ctx.eat(true).catch(() => {});
        res = roundOver(r, b.health, hpo, (now - t0) / 1000, d);
        if (!res.over && r.firstHit && (dealt > 0 || b.health < hp0)) res = { over: true, won: dealt > 0 && b.health >= hp0 - 0.1, why: "first_hit" };
        await ctx.sleep(80);
      }
    } finally {
      b.removeListener("entitySwingArm", onSwing);
      b.clearControlStates();
      try { b.deactivateItem(); } catch {}
    }
    // nessuna vita visibile e nessun KO: vince chi ha preso meno colpi (stima)
    let won = res.won;
    const taken = Math.max(0, hp0 - b.health);
    if (won == null && res.why === "time") won = dealt > taken;
    return { won, dealt: Math.round(dealt), taken: Math.round(taken), why: res.why, style: style(p) };
  }

  /** "duel <giocatore>": regole, conto alla rovescia, round, risultato. */
  SKILLS.duel = async (o, c) => {
    let e = ctx.playerEntity(o.who);
    if (!e) throw new Error(ctx.en() ? "I can't see you" : "non ti vedo");
    // regole: dalla frase; se la frase non ne dice, chiede (l'app risponde con quelle che ricorda)
    let rulesText = String(o.text || "");
    if (!/cuor|heart|k\.?o|primo colpo|first hit|meglio di|best of|round|senza|no bow|no rod|no pearl/i.test(rulesText) && ctx.cfg.ask !== false) {
      const ans = await ctx.ask("duel_rules", { who: e.username }, 20000, null);
      if (typeof ans === "string") rulesText += " " + ans;
    }
    const rules = parseRules(rulesText, o.rules || {});
    duel = { who: e.username, rules, round: 0, wins: 0, losses: 0 };
    const en = ctx.en();
    try {
      await ctx.chat((en ? "duel! rules: " : "duello! regole: ") + rulesLine(rules, en) + (en ? ". no hits before GO" : ". niente colpi prima del VIA"));
      const need = Math.ceil(rules.bestOf / 2);
      let st = o.style || null;
      while (duel.wins < need && duel.losses < need && duel.round < rules.bestOf) {
        ctx.checkCancel(c);
        duel.round++;
        e = ctx.playerEntity(duel.who) || e;
        // distanza di partenza e conto alla rovescia
        const away = bot().entity.position.minus(e.position);
        const back = e.position.plus(away.normalize().scaled(6));
        await ctx.goNear(back, 1, c).catch(() => {});
        await bot().lookAt(e.position.offset(0, 1.6, 0));
        const t = await waitTactic(st, o.stats || {});
        for (const n of ["3", "2", "1"]) { bot().chat(n); await ctx.sleep(1000); ctx.checkCancel(c); }
        bot().chat(en ? "GO!" : "VIA!");
        ctx.ev("duel_start", { who: duel.who, round: duel.round, tactic: t.name });
        const r = await round(e, t, c);
        if (r.won === true) duel.wins++;
        else if (r.won === false) duel.losses++;
        st = r.style;
        ctx.ev("duel_round", { who: duel.who, round: duel.round, tactic: t.name, won: r.won, dealt: r.dealt, taken: r.taken, why: r.why, style: r.style, rules });
        bot().chat(r.won === true ? (en ? "got you! gg" : "presa! gg") : r.won === false ? (en ? "ok you win this one, gg" : "ok questa l'hai vinta tu, gg") : en ? "round over" : "round finito");
        if (r.why === "left") break;
        if (duel.round < rules.bestOf && duel.wins < need && duel.losses < need) { await ctx.sleep(4000); await ctx.eat(true).catch(() => {}); }
      }
      const res = { wins: duel.wins, losses: duel.losses, rounds: duel.round };
      ctx.ev("duel_end", { who: duel.who, ...res });
      return res;
    } finally {
      duel = null;
    }
  };
}

module.exports = { TACTICS, NAMES, parseRules, rulesLine, newPattern, observe, style, counters, choose, reward, params, roundOver, attach };
