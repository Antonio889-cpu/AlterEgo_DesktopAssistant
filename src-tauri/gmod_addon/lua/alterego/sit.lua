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
    if pos:Distance(bot:GetPos()) > 90 then
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
  pose(bot, kind)
  if kind == "vehicle" then L.event("ride", { what = L.friendly(e) }) end
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
  -- giù dal veicolo: di lato, non dentro la carrozzeria
  if IsValid(was) and was ~= "ground" and was.IsVehicle and was:IsVehicle() then
    bot:SetPos(was:GetPos() + was:GetForward() * 90 + Vector(0, 0, 10))
  end
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
  elseif IsValid(S.on) then
    pos, ang = S.place(S.on)
  else
    return S.stand()
  end
  bot:SetPos(pos)
  bot:SetAngles(ang)
  if bot.loco and bot.loco.SetVelocity then bot.loco:SetVelocity(Vector()) end
  pose(bot, S.kind)
end

-- passeggera: se sali su un veicolo mentre ti segue, sale con te; se scendi, scende
hook.Add("PlayerEnteredVehicle", "AELinkSit", function(ply, veh)
  if ply ~= L.host() or not IsValid(L.bot) or L.mode ~= "follow" then return end
  if veh:GetClass() == "prop_vehicle_prisoner_pod" and not IsValid(veh:GetParent()) then return end -- una sedia: resta in piedi
  timer.Simple(0.3, function()
    if IsValid(veh) and IsValid(L.bot) and L.mode == "follow" then
      S.sit(veh)
      if L.play and L.play.quip then L.play.quip("ride") end
    end
  end)
end)
hook.Add("PlayerLeaveVehicle", "AELinkSit", function(ply, veh)
  if ply == L.host() and S.on == veh then S.stand() end
end)

return S
