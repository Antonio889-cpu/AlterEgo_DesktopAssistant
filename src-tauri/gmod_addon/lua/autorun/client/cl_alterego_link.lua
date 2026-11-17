-- Alter Ego Link (client): righe di chat, nome sopra la testa, raggio della "physgun".

net.Receive("alterego_chat", function()
  local name = net.ReadString()
  local text = net.ReadString()
  chat.AddText(Color(120, 255, 150), name, Color(235, 235, 235), ": " .. text)
end)

local beam = Material("sprites/physbeama")
local function bots()
  local out = {}
  for _, e in ipairs(ents.FindByClass("npc_citizen")) do
    if e:GetNWBool("AELink") then out[#out + 1] = e end
  end
  return out
end

hook.Add("PostDrawTranslucentRenderables", "AELink", function(depth, sky)
  if sky then return end
  local me = LocalPlayer()
  for _, b in ipairs(bots()) do
    -- raggio verso l'oggetto tenuto
    local held = b:GetNWEntity("AEHeld")
    if IsValid(held) then
      local from = b:EyePos() + b:GetForward() * 14 - Vector(0, 0, 14)
      local to = held:WorldSpaceCenter()
      render.SetMaterial(beam)
      local t = CurTime() * 4
      render.DrawBeam(from, to, 5, t, t + 1, Color(120, 255, 160, 220))
    end
    -- nome sopra la testa
    if IsValid(me) and b:GetPos():DistToSqr(me:GetPos()) < 700 * 700 then
      local pos = b:GetPos() + Vector(0, 0, 82)
      cam.Start3D2D(pos, Angle(0, me:EyeAngles().y - 90, 90), 0.12)
        local talking = b:GetNWFloat("AESpeakUntil", 0) > CurTime()
        draw.SimpleTextOutlined(b:GetNWString("AEName", "Alter Ego") .. (talking and " …" or ""), "DermaLarge", 0, 0, Color(150, 255, 170), TEXT_ALIGN_CENTER, TEXT_ALIGN_CENTER, 2, Color(0, 0, 0, 200))
      cam.End3D2D()
    end
  end
end)

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

list.Set("DesktopWindows", "AlterEgoLink", {
  title = "Alter Ego",
  icon = "icon64/playermodel.png",
  width = 720,
  height = 480,
  onewindow = true,
  init = function(icon, window)
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
    nameLbl:SetText("Nome / Name (Invio)")
    nameLbl:SetDark(true)

    local row = right:Add("DPanel")
    row:Dock(BOTTOM)
    row:SetTall(26)
    row:SetPaintBackground(false)
    local join = row:Add("DButton")
    join:Dock(LEFT)
    join:SetWide(115)
    join:SetText("Entra / Join")
    join.DoClick = function() send("join") end
    local leave = row:Add("DButton")
    leave:Dock(RIGHT)
    leave:SetWide(115)
    leave:SetText("Esci / Leave")
    leave.DoClick = function() send("leave") end

    local mine = right:Add("DButton")
    mine:Dock(BOTTOM)
    mine:SetTall(26)
    mine:SetText("Come il mio / Same as mine")
    mine.DoClick = function()
      local m = LocalPlayer():GetModel()
      show(m)
      send("model", m)
    end
    local reset = right:Add("DButton")
    reset:Dock(BOTTOM)
    reset:SetTall(26)
    reset:SetText("Predefinito / Default")
    reset.DoClick = function() show("") send("model", "") end

    -- elenco dei playermodel installati, con ricerca
    local left = window:Add("DPanel")
    left:Dock(FILL)
    left:DockPadding(6, 6, 6, 6)
    local search = left:Add("DTextEntry")
    search:Dock(TOP)
    search:SetPlaceholderText("Cerca / Search (es. chihiro)")
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
  end,
})
