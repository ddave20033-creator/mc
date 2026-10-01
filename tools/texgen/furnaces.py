"""The better furnaces, in the game's flat style (see BRIEF.md).

- Blast furnace: a heavy brick body bound with dark iron (posts, bands, a plinth) full of
  rivets, its two openings ringed by riveted iron collars; a brick chimney stack on top.
- Advanced furnace (two blocks wide, two tall): clean light-steel panels with rounded
  corners in a dark iron frame, bolts; the furnace, a gauge panel beside it, and a hood over
  both with a vent grille that glows while it burns.

The furnace fronts keep the plain furnace's two openings (`UP_ARCH`, `LOW_ARCH`): the game
cuts out everything between their dark outlines in each row (r + g + b < 60) and puts the
model's hollows behind them, so nothing else on a front is that dark.
"""

from __future__ import annotations

import numpy as np

import flat
from flat import S, Canvas, box, capsule, disk, ellipse, hexc, poly, tones, union

# ---------------------------------------------------------------------------- palette

BRICK = tones("#b5553f")            # dark, base, base light, light
MORTAR = tones("#d8cfc0", 3, 0.1)
IRON = tones("#c9ccd2")
STEEL = tones("#b2b7bf", spread=0.13)  # the advanced furnace's panels
DARK = [hexc("2e3138"), hexc("3d4048"), hexc("4f535c"), hexc("6b6f79")]
COPPER = tones("#c8743f")
SOOT = [hexc("2a2626"), hexc("37312f"), hexc("4a423e")]
GLOW = [hexc("ff8a2a"), hexc("ffcf5a"), hexc("ffe9a3")]
EMBER = hexc("c4471f")
HOLE_EDGE = hexc("100e0d")   # the openings' outline (the game finds the openings by it)
HOLE = hexc("1b1716")        # inside an opening (cut away in the world)
HOLE_BACK = hexc("251f1d")

# The openings, as the plain furnace has them: (center y, center x, rx, ry, flat bottom).
UP_ARCH = (56, 64, 42, 32, 56)
LOW_ARCH = (114, 64, 44, 22, 124)


# ---------------------------------------------------------------------------- helpers


def arch(cy, cx, rx, ry, bottom, grow_=0.0):
    """A round-topped opening (half ellipse over a rectangle) with a flat bottom, grown by
    `grow_` px on the top and sides."""
    rx, ry = rx + grow_, ry + grow_

    def f(y, x):
        top = ((y - cy) / ry) ** 2 + ((x - cx) / rx) ** 2 <= 1.0
        low = (y >= cy) & (np.abs(x - cx) <= rx)
        return (top | low) & (y < bottom)

    return f


def raised(c: Canvas, shape, t, base=1, rim=2.0, shadow=None):
    """`shape` lit from the top left in the 4-tone set `t` (t[0] shadow rim, t[base] face,
    t[3] light rim), with an optional drop shadow colour down-right."""
    if shadow is not None:
        c.fill(flat.minus(flat.moved_shape(shape, 1.5, 1.5), shape), shadow)
    c.raised(shape, t[base], light=t[3], dark=t[0], rim=rim)


def rivet(c: Canvas, cy, cx, r, t, shadow=None, tile=False):
    raised(c, disk(cy, cx, r, tile), t, 2, rim=max(1.0, r * 0.38), shadow=shadow)


def bricks(c: Canvas, seed, bw=32, bh=16, gap=3.0, r=3.0, mortar=None, t=BRICK, y0=0.0,
           y1=float(S), x0=0.0, x1=float(S), tile=True):
    """A running-bond brick wall in rows y0..y1 (whole courses of `bh`), each brick a
    rounded raised slab, two face tones mixed."""
    rng = np.random.default_rng(seed)
    c.fill(box(y0, x0, y1, x1), MORTAR[1] if mortar is None else mortar)
    area = box(y0, x0, y1, x1)
    n = int(round((y1 - y0) / bh))
    for row in range(n):
        top = y0 + row * bh
        off = (bw / 2) if row % 2 else 0.0
        for k in range(-1, int(S / bw) + 1):
            bx0 = k * bw + off + gap / 2
            bx1 = bx0 + bw - gap
            shape = box(top + gap / 2, bx0, top + bh - gap / 2, bx1, r=r, tile=tile)
            raised(c, clip(shape, area), t, base=1 + int(rng.random() < 0.45), rim=2.0)


