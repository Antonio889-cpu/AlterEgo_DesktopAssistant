"""Corpo Lambda Player dell'addon Alter Ego Link con un finto Lambda Players (vedi gmod_mock.lua).

    python3 src-tauri/gmod_addon/test/run_lambda_tests.py
"""
import json, os, sys, time
from lupa import luajit21 as lj

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
L = lj.LuaRuntime(unpack_returned_tuples=True)
G = L.globals()

def to_py(v):
    if lj.lua_type(v) == "table":
        keys = list(v.keys())
        if keys and all(isinstance(k, int) for k in keys) and sorted(keys) == list(range(1, len(keys) + 1)):
            return [to_py(v[k]) for k in sorted(keys)]
        return {str(k): to_py(v[k]) for k in keys} if keys else []
    return v

def to_lua(v):
    if isinstance(v, dict): return L.table_from({k: to_lua(x) for k, x in v.items()})
    if isinstance(v, list): return L.table_from([to_lua(x) for x in v])
    return v

L.execute(open(os.path.join(ROOT, "test", "gmod_mock.lua")).read())
G.MOCK.root = ROOT
G.MOCK.tojson = lambda t: json.dumps(to_py(t))
G.MOCK.fromjson = lambda s: to_lua(json.loads(s))
G.MOCK.has_lambda = True
failures = []
def check(cond, msg):
    print(("ok   " if cond else "FAIL ") + msg)
    if not cond: failures.append(msg)
def state(): return json.loads(G.MOCK.files["alterego/state.json"])
def send(cmds, cfg=None):
    d = {"t": int(time.time()), "cmds": cmds}
    if cfg: d["config"] = cfg
    G.MOCK.files["alterego/cmd.json"] = json.dumps(d)

host = L.eval('MOCK.entity("player", { name = "Makoto", host = true, pos = Vector(0, 0, 0) })')
chicken = L.eval('MOCK.entity("npc_chicken", { pos = Vector(400, 300, 0) })')
L.execute('dofile(MOCK.root .. "/lua/autorun/server/sv_alterego_link.lua")')
L.execute('MOCK.run("InitPostEntity")')
send([], {"name": "Chihiro", "model": "models/dr/chihiro.mdl"})
L.execute("MOCK.advance(4)")

check(L.eval("AELink.kind") == "lambda" and L.eval("AELink.bot.class") == "npc_lambdaplayer", "con Lambda installato entra come Lambda Player")
check(L.eval("AELink.bot.nocoms") is True and L.eval("AELink.bot.lname") == "Chihiro", "niente chat/voce sue, nome giusto")
check(L.eval("AELink.bot.model") == "models/dr/chihiro.mdl", "playermodel scelto")
check(L.eval("AELink.bot:GetNW2String('lambda_state')") == "AELinkCtl", "stato comandato da Alter Ego")
s = state()
check(s.get("lambda") is True and s["bot"].get("body") == "lambda", "lo stato dice il corpo in uso")

# niente iniziative sue
L.execute("AELink.bot:SetState('Idle')")
check(L.eval("AELink.bot:GetNW2String('lambda_state')") == "AELinkCtl", "non può tornare a girare da sola")
L.execute("AELink.bot:AttackTarget(Entity(%d))" % host.id)
check(L.eval("AELink.bot:GetNW2String('lambda_state')") == "AELinkCtl", "non attacca chi ospita")
check(L.eval("MOCK.run('LambdaOnToolUse', AELink.bot)") is True, "niente toolgun da sola")
check(L.eval("MOCK.run('LambdaOnSetPlayerModel', AELink.bot)") == "models/dr/chihiro.mdl", "il modello resta quello scelto")

# ti segue
L.execute("Entity(%d):SetPos(Vector(1500, 0, 0))" % host.id)
L.execute("MOCK.advance(1)")
check(L.eval("AELink.bot.pos:Distance(Entity(%d).pos)" % host.id) < 200, "ti segue")
check(not L.eval("AELink.bot.wandered"), "non vaga per conto suo")

# comandi
send([{"id": 1, "op": "gesture", "name": "wave"}, {"id": 2, "op": "give", "weapon": "smg"}, {"id": 3, "op": "jump"}])
L.execute("MOCK.advance(0.3)")
check(L.eval("AELink.bot.gesture") == 101 and L.eval("AELink.bot.l_Weapon") == "smg" and L.eval("AELink.bot.jumped"), "gesto, arma e salto da giocatore")

# attacco su comando: poi torna a seguirti
send([{"id": 4, "op": "attack", "what": "#%d" % chicken.id}])
L.execute("MOCK.advance(0.3)")
check(L.eval("AELink.bot:GetNW2String('lambda_state')") == "Combat", "attacca il bersaglio indicato")
L.execute("Entity(%d).hp = 0" % chicken.id)
L.execute("MOCK.advance(1)")
check(L.eval("AELink.mode") == "follow" and L.eval("AELink.bot:GetNW2String('lambda_state')") == "AELinkCtl", "finito l'attacco torna a seguirti")

# task: vai in un punto
send([{"id": 5, "op": "task", "name": "vai", "code": "if go(vec(3000, 0, 0), 100, 10) then say('Arrivata!') end"}])
L.execute("MOCK.advance(2)")
check(any(e["kind"] == "say" and e["text"] == "Arrivata!" for e in state()["events"]), "go() funziona col corpo Lambda")

# physgun vera in mano quando prende un oggetto
melon = L.eval('MOCK.entity("prop_physics", { model = "models/props_junk/watermelon01.mdl", pos = AELink.bot.pos + Vector(50, 0, 0) })')
send([{"id": 6, "op": "grab", "what": "#%d" % melon.id}])
L.execute("MOCK.advance(0.3)")
check(L.eval("AELink.held and AELink.held.id") == melon.id and L.eval("AELink.bot.l_Weapon") == "physgun", "prende con la physgun")

# morte e rinascita del Lambda
L.execute("AELink.bot.dead = true; MOCK.run('LambdaOnKilled', AELink.bot, nil)")
check(any(e["kind"] == "died" for e in to_py(L.eval("AELink.events"))), "evento died")
L.execute("AELink.bot.dead = false; MOCK.run('LambdaOnRespawn', AELink.bot)")
check(L.eval("AELink.deadAt") is None and L.eval("AELink.bot:GetNW2String('lambda_state')") == "AELinkCtl", "rinasce comandata")

# cambio corpo dal menu C
L.execute('MOCK.fromClient("alterego_menu", Entity(%d), "body", "citizen")' % host.id)
check(L.eval("AELink.kind") == "citizen" and L.eval("AELink.bot.class") == "npc_citizen", "dal menu passa al corpo cittadino")
L.execute('MOCK.fromClient("alterego_menu", Entity(%d), "body", "auto")' % host.id)
check(L.eval("AELink.kind") == "lambda", "e torna Lambda in automatico")

print()
print("FALLITI: %d" % len(failures) if failures else "tutto ok")
sys.exit(1 if failures else 0)
