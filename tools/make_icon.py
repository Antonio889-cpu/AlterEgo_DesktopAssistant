"""Genera un'icona provvisoria (laptop verde con faccina). Uso: python tools/make_icon.py out.png"""
import sys
from PIL import Image, ImageDraw

S = 1024
img = Image.new("RGBA", (S, S), (0, 0, 0, 0))
d = ImageDraw.Draw(img)
g, dark, white = (156, 255, 58, 255), (12, 58, 18, 255), (244, 255, 240, 255)
# schermo
d.rounded_rectangle((152, 140, 872, 640), 48, fill=white)
d.rounded_rectangle((192, 180, 832, 600), 28, fill=dark)
# faccina
d.ellipse((370, 300, 430, 380), fill=g)
d.ellipse((594, 300, 654, 380), fill=g)
d.arc((392, 360, 632, 520), 20, 160, fill=g, width=26)
# base
d.polygon([(72, 700), (952, 700), (872, 640), (152, 640)], fill=white)
d.rounded_rectangle((60, 700, 964, 760), 20, fill=white)
img.save(sys.argv[1] if len(sys.argv) > 1 else "icon.png")
