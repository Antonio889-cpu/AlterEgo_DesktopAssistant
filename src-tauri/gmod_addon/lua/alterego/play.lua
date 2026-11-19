-- Alter Ego Link: Alter Ego come GIOCATRICE (non come insetto).
-- Il cervello di moscerino dà le spinte (adrenalina, sussulti, curiosità); qui si decide come le
-- esprimerebbe una persona che sta giocando a Garry's Mod con gli amici:
--   * schiva invece di scappare, e si abitua (al decimo barile non sobbalza più);
--   * si ricorda chi la bersaglia (rivalità che sfuma) e prima o poi si vendica con uno scherzo;
--   * quando non succede niente gioca per conto suo (costruisce, tira ai barili, emote, saltelli);
--   * battute brevi immediate senza modello AI (voce e chat): le scrive il modello una volta ogni tanto
--     (un pacchetto per categoria, nella lingua e coi pronomi giusti); le tabelle qui sotto sono la riserva.
-- Tutto qui gira nel gioco: zero token.

local L = AELink
local P = L.play or {}
L.play = P
P.rival = P.rival or {}       -- nick → { score, ply }
P.habit = {}                  -- abituazione per tipo di riflesso
P.cool = {}
P.mischief = P.mischief or "full" -- off | light | full
P.idle = P.idle ~= false
P.lang = P.lang or "it"
P.lastQuip = 0
P.lastAction = 0              -- ultimo riflesso/scherzo (per non fare attività nel mezzo dell'azione)
P.nextIdle = 0
P.chaos = P.chaos or 0.5       -- imprevedibilità (dall'app, 0..1)
P.pron = P.pron or "she"        -- she | he | they (per le frasi di riserva che parlano di lei)
P.gen = P.gen or nil            -- battute generate dal modello: tipo → righe
P.used = {}                     -- righe già dette (niente ripetizioni)
P.lastLine = {}

-- ---------- battute immediate (niente modello) ----------
-- Riserva quando il modello non c'è. Niente frasi al femminile o al maschile su di lei:
-- dove servirebbe c'è una riga per pronome ({ she, he, they }).
local Q = {
  it = {
    dodge = { "Whoa!", "Mancato!", "Eheh, troppo lento!", "Ehi!" },
    flee = { "Aiuto!", "Nonono!", "Scappo!" },
    annoyed = { "Di nuovo?!", "Ancora…?", "Sì sì, ho capito.", "Che originalità…" },
    hit = { "Ahia!", "Ehi! Fa male!", "Ma dai!", "Ahi!" },
    died = { "Nooo!", "gg…", "Me la paghi!", "Respawno subito!" },
    prank = { "Ehehe, vendetta!", "Te lo meritavi!", "Ops… è scivolato!", "Pareggio!" },
    melon = { "In arrivo!", "Prendi!", "Al volo!" },
    catch = { "Presa!", "Al volo!", "Grazie, è mia adesso.", "Troppo facile!" },
    back = { "Te la rimando!", "Rispedita al mittente!", "Tieni, è tua!" },
    juggle = { "Guarda!", "Hop!" },
    build = { "Guarda cosa costruisco!", "Torre!", "Ta-dan!" },
    target = { "Tiro al bersaglio!", "Vediamo se lo prendo…" },
    light = { "La luce! Non vedo niente!", "Abbassa la torcia!" },
    wheee = { "Wheee!", "Sto volando!" },
    ride = { "Passaggio!", "Vengo anch'io!", "Allaccio la cintura!" },
    boom = { "Bum!", "Che botto!" },
    greet = { "Ciao!", "Ehi, ciao!", "Yo!", "Ciaooo" },
    laugh = { "Ahahah!", "LOL", "Ahah, scusa…", "Pfff ahah" },
    oops = { "Ops!", "Ehm… volevo farlo.", { she = "Che imbranata…", he = "Che imbranato…", they = "Che disastro…" }, "Non è successo niente." },
    bored = { "Mi annoio…", "Facciamo qualcosa?", "C'è nessuno?" },
    tired = { "Che sonno…", "Pausa caffè?", "Ho le mani a pezzi…" },
  },
  en = {
    dodge = { "Whoa!", "Missed!", "Hehe, too slow!", "Hey!" },
    flee = { "Help!", "Nonono!", "I'm out!" },
    annoyed = { "Again?!", "Seriously…?", "Yeah yeah, I get it.", "So original…" },
    hit = { "Ow!", "Hey! That hurts!", "Come on!", "Ouch!" },
    died = { "Nooo!", "gg…", "You'll pay for that!", "Respawning!" },
    prank = { "Hehe, revenge!", "You had it coming!", "Oops… it slipped!", "Now we're even!" },
    melon = { "Incoming!", "Catch!", "Heads up!" },
    catch = { "Got it!", "Caught it!", "Thanks, it's mine now.", "Too easy!" },
    back = { "Return to sender!", "Here, have it back!", "Catch this!" },
    juggle = { "Look!", "Hop!" },
    build = { "Look what I'm building!", "Tower!", "Ta-da!" },
    target = { "Target practice!", "Let's see if I hit it…" },
    light = { "The light! I can't see!", "Lower the flashlight!" },
    wheee = { "Wheee!", "I'm flying!" },
    ride = { "Free ride!", "I'm coming too!", "Buckling up!" },
    boom = { "Boom!", "What a blast!" },
    greet = { "Hi!", "Hey!", "Yo!", "Heyyy" },
    laugh = { "Hahaha!", "LOL", "Haha, sorry…", "Pfft haha" },
    oops = { "Oops!", "Um… I meant to do that.", "Wow, smooth…", "That didn't happen." },
    bored = { "I'm bored…", "Let's do something?", "Anyone?" },
    tired = { "So sleepy…", "Coffee break?", "My hands are dead…" },
  },
}
P.Q = Q
local EMO = { dodge = "surprised", flee = "worried", annoyed = "embarrassed", hit = "surprised", died = "sad", prank = "happy",
  melon = "happy", juggle = "happy", build = "happy", target = "happy", light = "surprised", wheee = "happy", ride = "happy", boom = "surprised",
  catch = "happy", back = "happy", greet = "happy", laugh = "happy", oops = "embarrassed", bored = "idle", tired = "sad" }

--- Frase di riserva secondo i pronomi.
function P.g(x)
  if type(x) == "table" then return x[P.pron] or x.they end
  return x
end

--- Una riga per `kind`: generata se ce ne sono, se no di riserva; mai la stessa finché ce ne sono altre.
function P.line(kind)
  local gen = P.gen and P.gen[kind]
  local generated = gen ~= nil and #gen >= 2
  local pool = generated and gen or (Q[P.lang] or Q.it)[kind]
  if not pool then return nil end
  local used = P.used[kind] or {}
  P.used[kind] = used
  local fresh = {}
  for _, x in ipairs(pool) do
    local l = P.g(x)
    if not used[l] then fresh[#fresh + 1] = l end
  end
  if #fresh == 0 then
    -- tutte già dette: si ricomincia, ma non con l'ultima
    used = {}
    P.used[kind] = used
    for _, x in ipairs(pool) do
      local l = P.g(x)
      if l ~= P.lastLine[kind] then fresh[#fresh + 1] = l end
    end
    if #fresh == 0 then fresh = { P.g(pool[1]) } end
  end
  local line = fresh[math.min(#fresh, 1 + math.floor(math.random() * #fresh))]
  used[line] = true
  P.lastLine[kind] = line
  -- quasi finite: lo dice all'app (ne fa scrivere di nuove), al massimo ogni 2 minuti per tipo
  local now = CurTime()
  if generated and #fresh <= 2 and now >= (P.cool["low_" .. kind] or 0) then
    P.cool["low_" .. kind] = now + 120
    L.event("quips_low", { kind = kind })
  end
  return line
end

--- Battute nuove dall'app (versione `v`): si ricomincia a pescare da capo.
function P.setQuips(t, v)
  local gen = {}
  for k, list in pairs(t) do
    if type(list) == "table" then
      local out = {}
      for _, x in ipairs(list) do
        if type(x) == "string" and x ~= "" then out[#out + 1] = x:sub(1, 80) end
      end
      if #out > 0 then gen[tostring(k)] = out end
    end
  end
  P.gen = gen
  P.genV = v
  P.used = {}
end

--- Battuta breve (voce e chat). prob: probabilità; gap minimo 6 s tra una e l'altra.
-- Esce dopo un tempo di reazione, e non sopra a chi sta parlando in voce.
function P.quip(kind, prob)
  local now = CurTime()
  if now - P.lastQuip < 6 then return false end
  if prob and math.random() > prob then return false end
  local H = L.human
  if H and H.realism > 0 and H.anyTalking() then return false end
  local text = P.line(kind)
  if not text then return false end
  P.lastQuip = now
  local function emit() L.event("say", { text = text, emotion = EMO[kind] or "happy", quick = true }) end
  local d = H and H.reaction() or 0
  if d > 0 then timer.Simple(d, emit) else emit() end
  return true
end

-- ---------- utilità ----------
local function ready(name, secs)
  local now = CurTime()
  if now < (P.cool[name] or 0) then return false end
  P.cool[name] = now + secs
  return true
end
P.ready = ready

--- Abituazione: 1 la prima volta, poi sempre meno (si riprende dopo un po' di calma).
local function gain(kind)
  local h = P.habit[kind]
  local now = CurTime()
  local n = 0
  if h then n = h.n * math.exp(-(now - h.t) / 45) end
  P.habit[kind] = { n = n + 1, t = now }
  return 1 / (1 + n * 0.6)
end
P.gain = gain

local function bot() return L.bot end

--- Libera di fare quello che vuole (niente ordini in corso).
function P.free()
  local b = bot()
  if not IsValid(b) then return false end
  if L.mode ~= "follow" and L.mode ~= "stay" then return false end
  if L.sandbox and #L.sandbox.list() > 0 then return false end
  if IsValid(L.held) then return false end
  return true
end

--- Chi ha lanciato questo oggetto (physgun / gravity gun, negli ultimi 5 s).
local function thrower(e)
  if not IsValid(e) then return nil end
  if e:IsPlayer() then return e end
  if IsValid(e.AEThrower) and CurTime() - (e.AEThrownAt or 0) < 5 then return e.AEThrower end
  local d = e.GetDriver and e:GetDriver()
  if IsValid(d) and d:IsPlayer() then return d end
  return nil
end
P.thrower = thrower

local function dangerous(e)
  if not IsValid(e) then return false end
  if e:IsVehicle() then return true end
  local m = (e:GetModel() or ""):lower()
  return m:find("explosive", 1, true) ~= nil or (e.BoundingRadius and e:BoundingRadius() > 70)
end

--- Si può prendere al volo: un oggetto fisico non troppo grande, e lei ha le mani libere.
function P.canCatch(e)
  if not IsValid(e) or IsValid(L.held) or L.mode == "sit" or dangerous(e) then return false end
  if e:GetClass():find("^prop_physics") == nil then return false end
  if (e.BoundingRadius and e:BoundingRadius() or 99) > 60 then return false end
  local ph = e:GetPhysicsObject()
  return IsValid(ph) and ph:IsMoveable()
end

--- La prende con la physgun; a volte la rimanda a chi l'ha lanciata. Ritorna "caught" o "returned".
function P.catch(e, culprit)
  local H = L.human
  if H and H.oops("catch") then
    -- le sfugge: la physgun arriva tardi
    H.tried("catch", false)
    return "fumbled"
  end
  if not L.grab(e) then return "dodged" end
  if H then H.tried("catch", true) end
  local ph = e:GetPhysicsObject()
  if IsValid(ph) then ph:SetVelocity(Vector()) end
  if IsValid(culprit) then L.look(culprit) end
  local back = IsValid(culprit) and P.mischief ~= "off" and math.random() < 0.6
  timer.Simple(back and 0.9 or 2.5, function()
    if L.held ~= e or not IsValid(e) then return end
    if back and IsValid(culprit) then
      if H and H.oops("throw") then
        L.drop() -- le scivola
        H.tried("throw", false)
      else
        L.throw(culprit)
        P.quip("back", 0.8)
        if H then H.tried("throw", true) end
      end
    else
      L.drop()
    end
  end)
  if not back then P.quip("catch", 0.7) end
  return back and "returned" or "caught"
end

-- ---------- rivalità ----------
function P.offend(ply, amount, why)
  if not IsValid(ply) or not ply:IsPlayer() then return end
  local r = P.rival[ply:Nick()] or { score = 0 }
  r.score = math.min(6, r.score + amount)
  r.why = why or r.why
  r.ply = ply
  P.rival[ply:Nick()] = r
end

--- Rivali per lo stato del gioco ({ nick = punteggio } solo se ≥ 1).
function P.rivals()
  local out = {}
  for n, r in pairs(P.rival) do
    if r.score >= 1 then out[n] = math.Round(r.score * 10) / 10 end
  end
  return out
end

-- ---------- reazioni alle spinte del moscerino ----------
function P.react(o, F)
  local b = bot()
  if not IsValid(b) then return end
  local now = CurTime()
  local threat = F.threatEnt
  local from = F.threat or (b:GetPos() - b:GetForward() * 100)
  if L.mode == "sit" then
    -- seduta: solo uno sguardo e una battuta
    if (o.escape or 0) >= 0.5 and ready("sitstartle", 3) then
      if IsValid(threat) then L.look(threat) end
      P.quip("dodge", 0.5)
    end
    return
  end
  local esc = o.escape or 0
  if esc >= 0.5 and ready("escape", 1.2) then
    P.lastAction = now
    if L.human then L.human.excite(esc) end
    local g = gain("escape")
    local culprit = thrower(threat)
    if culprit then P.offend(culprit, 0.7, "keeps throwing things at you") end
    -- a un barile esplosivo o a una macchina non ci si abitua
    local danger = dangerous(threat)
    local strength = danger and esc or esc * g
    local how
    if danger and strength >= 0.5 then
      L.jump()
      F.override(F.awayFrom(b, from, 340, o), 1.6, true)
      P.quip("flee", 0.8)
      how = "fled"
    elseif P.canCatch(threat) and math.random() < 0.25 + (1 - g) * 0.45 + (math.random() - 0.5) * P.chaos * 0.4 then
      -- da persona: la prende al volo con la physgun (più ci si abitua, più è tranquilla)
      how = P.catch(threat, culprit)
    elseif strength >= 0.35 then
      -- schivata: un passo di lato (i neuroni di svolta scelgono la parte), a volte un salto
      local away = (b:GetPos() - from)
      away.z = 0
      away = away:GetNormalized()
      local side = Vector(-away.y, away.x, 0)
      if (o.turn_r or 0) > (o.turn_l or 0) then side = -side end
      how = "dodged"
      if math.random() < 0.35 then L.jump() how = "jumped" end
      F.override(b:GetPos() + side * 110 + away * 40, 0.7, true)
      local who = culprit or threat
      timer.Simple(0.8, function() if IsValid(who) and IsValid(L.bot) then L.look(who) end end)
      P.quip("dodge", 0.45)
    else
      -- ci è abituata: uno sguardo storto
      if IsValid(culprit) then L.look(culprit) elseif IsValid(threat) then L.look(threat) end
      if culprit then P.quip("annoyed", 0.5) end
      how = "ignored"
    end
    -- se lo ricorda (l'app lo mette nel contesto e nella memoria della partita)
    if culprit and ready("thrownev", 3) then
      L.event("thrown", { by = culprit:Nick(), what = L.friendly(threat), how = how })
    end
    return
  end
  if (o.backward or 0) >= 0.5 and ready("backward", 2.5) then
    P.lastAction = now
    local g = gain("backward")
    if g > 0.3 then F.override(F.awayFrom(b, from, 70 * g + 30, o), 0.6, false) end
    return
  end
  -- il dolce non le fa venire fame: le fa venire voglia di giocarci
  if (o.feed or 0) >= 0.5 and IsValid(F.food) and P.free() and ready("melon", 15) then
    P.lastAction = now
    P.playWith(F.food)
    return
  end
  if (o.groom or 0) >= 0.5 and ready("groom", 6) and gain("groom") > 0.4 then L.gesture("shrug") end
  if (o.listen or 0) >= 0.5 and F.soundPos and ready("listen", 3) then
    L.face(F.soundPos)
    -- curiosa: se è libera va a vedere cosa è esploso
    if P.free() and F.blastPos and ready("gosee", 20) then
      F.override(b:GetPos() + (F.blastPos - b:GetPos()) * 0.5, 2.5, true)
    end
  end
  if (o.squint or 0) >= 0.5 and F.lightPos and ready("squint", 4) then
    L.face(b:GetPos() * 2 - F.lightPos)
    P.quip("light", 0.6)
  end
  if (o.brace or 0) >= 0.5 and ready("brace", 4) then
    L.gesture("cheer")
    P.quip("wheee", 0.7)
  end
end

-- ---------- eventi ----------
function P.onHurt(attacker, inflictor, dmg)
  local culprit = (IsValid(attacker) and attacker:IsPlayer()) and attacker or thrower(inflictor) or thrower(attacker)
  if culprit then P.offend(culprit, 0.5 + math.min(dmg or 0, 60) / 30, "hurt you") end
  if L.human then L.human.excite(0.6) end
  P.lastAction = CurTime()
  P.quip("hit", 0.6)
end

function P.onDied(attacker)
  local culprit = (IsValid(attacker) and attacker:IsPlayer()) and attacker or thrower(attacker)
  if culprit then P.offend(culprit, 1.5, "killed you") end
  P.quip("died", 1)
end

hook.Add("PhysgunDrop", "AELinkPlay", function(ply, e)
  if IsValid(e) and e ~= L.bot then e.AEThrower, e.AEThrownAt = ply, CurTime() end
end)
hook.Add("GravGunPunt", "AELinkPlay", function(ply, e)
  if IsValid(e) then e.AEThrower, e.AEThrownAt = ply, CurTime() end
end)
-- qualcuno muore in modo buffo (schiacciato, caduto, esploso da solo) davanti a lei: ride
hook.Add("PlayerDeath", "AELinkPlay", function(victim, _, attacker)
  if not IsValid(L.bot) or not IsValid(victim) or (IsValid(attacker) and (attacker:IsPlayer() and attacker ~= victim or attacker == L.bot)) then return end
  if L.human and not L.human.sees(victim) then return end
  if P.quip("laugh", 0.4) then L.gesture("laugh") end
end)

-- ---------- oggetti ----------
local function spawnAt(model, pos)
  local h = L.host()
  local p = ents.Create("prop_physics")
  if not IsValid(p) then return nil end
  p:SetModel(model)
  p:SetPos(pos)
  p:Spawn()
  p:Activate()
  if IsValid(h) then cleanup.Add(h, "props", p) end
  L.spawned[#L.spawned + 1] = p
  while #L.spawned > (L.MAX_SPAWNED or 80) do
    local old = table.remove(L.spawned, 1)
    if IsValid(old) then old:Remove() end
  end
  return p
end
P.spawnAt = spawnAt

--- Prende un oggetto con la physgun e ci gioca: lo lancia a un rivale vicino o in aria.
function P.playWith(e)
  if not L.grab(e) then return end
  -- prima guarda a chi la tirerà (la mira si assesta)
  local r0 = P.mischief ~= "off" and P.topRival(700)
  if r0 then timer.Simple(0.4, function() if IsValid(r0) then L.look(r0) end end) end
  timer.Simple(1.3, function()
    if L.held ~= e or not IsValid(e) then return end
    local rival = P.topRival(700)
    local H = L.human
    if rival and P.mischief ~= "off" then
      L.look(rival)
      if H and H.oops("throw") then
        L.drop()
        H.tried("throw", false)
      else
        L.throw(rival)
        P.quip("melon", 0.8)
        if H then H.tried("throw", true) end
      end
    else
      L.drop()
      local ph = e:GetPhysicsObject()
      if IsValid(ph) then ph:SetVelocity(Vector(0, 0, 420)) end
      P.quip("juggle", 0.4)
    end
  end)
end

function P.topRival(range, min)
  local b = bot()
  local best, bs
  for _, r in pairs(P.rival) do
    if IsValid(r.ply) and r.ply:Alive() and r.score >= (min or 1) and (not range or (IsValid(b) and r.ply:GetPos():Distance(b:GetPos()) < range)) then
      if not bs or r.score > bs then best, bs = r.ply, r.score end
    end
  end
  return best
end

--- Smette di attaccare `ply` (gli scherzi con le armi durano poco).
local function ceasefire(ply)
  if L.mode == "attack" and L.goal == ply then
    L.mode = "follow"
    L.goal = nil
    L.target = L.host()
  end
  local b = bot()
  if IsValid(b) and L.kind ~= "lambda" and IsValid(ply) then
    pcall(b.AddEntityRelationship, b, ply, D_LI, 99)
    if b.SetEnemy then pcall(b.SetEnemy, b, NULL) end
  end
end

-- ---------- scherzi ----------
P.recent = P.recent or {}
function P.remember(kind)
  table.insert(P.recent, 1, kind)
  while #P.recent > 4 do table.remove(P.recent) end
end

--- Può chiedere all'app di inventarsi qualcosa (app collegata e script permessi).
function P.inventive()
  return L.allowTasks ~= false and L.linked ~= nil and L.linked()
end

--- Vendetta inventata da lei (il modello sceglie: azione pronta o script Lua suo).
function P.askPrank(ply)
  local r = P.rival[ply:Nick()]
  L.event("prank_idea", { who = ply:Nick(), why = r and r.why or "messing with you", recent = table.concat(P.recent, ", ") })
  P.remember("invented")
  if r then r.score = math.max(0, r.score - 1.5) end
  P.lastAction = CurTime()
end

P.PRANKS = {
  light = { "throw", "melon", "crate" },
  full = { "throw", "melon", "crate", "shoot", "boom" },
}

--- Uno scherzo a `ply` (kind facoltativo). Ritorna il tipo fatto o nil.
function P.prank(ply, kind)
  local b = bot()
  if not IsValid(b) or not IsValid(ply) or P.mischief == "off" then return nil end
  local list = P.PRANKS[P.mischief] or P.PRANKS.light
  if not kind or kind == "" then
    kind = list[math.random(#list)]
  else
    local ok = false
    for _, k in ipairs(list) do if k == kind then ok = true end end
    if not ok then kind = list[1] end
  end
  local pp = ply:GetPos()
  if kind == "throw" then
    -- l'oggetto più vicino a lei, o un'anguria nuova
    local best, bd
    for _, e in ipairs(ents.FindInSphere(b:GetPos(), 450)) do
      if e:GetClass() == "prop_physics" and e ~= L.held then
        local ph = e:GetPhysicsObject()
        local d = e:GetPos():Distance(b:GetPos())
        if IsValid(ph) and ph:IsMoveable() and (not bd or d < bd) then best, bd = e, d end
      end
    end
    best = best or spawnAt(L.ITEMS.melon, L.eye() + b:GetForward() * 60)
    if not best or not L.grab(best) then return nil end
    L.look(ply)
    timer.Simple(1, function() if L.held == best then L.throw(ply) end end)
  elseif kind == "melon" then
    spawnAt(L.ITEMS.melon, pp + Vector(0, 0, 140))
  elseif kind == "crate" then
    local f = ply:GetForward()
    f.z = 0
    spawnAt(L.ITEMS.crate, pp + f:GetNormalized() * 60 + Vector(0, 0, 30))
  elseif kind == "shoot" then
    L.give("pistol")
    if not L.attack(ply) then return nil end
    timer.Simple(2.5, function() ceasefire(ply) end)
  elseif kind == "boom" then
    if b:GetPos():Distance(pp) < 260 then return P.prank(ply, "melon") end
    local off = VectorRand and VectorRand() or Vector(1, 0, 0)
    off.z = 0
    local barrel = spawnAt(L.ITEMS.explosive_barrel, pp + off:GetNormalized() * 90 + Vector(0, 0, 20))
    if not barrel then return nil end
    L.give("pistol")
    timer.Simple(0.6, function()
      if IsValid(barrel) and L.attack(barrel) then
        timer.Simple(3, function() if L.goal == barrel then L.mode = "follow" L.goal = nil L.target = L.host() end end)
      end
    end)
  else
    return nil
  end
  P.lastAction = CurTime()
  local r = P.rival[ply:Nick()]
  if r then r.score = math.max(0, r.score - 1.5) end
  P.remember(kind)
  L.event("prank", { who = ply:Nick(), kind = kind })
  P.quip(kind == "boom" and "boom" or "prank", 0.9)
  return kind
end

-- ---------- attività quando non succede niente ----------
P.IDLE = { "stack", "target", "emote", "hop", "watch", "juggle", "flashlight", "browse", "fiddle" }
local BROWSE = { "ball", "crate", "chair", "cone", "bucket", "tire", "box", "melon", "lamp" }
function P.idleAct(kind)
  local b = bot()
  if not IsValid(b) then return nil end
  local H = L.human
  kind = kind or P.IDLE[math.random(#P.IDLE)]
  local fwd = b:GetForward()
  fwd.z = 0
  fwd = fwd:GetNormalized()
  if kind == "stack" then
    local base = b:GetPos() + fwd * 90
    local m = math.random() < 0.5 and L.ITEMS.crate or L.ITEMS.barrel
    -- a volte l'ultima la mette storta e cade
    local bad = H and H.oops("build")
    for i = 1, 3 do spawnAt(m, base + Vector((bad and i == 3) and 22 or 0, 0, 25 + 42 * (i - 1))) end
    if H then H.tried("build", not bad) end
    if not bad then P.quip("build", 0.6) end
  elseif kind == "target" then
    if P.mischief == "off" then return P.idleAct("emote") end
    local barrel = spawnAt(L.ITEMS.barrel, b:GetPos() + fwd * 260 + Vector(0, 0, 20))
    if not barrel then return nil end
    local function shoot()
      if not IsValid(barrel) or not IsValid(L.bot) then return end
      L.give("pistol")
      L.attack(barrel)
      timer.Simple(3, function() if L.goal == barrel then L.mode = "follow" L.goal = nil L.target = L.host() end end)
    end
    if H and H.oops("tool") then
      -- prende l'arma sbagliata, se ne accorge e cambia
      L.give("crowbar")
      H.tried("tool", false)
      timer.Simple(H.rnd(0.8, 1.3), shoot)
    else
      shoot()
      P.quip("target", 0.5)
    end
  elseif kind == "emote" then
    local g = ({ "cheer", "laugh", "wave", "dance" })[math.random(4)]
    if g == "dance" then L.dance() else L.gesture(g) end
  elseif kind == "hop" then
    -- a volte un salto le viene moscio
    local bad = H and H.oops("jump")
    for i = 0, 2 do timer.Simple(i * 0.7, function() if IsValid(L.bot) and L.mode ~= "sit" then L.jump(bad and i == 2) end end) end
    if H then timer.Simple(1.6, function() if IsValid(L.bot) then H.tried("jump", not bad) end end) end
  elseif kind == "watch" then
    local h = L.host()
    local tr = IsValid(h) and h:GetEyeTrace()
    if tr and IsValid(tr.Entity) then L.look(tr.Entity) elseif IsValid(h) then L.look(h) end
  elseif kind == "juggle" then
    local m = spawnAt(L.ITEMS.melon, L.eye() + fwd * 60)
    if m then P.playWith(m) end
  elseif kind == "flashlight" then
    if not H then return nil end
    -- accende e spegne la torcia un paio di volte, come chi non ha niente da fare
    local t = 0
    for _ = 1, H.int(2, 4) do
      timer.Simple(t, function() if IsValid(L.bot) then H.flashlight() end end)
      t = t + H.rnd(0.25, 0.6)
    end
    timer.Simple(t + 12, function() if H.flash and IsValid(L.bot) then H.flashlight(false) end end)
  elseif kind == "browse" then
    if not H then return nil end
    -- ferma a "sfogliare il menu degli oggetti", guarda in basso, poi spawna qualcosa
    local secs = H.rnd(2.5, 5)
    H.pauseUntil = CurTime() + secs
    H.look(L.eye() + fwd * 60 - Vector(0, 0, 45), 2, secs)
    timer.Simple(secs, function()
      if not IsValid(L.bot) then return end
      local f = L.bot:GetForward()
      f.z = 0
      spawnAt(L.ITEMS[H.pick(BROWSE)], L.eye() + f:GetNormalized() * 70)
      L.bot:EmitSound("ui/buttonclickrelease.wav", 50, 110)
    end)
  elseif kind == "fiddle" then
    -- giocherella con la physgun con una sua cosa vicina (o una palla nuova)
    local it
    for i = #L.spawned, 1, -1 do
      local e = L.spawned[i]
      if IsValid(e) and e:GetClass() == "prop_physics" and e:GetPos():Distance(b:GetPos()) < 400 then it = e break end
    end
    it = it or spawnAt(L.ITEMS.ball, L.eye() + fwd * 70)
    if not it or not L.grab(it) then return nil end
    local t0 = CurTime()
    timer.Create("AELinkFiddle", 0.1, 25, function()
      if L.held ~= it or not IsValid(L.bot) then L.holdAt = nil return end
      local k = CurTime() - t0
      if k > 2.3 then
        L.holdAt = nil
        L.drop()
        return
      end
      L.holdAt = L.eye() + fwd * 90 + Vector(0, math.sin(k * 4) * 30, math.cos(k * 3) * 20)
    end)
  end
  P.lastAction = CurTime()
  return kind
end

-- ---------- giro (2 volte al secondo) ----------
local nextTick = 0
function P.tick()
  local now = CurTime()
  if now < nextTick then return end
  local dt = 0.5
  nextTick = now + dt
  for n, r in pairs(P.rival) do
    r.score = r.score - 0.015 * dt
    if r.score <= 0 then P.rival[n] = nil end
  end
  local b = bot()
  if not IsValid(b) or L.mode == "sit" then return end
  -- vendetta
  if P.mischief ~= "off" and P.free() and now - P.lastAction > 4 then
    local rival = P.topRival(1400, 2)
    if rival and ready("prank", P.mischief == "full" and 20 or 40) then
      -- di solito se lo inventa; a volte (o senza app) uno scherzo pronto
      -- l'app decide: un suo classico (zero token) o uno nuovo inventato dal modello
      if P.inventive() and math.random() < 0.85 then P.askPrank(rival) else P.prank(rival) end
      return
    end
  end
  -- gioca da sola
  -- annoiata: ha voglia di fare qualcosa prima
  local bored = L.human and L.human.bored or 0
  if P.idle and P.free() and now >= P.nextIdle and now - P.lastAction > 20 * (1 - 0.6 * bored) then
    -- più imprevedibile = intervalli più irregolari
    P.nextIdle = now + math.random(30, 60) * (1.2 - P.chaos * 0.5) + (math.random() - 0.5) * 30 * P.chaos
    local h = L.host()
    if IsValid(h) and h:GetPos():Distance(b:GetPos()) < 1500 then
      if P.inventive() and math.random() < 0.2 + 0.3 * P.chaos and ready("idea", 180) then
        L.event("idea", {})
        P.lastAction = now
      else
        P.idleAct()
      end
    end
  end
end

function P.config(c)
  if type(c) ~= "table" then return end
  if c.mischief == "off" or c.mischief == "light" or c.mischief == "full" then P.mischief = c.mischief end
  if c.idle ~= nil then P.idle = c.idle == true end
  if c.lang == "it" or c.lang == "en" then P.lang = c.lang end
  if tonumber(c.chaos) then P.chaos = math.Clamp(tonumber(c.chaos), 0, 1) end
  if c.tough == "normal" or c.tough == "high" or c.tough == "tough" then L.tough = c.tough end
  if c.pronouns == "she" or c.pronouns == "he" or c.pronouns == "they" then P.pron = c.pronouns end
  if type(c.quips) == "table" and c.quips_v ~= nil and c.quips_v ~= P.genV then P.setQuips(c.quips, c.quips_v) end
  if L.human then L.human.config(c) end
end

return P
