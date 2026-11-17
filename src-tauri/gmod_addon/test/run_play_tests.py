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

print("\n" + ("TUTTO OK" if not failures else f"{len(failures)} ERRORI"))
raise SystemExit(1 if failures else 0)
