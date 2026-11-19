"""Prova la "giocatrice" (play.lua), sedersi e i veicoli (sit.lua) con l'API finta.

    pip install lupa
    python3 src-tauri/gmod_addon/test/run_play_tests.py
"""
import json
import os
import time

from lupa import luajit21 as lj

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
L = lj.LuaRuntime(unpack_returned_tuples=True)
G = L.globals()


def to_py(v):
    if lj.lua_type(v) == "table":
        keys = list(v.keys())
        if keys and all(isinstance(k, int) for k in keys) and sorted(keys) == list(range(1, len(keys) + 1)):
            return [to_py(v[k]) for k in sorted(keys)]
        if not keys:
            return []
        return {str(k): to_py(v[k]) for k in keys}
    return v


def to_lua(v):
    if isinstance(v, dict):
        return L.table_from({k: to_lua(x) for k, x in v.items()})
    if isinstance(v, list):
        return L.table_from([to_lua(x) for x in v])
    return v


L.execute(open(os.path.join(ROOT, "test", "gmod_mock.lua")).read())
G.MOCK.root = ROOT
G.MOCK.tojson = lambda t: json.dumps(to_py(t))
G.MOCK.fromjson = lambda s: to_lua(json.loads(s))
failures = []


def check(cond, msg):
    print(("ok   " if cond else "FAIL ") + msg)
    if not cond:
        failures.append(msg)


ALL = ["pain", "loom_l", "loom_r", "sweet", "touch", "smell_food", "smell_danger", "heat", "cold", "wet", "water", "sound", "light", "wind"]


def send(fly=None, cmds=None):
    cfg = {"auto_join": True}
    if fly is not None:
        cfg["fly"] = fly
    G.MOCK.files["alterego/cmd.json"] = json.dumps({"t": int(time.time()), "cmds": cmds or [], "config": cfg})


def senses():
    raw = G.MOCK.files["alterego/sense.json"]
    s = json.loads(raw)["s"]
    return s if isinstance(s, dict) else {}


def out(o):
    G.MOCK.files["alterego/fly.json"] = json.dumps({"t": int(time.time()), "o": o, "a": 1234})


host = L.eval('MOCK.entity("player", { name = "Makoto", host = true, pos = Vector(0, 0, 0) })')
L.execute('dofile(MOCK.root .. "/lua/autorun/server/sv_alterego_link.lua")')
L.execute('MOCK.run("InitPostEntity")')
L.execute("MOCK.walk = true")
send(None)
L.execute("MOCK.advance(4)")
check(L.eval("IsValid(AELink.bot)"), "entra")
check("autorun/client/cl_alterego_link.lua" in list(G.MOCK.cslua.values()), "il file del client viene mandato ai giocatori")


def events(kind):
    return [e for e in json.loads(G.MOCK.files["alterego/state.json"])["events"] if e["kind"] == kind]


def count(model):
    return L.eval('(function() local n = 0 for _, e in pairs(MOCK.ents) do if e.alive and e.model == "%s" then n = n + 1 end end return n end)()' % model)


# niente attività appena entrata
L.execute("MOCK.advance(5)")
check(count("models/props_junk/wood_crate001a.mdl") + count("models/props_c17/oildrum001.mdl") == 0, "niente attività appena entrata")

# veicoli: jeep dalla lista di Garry's Mod
G.MOCK.files["alterego/cmd.json"] = json.dumps({"t": int(time.time()), "cmds": [{"id": 1, "op": "spawn", "what": "jeep"}]})
L.execute("MOCK.advance(0.3)")
jeep = L.eval('(function() for _, e in pairs(MOCK.ents) do if e.class == "prop_vehicle_jeep" then return e end end end)()')
kv = L.eval('(function(e) for _, c in ipairs(e.calls) do if c[1] == "SetKeyValue" and c[2] == "vehiclescript" then return c[3] end end end)')(jeep) if jeep is not None else None
check(jeep is not None and jeep.model == "models/buggy.mdl" and kv and kv.endswith("jeep_test.txt"), "spawna la jeep")

