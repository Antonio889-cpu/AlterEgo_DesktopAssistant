-- Alter Ego Link: sedersi (sedie, divani, sedili, veicoli come passeggera, per terra).
-- Nessuno dei due corpi sa sedersi da solo: la si mette al posto giusto a ogni tick e si fa
-- girare l'animazione da seduta (quella dei playermodel o quella del cittadino).

local L = AELink
local S = L.sit or {}
L.sit = S

local SEATS = { "chair", "couch", "sofa", "bench", "seat", "stool", "pod" }
local SEQ = {
  vehicle = { "sit", "sit_rollercoaster", "sitchair1", "Sit_Chair" },
  chair = { "sit", "sitchair1", "Sit_Chair", "sitccouch1" },
  ground = { "sit_zen", "Sit_Ground", "sit" },
}

local function seatLike(e)
  if not IsValid(e) then return false end
  if e:IsVehicle() then return true end
  local m = (e:GetModel() or ""):lower()
  for _, w in ipairs(SEATS) do if m:find(w, 1, true) then return true end end
  return false
end
S.seatLike = seatLike

--- Posizione e angolo per sedersi su `e` (o per terra se nil).
function S.place(e)
  local bot = L.bot
  if not IsValid(e) then return bot:GetPos(), Angle(0, bot:GetAngles().y, 0), "ground" end
  if e:IsVehicle() then
    local base = e:GetPos()
    local id = e.LookupAttachment and e:LookupAttachment("vehicle_feet_passenger0") or 0
    if id and id > 0 then
      local a = e:GetAttachment(id)
      if a and a.Pos then base = a.Pos end
    end
    local driver = e.GetDriver and e:GetDriver()
    if e:GetClass() ~= "prop_vehicle_prisoner_pod" or IsValid(driver) then
      -- passeggera: il posto accanto a quello di chi guida (le macchine sono simmetriche)
      local lp = e:WorldToLocal(base)
      if math.abs(lp.x) > 6 then lp.x = -lp.x else lp.x = lp.x + 34 end
      base = e:LocalToWorld(lp)
    end
    -- i veicoli di Source guardano lungo +Y
    return base, Angle(0, e:GetAngles().y + 90, 0), "vehicle"
  end
  local p = e:GetPos()
  return Vector(p.x, p.y, p.z), Angle(0, e:GetAngles().y, 0), "chair"
end

local function pose(bot, kind)
  for _, name in ipairs(SEQ[kind] or SEQ.chair) do
    local id = bot:LookupSequence(name)
    if id and id >= 0 then
      if bot:GetSequence() ~= id then bot:ResetSequence(id) end
      return true
    end
  end
  return false
end

--- Si siede su `e` (entità), o sulla sedia/sedile più vicino, o per terra ("ground").
function S.sit(e)
  local bot = L.bot
  if not IsValid(bot) then return false end
  if e == "ground" then e = nil
  elseif not IsValid(e) then
    local best, bd = nil, 600
    for _, x in ipairs(ents.FindInSphere(bot:GetPos(), 600)) do
      if x ~= bot and seatLike(x) and not (x.GetDriver and IsValid(x:GetDriver()) and x:GetClass() == "prop_vehicle_prisoner_pod") then
        local d = x:GetPos():Distance(bot:GetPos())
        if d < bd then best, bd = x, d end
      end
    end
    e = best
  end
  S.stand(true)
  if e then
    local pos = S.place(e)
    local d = pos:Distance(bot:GetPos())
    -- in macchina si sale "con un salto": il posto è dentro la carrozzeria, a piedi non ci si arriva
    if e.IsVehicle and e:IsVehicle() and d < 700 then return S.attach(e) end
    if d > 90 then
      -- prima ci va a piedi
      S.pending = e
      L.mode = "goto"
      L.goal = pos
      L.goTo(pos, pos:Distance(bot:GetPos()) > 400)
      return true
    end
  end
  S.attach(e)
  return true
end

function S.attach(e)
  local bot = L.bot
  S.pending = nil
  S.on = e or "ground"
  local pos, ang, kind = S.place(e)
  S.kind = kind
  S.groundPos = pos
  S.groundAng = ang
  L.mode = "sit"
  L.goal = nil
  if L.kind == "lambda" then
    bot.l_UpdateAnimations = false
    if bot.CancelMovement then bot:CancelMovement() end
  else
    if SCHED_NPC_FREEZE then pcall(bot.SetSchedule, bot, SCHED_NPC_FREEZE) end
  end
  bot:SetPos(pos)
  bot:SetAngles(ang)
  if kind == "vehicle" then
    -- attaccata al veicolo: si muove con lui senza scatti e non sbatte contro la carrozzeria
    S.oldGroup = bot.GetCollisionGroup and bot:GetCollisionGroup() or nil
    if bot.SetCollisionGroup then bot:SetCollisionGroup(COLLISION_GROUP_IN_VEHICLE or 10) end
    if bot.SetNotSolid then bot:SetNotSolid(true) end
    S.localPos = e:WorldToLocal(pos)
    S.localAng = e.WorldToLocalAngles and e:WorldToLocalAngles(ang) or Angle(0, 90, 0)
    bot:SetParent(e)
    if bot.SetLocalPos then bot:SetLocalPos(S.localPos) bot:SetLocalAngles(S.localAng) end
    S.parented = true
  end
  pose(bot, kind)
  if kind == "vehicle" then L.event("ride", { what = L.friendly(e) }) end
