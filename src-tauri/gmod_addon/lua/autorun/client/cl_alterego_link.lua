-- Alter Ego Link (client): righe di chat, nome sopra la testa, raggio della "physgun".

net.Receive("alterego_chat", function()
  local name = net.ReadString()
  local text = net.ReadString()
  chat.AddText(Color(120, 255, 150), name, Color(235, 235, 235), ": " .. text)
end)

local beam = Material("sprites/physbeama")
local function bots()
  local out = {}
  for _, c in ipairs({ "npc_citizen", "npc_lambdaplayer" }) do
    for _, e in ipairs(ents.FindByClass(c)) do
      if e:GetNWBool("AELink") then out[#out + 1] = e end
    end
  end
  return out
end

-- testi nella lingua di Garry's Mod (italiano o inglese)
local function T(it, en)
  local cv = GetConVar("gmod_language")
  return (cv and cv:GetString() == "it") and it or en
end

local FLY = {
  { "e", "schivata", "dodge", Color(255, 90, 90) }, { "b", "indietro", "back", Color(255, 160, 80) },
  { "g", "sussulto", "flinch", Color(255, 220, 90) }, { "f", "gioco", "play", Color(120, 255, 120) },
  { "d", "acqua", "water", Color(90, 200, 255) }, { "l", "ascolta", "listen", Color(180, 140, 255) },
  { "s", "abbagliata", "dazzled", Color(255, 255, 200) }, { "w", "in volo", "airborne", Color(160, 220, 220) },
  { "tl", "gira sx", "turn L", Color(200, 200, 200) }, { "tr", "gira dx", "turn R", Color(200, 200, 200) },
}
local function drawFly(b, me, raw)
  local vals, act = {}, 0
  for tok in raw:gmatch("%S+") do
    local k, v = tok:match("^(%a+)=([%d%.]+)$")
    if k then vals[k] = tonumber(v) else act = tonumber(tok) or act end
  end
  local top = b:GetClass() == "npc_lambdaplayer" and 90 or 76
  cam.Start3D2D(b:GetPos() + Vector(0, 0, top), Angle(0, me:EyeAngles().y - 90, 90), 0.08)
    -- scintille: più spike al secondo, più puntini accesi
    local dots = math.min(40, math.floor(math.sqrt(act)))
    for i = 1, 40 do
      local on = i <= dots and math.random() < 0.8
      surface.SetDrawColor(on and Color(140, 255, 170, 230) or Color(40, 60, 50, 140))
      surface.DrawRect(-200 + (i - 1) * 10, -26, 7, 7)
    end
    local x = -200
    for _, f in ipairs(FLY) do
      local v = vals[f[1]] or 0
      if v > 0 then
        surface.SetDrawColor(0, 0, 0, 170)
        surface.DrawRect(x, 0, 76, 40)
        surface.SetDrawColor(f[4].r, f[4].g, f[4].b, 230)
        surface.DrawRect(x + 3, 31, math.floor(70 * v), 6)
        draw.SimpleText(T(f[2], f[3]), "DermaDefault", x + 38, 14, f[4], TEXT_ALIGN_CENTER, TEXT_ALIGN_CENTER)
        x = x + 82
      end
    end
    draw.SimpleTextOutlined(T("cervello di moscerino", "fruit-fly brain") .. "  " .. act .. " spike/s", "DermaDefault", 0, -40, Color(140, 255, 170), TEXT_ALIGN_CENTER, TEXT_ALIGN_CENTER, 1, Color(0, 0, 0, 200))
  cam.End3D2D()
end

--- Mano destra (il raggio parte da lì): arma del Lambda, osso della mano, o all'altezza del petto.
local function handPos(b)
  local w = b.GetWeaponENT and b:GetWeaponENT()
  if IsValid(w) and not w:GetNoDraw() then
    local a = w:GetAttachment(1)
    if a and a.Pos then return a.Pos end
  end
  for _, e in ipairs({ b:GetNWEntity("AESkinEnt"), b }) do
    if IsValid(e) then
      local bone = e:LookupBone("ValveBiped.Bip01_R_Hand")
      if bone then
        local pos = e:GetBonePosition(bone)
        if pos and pos ~= e:GetPos() then return pos end
      end
    end
  end
  return b:GetPos() + Vector(0, 0, 48) + b:GetForward() * 14
end

hook.Add("PostDrawTranslucentRenderables", "AELink", function(depth, sky)
  if sky then return end
  local me = LocalPlayer()
  for _, b in ipairs(bots()) do
    -- raggio verso l'oggetto tenuto
    local held = b:GetNWEntity("AEHeld")
    if IsValid(held) then
      local from = handPos(b)
      local to = held:WorldSpaceCenter()
      render.SetMaterial(beam)
      local t = CurTime() * 4
      render.DrawBeam(from, to, 5, t, t + 1, Color(120, 255, 160, 220))
    end
    -- nome sopra la testa (il corpo Lambda mostra già il suo)
    if b:GetClass() ~= "npc_lambdaplayer" and IsValid(me) and b:GetPos():DistToSqr(me:GetPos()) < 700 * 700 then
      local pos = b:GetPos() + Vector(0, 0, 82)
      cam.Start3D2D(pos, Angle(0, me:EyeAngles().y - 90, 90), 0.12)
        local talking = b:GetNWFloat("AESpeakUntil", 0) > CurTime()
        draw.SimpleTextOutlined(b:GetNWString("AEName", "Alter Ego") .. (talking and " …" or ""), "DermaLarge", 0, 0, Color(150, 255, 170), TEXT_ALIGN_CENTER, TEXT_ALIGN_CENTER, 2, Color(0, 0, 0, 200))
        if b:GetNWBool("AETyping", false) then
          draw.SimpleTextOutlined(T("sta scrivendo…", "typing…"), "DermaDefaultBold", 0, 26, Color(220, 220, 220), TEXT_ALIGN_CENTER, TEXT_ALIGN_CENTER, 1, Color(0, 0, 0, 200))
        end
      cam.End3D2D()
    end
    -- cervello di moscerino: attività dei neuroni sopra la testa (se attivata nell'app)
    local fly = b:GetNWString("AEFly", "")
    if fly ~= "" and IsValid(me) and b:GetPos():DistToSqr(me:GetPos()) < 700 * 700 then
      drawFly(b, me, fly)
    end
  end
end)

-- torcia: luce davanti ai suoi occhi quando la accende
hook.Add("Think", "AELinkFlash", function()
  for _, b in ipairs(bots()) do
    if b:GetNWBool("AEFlash", false) then
      local d = DynamicLight(b:EntIndex())
      if d then
        d.pos = b:GetPos() + Vector(0, 0, 60) + b:GetForward() * 90
        d.r, d.g, d.b = 255, 250, 230
        d.brightness = 2
        d.decay = 1000
        d.size = 320
        d.dietime = CurTime() + 0.2
      end
    end
  end
end)

-- chi parla in voce: lo dice al server (lei si gira e non parla sopra)
local function voice(ply, on)
  if ply ~= LocalPlayer() then return end
  net.Start("alterego_voice")
  net.WriteString(on and "1" or "0")
  net.SendToServer()
end
hook.Add("PlayerStartVoice", "AELink", function(ply) voice(ply, true) end)
hook.Add("PlayerEndVoice", "AELink", function(ply) voice(ply, false) end)

-- ---------- menu "Alter Ego" nel menu contestuale (tieni premuto C) ----------
-- Scegli il suo modello tra i playermodel installati (anteprima), il nome, entra/esci.

local function send(what, arg)
  net.Start("alterego_menu")
  net.WriteString(what)
  net.WriteString(arg or "")
  net.SendToServer()
end

local function currentModel()
  for _, b in ipairs(bots()) do
    local skin = b:GetNWEntity("AESkinEnt")
    if IsValid(skin) then return skin:GetModel() end
    return b:GetModel()
  end
  return ""
end

local function buildMenu(window)
    window:SetTitle("Alter Ego")
    window:SetSizable(true)
    window:SetMinWidth(560)
    window:SetMinHeight(360)

    local right = window:Add("DPanel")
    right:Dock(RIGHT)
    right:SetWide(250)
    right:DockPadding(6, 6, 6, 6)

    local preview = right:Add("DModelPanel")
    preview:Dock(FILL)
    preview:SetFOV(38)
    preview:SetCamPos(Vector(70, 0, 50))
    preview:SetLookAt(Vector(0, 0, 38))
    function preview:LayoutEntity(ent) ent:SetAngles(Angle(0, RealTime() * 30 % 360, 0)) end
    local function show(m) preview:SetModel(m ~= "" and m or "models/humans/group01/female_01.mdl") end
    show(currentModel())

    local name = right:Add("DTextEntry")
    name:Dock(BOTTOM)
    name:SetPlaceholderText("Alter Ego")
    for _, b in ipairs(bots()) do name:SetValue(b:GetNWString("AEName", "Alter Ego")) end
    name.OnEnter = function(self) send("name", self:GetValue()) end
    local nameLbl = right:Add("DLabel")
    nameLbl:Dock(BOTTOM)
    nameLbl:SetText(T("Nome (Invio per salvare)", "Name (Enter to save)"))
    nameLbl:SetDark(true)

    local row = right:Add("DPanel")
    row:Dock(BOTTOM)
    row:SetTall(26)
    row:SetPaintBackground(false)
    local join = row:Add("DButton")
    join:Dock(LEFT)
    join:SetWide(115)
    join:SetText(T("Entra", "Join"))
    join.DoClick = function() send("join") end
    local leave = row:Add("DButton")
    leave:Dock(RIGHT)
    leave:SetWide(115)
    leave:SetText(T("Esci", "Leave"))
    leave.DoClick = function() send("leave") end
    local mon = right:Add("DButton")
    mon:Dock(BOTTOM)
    mon:SetTall(26)
    mon:SetText(T("FruitFly Monitor (sul PC)", "FruitFly Monitor (on the PC)"))
    mon.DoClick = function() send("monitor") end

    -- corpo: automatico (Lambda se c'è), cittadino, Lambda Players
    local body = right:Add("DComboBox")
    body:Dock(BOTTOM)
    body:SetTall(24)
    local hasLambda = scripted_ents.GetStored("npc_lambdaplayer") ~= nil
    body:AddChoice(T("Corpo: automatico", "Body: automatic"), "auto")
    body:AddChoice(T("Corpo: NPC cittadino", "Body: citizen NPC"), "citizen")
    body:AddChoice(T("Corpo: Lambda Player", "Body: Lambda Player") .. (hasLambda and "" or T(" (non installato)", " (not installed)")), "lambda")
    local cur = "auto"
    for _, b in ipairs(bots()) do cur = b:GetClass() == "npc_lambdaplayer" and "lambda" or "citizen" end
    body:ChooseOptionID(cur == "lambda" and 3 or cur == "citizen" and 2 or 1)
    body.OnSelect = function(_, _, _, data) send("body", data) end

    local mine = right:Add("DButton")
    mine:Dock(BOTTOM)
    mine:SetTall(26)
    mine:SetText(T("Come il mio", "Same as mine"))
    mine.DoClick = function()
      local m = LocalPlayer():GetModel()
      show(m)
      send("model", m)
    end
    local reset = right:Add("DButton")
    reset:Dock(BOTTOM)
    reset:SetTall(26)
    reset:SetText(T("Predefinito", "Default"))
    reset.DoClick = function() show("") send("model", "") end

    -- elenco dei playermodel installati, con ricerca
    local left = window:Add("DPanel")
    left:Dock(FILL)
    left:DockPadding(6, 6, 6, 6)
    local search = left:Add("DTextEntry")
    search:Dock(TOP)
    search:SetPlaceholderText(T("Cerca (es. chihiro)", "Search (e.g. chihiro)"))
    local scroll = left:Add("DScrollPanel")
    scroll:Dock(FILL)
    local grid = scroll:Add("DIconLayout")
    grid:Dock(FILL)
    grid:SetSpaceX(4)
    grid:SetSpaceY(4)

    local models = {}
    for n, m in SortedPairs(player_manager.AllValidModels()) do models[#models + 1] = { n, m } end
    local function fill(filter)
      grid:Clear()
      filter = (filter or ""):lower()
      for _, e in ipairs(models) do
        if filter == "" or e[1]:lower():find(filter, 1, true) or e[2]:lower():find(filter, 1, true) then
          local ic = grid:Add("SpawnIcon")
          ic:SetModel(e[2])
          ic:SetSize(64, 64)
          ic:SetTooltip(e[1])
          ic.DoClick = function()
            show(e[2])
            send("model", e[2])
            surface.PlaySound("buttons/button14.wav")
          end
        end
      end
    end
    search.OnChange = function(self) fill(self:GetValue()) end
    fill("")
end

-- 1. icona nel menu contestuale (tieni premuto C)
list.Set("DesktopWindows", "AlterEgoLink", {
  title = "Alter Ego",
  icon = "icon64/playermodel.png",
  width = 720,
  height = 480,
  onewindow = true,
  init = function(icon, window) buildMenu(window) end,
})

-- 2. finestra aperta a mano (barra del menu C, tasto destro su di lei, console "alterego_menu")
local frame
local function openMenu()
  if IsValid(frame) then frame:MakePopup() return end
  frame = vgui.Create("DFrame")
  frame:SetSize(math.min(720, ScrW() - 40), math.min(480, ScrH() - 40))
  frame:Center()
  buildMenu(frame)
  frame:MakePopup()
end
concommand.Add("alterego_menu", openMenu)

-- 3. voce "Alter Ego" nella barra in alto del menu C
hook.Add("PopulateMenuBar", "AELink", function(menubar)
  local m = menubar:AddOrGetMenu("Alter Ego")
  m:AddOption(T("Modello, corpo e nome…", "Model, body and name…"), openMenu):SetIcon("icon16/user_female.png")
  m:AddOption(T("Entra", "Join"), function() send("join") end):SetIcon("icon16/user_add.png")
  m:AddOption(T("Esci", "Leave"), function() send("leave") end):SetIcon("icon16/user_delete.png")
  m:AddOption("FruitFly Monitor", function() send("monitor") end):SetIcon("icon16/monitor.png")
end)

-- 4. tasto destro su di lei (menu C) → Alter Ego…
if properties and properties.Add then
  properties.Add("alterego", {
    MenuLabel = "Alter Ego…",
    MenuIcon = "icon16/user_female.png",
    Order = 1,
    Filter = function(self, ent) return IsValid(ent) and ent:GetNWBool("AELink") end,
    Action = function() openMenu() end,
  })
end