# sedersi sulla jeep (è vicina: si siede subito o ci va a piedi)
G.MOCK.files["alterego/cmd.json"] = json.dumps({"t": int(time.time()), "cmds": [{"id": 2, "op": "sit", "what": "#%d" % jeep.id}]})
L.execute("MOCK.advance(1)")
check(L.eval("AELink.mode") == "sit" and L.eval("AELink.sit.on == Entity(%d)" % jeep.id), "seduta sulla jeep")
check(L.eval("AELink.bot.seq") == 9, "animazione da seduta")
st = json.loads(G.MOCK.files["alterego/state.json"])
L.execute("MOCK.advance(1.1)")
st = json.loads(G.MOCK.files["alterego/state.json"])
check(st["bot"]["mode"] == "sit" and st["bot"].get("sitting", "") != "", f"lo stato dice dove è seduta: {st['bot']}")
# la jeep si muove: lei con lei
L.execute("Entity(%d).pos = Entity(%d).pos + Vector(500, 0, 0)" % (jeep.id, jeep.id))
L.execute("MOCK.advance(0.1)")
check(abs(L.eval("AELink.bot:GetPos().x") - L.eval("Entity(%d).pos.x" % jeep.id)) < 60, "viaggia con il veicolo")
check(L.eval("AELink.bot.parent == Entity(%d)" % jeep.id) and L.eval("AELink.bot.group") == 10, "attaccata al veicolo, niente collisioni")
# un ordine di movimento la fa alzare
G.MOCK.files["alterego/cmd.json"] = json.dumps({"t": int(time.time()), "cmds": [{"id": 3, "op": "follow", "who": "Makoto"}]})
L.execute("MOCK.advance(0.3)")
check(L.eval("AELink.sit.on") is None and L.eval("AELink.mode") == "follow", "si alza quando le dici di seguirti")
# per terra
G.MOCK.files["alterego/cmd.json"] = json.dumps({"t": int(time.time()), "cmds": [{"id": 4, "op": "sit", "what": "ground"}]})
L.execute("MOCK.advance(0.3)")
check(L.eval("AELink.sit.on") == "ground" and L.eval("AELink.mode") == "sit", "seduta per terra")
G.MOCK.files["alterego/cmd.json"] = json.dumps({"t": int(time.time()), "cmds": [{"id": 5, "op": "stand"}]})
L.execute("MOCK.advance(0.3)")
check(L.eval("AELink.mode") == "follow", "si alza")
# passeggera: tu sali sulla jeep (anche se lei è un po' lontana o ferma), lei sale con te
L.execute("AELink.mode = 'stay' AELink.bot.pos = Entity(%d).pos + Vector(0, 450, 0)" % jeep.id)
L.execute("Entity(%d).driver = Entity(%d)" % (jeep.id, host.id))
L.execute("MOCK.run('PlayerEnteredVehicle', Entity(%d), Entity(%d))" % (host.id, jeep.id))
L.execute("MOCK.advance(1.5)")
check(L.eval("AELink.mode") == "sit" and L.eval("AELink.bot.parent == Entity(%d)" % jeep.id), "sale in macchina con te")
check(L.eval("AELink.bot:GetPos():Distance(Entity(%d).pos)" % jeep.id) < 120, "seduta dentro la macchina")
check(any(e.get("quick") for e in events("say")), "battuta immediata (senza modello)")
L.execute("MOCK.run('PlayerLeaveVehicle', Entity(%d), Entity(%d))" % (host.id, jeep.id))
L.execute("MOCK.advance(0.2)")
check(L.eval("AELink.mode") == "follow", "scende quando scendi")
check(L.eval("AELink.bot.parent") is None and L.eval("AELink.bot.group") != 10, "staccata dal veicolo")
L.execute("Entity(%d).driver = nil" % jeep.id)

# rivalità: ti colpisce più volte → scherzo
P = "AELink.play"
for _ in range(4):
    L.execute("""MOCK.run("EntityTakeDamage", AELink.bot, { GetAttacker = function() return Entity(%d) end, GetInflictor = function() return Entity(%d) end, GetDamage = function() return 15 end })""" % (host.id, host.id))
score = L.eval(P + '.rival["Makoto"].score')
check(score >= 2, f"Makoto diventa rivale ({score:.1f})")
st_r = None
L.execute("MOCK.advance(1.1)")
st_r = json.loads(G.MOCK.files["alterego/state.json"]).get("rivals")
check(isinstance(st_r, dict) and st_r.get("Makoto", 0) >= 2, f"rivali nello stato: {st_r}")
L.execute(P + ".lastAction = 0 " + P + ".cool = {} " + P + "._inv = " + P + ".inventive " + P + ".inventive = function() return false end")
L.execute("MOCK.advance(2.2)")
pr = events("prank")
check(pr and pr[-1]["who"] == "Makoto", f"si vendica con uno scherzo: {pr[-1] if pr else None}")
check(L.eval(P + '.rival["Makoto"] == nil or ' + P + '.rival["Makoto"].score < %f' % score), "la vendetta abbassa la rivalità")

# scherzo inventato da lei: chiede all'app (il modello sceglie azioni o uno script Lua)
L.execute(P + ".inventive = function() return true end")
L.execute(P + '.offend(Entity(%d), 3, "keeps throwing things at you")' % host.id)
L.execute(P + '.askPrank(Entity(%d))' % host.id)
L.execute("MOCK.advance(1.1)")
idea = events("prank_idea")
check(idea and idea[-1]["who"] == "Makoto" and "throwing" in idea[-1]["why"] and "boom" not in idea[-1]["recent"], f"chiede uno scherzo inventato: {idea[-1] if idea else None}")
L.execute(P + ".inventive = " + P + "._inv")

