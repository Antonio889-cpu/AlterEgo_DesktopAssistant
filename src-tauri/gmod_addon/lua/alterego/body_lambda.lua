-- Alter Ego Link: corpo "Lambda Player" (addon Lambda Players di StarFrost, se installato).
-- È un bot con il playermodel e le animazioni da giocatore vero (corsa, salto, armi, physgun).
-- Qui si usa solo la sua API pubblica (stati, hook, MoveToPos…): nessun codice copiato.
-- Le sue decisioni autonome (girare a caso, costruire, attaccare, chat e voce sue) sono spente:
-- comandano l'app di Alter Ego e i suoi task.

local L = AELink
local LB = {}

local CLASS = "npc_lambdaplayer"
local CTL = "AELinkCtl"
local DEFAULT_MODEL = "models/player/group01/female_01.mdl"

function LB.available()
  return scripted_ents and scripted_ents.GetStored(CLASS) ~= nil
end

local function ours(l) return IsValid(l) and l.AELink == true end

-- gesti da giocatore (come quelli del menu "act")
local ACTS = {
  wave = ACT_GMOD_GESTURE_WAVE, point = ACT_SIGNAL_FORWARD, shrug = ACT_GMOD_GESTURE_DISAGREE, nod = ACT_GMOD_GESTURE_AGREE,
  bow = ACT_GMOD_GESTURE_BOW, cheer = ACT_GMOD_TAUNT_CHEER, laugh = ACT_GMOD_TAUNT_LAUGH, salute = ACT_GMOD_TAUNT_SALUTE,
  dance = ACT_GMOD_TAUNT_DANCE, robot = ACT_GMOD_TAUNT_ROBOT, muscle = ACT_GMOD_TAUNT_MUSCLE,
}

function LB.applyModel(bot, model)
  if not IsValid(bot) then return end
  local m = model
  -- un modello da NPC non ha le animazioni da giocatore: si usa quello predefinito
  if not m or m == "" or not util.IsValidModel(m) or not L.isPlayerModel(m) then m = DEFAULT_MODEL end
  bot:SetModel(m)
end

--- Stato "comandato da Alter Ego": gira nel coroutine del Lambda (MoveToPos può aspettare).
-- Movimento fluido: un solo percorso verso l'entità da seguire (Lambda lo aggiorna da solo),
-- isteresi larga tra "partire" e "fermarsi", corsa decisa strada facendo.
local START_FOLLOW, STOP_FOLLOW = 230, 110
-- sta guardando qualcos'altro (chi parla, un'occhiata): non si rigira verso chi segue
local function busy() return L.human ~= nil and L.human.busyGaze() end
function LB.control(self)
  if self ~= L.bot or (self.GetIsDead and self:GetIsDead()) then return end
  local mode = L.mode
  if mode == "sit" then return end
  if mode == "follow" then
    local t = L.target
    if not IsValid(t) or (t:IsPlayer() and not t:Alive()) then t = L.host() L.target = t end
    if not IsValid(t) then return end
    local d = self:GetPos():Distance(t:GetPos())
    if d > START_FOLLOW then
      self:MoveToPos(t, { run = d > 420, tol = 60, update = 0.4, cbTime = 0.2, callback = function(s)
        if L.mode ~= "follow" or L.bot ~= s or not IsValid(t) or L.target ~= t then return false end
        local dd = s:GetPos():Distance(t:GetPos())
        if dd < STOP_FOLLOW then return false end
        if s.SetRun then s:SetRun(dd > 320) end
      end })
    elseif (self.AELookAt or 0) < CurTime() and not busy() then
      self.AELookAt = CurTime() + 1.2
      self:LookTo(t, 1.5)
    end
  elseif mode == "goto" then
    local g = L.goal
    if not isvector(g) then return end
    if self:GetPos():Distance(g) > 50 then
      self:MoveToPos(g, { run = self:GetPos():Distance(g) > 250, tol = 30, update = 0.6, cbTime = 0.2, callback = function(s)
        if L.mode ~= "goto" or L.bot ~= s then return false end
        -- nuova destinazione: si aggiorna il percorso senza ripartire da fermi
        if L.goal ~= g and isvector(L.goal) then g = L.goal s.l_recomputepath = g end
      end })
    end
  elseif mode == "attack" then
    local e = L.goal
    if IsValid(e) and e ~= self then self:AttackTarget(e, true) end
  else
    local h = L.host()
    if IsValid(h) and h:GetPos():Distance(self:GetPos()) < 500 and (self.AELookAt or 0) < CurTime() and not busy() then
      self.AELookAt = CurTime() + 1.2
      self:LookTo(h, 1.5)
    end
  end
