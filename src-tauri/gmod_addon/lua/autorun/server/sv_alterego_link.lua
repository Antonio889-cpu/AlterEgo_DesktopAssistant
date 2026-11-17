-- Alter Ego Link (server): Alter Ego entra in Garry's Mod come NPC e gioca con te.
-- Il cervello è l'app di Alter Ego sul PC: questo addon è solo corpo, sensi e mani.
--
-- Comunicazione con l'app: due file in garrysmod/data/alterego/
--   state.json  scritto qui: stato compatto ed eventi (chat, danni, morti, task…)
--   cmd.json    scritto dall'app: comandi numerati (entra, parla, segui, task Lua…)
-- Niente opzioni di avvio, niente moduli: funziona in single player e sui server che ospiti.
-- Installato e aggiornato dall'app (Impostazioni → Voce e audio → Garry's Mod).

if not SERVER then return end

AELink = AELink or {}
local L = AELink
L.VERSION = "1"
L.DIR = "alterego"

local Sandbox = include("alterego/sandbox.lua")
util.AddNetworkString("alterego_chat")
util.AddNetworkString("alterego_menu")

-- ---------- stato ----------
L.bot = L.bot or nil
L.mode = L.mode or "follow"          -- follow | stay | goto | attack | idle
L.target = L.target or nil           -- giocatore da seguire / guardare
L.goal = nil                         -- posizione da raggiungere
L.held = nil                         -- oggetto tenuto con la "physgun"
L.events = L.events or {}
L.seq = L.seq or 0
L.lastCmd = L.lastCmd or -1
L.linkedAt = 0                       -- ultimo battito dell'app (os.time)
L.model = ""                         -- vuoto = cittadina di Half-Life 2
L.name = "Alter Ego"
L.autoJoin = true
L.allowTasks = true
L.spawned = L.spawned or {}          -- oggetti creati da lei (massimo MAX_SPAWNED)
L.speakUntil = 0
L.deadAt = nil

local MAX_SPAWNED = 25
local MAX_EVENTS = 40

-- Oggetti che può creare (nome → modello): niente modelli arbitrari dal modello AI.
L.ITEMS = {
  melon = "models/props_junk/watermelon01.mdl",
  chair = "models/props_c17/FurnitureChair001a.mdl",
  crate = "models/props_junk/wood_crate001a.mdl",
  barrel = "models/props_c17/oildrum001.mdl",
  explosive_barrel = "models/props_c17/oildrum001_explosive.mdl",
  ball = "models/props_phx/misc/soccerball.mdl",
  box = "models/props_junk/cardboard_box001a.mdl",
  tire = "models/props_vehicles/carparts_tire01a.mdl",
  table = "models/props_c17/FurnitureTable001a.mdl",
  couch = "models/props_c17/FurnitureCouch001a.mdl",
  bathtub = "models/props_interiors/BathTub01a.mdl",
  lamp = "models/props_wasteland/prison_lamp001c.mdl",
  bucket = "models/props_junk/MetalBucket01a.mdl",
  cone = "models/props_junk/TrafficCone001a.mdl",
}
L.WEAPONS = { pistol = "weapon_pistol", smg = "weapon_smg1", shotgun = "weapon_shotgun", crowbar = "weapon_crowbar", ar2 = "weapon_ar2", crossbow = "weapon_crossbow" }

local function valid(e) return IsValid(e) end
local function botOk() return IsValid(L.bot) and L.bot:Health() > 0 end

