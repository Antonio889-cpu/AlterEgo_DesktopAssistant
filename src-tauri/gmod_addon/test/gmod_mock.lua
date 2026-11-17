-- Finta API di Garry's Mod, quanto basta per far girare l'addon fuori dal gioco (test).
-- Non è completa: copre le funzioni usate da sv_alterego_link.lua e sandbox.lua.

SERVER = true
MOCK = { files = {}, hooks = {}, timers = {}, ents = {}, now = 0, net = {}, sounds = {} }

function CurTime() return MOCK.now end
function SysTime() return MOCK.now end

-- ---------- vettori e angoli ----------
local V = {}
V.__index = V
function Vector(x, y, z) return setmetatable({ x = x or 0, y = y or 0, z = z or 0 }, V) end
V.__add = function(a, b) return Vector(a.x + b.x, a.y + b.y, a.z + b.z) end
V.__sub = function(a, b) return Vector(a.x - b.x, a.y - b.y, a.z - b.z) end
V.__mul = function(a, b)
  if type(a) == "number" then a, b = b, a end
  return Vector(a.x * b, a.y * b, a.z * b)
end
V.__unm = function(a) return Vector(-a.x, -a.y, -a.z) end
V.__tostring = function(a) return string.format("%g %g %g", a.x, a.y, a.z) end
function V:Length() return math.sqrt(self.x ^ 2 + self.y ^ 2 + self.z ^ 2) end
function V:Distance(o) return (self - o):Length() end
function V:DistToSqr(o) local d = self - o return d.x ^ 2 + d.y ^ 2 + d.z ^ 2 end
function V:GetNormalized() local l = self:Length() if l == 0 then return Vector() end return self * (1 / l) end
function V:Angle() return Angle(0, math.deg(math.atan2(self.y, self.x)), 0) end
function isvector(v) return getmetatable(v) == V end

local A = {}
A.__index = A
function Angle(p, y, r) return setmetatable({ p = p or 0, y = y or 0, r = r or 0 }, A) end
A.__add = function(a, b) return Angle(a.p + b.p, a.y + b.y, a.r + b.r) end
function A:Forward() local r = math.rad(self.y) return Vector(math.cos(r), math.sin(r), 0) end

-- ---------- entità ----------
NULL = setmetatable({}, { __index = function() return function() end end })
local E = {}
E.__index = E
local nextId = 1
function MOCK.entity(class, props)
  local e = setmetatable({ class = class, pos = Vector(), ang = Angle(), hp = 100, nw = {}, calls = {}, model = "", alive = true }, E)
  for k, v in pairs(props or {}) do e[k] = v end
  e.id = nextId
  nextId = nextId + 1
  MOCK.ents[e.id] = e
  return e
end
function IsValid(e)
  if type(e) ~= "table" then return false end
  if getmetatable(e) == E then return e.alive end
  return type(e.IsValid) == "function" and e:IsValid() or false
