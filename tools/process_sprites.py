"""Elabora il foglio sprite di Alter Ego in PNG trasparenti, tutti con stesso formato e ancoraggio.

Uso:  python tools/process_sprites.py [foglio.png] [cartella_output]
Default: assets/sprites/raw/alter_ego_sheet.png -> src/assets/sprites/

Il foglio originale non viene modificato. Passi:
1. ritaglio dei pannelli 3x3 (il 9° non è Alter Ego: ignorato);
2. sfocatura verticale leggera che toglie le scanline "cotte" nell'immagine
   (le scanline vere le disegna il CSS, a qualunque dimensione);
3. chroma key: lo sfondo verde ha G-R alto (~55), capelli/pelle/contorni no (<20).
   Si rimuove solo il verde collegato ai bordi (flood fill), così gli occhi verdi restano;
4. stesso ritaglio per tutti (bounding box comune) e ingrandimento x3.
Richiede: pip install pillow numpy
"""
import sys
from pathlib import Path

import numpy as np
from PIL import Image

ROOT = Path(__file__).resolve().parent.parent
SHEET = Path(sys.argv[1]) if len(sys.argv) > 1 else ROOT / "assets/sprites/raw/alter_ego_sheet.png"
OUT = Path(sys.argv[2]) if len(sys.argv) > 2 else ROOT / "src/assets/sprites"

# Interno dei pannelli (cornice e glow esclusi), misurato sul foglio 1020x597.
COLS = [(17, 323), (357, 663), (697, 1003)]
ROWS = [(18, 181), (217, 380), (416, 579)]
# ordine: riga per riga. None = pannello da ignorare.
NAMES = ["idle", "happy", "embarrassed", "worried", "thinking", "sad", "surprised", "error", None]

KEY_HARD = 35   # G-R oltre questo = candidato sfondo
KEY_SOFT = (18, 48)  # rampa di trasparenza sul bordo
SCALE = 3


def vblur(a):
    k = np.array([1, 2, 3, 2, 1], float)
    k /= k.sum()
    pad = np.pad(a, ((2, 2), (0, 0), (0, 0)), mode="edge")
    return sum(k[i] * pad[i : i + a.shape[0]] for i in range(5))


def flood_from_border(cand):
    """Pixel candidati collegati al bordo (connettività 4), con numpy puro."""
    reach = np.zeros_like(cand)
    reach[0, :] = cand[0, :]
    reach[:, 0] = cand[:, 0]
    reach[:, -1] = cand[:, -1]
    while True:
        grown = reach.copy()
        grown[1:, :] |= reach[:-1, :]
        grown[:-1, :] |= reach[1:, :]
        grown[:, 1:] |= reach[:, :-1]
        grown[:, :-1] |= reach[:, 1:]
        grown &= cand
        if (grown == reach).all():
            return reach
        reach = grown


def dilate(m, n=1):
    for _ in range(n):
        g = m.copy()
        g[1:, :] |= m[:-1, :]
        g[:-1, :] |= m[1:, :]
        g[:, 1:] |= m[:, :-1]
        g[:, :-1] |= m[:, 1:]
        m = g
    return m


def process(panel):
    rgb = vblur(panel.astype(float))
    gr = rgb[..., 1] - rgb[..., 0]
    bg = flood_from_border(gr > KEY_HARD)
    band = dilate(bg, 2) & ~bg
    alpha = np.ones(gr.shape)
    alpha[bg] = 0
    lo, hi = KEY_SOFT
    alpha[band] = np.clip((hi - gr[band]) / (hi - lo), 0, 1)
    # bordo superiore e laterali: via eventuali residui del glow della cornice
    alpha[:2, :] = 0
    alpha[:, :2] = 0
    alpha[:, -2:] = 0
    # niente alone verde sui bordi semitrasparenti: porta G vicino a R
    spill = (alpha < 1) & (alpha > 0)
    rgb[..., 1][spill] = np.minimum(rgb[..., 1][spill], rgb[..., 0][spill] + 10)
    return np.dstack([np.clip(rgb, 0, 255), alpha * 255]).astype(np.uint8)


def main():
    sheet = np.asarray(Image.open(SHEET).convert("RGB"))
    sprites = {}
    for i, name in enumerate(NAMES):
        if name is None:
            continue
        (x0, x1), (y0, y1) = COLS[i % 3], ROWS[i // 3]
        sprites[name] = process(sheet[y0:y1, x0:x1])
    # bounding box comune: stesso ritaglio = stesso ancoraggio per tutti
    union = np.zeros(next(iter(sprites.values())).shape[:2], bool)
    for s in sprites.values():
        union |= s[..., 3] > 8
    ys, xs = np.where(union)
    box = (xs.min(), ys.min(), xs.max() + 1, ys.max() + 1)
    OUT.mkdir(parents=True, exist_ok=True)
    for name, s in sprites.items():
        img = Image.fromarray(s, "RGBA").crop(box)
        img = img.resize((img.width * SCALE, img.height * SCALE), Image.LANCZOS)
        img.save(OUT / f"{name}.png", optimize=True)
        print(f"{name}.png  {img.width}x{img.height}")


if __name__ == "__main__":
    main()
