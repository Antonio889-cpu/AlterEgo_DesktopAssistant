-- Alter Ego Link: come una persona vera, non come un bot.
--   * tempi umani: reagisce in 0,15-0,35 s (prima con l'adrenalina, dopo se annoiata o stanca);
--   * sguardo che gira morbido (a volte va un po' oltre e torna), mira che si assesta in qualche decimo;
--   * percezione da giocatrice: vede davanti a sé e non attraverso i muri, sente entro un raggio;
--   * attenzione corta: guarda chi parla, dà occhiate alle cose che si muovono, si annoia, si stanca;
--   * chat da giocatrice: scrive (con "sta scrivendo…"), minuscolo, a raffiche, qualche errore e "*parola";
--   * sbaglia ogni tanto (presa, lancio, salto, arma) e con la pratica sbaglia meno.
-- Tutto nel gioco: zero token.

local L = AELink
local H = L.human or {}
L.human = H
H.realism = H.realism or 1      -- 0 = come un bot (istantanea, precisa), 1 = umana
H.arousal = 0                   -- adrenalina 0..1 (colpi, minacce, botti)
H.bored = 0                     -- noia 0..1 (sale quando non succede niente)
H.skill = H.skill or {}         -- pratica 0..1 per tipo di gesto (dall'app: persona)
H.chatQ = {}
H.met = {}
H.voice = {}
H.focus = nil
H.gz = { pitch = 0, vy = 0, vp = 0, zeta = 0.85 }

local function rnd(a, b) return a + (b - a) * math.random() end
H.rnd = rnd
--- Intero a caso tra a e b (solo con math.random(), anche nei test che lo sostituiscono).
local function int(a, b) return math.min(b, a + math.floor(math.random() * (b - a + 1))) end
H.int = int
function H.pick(t) return t[int(1, #t)] end

-- ---------- tempi ----------
--- Stanchezza 0..0,6: dopo mezz'ora di partita cresce piano.
function H.fatigue()
  if not H.since then return 0 end
  return math.Clamp(((CurTime() - H.since) / 60 - 30) / 120, 0, 0.6)
end

--- Tempo di reazione in secondi.
function H.reaction()
  if H.realism <= 0 then return 0 end
  local base = 0.15 + 0.2 * (math.random() + math.random()) / 2
  local k = 1 - 0.3 * H.arousal + 0.35 * H.bored + 0.5 * H.fatigue()
  return math.Clamp(base * k, 0.1, 0.7) * H.realism
end

--- Fa `fn` dopo un tempo di reazione (+ `extra`), se lei c'è ancora.
function H.after(fn, extra)
  local d = H.reaction() + (extra or 0)
  if d <= 0 then return fn() end
  timer.Simple(d, function() if IsValid(L.bot) then fn() end end)
end

--- Qualcosa di forte (colpo, minaccia, botto): adrenalina.
function H.excite(v) H.arousal = math.max(H.arousal, math.Clamp(v or 0.5, 0, 1)) H.bored = 0 end
--- Succede qualcosa: passa la noia.
function H.poke() H.bored = math.max(0, H.bored - 0.5) end

-- ---------- percezione ----------
H.SIGHT = 4000                  -- oltre non distingue niente
H.FOV = -0.17                   -- coseno di ~100° dal davanti (con la vista periferica)

--- Niente muri in mezzo (gli oggetti grossi e opachi contano come muri).
function H.clear(from, to, ent)
  local tr = util.TraceLine({ start = from, endpos = to, filter = { L.bot, ent }, mask = MASK_OPAQUE or MASK_SOLID_BRUSHONLY })
  if not tr then return true end
  return (tr.Fraction or 1) >= 0.97 or (ent ~= nil and tr.Entity == ent)
end

local function where(x)
  if isvector(x) then return x end
  if not IsValid(x) then return nil end
  if x:IsPlayer() then return x:EyePos() end
  return x:WorldSpaceCenter()
end
H.where = where

--- Verso dove guarda (lo sguardo, se lo segue; se no il corpo).
function H.facing()
  if H.gz.yaw and L.kind ~= "lambda" then
    local r = math.rad(H.gz.yaw)
    return Vector(math.cos(r), math.sin(r), 0)
  end
  return L.bot:GetForward()
end

--- Lo vede: davanti a sé (vista larga), non troppo lontano, niente muri in mezzo.
function H.sees(x)
  if H.realism <= 0 then return true end
  local b = L.bot
  local p = where(x)
  if not IsValid(b) or not p then return false end
  local eye = L.eye()
  local d = p - eye
  local len = d:Length()
  if len < 80 then return true end
  if len > H.SIGHT then return false end
  local f = H.facing()
  local flat = math.sqrt(d.x * d.x + d.y * d.y)
  if flat > 1 and (d.x * f.x + d.y * f.y) / flat < H.FOV then return false end
  return H.clear(eye, p, not isvector(x) and x or nil)
end

--- Lo sente: entro `radius`; attraverso i muri solo a metà distanza.
function H.hears(pos, radius)
  if H.realism <= 0 then return true end
  if not IsValid(L.bot) or not pos then return false end
  local eye = L.eye()
  local d = pos:Distance(eye)
  if d > radius then return false end
  if d <= radius * 0.5 then return true end
  return H.clear(eye, pos)
end

--- Ne sa qualcosa adesso: lo tocca quasi, lo vede, o (se si muove/fa rumore) lo sente.
function H.perceives(e, hearR)
  if H.realism <= 0 then return true end
  if not IsValid(e) or not IsValid(L.bot) then return false end
  if e:GetPos():Distance(L.bot:GetPos()) < 150 then return true end
  if H.sees(e) then return true end
  if not hearR then return false end
  local v = e.GetVelocity and e:GetVelocity():Length() or 0
  local noisy = v > 100 or (e:IsPlayer() and H.talking(e))
  return noisy and H.hears(e:GetPos(), hearR) or false
end

--- Un giocatore sta parlando in voce (dal client, o dal motore se lo sa).
function H.talking(p)
  if not IsValid(p) or not p:IsPlayer() then return false end
  if p.AEVoice then return true end
  if p.IsSpeaking then
    local ok, v = pcall(p.IsSpeaking, p)
    return ok and v == true
  end
  return false
end

function H.anyTalking()
  for _, p in ipairs(player.GetAll()) do
    if H.talking(p) then return true end
  end
  return false
end

-- ---------- sguardo ----------
--- Cosa guardare: chi ha più priorità vince finché non scade (0 = sguardo di riposo, 1 = occhiata,
-- 2 = qualcuno/qualcosa di preciso, 3 = ordine o pericolo). Ritorna false se ha di meglio da guardare.
function H.look(x, prio, secs)
  prio = prio or 2
  local now = CurTime()
  local f = H.focus
  if f and f.untilT > now and f.prio > prio then return false end
  local same = f and f.what == x
  H.focus = { what = x, prio = prio, untilT = now + (secs or (prio == 0 and 1 or 2.5)), since = same and f.since or now }
  if not same then
    -- ogni tanto lo sguardo va un po' oltre e torna indietro
    H.gz.zeta = math.random() < 0.25 * H.realism and 0.55 or 0.85
  end
  return true
end

--- Ha qualcosa di preciso da guardare (il corpo Lambda non deve girarsi da solo verso chi ospita).
function H.busyGaze()
  local f = H.focus
  return f ~= nil and f.prio >= 1 and f.untilT > CurTime()
end

--- Molla e smorzamento sugli angoli: niente scatti, ~0,3 s per girarsi.
local W = 11
function H.gazeStep(ty, tp, dt)
  local g = H.gz
  if not g.yaw or H.realism <= 0 then
    g.yaw, g.pitch, g.vy, g.vp = ty, tp, 0, 0
    return
  end
  local steps = math.max(1, math.ceil(dt / 0.01))
  local h = dt / steps
  for _ = 1, steps do
    local ey = math.NormalizeAngle(ty - g.yaw)
    local ep = tp - g.pitch
    g.vy = g.vy + (W * W * ey - 2 * g.zeta * W * g.vy) * h
    g.vp = g.vp + (W * W * ep - 2 * g.zeta * W * g.vp) * h
    g.yaw = math.NormalizeAngle(g.yaw + g.vy * h)
    g.pitch = g.pitch + g.vp * h
  end
end

local lastGaze = 0
function H.gazeTick()
  local b = L.bot
  if not IsValid(b) or L.kind == "lambda" then return end
  local now = CurTime()
  local dt = now - lastGaze
  if dt < 0.03 then return end
  lastGaze = now
  dt = math.min(dt, 0.2)
  local f = H.focus
  if not f then return end
  local p = where(f.what)
  if not p then H.focus = nil return end
  local eye = L.eye()
  local d = p - eye
  local flat = math.sqrt(d.x * d.x + d.y * d.y)
  if not H.gz.yaw then H.gz.yaw = b:GetAngles().y end
  H.gazeStep(math.deg(math.atan2(d.y, d.x)), -math.deg(math.atan2(d.z, math.max(flat, 1))), dt)
  local cy, cp = math.rad(H.gz.yaw), math.rad(H.gz.pitch)
  local dist = math.max(d:Length(), 50)
  local at = eye + Vector(math.cos(cy) * math.cos(cp), math.sin(cy) * math.cos(cp), -math.sin(cp)) * dist
  pcall(b.SetEyeTarget, b, at)
  -- prima gli occhi e la testa, poi (se si gira tanto) il corpo
  if L.mode ~= "sit" and math.abs(math.NormalizeAngle(H.gz.yaw - b:GetAngles().y)) > 35 then
    pcall(b.SetIdealYawAndUpdate, b, H.gz.yaw)
  end
end

-- ---------- mira ----------
local function gauss() return (math.random() + math.random() + math.random() + math.random() - 2) * 1.73 end

--- Errore di mira (vettore) verso `ent` a distanza `dist`: grande appena lo guarda, poi si assesta.
function H.aimError(ent, dist)
  if H.realism <= 0 then return Vector() end
  local f = H.focus
  local t = (f and f.what == ent) and (CurTime() - f.since) or 0
  local sigma = (0.025 + 0.12 * math.exp(-t / 0.35)) * H.realism * (1 + 0.5 * H.fatigue())
  local r = sigma * (dist or 300)
  return Vector(gauss() * r, gauss() * r, gauss() * r * 0.5)
end

-- ---------- sbagli e pratica ----------
H.BASE = { catch = 0.18, throw = 0.08, jump = 0.15, tool = 0.12, build = 0.2 }

--- Stavolta sbaglia? (più pratica = meno sbagli; stanca = di più)
function H.oops(what)
  local p = (H.BASE[what] or 0.1) * H.realism * (1 - 0.75 * math.Clamp(H.skill[what] or 0, 0, 1)) * (1 + 0.6 * H.fatigue())
  return math.random() > 1 - p
end

--- Com'è andata (l'app impara: pratica e lezioni). Se ha sbagliato reagisce come una persona.
function H.tried(what, ok)
  L.event("try", { what = what, ok = ok })
  if ok then return end
  H.excite(0.3)
  local P = L.play
  if P and P.quip then
    timer.Simple(rnd(0.3, 0.7), function()
      if not IsValid(L.bot) then return end
      P.quip("oops", 0.85)
      if math.random() < 0.5 then L.gesture(math.random() < 0.5 and "shrug" or "laugh") end
    end)
  end
end

-- ---------- chat da giocatrice ----------
H.cps = 11                      -- caratteri al secondo (scrive svelta)

--- Minuscolo e senza punto finale, come in chat (le faccine restano).
function H.casual(text)
  local t = tostring(text or ""):gsub("…", "..."):gsub("%s+", " "):gsub("^ ", ""):gsub(" $", "")
  if t:sub(-1) == "." and t:sub(-2) ~= ".." then t = t:sub(1, -2) end
  local all = math.random() < 0.75
  local first = true
  t = t:gsub("%S+", function(w)
    local emote = w:find("[:;=]") ~= nil
    local out = w
    if not emote and (all or first) then out = w:lower() end
    first = false
    return out
  end)
  return t
end

local ROWS = { "qwertyuiop", "asdfghjkl", "zxcvbnm" }
local function neighbor(c)
  for _, r in ipairs(ROWS) do
    local i = r:find(c, 1, true)
    if i then
      local opts = {}
      if i > 1 then opts[#opts + 1] = r:sub(i - 1, i - 1) end
      if i < #r then opts[#opts + 1] = r:sub(i + 1, i + 1) end
      return H.pick(opts)
    end
  end
  return c
end

--- Un errore di battitura in una parola di almeno 4 lettere: testo sbagliato e parola giusta (o nil).
function H.typo(text)
  local words = {}
  for s, w in text:gmatch("()(%a+)") do
    if #w >= 4 then words[#words + 1] = { s, w } end
  end
  if #words == 0 then return nil end
  local pick = H.pick(words)
  local s, w = pick[1], pick[2]
  local i = int(2, #w - 1)
  local kind = int(1, 4)
  local bad
  if kind == 1 then bad = w:sub(1, i - 1) .. w:sub(i + 1, i + 1) .. w:sub(i, i) .. w:sub(i + 2)      -- lettere scambiate
  elseif kind == 2 then bad = w:sub(1, i) .. w:sub(i, i) .. w:sub(i + 1)                            -- doppia
  elseif kind == 3 then bad = w:sub(1, i - 1) .. w:sub(i + 1)                                       -- saltata
  else bad = w:sub(1, i - 1) .. neighbor(w:sub(i, i):lower()) .. w:sub(i + 1) end                  -- tasto vicino
  if bad == w then bad = w:sub(1, i - 1) .. w:sub(i + 1) end
  return text:sub(1, s - 1) .. bad .. text:sub(s + #w), w:lower()
end

--- Divide un messaggio lungo in raffiche (frasi), al massimo 3.
function H.bursts(text)
  text = tostring(text or "")
  if #text <= 45 then return { text } end
  local parts, start = {}, 1
  for e in text:gmatch("[%.!%?]+()%s") do
    parts[#parts + 1] = text:sub(start, e - 1)
    start = e
  end
  parts[#parts + 1] = text:sub(start)
  -- pezzi troppo corti insieme al precedente
  local out = {}
  for _, p in ipairs(parts) do
    p = p:gsub("^%s+", ""):gsub("%s+$", "")
    if out[#out] and (#p < 12 or #out[#out] < 12) then out[#out] = out[#out] .. " " .. p
    elseif p ~= "" then out[#out + 1] = p end
  end
  while #out > 3 do
    out[#out - 1] = out[#out - 1] .. " " .. out[#out]
    out[#out] = nil
  end
  return out
end

--- Secondi per scrivere `n` caratteri.
function H.typeTime(n)
  return math.min(5, n / (H.cps * rnd(0.8, 1.2)) * (1 + 0.3 * H.fatigue()))
end

--- Mette in coda una riga di chat: scrive, poi la manda (a raffiche, con qualche errore corretto).
function H.chat(text)
  text = tostring(text or "")
  if text == "" then return end
  if H.realism <= 0 then return L.sendChat(text) end
  local t = math.max(CurTime(), H.chatFree or 0) + H.reaction()
  for _, part in ipairs(H.bursts(text)) do
    part = H.casual(part)
    local wrong, word
    if math.random() > 1 - 0.07 * H.realism then wrong, word = H.typo(part) end
    t = t + H.typeTime(#(wrong or part))
    H.chatQ[#H.chatQ + 1] = { text = wrong or part, at = t }
    if word then
      t = t + rnd(0.5, 1.2)
      H.chatQ[#H.chatQ + 1] = { text = "*" .. word, at = t }
    end
    t = t + rnd(0.2, 0.5)
  end
  while #H.chatQ > 10 do table.remove(H.chatQ, 1) end
  H.chatFree = t
end

function H.chatTick()
  local now = CurTime()
  while H.chatQ[1] and H.chatQ[1].at <= now do
    L.sendChat(table.remove(H.chatQ, 1).text)
  end
  local typing = H.chatQ[1] ~= nil
  if typing ~= H.typing and IsValid(L.bot) then
    H.typing = typing
    L.bot:SetNWBool("AETyping", typing)
  end
end

-- ---------- abitudini da giocatrice sandbox ----------
--- Accovacciata (saluto "crouch-spam"). Il cittadino non sa accovacciarsi: animazione se c'è.
function H.crouch(on)
  local b = L.bot
  if not IsValid(b) then return end
  if L.kind == "lambda" then
    if b.SetCrouch then pcall(b.SetCrouch, b, on) end
    return
  end
  if on then
    for _, s in ipairs({ "crouchidle", "Crouch_idleD", "crouch_idle_pistol" }) do
      local id = b:LookupSequence(s)
      if id and id >= 0 then b:ResetSequence(id) return end
    end
  else
    pcall(b.SetSchedule, b, SCHED_IDLE_STAND)
  end
end

function H.crouchSpam(n)
  for i = 0, (n or 3) - 1 do
    timer.Simple(i * 0.32, function() if IsValid(L.bot) and L.mode ~= "sit" then H.crouch(true) end end)
    timer.Simple(i * 0.32 + 0.16, function() if IsValid(L.bot) and L.mode ~= "sit" then H.crouch(false) end end)
  end
end

--- Torcia: la accende e spegne come fanno tutti quando non hanno niente da fare.
function H.flashlight(on)
  local b = L.bot
  if not IsValid(b) then return end
  if on == nil then on = not H.flash end
  H.flash = on
  b:SetNWBool("AEFlash", on)
  b:EmitSound("items/flashlight1.wav", 55, 100)
end

--- Ferma a "sfogliare il menu degli oggetti" per qualche secondo (non segue).
function H.paused() return (H.pauseUntil or 0) > CurTime() end

-- ---------- attenzione (4 volte al secondo) ----------
local INTEREST = { prop_physics = true, prop_ragdoll = true }
local function interesting(e)
  if not IsValid(e) or e == L.bot or e == L.held then return false end
  if e:IsPlayer() then return e:Alive() end
  if e:IsNPC() or (e.IsNextBot and e:IsNextBot()) or e:IsVehicle() then return true end
  return INTEREST[e:GetClass()] and (e.GetVelocity and e:GetVelocity():Length() > 120) or false
end

local nextTick = 0
function H.tick()
  local now = CurTime()
  if now < nextTick then return end
  local dt = 0.25
  nextTick = now + dt
  H.arousal = H.arousal * math.exp(-dt / 6)
  H.bored = math.min(1, H.bored + dt / 120)
  local b = L.bot
  if not IsValid(b) then return end
  local P = L.play
  -- voce: si gira verso chi parla (se lo sente) e lo guarda finché parla
  for _, p in ipairs(player.GetAll()) do
    if H.talking(p) and p:GetPos():Distance(b:GetPos()) < 1800 then
      H.poke()
      if not H.voice[p] then
        H.voice[p] = true
        L.dirty = true
        H.after(function() L.look(p, 2, 2) end)
      elseif H.focus and H.focus.what == p then
        H.focus.untilT = now + 1.5
      end
    elseif H.voice[p] then
      H.voice[p] = nil
      L.dirty = true
    end
  end
  if L.mode == "sit" and L.kind ~= "lambda" then return end
  -- qualcuno di nuovo (o che non vedeva dall'inizio): saluto
  if now - (H.joinedAt or now) > 3 then
    for _, p in ipairs(player.GetAll()) do
      local nick = p:Nick()
      if not H.met[nick] and p:Alive() and p:GetPos():Distance(b:GetPos()) < 900 and H.sees(p) then
        H.met[nick] = true
        if now - (H.lastGreet or -100) > 20 then
          H.lastGreet = now
          H.after(function()
            L.look(p, 2, 2.5)
            local r = math.random()
            if r < 0.4 then H.crouchSpam(3) elseif r < 0.8 then L.gesture("wave") end
            if P then P.quip("greet", 0.7) end
          end, 0.2)
        end
      end
    end
  end
  -- occhiate: a quello che si muove, a chi passa (attenzione corta)
  if now >= (H.nextGlance or 0) then
    H.nextGlance = now + rnd(2.5, 7) * (1 - 0.4 * H.arousal + 0.5 * H.fatigue())
    if not H.busyGaze() then
      local cands = {}
      for _, e in ipairs(ents.FindInSphere(b:GetPos(), 1200)) do
        if interesting(e) and e ~= L.target and H.sees(e) then cands[#cands + 1] = e end
      end
      if #cands > 0 then
        local e = H.pick(cands)
        H.after(function() L.look(e, 1, rnd(0.6, 1.4)) end)
      end
    end
  end
  -- noia e stanchezza: se ne accorge anche lei
  if P and P.free and P.free() then
    if H.bored > 0.85 and now - (H.lastBored or -1000) > 180 then
      H.lastBored = now
      P.quip("bored", 0.6)
      P.nextIdle = math.min(P.nextIdle or now, now + rnd(2, 6))
    end
    if H.fatigue() > 0.3 and now - (H.lastTired or (H.since or now)) > 25 * 60 then
      H.lastTired = now
      P.quip("tired", 0.8)
      L.gesture("shrug")
    end
  end
end

--- È entrata nella partita (la stanchezza conta dalla prima volta).
function H.onJoin()
  local now = CurTime()
  H.since = H.since or now
  H.joinedAt = now
  H.bored, H.arousal = 0, 0
  H.focus = nil
  H.gz = { pitch = 0, vy = 0, vp = 0, zeta = 0.85 }
end

--- Un giocatore ha scritto in chat: se lo vede (o è vicino) lo guarda.
function H.onChat(ply)
  H.poke()
  if not IsValid(L.bot) or not IsValid(ply) then return end
  if ply:GetPos():Distance(L.bot:GetPos()) < 600 or H.sees(ply) then
    H.after(function() L.look(ply, 2, rnd(2, 3.5)) end, 0.1)
  end
end

function H.config(c)
  if tonumber(c.realism) then H.realism = math.Clamp(tonumber(c.realism), 0, 1) end
  if type(c.skills) == "table" then
    local s = {}
    for k, v in pairs(c.skills) do if tonumber(v) then s[tostring(k)] = math.Clamp(tonumber(v), 0, 1) end end
    H.skill = s
  end
end

return H
