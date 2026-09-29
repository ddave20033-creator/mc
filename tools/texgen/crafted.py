"""Crafted blocks: planks, glass, bricks, stone bricks, crafting table, furnace, storage
blocks, torch, lantern, chain, oak door, wool, bed and the block breaking cracks.

Drawn after the Faithful 64x look (composition, palette, flat shading with few colors) at
128x128 with 1-2 px detail. Coordinates below are in 128 px texture pixels."""

from __future__ import annotations

import numpy as np
from scipy import ndimage

from common import (
    S, Ramp, bayer, blank, edge_light, grid, grow, hexc, noise, paint, pix, polygon, rect,
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
COPPER = Ramp("9c4e2e", "a85634", "b45f3a", "c26b44", "c9724a", "d07b52", "d6845a", "dc8f64",
              "e39a70", "e8a57c", "f0b890")  # also used by furnaces.py
CU_BLOCK = Ramp("904931", "9a5038", "a75a40", "b26247", "c26b4c", "c87456", "d67b5b", "e3826c")
GOLD = Ramp("cc8e27", "d39632", "f9bd23", "f5cc27", "ffd83e", "fee048", "ffec4f", "fffd90",
            "feffbd")
DIAMOND = Ramp("0ebabd", "15c2c6", "3de0e5", "4bede6", "65f5e3", "70fbf0", "9efeeb", "d5fff6",
               "ffffff")
COAL = Ramp("050505", "0d0d0d", "151515", "1f1e1e", "292828")
WOOL = Ramp("d1d7d8", "dbe0e1", "e4e7e8", "eeeff0", "f4f5f6", "fafbfb", "fefefe")
RED_TOP = Ramp("6b1213", "851a1a", "902120", "a22722", "ac2922", "b53129", "bf3b33")
LANT = Ramp("252c3d", "3e4453", "424a5e", "495065", "5a6278")
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
    shade = np.zeros((S, S), bool)
    for (x0, y0, n) in [(40, 16, 24), (112, 94, 18)]:
        for k in range(n):
            glint |= rect(y0 + k, x0 - k - 2, y0 + k + 1, x0 - k + 1)
            if 2 <= k < n - 2:
                shade |= rect(y0 + k + 1, x0 - k - 2, y0 + k + 2, x0 - k)
    paint(img, shade & ~glint & ~frame, GLASS_C.at(1))
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
    """Two courses of long stone bricks (the lower one offset by half and wrapping around),
    mottled with small horizontal speckle clusters, lit top/left edges, darker toward the
    bottom, and a dark 4 px mortar."""
    yy, xx = ints()
    # SBRICK: 0 5a595a, 1 636363, 2 6a6d6a (greenish), 3 787678, 4 7f7f7f, 5 8b898b, 6 9c999c
    row = yy // 64
    ly = yy % 64
    lx = np.where(row == 0, xx, (xx - 64) % S)   # 0 at each brick's left edge
    v = 5.15 - 2.1 * (ly / 58.0)
    v += (anoise(seed, 5, 14) - 0.5) * 1.9
    v += (anoise(seed + 1, 3, 7) - 0.5) * 1.1
    v += (pix(seed + 2, 1) - 0.5) * 0.35 + (anoise(seed + 8, 2, 4) - 0.5) * 0.9
    # Darker toward each brick's right end.
    v -= np.clip((lx - 110) / 14.0, 0, 1) * 0.6
    idx = np.clip(np.round(v), 2, 5).astype(np.int32)
    # Rare light specks in the upper half and greenish-dark specks low down.
    idx = np.where(anoise(seed + 4, 4, 10) + (pix(seed + 9, 2) - 0.5) * 0.12 > 1.12 - ly / 110.0, 2, idx)
    # Edges: 2 px light top and left, 2 px dark last row, dark right end column.
    idx = np.where(ly < 2, 6, idx)
    idx = np.where((ly >= 2) & (ly < 4), np.where(pix(seed + 5, 2) > 0.9, 4, 5), idx)
    idx = np.where((lx < 2) & (ly < 60), 6, idx)
    idx = np.where((ly >= 58) & (ly < 60), 2, idx)
    idx = np.where((lx >= 122) & (lx < 124) & (ly < 60), np.minimum(idx, 3), idx)
    mort = (ly >= 60) | (lx >= 124)
    idx = np.where(mort, np.where(pix(seed + 6, 2) > 0.45, 1, 0), idx)
    idx = np.where(mort & (ly >= 60) & (ly < 62) & (lx < 124), np.where(pix(seed + 7, 2) > 0.2,
                                                                          1, 0), idx)
    return img_of(SBRICK.colors[idx])


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


def on_tiled(fn, a):
    """Applies an image operation to `a` as if it repeated (for seamless morphology)."""
    h, w = a.shape[:2]
    return fn(np.tile(a, (3, 3)))[h:2 * h, w:2 * w]


def disk_se(r):
    yy, xx = np.mgrid[-r:r + 1, -r:r + 1]
    return xx * xx + yy * yy <= r * r + r * 0.6


def cobble_value(seed, count=10, gap=1.0, jitter=0.75, wobble=8):
    """Faithful-style furnace cobbles: big round, softly domed stones (light speckled centre,
    darker rim) packed with narrow mid-grey gaps. Returns STONE indices (floats, tiling)."""
    ids, _, border, best = cells(seed, count, jitter=jitter, wobble=wobble, cell=32)
    R = 0.7 * S / np.sqrt(count)
    stone = np.minimum(border, (R - best) * 0.9) > gap
    stone = on_tiled(lambda m: ndimage.binary_opening(m, disk_se(8)), stone)
    d = on_tiled(ndimage.distance_transform_edt, stone)
    near = on_tiled(ndimage.distance_transform_edt, ~stone)
    lit = np.clip(d - shifted(d, 3, 3), -3, 3)
    per = rng(seed + 1).uniform(-0.25, 0.25, count).astype(np.float32)
    rim = np.select([d < 2, d < 5.5, d < 10], [3.5, 4.3, 5.0], 5.7)
    v = rim + lit * 0.18 + per[ids]
    v += (anoise(seed + 2, 3, 6) - 0.5) * 1.0 + (pix(seed + 3, 1) - 0.5) * 0.7
    g = 2.0 + (pix(seed + 4, 2) - 0.5) * 0.9 + np.where(near < 1.5, 0.6, 0.0)
    g = np.where(pix(seed + 5, 2) > 0.95, 1.0, g)
    v = np.where(stone, v, g)
    return np.clip(v, 0, 6.4)


def furnace_frame(v, yy, xx, top=False):
    """Dark 4 px frame (504e4e with 3c3b3b ticks) around a furnace face."""
    b = (xx < 4) | (xx >= S - 4) | (yy < 4) | (yy >= S - 4)
    v = np.where(b, 1.0, v)
    tick = pix(17, 2) > 0.55
    v = np.where(((yy < 2) | (yy >= S - 2)) & tick, 0.0, v)
    v = np.where(((xx < 2) & ~top) & (pix(18, 2) > 0.7), 0.0, v)
    return v


def slab_value(seed, y0, y1):
    """The smooth light slab of the furnace's lower half (STONE indices): a bright top
    edge, pale stone with soft horizontal blotches darkening toward the bottom."""
    yy, xx = ints()
    ly = (yy - y0) / float(y1 - y0)
    v = 8.9 - ly * 2.0 + (anoise(seed, 4, 16) - 0.5) * 1.3 + (anoise(seed + 1, 2, 6) - 0.5) * 0.6
    v += (pix(seed + 2, 1) - 0.5) * 0.3
    v = np.where(yy < y0 + 2, 10.0, v)
    v = np.where((yy >= y0 + 2) & (yy < y0 + 4), 9.0, v)
    v = np.where((xx >= 4) & (xx < 6) & (yy >= y0 + 4), np.minimum(v, 7.0), v)
    v = np.where((xx >= S - 6) & (xx < S - 4) & (yy >= y0 + 4), np.minimum(v, 6.6), v)
    v = np.where((yy >= y1 - 4) & (yy < y1 - 2), 6.0, v)
    v = np.where(yy >= y1 - 2, np.where(pix(seed + 3, 2) > 0.5, 3.0, 4.0), v)
    return v


def as_stone(v):
    return STONE.colors[np.clip(np.round(v), 0, len(STONE) - 1).astype(np.int32)]


def paint_furnace_side(seed):
    yy, xx = ints()
    v = cobble_value(seed + 2, 16)
    v = np.where((xx < 6) | (yy < 6) | (xx >= S - 6), np.minimum(v, 2.6), v)
    v = np.where(yy >= 72, slab_value(seed + 5, 72, 124), v)
    return img_of(as_stone(furnace_frame(v, yy, xx)))


def paint_furnace_top(seed):
    yy, xx = ints()
    v = cobble_value(seed + 9, 9)
    return img_of(as_stone(furnace_frame(v, yy, xx, top=True)))


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
    v = cobble_value(seed + 2, 16)
    v = np.where((xx < 6) | (yy < 6) | (xx >= S - 6), np.minimum(v, 2.6), v)
    # A smooth pale ledge under the mouth, fading into the cobbles.
    ledge = (yy >= 56) & (yy - 56 < 14 - np.maximum(np.abs(xx - 63.5) - 34, 0) * 1.5)
    lv = 8.6 - (yy - 56) / 12.0 * 4.0 + (anoise(seed + 7, 3, 10) - 0.5) * 1.6
    lv = np.where(yy < 58, 9.4, lv)
    v = np.where(yy >= 72, slab_value(seed + 5, 72, 124), v)
    v = furnace_frame(v, yy, xx)
    up = arch(*UP_ARCH, 56)
    low = arch(*LOW_ARCH, 124)
    # Soft sooty rims around both openings (still lighter than the openings' outline).
    for m in (up, low):
        r1 = grow(m, 2) & ~m
        r2 = grow(m, 4) & ~grow(m, 2)
        v = np.where(r2 & (yy < 124), np.maximum(v - 2.0, 2.0), v)
        v = np.where(r1 & (yy < 124), np.maximum(v - 4.0, 1.0), v)
    v = np.where((yy >= 56) & (yy < 58) & (xx >= 20) & (xx < 108), 9.4, v)
    col = as_stone(v)
    dark = Ramp("111111", "212121", "3c3b3b")
    # Openings: black at their outline, the back wall faintly lit low in the middle.
    ku = np.hypot((yy - 52) / 14.0, (xx - 64) / 30.0)
    tu = np.where(ku < 0.55, 2, np.where(ku < 1.0, 1, 0))
    tu = np.where(shrink(up, 2), tu, 0)
    col = np.where(up[..., None], dark.colors[tu], col)
    kl = np.hypot((yy - 122) / 16.0, (xx - 64) / 30.0)
    tl = np.where(kl < 1.0, 1, 0)
    tl = np.where(shrink(low, 2), tl, 0)
    col = np.where(low[..., None], dark.colors[tl], col)
    return img_of(col), low


def paint_furnace_front(seed):
    return furnace_front_base()[0]


FIRE = Ramp("c35d1b", "ff8f00", "ffd800", "ffff97", "ffffff")


def paint_furnace_front_on(seed):
    """The unlit front with a fire filling the lower opening: pointed tongues (three tall
    ones), orange edges with dark red seams, yellow bodies and pale rising streaks."""
    img, low = furnace_front_base()
    yy, xx = grid()
    iy, ix = ints()
    peaks = [(30, 15, 14), (50, 21, 13), (70, 11, 9), (90, 23, 15), (108, 9, 8)]
    height = np.full((S, S), 12.0, np.float32)
    for cx, hgt, w in peaks:
        prof = np.clip(1 - np.abs(xx - cx) / w, 0, 1) ** 1.1 * hgt + 9
        height = np.maximum(height, prof)
    height += (pix(FURNACE_SEED + 4, 2) - 0.5) * 3
    flame = low & ((124 - yy) < height)
    ext = flame.copy()
    ext[124:, :] = low[123:124, :].repeat(S - 124, 0)
    depth = ndimage.distance_transform_edt(np.pad(ext, ((0, 8), (0, 0)), mode="edge"))[:S]
    idx = np.select([depth < 2, depth < 5, depth < 9], [1, 1, 2], 2)
    # Dark red seams: the upper outline of each tongue and the clefts between them.
    top_edge = flame & ~shifted(flame, 1, 0)
    idx = np.where(top_edge & (iy < 118), 0, idx)
    s = ((xx - 64) * 0.95 + (124 - yy) * 0.55 + anoise(FURNACE_SEED + 5, 8, 8) * 14) % 17
    idx = np.where((s < 5) & (depth >= 3.5), 3, idx)
    idx = np.where((s > 1) & (s < 3.2) & (depth >= 6), 4, idx)
    idx = np.where((s > 9) & (s < 10.5) & (depth >= 3) & (depth < 12), 1, idx)
    img[..., :3] = np.where(flame[..., None], FIRE.colors[idx], img[..., :3])
    return img


# ---------------------------------------------------------------------------- storage blocks


def paint_iron_block(seed):
    """Smooth light plate: six soft horizontal bands (bright top edge, flat face, darker
    groove below), a thin bevelled frame."""
    yy, xx = ints()
    r = rng(seed)
    # IRON: 0 b1, 1 b9, 2 c1, 3 d1, 4 d6, 5 dc, 6 e0, 7 e6, 8 ea, 9 ec, 10 f2
    idx = np.full((S, S), 7, np.int32)
    fine = pix(seed + 1, 1)
    for k in range(6):
        y0 = 4 + 20 * k
        ly = yy - y0
        inb = (ly >= 0) & (ly < 20)
        a = int(r.integers(30, 44))
        b = a + int(r.integers(16, 30))
        c = b + int(r.integers(6, 16))
        # Bright top edge fading to the right (2 px steps with 1 px staircase ends).
        e0 = inb & (ly < 2)
        idx = np.where(e0, np.select([xx < a, xx < b, xx < c], [10, 9, 8], 7), idx)
        e1 = inb & (ly >= 2) & (ly < 4)
        a2 = a + int(r.integers(10, 24))
        idx = np.where(e1, np.select([xx < a - 14, xx < a2, xx < a2 + 10], [9, 9, 8], 7), idx)
        # One faint brushed hair line in the face.
        hy = int(r.integers(6, 11))
        hx0 = int(r.integers(8, 70))
        hx1 = hx0 + int(r.integers(14, 40))
        idx = np.where(inb & (ly == hy) & (xx >= hx0) & (xx < hx1), 8, idx)
        hy2 = int(r.integers(11, 14))
        hx2 = int(r.integers(40, 100))
        idx = np.where(inb & (ly == hy2) & (xx >= hx2) & (xx < hx2 + int(r.integers(10, 24))),
                       6, idx)
        # Shadow above the groove, then the groove (lighter on the left).
        idx = np.where(inb & (ly >= 14) & (ly < 16) & (xx >= S - 10), 6, idx)
        idx = np.where(inb & (ly >= 16) & (ly < 18), 6, idx)
        g1, g2 = int(r.integers(78, 100)), int(r.integers(96, 112))
        idx = np.where(inb & (ly >= 18) & (ly < 20), np.where(xx < np.where(ly == 18, g1, g2), 5,
                                                              4), idx)
        # Frame pieces beside this band.
        idx = np.where(inb & (xx < 4), np.select([ly < 16, ly < 18], [2, 1], 0), idx)
        idx = np.where(inb & (ly == 19) & (xx >= 2) & (xx < 4), 1, idx)
        idx = np.where(inb & (xx >= S - 6) & (xx < S - 4) & (ly < 16), 6, idx)
        idx = np.where(inb & (xx >= S - 6) & (xx < S - 4) & (ly >= 16) & (ly < 18), 5, idx)
        idx = np.where(inb & (xx >= S - 4), np.where(ly < 16, 1, 0), idx)
        idx = np.where(inb & (xx >= S - 4) & (ly < 2) & (xx < S - 2), 2, idx)
    # Top and bottom frame.
    idx = np.where(yy < 4, 3, idx)
    idx = np.where((yy < 2) & (((xx >= 2) & (xx < 14)) | (xx >= 90)), 2, idx)
    idx = np.where((yy >= 2) & (yy < 4) & ((xx < 6) | (xx >= 102)), 2, idx)
    idx = np.where((yy < 2) & (xx < 2), 3, idx)
    bot = yy >= S - 4
    idx = np.where(bot, np.where(xx < np.where(yy < S - 2, 62, 56), 2, 1), idx)
    idx = np.where(bot & (yy < S - 2) & (xx >= 70) & (xx < 76), 2, idx)
    idx = np.where((yy >= S - 4) & (yy < S - 2) & (xx >= S - 2), 0, idx)
    return img_of(IRON.colors[idx])


def paint_copper_block(seed):
    """Smooth copper plate: soft diagonal sheen (light stripes, a dark valley across the
    middle), four corner rivets and a lit frame."""
    yy, xx = grid()
    s = (xx + yy) / 2.0          # 0..128 in Faithful 64 px units along the diagonal
    d = (xx - yy) / 2.0
    s = s + (noise(seed, 32) - 0.5) * 2.0
    keys = [(0, 5.0), (11, 5.0), (13, 4.6), (16, 3.9), (18, 4.4), (21, 4.4), (22.5, 5.0),
            (31, 5.0), (33, 5.6), (35, 6.3), (41, 6.3), (43, 5.6), (45, 5.0), (47, 5.3),
            (49, 5.3), (51, 5.0), (53, 4.4), (55, 3.6), (61, 3.6), (63, 2.6), (66, 2.0),
            (71, 2.2), (73, 2.8), (74.5, 4.0), (80, 3.9), (83, 3.5), (85, 4.0), (86.5, 5.0),
            (97, 5.0), (99, 5.6), (101, 6.3), (107, 6.2), (109, 5.3), (111, 4.4), (113, 4.0),
            (115, 4.5), (117, 4.0), (119, 4.4), (121, 5.0), (130, 5.0)]
    ks, vs = zip(*keys)
    v = np.interp(s, ks, vs).astype(np.float32)
    # The valley is a little darker toward the top right, lighter toward the bottom left.
    valley = np.clip(1 - np.abs(s - 62) / 12, 0, 1)
    v -= valley * np.clip(d / 50, -1, 1) * 0.5
    # Dashed bright stripes (7) inside the light bands, dark hairlines in the valley.
    dash = anoise(seed + 1, 4, 4)
    for c, w in ((36.5, 0.9), (39.5, 0.8), (103.0, 0.9), (105.8, 0.7)):
        on = (np.abs(s - c) < w) & (np.sin((d + c * 1.7) / 5.1 + dash * 2.5) > -0.55)
        v = np.where(on, 7.0, v)
    for c, w in ((66.5, 0.5), (57.5, 0.4), (116.0, 0.4)):
        on = (np.abs(s - c) < w) & (np.sin((d + c) / 4.3 + dash * 3) > 0.3)
        v = np.where(on, v - 1.0, v)
    idx = np.clip(np.round(v + (bayer(S, S) - 0.5) * 0.12), 0, 7).astype(np.int32)
    # Frame: bright left/top, dark right/bottom (2 px each side).
    iy, ix = ints()
    idx = np.where(ix < 2, 7, idx)
    idx = np.where((ix >= 2) & (ix < 4) & (iy >= 2), 6, idx)
    idx = np.where(iy < 2, np.where((ix // 8) % 5 == 2, 6, 7), idx)
    idx = np.where((iy >= 2) & (iy < 4) & (ix >= 4) & (ix < S - 4), np.clip(idx + 1, 0, 6), idx)
    idx = np.where((ix >= S - 4) & (ix < S - 2) & (iy >= 2), 2, idx)
    idx = np.where(ix >= S - 2, np.where(iy < 2, 3, 1), idx)
    idx = np.where((iy >= S - 4) & (iy < S - 2) & (ix >= 2) & (ix < S - 2), 2, idx)
    idx = np.where(iy >= S - 2, np.where(pix(seed + 3, 2) > 0.5, 1, 0), idx)
    idx = np.where((iy >= S - 2) & (ix < 2), 6, idx)
    # Rivets: small domed heads lit from the top left.
    for cy, cx in ((17, 17), (17, 109), (109, 17), (109, 107)):
        rr = np.hypot(yy - cy, xx - cx)
        head = rr <= 5.6
        lit = ((xx - cx) + (yy - cy)) / 1.414
        ri = np.where(lit < -2.5, 7, np.where(lit < 0.5, 5, np.where(lit < 3, 4, 3)))
        ri = np.where(rr > 4.3, np.where(lit < -1, 7, np.where(lit < 2, 4, 1)), ri)
        ri = np.where(np.hypot(yy - cy + 1.5, xx - cx + 1.5) < 1.3, 7, ri)
        idx = np.where(head, ri, idx)
        shadow = (rr > 5.6) & (np.hypot(yy - cy - 1.5, xx - cx - 1.5) <= 5.6)
        idx = np.where(shadow, np.clip(idx - 2, 0, 7), idx)
    return img_of(CU_BLOCK.colors[idx])


def gem_bands(seed, ramp):
    """Gold/diamond block (9-colour ramps): a bevelled frame around a polished face with
    broad, softly stepped diagonal sheen bands and a few thin bright glints, rising to the
    top right like Faithful's."""
    yy, xx = grid()
    iy, ix = ints()
    q = xx - yy
    p = xx + yy + 4 * np.sin(q / 60.0 + 0.6) + 1.5 * np.sin(q / 17.0 + 2.0)
    v = np.full((S, S), 4.0, np.float32)
    bands = [(-8, 18, -1.3), (30, 12, 1.2), (52, 13, 1.9), (60, 3.5, 1.4), (92, 17, -1.8),
             (128, 20, 0.9), (150, 9, 1.8), (156, 2.5, 1.2), (184, 17, -1.6), (212, 12, 1.5),
             (222, 3, 1.3), (248, 14, -1.1), (270, 12, 1.0)]
    for c, w, amp in bands:
        g = np.clip(1 - np.abs(p - c) / w, 0, 1) ** 0.8
        v += amp * g * (1 + 0.35 * np.sin(q / 23.0 + c))
    v += (anoise(seed, 12, 12) - 0.5) * 1.0
    # Short bright glints along the bands.
    r = rng(seed + 3)
    for c, q0, ln, wd in ((58, -10, 44, 4.5), (150, 30, 50, 4.0), (220, -20, 30, 3.5),
                          (104, 60, 24, 3.0), (190, -80, 20, 3.0)):
        c += r.uniform(-6, 6)
        dq = np.abs(q - q0) / ln
        dp = np.abs(p - c) / (wd * np.sqrt(np.clip(1 - dq ** 2, 0, 1)) + 1e-3)
        v = np.where(dp < 1, np.maximum(v + 1.6, 6.0), v)
        v = np.where(dp < 0.45, np.maximum(v + 1.0, 7.4), v)
    idx = np.clip(np.round(v + (bayer(S, S) - 0.5) * 0.2), 2, 8).astype(np.int32)
    # Frame: lit top/left, shaded bottom/right, with a bright inner bevel line top/left.
    idx = np.where((iy < 4) | (ix < 4), 3, idx)
    idx = np.where(((iy < 2) & (ix >= 36) & (ix < 112)) | ((ix < 2) & (iy >= 36)), 2, idx)
    idx = np.where(((iy >= 4) & (iy < 6) & (ix >= 4)) | ((ix >= 4) & (ix < 6) & (iy >= 4)),
                   np.where((ix < 64 - iy // 2) & (iy < 64 - ix // 2), 8, 7), idx)
    idx = np.where(((iy >= 6) & (iy < 8) & (ix >= 6)) | ((ix >= 6) & (ix < 8) & (iy >= 6)),
                   np.where((ix < 40) & (iy < 40), 7, 6), idx)
    idx = np.where((ix >= S - 6) & (ix < S - 4) & (iy >= 4), np.clip(idx + 1, 3, 7), idx)
    idx = np.where((iy >= S - 6) & (iy < S - 4) & (ix >= 4), np.clip(idx - 1, 2, 6), idx)
    idx = np.where((ix >= S - 4) | (iy >= S - 4), 1, idx)
    idx = np.where(((ix >= S - 2) | (iy >= S - 2)) & (pix(seed + 1, 2) > 0.5), 0, idx)
    idx = np.where(((ix >= S - 4) & (iy < 4)) | ((iy >= S - 4) & (ix < 4)), 2, idx)
    return img_of(ramp.colors[idx])


def paint_gold_block(seed):
    return gem_bands(seed, GOLD)


def paint_diamond_block(seed):
    return gem_bands(seed, DIAMOND)


def lumps(seed, n, angle, lengths, widths, shade_fn, base, h=S, w=S, jag=0.0):
    """Tiling pile of rounded elongated lumps along `angle` (radians, y down). For each lump
    `shade_fn(u, q)` gives a value from the along (-1..1) and across (-1 top .. 1 bottom)
    coordinates; later lumps lie on top. Returns (value map, lump id map)."""
    r = rng(seed)
    t = np.full((h, w), base, np.float32)
    ids = np.full((h, w), -1, np.int32)
    jn = pix(seed + 99, 2, h, w) - 0.5
    for k in range(n):
        cy, cx = r.uniform(0, h), r.uniform(0, w)
        a = angle + r.uniform(-0.12, 0.12)
        L = r.uniform(*lengths) / 2
        W = r.uniform(*widths) / 2
        R = int(L + W + 2)
        ys = np.arange(int(cy) - R, int(cy) + R + 1)
        xs = np.arange(int(cx) - R, int(cx) + R + 1)
        dy = (ys + 0.5 - cy)[:, None]
        dx = (xs + 0.5 - cx)[None, :]
        ca, sa = np.cos(a), np.sin(a)
        u = (dx * ca + dy * sa) / L
        q = (-dx * sa + dy * ca) / W
        iy, ix = np.ix_(ys % h, xs % w)
        rad = u ** 2 + q ** 2 + jn[iy, ix] * jag
        body = rad < 1
        cur = t[iy, ix]
        t[iy, ix] = np.where(body, shade_fn(u, q, rad), cur)
        ids[iy, ix] = np.where(body, k, ids[iy, ix])
    return t, ids


def paint_coal_block(seed):
    """Packed glossy coal lumps along a shallow rising diagonal: black gaps, dark bodies, a
    lighter upper band and a small bright gloss streak on each."""
    # COAL: 0 050505, 1 0d0d0d, 2 151515, 3 1f1e1e, 4 292828

    def shade(u, q, rad):
        v = np.full(u.shape, 1.0, np.float32)
        v = np.where(q > 0.45, 0.0, v)
        band = (q > -0.8) & (q < 0.15) & (np.abs(u) < 0.85) & (rad < 0.8)
        v = np.where(band, 2.0, v)
        band2 = (q > -0.62) & (q < -0.12) & (u > -0.7) & (u < 0.45)
        v = np.where(band2, 3.0, v)
        gl = (q > -0.52) & (q < -0.3) & (u > -0.5) & (u < 0.05)
        v = np.where(gl, 4.0, v)
        return v

    t, ids = lumps(seed, 17, -0.34, (56, 96), (18, 30), shade, 0.0, jag=0.3)
    # Outline each lump where it meets another lump or the gap.
    edge = np.zeros((S, S), bool)
    for dy, dx in ((1, 0), (0, 1), (-1, 0), (0, -1)):
        edge |= shifted(ids, dy, dx) != ids
    t = np.where(edge & (shifted(ids, 1, 0) != ids) | edge & (shifted(ids, 0, 1) != ids), 0.0, t)
    # Rough dull fragments in the gaps.
    gap = ids < 0
    t = np.where(gap & (anoise(seed + 5, 4, 8) > 0.6), 1.0, t)
    idx = np.clip(np.round(t), 0, 4).astype(np.int32)
    return img_of(COAL.colors[idx])


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


CHAIN_C = Ramp("252c3d", "3e4453", "495065", "5a6278")
LGLOW = Ramp("814023", "8b5230", "c36322", "f09149", "f9c966", "fdfd8b", "ffffd5")


def rrect_sd(yy, xx, y0, x0, y1, x1, r):
    """Signed distance to a rounded rectangle [y0, y1) x [x0, x1) (negative inside)."""
    cy, cx = (y0 + y1) / 2, (x0 + x1) / 2
    hy, hx = (y1 - y0) / 2 - r, (x1 - x0) / 2 - r
    qy, qx = np.abs(yy - cy) - hy, np.abs(xx - cx) - hx
    out = np.hypot(np.maximum(qy, 0), np.maximum(qx, 0))
    return out + np.minimum(np.maximum(qy, qx), 0) - r


def tube_ring(img, y0, x0, y1, x1, wall, r, clip=None, wrap=False, ramp=CHAIN_C):
    """A metal link seen face on: a rounded rectangular ring of round wire `wall` px thick,
    lit from the top left (a lit ridge on the wire's upper-left side, dark lower right)."""
    yy, xx = grid()
    shifts = (-S, 0, S) if wrap else (0,)
    sd = np.min([rrect_sd(yy + s, xx, y0, x0, y1, x1, r) for s in shifts], axis=0)
    m = (sd <= 0) & (sd > -wall)
    if clip is not None:
        m &= clip
    gy, gx = np.gradient(sd)
    nl = np.hypot(gy, gx) + 1e-6
    ny, nx = gy / nl, gx / nl
    w = np.clip(-sd / wall, 0, 1)
    c, s = np.cos(np.pi * w), np.sin(np.pi * w)
    L = np.array([-0.55, -0.55, 0.63])
    b = nx * c * L[0] + ny * c * L[1] + s * L[2]
    idx = np.select([b > 0.9, b > 0.74, b > 0.3], [3, 2, 1], 0)
    paint(img, m, ramp.colors[idx])
    return m


def paint_lantern(seed):
    """Minecraft's lantern layout (8 px units): cap, glowing body, top/bottom, ring, handle."""
    img = blank()
    yy, xx = grid()
    iy, ix = ints()
    M = LANT  # 0 252c3d, 1 3e4453, 2 424a5e, 3 495065, 4 5a6278
    # Cap sides (1,0)-(5,2): dark rim, lighter plate, a rusty band.
    cap = rect(0, 8, 16, 40)
    idx = np.full((S, S), 3)
    idx = np.where((iy < 2) | (ix < 10) | (ix >= 38) | (iy >= 14), 1, idx)
    idx = np.where(((iy < 4) | (iy >= 12)) & ((ix < 12) | (ix >= 36)), 1, idx)
    idx = np.where((iy >= 2) & (iy < 4) & (ix >= 12) & (ix < 26), 4, idx)
    paint(img, cap, M.colors[idx])
    band = rect(8, 8, 12, 40)
    paint(img, band, LGLOW.colors[np.where((ix < 10) | (ix >= 38), 1, 0)])
    paint(img, rect(8, 12, 9, 36) & (pix(seed, 2) > 0.6), LGLOW.at(1))
    # Body sides (0,2)-(6,9): rounded metal shoulders, rusty frame, glowing window.
    body = rect(16, 0, 72, 48)
    paint(img, body, M.at(1))
    for y0, flip in ((16, False), (64, True)):
        ly = iy - y0 if not flip else (y0 + 7) - iy
        inset = np.select([ly < 2, ly < 4, ly < 6], [99, 12, 8], 6)
        plate = rect(y0, 0, y0 + 8, 48) & (np.abs(ix - 23.5) < 24 - inset)
        paint(img, plate, M.at(3))
        paint(img, plate & (ly >= 2) & (ly < 4) & (ix < 24) & ~flip, M.at(4))
    win = rect(24, 0, 64, 48)
    paint(img, win, LGLOW.colors[np.where((ix < 2) | (ix >= 46), 0, 1)])
    glass = rect(24, 4, 64, 44)
    k = np.maximum(np.abs(xx - 24) / 20.0, np.abs(yy - 44) / 20.0) * 0.55 +         np.hypot((xx - 24) / 20.0, (yy - 44) / 20.0) * 0.5
    gi = np.select([k > 0.98, k > 0.86, k > 0.62], [2, 3, 4], 5)
    streak = (np.abs((xx - 14) - (yy - 40) * 1.1) < 2.6) & (yy > 38) & (yy < 58)
    gi = np.where(streak & (k < 0.8), 6, gi)
    gi = np.where((np.abs((xx - 20) - (yy - 38) * 1.1) < 1.0) & (yy > 36) & (yy < 44) & (k < 0.8),
                  6, gi)
    paint(img, glass, LGLOW.colors[gi])
    # Body top/bottom (0,9)-(6,15): a plate with a raised round lid.
    top = rect(72, 0, 120, 48)
    paint(img, top, M.at(1))
    dd = np.hypot(yy - 96, xx - 24)
    paint(img, top & (dd < 24), M.at(2))
    paint(img, top & (dd < 22), M.at(3))
    paint(img, top & (dd < 20) & (dd > 17) & (xx + yy < 112), M.at(4))
    paint(img, top & (dd < 20) & (dd > 17) & (xx + yy > 128), M.at(2))
    # Hanging ring (11,1)-(14,5), a lower loop (11,6)-(14,8), standing handle (11,10)-(14,12).
    tube_ring(img, 8, 88, 40, 112, 6, 9)
    tube_ring(img, 32, 88, 64, 112, 6, 9, clip=rect(48, 0, 64, S))
    tube_ring(img, 80, 88, 112, 112, 6, 9, clip=rect(80, 0, 96, S))
    return img


def paint_iron_chain(seed):
    """Minecraft's chain layout: two 24 px strips (the two crossed planes), each a column of
    round-wire links seen face on, staggered between the strips; tiles vertically."""
    img = blank()
    for x0, links in ((0, ((8, 32), (48, 80), (96, 120))), (24, ((24, 56), (72, 104), (112, 144)))):
        for y0, y1 in links:
            tube_ring(img, y0, x0, y1, x0 + 24, 6, 9, clip=rect(0, x0, S, x0 + 24), wrap=True)
    return img


# ---------------------------------------------------------------------------- door


def door_wood(seed, Y, xx):
    """Door wood as DOOR indices: short horizontal grain streaks, dark and light."""
    n = anoise(seed, 3, 14) * 0.75 + anoise(seed + 1, 2, 6) * 0.25
    idx = np.select([n < 0.3, n < 0.35, n > 0.63], [2, 2.5, 4], 3)
    idx = np.where((n < 0.24) & (pix(seed + 4, 2) > 0.5), 1, idx)
    idx = np.where((n > 0.78) & (pix(seed + 5, 2) > 0.6), 5, idx)
    return np.floor(idx + (pix(seed + 6, 1) > 0.5) * (idx % 1 > 0)).astype(np.int32)


DOOR_PANELS = [(20, 48), (60, 88), (108, 136), (156, 184), (204, 232)]


def door_panels(seed, top: bool):
    """Door half as DOOR indices: stiles and rails with a 2 x 5 grid of recessed panels over
    the whole door (glass windows in the top two rows). Returns (indices, window mask)."""
    yy, xx = ints()
    Y = yy + (0 if top else S)  # door coordinates 0..255
    idx = door_wood(seed + (0 if top else 50), Y, xx)
    windows = np.zeros((S, S), bool)
    for i, (y0, y1) in enumerate(DOOR_PANELS):
        for (x0, x1) in ((20, 56), (68, 104)):
            m = (Y >= y0) & (Y < y1) & (xx >= x0) & (xx < x1)
            idx = np.where(m, np.where((idx == 3) & (pix(seed + 11, 2) > 0.55), 2,
                                       np.minimum(idx, 3)), idx)
            idx = np.where(m & (Y < y0 + 2), 0, idx)
            idx = np.where(m & (Y >= y0 + 2) & (Y < y0 + 4), 1, idx)
            idx = np.where(m & (xx < x0 + 2) & (Y >= y0 + 2), np.where(Y < y0 + 8, 0, 1), idx)
            idx = np.where(m & (xx >= x0 + 2) & (xx < x0 + 4) & (Y >= y0 + 4),
                           np.minimum(idx, 2), idx)
            idx = np.where((Y >= y0) & (Y < y1 + 4) & (xx >= x1) & (xx < x1 + 4),
                           np.where(xx < x1 + 2, 4, 5), idx)
            idx = np.where((Y >= y1) & (Y < y1 + 4) & (xx >= x0) & (xx < x1 + 2),
                           np.where(Y < y1 + 2, 5, 4), idx)
            if i < 2:
                windows |= (Y >= y0 + 4) & (Y < y1) & (xx >= x0 + 4) & (xx < x1)
    # Outer stiles: light left edge, dark right edge; top and bottom edges.
    idx = np.where(xx < 2, np.where(pix(seed + 7, 2) > 0.5, 6, 5), idx)
    idx = np.where((xx >= 2) & (xx < 4), 4, idx)
    idx = np.where(xx >= S - 2, 1, idx)
    idx = np.where((xx >= S - 4) & (xx < S - 2), 2, idx)
    if top:
        idx = np.where(yy < 2, np.where(pix(seed + 8, 2) > 0.4, 6, 3), idx)
        idx = np.where((yy >= 2) & (yy < 4) & (xx >= 2), 4, idx)
    else:
        idx = np.where(yy >= S - 4, np.where((yy >= S - 2) & (pix(seed + 9, 2) > 0.5), 0, 1), idx)
    return idx, windows


def hinge(img, y0, y1, top_edge=True, bottom_edge=True):
    """A grey hinge leaf on the left edge, lit on the left, a dark lower edge."""
    yy, xx = ints()
    m = rect(y0, 0, y1, 8)
    idx = np.select([xx < 2, xx < 4, xx < 6], [4, 3, 2], 1)
    if bottom_edge:
        idx = np.where(yy >= y1 - 2, 0, idx)
    if top_edge:
        idx = np.where((yy < y0 + 2) & (xx < 6), 5, idx)
    paint(img, m, HINGE.colors[idx])


def handle(img):
    """The door's pull handle at the bottom right of the top half: a rounded bar on two posts
    with a shadow on the wood."""
    yy, xx = grid()
    iy, ix = ints()
    bar = (rrect_sd(yy, xx, 116, 92, 123, 118, 3) <= 0)
    posts = rect(122, 94, 128, 98) | rect(122, 112, 128, 116)
    shadow = (rect(123, 90, 128, 120) & ~posts) & ~bar
    paint(img, shadow & (ix >= 98) & (ix < 112), DOOR.at(0))
    idx = np.where(iy < 118, 5, np.where(iy < 120, 4, 2))
    idx = np.where((ix < 94) | (ix >= 116), np.minimum(idx, 3), idx)
    paint(img, bar, HINGE.colors[idx])
    paint(img, posts, HINGE.colors[np.where(ix % 4 < 2, 2, 0)])


def paint_oak_door_top(seed):
    idx, win = door_panels(seed, True)
    img = img_of(DOOR.colors[idx])
    img[win] = 0
    hinge(img, 32, 48)
    hinge(img, 120, 128, bottom_edge=False)
    handle(img)
    return img


def paint_oak_door_bottom(seed):
    idx, _ = door_panels(seed, False)
    img = img_of(DOOR.colors[idx])
    hinge(img, 0, 8, top_edge=False)
    hinge(img, 80, 96)
    return img


# ---------------------------------------------------------------------------- wool


def fibres(seed, n, angles, lengths, widths, base=0.5, h=S, w=S, shadow=0.22, light=0.3,
           values=(0.35, 0.75)):
    """Tiling tangle of short lens-shaped strands (wool, fluff). Each strand is rounded:
    lit on its upper side, darker below, with a thin dark shadow under it; later strands lie
    on top. Returns a 0..1 value map."""
    r = rng(seed)
    t = np.full((h, w), base, np.float32)
    for _ in range(n):
        cy, cx = r.uniform(0, h), r.uniform(0, w)
        a = angles[int(r.integers(len(angles)))] + r.uniform(-0.15, 0.15)
        L = r.uniform(*lengths) / 2
        W = r.uniform(*widths) / 2
        v = r.uniform(*values)
        R = int(L + W + 3)
        ys = np.arange(int(cy) - R, int(cy) + R + 1)
        xs = np.arange(int(cx) - R, int(cx) + R + 1)
        dy = (ys + 0.5 - cy)[:, None]
        dx = (xs + 0.5 - cx)[None, :]
        ca, sa = np.cos(a), np.sin(a)
        u = dx * ca + dy * sa
        q = -dx * sa + dy * ca
        if ca < 0:
            q = -q
        prof = np.sqrt(np.clip(1 - (u / L) ** 2, 0, 1))
        half = W * prof
        body = np.abs(q) < half
        sh = (~body) & (q > 0) & (q < half + 1.6) & (prof > 0.15)
        # q < 0 is the upper side of the strand (y grows downward).
        across = np.where(half > 0, q / np.maximum(half, 1e-3), 0)
        val = v - across * light - (u / L) ** 2 * 0.12
        iy, ix = np.ix_(ys % h, xs % w)
        cur = t[iy, ix]
        cur = np.where(sh, cur - shadow, cur)
        cur = np.where(body, val, cur)
        t[iy, ix] = cur
    return t


def paint_white_wool(seed):
    """Soft felted wool: a dense tangle of short fibre strands at shallow crossing angles."""
    t = fibres(seed, 360, (0.45, -0.45, 0.32, -0.32), (16, 38), (4.5, 8.5), base=0.2,
               values=(0.25, 0.62), light=0.24, shadow=0.12)
    t += (noise(seed + 3, 32) - 0.5) * 0.16 + (pix(seed + 4, 2) - 0.5) * 0.08
    return img_of(WOOL.shade(t, 0.45))


# ---------------------------------------------------------------------------- bed


BLANKET_FOLDS = [
    # centre x, centre y, radius, start and end angle (degrees, y down), thickness, ridge side
    (64, -70, 86, 42, 138, 13, 1),      # the sagging fold across the top
    (-60, 64, 70, -52, 52, 12, -1),     # shade down the left edge
    (54, 34, 26, 58, 142, 7, 1),        # crescent in the middle
    (40, 76, 62, -44, 44, 10, 1),       # the long curve on the right
    (50, 20, 68, 93, 152, 9, 1),        # sweep in the lower left
    (64, 250, 130, -118, -62, 12, -1),  # fold along the bottom
]


def swirl_value(seed):
    """Red blanket: plain cloth in broad soft light and shade, with a few long crescent folds
    (a darker hollow with a deeper crease, a light ridge beside it). RED_TOP values."""
    yy, xx = grid()
    r = rng(seed)
    wy = (noise(seed, 64) - 0.5) * 60 + (noise(seed + 5, 32) - 0.5) * 16
    wx = (noise(seed + 1, 64) - 0.5) * 60
    f = np.sin((yy + wy) / 70 * 2 * np.pi + (xx + wx) / 97.0 * np.pi) * 0.7
    f += (noise(seed + 2, 64) - 0.5) * 1.1
    idx = np.select([f < -0.62, f < 0.72], [4.0, 5.0], 6.0).astype(np.float32)
    light = np.zeros((S, S), bool)
    wob = (noise(seed + 9, 32) - 0.5) * 12
    for cx, cy, rad, a0, a1, th, side in BLANKET_FOLDS:
        cx, cy = cx + r.uniform(-3, 3), cy + r.uniform(-3, 3)
        dy, dx = yy - cy, xx - cx
        rr = np.hypot(dy, dx) + wob
        ang = np.degrees(np.arctan2(dy, dx))
        ang = np.where(ang < a0 - 90, ang + 360, ang)
        along = np.clip((ang - a0) / (a1 - a0), 0, 1)
        inside = (ang > a0) & (ang < a1)
        half = th / 2 * np.maximum(np.sin(np.pi * along ** 0.8), 0) ** 1.1
        off = rr - rad
        body = inside & (np.abs(off) < half)
        idx = np.where(body, np.minimum(idx, 4.0), idx)
        crease = body & (np.abs(off + side * half * 0.3) < half * 0.35) & (half > 3)
        idx = np.where(crease, 3.0, idx)
        light |= inside & (off * side > half) & (off * side < half + 3) & (half > 2)
    idx = np.where(light & (idx >= 5), 6.0, idx)
    return idx / 6.0


def paint_red_bed_foot_up(seed):
    return img_of(stepped(swirl_value(seed), RED_TOP, 0))


def paint_red_bed_head_up(seed):
    yy, xx = ints()
    t = swirl_value(seed + 1) - level(RED_TOP, 1)
    # The blanket's turned-down edge (rows 64..108): darker cloth with a wavy light ridge.
    fy, fx = grid()
    idx = np.full((S, S), 2.0)
    idx = np.where(anoise(seed + 4, 8, 24) > 0.62, 3.0, idx)
    bowl = (fy >= 66) & (fy < 66 + 12 * np.clip(1 - np.abs(fx - 76) / 34, 0, 1) ** 0.7)
    idx = np.where(bowl, 1.0, idx)
    idx = np.where((fy >= 86) & (anoise(seed + 5, 10, 30) > 0.45), np.maximum(idx, 3.0), idx)
    kx = [0, 30, 64, 90, 110, 128]
    yc = np.interp(fx, kx, [95, 97, 100, 92, 82, 80]) + (noise(seed + 6, 16) - 0.5) * 3
    th = np.interp(fx, kx, [21, 14, 6, 5, 7, 4])
    dd = np.abs(fy - yc)
    idx = np.where(dd < th / 2, 5.0, idx)
    idx = np.where(dd < th * 0.3, 6.0, idx)
    idx = np.where(yy < 66, 0.0, idx)
    t = np.where((yy >= 64) & (yy < 108), idx / 6.0, t)
    t = np.where((yy >= 108) & (yy < 110), np.where(pix(seed + 7, 2) > 0.5, 1, 2) / 6.0, t)
    col = stepped(t, RED_TOP, 0)
    # Sheet and the rounded pillow.
    ts = np.full((S, S), level(SHEET, 4))
    ts = np.where((yy >= 58), level(SHEET, 3), ts)
    ts = np.where((yy >= 60) & (pix(seed + 8, 2) > 0.5), level(SHEET, 2), ts)
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
    # The blanket hanging over the side: darker cloth with soft horizontal folds.
    n = anoise(seed, 6, 40) * 0.7 + anoise(seed + 1, 3, 14) * 0.3
    bi = np.select([n < 0.3, n < 0.62, n < 0.74], [1, 2, 3], 4)
    bi = np.where((yy >= 56) & (yy < 58), 4, bi)
    bi = np.where((yy >= 58) & (yy < 60), np.minimum(bi + 1, 4), bi)
    bi = np.where(yy >= 84, 1, bi)
    bi = np.where(yy >= 86, 0, bi)
    paint(img, band, RED_TOP.colors[bi])
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
    n = 4
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
        bend = r.uniform(-0.08, 0.08) * L
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
    for _ in range(10):
        k = int(r.integers(len(ys_)))
        x, y, d = xs_[k] + 0.5, ys_[k] + 0.5, max(float(dist[ys_[k], xs_[k]]), 70.0)
        a = r.uniform(0, 2 * np.pi)
        for q in range(int(r.uniform(5, 11))):
            a += r.uniform(-0.3, 0.3)
            x, y = x + np.cos(a) * 1.5, y + np.sin(a) * 1.5
            stamp(x, y, d + q * 1.5 + 1)
    crack = np.isfinite(dist)
    # Stage limits from the share of all crack pixels each stage shows (like Faithful's).
    ds = np.sort(dist[crack])
    limits = [ds[min(len(ds) - 1, int(f * len(ds)))] for f in STAGE_SHARE[:9]] + [np.inf]
    stage = np.full((S, S), 9, np.int32)
    for k in range(9, -1, -1):
        stage = np.where(crack & (dist <= limits[k]), k, stage)
    _CRACKS["f"] = (crack, stage)
    return _CRACKS["f"]


STAGE_SHARE = [0.025, 0.065, 0.12, 0.19, 0.25, 0.36, 0.48, 0.65, 0.87, 1.0]


def destroy(k):
    def paint_stage(seed):
        crack, stage = crack_field()
        m = crack & (stage <= k)
        img = blank()
        light = (shifted(m, 1, 0) | shifted(m, 0, 1) | shifted(m, 2, 0) | shifted(m, 0, 2)) & ~m
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
    "block/copper_block": paint_copper_block,
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