end

local function unparent(bot)
  if not S.parented then return end
  S.parented = nil
  bot:SetParent(nil)
  if bot.SetNotSolid then bot:SetNotSolid(false) end
  if bot.SetCollisionGroup then bot:SetCollisionGroup(S.oldGroup or COLLISION_GROUP_NPC or 9) end
end

--- Si alza (quiet: senza tornare a seguire).
function S.stand(quiet)
  local bot = L.bot
  S.pending = nil
  if not S.on then return end
  local was = S.on
  S.on = nil
  if not IsValid(bot) then return end
  if L.kind == "lambda" then
    bot.l_UpdateAnimations = true
  else
    pcall(bot.SetSchedule, bot, SCHED_IDLE_STAND)
  end
  unparent(bot)
  -- giù dal veicolo: dalla sua parte (accanto alla portiera), non dentro la carrozzeria
  if IsValid(was) and was ~= "ground" and was.IsVehicle and was:IsVehicle() then
    local side = (S.localPos and S.localPos.x or 1) >= 0 and 1 or -1
    bot:SetPos(was:LocalToWorld(Vector(side * 95, 0, 12)))
  end
  S.localPos, S.localAng = nil, nil
  bot:SetAngles(Angle(0, bot:GetAngles().y, 0))
  if not quiet then
    L.mode = "follow"
    L.target = L.host()
  end
end

function S.tick()
  local bot = L.bot
  if not IsValid(bot) then S.on, S.pending = nil, nil return end
  if S.pending then
    if not IsValid(S.pending) then S.pending = nil return end
    local pos = S.place(S.pending)
    if pos:Distance(bot:GetPos()) < 90 then return S.attach(S.pending) end
    -- arrivata altrove o ha cambiato idea
    if L.mode ~= "goto" then S.pending = nil end
    return
  end
  if not S.on then return end
  if L.mode ~= "sit" then return S.stand(true) end
  local pos, ang
  if S.on == "ground" then
    pos, ang = S.groundPos, S.groundAng
  elseif not IsValid(S.on) then
    return S.stand()
  elseif S.parented and bot.SetLocalPos then
    bot:SetLocalPos(S.localPos)
    bot:SetLocalAngles(S.localAng)
  else
    pos, ang = S.place(S.on)
  end
  if pos then
    bot:SetPos(pos)
    bot:SetAngles(ang)
  end
  if bot.loco and bot.loco.SetVelocity then bot.loco:SetVelocity(Vector()) end
  pose(bot, S.kind)
end

-- passeggera: se sali su un veicolo mentre ti segue, sale con te; se scendi, scende
local FREE = { follow = true, stay = true, idle = true }
hook.Add("PlayerEnteredVehicle", "AELinkSit", function(ply, veh)
  if ply ~= L.host() or not IsValid(L.bot) or not FREE[L.mode] then return end
  if veh:GetClass() == "prop_vehicle_prisoner_pod" and not IsValid(veh:GetParent()) then return end -- una sedia: resta in piedi
  if L.sandbox and #L.sandbox.list() > 0 then return end -- sta facendo qualcosa per conto suo
  if veh:GetPos():Distance(L.bot:GetPos()) > 1200 then return end
  timer.Simple(0.5, function()
    if IsValid(veh) and IsValid(L.bot) and FREE[L.mode] then
      S.sit(veh)
      if L.play and L.play.quip then L.play.quip("ride") end
    end
  end)
end)
hook.Add("PlayerLeaveVehicle", "AELinkSit", function(ply, veh)
  if ply == L.host() and S.on == veh then S.stand() end
end)
-- il veicolo sparisce (undo, cleanup) mentre è seduta: giù subito, senza restare attaccata al nulla
hook.Add("EntityRemoved", "AELinkSit", function(e)
  if S.on == e and IsValid(L.bot) then
    unparent(L.bot)
    S.on = nil
    if L.kind == "lambda" then L.bot.l_UpdateAnimations = true end
    L.mode = "follow"
    L.target = L.host()
  end
end)

return S