def clip(shape, area):
    return lambda y, x: shape(y, x) & area(y, x)


def iron_bar(c: Canvas, y0, x0, y1, x1, rivets_at=(), r=0.0, rr=3.0):
    """A dark iron strap or post with rivets."""
    raised(c, box(y0, x0, y1, x1, r=r), DARK, 1, rim=2.0)
    for cy, cx in rivets_at:
        rivet(c, cy, cx, rr, IRON, shadow=DARK[0])


def opening(c: Canvas, spec, collar=7.0, collar_t=DARK, n_rivets=5, edge=3.0):
    """An opening ringed by a collar: a raised band round it, rivets along its arc, then the
    dark hole with its outline."""
    cy, cx, rx, ry, bottom = spec
    raised(c, arch(cy, cx, rx, ry, bottom, collar), collar_t, 1, rim=2.0, shadow=None)
    for a in np.linspace(np.pi * 1.1, np.pi * 1.9, n_rivets):
        py = cy + np.sin(a) * (ry + collar * 0.5)
        px = cx + np.cos(a) * (rx + collar * 0.5)
        rivet(c, py, px, 2.3, IRON)
    c.fill(arch(cy, cx, rx, ry, bottom), HOLE_EDGE)
    c.fill(arch(cy, cx, rx - edge, ry - edge, bottom), HOLE)
    # The back wall, a little lighter low in the middle.
    c.fill(clip(ellipse(bottom, cx, ry * 0.55, rx * 0.6, tile=False),
                arch(cy, cx, rx - edge, ry - edge, bottom)), HOLE_BACK)


def frame(c: Canvas, w=6.0, t=DARK, sides="tlbr", W=S, H=S):
    """A dark iron frame round the block (only on `sides`: t, l, b, r)."""
    m = np.zeros(c.alpha.shape, bool)
    if "t" in sides:
        m |= c.y < w
    if "b" in sides:
        m |= c.y > H - w
    if "l" in sides:
        m |= c.x < w
    if "r" in sides:
        m |= c.x > W - w
    c.fill(m, t[1])
    c.fill(m & ((c.y < 2) | (c.x < 2)), t[2])


def steel_panel(c: Canvas, y0, x0, y1, x1, r=7.0, bolts=True, bolt_in=7.0):
    """A clean light-steel panel with rounded corners and a bolt in each corner."""
    raised(c, box(y0, x0, y1, x1, r=r), STEEL, 2, rim=2.5, shadow=DARK[0])
    if bolts:
        for by in (y0 + bolt_in, y1 - bolt_in):
            for bx in (x0 + bolt_in, x1 - bolt_in):
                bolt(c, by, bx)


def bolt(c: Canvas, cy, cx, r=3.2):
    """A hex-ish bolt head: dark ring, steel head lit from the top left."""
    c.fill(disk(cy + 0.8, cx + 0.8, r + 0.6, False), STEEL[0])
    raised(c, disk(cy, cx, r, False), STEEL, 1, rim=1.2)


# ---------------------------------------------------------------------------- blast furnace


def blast_body(c: Canvas, seed, mid=(56, 66), plinth=120):
    """The brick body with its iron posts, top band, middle band and plinth."""
    bricks(c, seed)
    iron_bar(c, 0, 0, 12, S, [(6, x) for x in (32, 64, 96)])
    iron_bar(c, mid[0], 0, mid[1], S, [((mid[0] + mid[1]) / 2, x) for x in (32, 96)])
    iron_bar(c, plinth, 0, S, S, [((plinth + S) / 2, x) for x in (32, 64, 96)] if S - plinth > 7
             else [])
    for x0 in (0, S - 11):
        iron_bar(c, 0, x0, S, x0 + 11, [(y, x0 + 5.5) for y in (6, 61, 116)])


