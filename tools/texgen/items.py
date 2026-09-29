"""Item sprites in the Faithful 64x look, drawn natively at 128x128.

Faithful's items are flat-shaded pixel art: each part is a few flat colors (a body color, a
lit rim on the top-left edges, a shade band on the bottom-right edges) with a thin outline,
lighter on the lit side. We lay every part out on Faithful's 64 grid (outlines, facet lines,
tool staircases; doubled to 128) and draw it from our own geometric masks (polygons,
ellipses, variable-width strokes, 45 degree pixel bands) at full 128 resolution with 2 px
rims and outlines. Colors are Faithful's palettes, measured by eye.
"""

from __future__ import annotations

import numpy as np
from scipy import ndimage

from common import (
    S, blank, disk, ellipse, grid, grow, hexc, noise, pix, polygon, rect, rng, shrink,
    thick_line,
)

YY, XX = grid()


def C(h: str) -> np.ndarray:
    return hexc(h)


# ---------------------------------------------------------------------------- mask helpers


def shift(m: np.ndarray, dy: int, dx: int) -> np.ndarray:
    """Mask moved by (dy, dx), not wrapping."""
    out = np.zeros_like(m)
    h, w = m.shape
    ys, yd = (slice(0, h - dy), slice(dy, h)) if dy >= 0 else (slice(-dy, h), slice(0, h + dy))
    xs, xd = (slice(0, w - dx), slice(dx, w)) if dx >= 0 else (slice(-dx, w), slice(0, w + dx))
    out[yd, xd] = m[ys, xs]
    return out


def band(m: np.ndarray, dy: int, dx: int, k: int = 2) -> np.ndarray:
    """Pixels of `m` that leave `m` within k steps in direction (dy, dx): an edge band on
    that side (dy, dx = -1, -1 is the lit top-left rim)."""
    inside = m.copy()
    for j in range(1, k + 1):
        inside &= shift(m, -dy * j, -dx * j)
    return m & ~inside


def lit(m, k=2):
    """Top-left rim: pixels near the top or the left edge."""
    return band(m, -1, 0, k) | band(m, 0, -1, k)


def dark(m, k=2):
    """Bottom-right rim."""
    return band(m, 1, 0, k) | band(m, 0, 1, k)


def drop(m: np.ndarray, k: int = 2) -> np.ndarray:
    """Outline ring on the shadow side (below and right of the shape)."""
    g = np.zeros_like(m)
    for a in range(0, k + 1):
        for b in range(0, k + 1):
            if a or b:
                g |= shift(m, a, b)
    return g & ~m


def ring(m, k=2):
    return grow(m, k) & ~m


def smooth_poly(pts, iters=2):
    """Chaikin-rounded closed polygon (x, y) -> mask."""
    p = np.array(pts, np.float32)
    for _ in range(iters):
        q = np.roll(p, -1, 0)
        p = np.concatenate([p * 0.75 + q * 0.25, p * 0.25 + q * 0.75], 1).reshape(-1, 2)
    return polygon([tuple(v) for v in p])


def P(pts, iters=2):
    """`smooth_poly` with the points given on Faithful's 64 grid (x, y), doubled."""
    return smooth_poly([(2 * x, 2 * y) for x, y in pts], iters)


def line_dist(pts):
    """Distance (px) to a smooth centre line through `pts` given on the 64 grid."""
    c = stroke([(2 * x, 2 * y) for x, y in pts], [1.5] * len(pts))[0]
    return ndimage.distance_transform_edt(~c)


def Q(pts):
    """Plain polygon with points on the 64 grid, doubled."""
    return polygon([(2 * x, 2 * y) for x, y in pts])


def seg(p0, p1, w=2.0):
    """A straight facet line between two points on the 64 grid."""
    return thick_line((2 * p0[0], 2 * p0[1]), (2 * p1[0], 2 * p1[1]), w)


def rim(m, k):
    """Pixels of `m` within k steps (horizontally or vertically) of its edge: like Faithful's
    pixel runs, so a 45 degree edge gets a band k px wide measured along a row."""
    return band(m, 0, 1, k) | band(m, 0, -1, k) | band(m, 1, 0, k) | band(m, -1, 0, k)


def rrect(cx, cy, hl, hw, deg, r):
    """Rounded rectangle centred at (cx, cy), half length hl along `deg` (screen degrees,
    counter-clockwise from +x), half width hw, corner radius r. Also returns (u, v)."""
    t = np.radians(deg)
    dx, dy = XX - cx, YY - cy
    u = dx * np.cos(t) - dy * np.sin(t)
    v = -dx * np.sin(t) - dy * np.cos(t)
    qu, qv = np.maximum(np.abs(u) - (hl - r), 0), np.maximum(np.abs(v) - (hw - r), 0)
    return (qu**2 + qv**2 <= r * r), u, v


