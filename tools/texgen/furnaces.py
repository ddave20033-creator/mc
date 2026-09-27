"""The better furnaces: the blast furnace (bricks bound with copper, with a brick chimney on
top) and the advanced furnace (riveted steel, two blocks wide and two tall: the furnace,
a gauge panel beside it and a hood with a vent grille over both, glowing while it burns).

Drawn like crafted.py (flat shading, few colors, 128 x 128). The furnace fronts keep the
furnace's two openings exactly (`UP_ARCH`, `LOW_ARCH`): the game cuts them out between their
dark outlines (so nothing else on a front may be that dark: r + g + b < 60) and puts the
model's hollows behind them."""

from __future__ import annotations

import numpy as np

from common import S, Ramp, disk, grow, pix, rect, rgba, rng, thick_line
from crafted import (
    BRICK, COPPER, LOW_ARCH, MORTAR, STONE, UP_ARCH, anoise, arch, ints, level,
    paint_bricks, stepped,
)

STEEL = Ramp("2c2f35", "363a41", "40454d", "4b5059", "575d67", "646b76", "737b87", "86909c",
             "9ba4b0", "b3bbc5")
BRASS = Ramp("5e4514", "7d5c1b", "a07824", "c09430", "d8b04a", "ecd07a")
SOOT = Ramp("141414", "1c1b1a", "262422", "302d2a", "3b3733")
# Opening insides: dark enough for the game to find the openings' outlines.
HOLE = Ramp("0d0d0d", "151515")


def rgb_of(img):
    return img[..., :3].copy()


def put(col, mask, color):
    col[mask] = color
    return col


def rivets(col, points, r=3.2, ramp=STEEL, hi=8, mid=6, lo=3):
    """Round rivet heads lit from the top left (no pixel darker than the steel's darkest)."""
    yy, xx = ints()
    for cy, cx in points:
        m = disk(cy, cx, r)
        col[m] = ramp.at(mid)
        col[m & ((yy - cy) + (xx - cx) > 1.2)] = ramp.at(lo)
        col[disk(cy - 1, cx - 1, r * 0.45)] = ramp.at(hi)
    return col


def bevel(col, mask, ramp, light, dark, w=2):
    """Lit top/left and shaded bottom/right edges of `mask`."""
    inner = ~grow(~mask, w) & mask
    edge = mask & ~inner
    yy, xx = ints()
    ys, xs = np.nonzero(mask)
    if len(ys) == 0:
        return col
    cy, cx = ys.mean(), xs.mean()
    top_left = edge & (((yy - cy) / max(1, np.ptp(ys))) + ((xx - cx) / max(1, np.ptp(xs))) < 0)
    col[top_left] = ramp.at(light)
    col[edge & ~top_left] = ramp.at(dark)
    return col


def steel_value(seed, lo=4, hi=5):
    """Brushed steel: fine horizontal streaks and a few soft blotches."""
    yy, xx = ints()
    streak = anoise(seed, 2, 40)
    blot = anoise(seed + 1, 24, 24)
    t = level(STEEL, lo) + (streak - 0.5) * 0.12 + (blot - 0.5) * 0.1
    t += (pix(seed + 2, 2) - 0.5) * 0.05
    return np.clip(t, level(STEEL, 1), level(STEEL, hi))


def steel_plates(seed, cells, frame=4, rivet_step=24):
    """Steel sheet split into plates (y0, x0, y1, x1), each bevelled, with rivets along its
    edges, and a heavy frame round the block."""
    col = stepped(steel_value(seed), STEEL, 0.3)
    for k, (y0, x0, y1, x1) in enumerate(cells):
        m = rect(y0, x0, y1, x1)
        col = bevel(col, m, STEEL, 7, 2, 2)
        pts = []
        for y in (y0 + 6, y1 - 6):
            for x in np.arange(x0 + 7, x1 - 4, rivet_step):
                pts.append((y, x))
        col = rivets(col, pts, r=2.6)
    yy, xx = ints()
    f = (yy < frame) | (yy >= S - frame) | (xx < frame) | (xx >= S - frame)
    col[f] = STEEL.at(2)
    col[f & ((yy < 2) | (xx < 2))] = STEEL.at(5)
    return col


# ---------------------------------------------------------------------------- blast furnace