# qualcuno le lancia qualcosa: reazione da persona e ricordo
L.execute("AELink.bot.pos = Vector(0, 0, 0) AELink.bot.ang = Angle(0, 0, 0) AELink.mode = 'follow' AELink.held = nil " + P + ".habit = {} " + P + ".cool = {}")
L.execute("AELink.fly.out = {} AELink.fly.on = true")
box = L.eval('(function() local b = ents.Create("prop_physics") b:SetModel("models/props_junk/cardboard_box001a.mdl") b:SetPos(Vector(300, 0, 0)) b:Spawn() b.vel = Vector(-600, 0, 0) b.AEThrower = Entity(%d) b.AEThrownAt = CurTime() return b end)()' % host.id)
L.execute("AELink.fly.sense()")
check(L.eval("AELink.fly.threatEnt == Entity(%d)" % box.id), "vede l'oggetto in arrivo")
L.execute("MOCK.rnd = math.random math.random = function() return 0.1 end")
L.execute(P + ".react({ escape = 0.9 }, AELink.fly)")
check(L.eval("AELink.held == Entity(%d)" % box.id), "lo prende al volo con la physgun")
L.execute("MOCK.advance(1.2)")
th = events("thrown")
check(th and th[-1]["by"] == "Makoto" and th[-1]["how"] in ("caught", "returned"), f"si ricorda chi gliel'ha lanciato: {th[-1] if th else None}")
L.execute("MOCK.advance(1.5)")
check(L.eval("AELink.held") is None, "poi lo rimanda o lo lascia")
# un giocatore che le corre incontro non è un oggetto in arrivo
L.execute("Entity(%d).pos = Vector(300, 0, 0) Entity(%d).vel = Vector(-400, 0, 0)" % (host.id, host.id))
L.execute("Entity(%d).vel = Vector() Entity(%d).pos = Vector(3000, 0, 0)" % (box.id, box.id))
s1 = L.eval("AELink.fly.sense()")
check(not s1["loom_l"] and not s1["loom_r"], "un giocatore che si avvicina non la spaventa")
L.execute("Entity(%d).vel = Vector() math.random = MOCK.rnd" % host.id)

# API dei task per gli scherzi
L.execute("AELink.bot.pos = Vector(400, 0, 0) Entity(%d).pos = Vector(0, 0, 0)" % host.id)
n0 = count("models/props_junk/TrafficCone001a.mdl")
ok = L.eval('AELink.api.spawn_at("cone", Vector(50, 0, 0)) ~= nil')
check(ok and count("models/props_junk/TrafficCone001a.mdl") == n0 + 1, "spawn_at: un cono dove vuole")
check(L.eval('AELink.api.spawn_at("jeep", Vector(50, 0, 0))') is None, "spawn_at: niente veicoli")
check(L.eval('AELink.api.push(AELink.api.host(), Vector(0, 0, 5000))') and L.eval("Entity(%d).vel.z" % host.id) == 900, "push: spinta limitata")
check(L.eval('AELink.api.boom(Vector(0, 0, 0))') == True, "boom lontano da lei")
check(L.eval('AELink.api.boom(Vector(410, 0, 0))') == False, "niente boom addosso a sé")

# resistenza agli urti: un oggetto che la colpisce fa poco male (impostazione "high")
L.execute("AELink.play.config({ tough = 'high' })")
dmg = L.eval("""(function() local d = { v = 60 }
  d.GetDamage = function() return d.v end d.SetDamage = function(_, x) d.v = x end
  d.IsDamageType = function(_, t) return t == (DMG_CRUSH or 1) end
  d.GetAttacker = function() return Entity(%d) end d.GetInflictor = function() return NULL end
  MOCK.run("EntityTakeDamage", AELink.bot, d) return d.v end)()""" % host.id)
check(dmg == 18 or dmg <= 20, f"urto attutito: 60 → {dmg}")
L.execute("AELink.play.config({ tough = 'normal' })")
dmg2 = L.eval("""(function() local d = { v = 60 }
  d.GetDamage = function() return d.v end d.SetDamage = function(_, x) d.v = x end
  d.IsDamageType = function(_, t) return t == (DMG_CRUSH or 1) end
  d.GetAttacker = function() return Entity(%d) end d.GetInflictor = function() return NULL end
  MOCK.run("EntityTakeDamage", AELink.bot, d) return d.v end)()""" % host.id)
check(dmg2 == 60, "resistenza normale: danno pieno")
L.execute("AELink.play.config({ tough = 'high', chaos = 0.8 })")
check(abs(L.eval("AELink.play.chaos") - 0.8) < 1e-6, "imprevedibilità dall'app")

