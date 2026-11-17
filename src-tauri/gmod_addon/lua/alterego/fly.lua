-- Alter Ego Link: sensi e riflessi del "cervello di moscerino".
-- Il cervello (un pezzo del connettoma FlyWire di Drosophila, ~9.500 neuroni) gira nell'app sul PC:
-- qui si misurano i sensi (sense.json, 10 volte al secondo) e si eseguono le sue uscite (fly.json):
-- fuga, retromarcia, grooming, mangiare… Zero token: il modello AI vede solo l'umore che ne resta.

local L = AELink
local F = L.fly or {}
L.fly = F
F.on = F.on or false
F.reflexes = F.reflexes ~= false
F.hud = F.hud or false
F.senses = F.senses or {}
F.out = F.out or {}
F.act = F.act or 0
F.pulses = {}
F.cool = {}

-- ---------- impulsi che svaniscono (dolore, tocco, suoni) ----------
function F.pulse(name, v, secs)
  local now = CurTime()
  local p = F.pulses[name]
  local cur = p and p.v * math.max(0, (p.t - now) / p.d) or 0
  if v >= cur then F.pulses[name] = { v = math.min(1, v), t = now + secs, d = secs } end
end

local function pulseVal(name)
  local p = F.pulses[name]
  if not p then return 0 end
  local r = (p.t - CurTime()) / p.d
  if r <= 0 then F.pulses[name] = nil return 0 end
  return p.v * r
end

local function clamp01(v) return math.Clamp(v, 0, 1) end

local FOOD = { "watermelon", "melon", "banana", "orange", "apple", "cake", "bread", "pizza", "burger", "food" }
local function isFood(e)
  local m = (e.GetModel and e:GetModel() or ""):lower()
  for _, w in ipairs(FOOD) do if m:find(w, 1, true) then return true end end
  return false
end
local function isMelon(e) return (e.GetModel and e:GetModel() or ""):lower():find("melon", 1, true) ~= nil end

local PROJ = { npc_grenade_frag = true, rpg_missile = true, grenade_ar2 = true, crossbow_bolt = true, prop_combine_ball = true }
--- Può "piombarle addosso": oggetti fisici, veicoli, proiettili. Non persone, NPC o lei stessa.
function F.projectile(e)
  if e:IsPlayer() or e:IsNPC() or (e.IsNextBot and e:IsNextBot()) then return false end
  if e:IsVehicle() then return true end
  local c = e:GetClass()
  if PROJ[c] then return true end
  return c:find("^prop_physics") ~= nil or c:find("^prop_ragdoll") ~= nil
end

local function onFire(e) return e.IsOnFire and e:IsOnFire() or false end

--- Lato rispetto a lei: "l" o "r".
local function sideOf(bot, pos)
  local d = pos - bot:GetPos()
  local r = bot.GetRight and bot:GetRight() or Vector(0, -1, 0)
  return (d.x * r.x + d.y * r.y) >= 0 and "r" or "l"
end

