"""The game's own player skin (`entity/player/wide/rustcraft`): an original character, the
RustCraft explorer - copper hair, brass goggles pushed up on the forehead, a green neckerchief,
a rust-brown leather vest over a cream linen shirt with rolled sleeves, leather bracers, a
belt with a copper buckle, olive canvas trousers and brown boots.

Minecraft's 64x64 skin layout (wide arms) at 8 px per unit (512 x 512). The base layer holds
the body and the clothes worn close; the outer layer (hat, jacket, sleeves, trousers: 32 units
right of the head's faces, 16 below the others') holds what sits on top: the goggles and their
strap, the neckerchief, the vest, the belt and the bracers. The game draws the outer layer over
the base (`src/textures/from_pack.rs`), and it uses the front of each arm and leg on all four
of its sides, the body's side on its top and the head's bottom for the hands' ends - so those
faces are drawn to work there too.

    python tools/texgen/player.py   # tools/texgen/out/player.png: the atlas and the body put together
"""

from __future__ import annotations

import numpy as np

from common import Ramp, blank, grid, hexc, pix, polygon, rng, thick_line
from entities import aniso, box_uv, put

K = 8  # px per unit

SKIN = Ramp("#7d4a31", "#9a5f42", "#b67656", "#c98a68", "#d99e7c", "#e6b291", "#efc4a6")
HAIR = Ramp("#2a120b", "#3e1b10", "#552615", "#6d311a", "#853f20", "#9b4f28", "#b06334")
LINEN = Ramp("#7e7258", "#9a8d6e", "#b4a684", "#c9bb97", "#d9cca8", "#e7dcbb", "#f1e9cf")
VEST = Ramp("#331810", "#4d2416", "#68311c", "#814024", "#99502c", "#ae6236", "#c27744")
SCARF = Ramp("#18301f", "#21412a", "#2c5436", "#386844", "#467d52", "#5a9464")
STRAP = Ramp("#1e130c", "#2c1d13", "#3c2a1c", "#4f3826", "#644831")
BRASS = Ramp("#5b2f16", "#86461f", "#b06429", "#d08640", "#e8a95e", "#f6cf8c")
LENS = Ramp("#123234", "#1d4c4c", "#2b6965", "#3f8a82", "#6db7a9", "#b9e6d8")
CANVAS = Ramp("#25271b", "#323524", "#40442e", "#4e5338", "#5d6343", "#6d744f")
BOOT = Ramp("#1f130c", "#332015", "#482e1d", "#5e3d26", "#764f31", "#8c613d")
EYE_WHITE = hexc("#f2efe8")
IRIS = Ramp("#1e3a1f", "#2f5a2d", "#43793d", "#5d9850")
MOUTH = hexc("#7a3a31")
LIP = hexc("#9a5245")


# ---------------------------------------------------------------------------- helpers


def face(h, w):
    """An empty face (transparent)."""
    return blank(h, w)


def fill(img, mask, rgb):
    rgb = np.asarray(rgb, np.float32)
    img[mask, :3] = rgb[mask] if rgb.ndim == 3 else rgb
    img[mask, 3] = 255


def shade(ramp: Ramp, t, dither=0.45):
    return ramp.shade(np.clip(t, 0, 1), dither)


def lit(h, w, top=0.08, left=0.05):
    """Light from the top left: + near the top/left, - towards the bottom/right."""
    yy, xx = grid(h, w)
    return top * (0.5 - yy / h) + left * (0.5 - xx / w)


def cloth(seed, h, w, ramp, base, amp=0.16, cy=10.0, cx=3.0, grain=0.05):
    """Woven cloth: long soft folds along y, a fine grain."""
    t = base + (aniso(seed, h, w, cy, cx) - 0.5) * amp + (pix(seed + 1, 1, h, w) - 0.5) * grain
    return shade(ramp, t + lit(h, w))


def leather(seed, h, w, ramp, base, amp=0.12):
    """Leather: soft blotches and a few scuffs."""
    t = base + (aniso(seed, h, w, 9, 9) - 0.5) * amp + (pix(seed + 1, 2, h, w) - 0.5) * 0.04
    return shade(ramp, t + lit(h, w, 0.1, 0.06))


def rows(h, w, y0, y1):
    yy, _ = grid(h, w)
    return (yy >= y0) & (yy < y1)


def cols_(h, w, x0, x1):
    _, xx = grid(h, w)
    return (xx >= x0) & (xx < x1)


