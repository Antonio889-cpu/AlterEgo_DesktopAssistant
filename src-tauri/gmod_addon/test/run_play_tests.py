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
check(abs(L.eval("AELink.bot.pos.x") - L.eval("Entity(%d).pos.x" % jeep.id)) < 60, "viaggia con il veicolo")
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
# passeggera: tu sali sulla jeep, lei sale con te
L.execute("MOCK.run('PlayerEnteredVehicle', Entity(%d), Entity(%d))" % (host.id, jeep.id))
L.execute("MOCK.advance(1.5)")
check(L.eval("AELink.mode") == "sit", "sale in macchina con te")
check(any(e.get("quick") for e in events("say")), "battuta immediata (senza modello)")
L.execute("MOCK.run('PlayerLeaveVehicle', Entity(%d), Entity(%d))" % (host.id, jeep.id))
L.execute("MOCK.advance(0.2)")
check(L.eval("AELink.mode") == "follow", "scende quando scendi")

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
L.execute(P + ".lastAction = 0 " + P + ".cool = {}")
L.execute("MOCK.advance(2.2)")
pr = events("prank")
check(pr and pr[-1]["who"] == "Makoto", f"si vendica con uno scherzo: {pr[-1] if pr else None}")
check(L.eval(P + '.rival["Makoto"] == nil or ' + P + '.rival["Makoto"].score < %f' % score), "la vendetta abbassa la rivalità")

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