-- ---------- sensi ----------
function F.sense()
  local bot = L.bot
  local s = {}
  local want = F.senses
  local bp = bot:GetPos()
  -- dolore: colpi presi, fuoco addosso
  s.pain = pulseVal("pain")
  if onFire(bot) then s.pain = math.max(s.pain, 0.8) end
  -- qualcosa che arriva addosso (looming): tempo all'impatto sotto 1,5 s
  local ll, lr = 0, 0
  F.threat = nil
  F.threatEnt = nil
  local bestT = 0
  local food, foodD = nil, 1e9
  local smellFood, danger, heat = 0, 0, 0
  for _, e in ipairs(ents.FindInSphere(bp, 900)) do
    if e ~= bot and IsValid(e) and e ~= L.held then
      local ep = e:GetPos()
      local rel = bp - ep
      local d = rel:Length()
      -- solo cose lanciate o guidate: un giocatore (o un NPC) che le viene vicino non è una minaccia
      if d < 700 and e.GetVelocity and F.projectile(e) then
        local v = e:GetVelocity()
        local dirv = rel:GetNormalized()
        local closing = v.x * dirv.x + v.y * dirv.y + v.z * dirv.z
        if closing > 120 then
          local ttc = d / closing
          if ttc < 1.5 then
            local size = math.Clamp((e.BoundingRadius and e:BoundingRadius() or 20) / 40, 0.4, 1)
            local val = clamp01((1.5 - ttc) / 1.1) * size
            if sideOf(bot, ep) == "l" then ll = math.max(ll, val) else lr = math.max(lr, val) end
            if val > bestT then bestT = val F.threat = ep F.threatEnt = e end
          end
        end
      end
      if e:GetClass() == "prop_physics" and isFood(e) then
        if isMelon(e) and d < foodD and not (e.IsPlayerHolding and e:IsPlayerHolding()) then food, foodD = e, d end
        smellFood = math.max(smellFood, 1 - d / 900)
      end
      local cls = e:GetClass()
      if onFire(e) or cls == "env_fire" or cls == "entityflame" then
        danger = math.max(danger, 1 - d / 900)
        if d < 300 then heat = math.max(heat, 1 - d / 300) end
      elseif cls == "npc_grenade_frag" or cls == "rpg_missile" or cls == "grenade_ar2" then
        danger = math.max(danger, 1 - d / 900)
      end
    end
  end
  s.loom_l, s.loom_r = ll, lr
  F.food = food
  s.sweet = food and (foodD < 110 and 1 or (foodD < 260 and 0.5 or 0)) or 0
  -- tocco: physgun di qualcuno, "usa", qualcuno attaccato a lei
  s.touch = pulseVal("touch")
  if F.grabbedBy and IsValid(F.grabbedBy) then s.touch = math.max(s.touch, 0.8) end
  for _, p in ipairs(player.GetAll()) do
    if p:GetPos():Distance(bp) < 45 then s.touch = math.max(s.touch, 0.4) end
  end
  -- opzionali
  if want.smell_food then s.smell_food = smellFood end
  if want.smell_danger then s.smell_danger = math.max(danger, pulseVal("blast")) end
  if want.heat then s.heat = onFire(bot) and 1 or heat end
  local wl = bot.WaterLevel and bot:WaterLevel() or 0
  if want.wet then s.wet = wl >= 1 and 1 or 0 end
  if want.cold then s.cold = wl >= 2 and 0.8 or 0 end
  if want.water then s.water = (wl == 1 or wl == 2) and 0.8 or 0 end
  if want.sound then s.sound = pulseVal("sound") end
  if want.light then
    local best = 0
    F.lightPos = nil
    for _, p in ipairs(player.GetAll()) do
      if p.FlashlightIsOn and p:FlashlightIsOn() then
        local d = p:EyePos():Distance(bot:EyePos())
        if d < 900 then
          local to = (bot:EyePos() - p:EyePos()):GetNormalized()
          local aim = p.GetAimVector and p:GetAimVector() or p:GetForward()
          local dot = to.x * aim.x + to.y * aim.y + to.z * aim.z
          if dot > 0.97 then
            local val = clamp01((dot - 0.97) / 0.03) * (1 - d / 1200)
            if val > best then best = val F.lightPos = p:EyePos() end
          end
        end
      end
    end
    s.light = best
  end
  if want.wind then
    local v = bot.GetVelocity and bot:GetVelocity() or Vector()
    s.wind = clamp01((v:Length() - 250) / 500)
  end
  -- solo i valori diversi da zero (file piccolo)
  local out = {}
  for k, v in pairs(s) do
    if v > 0.01 and (want[k] or k == "pain" or k == "loom_l" or k == "loom_r" or k == "sweet" or k == "touch") then
      out[k] = math.Round(v * 100) / 100
    end
  end
  return out
end

-- ---------- riflessi ----------
local function ready(name, secs)
  local now = CurTime()
  if now < (F.cool[name] or 0) then return false end
  F.cool[name] = now + secs
  return true
end

--- Prende il controllo dei piedi per poco, poi torna a quello che stava facendo.
local function override(dest, secs, run)
  if not F.restore then F.restore = { mode = L.mode, goal = L.goal, target = L.target } end
  F.restore.at = CurTime() + secs
  L.mode = "goto"
  L.goal = dest
  L.goTo(dest, run)
end

local function awayFrom(bot, from, dist, o)
  local away = bot:GetPos() - from
  away.z = 0
  if away:Length() < 1 then away = -bot:GetForward() end
  away = away:GetNormalized()
  -- i neuroni di svolta (DNa02) decidono da che parte schivare
  local r = bot.GetRight and bot:GetRight() or Vector(0, -1, 0)
  local turn = ((o.turn_r or 0) - (o.turn_l or 0)) * 0.7
  local dir = (away + r * turn):GetNormalized()
  return bot:GetPos() + dir * dist