-- ---------- eventi per l'app ----------
function L.event(kind, data)
  L.seq = L.seq + 1
  data = data or {}
  data.seq = L.seq
  data.kind = kind
  L.events[#L.events + 1] = data
  while #L.events > MAX_EVENTS do table.remove(L.events, 1) end
  L.dirty = true
end

-- ---------- nomi leggibili ----------
local function friendly(e)
  if not IsValid(e) then return "?" end
  if e:IsPlayer() then return e:Nick() end
  if e == L.bot then return L.name end
  local c = e:GetClass()
  if e:IsNPC() or e:IsNextBot() then
    local n = c:gsub("^npc_", ""):gsub("_", " ")
    return n
  end
  if e:IsVehicle() then return "vehicle" end
  local m = (e:GetModel() or ""):match("([^/]+)%.mdl$")
  if m then
    for k, v in pairs(L.ITEMS) do if v:lower() == (e:GetModel() or ""):lower() then return k end end
    m = m:lower():gsub("^props?_", ""):gsub("%d+%a?$", ""):gsub("_", " ")
    if m ~= "" then return m end
  end
  return c
end
L.friendly = friendly

-- giocatore che ospita (single player o listen server)
local function host()
  for _, p in ipairs(player.GetAll()) do
    if p:IsListenServerHost() then return p end
  end
  return player.GetAll()[1]
end
L.host = host

local function findPlayer(name)
  if IsEntity(name) then return IsValid(name) and name or nil end
  name = tostring(name or ""):lower()
  if name == "" or name == "me" or name == "host" then return host() end
  for _, p in ipairs(player.GetAll()) do
    if p:Nick():lower() == name then return p end
  end
  for _, p in ipairs(player.GetAll()) do
    if p:Nick():lower():find(name, 1, true) then return p end
  end
  return nil
end

local function byId(id)
  id = tonumber(tostring(id or ""):match("%d+"))
  if not id then return nil end
  local e = Entity(id)
  return IsValid(e) and e or nil
end

-- "#123", nome di un giocatore o una parola ("melon", "zombie")
local function resolve(what)
  if IsEntity(what) then return IsValid(what) and what or nil end
  local s = tostring(what or "")
  if s:match("^#?%d+$") then return byId(s) end
  local p = findPlayer(s)
  if p then return p end
  if botOk() then
    local best, bd
    for _, e in ipairs(ents.FindInSphere(L.bot:GetPos(), 3000)) do
      if e ~= L.bot and not e:IsWeapon() and friendly(e):lower():find(s:lower(), 1, true) then
        local d = e:GetPos():DistToSqr(L.bot:GetPos())
        if not bd or d < bd then best, bd = e, d end
      end
    end
    return best
  end
end
L.resolve = resolve

-- ---------- corpo ----------
-- Il corpo che si muove è un npc_citizen (navigazione, armi, animazioni da NPC). Se il modello
-- scelto è un playermodel (senza animazioni da NPC) l'NPC resta invisibile e il playermodel gli
-- viene "incollato" sopra (bonemerge): stesso scheletro ValveBiped, ne copia le animazioni.
local function isPlayerModel(m)
  m = tostring(m or ""):lower()
  if m:find("/player/", 1, true) then return true end
  for _, v in pairs(player_manager and player_manager.AllValidModels() or {}) do
    if v:lower() == m then return true end
  end
  return false
end
L.isPlayerModel = isPlayerModel

function L.applyModel(bot, model)
  if not IsValid(bot) then return end
  if IsValid(bot.AESkin) then bot.AESkin:Remove() end
  bot.AESkin = nil
  if not model or model == "" or not util.IsValidModel(model) then
    bot:SetModel("models/humans/group01/female_01.mdl")
    bot:SetRenderMode(RENDERMODE_NORMAL)
    bot:SetColor(Color(255, 255, 255, 255))
    return
  end
  if not isPlayerModel(model) then
    bot:SetModel(model)
    bot:SetRenderMode(RENDERMODE_NORMAL)
    bot:SetColor(Color(255, 255, 255, 255))
    return
  end
  bot:SetModel("models/humans/group01/female_01.mdl")
  -- invisibile ma "disegnato" (le ossa servono al bonemerge)
  bot:SetRenderMode(RENDERMODE_TRANSALPHA)
  bot:SetColor(Color(255, 255, 255, 0))
  bot:DrawShadow(false)
  local skin = ents.Create("prop_dynamic")
  if not IsValid(skin) then return end
  skin:SetModel(model)
  skin:SetPos(bot:GetPos())
  skin:SetAngles(bot:GetAngles())
  skin:SetParent(bot)
  skin:Spawn()
  skin:AddEffects(EF_BONEMERGE)
  skin:SetNotSolid(true)
  skin:SetNWBool("AESkin", true)
  bot.AESkin = skin
  bot:SetNWEntity("AESkinEnt", skin)
end
local function relate(bot)
  for _, p in ipairs(player.GetAll()) do
    pcall(bot.AddEntityRelationship, bot, p, D_LI, 99)
  end
end

function L.join(model, name)
  local h = host()
  if not IsValid(h) then return end
  if botOk() then return end
  if IsValid(L.bot) then L.bot:Remove() end
  if model and model ~= "" then L.model = model end
  if name and name ~= "" then L.name = name end
  local tr = util.TraceLine({ start = h:EyePos(), endpos = h:EyePos() + h:GetForward() * 120, filter = h })
  local pos = tr.HitPos - h:GetForward() * 20
  local bot = ents.Create("npc_citizen")
  if not IsValid(bot) then return end
  bot:SetPos(pos + Vector(0, 0, 8))
  bot:SetAngles(Angle(0, (h:GetPos() - pos):Angle().y, 0))
  bot:SetKeyValue("citizentype", "4")
  -- non comandabile dal giocatore (+use), non lascia cadere le armi
  bot:SetKeyValue("spawnflags", tostring(1048576 + 8192))
  bot:Spawn()
  bot:Activate()
  L.applyModel(bot, L.model)
  bot:SetName("alterego")
  bot:SetNWString("AEName", L.name)
  bot:SetNWBool("AELink", true)
  bot:SetMaxHealth(100)
  bot:SetHealth(100)
  relate(bot)
  L.bot = bot
  L.mode = "follow"
  L.target = h
  L.deadAt = nil
  local fx = EffectData()
  fx:SetOrigin(bot:GetPos())
  util.Effect("propspawn", fx)
  bot:EmitSound("buttons/combine_button1.wav", 60, 140)
  L.event("joined", {})
end

function L.leave()
  Sandbox.cancel_all("left")
  L.drop()
  if IsValid(L.bot) then
    local fx = EffectData()
    fx:SetOrigin(L.bot:GetPos())
    util.Effect("propspawn", fx)
    L.bot:Remove()
  end
  L.bot = nil
  L.event("left", {})
end

local function stand()
  if botOk() then pcall(L.bot.SetSchedule, L.bot, SCHED_IDLE_STAND) end
end

local function goTo(pos, run)
  if not botOk() then return end
  L.bot:SetLastPosition(pos)
  L.bot:SetSchedule(run and SCHED_FORCED_GO_RUN or SCHED_FORCED_GO)
end

local function face(pos)
  if not botOk() then return end
  local y = (pos - L.bot:GetPos()):Angle().y
  pcall(L.bot.SetIdealYawAndUpdate, L.bot, y)
end

function L.look(e)
  if botOk() and IsValid(e) then
    local p = e:IsPlayer() and e:EyePos() or e:WorldSpaceCenter()
    pcall(L.bot.SetEyeTarget, L.bot, p)
    face(p)
  end
end

function L.jump()
  if botOk() then
    L.bot:SetVelocity(Vector(0, 0, 260))
    L.bot:EmitSound("player/pl_jump" .. math.random(1, 2) .. ".wav", 60, 120)
  end
end

-- gesti: il primo nome che esiste nel modello
local GESTURES = {
  wave = { "g_wave", "Wave", "wave_close", "wave_smg1" },
  point = { "g_point", "g_point_l", "point" },
  shrug = { "g_shrug", "G_shrug" },
  cheer = { "cheer1", "cheer2", "g_cheer" },
  nod = { "g_nod", "nod" },
}
function L.gesture(name)
  if not botOk() then return end
  for _, s in ipairs(GESTURES[name] or { name }) do
    local id = L.bot:LookupSequence(s)
    if id and id >= 0 then
      pcall(L.bot.AddGestureSequence, L.bot, id, true)
      return true
    end
  end
  return false
end

function L.dance()
  if not botOk() then return end
  local bot, n = L.bot, 0
  timer.Create("AELinkDance", 0.35, 12, function()
    if not IsValid(bot) then return end
    n = n + 1
    bot:SetAngles(bot:GetAngles() + Angle(0, 30, 0))
    if n % 3 == 0 then L.jump() end
  end)
end

function L.give(w)
  if not botOk() then return end
  local cls = L.WEAPONS[w] or (tostring(w):match("^weapon_") and w) or "weapon_pistol"
  local cur = L.bot:GetActiveWeapon()
  if IsValid(cur) then cur:Remove() end
  L.bot:Give(cls)
end

function L.attack(e)
  if not botOk() or not IsValid(e) or e == L.bot then return false end
  if not IsValid(L.bot:GetActiveWeapon()) then L.bot:Give("weapon_pistol") end
  pcall(L.bot.AddEntityRelationship, L.bot, e, D_HT, 99)
  L.bot:SetEnemy(e)
  L.bot:UpdateEnemyMemory(e, e:GetPos())
  L.bot:SetSchedule(SCHED_SHOOT_ENEMY_COVER)
  L.mode = "attack"
  L.goal = e
  return true
end

-- "physgun": l'oggetto le resta davanti, si muove con la sua fisica (anche i veicoli)
function L.grab(e)
  if not botOk() or not IsValid(e) or e:IsPlayer() or e == L.bot then return false end
  local ph = e:GetPhysicsObject()
  if not IsValid(ph) or not ph:IsMoveable() then return false end
  L.held = e
  L.bot:SetNWEntity("AEHeld", e)
  ph:Wake()
  L.bot:EmitSound("weapons/physcannon/physcannon_pickup.wav", 55, 120)
  return true
end

function L.drop()
  if IsValid(L.held) then
    local ph = L.held:GetPhysicsObject()
    if IsValid(ph) then ph:SetVelocity(L.held:GetVelocity() * 0.2) end
  end
  L.held = nil
  if IsValid(L.bot) then L.bot:SetNWEntity("AEHeld", NULL) end
end

function L.throw(at)
  local e = L.held
  if not IsValid(e) or not botOk() then return false end
  local dir = L.bot:GetForward()
  if IsValid(at) then dir = (at:WorldSpaceCenter() - e:WorldSpaceCenter()):GetNormalized() end
  L.drop()
  local ph = e:GetPhysicsObject()
  if IsValid(ph) then ph:SetVelocity(dir * 1100 + Vector(0, 0, 180)) end
  L.bot:EmitSound("weapons/physcannon/superphys_launch1.wav", 60, 120)
  return true
end

local function holdTick()
  local e = L.held
  if not IsValid(e) or not botOk() then
    if L.held then L.drop() end
    return
  end
  local ph = e:GetPhysicsObject()
  if not IsValid(ph) then return L.drop() end
  local size = e:BoundingRadius()
  local want = L.bot:EyePos() + L.bot:GetForward() * (60 + size) + Vector(0, 0, 10)
  local d = want - e:WorldSpaceCenter()
  ph:Wake()
  ph:SetVelocity(d * 8)
  ph:AddAngleVelocity(-ph:GetAngleVelocity() * 0.5)
end

function L.spawn(what, n)
  local h = host()
  local mdl = L.ITEMS[tostring(what or ""):lower()]
  if not mdl or not botOk() then return {} end
  local out = {}
  for i = 1, math.Clamp(tonumber(n) or 1, 1, 5) do
    while #L.spawned >= MAX_SPAWNED do
      local old = table.remove(L.spawned, 1)
      if IsValid(old) then old:Remove() end
    end
    local p = ents.Create("prop_physics")
    if IsValid(p) then
      p:SetModel(mdl)
      p:SetPos(L.bot:EyePos() + L.bot:GetForward() * 70 + Vector(0, 0, 25 * i))
      p:Spawn()
      p:Activate()
      if IsValid(h) then
        undo.Create("prop")
        undo.AddEntity(p)
        undo.SetPlayer(h)
        undo.SetCustomUndoText("Undone Alter Ego's " .. what)
        undo.Finish()
        cleanup.Add(h, "props", p)
      end
      L.spawned[#L.spawned + 1] = p
      out[#out + 1] = p
    end
  end
  if #out > 0 then
    local fx = EffectData()
    fx:SetOrigin(out[1]:GetPos())
    util.Effect("propspawn", fx)
    L.bot:EmitSound("ui/buttonclickrelease.wav", 50, 110)
  end
  return out
end

-- ---------- parla ----------
function L.say(text, secs)
  text = tostring(text or ""):sub(1, 400)
  if text == "" then return end
  net.Start("alterego_chat")
  net.WriteString(IsValid(L.bot) and L.bot:GetNWString("AEName", L.name) or L.name)
  net.WriteString(text)
  net.Broadcast()
  L.speakUntil = CurTime() + (tonumber(secs) or math.Clamp(#text * 0.06, 1.5, 12))
  if IsValid(L.bot) then L.bot:SetNWFloat("AESpeakUntil", L.speakUntil) end
end

-- ---------- API dei task (maniglie opache: il task non tocca le entità vere) ----------
local handles = setmetatable({}, { __mode = "k" })
local H = { __tostring = function(h) local e = handles[h] return IsValid(e) and (friendly(e) .. "#" .. e:EntIndex()) or "gone" end }
local function wrap(e)
  if not IsValid(e) then return nil end
  local h = setmetatable({}, H)
  handles[h] = e
  return h
end
local function unwrap(h)
  if type(h) == "table" then return handles[h] end
  return resolve(h)
end
local function topos(x)
  if isvector(x) then return x end
  local e = unwrap(x)
  return IsValid(e) and e:GetPos() or nil
end

local function botPos() return botOk() and L.bot:GetPos() or Vector() end

local function matches(e, what)
  what = tostring(what or ""):lower()
  if what == "" or what == "any" then return true end
  if what == "player" then return e:IsPlayer() end
  if what == "npc" then return e:IsNPC() or e:IsNextBot() end
  if what == "vehicle" then return e:IsVehicle() end
  if what == "prop" then return e:GetClass():find("^prop_physics") ~= nil end
  return friendly(e):lower():find(what, 1, true) ~= nil or e:GetClass():lower():find(what, 1, true) ~= nil
    or (e:GetModel() or ""):lower():find(what, 1, true) ~= nil
end

local function find(what, r)
  local out = {}
  if not botOk() then return out end
  local me = botPos()
  for _, e in ipairs(ents.FindInSphere(me, math.Clamp(tonumber(r) or 1500, 50, 8000))) do
    if e ~= L.bot and not e:IsWeapon() and (e:IsPlayer() or e:IsNPC() or e:IsNextBot() or e:IsVehicle() or e:GetMoveType() == MOVETYPE_VPHYSICS)
        and not (e:IsNPC() and e:Health() <= 0) and matches(e, what) then
      out[#out + 1] = e
    end
  end
  table.sort(out, function(a, b) return a:GetPos():DistToSqr(me) < b:GetPos():DistToSqr(me) end)
  local w = {}
  for i = 1, math.min(#out, 20) do w[i] = wrap(out[i]) end
  return w
end

local lastSay, lastNotify = 0, 0
L.api = {
  vec = function(x, y, z) return Vector(tonumber(x) or 0, tonumber(y) or 0, tonumber(z) or 0) end,
  time = function() return CurTime() end,
  host = function() return wrap(host()) end,
  player = function(n) return wrap(findPlayer(n)) end,
  me = function() return { pos = botPos(), hp = botOk() and L.bot:Health() or 0 } end,
  find = find,
  nearest = function(what, r) return find(what, r)[1] end,
  alive = function(h) local e = unwrap(h) return IsValid(e) and (not (e:IsNPC() or e:IsPlayer()) or e:Health() > 0) end,
  pos = function(h) return topos(h) end,
  dist = function(h) local p = topos(h) return p and p:Distance(botPos()) or math.huge end,
  hp = function(h) local e = unwrap(h) return IsValid(e) and e:Health() or 0 end,
  name = function(h) local e = unwrap(h) return IsValid(e) and friendly(e) or "" end,
  class = function(h) local e = unwrap(h) return IsValid(e) and e:GetClass() or "" end,
  aim = function(h)
    local p = unwrap(h) or host()
    if not IsValid(p) or not p:IsPlayer() then return nil end
    return p:GetEyeTrace().HitPos
  end,
  go = function(target, near, timeout)
    near = tonumber(near) or 90
    L.mode = "goto"
    local ok = Sandbox.until_(function()
      local p = topos(target)
      if not p or not botOk() then return true end
      if p:Distance(botPos()) <= near then return true end
      if not L.bot:IsCurrentSchedule(SCHED_FORCED_GO_RUN) or (L.goal and L.goal:Distance(p) > 60) then
        L.goal = p
        goTo(p, true)
      end
      return false
    end, timeout or 40, 0.3)
    local p = topos(target)
    stand()
    L.mode = "idle"
    return ok and p ~= nil and p:Distance(botPos()) <= near + 40
  end,
  follow = function(h) local e = unwrap(h) if IsValid(e) then L.target = e L.mode = "follow" end end,
  stay = function() L.mode = "stay" stand() end,
  look = function(h) L.look(unwrap(h)) end,
  attack = function(h, timeout)
    local e = unwrap(h)
    if not L.attack(e) then return false end
    local ok = Sandbox.until_(function()
      return not IsValid(e) or e:Health() <= 0 or not botOk()
    end, timeout or 20, 0.3)
    if L.mode == "attack" then L.mode = "idle" end
    return ok
  end,
  shoot = function(h) return L.attack(unwrap(h)) end,
  grab = function(h)
    local e = unwrap(h)
    if not IsValid(e) then return false end
    if e:GetPos():Distance(botPos()) > 260 then
      L.api.go(h, 160, 25)
    end
    return L.grab(e)
  end,
  drop = function() L.drop() end,
  throw = function(h) return L.throw(unwrap(h)) end,
  bring = function(h, to)
    if not L.api.grab(h) then return false end
    local ok = L.api.go(to or wrap(host()), 160, 60)
    L.drop()
    return ok
  end,
  spawn = function(what) return wrap(L.spawn(what, 1)[1]) end,
  say = function(text)
    if CurTime() - lastSay < 3 then Sandbox.sleep(3 - (CurTime() - lastSay)) end
    lastSay = CurTime()
    L.event("say", { text = tostring(text):sub(1, 200) })
  end,
  notify = function(text)
    if CurTime() - lastNotify < 60 then return false end
    lastNotify = CurTime()
    L.event("notify", { task = Sandbox.current and Sandbox.current.name or "", text = tostring(text):sub(1, 200) })
    return true
  end,
  gesture = function(g) return L.gesture(tostring(g)) end,
  jump = function() L.jump() end,
}

local function taskEnded(name, status, err)
  L.event("task", { name = name, status = status, error = err ~= "" and err:sub(1, 200) or nil })
end

-- ---------- comandi dall'app ----------
local OPS = {}
OPS.join = function(c) L.join(c.model, c.name) end
OPS.leave = function() L.leave() end
OPS.say = function(c) L.say(c.text, c.secs) end
OPS.follow = function(c) local p = findPlayer(c.who) or resolve(c.who) if IsValid(p) then L.target = p L.mode = "follow" end end
OPS.stay = function() L.mode = "stay" stand() end
OPS.stop = function() Sandbox.cancel_all("stopped") L.drop() L.mode = "stay" stand() end
OPS.come = function(c)
  local p = findPlayer(c.who) or host()
  if IsValid(p) and botOk() then L.mode = "goto" L.goal = p:GetPos() goTo(L.goal, true) L.target = p end
end
OPS.goto_aim = function(c)
  local p = findPlayer(c.who) or host()
  if IsValid(p) and botOk() then L.mode = "goto" L.goal = p:GetEyeTrace().HitPos goTo(L.goal, true) L.target = p end
end
OPS.look = function(c) L.look(resolve(c.who) or host()) end
OPS.jump = function() L.jump() end
OPS.gesture = function(c) L.gesture(c.name or "wave") end
OPS.dance = function() L.dance() end
OPS.spawn = function(c) L.spawn(c.what, c.n) end
OPS.grab = function(c)
  local e = resolve(c.what)
  if IsValid(e) then
    if e:GetPos():Distance(botPos()) > 260 then
      -- prima ci va (task interno, senza token)
      local api = table.Copy(L.api)
      api.target = wrap(e)
      Sandbox.start("grab", "go(target, 160, 25) grab(target)", api, taskEnded)
    else
      L.grab(e)
    end
  end
end
OPS.drop = function() L.drop() end
OPS.throw = function(c) L.throw(resolve(c.at)) end
OPS.give = function(c) L.give(c.weapon) end
OPS.attack = function(c) L.attack(resolve(c.what)) end
OPS.task = function(c)
  if not L.allowTasks then return L.event("task", { name = c.name, status = "error", error = "scripts disabled" }) end
  local ok, err = Sandbox.start(c.name, c.code, L.api, taskEnded)
  if not ok then
    L.event("task", { name = c.name, status = "error", error = tostring(err):sub(1, 200) })
  else
    L.event("task", { name = c.name, status = "running" })
  end
end
OPS.cancel = function(c)
  if c.name and c.name ~= "" and c.name ~= "all" then Sandbox.cancel(c.name, "cancelled") else Sandbox.cancel_all("cancelled") end
end
OPS.config = function(c)
  -- il modello scelto nel menu del gioco vale più di quello delle impostazioni (finché l'app non si aggiorna)
  if c.model and c.model ~= "" and c.model ~= L.model and not L.menuModel then
    L.model = c.model
    if botOk() then L.applyModel(L.bot, c.model) end
  elseif L.menuModel and c.model == L.menuModel then
    L.menuModel = nil
  end
  if c.name and c.name ~= "" then L.name = c.name end
  if c.auto_join ~= nil then L.autoJoin = c.auto_join end
  if c.tasks ~= nil then L.allowTasks = c.tasks end
end

local function readCommands(initial)
  local raw = file.Read(L.DIR .. "/cmd.json", "DATA")
  if not raw or raw == "" then return end
  local data = util.JSONToTable(raw)
  if not data then return end
  L.linkedAt = tonumber(data.t) or L.linkedAt
  local cmds = data.cmds or {}
  if initial then
    -- comandi rimasti da una partita precedente: non si rieseguono
    for _, c in ipairs(cmds) do L.lastCmd = math.max(L.lastCmd, tonumber(c.id) or 0) end
    if data.config then OPS.config(data.config) end
    return
  end
  if data.config then OPS.config(data.config) end
  for _, c in ipairs(cmds) do
    local id = tonumber(c.id) or 0
    if id > L.lastCmd then
      L.lastCmd = id
      local f = OPS[c.op]
      if f then
        local ok, err = pcall(f, c)
        if not ok then print("[Alter Ego Link] " .. tostring(c.op) .. ": " .. tostring(err)) end
      end
    end
  end
end

function L.linked() return os.time() - (L.linkedAt or 0) < 12 end

-- ---------- stato per l'app ----------
local function dirOf(from, e)
  local d = e:GetPos() - from:GetPos()
  local y = math.NormalizeAngle(d:Angle().y - from:EyeAngles().y)
  if math.abs(y) < 45 then return "front" elseif math.abs(y) > 135 then return "back" elseif y > 0 then return "left" else return "right" end
end

local function snapshot()
  local s = { v = L.VERSION, t = os.time(), map = game.GetMap(), seq = L.seq, events = L.events, tasks = Sandbox.list() }
  local h = host()
  if botOk() then
    local b = L.bot
    local w = b:GetActiveWeapon()
    s.bot = { hp = b:Health(), mode = L.mode, weapon = IsValid(w) and w:GetClass():gsub("^weapon_", "") or nil,
      held = IsValid(L.held) and (friendly(L.held) .. "#" .. L.held:EntIndex()) or nil,
      following = IsValid(L.target) and L.target:IsPlayer() and L.target:Nick() or nil }
    local players = {}
    for _, p in ipairs(player.GetAll()) do
      players[#players + 1] = { name = p:Nick(), host = p == h, hp = p:Alive() and p:Health() or 0,
        d = math.Round(p:GetPos():Distance(b:GetPos()) / 52), dir = dirOf(b, p),
        looking = p:GetEyeTrace().Entity == b }
    end
    s.players = players
    local near = {}
    for _, e in ipairs(ents.FindInSphere(b:GetPos(), 1000)) do
      if e ~= b and not e:IsPlayer() and not e:IsWeapon() and (e:IsNPC() or e:IsNextBot() or e:IsVehicle() or e:GetClass() == "prop_physics") then
        near[#near + 1] = e
      end
    end
    table.sort(near, function(a, c) return a:GetPos():DistToSqr(b:GetPos()) < c:GetPos():DistToSqr(b:GetPos()) end)
    local list = {}
    for i = 1, math.min(#near, 6) do
      local e = near[i]
      local hostile = e:IsNPC() and IsValid(h) and e.Disposition and e:Disposition(h) == D_HT or nil
      list[i] = { name = friendly(e), id = e:EntIndex(), d = math.Round(e:GetPos():Distance(b:GetPos()) / 52), dir = dirOf(b, e), hostile = hostile }
    end
    s.near = list
  elseif IsValid(h) then
    s.players = { { name = h:Nick(), host = true, hp = h:Health() } }
  end
  return s
end

local nextWrite, nextRead = 0, 0
local function bridge()
  local now = CurTime()
  if now >= nextRead then
    nextRead = now + 0.1
    readCommands(false)
  end
  if now >= nextWrite or (L.dirty and now >= nextWrite - 0.75) then
    nextWrite = now + 1
    L.dirty = false
    file.Write(L.DIR .. "/state.json", util.TableToJSON(snapshot()))
  end
end

-- ---------- comportamento continuo (zero token) ----------
local nextBrain = 0
local function brain()
  if not botOk() then return end
  local now = CurTime()
  holdTick()
  -- bocca mentre parla
  local open = 0
  if now < L.speakUntil then open = math.Clamp(math.abs(math.sin(now * 12)) * 0.6 + math.abs(math.sin(now * 20.7)) * 0.3, 0, 1) end
  for _, ent in ipairs({ L.bot, L.bot.AESkin }) do
    if IsValid(ent) then
      local fid = ent:GetFlexIDByName("jaw_drop")
      if fid then ent:SetFlexWeight(fid, open) end
    end
  end
  if now < nextBrain then return end
  nextBrain = now + 0.5
  if #Sandbox.list() > 0 and L.mode == "goto" then return end -- un task la sta muovendo
  if L.mode == "follow" then
    local t = L.target
    if not IsValid(t) or (t:IsPlayer() and not t:Alive()) then t = host() L.target = t end
    if not IsValid(t) then return end
    local d = t:GetPos():Distance(L.bot:GetPos())
    if d > 170 then
      local behind = t:GetPos() - t:GetForward() * 70
      goTo(behind, d > 380)
    elseif L.bot:IsCurrentSchedule(SCHED_FORCED_GO) or L.bot:IsCurrentSchedule(SCHED_FORCED_GO_RUN) then
      stand()
    else
      L.look(t)
    end
  elseif L.mode == "goto" then
    if L.goal and isvector(L.goal) and L.goal:Distance(L.bot:GetPos()) < 90 then
      stand()
      L.mode = "stay"
      if IsValid(L.target) then L.look(L.target) end
    end
  elseif L.mode == "attack" then
    local e = L.goal
    if not IsValid(e) or e:Health() <= 0 then
      L.mode = "follow"
      L.goal = nil
    end
  elseif L.mode == "stay" then
    local h = host()
    if IsValid(h) and h:GetPos():Distance(L.bot:GetPos()) < 500 then L.look(h) end
  end
end

hook.Add("Think", "AELink", function()
  bridge()
  brain()
  Sandbox.tick()
  -- rientra da sola dopo una morte, se l'app è collegata
  if L.deadAt and CurTime() - L.deadAt > 6 and L.linked() then
    L.deadAt = nil
    L.join()
  end
end)

-- ---------- sensi ----------
hook.Add("PlayerSay", "AELink", function(ply, text)
  if not IsValid(ply) then return end
  local d = botOk() and math.Round(ply:GetPos():Distance(L.bot:GetPos()) / 52) or nil
  L.event("chat", { from = ply:Nick(), host = ply == host(), text = tostring(text):sub(1, 300), d = d })
end)

hook.Add("EntityTakeDamage", "AELink", function(target, dmg)
  if target ~= L.bot or not IsValid(target) then return end
  local a = dmg:GetAttacker()
  local now = CurTime()
  if (L.lastHurt or 0) + 4 > now then return end
  L.lastHurt = now
  L.event("hurt", { by = IsValid(a) and friendly(a) or "world", dmg = math.Round(dmg:GetDamage()) })
end)

hook.Add("OnNPCKilled", "AELink", function(npc, attacker)
  if npc == L.bot then
    L.drop()
    L.bot = nil
    L.deadAt = CurTime()
    L.event("died", { by = IsValid(attacker) and friendly(attacker) or "world" })
    return
  end
  if IsValid(L.bot) and npc:GetPos():Distance(L.bot:GetPos()) < 1500 then
    L.event("kill", { who = friendly(npc), by = IsValid(attacker) and friendly(attacker) or "world" })
  end
end)

hook.Add("PlayerDeath", "AELink", function(victim, _, attacker)
  L.event("player_died", { who = victim:Nick(), by = IsValid(attacker) and attacker ~= victim and friendly(attacker) or "self" })
end)

hook.Add("PlayerSpawnedProp", "AELink", function(ply, model)
  local m = model:match("([^/]+)%.mdl$") or model
  L.event("spawned", { who = ply:Nick(), what = m:lower():gsub("%d+%a?$", ""):gsub("_", " ") })
end)
hook.Add("PlayerSpawnedNPC", "AELink", function(ply, npc)
  L.event("spawned", { who = ply:Nick(), what = friendly(npc) })
end)

-- niente frasi da cittadino di Half-Life 2: parla solo con la voce di Alter Ego (dal PC)
hook.Add("EntityEmitSound", "AELink", function(data)
  if IsValid(L.bot) and data.Entity == L.bot and data.SoundName and data.SoundName:find("^vo/") then return false end
end)
hook.Add("PlayerSpawnedVehicle", "AELink", function(ply, v) L.event("spawned", { who = ply:Nick(), what = "vehicle" }) end)

hook.Add("PlayerInitialSpawn", "AELink", function(ply)
  timer.Simple(1, function() if botOk() and IsValid(ply) then relate(L.bot) end end)
end)

hook.Add("PostCleanupMap", "AELink", function()
  L.bot = nil
  L.spawned = {}
  timer.Simple(1, function() if L.linked() and L.autoJoin then L.join() end end)
end)

-- all'avvio della mappa: entra da sola se l'app è aperta
hook.Add("InitPostEntity", "AELink", function()
  file.CreateDir(L.DIR)
  readCommands(true)
end)
timer.Create("AELinkAutoJoin", 3, 0, function()
  if L.autoJoin and not IsValid(L.bot) and not L.deadAt and not L.left and L.linked() and IsValid(host()) then
    L.join()
  end
end)
-- "leave" ferma il rientro automatico fino a un nuovo "join"
local leave, join = L.leave, L.join
L.leave = function(...) L.left = true return leave(...) end
L.join = function(...) L.left = false return join(...) end
OPS.leave = function() L.leave() end
OPS.join = function(c) L.join(c.model, c.name) end

-- menu "Alter Ego" nel menu contestuale (C): modello, nome, entra/esci
net.Receive("alterego_menu", function(_, ply)
  if not IsValid(ply) or not (ply == host() or ply:IsSuperAdmin()) then return end
  local what = net.ReadString()
  local arg = net.ReadString()
  if what == "model" then
    arg = arg:sub(1, 200)
    if arg ~= "" and not util.IsValidModel(arg) then return end
    L.model = arg
    L.menuModel = arg
    if botOk() then L.applyModel(L.bot, arg) end
    L.event("model", { model = arg })
  elseif what == "name" then
    arg = arg:gsub("[%c]", ""):sub(1, 32)
    if arg ~= "" then
      L.name = arg
      if IsValid(L.bot) then L.bot:SetNWString("AEName", arg) end
      L.event("name", { name = arg })
    end
  elseif what == "join" then L.join()
  elseif what == "leave" then L.leave()
  end
end)

concommand.Add("alterego_join", function(ply) if not IsValid(ply) or ply == host() then L.join() end end)
concommand.Add("alterego_leave", function(ply) if not IsValid(ply) or ply == host() then L.leave() end end)

file.CreateDir(L.DIR)
print("[Alter Ego Link] v" .. L.VERSION .. " caricato")