def copper_band(col, y0, y1, seed, rivet_every=32, x0=0, x1=S):
    """A copper strap across the texture, with rivets."""
    yy, xx = ints()
    m = rect(y0, x0, y1, x1)
    t = level(COPPER, 5) + (anoise(seed, 2, 20) - 0.5) * 0.2
    col[m] = stepped(t, COPPER, 0.3)[m]
    col[m & (yy == y0)] = COPPER.at(9)
    col[m & (yy == y1 - 1)] = COPPER.at(1)
    cy = (y0 + y1) / 2
    pts = [(cy, x) for x in np.arange(x0 + rivet_every / 2, x1, rivet_every)]
    return rivets(col, pts, r=min(3.0, (y1 - y0) / 2 - 1), ramp=COPPER, hi=10, mid=6, lo=2)


def copper_post(col, x0, x1, seed):
    yy, xx = ints()
    m = rect(0, x0, S, x1)
    t = level(COPPER, 5) + (anoise(seed, 20, 2) - 0.5) * 0.2
    col[m] = stepped(t, COPPER, 0.3)[m]
    col[m & (xx == x0)] = COPPER.at(9)
    col[m & (xx == x1 - 1)] = COPPER.at(1)
    cx = (x0 + x1) / 2
    return rivets(col, [(y, cx) for y in (12, 44, 76, 108)], r=2.6, ramp=COPPER, hi=10,
                  mid=6, lo=2)


def voussoirs(col, opening, width, seed):
    """Wedge bricks round an arched opening, a keystone on top."""
    yy, xx = ints()
    ring = grow(opening, width) & ~opening
    ys, xs = np.nonzero(opening)
    cy, cx = ys.max(), xs.mean()
    ang = np.arctan2(yy - cy, xx - cx)
    seg = np.floor((ang + np.pi) / (np.pi / 9)).astype(int)
    per = rng(seed).uniform(-0.08, 0.08, 20).astype(np.float32)
    t = level(BRICK, 4) + per[seg % 20] + (anoise(seed + 1, 4, 4) - 0.5) * 0.15
    col[ring] = stepped(t, BRICK, 0.3)[ring]
    joint = ring & (np.abs(((ang + np.pi) % (np.pi / 9)) - np.pi / 18) > np.pi / 18 - 0.035)
    col[joint] = MORTAR.at(0)
    col[ring & ~grow(~ring, 1) & ~grow(opening, 1)] = BRICK.at(1)
    # Keystone.
    key = ring & (np.abs(xx - cx) < 7) & (yy < cy)
    col[key] = BRICK.at(5)
    col[key & (xx < cx - 5)] = BRICK.at(3)
    return col


def blast_front_base(seed=7301):
    yy, xx = ints()
    col = rgb_of(paint_bricks(seed))
    up = arch(*UP_ARCH, 56)
    low = arch(*LOW_ARCH, 124)
    col = voussoirs(col, up, 7, seed + 1)
    col = voussoirs(col, low, 6, seed + 2)
    # A copper lintel under the upper arch, straps at the top and a plinth at the bottom.
    col = copper_band(col, 56, 62, seed + 3, rivet_every=21, x0=16, x1=112)
    col = copper_band(col, 0, 9, seed + 4)
    sill = rect(124, 0, S, S)
    col[sill] = COPPER.at(3)
    col[sill & (yy == 124)] = COPPER.at(8)
    col = copper_post(col, 0, 7, seed + 5)
    col = copper_post(col, S - 7, S, seed + 6)
    # The openings, dark inside (the upper one with a faint glow of soot).
    core = (((yy - 56) / 13.0) ** 2 + ((xx - 64) / 24.0) ** 2 <= 1)
    col[up] = HOLE.at(0)
    col[up & core] = HOLE.at(1)
    col[low] = HOLE.at(0)
    return col


def paint_blast_front(seed):
    return rgba(blast_front_base(), 255)


def paint_blast_side(seed):
    yy, xx = ints()
    col = rgb_of(paint_bricks(seed + 11))
    col = copper_band(col, 0, 9, seed + 1)
    col = copper_band(col, 60, 68, seed + 2, rivet_every=21)
    col = copper_post(col, 0, 7, seed + 3)
    col = copper_post(col, S - 7, S, seed + 4)
    sill = rect(120, 0, S, S)
    col[sill] = COPPER.at(3)
    col[sill & (yy == 120)] = COPPER.at(8)
    return rgba(col, 255)


def paint_blast_top(seed):
    yy, xx = ints()
    col = rgb_of(paint_bricks(seed + 21))
    edge = (yy < 7) | (yy >= S - 7) | (xx < 7) | (xx >= S - 7)
    t = level(COPPER, 4) + (anoise(seed, 4, 4) - 0.5) * 0.2
    col[edge] = stepped(t, COPPER, 0.3)[edge]
    col[edge & ((yy < 2) | (xx < 2))] = COPPER.at(9)
    return rgba(col, 255)