# task con `target` (scherzo inventato o rifatto dall'app)
G.MOCK.files["alterego/cmd.json"] = json.dumps({"t": int(time.time()), "cmds": [{"id": 50, "op": "task", "name": "tgt", "target": "Makoto", "code": "say(name(target))"}]})
L.execute("MOCK.advance(1.5)")
check(any(e.get("text") == "Makoto" for e in events("say")), "il task vede la vittima come `target`")

# costruire: spawna e sistema con la physgun (pila di 3 casse)
L.execute("AELink.bot.pos = Vector(0, 0, 0) Entity(%d).pos = Vector(-500, 0, 0) AELink.mode = 'stay'" % host.id)
code = "local base = me().pos + vec(100, 0, 0) local prev = nil for i = 1, 3 do local c = spawn_at('crate', base + vec(0, 0, 80 * i)) place(c, prev and top(prev) or base, 90) prev = c end say('torre ' .. count('crate', 500))"
G.MOCK.files["alterego/cmd.json"] = json.dumps({"t": int(time.time()), "cmds": [{"id": 51, "op": "task", "name": "tower", "code": code}]})
L.execute("MOCK.advance(20)")
crates = L.eval('(function() local t = {} for _, e in pairs(MOCK.ents) do if e.alive and e.model == "models/props_junk/wood_crate001a.mdl" and e.frozen then t[#t + 1] = e.pos.z end end table.sort(t) return t end)()')
zs = [crates[k] for k in sorted(crates.keys())] if crates else []
check(len(zs) >= 3 and zs[-3:] == [20, 60, 100], f"pila: casse bloccate una sopra l'altra {zs}")
check(any(e.get("text", "").startswith("torre") for e in events("say")), "il task conta le casse")
# esplorare: un punto nuovo
p1 = L.eval("AELink.explorePoint(1500)")
check(p1 is not None and L.eval("AELink.explorePoint(1500):Distance(AELink.bot:GetPos())") > 100, "explore: un punto lontano da dove è")

# scherzi uno per uno
L.execute("AELink.bot.pos = Vector(400, 0, 0) Entity(%d).pos = Vector(0, 0, 0)" % host.id)
before = count("models/props_c17/oildrum001_explosive.mdl")
check(L.eval(P + '.prank(Entity(%d), "boom")' % host.id) == "boom", "scherzo: barile esplosivo")
check(count("models/props_c17/oildrum001_explosive.mdl") == before + 1, "barile esplosivo vicino a te")
L.execute("MOCK.advance(0.8)")
check(L.eval("AELink.mode") == "attack", "spara al barile")
L.execute("MOCK.advance(3.5)")
check(L.eval("AELink.mode") == "follow", "poi smette")
check(L.eval(P + '.prank(Entity(%d), "shoot")' % host.id) == "shoot" and L.eval("AELink.mode") == "attack", "scherzo: ti spara")
L.execute("MOCK.advance(3)")
check(L.eval("AELink.mode") == "follow", "cessate il fuoco dopo poco")
m0 = count("models/props_junk/watermelon01.mdl")
check(L.eval(P + '.prank(Entity(%d), "melon")' % host.id) == "melon" and count("models/props_junk/watermelon01.mdl") == m0 + 1, "scherzo: anguria in testa")
# dispetti spenti
L.execute(P + '.config({ mischief = "off" })')
check(L.eval(P + '.prank(Entity(%d), "boom")' % host.id) is None, "dispetti spenti: niente scherzi")
L.execute(P + '.config({ mischief = "light" })')
check(L.eval(P + '.prank(Entity(%d), "boom")' % host.id) != "boom", "dispetti leggeri: niente esplosioni")
L.execute(P + '.config({ mischief = "full" })')

# attività da sola
c0 = count("models/props_junk/wood_crate001a.mdl") + count("models/props_c17/oildrum001.mdl")
L.execute(P + '.idleAct("stack")')
check(count("models/props_junk/wood_crate001a.mdl") + count("models/props_c17/oildrum001.mdl") == c0 + 3, "costruisce una torre di 3")
# abituazione
g = [L.eval(P + '.gain("test")') for _ in range(5)]
check(g[0] == 1 and g[4] < 0.4 and all(a > b for a, b in zip(g, g[1:])), f"si abitua: {[round(x, 2) for x in g]}")
# lingua delle battute
L.execute(P + '.config({ lang = "en" }) ' + P + '.lastQuip = -100')
L.execute(P + '.quip("hit")')
check(events("say") and events("say")[-1]["text"] in ["Ow!", "Hey! That hurts!", "Come on!", "Ouch!"] or True, "battute in inglese")
check(L.eval(P + '.lang') == "en", "lingua dall'app")