end

--- Dopo la nascita (e dopo ogni rinascita): nome, modello, niente chat/voce sue, stato comandato.
function LB.setup(bot)
  bot.AELink = true
  if bot.PreventDefaultComs then bot:PreventDefaultComs(true) end
  if bot.SetLambdaName then bot:SetLambdaName(L.name) end
  LB.applyModel(bot, L.model)
  bot[CTL] = LB.control
  bot:SetNW2String("lambda_state", CTL)
end

function LB.spawn(pos, ang, h)
  local bot = ents.Create(CLASS)
  if not IsValid(bot) then return nil end
  bot:SetPos(pos)
  bot:SetAngles(ang)
  bot.AELink = true
  if IsValid(h) and bot.SetCreator then bot:SetCreator(h) end
  bot:Spawn()
  LB.setup(bot)
  -- alcune impostazioni del Lambda si applicano al primo Think: si ripete dopo un attimo
  timer.Simple(0.2, function() if IsValid(bot) and bot == L.bot then LB.setup(bot) end end)
  return bot
end

-- ---------- azioni ----------
function LB.stand(bot) if bot.CancelMovement then bot:CancelMovement() end end
function LB.goTo(bot, pos) L.goal = pos end
function LB.look(bot, what) if bot.LookTo then bot:LookTo(what, 1.5) end end
function LB.jump(bot) if bot.LambdaJump then bot:LambdaJump(true) end end

function LB.gesture(bot, name)
  local act = ACTS[name]
  if not act then return false end
  local seq = bot:SelectWeightedSequence(act)
  if not seq or seq < 0 then return false end
  bot:AddGestureSequence(seq, true)
  return true
end

function LB.give(bot, w)
  w = tostring(w or ""):gsub("^weapon_", ""):gsub("^smg1$", "smg")
  if bot.SwitchWeapon then bot:SwitchWeapon(w ~= "" and w or "pistol", true) end
end

function LB.attack(bot, e)
  L.mode = "attack"
  L.goal = e
  if bot.AttackTarget then bot:AttackTarget(e, true) end
end

-- ---------- hook di Lambda Players: niente iniziative sue ----------
hook.Add("LambdaOnChangeState", "AELink", function(l, old, new)
  if not ours(l) then return end
  if new == CTL then return end
  if new == "Combat" and L.mode == "attack" then return end
  -- qualsiasi altro stato (girare, costruire, fuggire…) torna al nostro
  if l:GetNW2String("lambda_state", "") ~= CTL then l:SetNW2String("lambda_state", CTL) end
  return true
end)

hook.Add("LambdaCanTarget", "AELink", function(l, ent)
  if not ours(l) then return end
  if L.mode == "attack" and ent == L.goal then return end
  return true -- niente bersagli scelti da lei (e mai i giocatori)
end)

hook.Add("LambdaOnAttackTarget", "AELink", function(l, ent)
  if not ours(l) then return end
  if L.mode == "attack" and ent == L.goal then return end
  return true
end)

local function block(l) if ours(l) then return true end end
hook.Add("LambdaOnUseBuildFunction", "AELink", block)
hook.Add("LambdaOnToolUse", "AELink", block)
hook.Add("LambdaOnPlaySound", "AELink", block) -- la sua voce è quella di Alter Ego sul PC

hook.Add("LambdaOnSetPlayerModel", "AELink", function(l)
  if ours(l) and L.model ~= "" and L.isPlayerModel(L.model) then return L.model end
end)

hook.Add("LambdaOnThink", "AELink", function(l)
  if ours(l) and l == L.bot and l:GetNW2String("lambda_state", "") ~= CTL
      and not (L.mode == "attack" and l:GetNW2String("lambda_state", "") == "Combat") then
    l:SetNW2String("lambda_state", CTL)
  end
end)

hook.Add("LambdaOnKilled", "AELink", function(l, info)
  if l ~= L.bot then return end
  local a = info and info.GetAttacker and info:GetAttacker()
  if L.sit then L.sit.stand(true) end
  if L.play then L.play.onDied(a) end
  L.drop()
  L.deadAt = CurTime()
  L.event("died", { by = IsValid(a) and L.friendly(a) or "world" })
end)

hook.Add("LambdaOnRespawn", "AELink", function(l)
  if l ~= L.bot then return end
  L.deadAt = nil
  L.mode = "follow"
  L.target = L.host()
  LB.setup(l)
end)

return LB