def stroke(pts, widths, n=160):
    """Variable-width smooth stroke through `pts` (x, y) (Catmull-Rom). Returns the mask, the
    signed side (+ = left of the travel direction on screen) and the parameter 0..1."""
    p = np.array(pts, np.float32)
    w = np.array(widths, np.float32)
    P = np.concatenate([p[:1], p, p[-1:]])
    samples, ws = [], []
    segs = len(p) - 1
    for i in range(segs):
        p0, p1, p2, p3 = P[i], P[i + 1], P[i + 2], P[i + 3]
        for t in np.linspace(0, 1, max(2, n // segs), endpoint=False):
            t2, t3 = t * t, t * t * t
            samples.append(0.5 * ((2 * p1) + (-p0 + p2) * t + (2 * p0 - 5 * p1 + 4 * p2 - p3) * t2
                                  + (-p0 + 3 * p1 - 3 * p2 + p3) * t3))
            ws.append(w[i] + (w[i + 1] - w[i]) * t)
    samples.append(p[-1])
    ws.append(w[-1])
    s = np.array(samples)
    ws = np.array(ws)
    tang = np.gradient(s, axis=0)
    d = np.hypot(XX[..., None] - s[:, 0], YY[..., None] - s[:, 1])
    k = np.argmin(d - ws / 2, axis=2)
    m = np.take_along_axis(d - ws / 2, k[..., None], 2)[..., 0] <= 0
    tx, ty = tang[k, 0], tang[k, 1]
    vx, vy = XX - s[k, 0], YY - s[k, 1]
    side = np.sign(tx * vy - ty * vx) * -1  # screen y is down: + = left of travel
    return m, side, k / (len(s) - 1)


class Img:
    def __init__(self):
        self.a = blank()

    def put(self, mask, col, alpha=255):
        m = mask.astype(bool)
        col = np.asarray(col, np.float32)
        self.a[m, :3] = col[m] if col.ndim == 3 else col
        self.a[m, 3] = alpha
        return self

    @property
    def m(self):
        return self.a[..., 3] > 0


# ---------------------------------------------------------------------------- tools
#
# Faithful's tools are drawn on exact 45 degree staircases: a handle is the band of pixels
# with ix + iy in a fixed range (constant along the up-right diagonal), so we work in integer
# pixel coordinates: SUM = ix + iy runs across the handle, DIF = ix - iy along it.

IY, IX = np.mgrid[0:S, 0:S]
SUM, DIF = IX + IY, IX - IY

HANDLE = dict(o="493615", lo="684e1e", hi="896727", body="493615", dark="281e0b")

# per tier: outer (lit side) outline, inner (shadow side) outline, shade, body, highlight,
# mid tone (Faithful's colors; diamond kept a clearer cyan, which reads better in the hand).
TIER = {
    "wooden": dict(o="372910", i="20180a", s="6b511f", b="755821", h="866526", m="594319"),
    "stone": dict(o="494949", i="181818", s="7f7f7f", b="898989", h="9a9a9a", m="6c6c6c"),
    "iron": dict(o="444444", i="181818", s="c1c1c1", b="d8d8d8", h="ffffff", m="969696"),
    "golden": dict(o="825d16", i="3f2e0e", s="e9b115", b="eaee57", h="fdff76", m="dc9613"),
    "diamond": dict(o="13586a", i="0a2c38", s="36b4cc", b="55d6ea", h="a8f4fc", m="2a8fa6"),
    "copper": dict(o="5f2c1c", i="421c11", s="d66d48", b="e3826c", h="fc9982", m="9c4e31"),
}
# sword: blade light, blade, fuller, guard, guard shade, outline, dark outline
SWORD = {
    "wooden": ("866526", "755821", "6b511f", "594319", "473614", "372910", "20180a"),
    "stone": ("b3b1af", "95918d", "878582", "787777", "5a5a5a", "494949", "212121"),
    "iron": ("ffffff", "d8d8d8", "bebebe", "969696", "6b6b6b", "444444", "181818"),
    "golden": ("ffffff", "fdff76", "eaee57", "e9b115", "dc9613", "825d16", "3f2e0e"),
    "diamond": ("c8f8fe", "5fdcee", "40c0d8", "2a8fa6", "1b6a80", "13586a", "0a2c38"),
    "copper": ("fdd4cb", "fc9982", "e3826c", "9c4e31", "803921", "5f2c1c", "421c11"),
}


def handle_mask(s0=126, s1=144, top=None, bottom=(16, 119)):
    """The diagonal stick: SUM in [s0, s1); cut square at its ends (top = (min y, max x) of
    its upper end, bottom = (min x, max y) of its lower end)."""
    m = (SUM >= s0) & (SUM < s1)
    if top is not None:
        m &= (IY >= top[0]) & (IX <= top[1])
    if bottom is not None:
        m &= (IX >= bottom[0]) & (IY <= bottom[1])
    return m


def paint_handle(img: Img, s0=126, s1=144, top=None, bottom=(16, 119), seed=0):
    """Faithful's wooden handle: outline, a lit stripe interrupted every so often (knots),
    a dark body and a dark outline on the lower right; plus a little 128 grain."""
    m = handle_mask(s0, s1, top, bottom)
    w = SUM - s0
    n = s1 - s0
    ph = (DIF + 7 + (seed % 5)) % 30
    col = np.zeros((S, S, 3), np.float32)
    col[:] = C(HANDLE["body"])
    col[(w >= 2) & (w < 10)] = C(HANDLE["lo"])
    stripe = (w >= 4) & (w < 8)
    col[stripe & (ph >= 7)] = C(HANDLE["hi"])
    col[stripe & (ph >= 5) & (ph < 7) & (w >= 5) & (w < 7)] = C(HANDLE["hi"])
    # grain: short lighter streaks in the dark body
    col[(w >= 11) & (w < 13) & (((DIF + 19) % 22) < 6)] = C(HANDLE["lo"])
    col[w < 2] = C(HANDLE["o"])
    col[w >= n - 2] = C(HANDLE["dark"])
    if bottom is not None:
        col[m & (IY >= bottom[1] - 1)] = C(HANDLE["dark"])
    img.put(m, col)
    return m


def outline2(m, lit_col, dark_col, img: Img, k=2):
    """A k px outline around `m`: `lit_col` on its top / left edges, `dark_col` elsewhere."""
    rg = grow(m, k) & ~m
    litside = np.zeros_like(m)
    for j in range(1, k + 1):
        litside |= shift(m, -j, 0) | shift(m, 0, -j)
    img.put(rg & ~litside, C(dark_col))
    img.put(rg & litside, C(lit_col))
    return rg


def tool_pickaxe(tier):
    t = TIER[tier]

    def fn(seed):
        img = Img()
        paint_handle(img, top=(30, 107), seed=seed)
        # the head: a crescent over the handle top, thickest in the middle, blunt ends
        pts = [(44, 26), (60, 26.5), (76, 28.5), (88, 31.5), (97, 36), (104, 43), (109, 53),
               (111.5, 66), (112.5, 80), (112.5, 90)]
        wid = [3, 4.5, 5.5, 7, 8, 8, 7.5, 7, 6, 3.5]
        m, side, tt = stroke(pts, wid)
        col = np.zeros((S, S, 3), np.float32)
        col[:] = C(t["b"])
        edge2 = m & ~shrink(m, 2)
        inner = m & (side < 0)
        col[inner & edge2] = C(t["s"])
        col[inner & ~shrink(m, 3) & (tt < 0.55)] = C(t["s"])
        col[m & (side > 0) & edge2 & ((tt < 0.32) | (tt > 0.72))] = C(t["h"])
        outline2(m, t["o"], t["i"], img)
        img.put(m, col)
        return img.a
    return fn


def tool_axe(tier):
    t = TIER[tier]

    def fn(seed):
        img = Img()
        paint_handle(img, top=(36, 101), seed=seed)
        # the eye: a collar over the handle top (grooved along the handle) and the poll, a
        # round knob behind it on the lower right
        poll = disk(57, 98, 8.5)
        pc = np.zeros((S, S, 3), np.float32)
        pc[:] = C(t["b"])
        pc[band(poll, 1, 0, 3) | band(poll, 0, 1, 3)] = C(t["s"])
        pc[lit(poll, 2)] = C(t["h"])
        outline2(poll, t["o"], t["i"], img)
        img.put(poll, pc)
        coll = (SUM >= 124) & (SUM < 146) & (DIF >= 24) & (DIF < 54)
        cc = np.zeros((S, S, 3), np.float32)
        cc[:] = C(t["b"])
        cc[(SUM >= 128) & (SUM < 132)] = C(t["h"])
        cc[(SUM >= 138) & (SUM < 140)] = C(t["m"])
        cc[SUM >= 142] = C(t["s"])
        cc[(DIF < 28)] = C(t["s"])
        outline2(coll, t["o"], t["i"], img)
        img.put(coll, cc)
        # blade: straight 45 degree cutting edge on the upper left
        blade = shrink(polygon([(78, 16), (90, 16), (90, 40), (79, 51), (58, 51), (56, 49),
                                (56, 35), (75, 16)]), 2)
        bc = np.zeros((S, S, 3), np.float32)
        bc[:] = C(t["s"])
        cut = band(blade, 0, -1, 7) & (YY > 18)
        bc[cut] = C(t["b"])
        bc[band(blade, 0, -1, 5) & (YY > 18)] = C(t["h"])
        bc[blade & (SUM >= 124) & (SUM < 126) & (YY > 36)] = C(t["m"])
        outline2(blade, t["o"], t["i"], img)
        img.put(blade, bc)
        return img.a
    return fn


def tool_shovel(tier):
    t = TIER[tier]

    def fn(seed):
        img = Img()
        paint_handle(img, top=(40, 90), seed=seed)
        # the flared grip end at the bottom left
        grip = polygon([(16, 100), (29, 100), (16, 113)]) & (SUM < 144)
        gc = np.zeros((S, S, 3), np.float32)
        gc[:] = C(HANDLE["hi"])
        gc[(SUM >= 126) & (SUM < 130)] = C(HANDLE["lo"])
        gc[band(grip, 0, -1, 2) | band(grip, -1, 0, 2)] = C(HANDLE["o"])
        img.put(grip, gc)
        # blade: a spade with its far corner squared off and rounded
        blade = polygon([(70, 43), (94, 16), (111, 16), (120, 25), (120, 41), (93, 67)])
        blade &= ~(((XX - 111) ** 2 + (YY - 25) ** 2 > 81) & (XX > 111) & (YY < 25))
        blade = shrink(blade, 2)  # the outline goes around it
        bc = np.zeros((S, S, 3), np.float32)
        bc[:] = C(t["s"])
        bc[rim(blade, 8)] = C(t["b"])
        bc[rim(blade, 6) & ~rim(blade, 2)] = C(t["h"])
        # no rim along the edge where the handle joins
        bc[blade & (SUM < 118) & (DIF < -20) & ~rim(blade, 1)] = C(t["s"])
        outline2(blade, t["o"], t["i"], img)
        img.put(blade, bc)
        return img.a
    return fn


def tool_sword(tier):
    hi, body, fuller, guard, guard_s, o, dk = SWORD[tier]

    def fn(seed):
        img = Img()
        # grip and pommel
        grip = (SUM >= 118) & (SUM < 136) & (IY >= 90) & (IX >= 12) & (IY <= 112)
        gw = SUM - 118
        gc = np.zeros((S, S, 3), np.float32)
        gc[:] = C(HANDLE["body"])
        gc[(gw >= 2) & (gw < 10)] = C(HANDLE["lo"])
        gc[(gw >= 4) & (gw < 8) & (((DIF + 3) % 16) >= 4)] = C(HANDLE["hi"])
        gc[gw >= 16] = C(HANDLE["dark"])
        img.put(grip, gc)
        pom = rrect(9.5, 117.5, 9.5, 9.5, 0, 7)[0]
        pc = np.zeros((S, S, 3), np.float32)
        pc[:] = C(guard_s)
        pc[band(pom, 1, 0, 4) | band(pom, 0, 1, 4)] = C(o)
        pc[lit(pom, 4) & ~lit(pom, 2) & (XX < 12) & (YY < 118)] = C(guard)
        pc[lit(pom, 2)] = C(o)
        pc[dark(pom, 2)] = C(dk)
        img.put(pom, pc)
        # blade: SUM in [114, 140), running to the top right corner (square tip)
        blade = (SUM >= 114) & (SUM < 140) & (DIF > -34)
        bw = SUM - 114
        bt = (DIF + 34) / 160.0  # 0 at the guard .. 1 at the tip
        bc = np.zeros((S, S, 3), np.float32)
        bc[:] = C(body)
        bc[(bw >= 2) & (bw < 12) & (bw < 2 + (bt - 0.25) * 16)] = C(hi)
        bc[(bw >= 14) & (bt > 0.8)] = C(hi)
        bc[(bw >= 12) & (bw < 14)] = C(fuller)
        bc[(bw >= 12) & (bw < 14) & (bt > 0.9)] = C(body)
        bc[(bw >= 22) & (bw < 24) & (bt < 0.85)] = C(fuller)
        bc[bw < 2] = C(o)
        bc[bw >= 24] = C(dk)
        bc[IY <= 1] = C(o)
        bc[(IX >= 126) & (bw >= 2)] = C(dk)
        img.put(blade, bc)
        # cross guard: a horn curving up to the upper left, a hook down to the lower right
        up = Q([(8, 28), (12, 28), (17, 32), (22, 36.5), (24, 40), (20, 44), (16, 46.5),
                (14.5, 42), (13, 37), (11, 33), (8, 30)])
        low = Q([(20, 43), (26, 40), (29, 44), (32, 48.5), (34.5, 53), (35.5, 56), (33, 56.5),
                 (27, 52.5), (22, 50.5), (17, 48), (16, 46.5)])
        gm = up | low
        gcol = np.zeros((S, S, 3), np.float32)
        gcol[:] = C(guard)
        gcol[up & ~shrink(up, 2)] = C(guard_s)
        gcol[low] = C(o)
        gcol[low & (DIF < -44) & (SUM < 140)] = C(guard_s)
        gcol[lit(gm, 2)] = C(o)
        gcol[dark(gm, 2)] = C(dk)
        gcol[low & ~up & dark(low, 2)] = C(dk)
        img.put(gm, gcol)
        return img.a
    return fn


# ---------------------------------------------------------------------------- materials


def stick(seed):
    img = Img()
    paint_handle(img, top=(16, 119), seed=seed)
    return img.a


def rock_lumps(m, lumps, seed):
    """Overlapping rounded lumps (x, y, r on the 64 grid) drawn back to front: per pixel the
    owning lump, its local height (-1 top .. 1 bottom) and the seams along lump edges."""
    order = sorted(range(len(lumps)), key=lambda i: lumps[i][1])
    owner = np.full((S, S), -1, np.int32)
    ly = np.zeros((S, S), np.float32)
    lx = np.zeros((S, S), np.float32)
    wy = (noise(seed + 3, 16, tile=False) - 0.5) * 7
    wx = (noise(seed + 4, 16, tile=False) - 0.5) * 7
    best = np.full((S, S), 1e9, np.float32)
    for i in order:
        x, y, r = lumps[i]
        dy, dx = (YY + wy - 2 * y) / (2 * r), (XX + wx - 2 * x) / (2 * r)
        d = np.hypot(dx, dy)
        inside = d <= 1
        owner = np.where(inside, i, owner)
        ly = np.where(inside, dy, ly)
        lx = np.where(inside, dx, lx)
        best = np.minimum(best, d)
        # fallback: nearest lump for pixels no lump covers
        miss = (owner < 0) & (d <= best)
        ly = np.where(miss, np.clip(dy, -1, 1), ly)
        lx = np.where(miss, np.clip(dx, -1, 1), lx)
    owner = np.where(owner < 0, -2, owner)
    seam = np.zeros((S, S), bool)
    for sy, sx in ((2, 0), (0, 2), (2, 2), (1, 1)):
        seam |= owner != np.roll(owner, (sy, sx), (0, 1))
    top_rim = np.zeros((S, S), bool)
    for k in (3, 4, 5):
        top_rim |= owner != np.roll(owner, (k, k // 2), (0, 1))
    return owner, ly, lx, seam & m, top_rim & m & ~seam


def coal(seed):
    """Faithful's coal: a heap of rounded lumps, lit and a little bluish on their tops,
    darkening to a purplish black at the bottom."""
    img = Img()
    m = P([(24, 9), (32, 9), (37, 11), (41, 15), (46, 19), (50, 23), (53, 28), (55, 33),
           (56, 38), (56, 45), (54, 50), (50, 53), (46, 55), (38, 57), (32, 59), (22, 59),
           (18, 57), (13, 52), (10, 47), (9, 40), (9, 30), (11, 25), (13, 20), (16, 15),
           (20, 11)])
    lumps = [(26, 17, 11), (43, 27, 11), (18, 32, 10), (34, 35, 10), (49, 43, 9),
             (22, 48, 11), (40, 50, 10)]
    owner, ly, lx, seam, crest = rock_lumps(m, lumps, seed)
    pal = np.stack([C(c) for c in ["1f1721", "261e24", "252525", "2e2e2e", "323232",
                                   "363636", "393e46"]])
    gy = np.clip((YY - 18) / 96, 0, 1)
    v = 5.2 - gy * 4.6 - ly * 0.9 - lx * 0.3
    v += (pix(seed + 5, 2) - 0.5) * 1.1
    idx = np.clip(np.round(v), 0, 5)
    idx[crest] = np.minimum(idx[crest] + 1, 5)
    idx[seam] = np.maximum(idx[seam] - 1, 0)
    col = pal[idx.astype(int)]
    # a few bluish glints on the lit tops of the upper lumps
    glint = (ly < -0.35) & (ly > -0.8) & (lx < 0.3) & (gy < 0.45) & ~seam
    glint &= pix(seed + 6, 2) > 0.8
    col[glint & m] = pal[6]
    col[lit(m, 2)] = C("1c1c1e")
    col[dark(m, 2)] = C("101015")
    img.put(m, col)
    return img.a


def charcoal(seed):
    """Faithful's charcoal: a chunky block of burnt wood seen from above the front: a
    lighter grey-brown top with bright flecks, two dark front faces parted by a crease."""
    img = Img()
    m = P([(26, 5), (37, 5), (43, 9), (50, 16), (55, 23), (58, 31), (58, 44), (55, 49),
           (50, 54), (44, 57), (36, 59), (22, 59), (16, 56), (11, 51), (8, 46), (7, 40),
           (7, 30), (9, 22), (13, 14), (19, 8)], 1)
    wob = (noise(seed + 9, 8, tile=False) - 0.5) * 5
    ridge = 2 * np.interp(XX / 2, [0, 8, 24, 40, 50, 64], [17, 18, 26, 34, 31, 27]) + wob
    topf = YY < ridge
    crease = 2 * np.interp(YY / 2, [0, 34, 46, 64], [40, 40, 37.5, 34]) + wob * 0.6
    leftf = ~topf & (XX < crease)
    pal = np.stack([C(c) for c in ["13110d", "1d1a14", "231f18", "2b261d", "312b22",
                                   "423b2f", "4e4536", "605543", "7d6f58"]])
    n1 = noise(seed + 7, 10, tile=False)
    n2 = pix(seed + 5, 2)
    v = np.where(topf, 6.3 - (YY / np.maximum(ridge, 1)) * 1.8,
                 np.where(leftf, 2.4, 2.9) - (YY - ridge) / 80)
    v += (n1 - 0.5) * 2.4 + (n2 - 0.5) * 1.2
    # a few pale patches on the front faces
    v += ~topf * (noise(seed + 12, 6, tile=False) > 0.72) * 1.6
    idx = np.clip(np.round(v), 1, 7)
    # edges: a lit rim above the ridge, a dark line below it, a dark crease
    idx[topf & ~shift(topf, -2, 0) & m] = 6
    idx[~topf & shift(topf, 2, 0) & m] = 1
    idx[~topf & (np.abs(XX - crease) < 1.5)] = 1
    idx[~topf & ~leftf & (XX - crease >= 1.5) & (XX - crease < 3.5)] = 4
    col = pal[idx.astype(int)]
    fleck = topf & (noise(seed + 6, 5, tile=False) > 0.68) & (pix(seed + 8, 2) > 0.55)
    fleck &= YY < ridge - 5
    col[fleck] = pal[7]
    col[fleck & (pix(seed + 10, 2) > 0.8)] = pal[8]
    col[rim(m, 2)] = C("13110d")
    img.put(m, col)
    return img.a


def clay_ball(seed):
    """Faithful's clay ball: a soft blue-grey dome, speckled light on the upper left and
    shaded blue on the right and underneath."""
    img = Img()
    m = P([(29, 13), (35, 13), (41, 15), (46, 18), (50, 22), (53, 27), (55, 33), (55, 42),
           (53, 47), (49, 51), (43, 54), (36, 55), (27, 55), (20, 53), (14, 50), (10, 46),
           (9, 39), (9, 32), (11, 26), (15, 21), (20, 16)], 2)
    t = noise(seed, 12, tile=False) * 0.5 + pix(seed + 1, 2) * 0.5
    pal_l = np.stack([C(c) for c in ["9499a4", "a1a7b1", "acaebd", "afb9d6"]])
    col = pal_l[np.clip((t * 4.2 - 0.4).astype(int), 0, 3)]
    # shadow side: right and bottom, with a stripy transition
    sh = (XX - 60) * 0.9 + (YY - 60) * 0.55 + (noise(seed + 2, 8, tile=False) - 0.5) * 18
    sh2 = (YY - 98) + np.abs(XX - 64) * 0.2 + (noise(seed + 3, 8, tile=False) - 0.5) * 10
    jit = (pix(seed + 6, 2) - 0.5) * 16
    shade = (sh + jit > 14) | (sh2 + jit * 0.5 > 0)
    dpal = np.stack([C(c) for c in ["5e6c8d", "757d90"]])
    dcol = dpal[(pix(seed + 4, 2) > 0.3).astype(int)]
    col[shade] = dcol[shade]
    col[shade & ((sh > 34) | (sh2 > 6)) & (pix(seed + 5, 2) > 0.45)] = C("5e6c8d")
    edge = (sh + jit > 6) & ~shade
    col[edge] = C("9499a4")
    col[lit(m, 2)] = C("40445a")
    col[dark(m, 2)] = C("373944")
    img.put(m, col)
    return img.a


# A bar (ingot, brick) seen from above-front-left: a long top face, a short end face on the
# left and a long front face, as in Faithful (64 grid: L (0,24), T (47,8), R (64,24),
# F (16,40), 15.5 thick), drawn at 128.
BAR_L, BAR_T, BAR_R, BAR_F, BAR_H = (0, 48), (94, 16), (128, 48), (34, 82), 31


def bar_faces():
    (lx, ly), (tx, ty), (rx, ry), (fx, fy), h = BAR_L, BAR_T, BAR_R, BAR_F, BAR_H
    top = polygon([(lx, ly), (tx, ty), (rx, ry), (fx, fy)])
    end = polygon([(lx, ly), (fx, fy), (fx, fy + h), (lx, ly + h)]) & ~top
    front = polygon([(fx, fy), (rx, ry), (rx, ry + h), (fx, fy + h)]) & ~top & ~end
    top &= XX < 128
    return top, end, front


def bar(pal, extra=None, seed=0):
    """`pal`: top, hi (edge highlight), end, end_edge, front, front_rim, front_end, o_top,
    o_bot, spark."""
    img = Img()
    T, E, F = bar_faces()
    whole = T | E | F
    fx = BAR_F[0]
    col = np.zeros((S, S, 3), np.float32)
    col[:] = C(pal["top"])
    # top face: an inset highlight line along the two near edges
    near = band(T, 1, 0, 4) & ~band(T, 1, 0, 2) & (XX >= fx - 1)
    near |= band(T, 1, -1, 2) & (XX < fx + 1)
    col[near] = C(pal["hi"])
    col[band(T, 0, 1, 3) & (YY < BAR_L[1] + 2) & (XX > BAR_T[0])] = C(pal["end"])
    # end face
    col[E] = C(pal["end"])
    col[E & (XX >= fx - 2)] = C(pal["end_edge"])
    col[E & band(E, 1, 0, 2) & (XX < fx - 2)] = C(pal["front_rim"])
    # front face
    col[F] = C(pal["front"])
    col[F & (XX < fx + 2)] = C(pal["front_rim"])
    col[band(F, 1, 0, 2) | (band(F, 0, 1, 4) & ~band(F, 0, 1, 2))] = C(pal["front_rim"])
    col[F & band(F, 0, 1, 4) & ~band(F, 0, 1, 2) & (YY < BAR_R[1] + BAR_H - 2)] = \
        C(pal["front_end"])
    if extra is not None:
        extra(col, T, E, F)
    # outline: lighter on the upper left, dark on the lower right
    col[lit(whole, 2) | (band(whole, 1, 0, 2) & (XX < fx))] = C(pal["o_top"])
    col[dark(whole, 2) & ~(band(whole, 1, 0, 2) & (XX < fx))] = C(pal["o_bot"])
    col[rect(BAR_F[1] - 2, fx - 2, BAR_F[1] + 1, fx + 1)] = C(pal["spark"])
    img.put(whole, col)
    return img.a


IRON_BAR = dict(top="d8d8d8", hi="ffffff", end="a8a8a8", end_edge="d8d8d8", front="727272",
                front_rim="828282", front_end="a8a8a8", o_top="5e5e5e", o_bot="353535",
                spark="ffffff")
GOLD_BAR = dict(top="fdf55f", hi="fffde0", end="fad64a", end_edge="fdf55f", front="dc9613",
                front_rim="e9b115", front_end="fad64a", o_top="b26411", o_bot="752802",
                spark="ffffff")
COPPER_BAR = dict(top="e77c56", hi="fc9982", end="9c4e31", end_edge="c15a36", front="c15a36",
                  front_rim="c15a36", front_end="c15a36", o_top="9c4529", o_bot="6d3421",
                  spark="fbc3b6")
BRICK_BAR = dict(top="b75a40", hi="c76245", end="8e4631", end_edge="b75a40", front="7f3e2c",
                 front_rim="7f3e2c", front_end="612f22", o_top="492319", o_bot="2d1610",
                 spark="c76245")


def gold_extra(col, T, E, F):
    """Faithful's gold bar: its end face is striped light to dark."""
    x = (XX - BAR_L[0]) / (BAR_F[0] - BAR_L[0])
    stripes = ((XX // 2).astype(int) * 5) % 7 / 7.0
    col[E & (stripes > x * 1.2 - 0.1) & ~band(E, 1, 0, 2)] = C("fdf55f")


def copper_extra(col, T, E, F):
    """Faithful's copper ingot: a shaded diagonal groove across the top, the front split
    into a dark left end, a lit middle and a darker right end by 45 degree edges."""
    u = XX + YY  # 45 degree lines, down-left
    col[T & (u >= 110) & (u < 126)] = C("9c4e31")
    col[T & ((u >= 108) & (u < 110) | (u >= 126) & (u < 128))] = C("c15a36")
    col[T & (u >= 150)] = C("c15a36")
    col[F & (u < 144) & (XX >= BAR_F[0] + 2)] = C("9c4e31")
    col[F & (u >= 170)] = C("8a4129")
    col[F & (u >= 170) & (u < 172)] = C("9c4e31")
    col[E] = C("9c4e31")
    col[E & (XX < 4)] = C("c15a36")
    col[E & (XX < 2)] = C("e77c56")


def ingot(pal, extra=None):
    def fn(seed):
        return bar(pal, extra, seed)
    return fn


def brick(seed):
    def tex(col, T, E, F):
        g = pix(seed, 2)
        col[T & (g > 0.975)] = C("c76245")
        col[F & (g > 0.975)] = C("612f22")
        col[F & (g < 0.02)] = C("8e4631")
    return bar(BRICK_BAR, tex, seed)


def diamond(seed):
    """Faithful's diamond: a cut gem with a flat table on top, light facet lines above the
    girdle and dark ones below, a light diagonal reflection across the middle."""
    img = Img()
    m = P([(25, 8), (39, 8), (42, 9.5), (46, 13), (50, 17.5), (53, 23), (54, 28), (54, 41),
           (53, 46), (50, 51), (45, 56), (40, 58), (24, 58), (19, 56), (14, 51), (11, 46),
           (10, 41), (10, 29), (11, 23), (14, 17.5), (18, 13), (22, 9.5)], 1)
    A, B, Cc, E, I, J = C("4aedd9"), C("20c5b5"), C("a1fbe8"), C("1aaaa7"), C("2ce0d8"), \
        C("1c919a")
    col = np.zeros((S, S, 3), np.float32)
    col[:] = A
    # lower half and the right facet: the darker body color
    col[YY >= 81] = B
    col[(XX >= 84) & (YY >= 50)] = B
    # the table's lit lower-right triangle and the reflection across the middle facet
    table = Q([(21, 9), (41, 9), (40, 25), (26, 25)])
    col[table & (YY > 2 * 25 - (XX - 52) * 0.55)] = Cc
    mid = Q([(22, 25), (41, 25), (41, 40), (22, 40)])
    u = XX + YY
    col[mid & (np.abs(u - 133) < 7)] = Cc
    col[mid & (u < 106)] = Cc
    # lower-left facet: lit
    low_l = Q([(10, 40), (23, 40), (18, 58), (10, 58)])
    col[low_l] = A
    col[low_l & ~shift(low_l, 0, -3)] = I
    col[low_l & (YY >= 98) & ~shift(low_l, 3, 0)] = I
    # facet lines: light above the girdle, dark below
    light = (seg((21, 14), (26, 25)) | seg((41, 13), (40, 25)) | seg((12, 25), (41, 25))
             | seg((22, 26), (21, 38)) | seg((21, 38), (16, 39.5)) | seg((16, 39.5), (11, 40.5))
             | seg((13, 26), (22, 27)))
    col[light] = Cc
    col[seg((12, 25), (41, 25)) & (XX < 80) & (XX > 44)] = C("ffffff")
    col[seg((21, 14), (26, 25)) & (YY > 40)] = C("d5fff6")
    darkl = (seg((23, 40), (40, 40)) | seg((23, 40.5), (18, 55)) | seg((40, 40.5), (43, 54))
             | seg((42, 40), (52, 38)))
    col[darkl] = E
    col[seg((42, 27), (42, 40))] = J
    # the bottom tip catches a little light
    col[(YY >= 110) & (XX > 48) & (XX < 84)] = A
    col[(YY >= 110) & (XX > 62) & (XX < 76)] = Cc
    # rims and outline
    col[lit(m, 4) & ~lit(m, 2) & (YY < 86)] = C("ffffff")
    col[lit(m, 4) & ~lit(m, 2) & (YY < 22) & (XX > 70)] = C("d5fff6")
    col[dark(m, 4) & ~dark(m, 2) & (YY < 96) & (XX > 90)] = E
    col[lit(m, 2) & (YY < 84)] = C("11727a")
    col[dark(m, 2) | (lit(m, 2) & (YY >= 84))] = C("145e53")
    img.put(m, col)
    return img.a


def iron_nugget(seed):
    """Faithful's iron nugget: a small teardrop lump pointing right, a bright patch on its
    rounded left end, bluish grey shading toward the tip and underneath."""
    img = Img()
    m = P([(25, 23), (30, 23), (34, 24.5), (38, 26), (42, 27.5), (45, 29.5), (47, 31.5),
           (44, 35), (39, 38.5), (33, 40.5), (27, 41.5), (21, 41), (18, 39), (16, 36),
           (16, 31), (17.5, 28), (20, 25.5)], 2)
    col = np.zeros((S, S, 3), np.float32)
    col[:] = C("d9dfe7")
    sh = (XX - 40) * 0.55 + (YY - 62) * 1.0
    col[sh > 4] = C("bdcadb")
    col[(sh > 18)] = C("a2b0be")
    col[(sh > 24)] = C("738d8d")
    hl = ellipse(60, 50, 6, 13) & (XX - 44 + (YY - 56) * 1.3 < 22)
    col[hl] = C("f2f2f2")
    col[rim(m, 4) & ~rim(m, 2) & (YY < 58) & (XX < 80)] = C("bdcadb")
    col[band(m, 0, 1, 4) & (YY < 70)] = C("a2b0be")
    col[lit(m, 2)] = C("585f68")
    col[dark(m, 2) | (band(m, 0, 1, 2) & (YY > 58))] = C("393c40")
    img.put(m, col)
    return img.a


# ---------------------------------------------------------------------------- containers

# Faithful's bucket: a tapered pail seen a little from above: a wide elliptic opening
# (center 64, 33), straight sides narrowing to a rounded bottom.
B_CY, B_RX, B_RY = 33, 44, 21


def bucket_shape():
    top = ellipse(B_CY, 64, B_RY, B_RX)
    t = np.clip((YY - 62) / 53, 0, 1)
    hw = 43 - 12 * t - 16 * t**6
    body = (np.abs(XX - 64) <= hw) & (YY >= B_CY) & (YY <= 114)
    whole = top | body
    opening = ellipse(B_CY + 0.5, 64, B_RY - 2.5, B_RX - 2.5)
    return whole, body, opening, hw


def bucket_body(img: Img):
    """Paints the pail; returns the opening mask (inside the rim)."""
    whole, body, opening, hw = bucket_shape()
    f = (XX - 64) / np.maximum(hw, 1)
    col = np.zeros((S, S, 3), np.float32)
    col[:] = C("a8a8a8")
    col[f < -0.3] = C("d8d8d8")
    col[(f >= 0.28) & (f < 0.48)] = C("969696")
    col[f >= 0.48] = C("727272")
    col[f >= 0.88] = C("a8a8a8")
    col[(f >= 0.88) & (YY > 78)] = C("969696")
    # the white glint down the lit side
    col[(f > -0.7) & (f < -0.55) & (YY > 60) & (YY < 80)] = C("ffffff")
    col[(f > -0.66) & (f < -0.58) & (YY >= 80) & (YY < 84)] = C("ffffff")
    # a faintly lit left rim, dithered like Faithful's
    col[(f < -0.93) & (((YY // 2) % 4) == 0)] = C("727272")
    col[(f < -0.93) & (((YY // 2) % 4) == 2)] = C("a8a8a8")
    img.put(whole, col)
    img.put(grow(whole, 2) & ~whole, C("353535"))
    return opening


def bucket_inside(img: Img, opening):
    """The empty inside: shaded back wall, lit back lip, the dark front lip."""
    col = np.zeros((S, S, 3), np.float32)
    col[:] = C("727272")
    col[(XX > 36) & (XX < 66)] = C("5f5f5f")
    col[XX <= 50] = C("545454")
    col[(XX <= 36)] = C("545454")
    col[(XX > 94) & (XX < 100)] = C("5f5f5f")
    # the back lip: the rim's inner face at the top of the opening
    lip = opening & ~shift(opening, 5, 0)
    col[lip] = C("969696")
    col[lip & (XX > 72)] = C("a8a8a8")
    col[lip & band(opening, -1, 0, 2)] = C("969696")
    col[band(opening, -1, 0, 4) & ~band(opening, -1, 0, 2) & (XX < 40)] = C("727272")
    img.put(opening, col)
    front_lip(img, opening, C("353535"))


def front_lip(img: Img, opening, col):
    """The front rim's inner edge: a 2-4 px line along the bottom of the opening."""
    lo = opening & ~shift(opening, -3, 0)
    img.put(lo & (YY > B_CY), col)


def bucket(content=None):
    def fn(seed):
        img = Img()
        opening = bucket_body(img)
        if content is None:
            bucket_inside(img, opening)
        elif content == "water":
            water(img, opening, seed)
        else:
            lava(img, opening, seed)
        return img.a
    return fn


def water(img: Img, opening, seed):
    """Water to the brim: deep blue ripples, lighter crests in the middle, a darker edge."""
    n = noise(seed, 10, tile=False) * 0.6 + noise(seed + 1, 5, tile=False) * 0.4
    wave = np.sin((XX * 0.35 + YY * 0.9) * 0.55 + n * 7)
    pal = np.stack([C(c) for c in ["234fcc", "2e58d3", "345fda", "446fe9", "5a82f3"]])
    v = 1.6 + wave * 0.9 + (n - 0.5) * 2.2
    v -= np.clip(np.abs(XX - 64) / 44, 0, 1) ** 3 * 1.2
    idx = np.clip(np.round(v), 0, 4).astype(int)
    col = pal[idx]
    col[rim(opening, 3)] = C("2e58d3")
    col[rim(opening, 2) & (YY > B_CY)] = C("234fcc")
    lip = opening & ~shift(opening, 5, 0)
    col[lip & (YY < B_CY - 10)] = C("787878")
    col[lip & (YY < B_CY - 10) & (XX > 56) & (XX < 90)] = C("989898")
    img.put(opening, col)
    front_lip(img, opening, C("353535"))


def lava(img: Img, opening, seed):
    """Lava to the brim: yellow crusts parted by orange and red cracks, one drip running
    over the front."""
    g = rng(seed)
    pts = [(x + g.uniform(-5, 5), y + g.uniform(-2.5, 2.5))
           for y in (16, 24, 32, 40, 48) for x in range(16 + (y // 8) % 2 * 7, 118, 14)]
    wx = (noise(seed + 4, 8, tile=False) - 0.5) * 8
    wy = (noise(seed + 5, 8, tile=False) - 0.5) * 5
    d = np.stack([np.hypot(XX + wx - x, (YY + wy - y) * 1.5) for x, y in pts])
    ds = np.sort(d, 0)
    gap = ds[1] - ds[0] + (pix(seed + 3, 2) - 0.5) * 2.5
    col = np.zeros((S, S, 3), np.float32)
    col[:] = C("e4d25c")
    col[gap < 5] = C("e38c3f")
    col[gap < 2] = C("cc4628")
    col[(ds[0] < 3) & (gap > 5)] = C("e4d25c")
    col[rim(opening, 3) & (gap < 9)] = C("cc4628")
    lip = opening & ~shift(opening, 5, 0)
    col[lip & (YY < B_CY - 10)] = C("b68c7b")
    col[lip & (YY < B_CY - 10) & (XX < 52)] = C("9f7f78")
    img.put(opening, col)
    front_lip(img, opening, C("7f3e2c"))
    # the drip over the front rim
    drip = stroke([(78, 50), (78.5, 58), (78, 66), (78, 71)], [7, 6, 5, 4])[0] & ~opening
    dc = np.zeros((S, S, 3), np.float32)
    dc[:] = C("e38c3f")
    dc[XX < 77] = C("e4d25c")
    dc[XX > 80] = C("cc4628")
    img.put(drip, dc)
    img.put(drop(drip, 1) & ~opening & (YY > 54) & ~drip, C("969696"))


FLASK = [(26.5, 27), (22.5, 30.5), (19, 34.5), (16.5, 38.5), (15, 43), (15, 50.5), (16.5, 54),
         (19, 57), (22.5, 59), (27, 60.3), (37, 60.3), (41.5, 59), (45, 57), (47.5, 54),
         (49, 50.5), (49, 43), (47.5, 38.5), (45, 34.5), (41.5, 30.5), (37.5, 27)]


def bottle_parts():
    """(flask outline, glass interior, neck walls, lip) on the 64 grid doubled: a conical
    flask with a round bottom, a narrow neck and a lip under the cork."""
    flask = P(FLASK, 1)
    neck = Q([(27, 17), (37, 17), (37, 28), (27, 28)])
    outer = flask | neck
    inner = shrink(flask, 4) | (Q([(29, 17), (35, 17), (35, 30), (29, 30)]) & flask) | \
        Q([(29, 17), (35, 17), (35, 29), (29, 29)])
    inner &= outer
    return outer, inner


def bottle_glints():
    """The two curved reflections inside the glass (also cut out of the potion liquid)."""
    g1, s1, _ = stroke([(54, 64), (49, 71), (45, 78), (43, 86)], [4, 4, 4, 3.5])
    g2, s2, _ = stroke([(85, 92), (82, 102), (77, 112)], [4, 4, 3.5])
    return g1, s1, g2, s2


def bottle(seed):
    """Faithful's glass bottle: a thin blue glass outline (light on the left, dark on the
    right), two curved glints, a small cork over a pinkish neck ring."""
    img = Img()
    outer, inner = bottle_parts()
    wall = outer & ~inner
    col = np.zeros((S, S, 3), np.float32)
    col[:] = C("8badd0")
    outer_band = wall & ~shrink(outer, 2)
    col[wall & (XX < 64)] = C("b3cfec")
    col[outer_band & (XX < 64)] = C("d4e5f7")
    col[outer_band & (XX >= 64)] = C("5d8fc2")
    col[outer_band & (YY > 108)] = C("5d8fc2")
    col[wall & ~outer_band & (YY > 104) & (XX < 44)] = C("8badd0")
    col[outer_band & (YY > 100) & (XX < 40)] = C("8badd0")
    col[wall & ~outer_band & (XX >= 64)] = C("8badd0")
    img.put(wall, col)
    g1, s1, g2, s2 = bottle_glints()
    img.put(g1, C("8badd0"))
    img.put(g1 & (s1 > 0), C("b3cfec"))
    img.put(g2, C("5d8fc2"))
    img.put(g2 & (s2 > 0), C("8badd0"))
    # lip: a ring under the cork, the cork's foot showing through it
    lip = Q([(25, 14), (39, 14), (39, 18), (25, 18)])
    lc = np.zeros((S, S, 3), np.float32)
    lc[:] = C("973716")
    lc[(XX < 64) & (pix(seed, 2) > 0.5)] = C("a94725")
    lc[lip & (YY < 30)] = C("ccafa5")
    lc[lip & (XX < 54)] = C("b3cfec")
    lc[lip & (XX < 52)] = C("d4e5f7")
    lc[lip & (XX >= 74)] = C("8badd0")
    lc[lip & (XX >= 76)] = C("5d8fc2")
    lc[lip & (YY >= 34) & (XX >= 70)] = C("8badd0")
    img.put(lip, lc)
    # cork
    cork = Q([(28, 10), (36, 10), (37, 11), (37, 14), (27, 14), (27, 11)])
    cc = np.zeros((S, S, 3), np.float32)
    cc[:] = C("a94725")
    cc[band(cork, 1, 0, 2) | band(cork, 0, 1, 3)] = C("973716")
    cc[cork & (XX < 64) & (YY < 26) & ~band(cork, 0, -1, 2)] = C("d46d49")
    cc[band(cork, 0, -1, 2)] = C("a94725")
    img.put(cork, cc)
    return img.a


def potion_overlay(seed):
    """The liquid filling the flask up to its shoulders, light grey for the game to tint:
    a white streak along the left, a darker drop-shaped reflection in the middle, shaded
    to the right and bottom; the glass glints stay clear."""
    img = Img()
    outer, inner = bottle_parts()
    liquid = inner & (YY >= 64)
    liquid |= shrink(P(FLASK, 1), 2) & (YY >= 64) & (XX >= 64)
    col = np.zeros((S, S, 3), np.float32)
    col[:] = C("c5c5c5")
    # shading to the right / bottom
    sh = (XX - 64) * 0.8 + (YY - 88) * 0.9
    col[sh > 14] = C("a6a6a6")
    col[sh > 30] = C("9a9a9a")
    col[band(liquid, 0, -1, 4)] = C("9a9a9a")
    col[band(liquid, 0, -1, 2) & (YY < 100)] = C("a6a6a6")
    # the white streak inside the left edge
    streak = stroke([(56, 64), (49, 73), (42, 84), (38, 96), (40, 104)], [5, 5, 5, 4, 3])[0]
    col[streak] = C("ffffff")
    # the drop-shaped reflection
    drop_ = ellipse(92, 66, 12, 9) | polygon([(58, 90), (74, 90), (66, 74)])
    col[drop_] = C("a6a6a6")
    col[shrink(drop_, 2)] = C("9a9a9a")
    col[band(liquid, 1, 0, 2) | band(liquid, 0, 1, 2)] = C("636363")
    col[band(liquid, 1, 0, 4) & ~band(liquid, 1, 0, 2)] = C("9a9a9a")
    g1, _, g2, _ = bottle_glints()
    liquid &= ~(g1 | g2)
    img.put(liquid, col)
    return img.a


# ---------------------------------------------------------------------------- food


# Faithful-style meat: every outline drawn on its 64 grid (x, y) and doubled.
PORK_OUT = [(40, 9), (50, 9), (55, 11), (58, 15), (60, 19), (61, 26), (60, 33), (57, 38),
            (53, 43), (45, 51), (38, 55), (32, 59), (20, 59), (14, 55), (9, 48), (8, 38),
            (11, 32), (16, 27.5), (21, 25), (23, 23), (27, 19), (35, 11)]
COOKED_PORK_OUT = [(40, 13), (50, 13), (54, 15), (57, 18), (59, 21), (60, 26), (60, 31),
                   (58, 36), (55, 40), (50, 46), (44, 51), (38, 55), (32, 59), (20, 59),
                   (14, 56), (10, 50), (10, 44), (12, 40), (20, 31), (30, 21), (35, 16)]


def chop_faces(pts):
    """The chop's outline, its top face and the thick side showing below / right of it."""
    whole = P(pts)
    top = whole & shift(whole, -10, -4)
    return whole, top, whole & ~top


def porkchop(seed):
    """Raw porkchop: pink meat, a pale fat rim thickening along the bottom, a few pale
    sinew streaks on the upper left, a thick darker side below."""
    img = Img()
    whole, top, side = chop_faces(PORK_OUT)
    # fat: a band inside the bottom-right edge of the top face, widening toward the bottom
    dbr = ndimage.distance_transform_edt(~dark(top, 1))
    s = np.clip(1.15 - np.hypot((XX - 70) / 46, (YY - 104) / 40), 0, 1) ** 0.8
    centre = 6 + 10 * s
    flat_ = 0.5 + 9 * s
    half = 2.8 + 4 * s
    f = (1 - np.maximum(np.abs(dbr - centre) - flat_, 0) / half) * (0.78 + 0.3 * s)
    for pts, w in [([(26, 20), (31, 22), (35, 25)], 3.2),
                   ([(16, 28), (19, 31), (21, 35), (24, 39), (28, 43)], 3.6),
                   ([(33, 36), (37, 38), (41, 41)], 2.4),
                   ([(21, 28), (24, 29)], 1.8)]:
        f = np.maximum(f, 1 - line_dist(pts) / w)
    f += (noise(seed + 5, 8, tile=False) - 0.5) * 0.25
    col = np.zeros((S, S, 3), np.float32)
    col[:] = C("ef7070")
    broad = (dbr < centre + half + 12) & (XX > 36)
    col[broad | (f > -0.35)] = C("ff7777")
    col[f > 0.12] = C("ff8c8c")
    col[f > 0.42] = C("ffadad")
    col[f > 0.72] = C("ffc6c6")
    col[dark(top, 2)] = C("ef7070")
    col[side] = C("a75353")
    col[side & ((XX - 60) + (YY - 100) * -0.3 > 0)] = C("853e3e")
    col[band(side, 1, 0, 2) | band(side, 0, 1, 2)] = C("853e3e")
    col[lit(whole, 2)] = C("a75353")
    col[lit(whole, 2) & (YY > 76)] = C("853e3e")
    col[dark(whole, 2) & (YY < 50)] = C("853e3e")
    col[dark(whole, 2) & (YY >= 50)] = C("512626")
    img.put(whole, col)
    return img.a


def cooked_porkchop(seed):
    """Cooked porkchop: pale golden-beige, grill-like light stripes across it, a darker rim."""
    img = Img()
    whole, top, side = chop_faces(COOKED_PORK_OUT)
    col = np.zeros((S, S, 3), np.float32)
    col[:] = C("d3c088")
    col[(pix(seed, 2) > 0.55) ^ (noise(seed + 1, 6, tile=False) > 0.6)] = C("cfba81")
    # stripes running down-right, across the chop
    u = XX - YY + (noise(seed + 2, 24, tile=False) - 0.5) * 10
    inner = shrink(top, 5)
    for u0 in (56, 35, 13, -9, -31, -53):
        du = np.abs(u - u0)
        col[(du < 7) & (du >= 3.8) & inner & (pix(seed + 3, 2) > 0.5)] = C("c5ad77")
        col[(du < 3.8) & inner] = C("dacba4")
        col[(du < 1.6) & inner] = C("e2d3ac")
    for u0 in (45, 24, 2, -20, -42):
        col[(np.abs(u - u0) < 1.5) & inner & (noise(seed + 4, 10, tile=False) > 0.55)] = \
            C("c5ad77")
    # rims: a darker toasted band inside the top-left edge, darker at the bottom right
    col[lit(top, 6) & ~lit(top, 2)] = C("bca474")
    col[lit(top, 8) & ~lit(top, 6) & (pix(seed + 6, 2) > 0.5)] = C("c5ad77")
    col[dark(top, 4)] = C("bca474")
    col[dark(top, 2)] = C("997942")
    col[side] = C("997942")
    col[side & ((XX - 56) + (YY - 100) * -0.4 > 0)] = C("8c6932")
    col[band(side, 1, 0, 3) & (XX > 60)] = C("81602b")
    col[lit(whole, 2)] = C("8c6932")
    col[dark(whole, 2)] = C("5f4f27")
    img.put(whole, col)
    return img.a


MUTTON_OUT = [(46, 4), (49, 4), (51, 6), (53, 9), (54, 14), (54, 22), (55, 28), (55, 33),
              (53, 38), (51, 44), (49, 50), (46, 55), (42, 59), (38, 61), (32, 63), (20, 64),
              (14, 62), (10, 59), (8, 55), (8, 49), (10, 44), (14, 40), (20, 36), (26, 33),
              (31, 30), (35, 26), (38, 20), (42, 12), (44, 7)]


def mutton_like(c):
    """A leg-of-lamb cut: a pale fat line looping inside the edge, a seam from the neck to
    the right side, a thick shaded side below and to the right."""
    def fn(seed):
        img = Img()
        whole = P(MUTTON_OUT)
        top = whole & shift(whole, -12, -4)
        side = whole & ~top
        din = ndimage.distance_transform_edt(top)
        w = 0.85 + 0.45 * noise(seed + 3, 12, tile=False)
        fd = np.abs(din - 5.5) / w
        fd = np.minimum(fd, line_dist([(31, 30), (35, 33), (38, 36), (41, 40), (44, 45),
                                       (46, 50)]) / w)
        col = np.zeros((S, S, 3), np.float32)
        col[:] = C(c["main"])
        col[fd < 8] = C(c["light"])
        col[(fd < 8) & (fd >= 6) & (pix(seed, 2) > 0.5)] = C(c["main"])
        col[fd < 4.5] = C(c["fat"])
        col[fd < 3] = C(c["fat2"])
        col[fd < 1.4] = C(c["hi"])
        col[side] = C(c["side"])
        col[band(side, -1, 0, 2) & ~shift(side, -2, 0) | (side & shift(top, 2, 0))] = \
            C(c["side_hi"])
        col[lit(whole, 2)] = C(c["rim"])
        col[dark(whole, 2)] = C(c["o"])
        img.put(whole, col)
        return img.a
    return fn


MUTTON = dict(main="d12e26", light="dd3830", fat="e2625a", fat2="e27269", hi="e88a82",
              side="96211b", side_hi="bc2922", rim="ad332e", o="470a06")
COOKED_MUTTON = dict(main="82483a", light="884f40", fat="9d6147", fat2="9c6854", hi="a3705a",
                     side="522f1f", side_hi="7c402f", rim="6b3a2a", o="2a160d")


def egg_mask():
    """Faithful's spawn egg outline: 14..119 tall, widest a little below the middle."""
    vy = (YY - 66.5) / 52.5
    half = 44 * np.sqrt(np.clip(1 - vy**2, 0, 1)) * (1 + 0.1 * np.clip(vy, -1, 1))
    return (np.abs(XX - 64) <= half) & (np.abs(vy) <= 1)


def egg_base(m, body, hi, hi2, mid, low, rim_col, o_lit, o_dark, seed):
    """Shared egg shading: a big soft highlight on the upper left, a mid band on the upper
    right, the darker lower half and right side, an inner rim and a 2 px outline."""
    col = np.zeros((S, S, 3), np.float32)
    col[:] = C(body)
    slant = (XX - 70) - (YY - 40) * 0.35
    col[(YY < 26) | ((slant > 0) & (slant < 20) & (YY < 72))] = C(mid)
    col[(YY > 88) | ((XX > 88) & (YY > 56))] = C(low)
    col[(YY > 84) & (YY <= 88) & (XX > 84) & (pix(seed, 2) > 0.5)] = C(low)
    col[(YY > 88) & (XX < 54) & (YY < 96) & ((XX - 26) + (YY - 88) < 30)] = C(body)
    hl = ellipse(54, 44, 22, 15)
    col[hl] = C(hi)
    col[ellipse(46, 41, 9, 7)] = C(hi2) if hi2 else C(hi)
    col[band(m, 0, 1, 4) & ~band(m, 0, 1, 2) & (YY > 36)] = C(rim_col)
    col[band(m, 1, 0, 4) & ~band(m, 1, 0, 2) & (YY > 100)] = C(rim_col)
    col[lit(m, 2)] = C(o_lit)
    col[dark(m, 2)] = C(o_dark)
    col[band(m, 1, 0, 2) & (YY > 96)] = C(o_dark)
    return col


def pig_spawn_egg(seed):
    """Pink egg, a big pale highlight, the pig's snout with two dark nostrils."""
    img = Img()
    m = egg_mask()
    col = egg_base(m, "f19e98", "ffb9b5", None, "ec8985", "e6716f", "c6615a", "a64b4b",
                   "914040", seed)
    sn = rect(62, 46, 90, 82)
    col[sn] = C("ec8985")
    col[sn & (XX < 58)] = C("f19e98")
    col[rect(62, 46, 64, 82)] = C("f19e98")
    col[rect(62, 62, 64, 82)] = C("e6716f")
    col[rect(64, 46, 66, 82)] = C("f19e98")
    col[rect(64, 46, 90, 48)] = C("e6716f")
    col[rect(64, 80, 88, 82)] = C("c6615a")
    col[rect(88, 46, 90, 82)] = C("a64b4b")
    col[rect(86, 48, 88, 80)] = C("e6716f")
    for x0 in (52, 68):
        n = rect(72, x0 + 2, 80, x0 + 6) | rect(74, x0, 78, x0 + 8)
        col[n] = C("894746")
    img.put(m, col)
    return img.a


def sheep_spawn_egg(seed):
    """White egg, the sheep's tan face with a pink nose, the lower part brown."""
    img = Img()
    m = egg_mask()
    col = egg_base(m, "ececec", "ffffff", None, "ececec", "c7c7c7", "999999", "757575",
                   "5e5e5e", seed)
    col[(XX > 86 - (YY - 20) * 0.08) & (YY > 22)] = C("c7c7c7")
    col[band(m, 0, 1, 4) & ~band(m, 0, 1, 2) & (YY > 36) & (YY < 96)] = C("999999")
    # the brown lower part
    brown = m & (YY > 106 - ((XX - 64) / 44) ** 2 * 16)
    col[brown] = C("9c7960")
    col[brown & ~shift(brown, -2, 0)] = C("c7c7c7")
    col[brown & (XX > 60) & ~shift(brown, -4, 0) & shift(brown, -2, 0)] = C("9c7960")
    # face
    face = smooth_poly([(48, 52), (80, 52), (87, 58), (87, 86), (80, 95), (48, 95),
                        (40, 86), (40, 58)], 1)
    fc = np.zeros((S, S, 3), np.float32)
    fc[:] = C("bc9a81")
    upper_l = (XX - 40) + (YY - 52) * 1.2 < 30
    fc[upper_l & (YY < 72)] = C("e0b89d")
    fc[band(face, 0, 1, 2) | band(face, 1, 0, 2)] = C("9c7960")
    fc[(band(face, 0, 1, 4) | band(face, 1, 0, 4)) & ~(band(face, 0, 1, 2) |
                                                        band(face, 1, 0, 2))] = C("ad8a71")
    col[face] = fc[face]
    nose = ellipse(84, 65, 7, 9)
    col[nose] = C("ffb8b8")
    col[nose & (YY > 88)] = C("e69494")
    col[nose & ~shift(nose, 0, 2) & (YY > 86)] = C("e69494")
    # outline: grey around the white, brown around the brown
    col[lit(m, 2)] = C("757575")
    col[dark(m, 2)] = C("5e5e5e")
    col[(lit(m, 2) | dark(m, 2)) & (YY > 78)] = C("73543f")
    col[dark(m, 2) & (YY > 100)] = C("5e4534")
    col[brown & rim(m, 4) & ~rim(m, 2) & (YY > 104)] = C("87634a")
    img.put(m, col)
    return img.a


def wolf_spawn_egg(seed):
    """Faithful's wolf egg: a grey egg with two upright ears on its shoulders, a pale
    highlight on the upper left and the wolf's muzzle box with a dark nose and mouth."""
    img = Img()
    m = egg_mask()
    for sgn in (1, -1):
        def X(x):
            return x if sgn > 0 else 128 - x
        ear = polygon([(X(22), 14), (X(36), 14), (X(44), 22), (X(44), 52), (X(22), 52)])
        col = np.zeros((S, S, 3), np.float32)
        col[:] = C("e6e3e4")
        col[(np.abs(XX - 64) < 38) & (np.abs(XX - 64) >= 34)] = C("cac7c8")
        col[(np.abs(XX - 64) < 36) & (YY > 24) & (pix(seed + 7, 2) > 0.4)] = C("cac7c8")
        col[(np.abs(XX - 64) >= 38)] = C("b6b2b1")
        col[(np.abs(XX - 64) >= 40)] = C("262726")
        col[YY < 16] = C("393835")
        img.put(ear, col)
    col = np.zeros((S, S, 3), np.float32)
    col[:] = C("cac7c8")
    col[(XX > 84) & (YY > 36)] = C("9f9a96")
    col[(XX > 78) & (XX <= 84) & (YY > 40) & (pix(seed, 2) > 0.5)] = C("b6b2b1")
    col[YY > 96] = C("9f9a96")
    col[(YY > 84) & (YY <= 96) & (XX < 46) & (pix(seed + 1, 2) > 0.45)] = C("b6b2b1")
    hl = ellipse(50, 46, 17, 14)
    col[hl] = C("e6e3e4")
    col[(YY < 32) & (XX > 60) & (XX < 84)] = C("cac7c8")
    col[ellipse(26, 76, 8, 8) & (pix(seed + 2, 2) > 0.5)] = C("b6b2b1")
    col[band(m, 0, 1, 4) & ~band(m, 0, 1, 2) & (YY > 36)] = C("807b78")
    col[band(m, 1, 0, 4) & ~band(m, 1, 0, 2) & (YY > 100)] = C("807b78")
    col[lit(m, 2)] = C("665e57")
    col[dark(m, 2)] = C("4f4944")
    # muzzle box
    mz = rect(68, 42, 100, 86)
    mc = np.zeros((S, S, 3), np.float32)
    mc[:] = C("e4d8d9")
    mc[YY >= 84] = C("d3c0b8")
    mc[(YY >= 82) & (YY < 84) & ((XX < 52) | (XX >= 78))] = C("d3c0b8")
    mc[(XX < 46) | (YY < 72)] = C("c4ab9c")
    mc[(YY < 72) & (XX > 72)] = C("b19885")
    mc[(XX >= 80)] = C("9a8069")
    mc[(XX >= 84)] = C("665e57")
    nose = polygon([(52, 72), (68, 72), (68, 76), (65, 80), (56, 80), (52, 76)])
    mc[nose] = C("393835")
    mc[nose & ((XX - 60) + (YY - 72) > 8)] = C("262726")
    mc[rect(94, 46, 96, 80)] = C("393835")
    mc[rect(96, 46, 100, 84)] = C("9a8069")
    col[mz] = mc[mz]
    img.put(m, col)
    return img.a


def bone(seed):
    """Faithful's bone: a slender cream shaft on the diagonal, two round knuckles at each
    end, a pale rim on the upper left and a sandy shade on the lower right."""
    img = Img()
    shaft = (SUM >= 118) & (SUM < 142) & (DIF > -80) & (DIF < 80)
    knobs = [(95, 20, 11), (111, 31, 11), (21, 91, 13.5), (38, 104, 12)]
    m = shaft.copy()
    for x, y, r in knobs:
        m |= disk(y, x, r)
    m = ndimage.binary_closing(m, np.ones((5, 5), bool)) | m
    col = np.zeros((S, S, 3), np.float32)
    col[:] = C("fffffd")
    col[dark(m, 4)] = C("e8e5d2")
    col[lit(m, 4)] = C("fcfbed")
    col[(noise(seed + 1, 8, tile=False) > 0.68) & ~rim(m, 5)] = C("fcfbed")
    # the clefts between the knuckles
    cleft = seg((53, 11.5), (50.5, 15)) | seg((12, 53.5), (15.5, 49.5))
    col[grow(cleft, 1) & m] = C("e8e5d2")
    col[cleft & m] = C("cbc6a5")
    img.put(m, col)
    outline2(m, "cbc6a5", "7b7e6b", img)
    return img.a


def lantern(seed):
    """Faithful's lantern: a two-link hook, a small cap with a copper band, rounded top and
    bottom plates and a glowing glass framed in copper, brightest in the middle."""
    img = Img()
    A, Cc = C("495065"), C("3e4453")
    # hook: two chain links
    l1 = ellipse(23.5, 68, 8, 6) & ~ellipse(23.5, 68, 4.5, 2.2)
    l2 = ellipse(34, 63, 6.5, 5.5) & ~ellipse(33, 63, 3.5, 2)
    for ln in (l2, l1):
        c = np.zeros((S, S, 3), np.float32)
        c[:] = Cc
        c[XX < (68 if ln is l1 else 63)] = A
        img.put(ln, c)
    # cap with its copper band
    cap = rect(40, 48, 56, 80)
    c = np.zeros((S, S, 3), np.float32)
    c[:] = A
    c[rim(cap, 2)] = Cc
    c[rect(48, 48, 52, 80)] = C("814023")
    c[rect(48, 48, 52, 50) | rect(48, 78, 52, 80)] = C("8b5230")
    img.put(cap, c)
    # top and bottom plates: dark with a rounded lit middle
    for y0, flip in ((56, False), (104, True)):
        p = rect(y0, 40, y0 + 8, 88)
        yy = (YY - y0) if not flip else (y0 + 8 - YY)
        half = np.where(yy < 2, -1, np.where(yy < 4, 12, np.where(yy < 6, 16, 18)))
        c = np.zeros((S, S, 3), np.float32)
        c[:] = Cc
        c[np.abs(XX - 64) < half] = A
        img.put(p, c)
    # glass
    gl = rect(64, 40, 104, 88)
    d = (np.abs(XX - 64) ** 4 + (np.abs(YY - 87) * 1.05) ** 4) ** 0.25
    c = np.zeros((S, S, 3), np.float32)
    c[:] = C("c36322")
    c[d < 25] = C("f09149")
    c[d < 21] = C("f9c966")
    c[d < 12.5] = C("fdfd8b")
    c[(YY > 94) & (d < 24)] = C("f9c966")
    streak = seg((29, 40.5), (35.5, 46.5), 3.2)
    c[streak & (d < 13)] = C("ffffd5")
    c[(XX < 42) | (XX >= 86)] = C("814023")
    c[((XX >= 42) & (XX < 44)) | ((XX >= 84) & (XX < 86))] = C("8b5230")
    c[rect(100, 40, 104, 44) | rect(100, 84, 104, 88)] = C("814023")
    img.put(gl, c)
    return img.a


def oak_door(seed):
    """Faithful's oak door item: flat oak, two rows of window panes on top (their frame
    shaded on the top and left), two rows of sunken panels below, iron hinges and a lever
    handle."""
    img = Img()
    door = rect(16, 28, 124, 104)
    col = np.zeros((S, S, 3), np.float32)
    col[:] = C("9f844d")
    # a faint grain for the 128 detail
    gr = (noise(seed + 1, 16, tile=False) > 0.6) & (((XX + (noise(seed + 2, 24, tile=False)
                                                           * 16).astype(int)) % 12) == 0)
    col[gr] = C("967441")
    col[rect(16, 28, 124, 30)] = C("695433")
    col[rect(122, 28, 124, 104)] = C("695433")
    img.put(door, col)
    for x0 in (42, 74):
        for y0 in (26, 50):
            img.put(rect(y0 - 2, x0 - 2, y0, x0 + 22) | rect(y0, x0 - 2, y0 + 14, x0),
                    C("695433"))
            img.a[rect(y0, x0, y0 + 14, x0 + 22)] = 0
        for y0 in (74, 98):
            img.put(rect(y0, x0 - 2, y0 + 16, x0 + 22), C("967441"))
            img.put(rect(y0, x0 - 2, y0 + 2, x0 + 22) | rect(y0, x0 - 2, y0 + 16, x0),
                    C("7e6237"))
    for y0 in (32, 64, 96):
        h = rect(y0, 28, y0 + 8, 32)
        c = np.zeros((S, S, 3), np.float32)
        c[:] = C("6b6f7a")
        c[rect(y0, 28, y0 + 2, 32) | rect(y0, 28, y0 + 6, 30)] = C("808b95")
        img.put(h, c)
    # the lever handle, tilted down to the left
    lev = polygon([(82, 70), (86, 66), (100, 64), (100, 66), (88, 68), (84, 72)])
    img.put(rect(64, 86, 66, 96), C("808b95"))
    img.put(rect(66, 84, 68, 98), C("6b6f7a"))
    img.put(rect(66, 84, 68, 88), C("808b95"))
    img.put(rect(68, 82, 70, 92), C("6b6f7a"))
    img.put(rect(68, 90, 70, 92), C("7e6237"))
    img.put(rect(70, 82, 72, 86), C("6b6f7a"))
    img.put(rect(70, 86, 72, 88), C("7e6237"))
    del lev
    return img.a


def red_bed(seed):
    """Faithful's bed item: the bed in a 3/4 view, a long red blanket with a darker border
    line, the white pillow at the head (upper right), wooden frame and legs."""
    img = Img()
    Lp, Tp, Fp = np.array([10.0, 50.0]), np.array([84.0, 8.0]), np.array([40.0, 80.0])
    Rp = Fp + (Tp - Lp)
    # (s, t) of every pixel in the top face frame: s along L->T (foot -> head), t along L->F
    a, b = Tp - Lp, Fp - Lp
    det = a[0] * b[1] - a[1] * b[0]
    px, py = XX - Lp[0], YY - Lp[1]
    s = (px * b[1] - py * b[0]) / det
    t = (a[0] * py - a[1] * px) / det
    top = (s >= 0) & (s <= 1) & (t >= 0) & (t <= 1)
    H = 15

    def quad(p0, p1, h):
        return polygon([tuple(p0), tuple(p1), (p1[0], p1[1] + h), (p0[0], p0[1] + h)])
    foot = quad(Lp, Fp, H) & ~top
    side = quad(Fp, Rp, H) & ~top & ~foot
    # frame strips under the blanket, legs
    frame = (quad(Lp + [0, H], Fp + [0, H], 5) | quad(Fp + [0, H], Rp + [0, H], 5))
    frame &= ~top & ~foot & ~side
    legs = [rect(Lp[1] + H, 10, Lp[1] + H + 18, 17), rect(Fp[1] + H, 35, Fp[1] + H + 18, 44),
            rect(Rp[1] + H - 2, 110, Rp[1] + H + 14, 117)]
    for lg in legs:
        c = np.zeros((S, S, 3), np.float32)
        c[:] = C("695433")
        c[band(lg, 0, -1, 2)] = C("9f844d")
        c[band(lg, 0, 1, 2) | band(lg, 1, 0, 2)] = C("413421")
        img.put(lg, c)
    fc = np.zeros((S, S, 3), np.float32)
    fc[:] = C("695433")
    fc[band(frame, 1, 0, 2)] = C("413421")
    img.put(frame, fc)
    # sides: red blanket hanging down, the grey mattress under the pillow
    sc = np.zeros((S, S, 3), np.float32)
    sc[:] = C("720000")
    pil_side = side & (XX > Fp[0] + 0.76 * (Rp[0] - Fp[0]))
    sc[pil_side] = C("727272")
    sc[pil_side & band(side, -1, 0, 2)] = C("bebebe")
    img.put(foot | side, sc)
    img.put(band(foot, 0, 1, 2) & ~top, C("810707"))
    # top: blanket and pillow
    tc = np.zeros((S, S, 3), np.float32)
    tc[:] = C("8c1515")
    tc[(s > 0.045) & (s < 0.09) & (t > 0.08) & (t < 0.92)] = C("9c2626")
    tc[(s > 0.68) & (s < 0.72) & (t > 0.08)] = C("9c2626")
    tc[(t > 0.9) & (s < 0.72)] = C("810707")
    tc[(s >= 0.72) & (s < 0.76)] = C("810707")
    tc[s >= 0.76] = C("ebebeb")
    tc[(s >= 0.76) & ((s < 0.79) | (t > 0.93) | (s > 0.97) | (t < 0.05))] = C("bebebe")
    img.put(top, tc)
    whole = img.m
    rg = ring(whole, 2)
    upper = rg & (YY < Lp[1] + 2) | (rg & (s >= 0.74) & (YY < Rp[1] + 4))
    img.put(upper & (s < 0.74), C("720000"))
    img.put(upper & (s >= 0.74), C("727272"))
    img.put(rg & ~upper & (YY < Lp[1] + H), C("720000"))
    img.put(drop(whole, 2) & ~whole & ~img.m, C("413421"))
    return img.a


def shears(seed):
    """Faithful's shears: two flat steel blades on the upper right, red-brown handles bowing
    down to a steel ring at the bottom left."""
    img = Img()
    # handles
    h1, s1, t1 = stroke([(66, 22), (52, 36), (42, 50), (37, 64), (34, 78)], [12, 12, 13, 13, 12])
    h2, s2, t2 = stroke([(96, 68), (80, 77), (64, 85), (50, 91), (40, 95)], [12, 13, 12, 11, 9])
    for hm, sd in ((h2, s2), (h1, s1)):
        c = np.zeros((S, S, 3), np.float32)
        c[:] = C("8b4336")
        c[(sd > 0) & ~rim(hm, 3) & (hm is h1)] = C("a64f3f")
        c[(sd > 0) & ~rim(hm, 4) & (hm is h2) & (XX > 60)] = C("8b4336")
        c[band(hm, 1, 0, 2) | band(hm, 0, 1, 2)] = C("5c2d1f")
        c[band(hm, -1, 0, 2) | band(hm, 0, -1, 2)] = C("6f352b")
        img.put(hm, c)
    # ring
    rm = disk(95, 30, 15) & ~disk(95, 31, 7.5)
    c = np.zeros((S, S, 3), np.float32)
    c[:] = C("707070")
    c[(XX - 30) + (YY - 95) < -8] = C("a8a8a8")
    c[rm & ~rim(rm, 2) & ((XX - 30) + (YY - 95) < -12)] = C("a8a8a8")
    c[(XX - 30) + (YY - 95) > 10] = C("424242")
    c[band(rm, 1, 0, 2) | band(rm, 0, 1, 2)] = C("424242")
    img.put(rm, c)
    # blades
    b1 = Q([(36.5, 8), (51, 8), (35, 24), (29, 24)])
    b2 = Q([(56, 13), (57.5, 13), (57.5, 27.5), (40, 35), (39, 29)])
    for bm in (b1, b2):
        c = np.zeros((S, S, 3), np.float32)
        c[:] = C("d5d5d5")
        c[band(bm, 0, -1, 2) | band(bm, 0, 1, 2)] = C("a8a8a8")
        c[band(bm, 1, 0, 3)] = C("a8a8a8")
        c[band(bm, 1, 0, 2) & (XX > 80)] = C("707070")
        img.put(bm, c)
    img.put(band(b1, 0, -1, 2) & (YY > 16), C("707070"))
    return img.a


# ---------------------------------------------------------------------------- fish


# A cod lying corner to corner: its head at the bottom left, its tail fanning out at the top
# right. The spine (x, y) and the body's width along it.
COD_SPINE = [(22, 104), (34, 90), (50, 73), (66, 58), (80, 47), (92, 39)]
COD_W = [26, 46, 50, 42, 27, 14]


def spine_at(t):
    """The point of the spine a fraction `t` along it and the unit normal toward the back
    (up-left on screen)."""
    p = np.array(COD_SPINE, np.float32)
    seg_len = np.hypot(*np.diff(p, axis=0).T)
    s = t * seg_len.sum()
    i = min(int(np.searchsorted(np.cumsum(seg_len), s)), len(seg_len) - 1)
    f = (s - (np.cumsum(seg_len)[i] - seg_len[i])) / seg_len[i]
    d = (p[i + 1] - p[i]) / seg_len[i]
    return p[i] + (p[i + 1] - p[i]) * f, np.array([d[1], -d[0]]), d


def half_width(t):
    return np.interp(t, np.linspace(0, 1, len(COD_W)), np.array(COD_W, np.float32)) / 2


def fin(t0, t1, side, height, back=0.6):
    """A fin on the back (side 1) or the belly (side -1) between t0 and t1 along the spine,
    standing `height` px out of the body, swept back toward the tail."""
    pts = []
    for t, h in ((t0, -4), (t0 + (t1 - t0) * 0.25, height * 0.8), (t1 - (t1 - t0) * (1 - back) * 0.3, height),
                 (t1, height * 0.45), (t1, -4)):
        p, n, d = spine_at(t)
        q = p + n * side * (half_width(t) + h) + d * (h > 0) * height * 0.35
        pts.append((float(q[0]), float(q[1])))
    return smooth_poly(pts, 1)


def cod_fins():
    p, n, d = spine_at(1.0)
    fan = []
    for a, r in ((-0.6, 5), (-1.0, 38), (-0.3, 29), (0.0, 24), (0.3, 29), (1.0, 38), (0.6, 5)):
        # a fan behind the tail's root, notched in the middle
        dirv = d * np.cos(a * 0.75) + n * np.sin(a * 0.75)
        q = p - d * 3 + dirv * r
        fan.append((float(q[0]), float(q[1])))
    return dict(
        tail=smooth_poly(fan, 1),
        dorsal1=fin(0.26, 0.44, 1, 13),
        dorsal2=fin(0.5, 0.66, 1, 11),
        dorsal3=fin(0.72, 0.84, 1, 8),
        anal=fin(0.55, 0.72, -1, 9),
        pelvic=fin(0.24, 0.36, -1, 10),
    ), p


def cod_body():
    """The body's mask, its across coordinate (-1 belly .. +1 back) and along (0 head .. 1
    tail)."""
    body, side, t = stroke(COD_SPINE, COD_W)
    spine = stroke(COD_SPINE, [1.5] * len(COD_SPINE))[0]
    dist = ndimage.distance_transform_edt(~spine)
    u = np.clip(side * dist / np.maximum(half_width(t), 1), -1, 1)
    return body, u, t


def fin_rays(m, base, col, img, every=5):
    """A fin's colour with darker rays running out from `base` (x, y)."""
    ang = np.arctan2(YY - base[1], XX - base[0])
    rays = (np.floor(ang / np.radians(every * 2.2)) % 2 == 0) & ~rim(m, 2)
    c = np.zeros((S, S, 3), np.float32)
    c[:] = C(col[0])
    c[rays] = C(col[1])
    c[dark(m, 2)] = C(col[2])
    img.put(m, c)


def cod_like(cooked):
    """The cod: raw, a sandy fish with a mottled back, a pale lateral line and belly, three
    fins on its back and one under it, a fanned tail; cooked, the body white flaky flesh
    between a toasted head and tail, the eye gone pale."""
    def fn(seed):
        img = Img()
        body, u, t = cod_body()
        fins, root = cod_fins()
        fins = {k: v & ~body for k, v in fins.items()}
        whole = body.copy()
        for f in fins.values():
            whole |= f
        whole = ndimage.binary_closing(whole, np.ones((3, 3), bool)) | whole
        fin_col = ("c79b66", "a97c4c", "8a6038") if cooked else ("dcc6a0", "c3a47a", "a88458")
        fin_rays(fins["tail"], tuple(root), fin_col, img)
        for k in ("dorsal1", "dorsal2", "dorsal3", "anal", "pelvic"):
            ys, xs = np.nonzero(fins[k] | (grow(fins[k], 3) & body))
            fin_rays(fins[k], (xs.mean() + 12, ys.mean() + 12), fin_col, img, every=4)
        n = noise(seed + 1, 10, tile=False)
        g = pix(seed + 2, 2)
        col = np.zeros((S, S, 3), np.float32)
        head = t < 0.2
        if not cooked:
            col[:] = C("c49a66")
            col[u > 0.3] = C("b08654")
            col[(u > 0.15) & (n > 0.55)] = C("a07548")
            col[(u > 0.3) & (n > 0.62) & (g > 0.35)] = C("8e653b")
            spots = (pix(seed + 7, 4) > 0.8) & (u > 0.25) & (u < 0.85) & (t > 0.22) & (t < 0.9)
            col[spots] = C("8e653b")
            col[u < -0.3] = C("d6bf98")
            col[u < -0.55] = C("e2d0ae")
            col[(u < -0.3) & (u > -0.38) & (g > 0.5)] = C("c9ad84")
            # the lateral line, broken into dashes
            lat = (np.abs(u - 0.08) < 0.07) & (t > 0.2) & (t < 0.92)
            col[lat & ((XX + YY) % 6 < 4)] = C("e6d3ab")
            # scales: a faint net on the flank
            net = ((XX - YY) % 9 == 0) | ((XX + 2 * YY) % 11 == 0)
            col[net & (u > -0.3) & (u < 0.3) & (t > 0.24) & (t < 0.88) & ~lat] = C("b58c5a")
            col[head & (u > -0.3)] = C("c19664")
        else:
            col[:] = C("e6d9b8")
            # flakes: bands across the body, jagged, meeting at a seam along the middle
            k = t * 11 + np.abs(u) * 1.3 + (g - 0.5) * 0.35
            fl = np.floor(k) % 2 == 0
            col[fl] = C("f1e9cf")
            edge = (k % 1 < 0.14)
            col[edge & (t > 0.2)] = C("cfbd93")
            col[(np.abs(u) < 0.07) & (t > 0.2)] = C("d6c49b")
            # browned skin along the back and the belly edges
            col[u > 0.72] = C("d8b98a")
            col[u < -0.8] = C("dcc49a")
            col[head] = C("c79b68")
            col[head & (u < -0.3)] = C("d1a974")
            col[head & (u > 0.4)] = C("b88b5a")
            col[(t > 0.93)] = C("c79b68")
        # the gill cover's edge
        gill = (np.abs(np.hypot(XX - 20, YY - 106) - 25) < 1.2) & (u > -0.85) & body
        col[gill] = C("8c653c" if not cooked else "a37848")
        col[grow(gill, 1) & body & (np.hypot(XX - 20, YY - 106) > 25) & ~gill] = \
            C("d0ab77" if not cooked else "d8b384")
        # rims: lit on the top left, shaded on the bottom right
        col[lit(body, 2) & ~head] = C("d2ad78" if not cooked else "e8d6ad")
        col[dark(body, 3)] = C("9c7447" if not cooked else "b89262")
        col[dark(body, 3) & head] = C("8e6840")
        img.put(body, col)
        # the eye (and the mouth)
        eye = disk(92, 29, 4.6)
        if cooked:
            img.put(eye, C("5a4633"))
            img.put(disk(92, 29, 3.1), C("e9e2cf"))
            img.put(disk(91, 28, 1.2), C("ffffff"))
        else:
            img.put(eye, C("1b1410"))
            img.put(rect(89, 26, 91, 28), C("f2ece0"))
        mouth = seg((10.5, 50.5), (14.5, 52)) & body
        img.put(mouth, C("6e4c2c"))
        outline2(whole, "8a6440" if not cooked else "95704a", "553a22" if not cooked else "5c4028", img)
        return img.a
    return fn


# ---------------------------------------------------------------------------- table

TEXTURES = {
    "item/stick": stick,
    "item/coal": coal,
    "item/charcoal": charcoal,
    "item/iron_ingot": ingot(IRON_BAR),
    "item/gold_ingot": ingot(GOLD_BAR, gold_extra),
    "item/diamond": diamond,
    "item/iron_nugget": iron_nugget,
    "item/copper_ingot": ingot(COPPER_BAR, copper_extra),
    "item/clay_ball": clay_ball,
    "item/brick": brick,
    "item/bucket": bucket(None),
    "item/water_bucket": bucket("water"),
    "item/lava_bucket": bucket("lava"),
    "item/glass_bottle": bottle,
    "item/potion": bottle,
    "item/potion_overlay": potion_overlay,
    "item/porkchop": porkchop,
    "item/cooked_porkchop": cooked_porkchop,
    "item/mutton": mutton_like(MUTTON),
    "item/cooked_mutton": mutton_like(COOKED_MUTTON),
    "item/pig_spawn_egg": pig_spawn_egg,
    "item/sheep_spawn_egg": sheep_spawn_egg,
    "item/wolf_spawn_egg": wolf_spawn_egg,
    "item/bone": bone,
    "item/lantern": lantern,
    "item/oak_door": oak_door,
    "item/red_bed": red_bed,
    "item/shears": shears,
    "item/cod": cod_like(False),
    "item/cooked_cod": cod_like(True),
}
for _tier in TIER:
    TEXTURES[f"item/{_tier}_pickaxe"] = tool_pickaxe(_tier)
    TEXTURES[f"item/{_tier}_axe"] = tool_axe(_tier)
    TEXTURES[f"item/{_tier}_shovel"] = tool_shovel(_tier)
    TEXTURES[f"item/{_tier}_sword"] = tool_sword(_tier)
