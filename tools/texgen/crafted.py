"""Crafted blocks: planks, glass, bricks, stone bricks, crafting table, furnace, storage
blocks, torch, lantern, chain, oak door, wool, bed and the block breaking cracks.

Drawn after the Faithful 64x look (composition, palette, flat shading with few colors) at
128x128 with 1-2 px detail. Coordinates below are in 128 px texture pixels."""

from __future__ import annotations

import numpy as np
from scipy import ndimage

from common import (
    S, Ramp, blank, edge_light, grid, grow, hexc, noise, fbm, paint, pix, polygon, rect,
    rgba, rng, thick_line, shrink,
)

# ---------------------------------------------------------------------------- palettes

PLANK = Ramp("67502c", "7e6237", "967441", "9f844d", "af8f55", "b8945f", "c29d62")
STONE = Ramp("3c3b3b", "504e4e", "5d5b5b", "686868", "777777", "858585", "919191",
             "9d9d9d", "a8a8a8", "b0b0b0", "c5c5c5")
SBRICK = Ramp("5a595a", "636363", "6a6d6a", "787678", "7f7f7f", "8b898b", "9c999c")
BRICK = Ramp("733f31", "7c4536", "8f503f", "9b5643", "b1624d", "c66851")
MORTAR = Ramp("8b6e67", "a2867d", "a9948d")
IRON = Ramp("b1b0b0", "b9b9b9", "c1c1c1", "d1cfcf", "d6d6d6", "dcdcdc", "e0e0e0", "e6e6e6",
            "eaeaea", "ececec", "f2f2f2")
GOLD = Ramp("cc8e27", "d39632", "f9bd23", "f5cc27", "ffd83e", "fee048", "ffec4f", "fffd90",
            "feffbd")
DIAMOND = Ramp("0ebabd", "15c2c6", "3de0e5", "4bede6", "65f5e3", "70fbf0", "9efeeb", "d5fff6",
               "ffffff")
COAL = Ramp("050505", "0d0d0d", "151515", "1f1e1e", "292828")
WOOL = Ramp("d1d7d8", "dbe0e1", "e4e7e8", "eeeff0", "f4f5f6", "fafbfb", "fefefe")
RED_TOP = Ramp("6b1213", "851a1a", "902120", "a22722", "ac2922", "b53129", "bf3b33")
LANT = Ramp("252c3d", "3e4453", "424a5e", "495065", "5a6278")
GLOW = Ramp("814023", "8b5230", "c36322", "f09149", "f9c966", "fdfd8b", "ffffd5")
TWOOD = Ramp("372a17", "423522", "55452e", "6d5736", "81663e", "957546", "9f7f50")
FLAME = Ramp("ff8f00", "ffd800", "ffff97", "ffffff")
GLASS_C = Ramp("7baeb7", "8bc1cd", "a8d0d9", "d0eae9")
TABLE_RED = Ramp("41230e", "4b2b18", "553824", "733920", "9e5932", "ae693c")
TABLE_CORNER = Ramp("a98958", "bc9862")
DARKWOOD = Ramp("0e0b06", "19140c", "281e0b", "382116", "41230e", "4b2b18", "5a2e17")
DOOR = Ramp("513d24", "67502c", "7e6237", "967441", "a6824d", "b58d50", "b8945f")
HINGE = Ramp("4a4e56", "686c77", "757d87", "808b95", "838e98", "a4adb5")
BEDWOOD = Ramp("695433", "7c623e", "9b7742", "9f844d", "b38c51", "bc9862")
SHEET = Ramp("a8b0b1", "c2c2c2", "c7d3d3", "d8dfdf", "ececec", "ffffff")
TOOL_GREY = Ramp("8e8e8e", "b5b5b5", "d8d8d8", "f0f0f0")


# ---------------------------------------------------------------------------- helpers


def anoise(seed: int, cy: float, cx: float, h: int = S, w: int = S) -> np.ndarray:
    """Tiling value noise with features `cy` tall and `cx` wide."""
    gh, gw = max(1, int(round(h / cy))), max(1, int(round(w / cx)))
    g = rng(seed).random((gh + 1, gw + 1)).astype(np.float32)
    g[gh, :] = g[0, :]
    g[:, gw] = g[:, 0]
    ys = np.linspace(0, gh, h, endpoint=False, dtype=np.float32)
    xs = np.linspace(0, gw, w, endpoint=False, dtype=np.float32)
    y0, x0 = np.floor(ys).astype(int), np.floor(xs).astype(int)
    fy, fx = ys - y0, xs - x0
    fy = fy * fy * (3 - 2 * fy)
    fx = fx * fx * (3 - 2 * fx)
    a, b = g[y0][:, x0], g[y0][:, x0 + 1]
    c, d = g[y0 + 1][:, x0], g[y0 + 1][:, x0 + 1]
    top = a + (b - a) * fx[None, :]
    bot = c + (d - c) * fx[None, :]
    return top + (bot - top) * fy[:, None]


def ints():
    return np.mgrid[0:S, 0:S]


def level(ramp: Ramp, i: float) -> float:
    """Shade value that lands on palette entry i."""
    return i / (len(ramp) - 1)


def img_of(rgb3):
    return rgba(rgb3, 255)


def stepped(t, ramp, dither=0.25):
    return ramp.shade(t, dither)


def warp(seed, amp, cell):
    """Displaced pixel coordinates (tiling) for wobbly shapes."""
    yy, xx = grid()
    return (yy + (noise(seed, cell) - 0.5) * 2 * amp,
            xx + (noise(seed + 1, cell) - 0.5) * 2 * amp)


def cells(seed, count, jitter=0.9, wobble=0.0, cell=32):
    """Tiling Voronoi on (optionally warped) coordinates: nearest id, second id,
    distance to the border between cells."""
    r = rng(seed)
    side = int(np.ceil(np.sqrt(count)))
    pts = []
    for i in range(side):
        for j in range(side):
            if len(pts) >= count:
                break
            pts.append(((i + 0.5 + (r.random() - 0.5) * jitter) * S / side,
                        (j + 0.5 + (r.random() - 0.5) * jitter) * S / side))
    if wobble:
        yy, xx = warp(seed + 7, wobble, cell)
    else:
        yy, xx = grid()
    best = np.full((S, S), 1e9, np.float32)
    second = np.full((S, S), 1e9, np.float32)
    ids = np.zeros((S, S), np.int32)
    ids2 = np.zeros((S, S), np.int32)
    for k, (py, px) in enumerate(pts):
        for oy in (-S, 0, S):
            for ox in (-S, 0, S):
                d = np.hypot(yy - (py + oy), xx - (px + ox))
                closer = d < best
                sec = (~closer) & (d < second)
                ids2 = np.where(closer, ids, np.where(sec, k, ids2))
                second = np.where(closer, best, np.where(sec, d, second))
                ids = np.where(closer, k, ids)
                best = np.where(closer, d, best)
    return ids, ids2, (second - best) * 0.5, best


def shifted(a, dy, dx):
    return np.roll(np.roll(a, dy, 0), dx, 1)


