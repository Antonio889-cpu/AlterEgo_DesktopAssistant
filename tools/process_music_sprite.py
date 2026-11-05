# Sprite "Alter Ego con le cuffie" (disegno di Ayari, @xxayariii): toglie sfondo verde e scanline,
# tinge come gli altri sprite (giallo-verde CRT) e lo adatta alla tela 612x483.
# Uso: python tools/process_music_sprite.py originale.jpg src/assets/sprites/music.png (richiede pillow, numpy, scipy)
import sys
from PIL import Image
import numpy as np
from scipy import ndimage
src = Image.open(sys.argv[1]).convert('RGB')
a = np.array(src).astype(float)
r, g, b = a[..., 0], a[..., 1], a[..., 2]
h, w = r.shape
crit = (g - r > 46) & (g > 110)
lab, n = ndimage.label(crit)
sizes = ndimage.sum(crit, lab, range(1, n + 1))
big = np.zeros(n + 1, bool); big[1:] = sizes > 800
bgm = big[lab]
bgm[:18] = True; bgm[:, :430] = True; bgm[:, 1860:] = True; bgm[:300, 1600:] = True; bgm[:, :100] = True; bgm[:, w - 100:] = True; bgm[1402:] = True
fg = ~bgm
fg = ndimage.binary_opening(fg, iterations=2)
lab2, n2 = ndimage.label(fg)
s2 = ndimage.sum(fg, lab2, range(1, n2 + 1))
fg = lab2 == (np.argmax(s2) + 1)
fg = ndimage.binary_dilation(fg, iterations=2)
alpha = ndimage.gaussian_filter(fg.astype(float), 1.0)
L = a @ [.299, .587, .114]
# scanline removal: strong vertical blur, real (very dark) outlines kept sharp
Lb = ndimage.gaussian_filter1d(L, 7, axis=0)
Lb = ndimage.gaussian_filter1d(Lb, 1.2, axis=1)
dark = L < 80
dark = ndimage.binary_dilation(dark, iterations=1)
L = np.where(dark, np.minimum(L, Lb), Lb)
# contrast stretch on the character
v = L[fg]
lo, hi = np.percentile(v, 2), np.percentile(v, 99.5)
L = np.clip((L - lo) / (hi - lo), 0, 1) ** 1.15 * 245
pts = [(0, (20, 15, 0)), (37, (46, 45, 1)), (62, (71, 74, 2)), (87, (98, 101, 2)), (112, (122, 126, 3)), (137, (150, 157, 5)),
       (162, (185, 190, 5)), (187, (211, 205, 14)), (212, (229, 226, 120)), (237, (239, 236, 165)), (255, (248, 246, 200))]
xs = [p[0] for p in pts]
out = np.stack([np.interp(L, xs, [p[1][c] for p in pts]) for c in range(3)], -1)
rgba = np.dstack([out, alpha * 255]).clip(0, 255).astype(np.uint8)
im = Image.fromarray(rgba)
bbox = Image.fromarray((alpha > .5).astype(np.uint8) * 255).getbbox()
print('bbox', bbox)
im = im.crop((bbox[0], bbox[1], bbox[2], bbox[3]))
W, H = 612, 483
s = min(W / im.width, H * 0.995 / im.height)
im = im.resize((round(im.width * s), round(im.height * s)), Image.LANCZOS)
can = Image.new('RGBA', (W, H), (0, 0, 0, 0))
can.paste(im, ((W - im.width) // 2, 0), im)
can.save(sys.argv[2], optimize=True)
print(can.size, im.size)
