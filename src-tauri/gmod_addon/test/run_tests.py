"""Prova l'addon Alter Ego Link fuori da Garry's Mod (API finte in gmod_mock.lua).

    pip install lupa
    python3 src-tauri/gmod_addon/test/run_tests.py
"""
import json
import os
import sys

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


# giocatore che ospita, un'anguria, uno zombie, una gallina
host = L.eval('MOCK.entity("player", { name = "Makoto", host = true, pos = Vector(0, 0, 0) })')
melon = L.eval('MOCK.entity("prop_physics", { model = "models/props_junk/watermelon01.mdl", pos = Vector(200, 0, 0) })')
zombie = L.eval('MOCK.entity("npc_zombie", { pos = Vector(-600, 0, 0), hostile = true })')
chicken = L.eval('MOCK.entity("npc_chicken", { pos = Vector(400, 300, 0) })')

L.execute('dofile(MOCK.root .. "/lua/autorun/server/sv_alterego_link.lua")')
L.execute('MOCK.run("InitPostEntity")')


def state():
    return json.loads(G.MOCK.files["alterego/state.json"])


def send(cmds, t=None):
    import time
    G.MOCK.files["alterego/cmd.json"] = json.dumps({"t": t or int(time.time()), "cmds": cmds})


# l'app si collega: entra da sola
send([])
L.execute("MOCK.advance(4)")
check(L.eval("IsValid(AELink.bot)"), "entra da sola quando l'app è collegata")
s = state()
check(any(e["kind"] == "joined" for e in s["events"]), "evento joined")
check(s["bot"]["mode"] == "follow" and s["bot"]["following"] == "Makoto", "segue chi ospita")
names = [n["name"] for n in s.get("near", [])]
check("melon" in names and "zombie" in names, f"vede gli oggetti vicini: {names}")

# stato vero dell'addon per il test in Rust (gmodlink::tests::parses_real_addon_state)
with open(os.path.join(ROOT, "test", "sample_state.json"), "w") as f:
    f.write(G.MOCK.files["alterego/state.json"])

# configurazione dall'app (nome, modello)
G.MOCK.files["alterego/cmd.json"] = json.dumps({"t": int(__import__("time").time()), "cmds": [], "config": {"name": "Chihiro", "model": "models/x.mdl", "auto_join": True, "tasks": True}})
L.execute("MOCK.advance(0.3)")
check(L.eval("AELink.name") == "Chihiro" and L.eval("AELink.model") == "models/x.mdl", "nome e modello dalle impostazioni")

# menu C: playermodel scelto nel gioco → NPC invisibile + playermodel incollato sopra
L.execute('MOCK.fromClient("alterego_menu", Entity(%d), "model", "models/dr/chihiro.mdl")' % host.id)
skin = L.eval("AELink.bot.AESkin")
check(skin is not None and skin.model == "models/dr/chihiro.mdl" and L.eval("AELink.bot.AESkin.parent == AELink.bot"), "playermodel incollato sull'NPC")
L.execute("MOCK.advance(1.2)")
check(any(e["kind"] == "model" and e["model"] == "models/dr/chihiro.mdl" for e in state()["events"]), "evento model per l'app")
# l'app ha ancora il vecchio modello nelle impostazioni: non deve sovrascrivere la scelta del menu
G.MOCK.files["alterego/cmd.json"] = json.dumps({"t": int(__import__("time").time()), "cmds": [], "config": {"model": "models/x.mdl"}})
L.execute("MOCK.advance(0.3)")
check(L.eval("AELink.model") == "models/dr/chihiro.mdl", "la scelta del menu non viene sovrascritta")
# un modello NPC va direttamente sul corpo
L.execute('MOCK.fromClient("alterego_menu", Entity(%d), "model", "models/humans/group02/female_02.mdl")' % host.id)
check(L.eval("AELink.bot.AESkin") is None and L.eval("AELink.bot.model") == "models/humans/group02/female_02.mdl", "modello NPC direttamente sul corpo")
L.execute('MOCK.fromClient("alterego_menu", Entity(%d), "name", "Chihiro")' % host.id)
check(L.eval("AELink.bot.nw.AEName") == "Chihiro", "nome dal menu")

# comandi
send([{"id": 1, "op": "say", "text": "Ciao!"}, {"id": 2, "op": "spawn", "what": "crate", "n": 2}, {"id": 3, "op": "gesture", "name": "wave"}])
L.execute("MOCK.advance(0.5)")
check(len(list(G.MOCK.net.values())) >= 1, "riga di chat inviata ai giocatori")
crates = L.eval('(function() local n = 0 for _, e in pairs(MOCK.ents) do if e.model == "models/props_junk/wood_crate001a.mdl" then n = n + 1 end end return n end)()')
check(crates == 2, f"spawn di 2 casse ({crates})")
# un comando già eseguito non si ripete
send([{"id": 2, "op": "spawn", "what": "crate", "n": 2}])
L.execute("MOCK.advance(0.5)")
crates2 = L.eval('(function() local n = 0 for _, e in pairs(MOCK.ents) do if e.model == "models/props_junk/wood_crate001a.mdl" then n = n + 1 end end return n end)()')
check(crates2 == 2, "comando con id vecchio ignorato")