def paint_blast_front(seed):
    c = Canvas(tile=False)
    bricks(c, seed)
    iron_bar(c, 0, 0, 12, S, [(6, x) for x in (32, 96)])
    iron_bar(c, 124, 0, S, S)
    iron_bar(c, 56, 0, 66, S, [(61, x) for x in (16,)])
    for x0 in (0, S - 11):
        iron_bar(c, 0, x0, S, x0 + 11, [(y, x0 + 5.5) for y in (6, 61, 76)])
    opening(c, UP_ARCH, collar=7.0, n_rivets=5)
    opening(c, LOW_ARCH, collar=6.0, n_rivets=5)
    # The lintel under the mouth runs over the collar's feet.
    iron_bar(c, 56, 11, 66, S - 11, [(61, x) for x in (24, 64, 104)])
    return c.finish(opaque=True)


def paint_blast_side(seed):
    c = Canvas(tile=False)
    blast_body(c, seed + 11, mid=(58, 68), plinth=118)
    return c.finish(opaque=True)


def paint_blast_top(seed):
    c = Canvas(tile=False)
    bricks(c, seed + 21)
    frame(c, 10, DARK)
    raised(c, flat.minus(box(0, 0, S, S), box(10, 10, S - 10, S - 10, r=6)), DARK, 1, rim=2)
    for y in (5, S - 5):
        for x in (5, 64, S - 5):
            rivet(c, y, x, 2.8, IRON)
    for x in (5, S - 5):
        rivet(c, 64, x, 2.8, IRON)
    return c.finish(opaque=True)


# ---------------------------------------------------------------------------- chimney
# The chimney model (CHIMNEY_BOXES) maps the texture by position: the slab is rows 88..128
# of the side (the whole width), the stack rows 16..88 at x 32..96, its rim rows 0..16 at
# x 24..104; from above the rim is the square 24..104, the slab what is round it.


def paint_chimney_side(seed):
    c = Canvas(tile=False)
    # Stack: small round-cornered bricks, sooty (darker bricks) toward the top.
    bricks(c, seed, bw=16, bh=12, gap=2.5, r=2.5, y0=16, y1=88, x0=0, x1=S)
    soot = [SOOT[0], SOOT[1], SOOT[1], SOOT[2]]
    sooty = [shift_t(BRICK, -0.12), shift_t(BRICK, -0.22)]
    bricks(c, seed + 1, bw=16, bh=12, gap=2.5, r=2.5, y0=16, y1=28, x0=0, x1=S,
           t=sooty[1], mortar=MORTAR[0])
    bricks(c, seed + 2, bw=16, bh=12, gap=2.5, r=2.5, y0=28, y1=40, x0=0, x1=S,
           t=sooty[0], mortar=MORTAR[0])
    # Rim: a heavy capstone band (dark iron) with a soot lip.
    raised(c, box(0, 0, 16, S), DARK, 1, rim=2.0)
    c.fill(box(0, 0, 3, S), soot[2])
    for x in (32, 64, 96):
        rivet(c, 9.5, x, 2.4, IRON)
    # Slab: the dark iron base plate the stack stands on.
    raised(c, box(88, 0, S, S), DARK, 1, rim=2.5)
    c.fill(box(88, 0, 91, S), DARK[3])
    for x in (12, 64, 116):
        rivet(c, 108, x, 3.0, IRON, shadow=DARK[0])
    return c.finish(opaque=True)


def shift_t(t, dv):
    return [flat.shift(x, dv=dv, ds=-0.05) for x in t]


