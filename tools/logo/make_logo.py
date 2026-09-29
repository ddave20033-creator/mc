"""Draws the game's logo, "YOUR WORLDS", into src/ui/logo.png (1024x128, eight 128x128 tiles
the game loads as texture layers): the font8x8 letters the game's text uses, each pixel a
bevelled block with a gradient down it, extruded back to the lower right. "YOUR" in rust
orange, "WORLDS" in steel.

Run from the repository root: python tools/logo/make_logo.py
"""

import glob
import os
import random
import re

from PIL import Image

TEXT = "YOUR WORLDS"
CELL = 12  # pixels per font pixel
DEPTH = 4  # extrusion steps


def font():
    """font8x8's BASIC_LEGACY table (128 glyphs of 8 rows, bit 0 the leftmost pixel)."""
    home = os.path.expanduser("~/.cargo/registry/src")
    path = glob.glob(os.path.join(home, "*", "font8x8-*", "src", "legacy.rs"))[0]
    lines = open(path).read().splitlines()
    start = next(i for i, l in enumerate(lines) if "pub const BASIC_LEGACY" in l)
    glyphs = []
    for l in lines[start + 1:]:
        if l.strip().startswith("];"):
            break
        if "NOTHING_TO_DISPLAY" in l:
            glyphs.append([0] * 8)
            continue
        m = re.search(r"\[([^\]]*)\]", l)
        if m:
            glyphs.append([int(x, 16) for x in m.group(1).split(",") if x.strip()])
    return glyphs


def main():
    glyphs = font()
    cells = []
    pen = 0
    for i, ch in enumerate(TEXT):
        if ch == " ":
            pen += 4
            continue
        g = glyphs[ord(ch)]
        cols = [c for c in range(8) if any(r >> c & 1 for r in g)]
        lo, hi = min(cols), max(cols)
        for y, row in enumerate(g):
            for c in range(lo, hi + 1):
                if row >> c & 1:
                    cells.append((pen + c - lo, y, i < 4))
        pen += hi - lo + 2
    w, h = pen - 1, 7
    step = int(CELL * 0.3)
    iw, ih = w * CELL + DEPTH * step + 8, h * CELL + DEPTH * step + 8
    tw = (iw + 127) // 128 * 128
    img = Image.new("RGBA", (tw, 128), (0, 0, 0, 0))
    px = img.load()
    ox, oy = (tw - iw) // 2 + 4, (128 - ih) // 2 + 4
    filled = {(x, y) for x, y, _ in cells}
    random.seed(7)

    def block(x0, y0, color):
        for yy in range(y0, y0 + CELL):
            for xx in range(x0, x0 + CELL):
                px[xx, yy] = color

    for k in range(DEPTH, 0, -1):
        f = 1 - k / (DEPTH + 2)
        for x, y, rust in cells:
            base = (96, 44, 22) if rust else (46, 48, 58)
            block(ox + x * CELL + k * step, oy + y * CELL + k * step, tuple(int(b * f) for b in base) + (255,))
    for x, y, rust in cells:
        top = (246, 150, 86) if rust else (236, 238, 244)
        bot = (196, 92, 44) if rust else (164, 168, 182)
        for yy in range(CELL):
            t = (y * CELL + yy) / (h * CELL)
            c = [int(top[i] + (bot[i] - top[i]) * t) for i in range(3)]
            n = random.uniform(0.97, 1.03)
            for xx in range(CELL):
                cc = [min(255, int(v * n)) for v in c]
                if (x, y - 1) not in filled and yy < 2:
                    cc = [min(255, v + 45) for v in cc]
                if (x, y + 1) not in filled and yy >= CELL - 2:
                    cc = [int(v * 0.72) for v in cc]
                if (x - 1, y) not in filled and xx < 2:
                    cc = [min(255, v + 20) for v in cc]
                if (x + 1, y) not in filled and xx >= CELL - 2:
                    cc = [int(v * 0.85) for v in cc]
                px[ox + x * CELL + xx, oy + y * CELL + yy] = tuple(cc) + (255,)
    img.save("src/ui/logo.png")
    print(f"src/ui/logo.png: {tw}x128, logo {iw}x{ih}")


if __name__ == "__main__":
    main()
