-- Alter Ego Link: piccoli script Lua scritti dal modello AI ("task") che girano nel gioco
-- senza altri token: "quando vedi delle galline sparale", "porta qui quel veicolo"...
--
-- Sicurezza: ogni task vede SOLO le funzioni dell'API (niente _G, file, http, console,
-- entità vere: solo maniglie opache), ha un tempo massimo e un limite di calcolo tra un
-- wait() e l'altro (un ciclo infinito senza wait viene fermato, il gioco non si blocca).
-- Questo file non usa funzioni di GMod: si prova anche fuori dal gioco (test in Rust).

local S = {}

S.MAX_TASKS = 3
S.MAX_SECONDS = 1800      -- un task dura al massimo 30 minuti
S.BUDGET = 400            -- x1000 istruzioni tra un wait() e l'altro
S.MAX_CODE = 6000         -- caratteri

local loader = function(code, name)
  if CompileString then
    local f = CompileString(code, name, false)
    if type(f) == "string" then return nil, f end
    return f
  end
  return loadstring(code, "=" .. name)
end

-- copia (i task non possono modificare le librerie vere)
local function copy(t)
  local o = {}
  for k, v in pairs(t) do o[k] = v end
  return o
end

local tasks = {}          -- nome -> task
local order = {}          -- nomi in ordine di avvio
S.current = nil           -- task in esecuzione (per le funzioni bloccanti dell'API)

local function remove(name)
  tasks[name] = nil
  for i, n in ipairs(order) do
    if n == name then table.remove(order, i) break end
  end
end

--- Blocca il task corrente per `s` secondi (minimo un tick).
function S.sleep(s)
  local t = S.current
  if not t then error("wait() fuori da un task", 2) end
  t.wake = S.now() + math.max(0, tonumber(s) or 0)
  coroutine.yield()
end

--- Aspetta finché `cond()` è vera (controllata ogni `step` s) o scade `timeout`. true se vera.
function S.until_(cond, timeout, step)
  local deadline = S.now() + (tonumber(timeout) or 30)
  while true do
    if cond() then return true end
    if S.now() >= deadline then return false end
    S.sleep(step or 0.2)
  end
end

S.now = function() return CurTime and CurTime() or os.clock() end

--- Ambiente del task: solo l'API + librerie sicure.
local function make_env(api)
  local env = {
    math = copy(math), string = copy(string), table = copy(table),
    pairs = pairs, ipairs = ipairs, next = next, select = select, type = type,
    tostring = tostring, tonumber = tonumber, unpack = unpack or table.unpack,
    error = error, assert = assert,
  }
  -- niente string.dump / rep giganti
  env.string.dump = nil
  local rep = string.rep
  env.string.rep = function(s, n, sep)
    if (tonumber(n) or 0) * #tostring(s) > 10000 then error("string.rep troppo grande", 2) end
    return rep(s, n, sep)
  end
  for k, v in pairs(api) do env[k] = v end
  env.wait = S.sleep
  return env
end

--- Avvia (o sostituisce) il task `name`. Restituisce true o nil, errore.
function S.start(name, code, api, on_end)
  name = tostring(name or "task"):gsub("[^%w_%-]", ""):sub(1, 24)
  if name == "" then name = "task" end
  if type(code) ~= "string" or #code == 0 then return nil, "empty code" end
  if #code > S.MAX_CODE then return nil, "code too long" end
  if code:find("\27", 1, true) then return nil, "bytecode not allowed" end
  local f, err = loader(code, "task:" .. name)
  if not f then return nil, tostring(err) end
  if tasks[name] then S.cancel(name, "replaced") end
  local n = 0
  for _ in pairs(tasks) do n = n + 1 end
  if n >= S.MAX_TASKS then
    -- il più vecchio lascia il posto
    S.cancel(order[1], "too many tasks")
  end
  setfenv(f, make_env(api))
  -- LuaJIT: il codice compilato non controlla il limite di calcolo, quindi i task restano interpretati
  if jit and jit.off then jit.off(f, true) end
  local t = { name = name, co = coroutine.create(f), started = S.now(), wake = 0, on_end = on_end }
  tasks[name] = t
  order[#order + 1] = name
  return true
end

function S.cancel(name, why)
  local t = tasks[name]
  if not t then return false end
  remove(name)
  if t.on_end then t.on_end(t.name, "cancelled", why or "") end
  return true
end

function S.cancel_all(why)
  for _, n in ipairs(copy(order)) do S.cancel(n, why) end
end

function S.list()
  local out = {}
  for _, n in ipairs(order) do out[#out + 1] = n end
  return out
end

local over = function() error("too much work without wait()", 0) end

--- Un passo per ogni task pronto (chiamato a ogni tick del server).
function S.tick()
  local now = S.now()
  for _, name in ipairs(copy(order)) do
    local t = tasks[name]
    if t and now >= t.wake then
      if now - t.started > S.MAX_SECONDS then
        remove(name)
        if t.on_end then t.on_end(name, "timeout", "") end
      else
        S.current = t
        local count = 0
        debug.sethook(t.co, function()
          count = count + 1
          if count > S.BUDGET then over() end
        end, "", 1000)
        local ok, err = coroutine.resume(t.co)
        debug.sethook(t.co)
        S.current = nil
        if tasks[name] == t then
          if not ok then
            remove(name)
            if t.on_end then t.on_end(name, "error", tostring(err)) end
          elseif coroutine.status(t.co) == "dead" then
            remove(name)
            if t.on_end then t.on_end(name, "done", "") end
          end
        end
      end
    end
  end
end

return S