def stitches(mask_line, every=4):
    """Every `every`th pixel of a 1 px line (dashed stitching)."""
    yy, xx = grid(*mask_line.shape)
    return mask_line & (((yy + xx).astype(int) // 2) % (every // 2 + 1) == 0)


def skin_tone(seed, h, w, base=0.62):
    t = base + (aniso(seed, h, w, 12, 12) - 0.5) * 0.06 + (pix(seed + 1, 2, h, w) - 0.5) * 0.02
    return shade(SKIN, t + lit(h, w, 0.08, 0.05), 0.2)


def hair_tone(seed, h, w, base=0.55, flow=(7, 2)):
    """Hair: locks along `flow` (y, x feature size), with lighter strands."""
    t = base + (aniso(seed, h, w, *flow) - 0.5) * 0.45 + (pix(seed + 1, 1, h, w) - 0.5) * 0.1
    return shade(HAIR, t + lit(h, w, 0.12, 0.08), 0.3)


# ---------------------------------------------------------------------------- head


def head_front(seed):
    h = w = 8 * K
    img = face(h, w)
    yy, xx = grid(h, w)
    fill(img, np.ones((h, w), bool), skin_tone(seed, h, w))
    # Cheeks a touch warmer and the chin's underside in shadow.
    fill(img, (yy > 61), SKIN.at(3))
    # Hair: a fringe swept to the right, longer at the temples.
    r = rng(seed + 1)
    edge = 12 + 2.5 * np.sin(xx / 6.5 + 1.3) + np.where(xx < 10, (10 - xx) * 1.4, 0) + np.where(xx > 54, (xx - 54) * 1.1, 0)
    lock = r.integers(0, 3, 16)
    edge = edge + lock[(xx // 4).astype(int) % 16]
    hairm = yy < edge
    fill(img, hairm, hair_tone(seed + 2, h, w, 0.55, (9, 2.5)))
    # The fringe's tips darker (they throw a shadow on the forehead).
    tip = hairm & ~(np.roll(hairm, -2, 0))
    fill(img, tip, HAIR.at(1))
    shadow = ~hairm & np.roll(hairm, 2, 0)
    fill(img, shadow & (yy < 30), SKIN.at(2))
    # Eyebrows.
    for x0, x1, slope in ((8, 23, 0.12), (41, 56, -0.12)):
        brow = (xx >= x0) & (xx < x1) & (np.abs(yy - (27 + (xx - (x0 + x1) / 2) * slope)) < 1.6)
        fill(img, brow, HAIR.at(1))
    # Eyes: the green iris between the whites (looking ahead), a glint.
    for wx in (8, 40):
        ix = wx + 4
        fill(img, rows(h, w, 32, 40) & cols_(h, w, wx, wx + 16), EYE_WHITE)
        fill(img, rows(h, w, 38, 40) & cols_(h, w, wx, wx + 16), hexc("#d6d0c6"))
        iris = rows(h, w, 32, 40) & cols_(h, w, ix, ix + 8)
        fill(img, iris, IRIS.at(2))
        fill(img, iris & (yy < 34), IRIS.at(1))
        fill(img, rows(h, w, 34, 38) & cols_(h, w, ix + 2, ix + 6), IRIS.at(0))
        fill(img, rows(h, w, 34, 36) & cols_(h, w, ix + 2, ix + 4), hexc("#e8f0e0"))
        fill(img, rows(h, w, 38, 40) & cols_(h, w, ix, ix + 8), IRIS.at(3))
    # Lashes over each eye (the upper lid).
    fill(img, rows(h, w, 31, 32) & (cols_(h, w, 8, 24) | cols_(h, w, 40, 56)), SKIN.at(1))
    # Nose: a lit bridge and a shadow under it.
    fill(img, rows(h, w, 38, 45) & cols_(h, w, 28, 36), SKIN.at(4))
    fill(img, rows(h, w, 38, 45) & cols_(h, w, 34, 36), SKIN.at(3))
    fill(img, rows(h, w, 45, 47) & cols_(h, w, 27, 37), SKIN.at(2))
    # Freckles over the nose and cheeks.
    for (fy, fx) in ((42, 20), (44, 23), (41, 25), (43, 39), (41, 42), (44, 45), (46, 18), (46, 47)):
        fill(img, rows(h, w, fy, fy + 2) & cols_(h, w, fx, fx + 2), SKIN.at(3))
    # A small smile.
    mouth = rows(h, w, 51, 54) & cols_(h, w, 25, 39)
    mouth |= rows(h, w, 49, 52) & (cols_(h, w, 23, 25) | cols_(h, w, 39, 41))
    fill(img, mouth, MOUTH)
    fill(img, rows(h, w, 53, 54) & cols_(h, w, 27, 37), LIP)
    fill(img, rows(h, w, 54, 56) & cols_(h, w, 27, 37), SKIN.at(3))
    # Cheeks' rosy hint at the sides of the smile.
    for x0 in (12, 44):
        rosy = rows(h, w, 46, 50) & cols_(h, w, x0, x0 + 8) & (pix(seed + 5, 2, h, w) > 0.45)
        fill(img, rosy, hexc("#d48a72"))
    return img


def head_side(seed):
    """The right side: the back of the head on the left, the face on the right."""
    h = w = 8 * K
    img = face(h, w)
    yy, xx = grid(h, w)
    fill(img, np.ones((h, w), bool), skin_tone(seed, h, w, 0.56))
    r = rng(seed + 3)
    tips = r.integers(0, 4, 16)
    top = 17 - xx * 0.05 + 2 * np.sin(xx / 5) + tips[(xx // 4).astype(int) % 16]
    back = 24 - np.clip(yy - 40, 0, None) * 0.5 + tips[(yy // 4).astype(int) % 16]
    hairm = (yy < top) | ((xx < back) & (yy < 60))
    # The sideburn in front of the ear.
    hairm |= (xx >= 40) & (xx < 46) & (yy < 36 - (xx - 40) * 0.5)
    fill(img, hairm, hair_tone(seed + 1, h, w, 0.5, (8, 2.5)))
    nape = hairm & ~np.roll(hairm, -2, 0) & (yy > 20)
    fill(img, nape, HAIR.at(1))
    # The ear.
    ear = rows(h, w, 27, 45) & cols_(h, w, 28, 38)
    ear &= ~(rows(h, w, 27, 29) & (cols_(h, w, 28, 30) | cols_(h, w, 36, 38)))
    ear &= ~(rows(h, w, 43, 45) & cols_(h, w, 28, 31))
    fill(img, ear, SKIN.at(3))
    fill(img, ear & ~np.roll(ear, 2, 0), SKIN.at(4))
    fill(img, rows(h, w, 31, 41) & cols_(h, w, 31, 35), SKIN.at(2))
    fill(img, rows(h, w, 33, 39) & cols_(h, w, 32, 34), SKIN.at(1))
    fill(img, ear & ~np.roll(ear, -2, 1), SKIN.at(2))
    # The jaw's underside in shadow.
    fill(img, (yy > 61) & (xx > 30), SKIN.at(3))
    return img


def head_back(seed):
    h = w = 8 * K
    img = face(h, w)
    yy, xx = grid(h, w)
    fill(img, np.ones((h, w), bool), skin_tone(seed, h, w, 0.5))
    r = rng(seed + 2)
    tips = r.integers(0, 4, 16)
    edge = 58 + 2 * np.sin(xx / 3.1) + tips[(xx // 4).astype(int) % 16] + np.where(np.abs(xx - 32) > 22, 6, 0)
    hairm = yy < edge
    fill(img, hairm, hair_tone(seed + 1, h, w, 0.48, (10, 2.5)))
    fill(img, hairm & ~np.roll(hairm, -2, 0), HAIR.at(1))
    return img


def head_top(seed):
    h = w = 8 * K
    img = face(h, w)
    yy, xx = grid(h, w)
    # Locks parted from a crown towards the back.
    ang = np.arctan2(yy - 20, xx - 36)
    t = 0.56 + 0.18 * np.sin(ang * 5 + np.hypot(yy - 20, xx - 36) / 6)
    t += (aniso(seed, h, w, 4, 4) - 0.5) * 0.3 + (pix(seed + 1, 1, h, w) - 0.5) * 0.1
    fill(img, np.ones((h, w), bool), shade(HAIR, t + lit(h, w, 0.1, 0.08), 0.3))
    return img


def head_bottom(seed):
    """The chin's underside - and the ends of the hands (the game uses it there)."""
    h = w = 8 * K
    img = face(h, w)
    fill(img, np.ones((h, w), bool), skin_tone(seed, h, w, 0.5))
    return img


def goggles_front(seed):
    """The outer layer's front: brass goggles pushed up on the forehead, teal lenses."""
    h = w = 8 * K
    img = face(h, w)
    yy, xx = grid(h, w)
    band = rows(h, w, 11, 19)
    fill(img, band, leather(seed, h, w, STRAP, 0.45))
    fill(img, rows(h, w, 11, 12), STRAP.at(3))
    for cx in (17, 47):
        rim = ((yy - 15) / 8.5) ** 2 + ((xx - cx) / 11) ** 2 <= 1
        glass = ((yy - 15) / 6) ** 2 + ((xx - cx) / 8.5) ** 2 <= 1
        t = 0.55 + 0.35 * (15 - yy) / 8 + 0.15 * (cx - xx) / 11
        fill(img, rim, shade(BRASS, t, 0.3))
        fill(img, rim & ~np.roll(rim, 1, 0), BRASS.at(5))
        fill(img, rim & ~np.roll(rim, -1, 0), BRASS.at(1))
        g = 0.45 + 0.3 * (yy - 15) / 6 - 0.1 * (xx - cx) / 8
        fill(img, glass, shade(LENS, g, 0.4))
        # The lens' glint, top left.
        fill(img, glass & (np.abs((xx - cx + 3) + (yy - 13) * 1.2) < 1.3) & (yy < 15), LENS.at(5))
        fill(img, glass & (yy < 11) & (xx > cx + 2), LENS.at(1))
        # Rivets on the rims' sides.
        for rx in (cx - 9.5, cx + 9.5):
            fill(img, (np.abs(xx - rx) < 1) & (np.abs(yy - 15) < 1), BRASS.at(5))
    fill(img, rows(h, w, 13, 17) & cols_(h, w, 28, 36), shade(BRASS, np.full((h, w), 0.45)))
    fill(img, rows(h, w, 13, 14) & cols_(h, w, 28, 36), BRASS.at(4))
    # Locks of the fringe falling over the strap and the bridge, and at the temples: tapered
    # strands, lit on the left, a dark edge on the right.
    locks = np.zeros((h, w), bool)
    for x0, wd, tip, lean in ((0, 7, 27, 1), (5, 5, 21, 1), (26, 6, 23, 2), (32, 4, 18, 1),
                              (53, 5, 20, -1), (57, 7, 26, -1)):
        locks |= polygon([(x0, 5), (x0 + wd, 5), (x0 + wd * 0.5 + lean * 2, tip)], h, w)
    fill(img, locks, hair_tone(seed + 9, h, w, 0.6, (6, 2)))
    fill(img, locks & ~np.roll(locks, -1, 1), HAIR.at(1))
    fill(img, locks & ~np.roll(locks, 1, 1) & (yy > 8), HAIR.at(5))
    return img


def strap_around(seed, buckle=False):
    """The outer layer round the side and the back: the goggles' strap (and its buckle)."""
    h = w = 8 * K
    img = face(h, w)
    yy, xx = grid(h, w)
    band = rows(h, w, 11, 19)
    fill(img, band, leather(seed, h, w, STRAP, 0.45))
    fill(img, rows(h, w, 11, 12), STRAP.at(3))
    fill(img, stitches(rows(h, w, 17, 18)), STRAP.at(4))
    if buckle:
        frame = rows(h, w, 9, 21) & cols_(h, w, 27, 37)
        fill(img, frame, BRASS.at(3))
        fill(img, rows(h, w, 11, 19) & cols_(h, w, 29, 35), STRAP.at(1))
        fill(img, rows(h, w, 12, 18) & cols_(h, w, 31, 33), BRASS.at(4))
        fill(img, rows(h, w, 9, 10) & cols_(h, w, 27, 37), BRASS.at(5))
    return img


# ---------------------------------------------------------------------------- body


def shirt(seed, h, w):
    img = face(h, w)
    fill(img, np.ones((h, w), bool), cloth(seed, h, w, LINEN, 0.66))
    return img


def trousers(seed, h, w, base=0.5):
    """Olive canvas with a few folds."""
    yy, xx = grid(h, w)
    t = base + (aniso(seed, h, w, 24, 8) - 0.5) * 0.1 + (pix(seed + 1, 1, h, w) - 0.5) * 0.05
    # Diagonal twill.
    t += ((((yy + xx) // 2) % 4) == 0) * 0.03
    return shade(CANVAS, t + lit(h, w, 0.08, 0.05), 0.35)


def body_base(seed, name):
    h, w = (12 * K, 8 * K) if name in ("front", "back") else (12 * K, 4 * K)
    img = shirt(seed, h, w)
    yy, xx = grid(h, w)
    fill(img, yy >= 84, trousers(seed + 1, h, w))
    if name == "front":
        # The open collar; the placket and its buttons.
        v = np.abs(xx - 32) < 9 - yy * 0.9
        fill(img, v, skin_tone(seed + 2, h, w, 0.5))
        placket = cols_(h, w, 31, 33) & (yy > 9) & (yy < 84)
        fill(img, placket, LINEN.at(2))
        for by in (24, 40, 56, 70):
            fill(img, rows(h, w, by, by + 2) & cols_(h, w, 31, 33), LINEN.at(0))
    if name in ("right", "left"):
        fill(img, cols_(h, w, 15, 17) & (yy < 84), LINEN.at(3))
    return img


def body_over(seed, name):
    """The outer layer: the vest, the neckerchief and the belt."""
    h, w = (12 * K, 8 * K) if name in ("front", "back") else (12 * K, 4 * K)
    img = face(h, w)
    yy, xx = grid(h, w)
    vest = leather(seed, h, w, VEST, 0.5)
    if name == "front":
        inner = 19 + yy * 0.05
        left = (xx < inner) & (yy < 80)
        right = (xx >= 64 - inner) & (yy < 80)
        fill(img, left | right, vest)
        # Trim along the opening (lighter edge, stitching).
        trim = (left & (xx >= inner - 2)) | (right & (xx < 64 - inner + 2))
        fill(img, trim, VEST.at(5))
        fill(img, (left & (np.abs(xx - (inner - 4)) < 0.5)) | (right & (np.abs(xx - (64 - inner + 3.5)) < 0.5)), VEST.at(1))
        fill(img, stitches((left & (np.abs(xx - (inner - 4)) < 0.5)) | (right & (np.abs(xx - (64 - inner + 3.5)) < 0.5)), 4), VEST.at(6))
        # A pocket on the left panel (the wearer's right), a brass button on the right panel.
        pocket = rows(h, w, 46, 60) & cols_(h, w, 4, 15)
        fill(img, pocket, VEST.at(3))
        fill(img, rows(h, w, 46, 50) & cols_(h, w, 3, 16), VEST.at(4))
        fill(img, rows(h, w, 50, 51) & cols_(h, w, 3, 16), VEST.at(1))
        fill(img, rows(h, w, 47, 49) & cols_(h, w, 9, 11), BRASS.at(4))
        for by in (36, 56):
            fill(img, rows(h, w, by, by + 3) & cols_(h, w, 47, 50), BRASS.at(3))
            fill(img, rows(h, w, by, by + 1) & cols_(h, w, 47, 49), BRASS.at(5))
        # The neckerchief round the neck, its knot and its tip.
        scarf = rows(h, w, 0, 7) & cols_(h, w, 13, 51)
        tip = polygon([(22, 5), (42, 5), (34, 27), (31, 27)], h, w)
        sc = cloth(seed + 3, h, w, SCARF, 0.55, 0.2, 4, 4)
        fill(img, scarf | tip, sc)
        fill(img, rows(h, w, 6, 7) & cols_(h, w, 13, 51) & ~tip, SCARF.at(0))
        fill(img, tip & (np.abs(xx - 32.5 - (yy - 5) * 0.12) < 0.7) & (yy > 10), SCARF.at(1))
        knot = rows(h, w, 4, 11) & cols_(h, w, 28, 37)
        fill(img, knot, SCARF.at(4))
        fill(img, rows(h, w, 9, 11) & cols_(h, w, 28, 37), SCARF.at(2))
        fill(img, rows(h, w, 4, 5) & cols_(h, w, 29, 36), SCARF.at(5))
        # Dots on the neckerchief.
        dots = (scarf | tip) & ~knot & ((((yy // 2) + (xx // 2)) % 4) == 0) & (((yy // 2) % 2) == 0)
        fill(img, dots, SCARF.at(5))
    elif name == "back":
        fill(img, yy < 80, vest)
        fill(img, cols_(h, w, 31, 33) & (yy < 80), VEST.at(1))
        fill(img, stitches(cols_(h, w, 34, 35) & (yy < 80)), VEST.at(5))
        fill(img, stitches(cols_(h, w, 29, 30) & (yy < 80)), VEST.at(5))
        # A cinch strap across the waist.
        fill(img, rows(h, w, 66, 70) & cols_(h, w, 18, 46), VEST.at(1))
        fill(img, rows(h, w, 65, 71) & cols_(h, w, 30, 34), BRASS.at(3))
        scarf = rows(h, w, 0, 6) & cols_(h, w, 12, 52)
        fill(img, scarf, cloth(seed + 3, h, w, SCARF, 0.45, 0.2, 4, 4))
        fill(img, rows(h, w, 5, 6) & cols_(h, w, 12, 52), SCARF.at(0))
    else:
        fill(img, yy < 80, vest)
        fill(img, rows(h, w, 0, 10), VEST.at(3))
        fill(img, cols_(h, w, 15, 17) & (yy < 80), VEST.at(1))
        fill(img, stitches(cols_(h, w, 18, 19) & (yy < 80)), VEST.at(5))
    # The belt and its brass buckle (front).
    belt = rows(h, w, 76, 84)
    fill(img, belt, leather(seed + 4, h, w, STRAP, 0.5))
    fill(img, rows(h, w, 76, 77), STRAP.at(4))
    fill(img, rows(h, w, 83, 84), STRAP.at(0))
    if name == "front":
        frame = rows(h, w, 74, 86) & cols_(h, w, 26, 38)
        fill(img, frame, BRASS.at(3))
        fill(img, rows(h, w, 76, 84) & cols_(h, w, 28, 36), STRAP.at(1))
        fill(img, rows(h, w, 78, 82) & cols_(h, w, 30, 34), BRASS.at(2))
        fill(img, rows(h, w, 74, 75) & cols_(h, w, 26, 38), BRASS.at(5))
        fill(img, cols_(h, w, 26, 27) & rows(h, w, 74, 86), BRASS.at(4))
        fill(img, rows(h, w, 85, 86) & cols_(h, w, 26, 38), BRASS.at(1))
    else:
        # Belt loops.
        for lx in ((10, 22) if name == "back" else (14,)):
            fill(img, rows(h, w, 75, 85) & cols_(h, w, lx, lx + 3), STRAP.at(2))
    return img


def body_top_bottom(seed):
    """The shoulders (with the collar and the neckerchief round it) and the hips' underside."""
    top = shirt(seed, 4 * K, 8 * K)
    yy, xx = grid(4 * K, 8 * K)
    over = face(4 * K, 8 * K)
    vest = leather(seed + 1, 4 * K, 8 * K, VEST, 0.6)
    fill(over, (xx < 18) | (xx >= 46), vest)
    ring = (((yy - 16) / 12) ** 2 + ((xx - 32) / 15) ** 2 <= 1) & ~((((yy - 16) / 8) ** 2 + ((xx - 32) / 10) ** 2) <= 1)
    fill(over, ring, cloth(seed + 2, 4 * K, 8 * K, SCARF, 0.6, 0.2, 4, 4))
    fill(top, (((yy - 16) / 8) ** 2 + ((xx - 32) / 10) ** 2) <= 1, SKIN.at(2))
    bottom = face(4 * K, 8 * K)
    fill(bottom, np.ones((4 * K, 8 * K), bool), trousers(seed + 3, 4 * K, 8 * K, 0.35))
    return top, over, bottom


# ---------------------------------------------------------------------------- arms and legs


def arm_base(seed, inner=False):
    """One side of an arm (all four look alike: the game uses the front for each): the linen
    sleeve rolled up to the elbow, the forearm, the hand at the end."""
    h, w = 12 * K, 4 * K
    img = shirt(seed, h, w)
    yy, xx = grid(h, w)
    if inner:
        fill(img, np.ones((h, w), bool), cloth(seed, h, w, LINEN, 0.56))
    # The rolled cuff: a fat roll with a dark crease above and below.
    cuff = rows(h, w, 34, 43)
    fill(img, cuff, cloth(seed + 1, h, w, LINEN, 0.8, 0.1, 3, 6))
    fill(img, rows(h, w, 33, 34), LINEN.at(1))
    fill(img, rows(h, w, 38, 39), LINEN.at(3))
    fill(img, rows(h, w, 42, 43), LINEN.at(1))
    fill(img, yy >= 43, skin_tone(seed + 2, h, w, 0.58 if not inner else 0.5))
    fill(img, rows(h, w, 43, 45), SKIN.at(2))
    # The hand: a knuckle crease and the fingers' gaps at the end.
    fill(img, rows(h, w, 86, 87) & cols_(h, w, 6, 26), SKIN.at(3))
    for fx in (9, 16, 23):
        fill(img, rows(h, w, 89, 96) & cols_(h, w, fx, fx + 1), SKIN.at(3))
    fill(img, rows(h, w, 95, 96), SKIN.at(2))
    return img


def arm_over(seed):
    """The outer layer of an arm: a leather bracer on the forearm, laced, with a brass stud."""
    h, w = 12 * K, 4 * K
    img = face(h, w)
    yy, xx = grid(h, w)
    bracer = rows(h, w, 58, 82)
    fill(img, bracer, leather(seed, h, w, VEST, 0.45))
    fill(img, rows(h, w, 58, 60), VEST.at(5))
    fill(img, rows(h, w, 80, 82), VEST.at(1))
    for sy in (61, 79):
        fill(img, stitches(rows(h, w, sy, sy + 1)), VEST.at(6))
    # Two straps round it, a brass stud between them.
    for sy in (64, 73):
        fill(img, rows(h, w, sy, sy + 4), STRAP.at(2))
        fill(img, rows(h, w, sy, sy + 1), STRAP.at(4))
        fill(img, rows(h, w, sy, sy + 4) & cols_(h, w, 22, 25), BRASS.at(3))
        fill(img, rows(h, w, sy, sy + 1) & cols_(h, w, 22, 25), BRASS.at(5))
    fill(img, rows(h, w, 69, 72) & cols_(h, w, 14, 18), BRASS.at(4))
    fill(img, rows(h, w, 69, 70) & cols_(h, w, 14, 16), BRASS.at(5))
    fill(img, rows(h, w, 71, 72) & cols_(h, w, 15, 18), BRASS.at(1))
    return img


def arm_top(seed):
    img = shirt(seed, 4 * K, 4 * K)
    return img


def leg_base(seed):
    """One side of a leg (all four alike, see `arm_base`): olive canvas trousers tucked into
    boots."""
    h, w = 12 * K, 4 * K
    img = face(h, w)
    yy, xx = grid(h, w)
    fill(img, np.ones((h, w), bool), trousers(seed, h, w))
    # A crease down the middle and the knee's fold.
    fill(img, (np.abs(xx - 16 - np.sin(yy / 9) * 1.2) < 0.6) & (yy < 60), CANVAS.at(2))
    fill(img, rows(h, w, 40, 41) & cols_(h, w, 7, 25), CANVAS.at(1))
    fill(img, rows(h, w, 41, 42) & cols_(h, w, 9, 23), CANVAS.at(4))
    # Bunched over the boot tops.
    fill(img, rows(h, w, 60, 64), CANVAS.at(1))
    # Boots: a turned-down cuff, the shaft with a strap, the sole.
    boot = yy >= 64
    fill(img, boot, leather(seed + 3, h, w, BOOT, 0.5))
    fill(img, rows(h, w, 64, 72), leather(seed + 4, h, w, BOOT, 0.72))
    fill(img, rows(h, w, 64, 65), BOOT.at(5))
    fill(img, rows(h, w, 71, 72), BOOT.at(1))
    fill(img, rows(h, w, 78, 82), STRAP.at(2))
    fill(img, rows(h, w, 78, 79), STRAP.at(3))
    fill(img, rows(h, w, 89, 90), BOOT.at(4))
    fill(img, rows(h, w, 90, 96), BOOT.at(0))
    fill(img, rows(h, w, 90, 91), BOOT.at(2))
    return img


def leg_ends(seed):
    top = face(4 * K, 4 * K)
    fill(top, np.ones((4 * K, 4 * K), bool), trousers(seed, 4 * K, 4 * K, 0.4))
    sole = face(4 * K, 4 * K)
    yy, xx = grid(4 * K, 4 * K)
    fill(sole, np.ones((4 * K, 4 * K), bool), BOOT.at(0))
    fill(sole, ((yy // 2).astype(int) % 3 == 0) & (xx > 2) & (xx < 30), BOOT.at(1))
    return top, sole


# ---------------------------------------------------------------------------- the atlas


def mirror(img):
    return img[:, ::-1].copy()


def rustcraft(seed: int) -> np.ndarray:
    a = blank(64 * K, 64 * K)
    # Head (0, 0) and its outer layer (32, 0).
    hd, hat = box_uv(0, 0, 8, 8, 8), box_uv(32, 0, 8, 8, 8)
    put(a, head_top(seed + 1), *hd["top"][:2], K)
    put(a, head_bottom(seed + 2), *hd["bottom"][:2], K)
    put(a, head_front(seed + 3), *hd["front"][:2], K)
    put(a, head_side(seed + 4), *hd["right"][:2], K)
    put(a, mirror(head_side(seed + 4)), *hd["left"][:2], K)
    put(a, head_back(seed + 5), *hd["back"][:2], K)
    put(a, goggles_front(seed + 6), *hat["front"][:2], K)
    put(a, strap_around(seed + 7), *hat["right"][:2], K)
    put(a, mirror(strap_around(seed + 7)), *hat["left"][:2], K)
    put(a, strap_around(seed + 8, buckle=True), *hat["back"][:2], K)
    # Body (16, 16) and the jacket layer (16, 32).
    bd, jk = box_uv(16, 16, 8, 12, 4), box_uv(16, 32, 8, 12, 4)
    for name in ("front", "back", "right", "left"):
        base = body_base(seed + 10, name)
        over = body_over(seed + 20, name)
        if name == "left":
            base, over = mirror(base), mirror(over)
        put(a, base, *bd[name][:2], K)
        put(a, over, *jk[name][:2], K)
    top, top_over, bottom = body_top_bottom(seed + 30)
    put(a, top, *bd["top"][:2], K)
    put(a, top_over, *jk["top"][:2], K)
    put(a, bottom, *bd["bottom"][:2], K)
    # Arms: the right (40, 16) with its sleeve layer (40, 32), the left (32, 48) with (48, 48).
    for (u, v), (ou, ov), flip in (((40, 16), (40, 32), False), ((32, 48), (48, 48), True)):
        arm, sleeve = box_uv(u, v, 4, 12, 4), box_uv(ou, ov, 4, 12, 4)
        for i, name in enumerate(("front", "right", "back", "left")):
            inner = name == ("left" if not flip else "right")
            img = arm_base(seed + 40 + i, inner)
            over = arm_over(seed + 50 + i)
            if flip:
                img, over = mirror(img), mirror(over)
            put(a, img, *arm[name][:2], K)
            put(a, over, *sleeve[name][:2], K)
        put(a, arm_top(seed + 60), *arm["top"][:2], K)
        put(a, head_bottom(seed + 61)[: 4 * K, : 4 * K], *arm["bottom"][:2], K)
    # Legs: the right (0, 16) with its trouser layer (0, 32), the left (16, 48) with (0, 48).
    for (u, v), flip in (((0, 16), False), ((16, 48), True)):
        leg = box_uv(u, v, 4, 12, 4)
        for i, name in enumerate(("front", "right", "back", "left")):
            img = leg_base(seed + 70 + i)
            put(a, mirror(img) if flip else img, *leg[name][:2], K)
        top, sole = leg_ends(seed + 80)
        put(a, top, *leg["top"][:2], K)
        put(a, sole, *leg["bottom"][:2], K)
    return a


TEXTURES = {
    "entity/player/wide/rustcraft": rustcraft,
}


# ---------------------------------------------------------------------------- preview


def _preview():
    """The atlas (outer layer over a checkerboard) and the faces put together the way the game
    shows them: front, back and side, at 4x."""
    import zlib
    from pathlib import Path

    from PIL import Image

    here = Path(__file__).parent
    out = here / "out"
    out.mkdir(exist_ok=True)
    path = "entity/player/wide/rustcraft"
    a = rustcraft(zlib.crc32(path.encode()) & 0x7FFFFFFF)

    def region(x, y, w, h):
        return a[y * K : (y + h) * K, x * K : (x + w) * K]

    def composed(x, y, w, h, ox, oy):
        b = region(x, y, w, h).copy()
        o = region(x + ox, y + oy, w, h)
        al = o[..., 3:4] / 255.0
        b[..., :3] = o[..., :3] * al + b[..., :3] * (1 - al)
        return b

    def figure(head, body, arm_r, arm_l, leg_r, leg_l):
        fig = np.zeros((32 * K, 16 * K, 4), np.float32)
        def at(img, x, y):
            hh, ww = img.shape[:2]
            fig[y * K : y * K + hh, x * K : x * K + ww] = img
        at(head, 4, 0)
        at(body, 4, 8)
        at(arm_r, 0, 8)
        at(arm_l, 12, 8)
        at(leg_r, 4, 20)
        at(leg_l, 8, 20)
        return fig

    front = figure(composed(8, 8, 8, 8, 32, 0), composed(20, 20, 8, 12, 0, 16),
                   composed(44, 20, 4, 12, 0, 16), composed(44, 20, 4, 12, 0, 16)[:, ::-1],
                   composed(4, 20, 4, 12, 0, 16), composed(4, 20, 4, 12, 0, 16)[:, ::-1])
    back = figure(composed(24, 8, 8, 8, 32, 0), composed(32, 20, 8, 12, 0, 16),
                  composed(44, 20, 4, 12, 0, 16), composed(44, 20, 4, 12, 0, 16)[:, ::-1],
                  composed(4, 20, 4, 12, 0, 16), composed(4, 20, 4, 12, 0, 16)[:, ::-1])
    side = np.zeros((32 * K, 16 * K, 4), np.float32)
    side[0 : 8 * K, 4 * K : 12 * K] = composed(0, 8, 8, 8, 32, 0)
    side[8 * K : 20 * K, 6 * K : 10 * K] = composed(16, 20, 4, 12, 0, 16)
    side[8 * K : 20 * K, 6 * K : 10 * K] = composed(44, 20, 4, 12, 0, 16)
    side[20 * K : 32 * K, 6 * K : 10 * K] = composed(4, 20, 4, 12, 0, 16)

    def show(img, scale):
        h, w = img.shape[:2]
        bg = np.where((np.indices((h, w)).sum(0) // 8 % 2)[..., None] == 0, 70, 95).repeat(3, 2)
        al = img[..., 3:4] / 255.0
        rgb = (img[..., :3] * al + bg * (1 - al)).astype(np.uint8)
        return Image.fromarray(rgb, "RGB").resize((w * scale, h * scale), Image.NEAREST)

    show(a, 1).save(out / "player.png")
    figs = [show(f, 2) for f in (front, back, side)]
    sheet = Image.new("RGB", (3 * 256 + 40, 512), (30, 30, 34))
    for i, f in enumerate(figs):
        sheet.paste(f, (10 + i * 266, 0))
    sheet.save(out / "player_figures.png")
    print(out / "player.png", out / "player_figures.png")


if __name__ == "__main__":
    _preview()