end
function IsEntity(e) return type(e) == "table" and getmetatable(e) == E end
function Entity(i) return MOCK.ents[i] or NULL end
local function rec(name) return function(self, ...) self.calls[#self.calls + 1] = { name, ... } end end
-- movimento finto: con SCHED_FORCED_GO* l'NPC arriva subito all'ultima posizione
function E:SetLastPosition(p) self.lastpos = p end
function E:SetSchedule(s)
  self.calls[#self.calls + 1] = { "SetSchedule", s }
  if (s == SCHED_FORCED_GO or s == SCHED_FORCED_GO_RUN) and self.lastpos and MOCK.walk then self.pos = self.lastpos end
end
for _, n in ipairs({ "Activate", "SetKeyValue", "SetName", "SetMaxHealth",
  "SetIdealYawAndUpdate", "SetEyeTarget", "AddEntityRelationship", "SetEnemy", "UpdateEnemyMemory", "AddGestureSequence",
  "SetFlexWeight", "EmitSound", "SetRenderMode", "SetColor", "DrawShadow", "AddEffects", "SetNotSolid" }) do E[n] = rec(n) end
function E:SetParent(p) self.parent = p end
function E:SetVelocity(v) self.vel = v end
function E:Spawn() self.spawned = true end
function E:Remove() self.alive = false MOCK.ents[self.id] = nil end
function E:SetPos(p) self.pos = p end
function E:GetPos() return self.pos end
function E:SetAngles(a) self.ang = a end
function E:GetAngles() return self.ang end
function E:EyeAngles() return self.ang end
function E:GetForward() return self.ang:Forward() end
function E:EyePos() return self.pos + Vector(0, 0, 64) end
function E:WorldSpaceCenter() return self.pos + Vector(0, 0, 20) end
function E:SetModel(m) self.model = m end
function E:GetModel() return self.model end
function E:GetClass() return self.class end
function E:Health() return self.hp end
function E:SetHealth(h) self.hp = h end
function E:EntIndex() return self.id end
function E:IsPlayer() return self.class == "player" end
function E:IsNPC() return self.class:find("^npc_") ~= nil end
function E:IsNextBot() return false end
function E:IsVehicle() return self.class:find("^prop_vehicle") ~= nil end
function E:IsWeapon() return self.class:find("^weapon_") ~= nil end
function E:GetMoveType() return self.class == "prop_physics" and MOVETYPE_VPHYSICS or 0 end
function E:BoundingRadius() return 20 end
function E:GetVelocity() return self.vel or Vector() end
function E:GetRight() local r = math.rad(self.ang.y) return Vector(math.sin(r), -math.cos(r), 0) end
function E:WaterLevel() return self.water or 0 end
function E:IsOnFire() return self.fire == true end
function E:GetMaxHealth() return 100 end
function E:FlashlightIsOn() return self.flashlight == true end
function E:GetAimVector() return self.aim or self.ang:Forward() end
function E:SetNWString(k, v) self.nw[k] = v end
function E:GetNWString(k, d) return self.nw[k] or d end
E.SetNWBool, E.SetNWFloat, E.SetNWEntity = E.SetNWString, E.SetNWString, E.SetNWString
function E:LookupSequence(s) return (s == "g_wave" or s == "cheer1") and 5 or -1 end
function E:GetFlexIDByName(n) return n == "jaw_drop" and 1 or nil end
function E:IsCurrentSchedule(s) local c = self.calls for i = #c, 1, -1 do if c[i][1] == "SetSchedule" then return c[i][2] == s end end return false end
function E:Give(c) self.weapon = MOCK.entity(c) end
function E:GetActiveWeapon() return self.weapon or NULL end
function E:Disposition() return self.hostile and D_HT or D_LI end
function E:GetPhysicsObject()
  local e = self
  if e.class ~= "prop_physics" and not e:IsVehicle() then return NULL end
  e.phys = e.phys or {
    IsValid = function() return true end, IsMoveable = function() return true end, Wake = function() end,
    SetVelocity = function(_, v) e.physvel = v end, AddAngleVelocity = function() end, GetAngleVelocity = function() return Vector() end,
  }
  return e.phys
end
-- giocatori
function E:Nick() return self.name end
function E:IsListenServerHost() return self.host end
function E:IsSuperAdmin() return false end
function E:Alive() return self.hp > 0 end
function E:GetEyeTrace() return { HitPos = self.pos + Vector(300, 0, 0), Entity = self.lookAt } end

-- ---------- librerie ----------
ents = {
  Create = function(c) return MOCK.entity(c) end,
  FindInSphere = function(p, r)
    local out = {}
    for _, e in pairs(MOCK.ents) do if e.alive and e.pos:Distance(p) <= r then out[#out + 1] = e end end
    table.sort(out, function(a, b) return a.id < b.id end)
    return out
  end,
  FindByClass = function(c) local o = {} for _, e in pairs(MOCK.ents) do if e.class == c then o[#o + 1] = e end end return o end,
}
player = { GetAll = function() local o = {} for _, e in pairs(MOCK.ents) do if e.class == "player" and e.alive then o[#o + 1] = e end end table.sort(o, function(a, b) return a.id < b.id end) return o end }
-- più hook per evento (id diversi), come in GMod: vince il primo valore restituito
MOCK.hookids = {}
hook = { Add = function(ev, id, f)
  MOCK.hookids[ev] = MOCK.hookids[ev] or {}
  MOCK.hookids[ev][id] = f
  MOCK.hooks[ev] = function(...)
    local r
    for _, g in pairs(MOCK.hookids[ev]) do
      local x = g(...)
      if r == nil and x ~= nil then r = x end
    end
    return r
  end
end }
function MOCK.run(ev, ...) local f = MOCK.hooks[ev] if f then return f(...) end end
timer = {
  Create = function(id, d, n, f) MOCK.timers[id] = { d = d, n = n, f = f, at = MOCK.now + d } end,
  Simple = function(d, f) MOCK.timers[{}] = { d = d, n = 1, f = f, at = MOCK.now + d } end,
}
concommand = { Add = function() end }
net = { Start = function(n) MOCK.netmsg = { n } end, WriteString = function(s) table.insert(MOCK.netmsg, s) end,
  Broadcast = function() MOCK.net[#MOCK.net + 1] = MOCK.netmsg end,
  Receive = function(n, f) MOCK.receivers = MOCK.receivers or {} MOCK.receivers[n] = f end,
  ReadString = function() return table.remove(MOCK.incoming, 1) end }
--- Messaggio dal client (menu) come se l'avesse mandato `ply`.
function MOCK.fromClient(name, ply, ...)
  MOCK.incoming = { ... }
  MOCK.receivers[name](0, ply)
end
function Color(r, g, b, a) return { r = r, g = g, b = b, a = a } end
player_manager = { AllValidModels = function() return { chihiro = "models/dr/chihiro.mdl" } end }
RENDERMODE_NORMAL, RENDERMODE_TRANSALPHA, EF_BONEMERGE = 0, 4, 1
util = {
  AddNetworkString = function() end, Effect = function() end, IsValidModel = function() return true end,
  TraceLine = function(t) return { HitPos = t.endpos } end,
  TableToJSON = function(t) return MOCK.tojson(t) end, JSONToTable = function(s) return MOCK.fromjson(s) end,
}
function EffectData() return { SetOrigin = function() end } end
file = {
  Read = function(p) return MOCK.files[p] end,
  Write = function(p, s) MOCK.files[p] = s end,
  CreateDir = function() end,
}
game = { GetMap = function() return "gm_construct" end }
undo = { Create = function() end, AddEntity = function() end, SetPlayer = function() end, SetCustomUndoText = function() end, Finish = function() end }
cleanup = { Add = function() end }
function math.Clamp(v, a, b) return math.min(math.max(v, a), b) end
function math.Round(v) return math.floor(v + 0.5) end
function math.NormalizeAngle(a) return (a + 180) % 360 - 180 end
function table.Copy(t) local o = {} for k, v in pairs(t) do o[k] = v end return o end
function Material() return {} end
function include(p) return dofile(MOCK.root .. "/lua/" .. p) end
SCHED_IDLE_STAND, SCHED_FORCED_GO, SCHED_FORCED_GO_RUN, SCHED_SHOOT_ENEMY_COVER = 1, 2, 3, 4
D_LI, D_HT, MOVETYPE_VPHYSICS = 3, 1, 6

--- Avanza il tempo di `secs` a passi di `dt`, con Think e timer.
function MOCK.advance(secs, dt)
  dt = dt or 0.05
  local stop = MOCK.now + secs
  while MOCK.now < stop do
    MOCK.now = MOCK.now + dt
    for k, t in pairs(MOCK.timers) do
      if MOCK.now >= t.at then
        t.f()
        t.n = t.n - 1
        if t.n == 0 then MOCK.timers[k] = nil else t.at = t.at + t.d end
      end
    end
    MOCK.run("Think")
  end
end

-- ---------- finto Lambda Players (solo se MOCK.has_lambda = true prima di caricare l'addon) ----------
scripted_ents = { GetStored = function(c) return (MOCK.has_lambda and c == "npc_lambdaplayer") and {} or nil end }
ACT_GMOD_GESTURE_WAVE, ACT_SIGNAL_FORWARD, ACT_GMOD_GESTURE_DISAGREE, ACT_GMOD_GESTURE_AGREE = 101, 102, 103, 104
ACT_GMOD_GESTURE_BOW, ACT_GMOD_TAUNT_CHEER, ACT_GMOD_TAUNT_LAUGH, ACT_GMOD_TAUNT_SALUTE = 105, 106, 107, 108
ACT_GMOD_TAUNT_DANCE, ACT_GMOD_TAUNT_ROBOT, ACT_GMOD_TAUNT_MUSCLE = 109, 110, 111
local function lambdaHook(name, ...)
  local f = MOCK.hooks[name]
  if f then return f(...) end
end
local LM = {}
function LM:SetCreator(p) self.creator = p end
function LM:PreventDefaultComs(b) self.nocoms = b end
function LM:SetLambdaName(n) self.lname = n end
function LM:SetNW2String(k, v) self.nw2 = self.nw2 or {} self.nw2[k] = v end
function LM:GetNW2String(k, d) return (self.nw2 or {})[k] or d end
function LM:GetIsDead() return self.dead == true end
function LM:SetState(st)
  st = st or "Idle"
  if lambdaHook("LambdaOnChangeState", self, self:GetNW2String("lambda_state", "Idle"), st) == true then return end
  self:SetNW2String("lambda_state", st)
end
function LM:MoveToPos(target, opts)
  local p = isvector(target) and target or target:GetPos()
  self.moves = (self.moves or 0) + 1
  if opts and opts.callback and opts.callback(self) == false then return "callback" end
  self.pos = p + Vector(40, 0, 0)
  return "ok"
end
function LM:LookTo(what) self.lookedAt = what end
function LM:CancelMovement() self.cancelled = (self.cancelled or 0) + 1 end
function LM:LambdaJump() self.jumped = true end
function LM:SelectWeightedSequence(act) return act end
function LM:AddGestureSequence(seq) self.gesture = seq end
function LM:SwitchWeapon(w) self.l_Weapon = w end
function LM:AttackTarget(e)
  if lambdaHook("LambdaOnAttackTarget", self, e) == true then return end
  self.enemy = e
  self:SetState("Combat")
end
function LM:Combat() if not IsValid(self.enemy) or self.enemy.hp <= 0 then return true end end
function LM:Idle() self.wandered = true self.pos = self.pos + Vector(500, 500, 0) end
local create = ents.Create
ents.Create = function(c)
  local e = create(c)
  if c == "npc_lambdaplayer" then
    for k, v in pairs(LM) do e[k] = v end
    e.l_Weapon = "none"
    e.Spawn = function(self) self.spawned = true self:SetNW2String("lambda_state", "Idle") end
  end
  return e
end
-- il "RunBehaviour" del Lambda: a ogni tick chiama la funzione del suo stato
local think = MOCK.advance
function MOCK.advance(secs, dt)
  local h = MOCK.hooks.Think
  MOCK.hooks.Think = function(...)
    if h then h(...) end
    for _, e in pairs(MOCK.ents) do
      if e.alive and e.class == "npc_lambdaplayer" and not e.dead then
        local st = e:GetNW2String("lambda_state", "Idle")
        local f = e[st]
        if f then
          local r = f(e)
          if r and e:GetNW2String("lambda_state", "Idle") == st then e:SetState(type(r) == "string" and r or nil) end
        end
        lambdaHook("LambdaOnThink", e)
      end
    end
  end
  think(secs, dt)
  MOCK.hooks.Think = h
end
