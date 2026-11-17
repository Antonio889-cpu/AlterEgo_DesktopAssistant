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