def paint_chimney_top(seed):
    c = Canvas(tile=False)
    # Slab: dark iron plate with corner rivets.
    raised(c, box(0, 0, S, S), DARK, 1, rim=2.5)
    for y in (10, S - 10):
        for x in (10, S - 10):
            rivet(c, y, x, 3.2, IRON, shadow=DARK[0])
    # The rim's top: a rounded brick ring around the flue.
    c.fill(box(25.5, 25.5, 105.5, 105.5, r=12), DARK[0])
    raised(c, box(24, 24, 104, 104, r=12), BRICK, 1, rim=2.5)
    # Mortar joints across the ring, so it reads as bricks.
    for (y0, x0, y1, x1) in ((24, 62, 40, 66), (88, 62, 104, 66), (62, 24, 66, 40),
                             (62, 88, 66, 104)):
        c.fill(box(y0, x0, y1, x1), MORTAR[0])
    # The flue: sooty, darker toward the middle in flat steps.
    c.fill(box(40, 40, 88, 88, r=7), SOOT[2])
    c.fill(box(43, 43, 88, 88, r=6), SOOT[1])
    c.fill(box(48, 48, 84, 84, r=5), SOOT[0])
    return c.finish(opaque=True)


# ---------------------------------------------------------------------------- advanced furnace


def paint_advanced_furnace_front(seed):
    c = Canvas(tile=False, bg=DARK[1])
    frame(c, 5)
    steel_panel(c, 5, 5, S - 5, S - 5, r=8, bolts=False)
    for y, x in ((12, 12), (12, S - 12)):
        bolt(c, y, x)
    opening(c, UP_ARCH, collar=6.0, n_rivets=4)
    opening(c, LOW_ARCH, collar=5.0, n_rivets=4)
    # A copper lintel between the two openings.
    raised(c, box(56, 14, 63, S - 14, r=2.5), COPPER, 2, rim=1.5, shadow=STEEL[0])
    for x in (22, 106):
        bolt(c, 59.5, x, 2.2)
    return c.finish(opaque=True)


def paint_advanced_furnace_side(seed):
    c = Canvas(tile=False, bg=DARK[1])
    frame(c, 5)
    steel_panel(c, 8, 8, 62, S - 8, r=7)
    steel_panel(c, 66, 8, S - 8, S - 8, r=7)
    # A recessed louvre strip in the lower panel.
    for k in range(4):
        y = 80 + k * 9
        c.fill(capsule((30, y), (98, y), 4.5), DARK[1])
        c.fill(capsule((30, y + 1.2), (98, y + 1.2), 2.2), DARK[0])
    return c.finish(opaque=True)


def paint_advanced_furnace_top(seed):
    c = Canvas(tile=False, bg=DARK[1])
    frame(c, 5)
    steel_panel(c, 8, 8, S - 8, S - 8, r=9)
    # A raised centre plate.
    raised(c, box(30, 30, 98, 98, r=8), STEEL, 1, rim=2.0)
    raised(c, box(36, 36, 92, 92, r=6), STEEL, 2, rim=1.5)
    return c.finish(opaque=True)