def paint_chimney_side(seed):
    """Small bricks, sooty toward the top."""
    yy, xx = ints()
    course = yy // 16
    ly = yy % 16
    off = np.where(course % 2 == 0, 16, 0)
    bx = (xx + off) % S
    lx = bx % 32
    bid = (course * 4 + bx // 32) % 32
    per = rng(seed).uniform(-0.07, 0.07, 32).astype(np.float32)
    t = level(BRICK, 3) + per[bid] + (anoise(seed + 1, 4, 8) - 0.5) * 0.2
    t = np.where(ly < 2, t + 0.12, t)
    t = np.where(ly >= 11, t - 0.1, t)
    col = stepped(t, BRICK, 0.3)
    mortar = (ly >= 14) | (lx >= 30)
    col[mortar] = MORTAR.at(0)
    # Soot: darkening to the top, in streaks.
    soot = np.clip((72 - yy) / 72.0, 0, 1) * (0.55 + 0.45 * anoise(seed + 2, 30, 6))
    k = np.clip(soot, 0, 0.8)[..., None]
    col = col * (1 - k) + SOOT.at(3) * k
    return rgba(col, 255)


def paint_chimney_top(seed):
    """The slab (stone), the stack's rim, and the sooty hole in the middle."""
    yy, xx = ints()
    t = level(STONE, 5) + (anoise(seed, 8, 8) - 0.5) * 0.25 + (pix(seed + 1, 2) - 0.5) * 0.08
    col = stepped(t, STONE, 0.3)
    rim = rect(24, 24, 104, 104)
    t2 = level(BRICK, 2) + (anoise(seed + 2, 4, 4) - 0.5) * 0.2
    col[rim] = stepped(t2, BRICK, 0.3)[rim]
    col[rim & ((yy == 24) | (xx == 24))] = BRICK.at(5)
    hole = rect(40, 40, 88, 88)
    d = np.minimum.reduce([yy - 40, xx - 40, 87 - yy, 87 - xx]).astype(np.float32)
    ts = np.clip(1 - d / 10.0, 0, 1)
    col[hole] = SOOT.shade(ts, 0.4)[hole]
    return rgba(col, 255)


# ---------------------------------------------------------------------------- advanced furnace


def collar(col, opening, width, seed):
    """A heavy bolted steel collar round an opening."""
    yy, xx = ints()
    ring = grow(opening, width) & ~opening
    t = level(STEEL, 3) + (anoise(seed, 3, 3) - 0.5) * 0.1
    col[ring] = stepped(t, STEEL, 0.3)[ring]
    outer = ring & ~grow(~ring, 1)
    col[outer & ~grow(opening, 2)] = STEEL.at(6)
    col[ring & grow(opening, 1) & ~opening] = STEEL.at(1)
    ys, xs = np.nonzero(ring)
    pts = []
    cy, cx = np.nonzero(opening)[0].max(), np.nonzero(opening)[1].mean()
    for a in np.linspace(np.pi * 1.05, np.pi * 1.95, 5):
        ry = (ys.max() - ys.min()) - width * 0.5
        rx = (xs.max() - xs.min()) / 2 - width * 0.5
        pts.append((cy + np.sin(a) * ry, cx + np.cos(a) * rx))
    return rivets(col, pts, r=2.4)


def paint_advanced_furnace_front(seed):
    yy, xx = ints()
    col = steel_plates(seed, [(4, 4, 124, 124)], frame=5)
    up = arch(*UP_ARCH, 56)
    low = arch(*LOW_ARCH, 124)
    col = collar(col, up, 7, seed + 1)
    col = collar(col, low, 6, seed + 2)
    # Brass kick plate and a brass lintel.
    kick = rect(124, 0, S, S)
    col[kick] = BRASS.at(2)
    col[kick & (yy == 124)] = BRASS.at(5)
    lintel = rect(56, 14, 62, 114)
    col[lintel] = BRASS.at(3)
    col[lintel & (yy == 56)] = BRASS.at(5)
    col[lintel & (yy == 61)] = BRASS.at(1)
    col = rivets(col, [(59, x) for x in (20, 42, 86, 108)], r=2.0, ramp=BRASS, hi=5, mid=3,
                 lo=1)
    col = rivets(col, [(10, 10), (10, 118), (118, 10), (118, 118)], r=3.4)
    core = (((yy - 56) / 13.0) ** 2 + ((xx - 64) / 24.0) ** 2 <= 1)
    col[up] = HOLE.at(0)
    col[up & core] = HOLE.at(1)
    col[low] = HOLE.at(0)
    return rgba(col, 255)


def paint_advanced_furnace_side(seed):
    col = steel_plates(seed, [(4, 4, 64, 64), (4, 64, 64, 124), (64, 4, 124, 64),
                              (64, 64, 124, 124)])
    yy, xx = ints()
    # A vertical rib down the middle.
    rib = rect(0, 58, S, 70)
    col[rib] = STEEL.at(5)
    col[rib & (xx == 58)] = STEEL.at(8)
    col[rib & (xx == 69)] = STEEL.at(2)
    col = rivets(col, [(y, 64) for y in (14, 40, 88, 114)], r=2.8)
    return rgba(col, 255)


def paint_advanced_furnace_top(seed):
    """Tread plate: raised diagonal bumps in a steel frame."""
    yy, xx = ints()
    col = stepped(steel_value(seed, 4, 5), STEEL, 0.3)
    u = (xx + yy) % 16
    v = (xx - yy) % 32
    bump = (np.abs(u - 8) < 2) & (np.abs(v - 16) < 6) & ((yy // 16 + xx // 16) % 2 == 0)
    bump2 = (np.abs(((xx - yy) % 16) - 8) < 2) & (np.abs(((xx + yy) % 32) - 16) < 6) & \
            ((yy // 16 + xx // 16) % 2 == 1)
    col[bump | bump2] = STEEL.at(6)
    col[(bump | bump2) & ~grow(~(bump | bump2), 1)] = STEEL.at(7)
    f = (yy < 5) | (yy >= S - 5) | (xx < 5) | (xx >= S - 5)
    col[f] = STEEL.at(2)
    col[f & ((yy < 2) | (xx < 2))] = STEEL.at(5)
    return rgba(col, 255)


def paint_advanced_furnace_panel(seed):
    """Beside the furnace: a pressure gauge, a valve wheel and a copper pipe."""
    yy, xx = ints()
    col = steel_plates(seed, [(4, 4, 124, 124)], frame=5)
    # Gauge: brass bezel, white face, ticks and a red needle.
    gy, gx, gr = 44, 64, 26
    col[disk(gy, gx, gr)] = BRASS.at(3)
    col[disk(gy, gx, gr) & ~disk(gy + 1, gx + 1, gr - 1)] = BRASS.at(5)
    face = disk(gy, gx, gr - 5)
    col[face] = [230, 226, 212]
    col[face & ~disk(gy - 1, gx - 1, gr - 7)] = [196, 190, 172]
    for k in range(9):
        a = np.pi * (0.75 + 1.5 * k / 8)
        p0 = (gx + np.cos(a) * (gr - 10), gy + np.sin(a) * (gr - 10))
        p1 = (gx + np.cos(a) * (gr - 7), gy + np.sin(a) * (gr - 7))
        col[thick_line(p0, p1, 2.2) & face] = [70, 70, 76]
    red = thick_line((gx - 3, gy + 3), (gx + 14, gy - 10), 2.6) & face
    col[red] = [200, 44, 36]
    col[disk(gy, gx, 3)] = [90, 90, 96]
    # Valve wheel.
    vy, vx, vr = 94, 38, 17
    wheel = disk(vy, vx, vr) & ~disk(vy, vx, vr - 4)
    for a in (0, np.pi / 3, 2 * np.pi / 3):
        wheel |= thick_line((vx - np.cos(a) * vr, vy - np.sin(a) * vr),
                            (vx + np.cos(a) * vr, vy + np.sin(a) * vr), 3.5)
    col[wheel] = [178, 40, 34]
    col[wheel & ((yy - vy) + (xx - vx) < -6)] = [220, 80, 64]
    col[disk(vy, vx, 4)] = BRASS.at(4)
    # Copper pipe along the bottom, down from the right.
    pipe = rect(100, 60, 112, 124) | rect(70, 100, 112, 112)
    col[pipe] = COPPER.at(5)
    col[pipe & (((yy == 100) & (xx >= 60)) | ((xx == 100) & (yy < 100)))] = COPPER.at(9)
    col[pipe & (((yy == 111) & (xx >= 60)) | ((xx == 111) & (yy < 111)))] = COPPER.at(1)
    col[rect(98, 66, 114, 70)] = COPPER.at(3)
    # A little lamp.
    col[disk(96, 84, 5)] = [60, 120, 70]
    col[disk(95, 83, 2)] = [150, 230, 160]
    return rgba(col, 255)


def hood(seed, lit):
    """Both upper fronts together (256 x 128): a steel hood with a brass flame badge at the
    seam and a vent grille across both, glowing orange while the furnace burns."""
    W = 2 * S
    yy, xx = np.mgrid[0:S, 0:W]
    col = np.zeros((S, W, 3), np.float32)
    for h, x0 in ((0, 0), (1, S)):
        col[:, x0:x0 + S] = steel_plates(seed + h, [(4, 4, 60, 124), (60, 4, 124, 124)],
                                         frame=5)
    # The frame edges at the seam belong to one big hood: continuous plate there.
    seam = (xx >= S - 5) & (xx < S + 5) & (yy >= 5) & (yy < S - 5)
    col[seam] = STEEL.at(4)
    # Grille: horizontal slots in a bolted frame.
    gy0, gy1, gx0, gx1 = 70, 110, 16, W - 16
    frame = (yy >= gy0 - 5) & (yy < gy1 + 5) & (xx >= gx0 - 5) & (xx < gx1 + 5)
    col[frame] = STEEL.at(2)
    col[frame & ((yy == gy0 - 5) | (xx == gx0 - 5))] = STEEL.at(6)
    slots = (yy >= gy0) & (yy < gy1) & (xx >= gx0) & (xx < gx1) & ((yy - gy0) % 8 < 5)
    bars = (yy >= gy0) & (yy < gy1) & (xx >= gx0) & (xx < gx1) & ~slots
    col[bars] = STEEL.at(5)
    col[bars & ((yy - gy0) % 8 == 5)] = STEEL.at(7)
    if lit:
        # Hot in the middle, cooler toward the ends.
        u = np.clip(1 - np.abs(xx - W / 2) / (W / 2 - 16), 0, 1)
        v = ((yy - gy0) % 8) / 5.0
        t = 0.25 + 0.6 * u - 0.15 * v + (anoise(seed + 9, 6, 20, S, W) - 0.5) * 0.25
        col[slots] = _glow(np.clip(t, 0, 1))[slots]
    else:
        col[slots] = SOOT.at(2)
        col[slots & ((yy - gy0) % 8 == 0)] = SOOT.at(4)
    # Brass badge with a flame, centred on the seam.
    badge = (np.abs(xx - S) < 22) & (yy >= 16) & (yy < 52)
    col[badge] = BRASS.at(3)
    col[badge & ((yy == 16) | (xx == S - 22))] = BRASS.at(5)
    col[badge & ((yy == 51) | (xx == S + 21))] = BRASS.at(1)
    fy, fx = yy - 34.0, xx - S
    flame = ((fx / 10) ** 2 + ((fy - 4) / 13) ** 2 < 1) & ~(((fx - 5) / 7) ** 2 + ((fy + 6) / 10) ** 2 < 1)
    col[flame & badge] = [214, 88, 30] if lit else BRASS.at(1)
    inner = ((fx / 5) ** 2 + ((fy - 7) / 7) ** 2 < 1)
    col[inner & badge] = [255, 200, 80] if lit else BRASS.at(2)
    for x in (S - 17, S + 16):
        for y in (21, 47):
            col[disk(y, x, 2.2, S, W)] = BRASS.at(5)
    return col


def _glow(t):
    # Deep red to orange to yellow-white.
    stops = np.array([[120, 30, 10], [200, 70, 16], [246, 140, 40], [255, 206, 110],
                      [255, 244, 200]], np.float32)
    x = t * (len(stops) - 1)
    i = np.clip(np.floor(x).astype(int), 0, len(stops) - 2)
    f = (x - i)[..., None]
    return stops[i] * (1 - f) + stops[i + 1] * f


def hood_half(right, lit):
    col = hood(8801, lit)
    half = col[:, S:] if right else col[:, :S]
    return rgba(np.ascontiguousarray(half), 255)


def paint_advanced_furnace_vent_top(seed):
    """Top of the hood: a square vent grating between rivets."""
    yy, xx = ints()
    col = rgb_of(paint_advanced_furnace_top(seed))
    g = rect(28, 28, 100, 100)
    col[g] = STEEL.at(2)
    col[g & ((yy == 28) | (xx == 28))] = STEEL.at(6)
    holes = rect(32, 32, 96, 96) & ((yy - 32) % 8 < 5) & ((xx - 32) % 8 < 5)
    col[holes] = SOOT.at(1)
    col = rivets(col, [(22, 22), (22, 106), (106, 22), (106, 106)], r=3.0)
    return rgba(col, 255)


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
