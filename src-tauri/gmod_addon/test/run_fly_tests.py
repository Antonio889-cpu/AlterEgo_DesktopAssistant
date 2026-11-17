"""Prova i sensi e i riflessi del cervello di moscerino (lua/alterego/fly.lua) con l'API finta.

    pip install lupa
    python3 src-tauri/gmod_addon/test/run_fly_tests.py
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
send({"on": True, "reflexes": True, "hud": False, "senses": ALL})
L.execute("MOCK.advance(4)")
check(L.eval("IsValid(AELink.bot)"), "entra")
bot = L.eval("AELink.bot")
check("alterego/sense.json" in dict(G.MOCK.files), "scrive i sensi")
check(senses() == {}, f"a riposo nessun senso: {senses()}")

# qualcosa che arriva veloce dalla sua sinistra (+y con lo sguardo verso +x)
bp = L.eval("AELink.bot:GetPos()")
L.execute(f"MOCK.cannon = MOCK.entity('prop_physics', {{ model = 'models/props_c17/oildrum001.mdl', pos = Vector({bp.x}, {bp.y} + 250, 0), vel = Vector(0, -600, 0) }})")
L.execute("AELink.bot.ang = Angle(0, 0, 0)")
L.execute("MOCK.advance(0.2)")
s = senses()
check(s.get("loom_l", 0) > 0.3 and s.get("loom_r", 0) == 0, f"looming da sinistra: {s}")
# l'app risponde: fuga
out({"escape": 0.9, "turn_l": 0, "turn_r": 0.5})
L.execute("MOCK.advance(0.2)")
check(L.eval("AELink.mode") == "goto", "fugge (controllo dei piedi)")
goal = L.eval("AELink.goal")
check(goal is not None and goal.y < bp.y - 100, f"si allontana dalla minaccia: {goal}")
check(L.eval("AELink.bot.vel and AELink.bot.vel.z") == 260, "salta")
L.execute("AELink.bot.vel = nil")
L.execute("MOCK.cannon:Remove()")
out({})
L.execute("MOCK.advance(2.5)")
check(L.eval("AELink.mode") == "follow", "torna a seguire dopo la fuga")

# anguria vicina: dolce, la mangia
bp = L.eval("AELink.bot:GetPos()")
L.execute(f"MOCK.melon = MOCK.entity('prop_physics', {{ model = 'models/props_junk/watermelon01.mdl', pos = Vector({bp.x} + 60, {bp.y}, 0) }})")
L.execute("AELink.bot.hp = 50")
L.execute("MOCK.advance(0.2)")
s = senses()
check(s.get("sweet") == 1 and s.get("smell_food", 0) > 0.8, f"sente il dolce e l'odore: {s}")
out({"feed": 0.8})
L.execute("MOCK.advance(0.3)")
check(not L.eval("IsValid(MOCK.melon)") and L.eval("AELink.bot.hp") == 65, "mangia l'anguria e recupera vita")
L.execute("MOCK.advance(1.2)")
ev = json.loads(G.MOCK.files["alterego/state.json"])["events"]
check(any(e["kind"] == "ate" for e in ev), "evento 'ate' per l'app")
out({})

# dolore: un colpo
L.execute("""MOCK.run("EntityTakeDamage", AELink.bot, { GetAttacker = function() return Entity(%d) end, GetDamage = function() return 20 end })""" % host.id)
L.execute("MOCK.advance(0.15)")
check(senses().get("pain", 0) > 0.5, f"dolore dopo un colpo: {senses()}")
L.execute("MOCK.advance(1.5)")
check(senses().get("pain", 0) == 0, "il dolore passa")

# acqua e fuoco
L.execute("AELink.bot.water = 1 AELink.bot.fire = true")
L.execute("MOCK.advance(0.15)")
s = senses()
check(s.get("wet") == 1 and s.get("water", 0) > 0 and s.get("heat") == 1 and s.get("pain", 0) >= 0.8, f"acqua e fuoco: {s}")
L.execute("AELink.bot.water = 0 AELink.bot.fire = false")

# torcia puntata in faccia
L.execute("Entity(%d).flashlight = true" % host.id)
L.execute("Entity(%d).aim = (AELink.bot:EyePos() - Entity(%d):EyePos()):GetNormalized()" % (host.id, host.id))
L.execute("MOCK.advance(0.15)")
check(senses().get("light", 0) > 0.3, f"abbagliata dalla torcia: {senses()}")
L.execute("Entity(%d).flashlight = false" % host.id)

# suono forte vicino
L.execute("MOCK.run('EntityEmitSound', { Entity = Entity(%d), SoundName = 'weapons/pistol.wav', SoundLevel = 110 })" % host.id)
L.execute("MOCK.advance(0.15)")
check(senses().get("sound", 0) > 0.3, f"sente lo sparo: {senses()}")

# sensi opzionali spenti: niente
send({"on": True, "reflexes": True, "hud": True, "senses": ["pain", "loom_l", "loom_r", "sweet", "touch"]})
L.execute("MOCK.advance(0.3)")
L.execute("AELink.bot.water = 1")
out({"groom": 0.7, "escape": 0})
L.execute("MOCK.advance(0.3)")
check("wet" not in senses(), "senso opzionale spento")
check(L.eval("AELink.bot.nw.AEFly") and "g=0.7" in L.eval("AELink.bot.nw.AEFly"), f"attività sopra la testa: {L.eval('AELink.bot.nw.AEFly')}")

# riflessi spenti: niente movimenti, i sensi continuano
send({"on": True, "reflexes": False, "hud": False, "senses": []})
L.execute("MOCK.advance(0.3)")
out({"escape": 1})
L.execute("AELink.fly.cool = {}")
L.execute("MOCK.advance(0.3)")
check(L.eval("AELink.mode") == "follow", "riflessi spenti: non scappa")
check(L.eval("AELink.bot.nw.AEFly") == "", "attività nascosta")

# cervello spento: niente file
send({"on": False})
L.execute("MOCK.advance(0.3)")
del G.MOCK.files["alterego/sense.json"]
L.execute("MOCK.advance(0.5)")
check("alterego/sense.json" not in dict(G.MOCK.files), "spento: non scrive i sensi")

print("\n" + ("TUTTO OK" if not failures else f"{len(failures)} ERRORI"))
raise SystemExit(1 if failures else 0)