def paint_advanced_furnace_panel(seed):
    """Beside the furnace: a pressure gauge, two lamps and a copper pipe."""
    c = Canvas(tile=False, bg=DARK[1])
    frame(c, 5)
    steel_panel(c, 5, 5, S - 5, S - 5, r=8, bolts=False)
    for y, x in ((12, 12), (12, S - 12), (S - 12, 12), (S - 12, S - 12)):
        bolt(c, y, x)
    # Gauge: copper bezel, cream face, ticks, a dark needle.
    gy, gx, gr = 46, 64, 27
    c.fill(disk(gy + 2, gx + 2, gr, False), STEEL[0])
    raised(c, disk(gy, gx, gr, False), COPPER, 2, rim=2.5)
    face = disk(gy, gx, gr - 6, False)
    c.fill(face, hexc("efe9dc"))
    c.fill(flat.minus(face, disk(gy + 2, gx + 2, gr - 6, False)), hexc("cfc6b4"))
    for k in range(7):
        a = np.pi * (0.8 + 1.4 * k / 6)
        p0 = (gx + np.cos(a) * (gr - 12), gy + np.sin(a) * (gr - 12))
        p1 = (gx + np.cos(a) * (gr - 9), gy + np.sin(a) * (gr - 9))
        c.fill(capsule(p0, p1, 2.2), DARK[2])
    c.fill(poly([(gx + 15, gy - 9), (gx - 2.5, gy - 2.5), (gx + 2.5, gy + 2.5)]), hexc("c23b2a"))
    c.fill(disk(gy, gx, 3.5, False), DARK[1])
    # Two lamps: green (ready) and amber.
    for lx, col in ((30, hexc("6fbf73")), (52, hexc("ffcf5a"))):
        c.fill(disk(96 + 1, lx + 1, 7, False), STEEL[0])
        c.fill(disk(96, lx, 7, False), DARK[1])
        c.fill(disk(96, lx, 4.5, False), col)
        c.fill(disk(94.5, lx - 1.5, 1.6, False), hexc("ffffff"))
    # Copper pipe: out of the gauge's side, round an elbow and down out of the block.
    py0, py1, px0, px1 = 41.5, 50.5, 103.5, 112.5
    c.fill(box(py0 + 1.5, gx + 20, py1 + 1.5, px1 + 1.5, r=4), STEEL[0])
    c.fill(box(py0 + 1.5, px0 + 1.5, S - 5, px1 + 1.5), STEEL[0])
    elbow_h = box(py0, gx + 20, py1, px1, r=4)
    elbow_v = box(py0, px0, S - 5, px1, r=4)
    c.fill(elbow_h, COPPER[2])
    c.fill(elbow_v, COPPER[2])
    c.fill(box(py0, gx + 20, py0 + 2.5, px1 - 3), COPPER[3])
    c.fill(box(py0 + 3, px0, S - 5, px0 + 2.5), COPPER[3])
    c.fill(box(py1 - 2.5, gx + 20, py1, px0), COPPER[1])
    c.fill(box(py0 + 3, px1 - 2.5, S - 5, px1), COPPER[1])
    for y in (72, 104):  # couplings
        raised(c, box(y, px0 - 2, y + 5, px1 + 2, r=1.5), COPPER, 1, rim=1.0)
    return c.finish(opaque=True)


def hood(lit: bool) -> np.ndarray:
    """Both upper fronts together (256 x 128): one steel hood in a dark iron frame, a copper
    flame badge on the seam and a long vent grille across both, glowing while it burns."""
    W = 2 * S
    c = Canvas(w=W, h=S, tile=False, bg=DARK[1])
    frame(c, 5, W=W)
    raised(c, box(5, 5, S - 5, W - 5, r=8), STEEL, 2, rim=2.5)
    # A seam-less upper and lower plate, split by a groove.
    c.fill(box(58, 10, 60, W - 10), STEEL[0])
    c.fill(box(60, 10, 61.5, W - 10), STEEL[3])
    for y, x in ((12, 12), (12, W - 12), (S - 12, 12), (S - 12, W - 12)):
        bolt(c, y, x)
    # Grille: rounded dark frame with rounded slots.
    gy0, gy1, gx0, gx1 = 68, 114, 20, W - 20
    c.fill(box(gy0 + 1.5, gx0 + 1.5, gy1 + 1.5, gx1 + 1.5, r=7), STEEL[0])
    raised(c, box(gy0, gx0, gy1, gx1, r=7), DARK, 1, rim=2.0)
    for k in range(4):
        y = gy0 + 10 + k * 9
        slot = capsule((gx0 + 10, y), (gx1 - 10, y), 5.5)
        if lit:
            c.fill(slot, GLOW[0])
            c.fill(capsule((gx0 + 30, y), (gx1 - 30, y), 3.6), GLOW[1])
            c.fill(capsule((W / 2 - 55, y - 0.4), (W / 2 + 55, y - 0.4), 1.8), GLOW[2])
        else:
            c.fill(slot, SOOT[0])
            c.fill(capsule((gx0 + 10, y - 1.6), (gx1 - 10, y - 1.6), 2.0), SOOT[2])
    # Badge on the seam: a copper rounded plate with a flame.
    bx0, bx1, by0, by1 = S - 22, S + 22, 14, 52
    c.fill(box(by0 + 2, bx0 + 2, by1 + 2, bx1 + 2, r=7), STEEL[0])
    raised(c, box(by0, bx0, by1, bx1, r=7), COPPER, 2, rim=2.0)
    fy, fx = (by0 + by1) / 2 + 2, S
    outer = flame_shape(fy, fx, 11, 15)
    inner = flame_shape(fy + 5, fx, 6, 8)
    if lit:
        c.fill(outer, GLOW[0])
        c.fill(inner, GLOW[2])
    else:
        c.fill(outer, COPPER[0])
        c.fill(inner, COPPER[1])
    for x in (bx0 + 6, bx1 - 6):
        for y in (by0 + 6, by1 - 6):
            c.fill(disk(y, x, 2.0, False), COPPER[3])
    return c.finish(opaque=True)