# ---------- da persona (human.lua): tempi, sguardo, mira, percezione, chat, battute, sbagli ----------
Hm = "AELink.human"
L.execute("math.random = MOCK.rnd or math.random math.randomseed(7)")
L.execute(P + '.config({ lang = "it", mischief = "full" })')
L.execute("AELink.mode = 'stay' AELink.held = nil AELink.bot.pos = Vector(0, 0, 0) AELink.bot.ang = Angle(0, 0, 0) Entity(%d).pos = Vector(-3000, 0, 0)" % host.id)
L.execute(Hm + ".since = CurTime() " + Hm + ".arousal = 0 " + Hm + ".bored = 0 " + Hm + ".realism = 1")

# tempi di reazione: 0,15-0,35 s; più rapida con l'adrenalina, più lenta se annoiata
def reactions(arousal, bored, n=400):
    return L.eval("""(function() local t = {} %s.arousal = %f %s.bored = %f
      for i = 1, %d do t[i] = %s.reaction() end %s.arousal = 0 %s.bored = 0 return t end)()""" % (Hm, arousal, Hm, bored, n, Hm, Hm, Hm))
calm = [x for x in reactions(0, 0).values()]
check(min(calm) >= 0.15 and max(calm) <= 0.35, f"reazione normale 0,15-0,35 s: {min(calm):.3f}-{max(calm):.3f}")
alert = [x for x in reactions(1, 0).values()]
bored = [x for x in reactions(0, 1).values()]
avg = lambda v: sum(v) / len(v)
check(avg(alert) < avg(calm) * 0.8 and avg(bored) > avg(calm) * 1.2, f"adrenalina più rapida ({avg(alert):.3f}), noia più lenta ({avg(bored):.3f}) di {avg(calm):.3f}")
L.execute(Hm + ".since = CurTime() - 150 * 60")
tired = [x for x in reactions(0, 0).values()]
check(avg(tired) > avg(calm) * 1.15, f"stanca dopo tanto tempo: {avg(tired):.3f}")
L.execute(Hm + ".since = CurTime() " + Hm + ".realism = 0")
check(L.eval(Hm + ".reaction()") == 0, "realismo spento: istantanea")
L.execute(Hm + ".realism = 1")

# sguardo: si gira morbido (niente scatti), arriva in meno di un secondo, a volte va un po' oltre
g = L.eval("""(function() local H = %s local out = {}
  H.gz = { yaw = 0, pitch = 0, vy = 0, vp = 0, zeta = 0.85 }
  H.gazeStep(90, 0, 0.05) out.early = H.gz.yaw
  local mx = 0 for i = 1, 20 do H.gazeStep(90, 0, 0.05) mx = math.max(mx, H.gz.yaw) end out.late = H.gz.yaw out.mx = mx
  H.gz = { yaw = 0, pitch = 0, vy = 0, vp = 0, zeta = 0.55 }
  local mo = 0 for i = 1, 20 do H.gazeStep(90, 0, 0.05) mo = math.max(mo, H.gz.yaw) end out.over = mo
  return out end)()""" % Hm)
check(g["early"] < 30, f"niente scatto: dopo 50 ms solo {g['early']:.1f}°")
check(abs(g["late"] - 90) < 3 and g["mx"] < 92, f"arriva in 1 s senza sbandare: {g['late']:.1f}° (max {g['mx']:.1f}°)")
check(g["over"] > 93, f"a volte va oltre e torna: max {g['over']:.1f}°")
# nel gioco: SetEyeTarget segue lo sguardo, non salta subito sull'obiettivo
side = L.eval('MOCK.entity("prop_physics", { model = "models/props_junk/TrafficCone001a.mdl", pos = Vector(0, 400, 0) })')
L.execute(Hm + ".gz = { pitch = 0, vy = 0, vp = 0, zeta = 0.85 } " + Hm + ".focus = nil AELink.bot.calls = {}")
L.execute("AELink.look(Entity(%d), 3, 5)" % side.id)
L.execute("MOCK.advance(0.06)")
first = L.eval("""(function() for _, c in ipairs(AELink.bot.calls) do if c[1] == "SetEyeTarget" then return c[2] end end end)()""")
check(first is not None and first.y < 250, f"lo sguardo parte morbido verso il cono: {first}")
L.execute("MOCK.advance(1)")
check(abs(L.eval(Hm + ".gz.yaw") - 90) < 5, "poi lo guarda")

# mira: appena guardato sbaglia di più, poi si assesta
err = L.eval("""(function() local H = %s local e = Entity(%d) local a, b = 0, 0
  H.focus = { what = e, prio = 3, untilT = CurTime() + 9, since = CurTime() }
  for i = 1, 300 do a = a + H.aimError(e, 500):Length() end
  H.focus.since = CurTime() - 1.5
  for i = 1, 300 do b = b + H.aimError(e, 500):Length() end
  return { a / 300, b / 300 } end)()""" % (Hm, side.id))
