"""Disegna la skin Minecraft di Alter Ego (64x64, formato 1.8+): capelli castano chiaro a caschetto,
giacca verde, gonna verde scuro. Pixel art originale, semplice.

    python3 tools/make_mc_skin.py   →  src/assets/minecraft/alterego_skin.png
"""
from pathlib import Path

from PIL import Image

OUT = Path(__file__).resolve().parent.parent / "src" / "assets" / "minecraft" / "alterego_skin.png"

SKIN = (247, 214, 192, 255)
SKIN_D = (226, 186, 162, 255)
HAIR = (196, 152, 98, 255)
HAIR_D = (160, 118, 72, 255)
HAIR_L = (222, 184, 132, 255)
EYE = (92, 120, 60, 255)
EYE_D = (40, 52, 30, 255)
WHITE = (250, 250, 246, 255)
MOUTH = (214, 140, 130, 255)
BLUSH = (240, 176, 168, 255)
JACKET = (62, 122, 58, 255)
JACKET_D = (42, 92, 40, 255)
JACKET_L = (88, 150, 80, 255)
SHIRT = (240, 240, 232, 255)
TIE = (190, 60, 60, 255)
SKIRT = (40, 86, 46, 255)
SKIRT_D = (28, 64, 34, 255)
SOCK = (245, 245, 240, 255)
SHOE = (92, 60, 40, 255)
NONE = (0, 0, 0, 0)


def rect(img, x, y, w, h, c):
    for i in range(x, x + w):
        for j in range(y, y + h):
            img.putpixel((i, j), c)


def face(img, x, y, kind):
    """Una faccia 8x8 della testa: front, back, left, right, top, bottom."""
    if kind == "top":
        rect(img, x, y, 8, 8, HAIR)
        for i in range(8):
            img.putpixel((x + i, y + 3 + (i % 2)), HAIR_L)
        return
    if kind == "bottom":
        rect(img, x, y, 8, 8, SKIN_D)
        return
    if kind == "back":
        rect(img, x, y, 8, 8, HAIR)
        for i in range(0, 8, 2):
            rect(img, x + i, y + 2, 1, 6, HAIR_D)
        return
    rect(img, x, y, 8, 8, SKIN)
    if kind == "front":
        # frangia e ciocche ai lati
        rect(img, x, y, 8, 2, HAIR)
        rect(img, x, y + 2, 1, 6, HAIR)
        rect(img, x + 7, y + 2, 1, 6, HAIR)
        for i in (1, 3, 6):
            img.putpixel((x + i, y + 2), HAIR)
        img.putpixel((x + 2, y + 1), HAIR_L)
        img.putpixel((x + 5, y + 1), HAIR_L)
        # occhi grandi verde oliva
        for ex in (x + 1, x + 5):
            img.putpixel((ex, y + 4), WHITE)
            img.putpixel((ex + 1, y + 4), EYE)
            img.putpixel((ex + 1, y + 3), EYE_D)
        img.putpixel((x + 2, y + 5), BLUSH)
        img.putpixel((x + 5, y + 5), BLUSH)
        rect(img, x + 3, y + 6, 2, 1, MOUTH)
    else:
        # lato: capelli fino in fondo (caschetto)
        rect(img, x, y, 8, 3, HAIR)
        back = x if kind == "right" else x + 4
        rect(img, back, y, 4, 8, HAIR)
        rect(img, back + (0 if kind == "right" else 3), y + 2, 1, 6, HAIR_D)


def limb(img, x, y, top, front, side, back, bottom, kind):
    """Un braccio o una gamba 4x12 (box: lati da 4, profondità 4)."""
    # x, y = angolo in alto a sinistra della striscia (lato destro, fronte, lato sinistro, retro)
    rect(img, x + 4, y - 4, 4, 4, top)
    rect(img, x + 8, y - 4, 4, 4, bottom)
    for i, c in enumerate((side, front, side, back)):
        rect(img, x + i * 4, y, 4, 12, c)
    if kind == "arm":
        for i in range(4):
            rect(img, x + i * 4, y + 9, 4, 3, SKIN)  # mani
            rect(img, x + i * 4, y + 8, 4, 1, JACKET_D)  # polsini
        rect(img, x + 8, y - 4, 4, 4, SKIN)
    else:
        for i in range(4):
            rect(img, x + i * 4, y, 4, 2, SKIRT)  # orlo della gonna
            rect(img, x + i * 4, y + 6, 4, 4, SOCK)
            rect(img, x + i * 4, y + 10, 4, 2, SHOE)
        rect(img, x + 8, y - 4, 4, 4, SHOE)


def main():
    img = Image.new("RGBA", (64, 64), NONE)
    # testa
    face(img, 8, 0, "top")
    face(img, 16, 0, "bottom")
    face(img, 0, 8, "right")
    face(img, 8, 8, "front")
    face(img, 16, 8, "left")
    face(img, 24, 8, "back")
    # cappello (strato sopra): ciuffo che sporge in alto
    rect(img, 40, 8, 8, 1, HAIR_L)
    rect(img, 48, 8, 8, 1, HAIR)
    rect(img, 56, 8, 8, 2, HAIR)
    # corpo: giacca verde aperta, camicia bianca, cravattino, gonna
    rect(img, 20, 16, 8, 4, JACKET)  # sopra
    rect(img, 28, 16, 8, 4, SKIRT_D)  # sotto
    rect(img, 16, 20, 4, 12, JACKET_D)  # lato destro
    rect(img, 20, 20, 8, 12, JACKET)  # fronte
    rect(img, 28, 20, 4, 12, JACKET_D)  # lato sinistro
    rect(img, 32, 20, 8, 12, JACKET)  # retro
    rect(img, 23, 20, 2, 7, SHIRT)
    img.putpixel((23, 21), TIE)
    img.putpixel((24, 21), TIE)
    rect(img, 22, 20, 1, 7, JACKET_L)
    rect(img, 25, 20, 1, 7, JACKET_L)
    for x in (16, 20, 28, 32):
        w = 4 if x in (16, 28) else 8
        rect(img, x, 27, w, 5, SKIRT)
        for i in range(0, w, 2):
            rect(img, x + i, 28, 1, 4, SKIRT_D)  # pieghe
    # braccio destro (44,20) e sinistro (36,52)
    limb(img, 40, 20, JACKET, JACKET, JACKET_D, JACKET_D, SKIN, "arm")
    limb(img, 32, 52, JACKET, JACKET, JACKET_D, JACKET_D, SKIN, "arm")
    # gambe: destra (4,20) e sinistra (20,52)
    limb(img, 0, 20, SKIRT, SKIN, SKIN_D, SKIN, SHOE, "leg")
    limb(img, 16, 52, SKIRT, SKIN, SKIN_D, SKIN, SHOE, "leg")
    OUT.parent.mkdir(parents=True, exist_ok=True)
    img.save(OUT)
    print(OUT)


if __name__ == "__main__":
    main()