def flame_shape(cy, cx, rx, h):
    """A flat flame: a round bottom with a pointed top."""
    body = disk(cy + h * 0.25, cx, rx, False)
    tip = poly([(cx - rx * 0.95, cy + h * 0.15), (cx + rx * 0.2, cy - h), (cx + rx * 0.95,
                cy + h * 0.15)])
    return union(body, tip)


_HOOD: dict[bool, np.ndarray] = {}


def hood_half(right: bool, lit: bool):
    if lit not in _HOOD:
        _HOOD[lit] = hood(lit)
    img = _HOOD[lit]
    return np.ascontiguousarray(img[:, S:] if right else img[:, :S]).copy()


def paint_advanced_furnace_vent_top(seed):
    """Top of the hood: a square vent grating on the steel."""
    c = Canvas(tile=False, bg=DARK[1])
    frame(c, 5)
    steel_panel(c, 8, 8, S - 8, S - 8, r=9)
    c.fill(box(26 + 1.5, 26 + 1.5, 102 + 1.5, 102 + 1.5, r=10), STEEL[0])
    raised(c, box(26, 26, 102, 102, r=10), DARK, 1, rim=2.0)
    for k in range(6):
        y = 36 + k * 11.2
        c.fill(capsule((38, y), (90, y), 6), SOOT[0])
        c.fill(capsule((38, y - 1.8), (90, y - 1.8), 2.2), SOOT[2])
    return c.finish(opaque=True)


TEXTURES = {
    "block/rc_blast_furnace_front": paint_blast_front,
    "block/rc_blast_furnace_side": paint_blast_side,
    "block/rc_blast_furnace_top": paint_blast_top,
    "block/rc_chimney_side": paint_chimney_side,
    "block/rc_chimney_top": paint_chimney_top,
    "block/rc_advanced_furnace_front": paint_advanced_furnace_front,
    "block/rc_advanced_furnace_side": paint_advanced_furnace_side,
    "block/rc_advanced_furnace_top": paint_advanced_furnace_top,
    "block/rc_advanced_furnace_panel": paint_advanced_furnace_panel,
    "block/rc_advanced_furnace_hood_left": lambda seed: hood_half(False, False),
    "block/rc_advanced_furnace_hood_right": lambda seed: hood_half(True, False),
    "block/rc_advanced_furnace_hood_left_on": lambda seed: hood_half(False, True),
    "block/rc_advanced_furnace_hood_right_on": lambda seed: hood_half(True, True),
    "block/rc_advanced_furnace_vent_top": paint_advanced_furnace_vent_top,
}