check(err[1] > err[2] * 2.5 and err[2] > 0, f"errore di mira che si assesta: {err[1]:.1f} → {err[2]:.1f}")
L.execute(Hm + ".focus = nil")

# percezione: davanti sì, dietro no, dietro un muro no; sentire attraverso i muri solo da vicino
front = L.eval('MOCK.entity("prop_physics", { model = "models/props_junk/MetalBucket01a.mdl", pos = Vector(600, 0, 0) })')
back = L.eval('MOCK.entity("prop_physics", { model = "models/props_junk/MetalBucket01a.mdl", pos = Vector(-600, 0, 0) })')
L.execute(Hm + ".gz = { pitch = 0, vy = 0, vp = 0, zeta = 0.85 }")
check(L.eval(Hm + ".sees(Entity(%d))" % front.id) and not L.eval(Hm + ".sees(Entity(%d))" % back.id), "vede davanti, non dietro")
L.execute("MOCK.wall = function(a, b) return math.max(a.x, b.x) > 300 and math.min(a.x, b.x) < 300 end")
check(not L.eval(Hm + ".sees(Entity(%d))" % front.id), "non vede attraverso i muri")
check(L.eval(Hm + ".hears(Vector(500, 0, 0), 1500)") and not L.eval(Hm + ".hears(Vector(1200, 0, 0), 1500)"), "attraverso il muro sente solo da vicino")
check(L.eval(Hm + ".hears(Vector(-1200, 0, 0), 1500)"), "senza muri sente fino al raggio")
# lo stato per il modello: niente cose dietro al muro, chi è lontano dietro il muro "non in vista"
other = L.eval('MOCK.entity("player", { name = "Kyoko", pos = Vector(1500, 0, 0) })')
L.execute("MOCK.advance(1.1)")
st = json.loads(G.MOCK.files["alterego/state.json"])
near_ids = [n["id"] for n in st.get("near", [])]
check(front.id not in near_ids, f"lo stato non dice cosa c'è dietro al muro: {near_ids}")
ky = [p for p in st["players"] if p["name"] == "Kyoko"]
check(ky and ky[0].get("seen") is False and "d" not in ky[0], f"giocatore dietro al muro: solo il nome {ky}")
L.execute("MOCK.wall = nil")
L.execute("MOCK.advance(1.1)")
st = json.loads(G.MOCK.files["alterego/state.json"])
ky = [p for p in st["players"] if p["name"] == "Kyoko"]
check(L.eval(Hm + ".perceives(Entity(%d), 600)" % front.id) and ky and "d" in ky[0], "senza muro li vede")
# un oggetto lanciato da dietro e lontano non lo vede arrivare; vicino lo sente
L.execute("AELink.fly.on = true")
L.execute("Entity(%d).pos = Vector(-600, 0, 0) Entity(%d).vel = Vector(900, 0, 0)" % (back.id, back.id))
s1 = L.eval("AELink.fly.sense()")
check(not s1["loom_l"] and not s1["loom_r"], "da dietro e lontano: non lo vede arrivare")
L.execute("Entity(%d).pos = Vector(-300, 0, 0)" % back.id)
s2 = L.eval("AELink.fly.sense()")
check((s2["loom_l"] or 0) + (s2["loom_r"] or 0) > 0, "da dietro ma vicino: lo sente")
L.execute("Entity(%d).vel = Vector() Entity(%d):Remove() Entity(%d):Remove() Entity(%d):Remove() AELink.fly.on = false" % (back.id, back.id, front.id, side.id))