# ---------------------------------------------------------------------------- planks


def wood_grain(seed, h=S, w=S):
    """Faithful-like plank grain value around PLANK entries 4..5 with darker streaks."""
    t = np.full((h, w), (level(PLANK, 4) + level(PLANK, 5)) / 2, np.float32)
    n1 = anoise(seed, 8, 40, h, w)
    t += (n1 - 0.5) * 0.35
    n2 = anoise(seed + 1, 3, 20, h, w)
    t = np.where(n2 < 0.22, t - 0.2, t)
    t = np.where((n2 > 0.3) & (n2 < 0.36), t - 0.1, t)
    n3 = anoise(seed + 2, 4, 26, h, w)
    t = np.where(n3 > 0.8, t + 0.14, t)
    t += (pix(seed + 3, 2, h, w) - 0.5) * 0.05
    return t


def plank_value(seed, seams=(128, 64, 128, 64)):
    yy, xx = ints()
    t = wood_grain(seed)
    ly = yy % 32
    row = yy // 32
    t = np.where(ly < 2, t + 0.1, t)
    t = np.where((ly >= 22) & (ly < 28), t - 0.07, t)
    gap1 = (ly >= 28) & (ly < 30)
    gap2 = ly >= 30
    t = np.where(gap1, level(PLANK, 0) + (pix(seed + 5, 2) > 0.8) * 0.17, t)
    t = np.where(gap2, level(PLANK, 1) + (pix(seed + 6, 2) > 0.6) * 0.17, t)
    sx = np.array(seams)[row]
    dx = xx - sx
    body = ly < 28
    t = np.where(body & (dx >= -4) & (dx < -2), level(PLANK, 3), t)
    t = np.where(body & (dx >= -2) & (dx < 0), level(PLANK, 1), t)
    t = np.where(body & (dx >= 0) & (dx < 2) & (sx < 128), t + 0.08, t)
    return t


def paint_oak_planks(seed):
    return img_of(stepped(plank_value(seed), PLANK))


def paint_bed_down(seed):
    return img_of(stepped(plank_value(seed + 3, seams=(64, 128, 64, 128)), PLANK))


# ---------------------------------------------------------------------------- glass