# chat dei giocatori
L.execute('MOCK.run("PlayerSay", Entity(%d), "Alter Ego, vieni qui")' % host.id)
L.execute("MOCK.advance(1.2)")
chats = [e for e in state()["events"] if e["kind"] == "chat"]
check(chats and chats[-1]["from"] == "Makoto" and chats[-1]["host"], "messaggio di chat ricevuto")

# task Lua: caccia alle galline (attacca, la gallina muore, il task finisce)
code = 'local c = nearest("chicken", 2000) if c then say("Eccone una!") attack(c, 10) end'
send([{"id": 4, "op": "task", "name": "galline", "code": code}])
L.execute("MOCK.advance(0.5)")
check(L.eval("AELink.mode") == "attack", "il task attacca la gallina")
L.execute("Entity(%d).hp = 0" % chicken.id)
L.execute("MOCK.advance(1.5)")
ev = state()["events"]
check(any(e["kind"] == "say" and e["text"] == "Eccone una!" for e in ev), "say() dal task")
check(any(e["kind"] == "task" and e["name"] == "galline" and e["status"] == "done" for e in ev), "task finito")

# task che prova a uscire dalla sandbox
send([{"id": 5, "op": "task", "name": "furbo", "code": "RunConsoleCommand('quit')"}])
L.execute("MOCK.advance(0.5)")
ev = state()["events"]
check(any(e["kind"] == "task" and e["name"] == "furbo" and e["status"] == "error" for e in ev), "niente funzioni di GMod nei task")
send([{"id": 6, "op": "task", "name": "loop", "code": "while true do end"}])
L.execute("MOCK.advance(1.2)")
ev = state()["events"]
check(any(e["kind"] == "task" and e["name"] == "loop" and e["status"] == "error" for e in ev), "ciclo infinito fermato")
send([{"id": 7, "op": "task", "name": "rotto", "code": "questo non è lua"}])
L.execute("MOCK.advance(1.2)")
ev = state()["events"]
check(any(e["kind"] == "task" and e["name"] == "rotto" and e["status"] == "error" and e.get("error") for e in ev), "errore di sintassi riportato")

# physgun: porta l'anguria
send([{"id": 8, "op": "grab", "what": "#%d" % melon.id}])
L.execute("MOCK.advance(0.5)")
check(L.eval("AELink.held and AELink.held.id") == melon.id, "prende l'anguria")
L.execute("MOCK.advance(0.3)")
check(L.eval("Entity(%d).physvel ~= nil" % melon.id), "l'oggetto tenuto si muove")
send([{"id": 9, "op": "throw"}])
L.execute("MOCK.advance(0.2)")
check(L.eval("AELink.held") is None, "lancia l'oggetto")

# task lungo: porta qui l'anguria lontana (cammina, prende, torna, lascia)
L.execute("MOCK.walk = true; Entity(%d):SetPos(Vector(2500, 0, 0))" % melon.id)
send([{"id": 12, "op": "task", "name": "porta", "code": "local m = nearest('melon', 4000) if bring(m) then say('Eccola!') end"}])
L.execute("MOCK.advance(3)")
ev = state()["events"]
check(any(e["kind"] == "say" and e["text"] == "Eccola!" for e in ev), "bring(): va a prendere e riporta")
check(L.eval("AELink.held") is None, "alla fine lascia l'oggetto")

# morte e rientro
L.execute('MOCK.run("OnNPCKilled", AELink.bot, Entity(%d))' % zombie.id)
check(L.eval("AELink.bot") is None, "morta")
L.execute("MOCK.advance(7)")
check(L.eval("IsValid(AELink.bot)"), "rientra dopo la morte")

# esce e non rientra da sola
send([{"id": 13, "op": "leave"}])
L.execute("MOCK.advance(5)")
check(not L.eval("IsValid(AELink.bot)"), "esce e resta fuori")
send([{"id": 14, "op": "join"}])
L.execute("MOCK.advance(0.5)")
check(L.eval("IsValid(AELink.bot)"), "rientra con join")

# app scollegata: niente rientro automatico
L.execute("AELink.leave()")
send([], t=1)
L.execute("AELink.left = false")
L.execute("MOCK.advance(5)")
check(not L.eval("IsValid(AELink.bot)"), "senza l'app non entra")

print()
print("FALLITI: %d" % len(failures) if failures else "tutto ok")
sys.exit(1 if failures else 0)