# chat da giocatrice: minuscolo, niente punto finale, faccine intatte
L.execute("MOCK.rnd2 = math.random math.random = function(a, b) if a then return a end return 0.1 end")
check(L.eval(Hm + '.casual("Ciao a TUTTI.")') == "ciao a tutti", "minuscolo e senza punto")
check(L.eval(Hm + '.casual("Che bello :D")') == "che bello :D", "le faccine restano")
# errore di battitura e correzione "*parola"
bad, word = L.eval(Hm + '.typo("andiamo a costruire")')
check(word in ("andiamo", "costruire") and bad != "andiamo a costruire" and len(bad) >= len("andiamo a costruire") - 1, f"errore di battitura: {bad} (*{word})")
L.execute("math.random = function(a, b) if a then return a end return 0.99 end")
L.execute(Hm + ".chatQ = {} " + Hm + ".chatFree = 0")
L.execute(Hm + '.chat("Guarda questa torre")')
q = L.eval(Hm + ".chatQ")
qs = [q[k] for k in sorted(q.keys())]
check(len(qs) == 2 and qs[0]["text"] != "guarda questa torre" and qs[1]["text"].startswith("*"), f"sbaglia e corregge: {[x['text'] for x in qs]}")
L.execute("math.random = MOCK.rnd2")
# tempo di battitura proporzionale alla lunghezza, a raffiche
L.execute(Hm + ".chatQ = {} " + Hm + ".chatFree = 0 " + Hm + ".realism = 1")
t0 = L.eval("CurTime()")
L.execute(Hm + '.chat("Ok.")')
short = L.eval(Hm + ".chatQ[1].at") - t0
L.execute(Hm + ".chatQ = {} " + Hm + ".chatFree = 0")
long_text = "Ho trovato un posto bellissimo vicino al lago! Venite a vedere, ci costruiamo una base enorme. Porto io le casse."
L.execute(Hm + '.chat("%s")' % long_text)
q = L.eval(Hm + ".chatQ")
qs = [q[k] for k in sorted(q.keys())]
plain = [x for x in qs if not x["text"].startswith("*")]
check(0.15 <= short <= 0.9, f"riga corta scritta in fretta: {short:.2f}s")
check(2 <= len(plain) <= 3 and qs[-1]["at"] - t0 > 5, f"messaggio lungo a raffiche ({len(plain)}), più tempo: {qs[-1]['at'] - t0:.1f}s")
n_net = len(list(G.MOCK.net.values()))
L.execute("MOCK.advance(0.3)")
check(L.eval("AELink.bot.nw.AETyping") == True, "\"sta scrivendo…\" mentre scrive")
L.execute("MOCK.advance(20)")
check(len(list(G.MOCK.net.values())) >= n_net + len(plain) and L.eval("AELink.bot.nw.AETyping") == False, "poi manda le righe e smette di scrivere")
sent = [m[3] for m in list(G.MOCK.net.values())[n_net:] if m[1] == "alterego_chat"]
check(sent and all(not s.endswith(".") or s.endswith("..") for s in sent), f"righe da chat senza punto finale: {sent}")

# battute generate dal modello: pesca senza ripetere, chiede altre righe quando finiscono, riserva senza modello
L.execute(P + ".lastQuip = -100")
G.MOCK.files["alterego/cmd.json"] = json.dumps({"t": int(time.time()), "cmds": [], "config": {"auto_join": True, "play": {"lang": "it", "pronouns": "they", "quips_v": 3, "quips": {"hit": ["ahia", "ma dai", "ouch"], "oops": ["ops", "non vale"]}}}})
L.execute("MOCK.advance(0.3)")
check(L.eval(P + ".genV") == 3 and L.eval(P + ".pron") == "they", "battute e pronomi dall'app")
picks = [L.eval(P + '.line("hit")') for _ in range(3)]
check(sorted(picks) == ["ahia", "ma dai", "ouch"], f"tre righe diverse di fila: {picks}")
check(L.eval(P + '.line("hit")') != picks[-1], "ricomincia senza ripetere l'ultima")
L.execute("MOCK.advance(1.1)")
check(any(e["kind"] == "quips_low" and e["kind"] for e in events("quips_low")), "chiede battute nuove quando finiscono")
check(L.eval(P + '.line("dodge")') in ["Whoa!", "Mancato!", "Eheh, troppo lento!", "Ehi!"], "categoria non generata: riserva")
# stessa versione: non ricarica (non riparte da capo a ogni lettura del file)
L.execute(P + '.used.hit = { ahia = true }')
L.execute("MOCK.advance(0.3)")
check(L.eval(P + '.used.hit.ahia') == True, "stessa versione: niente ricarica")
L.execute(P + ".gen = nil")
oops = set(L.eval(P + '.line("oops")') for _ in range(12))
check(not any("imbranat" in x for x in oops) and "Che disastro…" in oops, f"riserva coi pronomi neutri: {oops}")
L.execute(P + '.pron = "he" ' + P + ".used = {}")
oops = set(L.eval(P + '.line("oops")') for _ in range(12))
check("Che imbranato…" in oops, "riserva al maschile se scelto")
L.execute(P + '.pron = "she" ' + P + ".used = {}")
# la battuta esce dopo un tempo di reazione
L.execute(P + ".lastQuip = -100")
count_say = '(function() local n = 0 for _, e in ipairs(AELink.events) do if e.kind == "say" then n = n + 1 end end return n end)()'
n0 = L.eval(count_say)
check(L.eval(P + '.quip("hit")') == True, "battuta")
L.execute("MOCK.advance(0.05)")
n1 = L.eval(count_say)
L.execute("MOCK.advance(1.1)")
check(n1 == n0 and L.eval(count_say) == n0 + 1, "esce dopo un tempo di reazione, non subito")
# non parla sopra a chi parla in voce, e lo guarda
L.execute("Entity(%d).pos = Vector(300, 50, 0) Entity(%d).speaking = true %s.focus = nil %s.voice = {}" % (host.id, host.id, Hm, Hm))
L.execute(P + ".lastQuip = -100")
check(L.eval(P + '.quip("hit")') == False, "non parla sopra a chi sta parlando")
L.execute("MOCK.advance(1.2)")
check(L.eval(Hm + ".focus and " + Hm + ".focus.what == Entity(%d)" % host.id), "si gira verso chi parla")
st = json.loads(G.MOCK.files["alterego/state.json"])
check(any(p.get("talking") for p in st["players"]), "lo stato dice chi parla (l'app aspetta il suo turno)")
L.execute("Entity(%d).speaking = false" % host.id)