def paint_glass(seed):
    img = blank()
    yy, xx = ints()
    t = np.zeros((S, S), np.float32)
    frame = (xx < 4) | (yy < 4) | (xx >= S - 4) | (yy >= S - 4)
    t[:] = level(GLASS_C, 3)
    rb = (xx >= S - 4) | (yy >= S - 4)
    t = np.where(rb, level(GLASS_C, 1), t)
    t = np.where(rb & (((xx >= S - 4) & (xx < S - 2)) | ((yy >= S - 4) & (yy < S - 2))),
                 level(GLASS_C, 2), t)
    t = np.where((xx >= S - 2) & (yy > S // 2), level(GLASS_C, 0), t)
    t = np.where((yy < 4) & (xx < 4), level(GLASS_C, 3), t)
    t = np.where((yy < 4) & (xx >= S - 4), level(GLASS_C, 2), t)
    paint(img, frame, GLASS_C.shade(t, 0))
    # Two thin diagonal glints (2 px staircase lines).
    glint = np.zeros((S, S), bool)
    for (x0, y0, n) in [(38, 16, 22), (112, 94, 16)]:
        for k in range(n):
            glint |= rect(y0 + k, x0 - k - 1, y0 + k + 1, x0 - k + 1)
    paint(img, glint & ~frame, GLASS_C.at(3))
    return img


# ---------------------------------------------------------------------------- bricks


def paint_bricks(seed):
    yy, xx = ints()
    course = yy // 32
    ly = yy % 32
    off = np.where(course % 2 == 0, 32, 0)
    bx = (xx + off) % S
    lx = bx % 64
    bid = course * 2 + bx // 64
    mortar = (ly >= 28) | (lx >= 60)
    per = rng(seed).uniform(-0.06, 0.06, 8).astype(np.float32)
    t = level(BRICK, 3) + per[bid] + (anoise(seed + 1, 6, 14) - 0.5) * 0.22
    t += (pix(seed + 2, 2) - 0.5) * 0.06
    t = np.where(ly < 2, level(BRICK, 5) - 0.05 + (pix(seed + 3, 2) > 0.6) * 0.15, t)
    t = np.where((ly >= 2) & (ly < 4), t + 0.12, t)
    t = np.where((ly >= 22) & (ly < 26), t - 0.12, t)
    t = np.where((ly >= 26) & (ly < 28), level(BRICK, 0) + (pix(seed + 4, 2) > 0.7) * 0.2, t)
    t = np.where((lx >= 54) & (lx < 60) & (ly < 28), t - 0.14, t)
    t = np.where((lx < 2) & (ly < 28), t + 0.08, t)
    col = stepped(t, BRICK)
    tm = np.full((S, S), level(MORTAR, 1), np.float32)
    tm = np.where(((ly >= 30) | ((lx >= 62) & (ly < 28))), level(MORTAR, 0), tm)
    tm = np.where((pix(seed + 5, 2) > 0.85), level(MORTAR, 2), tm)
    colm = MORTAR.shade(tm, 0)
    return img_of(np.where(mortar[..., None], colm, col))


def paint_stone_bricks(seed):
    yy, xx = ints()
    row = yy // 64
    ly = yy % 64
    lx = np.where(row == 0, xx, (xx + 64) % S) % np.where(row == 0, 128, 64)
    bw = np.where(row == 0, 128, 64)
    t = level(SBRICK, 4) + (fbm(seed, 32, 3) - 0.5) * 0.4
    blot = anoise(seed + 1, 10, 16)
    t = np.where(blot < 0.3, t - 0.12, t)
    t = np.where(blot > 0.72, t + 0.12, t)
    t += (pix(seed + 2, 2) - 0.5) * 0.08
    # Darker lower part of each brick, light top/left edge, dark mortar bottom/right.
    t -= np.clip((ly - 40) / 22.0, 0, 1) * 0.18
    t = np.where((ly < 2) | (lx < 2), level(SBRICK, 6), t)
    t = np.where((ly >= 2) & (ly < 4) & (lx >= 2), t + 0.1, t)
    mort = (ly >= 60) | (lx >= bw - 4)
    t = np.where(mort, level(SBRICK, 1) - 0.05, t)
    t = np.where(mort & ((ly >= 62) | (lx >= bw - 2)), 0.0, t)
    return img_of(stepped(t, SBRICK))


# ---------------------------------------------------------------------------- crafting table


def bezier(p0, p1, p2, n=24):
    ts = np.linspace(0, 1, n)
    return [((1 - t) ** 2 * p0[0] + 2 * (1 - t) * t * p1[0] + t * t * p2[0],
             (1 - t) ** 2 * p0[1] + 2 * (1 - t) * t * p1[1] + t * t * p2[1]) for t in ts]


def stroke(points, widths):
    m = np.zeros((S, S), bool)
    for i in range(len(points) - 1):
        w = widths[i] if hasattr(widths, "__len__") else widths
        m |= thick_line(points[i], points[i + 1], w)
    return m


def table_body(seed):
    """Side/front base: planks, black side edges, dark post with the red funnel on top."""
    yy, xx = ints()
    col = stepped(plank_value(seed, seams=(200, 200, 200, 200)), PLANK)
    ly = yy % 32
    # Post.
    post = (xx >= 58) & (xx < 70)
    tp = np.where((ly >= 28), level(DARKWOOD, 1), level(DARKWOOD, 3))
    tp = np.where(post & (xx < 60) & (ly < 28), level(DARKWOOD, 4), tp)
    tp = np.where(post & (xx >= 68) & (ly < 28), level(DARKWOOD, 2), tp)
    col = np.where(post[..., None], DARKWOOD.shade(tp, 0), col)
    # Funnel (inverted triangle) on top of the post. Its top edge spans exactly where the
    # top texture's red octagon reaches the block edge (x 29..98), so the red continues
    # around the corner; it narrows down to the post.
    for_outline = (yy < 31) & (xx >= 27 + yy) & (xx < 101 - yy)
    funnel = (yy < 29) & (xx >= 29 + yy) & (xx < 99 - yy)
    tf = np.full((S, S), level(TABLE_RED, 3), np.float32)
    tf = np.where((yy < 2), level(TABLE_RED, 1), tf)
    col = np.where((for_outline & ~funnel)[..., None], DARKWOOD.at(1), col)
    col = np.where(funnel[..., None], TABLE_RED.shade(tf, 0), col)
    # The post rises into the funnel but stops short of the edge.
    col = np.where((funnel & (yy >= 12) & (xx >= 58) & (xx < 70))[..., None],
                   DARKWOOD.at(3), col)
    # Black side edges.
    edge = (xx < 4) | (xx >= S - 4)
    col = np.where(edge[..., None], DARKWOOD.shade(np.where(ly >= 28, 0.0, level(DARKWOOD, 1)),
                                                   0), col)
    return col


def shaded_shape(mask, ramp, base, light=0.25, width=2, seed=0, grain=0.0):
    t = np.full((S, S), base, np.float32) + edge_light(mask, width) * light
    if grain:
        t += (pix(seed, 2) - 0.5) * grain
    return ramp.shade(t, 0)


def paint_crafting_table_side(seed):
    col = table_body(seed)
    # Shears hanging with crescent handles, blades crossing below.
    pts_l = bezier((23, 34), (18, 50), (33, 60))
    pts_r = bezier((46, 34), (50, 50), (35, 60))
    wl = [3 + 4 * np.sin(np.pi * i / 23) for i in range(24)]
    handles = stroke(pts_l, wl) | stroke(pts_r, wl)
    blade_l = thick_line((36, 60), (25, 84), 4.5)
    blade_r = thick_line((32, 60), (44, 84), 4.5)
    blades = blade_l | blade_r
    img = img_of(col)
    th = level(DARKWOOD, 4) + edge_light(handles, 2) * 0.25
    paint(img, handles, DARKWOOD.shade(th, 0))
    paint(img, blade_r, TOOL_GREY.shade(np.full((S, S), 0.7) + edge_light(blade_r, 1) * 0.35, 0))
    paint(img, blade_l, TOOL_GREY.shade(np.full((S, S), 0.7) + edge_light(blade_l, 1) * 0.35, 0))
    paint(img, rect(58, 32, 62, 36), TOOL_GREY.at(0))
    return img


def paint_crafting_table_front(seed):
    col = table_body(seed + 1)
    img = img_of(col)
    yy, xx = ints()
    # Hammer hanging head-down: dark handle, light grey head.
    handle = rect(40, 24, 74, 32)
    paint(img, handle, DARKWOOD.shade(np.where(xx < 27, level(DARKWOOD, 5),
                                                level(DARKWOOD, 3)), 0))
    head = rect(72, 16, 88, 40)
    th = np.full((S, S), 0.6) - edge_light(head, 2) * 0.3
    th = np.where((yy >= 84) & (xx >= 22), 1.0, th)
    th = np.where((xx >= 36) & (yy >= 74), 1.0, th)
    th = np.where((yy >= 84) & (xx < 22), 0.3, th)
    paint(img, head, TOOL_GREY.shade(th, 0))
    # Saw: dark handle frame with a round hole, blade tapering to a point.
    frame = rect(28, 88, 50, 112)
    hole = ((yy - 38.5) / 6.5) ** 2 + ((xx - 100) / 7.0) ** 2 <= 1
    paint(img, frame & ~hole, DARKWOOD.shade(np.full((S, S), level(DARKWOOD, 4))
                                              + edge_light(frame, 2) * 0.2, 0))
    blade = polygon([(88, 50), (112, 50), (112, 106), (110, 106)])
    # Serrated left edge: 2 px steps.
    teeth = blade & (((yy // 2) % 3) == 0) & ~shrink(blade, 2)
    tb = np.full((S, S), 0.66)
    tb = np.where(yy < 54, 0.33, tb)
    tb = np.where(teeth | ((xx < 94) & (yy > 58) & ~shrink(blade, 2)), 1.0, tb)
    paint(img, blade, TOOL_GREY.shade(tb, 0))
    return img


def paint_crafting_table_top(seed):
    yy, xx = ints()
    xm = np.minimum(xx, S - 1 - xx)
    ym = np.minimum(yy, S - 1 - yy)
    d = xm + ym  # distance to the nearest corner along the diagonal
    # Inner red wood with darker blotches.
    t = level(TABLE_RED, 5) + (anoise(seed, 12, 20) - 0.5) * 0.2
    blot = anoise(seed + 1, 6, 16)
    t = np.where(blot < 0.33, level(TABLE_RED, 4), level(TABLE_RED, 5))
    t = np.where(pix(seed + 2, 2) > 0.93, level(TABLE_RED, 4), t)
    # Darker ring just inside the octagon edge.
    ring = (d < 36) | (np.minimum(xm, ym) < 6)
    t = np.where(ring, level(TABLE_RED, 4), t)
    edge = (d < 32) | (np.minimum(xm, ym) < 2)
    t = np.where(edge, level(TABLE_RED, 1), t)
    col = TABLE_RED.shade(t, 0)
    # Light wood corners.
    corner = d < 29
    tc = np.where((d >= 25) | (np.minimum(xm, ym) < 4), 0.0, 1.0)
    col = np.where(corner[..., None], TABLE_CORNER.shade(tc, 0), col)
    # Grid: 3 x 3 cells with dark lines and shaded inner top/left edges.
    lines = np.zeros((S, S), bool)
    for p in (24, 50, 76, 102):
        lines |= (xx >= p) & (xx < p + 2) & (yy >= 24) & (yy < 104)
        lines |= (yy >= p) & (yy < p + 2) & (xx >= 24) & (xx < 104)
    shade = np.zeros((S, S), bool)
    for p in (24, 50, 76):
        shade |= (xx >= p + 2) & (xx < p + 5) & (yy >= 26) & (yy < 102)
        shade |= (yy >= p + 2) & (yy < p + 5) & (xx >= 26) & (xx < 102)
    col = np.where((shade & ~lines)[..., None], TABLE_RED.at(4), col)
    col = np.where(lines[..., None], TABLE_RED.at(2), col)
    # Outer black frame.
    outer = (xm < 2) | (ym < 2)
    col = np.where((outer & ((d < 32) | True))[..., None], DARKWOOD.at(0), col)
    top_mid = (ym < 2) & (xm >= 32)
    col = np.where(top_mid[..., None], TABLE_RED.at(1), col)
    side_mid = (xm < 2) & (ym >= 32)
    col = np.where(side_mid[..., None], TABLE_RED.at(1), col)
    return img_of(col)


# ---------------------------------------------------------------------------- furnace


def cobble_value(seed, count=16):
    """Rounded stones with soft dark gaps (STONE shade values)."""
    ids, _, border, best = cells(seed, count, jitter=0.7, wobble=6, cell=32)
    R = 0.72 * S / np.sqrt(count)
    border = np.minimum(border, (R - best) * 1.4)
    h = np.clip((border - 1.5) / 14.0, 0, 1)
    dome = np.sqrt(1 - (1 - h) ** 2)
    lit = dome - shifted(dome, 4, 4)
    per = rng(seed + 1).uniform(-0.03, 0.03, count).astype(np.float32)
    t = level(STONE, 3) + dome * 0.2 + lit * 0.25 + per[ids]
    blot = anoise(seed + 2, 6, 6)
    t = np.where(blot > 0.7, t + 0.05, t)
    t += (pix(seed + 3, 2) - 0.5) * 0.06
    t = np.where(border < 1.2, level(STONE, 3) - 0.04, t)
    return t


def furnace_border(t):
    yy, xx = ints()
    b = (xx < 4) | (xx >= S - 4) | (yy < 4) | (yy >= S - 2)
    t = np.where(b, level(STONE, 1), t)
    t = np.where(((xx >= 2) & (xx < 4) | (xx >= S - 4) & (xx < S - 2)) & (yy >= 4) &
                 (yy < S - 2), level(STONE, 2), t)
    t = np.where(yy < 2, level(STONE, 0) + (pix(3, 2) > 0.5) * 0.1, t)
    return t


def slab_value(seed, y0, y1):
    """The smooth light slab (lower part of furnace sides)."""
    yy, xx = ints()
    t = level(STONE, 7) + 0.03 + (pix(seed, 2) > 0.8) * 0.08 - (pix(seed + 1, 2) > 0.85) * 0.08
    t = np.where(yy < y0 + 2, level(STONE, 10), t)
    t = np.where((yy >= y0 + 2) & (yy < y0 + 4), level(STONE, 8), t)
    t -= np.clip((yy - (y1 - 12)) / 12.0, 0, 1) * 0.3
    t = np.where((xx < 8) & (yy > y0 + 4), t - 0.1, t)
    return t


def paint_furnace_side(seed):
    yy, xx = ints()
    t = cobble_value(seed, 25)
    t = np.where(yy >= 72, slab_value(seed + 5, 72, 124), t)
    return img_of(stepped(furnace_border(t), STONE, 0.2))


def paint_furnace_top(seed):
    t = cobble_value(seed + 9)
    yy, xx = ints()
    b = (xx < 4) | (xx >= S - 4) | (yy < 4) | (yy >= S - 4)
    t = np.where(b, level(STONE, 1), t)
    t = np.where(b & ((xx < 2) | (yy < 2) | (xx >= S - 2) | (yy >= S - 2)), level(STONE, 2), t)
    return img_of(stepped(t, STONE, 0.2))


UP_ARCH = (56, 64, 42, 32)   # center y, center x, rx, ry of the upper (smoke) arch
LOW_ARCH = (114, 64, 44, 22)  # the fire opening


def arch(cy, cx, rx, ry, flat_bottom):
    yy, xx = grid()
    return ((((yy - cy) / ry) ** 2 + ((xx - cx) / rx) ** 2 <= 1) |
            ((yy >= cy) & (np.abs(xx - cx) <= rx))) & (yy < flat_bottom)


FURNACE_SEED = 5151


def furnace_front_base():
    seed = FURNACE_SEED
    yy, xx = ints()
    t = cobble_value(seed, 25)
    t = np.where(yy >= 72, slab_value(seed + 5, 72, 124), t)
    t = furnace_border(t)
    # Lip under the upper arch.
    up = arch(*UP_ARCH, 56)
    t = np.where((yy >= 56) & (yy < 58) & (xx >= 22) & (xx < 106), level(STONE, 9), t)
    t = np.where((yy >= 58) & (yy < 60) & (xx >= 22) & (xx < 106), level(STONE, 6), t)
    # Rim around the lower opening, sill below.
    low = arch(*LOW_ARCH, 124)
    rim = grow(low, 2) & ~low & (yy < 124)
    t = np.where(rim, level(STONE, 6), t)
    t = np.where((yy >= 120) & (yy < 124) & ~low, level(STONE, 4), t)
    t = np.where((yy >= 124) & (yy < 126) & (xx >= 4) & (xx < S - 4), level(STONE, 3), t)
    col = stepped(t, STONE, 0.2)
    dark = Ramp("111111", "212121")
    # Upper arch: dark with a slightly lighter core.
    core = (((yy - 56) / 13.0) ** 2 + ((xx - 64) / 24.0) ** 2 <= 1)
    col = np.where(up[..., None], dark.shade(core.astype(np.float32), 0), col)
    col = np.where(low[..., None], dark.at(0), col)
    return img_of(col), low


def paint_furnace_front(seed):
    return furnace_front_base()[0]


def paint_furnace_front_on(seed):
    img, low = furnace_front_base()
    yy, xx = grid()
    # Flame tongues rising from the bottom of the opening.
    tongues = [(28, 16, 8), (40, 28, 10), (52, 20, 8), (63, 32, 10), (75, 24, 8), (87, 30, 10),
               (99, 18, 7)]
    height = np.zeros((S, S), np.float32)
    for cx, hgt, w in tongues:
        u = np.abs(xx - cx) / w
        prof = np.clip(1 - u, 0, 1) ** 0.7 * hgt
        height = np.maximum(height, prof)
    height += 10
    rel = 124 - yy
    flame = low & (rel < height)
    depth = ndimage.distance_transform_edt(flame)
    core = (anoise(FURNACE_SEED + 3, 10, 4) > 0.55) & (depth > 5)
    tf = np.where(depth > 3, level(FLAME, 1), level(FLAME, 0))
    tf = np.where(core, level(FLAME, 2), tf)
    tf = np.where(core & (depth > 8), level(FLAME, 3), tf)
    img[..., :3] = np.where(flame[..., None], FLAME.shade(tf, 0), img[..., :3])
    return img


# ---------------------------------------------------------------------------- storage blocks


def paint_iron_block(seed):
    yy, xx = ints()
    t = np.full((S, S), level(IRON, 7), np.float32)
    t = np.where(pix(seed, 2) > 0.9, level(IRON, 6), t)
    t = np.where(pix(seed + 1, 2) > 0.94, level(IRON, 8), t)
    r = rng(seed + 2)
    for k in range(6):
        y0 = 4 + k * 20
        hl = int(r.integers(52, 80))
        t = np.where((yy >= y0) & (yy < y0 + 2) & (xx < hl), level(IRON, 10), t)
        t = np.where((yy >= y0) & (yy < y0 + 2) & (xx >= hl), level(IRON, 9), t)
        gl = int(r.integers(68, 96))
        g = (yy >= y0 + 17) & (yy < y0 + 20)
        t = np.where(g & (xx < gl), level(IRON, 3), t)
        t = np.where(g & (xx >= gl), level(IRON, 5), t)
        t = np.where(g & (yy >= y0 + 19), level(IRON, 2), t)
    # Frame: darker left, right and bottom edges, top edge.
    t = np.where(xx < 4, level(IRON, 2), t)
    t = np.where((xx >= S - 6) & (xx < S - 4), level(IRON, 5), t)
    t = np.where(xx >= S - 4, level(IRON, 1), t)
    t = np.where(yy < 4, level(IRON, 3) + (xx > 12) * (xx < 90) * 0.2, t)
    t = np.where(yy >= S - 4, level(IRON, 1), t)
    return img_of(stepped(t, IRON, 0))


def streaks(seed, n, angle, lengths, widths, values, tile=True):
    """Lens-shaped strokes along `angle` (radians, 0 = right, y down). Returns (value map,
    mask); later strokes paint over earlier ones."""
    r = rng(seed)
    yy, xx = grid()
    ca, sa = np.cos(angle), np.sin(angle)
    val = np.zeros((S, S), np.float32)
    mask = np.zeros((S, S), bool)
    offs = [(-S, -S), (-S, 0), (-S, S), (0, -S), (0, 0), (0, S), (S, -S), (S, 0), (S, S)]         if tile else [(0, 0)]
    for _ in range(n):
        cy, cx = r.uniform(0, S), r.uniform(0, S)
        L = r.uniform(*lengths) / 2
        W = r.uniform(*widths) / 2
        v = values[int(r.integers(len(values)))]
        a2 = angle + r.uniform(-0.12, 0.12)
        c2, s2 = np.cos(a2), np.sin(a2)
        m = np.zeros((S, S), bool)
        for oy, ox in offs:
            dy, dx = yy - cy - oy, xx - cx - ox
            u = dx * c2 + dy * s2
            w = -dx * s2 + dy * c2
            m |= (np.abs(u) < L) & (np.abs(w) < W * np.sqrt(np.clip(1 - (u / L) ** 2, 0, 1)))
        val = np.where(m, v, val)
        mask |= m
    return val, mask


def gem_bands(seed, ramp):
    """Gold/diamond block: framed, with thin diagonal streaks of light and shade."""
    yy, xx = grid()
    n = len(ramp) - 1
    base = (noise(seed, 32) - 0.5) * 1.2 + 4.6
    v = np.round(base)
    ang = -0.62
    sv, sm = streaks(seed + 1, 14, ang, (50, 120), (8, 22), [3, 3, 5, 5, 6, 2], tile=False)
    v = np.where(sm, sv, v)
    hv, hm = streaks(seed + 2, 12, ang, (30, 80), (2, 5), [7, 7, 6, 2, 8], tile=False)
    v = np.where(hm, hv, v)
    t = v / n
    # Frame: dark rim, light inner line top/left.
    rim = (yy < 4) | (xx < 4) | (yy >= S - 4) | (xx >= S - 4)
    t = np.where(rim, level(ramp, 1), t)
    t = np.where(((yy < 2) | (xx < 2)) & ~((yy >= S - 4) | (xx >= S - 4)), level(ramp, 2), t)
    t = np.where((yy >= S - 2) | (xx >= S - 2), level(ramp, 0), t)
    inner = ((yy >= 4) & (yy < 6) & (xx >= 4) & (xx < S - 4)) |         ((xx >= 4) & (xx < 6) & (yy >= 4) & (yy < S - 4))
    t = np.where(inner, level(ramp, 7), t)
    t = np.where(inner & (xx < 40) & (yy < 40), level(ramp, 8), t)
    inner2 = ((yy >= S - 6) & (yy < S - 4) & (xx >= 4) & (xx < S - 4)) |         ((xx >= S - 6) & (xx < S - 4) & (yy >= 4) & (yy < S - 4))
    t = np.where(inner2, level(ramp, 2), t)
    return img_of(ramp.shade(t, 0))


def paint_gold_block(seed):
    return gem_bands(seed, GOLD)


def paint_diamond_block(seed):
    return gem_bands(seed, DIAMOND)


def paint_coal_block(seed):
    t = np.full((S, S), level(COAL, 0), np.float32)
    t = np.where(noise(seed, 16) > 0.5, level(COAL, 1), t)
    ang = -0.5
    v1, m1 = streaks(seed + 1, 18, ang, (40, 80), (10, 20), [1, 2, 2])
    t = np.where(m1, v1 / 4, t)
    v2, m2 = streaks(seed + 2, 14, ang, (20, 50), (4, 8), [3, 3, 4, 0])
    t = np.where(m2, v2 / 4, t)
    t = np.where(pix(seed + 3, 2) > 0.97, level(COAL, 3), t)
    return img_of(COAL.shade(t, 0))


# ---------------------------------------------------------------------------- torch


def paint_torch(seed):
    img = blank()
    yy, xx = ints()
    stick = rect(64, 56, S, 72)
    t = np.full((S, S), level(TWOOD, 4))
    t = np.where(xx < 60, level(TWOOD, 5), t)
    t = np.where((xx >= 60) & (xx < 62), level(TWOOD, 6), t)
    t = np.where((xx >= 66) & (xx < 68), level(TWOOD, 3), t)
    t = np.where(xx >= 68, level(TWOOD, 2), t)
    t = np.where(xx >= 70, level(TWOOD, 1), t)
    grain = pix(seed, 2) > 0.8
    t = np.where(grain & (xx < 66), t - level(TWOOD, 1), t)
    t = np.where(yy >= 124, t - level(TWOOD, 1), t)
    paint(img, stick, TWOOD.shade(t, 0))
    head = rect(48, 56, 64, 72)
    th = np.full((S, S), level(FLAME, 1))
    th = np.where((yy < 50) | (xx < 58) | (xx >= 70), level(FLAME, 0) + 0.15, th)
    core = ((yy - 60) / 5.0) ** 2 + ((xx - 63) / 4.0) ** 2 <= 1
    th = np.where(core, level(FLAME, 2), th)
    th = np.where(((yy - 61) / 3.0) ** 2 + ((xx - 62) / 2.5) ** 2 <= 1, level(FLAME, 3), th)
    paint(img, head, FLAME.shade(th, 0))
    return img


# ---------------------------------------------------------------------------- lantern / chain


def metal_part(img, mask, base=1, light=True):
    t = np.full((S, S), level(LANT, base), np.float32)
    if light:
        e = edge_light(mask, 2)
        t = np.where(e > 0, level(LANT, base + 2), t)
        t = np.where(e < 0, level(LANT, max(0, base - 1)), t)
    paint(img, mask, LANT.shade(t, 0))


def paint_lantern(seed):
    img = blank()
    yy, xx = grid()
    # Cap sides (1,0)-(5,2).
    cap = rect(0, 8, 16, 40)
    metal_part(img, cap, 1)
    paint(img, rect(0, 8, 2, 40), LANT.at(3))
    paint(img, rect(8, 8, 12, 40), GLOW.shade(np.where(grid()[1] < 12, 0.2, 0.0), 0))
    # Body sides (0,2)-(6,9): metal frame, round glowing window.
    body = rect(16, 0, 72, 48)
    metal_part(img, body, 2)
    win = rect(24, 2, 64, 46)
    win &= ~(rect(24, 2, 28, 6) | rect(24, 42, 28, 46) | rect(60, 2, 64, 6) | rect(60, 42, 64, 46))
    r = np.hypot((yy - 44) / 1.0, (xx - 24) / 1.15)
    tg = np.clip(1.0 - r / 30.0, 0, 1)
    idx = np.select([tg < 0.12, tg < 0.22, tg < 0.32, tg < 0.55], [0, 2, 3, 4], 5)
    streak = (np.abs((xx - 20) - (yy - 40)) < 2.5) & (r < 13)
    idx = np.where(streak, 6, idx)
    paint(img, win, GLOW.colors[idx])
    # Top/bottom frame lines of the body.
    paint(img, rect(22, 0, 24, 48), LANT.at(0))
    paint(img, rect(64, 0, 66, 48), LANT.at(0))
    # Body top/bottom (0,9)-(6,15), with the cap top (1,10)-(5,14) inside.
    top = rect(72, 0, 120, 48)
    metal_part(img, top, 3)
    disk_m = ((yy - 96) ** 2 + (xx - 24) ** 2) <= 21 ** 2
    paint(img, disk_m, LANT.at(2))
    paint(img, disk_m & ~(((yy - 97) ** 2 + (xx - 25) ** 2) <= 20 ** 2), LANT.at(1))
    # Hanging ring (11,1)-(14,5): a rounded loop.
    ring_o = (((yy - 24) / 16.0) ** 2 + ((xx - 100) / 12.0) ** 2 <= 1) & rect(8, 88, 40, 112)
    ring_i = (((yy - 24) / 10.0) ** 2 + ((xx - 100) / 6.5) ** 2 <= 1)
    ring = ring_o & ~ring_i
    metal_part(img, ring, 1)
    # Standing handle (11,10)-(14,12): an arch.
    h_o = (((yy - 96) / 16.0) ** 2 + ((xx - 100) / 12.0) ** 2 <= 1) & rect(80, 88, 96, 112)
    h_i = (((yy - 96) / 10.0) ** 2 + ((xx - 100) / 6.5) ** 2 <= 1)
    metal_part(img, h_o & ~h_i, 1)
    return img


def paint_iron_chain(seed):
    img = blank()
    yy, xx = grid()
    for x0, phase in ((0, 0), (24, 24)):
        for k in range(-1, 4):
            cy = phase + 20 + k * 48
            cx = x0 + 12
            outer = (np.abs(xx - cx) <= 12) & (np.abs(yy - cy) <= 14)
            outer &= ((np.maximum(np.abs(xx - cx) - 6, 0) / 6) ** 2 +
                      (np.maximum(np.abs(yy - cy) - 8, 0) / 6) ** 2) <= 1
            inner = (np.abs(xx - cx) < 6) & (np.abs(yy - cy) < 8)
            inner &= ((np.maximum(np.abs(xx - cx) - 2, 0) / 4) ** 2 +
                      (np.maximum(np.abs(yy - cy) - 4, 0) / 4) ** 2) <= 1
            m = outer & ~inner & (xx >= x0) & (xx < x0 + 24)
            t = np.full((S, S), level(LANT, 1))
            e = edge_light(m, 2)
            t = np.where(e < 0, level(LANT, 0), t)
            hl = m & (xx < cx) & (yy < cy) & ~shrink(m, 1) & grow(inner, 2)
            t = np.where(hl | (m & (xx < x0 + 4) & (yy > cy - 8) & (yy < cy + 6) & (xx >= x0 + 2)),
                         level(LANT, 3), t)
            paint(img, m, LANT.shade(t, 0))
    return img


# ---------------------------------------------------------------------------- door


def door_wood(seed):
    t = level(DOOR, 3) + 0.04 + (anoise(seed, 8, 30) - 0.5) * 0.2
    n = anoise(seed + 1, 2, 18)
    t = np.where(n < 0.2, t - 0.14, t)
    t = np.where(n > 0.8, t + 0.12, t)
    t += (pix(seed + 2, 2) - 0.5) * 0.05
    return t


def door_panels(seed, top: bool):
    """Door half: wood with a 2 x 4 grid of recessed panels over the whole door (windows in
    the top two rows). Returns (value, window mask)."""
    yy, xx = ints()
    Y = yy + (0 if top else S)  # door coordinates 0..255
    t = door_wood(seed)
    windows = np.zeros((S, S), bool)
    rows = [(20, 48), (60, 88), (108, 150), (170, 196), (218, 246)]
    for i, (y0, y1) in enumerate(rows):
        for (x0, x1) in ((20, 56), (68, 104)):
            m = (Y >= y0) & (Y < y1) & (xx >= x0) & (xx < x1)
            t = np.where(m, t - 0.05, t)
            t = np.where(m & (Y < y0 + 4), level(DOOR, 0), t)
            t = np.where(m & (xx < x0 + 4) & (Y >= y0 + 4), level(DOOR, 1), t)
            t = np.where(m & (xx >= x1 - 2), level(DOOR, 6), t)
            t = np.where((Y >= y1) & (Y < y1 + 2) & (xx >= x0) & (xx < x1 + 2), level(DOOR, 6), t)
            if i < 2:
                windows |= (Y >= y0 + 4) & (Y < y1) & (xx >= x0 + 4) & (xx < x1 - 2)
    # Outer stiles: light left edge, dark right edge.
    t = np.where(xx < 2, level(DOOR, 6), t)
    t = np.where((xx >= 2) & (xx < 4), level(DOOR, 4), t)
    t = np.where(xx >= S - 2, level(DOOR, 0), t)
    t = np.where((xx >= S - 4) & (xx < S - 2), level(DOOR, 1), t)
    if top:
        t = np.where(yy < 2, level(DOOR, 6), t)
    else:
        t = np.where(yy >= S - 2, level(DOOR, 0), t)
    return t, windows


def hinge(img, y0, y1):
    m = rect(y0, 0, y1, 6)
    t = np.full((S, S), level(HINGE, 2)) + edge_light(m, 1) * 0.3
    paint(img, m, HINGE.shade(t, 0))


def handle(img, top: bool):
    yy, xx = ints()
    Y = yy + (0 if top else S)
    ring = (Y >= 116) & (Y < 150) & (xx >= 92) & (xx < 116)
    hole = (Y >= 122) & (Y < 144) & (xx >= 98) & (xx < 110)
    m = ring & ~hole
    t = np.full((S, S), level(HINGE, 2)) + edge_light(m, 2) * 0.3
    paint(img, m, HINGE.shade(t, 0))


def paint_oak_door_top(seed):
    t, win = door_panels(seed, True)
    img = img_of(stepped(t, DOOR, 0))
    img[win] = 0
    hinge(img, 24, 44)
    handle(img, True)
    return img


def paint_oak_door_bottom(seed):
    t, _ = door_panels(seed, False)
    img = img_of(stepped(t, DOOR, 0))
    hinge(img, 80, 98)
    return img


# ---------------------------------------------------------------------------- wool


def paint_white_wool(seed):
    yy, xx = warp(seed + 5, 5, 32)
    wl, amp, per = 40.0, 6.0, 14.0
    phase = (anoise(seed, 32, 64) - 0.5) * 16
    tri = np.abs(((xx / wl) % 1.0) - 0.5) * 2 - 0.5  # -0.5..0.5
    u = ((yy + tri * 2 * amp + phase) / per) % 1.0
    t = np.where(u < 0.2, 0.72, np.where(u < 0.6, 0.55, np.where(u < 0.85, 0.44, 0.34)))
    t += (noise(seed + 1, 16) - 0.5) * 0.45 + (anoise(seed + 2, 4, 12) - 0.5) * 0.3
    return img_of(stepped(t, WOOL, 0.15))


# ---------------------------------------------------------------------------- bed


def swirl_value(seed):
    """Red blanket: smooth swirling streaks (contours of a warped field)."""
    yy, xx = warp(seed, 18, 64)
    f = noise(seed + 2, 64) * 0.7 + noise(seed + 3, 32) * 0.3
    g = (f * 2.5 + (xx + yy) / 128) % 1.0
    t = np.full((S, S), level(RED_TOP, 5), np.float32)
    t = np.where((g > 0.02) & (g < 0.12), level(RED_TOP, 6), t)
    t = np.where((g > 0.4) & (g < 0.68), level(RED_TOP, 4), t)
    t = np.where((g > 0.47) & (g < 0.6), level(RED_TOP, 3), t)
    return t


def paint_red_bed_foot_up(seed):
    return img_of(stepped(swirl_value(seed), RED_TOP, 0))


def paint_red_bed_head_up(seed):
    yy, xx = ints()
    t = swirl_value(seed)
    t = np.where((yy >= 64) & (yy < 82), t - 0.3, t)
    t = np.where((yy >= 104) & (yy < 106), level(RED_TOP, 1), t)
    t = np.where((yy >= 106) & (yy < 108), level(RED_TOP, 6), t)
    col = stepped(t, RED_TOP, 0)
    # Sheet and the rounded pillow.
    ts = np.full((S, S), level(SHEET, 4))
    ts = np.where((yy >= 60), level(SHEET, 2), ts)
    ts = np.where((yy >= 62), level(SHEET, 0), ts)
    pil = (np.abs(xx - 63.5) <= 47) & (yy >= 6) & (yy < 60)
    pil &= ((np.maximum(np.abs(xx - 63.5) - 31, 0) / 16) ** 2 +
            (np.maximum(np.abs(yy - 33) - 11, 0) / 16) ** 2) <= 1
    inner = shrink(pil, 3)
    ts = np.where(pil & ~inner, level(SHEET, 2), ts)
    ts = np.where(inner, level(SHEET, 5), ts)
    rim = inner & ~shrink(inner, 3)
    ts = np.where(rim & ((yy > 36) | (xx > 96)), level(SHEET, 3), ts)
    ts = np.where(rim & (yy > 44) & ~shrink(inner, 2), level(SHEET, 2), ts)
    col = np.where((yy < 64)[..., None], SHEET.shade(ts, 0), col)
    return img_of(col)


def bed_side(seed, pillow, legs):
    yy, xx = ints()
    img = blank()
    band = (yy >= 56) & (yy < 88)
    tb = swirl_value(seed) - 3 * level(RED_TOP, 1)
    tb = np.where(yy >= 86, level(RED_TOP, 0), tb)
    paint(img, band, RED_TOP.shade(tb, 0))
    if pillow:
        pm = band & ((xx >= 64) if pillow == "right" else (xx < 64) if pillow == "left"
                     else np.ones((S, S), bool))
        x0, x1 = (64, S) if pillow == "right" else (0, 64) if pillow == "left" else (0, S)
        # The pillow's side: white with rounded lower corners, grey sheet around them.
        px0, px1 = x0 + (4 if pillow == "right" else 0), x1 - (4 if pillow == "left" else 0)
        rr = 14.0
        cxl, cxr, cyb = px0 + rr, px1 - rr, 84 - rr
        dxc = np.maximum(np.maximum(cxl - xx, xx - cxr), 0)
        dyc = np.maximum(yy - cyb, 0)
        rd = np.hypot(dxc, dyc)
        tp = np.full((S, S), level(SHEET, 4))
        tp = np.where(rd > rr - 2, level(SHEET, 3), tp)
        tp = np.where(rd > rr, level(SHEET, 2), tp)
        tp = np.where(yy >= 84, level(SHEET, 2), tp)
        tp = np.where(yy >= 86, level(SHEET, 0), tp)
        if pillow == "right":
            tp = np.where(xx < 66, level(SHEET, 0), tp)
        elif pillow == "left":
            tp = np.where(xx >= 62, level(SHEET, 0), tp)
        paint(img, pm, SHEET.shade(tp, 0))
    frame = (yy >= 88) & (yy < 104)
    tw = level(BEDWOOD, 4) + np.where(anoise(seed + 2, 3, 16) < 0.35, -0.2, 0) + \
        np.where(pix(seed + 3, 2) > 0.8, 0.2, 0)
    tw = np.where(yy < 90, level(BEDWOOD, 3), tw)
    tw = np.where(yy >= 100, level(BEDWOOD, 0) + (pix(seed + 4, 2) > 0.6) * 0.2, tw)
    paint(img, frame, BEDWOOD.shade(tw, 0))
    for side in legs:
        x0 = 0 if side == "left" else S - 24
        leg = rect(104, x0, S, x0 + 24)
        tl = np.full((S, S), level(BEDWOOD, 2))
        tl = np.where(rect(106, x0 + 4, S - 4, x0 + 20), level(BEDWOOD, 0), tl)
        tl = np.where(rect(106, x0 + 4, 108, x0 + 20), level(BEDWOOD, 1), tl)
        tl = np.where(rect(104, x0 + (20 if side == "right" else 0), S,
                           x0 + (24 if side == "right" else 4)), level(BEDWOOD, 4), tl)
        paint(img, leg, BEDWOOD.shade(tl, 0))
    return img


def paint_red_bed_head_east(seed):
    return bed_side(seed, "right", ("right",))


def paint_red_bed_head_west(seed):
    return bed_side(seed, "left", ("left",))


def paint_red_bed_foot_east(seed):
    return bed_side(seed, None, ("left",))


def paint_red_bed_foot_west(seed):
    return bed_side(seed, None, ("right",))


def paint_bed_head_north(seed):
    return bed_side(seed, "all", ("left", "right"))


def paint_red_bed_foot_south(seed):
    return bed_side(seed, None, ("left", "right"))


# ---------------------------------------------------------------------------- cracks

_CRACKS: dict = {}


def crack_field():
    """(crack mask, stage per crack pixel). A Voronoi-like network of wobbly 2 px cracks
    (Y junctions, closed cells, a few dead ends); a crack pixel's stage comes from its
    distance along the cracks from a junction near the middle."""
    if "f" in _CRACKS:
        return _CRACKS["f"]
    import heapq
    from scipy.spatial import Voronoi
    r = rng(46)
    n = 5
    pts = []
    for i in range(n):
        for j in range(n):
            pts.append(((j + 0.5 + r.uniform(-0.45, 0.45)) * S / n,
                        (i + 0.5 + r.uniform(-0.45, 0.45)) * S / n))
    base = np.array(pts)
    allp = np.concatenate([base + [ox, oy] for oy in (-S, 0, S) for ox in (-S, 0, S)])
    vor = Voronoi(allp)
    V = vor.vertices
    inside = lambda v: -12 <= v[0] <= S + 12 and -12 <= v[1] <= S + 12
    edges = []
    for (i, j) in vor.ridge_vertices:
        if i < 0 or j < 0 or not (inside(V[i]) and inside(V[j])):
            continue
        mid = (V[i] + V[j]) / 2
        if r.random() < 0.14 and np.hypot(mid[0] - 64, mid[1] - 62) > 34:
            continue  # some edges never crack: open ends
        edges.append((i, j))
    # Dijkstra over the vertices from the one nearest the middle.
    adj: dict = {}
    for i, j in edges:
        L = float(np.hypot(*(V[i] - V[j])))
        adj.setdefault(i, []).append((j, L))
        adj.setdefault(j, []).append((i, L))
    start = min(adj, key=lambda v: np.hypot(V[v][0] - 64, V[v][1] - 62))
    vd = {start: 0.0}
    heap = [(0.0, start)]
    while heap:
        d, v = heapq.heappop(heap)
        if d > vd.get(v, 1e9):
            continue
        for w, L in adj[v]:
            if d + L < vd.get(w, 1e9):
                vd[w] = d + L
                heapq.heappush(heap, (d + L, w))
    dist = np.full((S, S), np.inf, np.float32)

    def stamp(x, y, d):
        ix, iy = int(np.floor(x)), int(np.floor(y))
        if iy + 2 <= 0 or ix + 2 <= 0 or iy >= S or ix >= S:
            return
        ys, xs = slice(max(iy, 0), min(iy + 2, S)), slice(max(ix, 0), min(ix + 2, S))
        dist[ys, xs] = np.minimum(dist[ys, xs], d)

    for i, j in edges:
        if i not in vd or j not in vd:
            continue
        p0, p1 = V[i], V[j]
        L = float(np.hypot(*(p1 - p0)))
        nseg = max(3, int(L / 6))
        nrm = np.array([-(p1 - p0)[1], (p1 - p0)[0]]) / max(L, 1e-6)
        bend = r.uniform(-0.12, 0.12) * L
        ks = np.arange(nseg + 1)
        offs = bend * np.sin(np.pi * ks / nseg) + np.concatenate([[0], r.uniform(-1.5, 1.5, nseg - 1), [0]])
        ctrl = [p0 + (p1 - p0) * k / nseg + nrm * offs[k] for k in range(nseg + 1)]
        sacc = 0.0
        for k in range(nseg):
            a_, b_ = ctrl[k], ctrl[k + 1]
            seg = float(np.hypot(*(b_ - a_)))
            m = max(1, int(seg))
            for q in range(m + 1):
                pnt = a_ + (b_ - a_) * q / m
                sp = sacc + seg * q / m
                stamp(pnt[0], pnt[1], min(vd[i] + sp, vd[j] + L - sp))
            sacc += seg
    # A few short dead-end spurs off the cracks.
    ys_, xs_ = np.nonzero(np.isfinite(dist))
    for _ in range(18):
        k = int(r.integers(len(ys_)))
        x, y, d = xs_[k] + 0.5, ys_[k] + 0.5, max(float(dist[ys_[k], xs_[k]]), 70.0)
        a = r.uniform(0, 2 * np.pi)
        for q in range(int(r.uniform(5, 11))):
            a += r.uniform(-0.3, 0.3)
            x, y = x + np.cos(a) * 1.5, y + np.sin(a) * 1.5
            stamp(x, y, d + q * 1.5 + 1)
    crack = np.isfinite(dist)
    stage = np.full((S, S), 9, np.int32)
    for k in range(9, -1, -1):
        stage = np.where(crack & (dist <= LIMITS[k]), k, stage)
    _CRACKS["f"] = (crack, stage)
    return _CRACKS["f"]


LIMITS = [5, 11, 18, 27, 36, 46, 58, 74, 96, 1e9]


def destroy(k):
    def paint_stage(seed):
        crack, stage = crack_field()
        m = crack & (stage <= k)
        img = blank()
        light = shifted(m, 2, -2) & ~m
        paint(img, light, hexc("9b9b9b"))
        paint(img, m, hexc("3d3d3d"))
        return img
    return paint_stage


# ---------------------------------------------------------------------------- table

TEXTURES = {
    "block/oak_planks": paint_oak_planks,
    "block/glass": paint_glass,
    "block/bricks": paint_bricks,
    "block/stone_bricks": paint_stone_bricks,
    "block/crafting_table_top": paint_crafting_table_top,
    "block/crafting_table_side": paint_crafting_table_side,
    "block/crafting_table_front": paint_crafting_table_front,
    "block/furnace_front": paint_furnace_front,
    "block/furnace_front_on": paint_furnace_front_on,
    "block/furnace_side": paint_furnace_side,
    "block/furnace_top": paint_furnace_top,
    "block/iron_block": paint_iron_block,
    "block/gold_block": paint_gold_block,
    "block/diamond_block": paint_diamond_block,
    "block/coal_block": paint_coal_block,
    "block/torch": paint_torch,
    "block/lantern": paint_lantern,
    "block/iron_chain": paint_iron_chain,
    "block/oak_door_top": paint_oak_door_top,
    "block/oak_door_bottom": paint_oak_door_bottom,
    "block/white_wool": paint_white_wool,
    "block/red_bed_head_up": paint_red_bed_head_up,
    "block/red_bed_foot_up": paint_red_bed_foot_up,
    "block/red_bed_head_east": paint_red_bed_head_east,
    "block/red_bed_head_west": paint_red_bed_head_west,
    "block/red_bed_foot_east": paint_red_bed_foot_east,
    "block/red_bed_foot_west": paint_red_bed_foot_west,
    "block/bed_head_north": paint_bed_head_north,
    "block/red_bed_foot_south": paint_red_bed_foot_south,
    "block/bed_down": paint_bed_down,
    **{f"block/destroy_stage_{i}": destroy(i) for i in range(10)},
}