end

F.override = override
F.awayFrom = awayFrom

--- Le uscite del cervello diventano spinte: le esprime lo strato "giocatrice" (play.lua).
function F.react()
  local bot = L.bot
  local now = CurTime()
  if F.restore and now >= F.restore.at then
    if L.mode == "goto" then
      L.mode, L.goal, L.target = F.restore.mode, F.restore.goal, F.restore.target
      if L.mode == "goto" and isvector(L.goal) then L.goTo(L.goal, true) end
    end
    F.restore = nil
  end
  if not F.reflexes or not IsValid(bot) then return end
  if L.play then L.play.react(F.out, F) end
end

-- ---------- attività sopra la testa ----------
local SHORT = { escape = "e", backward = "b", groom = "g", feed = "f", drink = "d", listen = "l", squint = "s", brace = "w", turn_l = "tl", turn_r = "tr" }
local function hud(bot)
  if not F.hud or not F.on then
    if bot:GetNWString("AEFly", "") ~= "" then bot:SetNWString("AEFly", "") end
    return
  end
  local parts = { tostring(math.Round(F.act or 0)) }
  for k, s in pairs(SHORT) do
    local v = F.out[k]
    if v and v > 0 then parts[#parts + 1] = s .. "=" .. v end
  end
  table.sort(parts)
  bot:SetNWString("AEFly", table.concat(parts, " "))
end

-- ---------- giro (10 volte al secondo) ----------
local nextTick = 0
function F.tick(botOk)
  local now = CurTime()
  if now < nextTick then return end
  nextTick = now + 0.1
  if not F.on or not botOk then return end
  local bot = L.bot
  file.Write(L.DIR .. "/sense.json", util.TableToJSON({ t = os.time(), s = F.sense() }))
  local raw = file.Read(L.DIR .. "/fly.json", "DATA")
  local data = raw and raw ~= "" and util.JSONToTable(raw)
  if data and math.abs(os.time() - (tonumber(data.t) or 0)) <= 3 then
    F.out = data.o or {}
    F.act = tonumber(data.a) or 0
  else
    F.out = {}
  end
  F.react()
  hud(bot)
end

function F.config(c)
  if type(c) ~= "table" then return end
  F.on = c.on == true
  F.reflexes = c.reflexes ~= false
  F.hud = c.hud == true
  F.senses = {}
  if type(c.senses) == "table" then
    for _, s in ipairs(c.senses) do F.senses[s] = true end
  end
  if not F.on then F.out = {} end
end

-- ---------- eventi che diventano sensi ----------
function F.onDamage(dmg)
  F.pulse("pain", 0.35 + dmg:GetDamage() / 40, 1.2)
  if dmg.IsExplosionDamage and dmg:IsExplosionDamage() then F.pulse("blast", 1, 2) end
end

hook.Add("PhysgunPickup", "AELinkFly", function(ply, e)
  if e == L.bot then F.grabbedBy = ply end
end)
hook.Add("PhysgunDrop", "AELinkFly", function(ply, e)
  if e == L.bot then F.grabbedBy = nil end
end)
hook.Add("PlayerUse", "AELinkFly", function(ply, e)
  if e == L.bot then F.pulse("touch", 0.9, 0.8) end
end)
hook.Add("EntityFireBullets", "AELinkFly", function(e, data)
  if not F.on or not IsValid(L.bot) or e == L.bot or not IsValid(e) then return end
  local d = e:GetPos():Distance(L.bot:GetPos())
  if d < 1500 then
    F.pulse("sound", 1 - d / 2000, 0.5)
    F.soundPos = e:GetPos()
  end
end)

--- Suoni forti vicini (chiamato dall'hook EntityEmitSound dell'addon).
function F.onSound(data)
  if not F.on or not IsValid(L.bot) then return end
  local lvl = data.SoundLevel or 75
  if lvl < 80 then return end
  local pos = data.Pos or (IsValid(data.Entity) and data.Entity:GetPos())
  if not pos then return end
  local d = pos:Distance(L.bot:GetPos())
  if d > 1500 then return end
  F.pulse("sound", clamp01((lvl - 70) / 40) * (1 - d / 2000), 0.5)
  F.soundPos = pos
  if lvl >= 120 and d < 1000 then F.pulse("blast", 1, 2) F.blastPos = pos end
end

return F