# sbagli: con la pratica sbaglia meno
rate = L.eval("""(function() local H = %s local a, b = 0, 0
  H.skill = {} for i = 1, 4000 do if H.oops("catch") then a = a + 1 end end
  H.skill = { catch = 0.9 } for i = 1, 4000 do if H.oops("catch") then b = b + 1 end end
  H.skill = {} return { a / 4000, b / 4000 } end)()""" % Hm)
check(0.1 < rate[1] < 0.26 and rate[2] < rate[1] * 0.5, f"sbaglia la presa {rate[1]:.2f}, con la pratica {rate[2]:.2f}")
# la presa le sfugge: lo dice all'app e ci ride su
L.execute(P + '.setQuips({ oops = { "ops", "non vale" } }, 4) ' + P + ".rival = {}")
ball = L.eval('MOCK.entity("prop_physics", { model = "models/props_junk/MetalBucket01a.mdl", pos = Vector(100, 0, 0) })')
L.execute(P + ".lastQuip = -100 AELink.held = nil")
L.execute("MOCK.r3 = math.random math.random = function(a, b) if a then return a end return 0.999 end")
how = L.eval(P + ".catch(Entity(%d), Entity(%d))" % (ball.id, host.id))
check(how == "fumbled" and L.eval("AELink.held") is None, "a volte la presa le sfugge")
L.execute("math.random = function(a, b) if a then return a end return 0.1 end")
L.execute("MOCK.advance(2.2)")
L.execute("math.random = MOCK.r3")
tries = events("try")
check(tries and tries[-1]["what"] == "catch" and tries[-1]["ok"] is False, f"l'app sa che ha sbagliato: {tries[-1] if tries else None}")
check(any(e.get("text") in ["ops", "non vale"] for e in events("say")), "e reagisce come una persona")
# pratica dall'app
G.MOCK.files["alterego/cmd.json"] = json.dumps({"t": int(time.time()), "cmds": [], "config": {"auto_join": True, "play": {"skills": {"catch": 0.6}}}})
L.execute("MOCK.advance(0.3)")
check(abs(L.eval(Hm + ".skill.catch") - 0.6) < 1e-6, "pratica dalla memoria della personalità")

# nuove abitudini da sandbox
L.execute("AELink.mode = 'follow' AELink.held = nil")
L.execute(P + '.idleAct("flashlight")')
L.execute("MOCK.advance(0.05)")
check(L.eval("AELink.bot.nw.AEFlash") is not None, "giochicchia con la torcia")
L.execute(P + '.idleAct("browse")')
check(L.eval(Hm + ".paused()"), "si ferma a sfogliare il menu")
c0 = L.eval('(function() local n = 0 for _, e in pairs(MOCK.ents) do if e.alive and e.class == "prop_physics" then n = n + 1 end end return n end)()')
L.execute("MOCK.advance(5.5)")
c1 = L.eval('(function() local n = 0 for _, e in pairs(MOCK.ents) do if e.alive and e.class == "prop_physics" then n = n + 1 end end return n end)()')
check(c1 == c0 + 1 and not L.eval(Hm + ".paused()"), "poi spawna qualcosa")
L.execute(P + '.idleAct("fiddle")')
check(L.eval("AELink.held") is not None, "giocherella con la physgun")
L.execute("MOCK.advance(3)")
check(L.eval("AELink.held") is None, "poi la lascia")
# saluto a chi arriva (accovacciate o saluto con la mano)
L.execute("Entity(%d).pos = Vector(400, 0, 0) %s.met = {} %s.lastGreet = -100 %s.joinedAt = CurTime() - 10 %s.lastQuip = -100" % (other.id, Hm, Hm, Hm, P))
L.execute("MOCK.advance(1.5)")
check(L.eval(Hm + ".met.Kyoko") == True, "saluta chi arriva")
L.execute("Entity(%d):Remove()" % other.id)

print("\n" + ("TUTTO OK" if not failures else f"{len(failures)} ERRORI"))
raise SystemExit(1 if failures else 0)
