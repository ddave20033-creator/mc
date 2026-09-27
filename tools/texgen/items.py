"""Item sprites in the Faithful 64x look, drawn natively at 128x128.

Faithful's items are flat-shaded pixel art: each part is a few flat colors (a body color, a
lit rim on the top-left edges, a shade band on the bottom-right edges) with a thin dark
outline mostly on the shadow side. We draw every part from geometric masks (polygons,
ellipses and variable-width strokes) at full 128 resolution with 2 px rims and outlines.
"""

from __future__ import annotations

import numpy as np
from scipy import ndimage

from common import (
    S, blank, disk, ellipse, grid, grow, hexc, noise, paint, pix, polygon, rect, rng,
    shrink,
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


def axis(p0, p1):
    """(u, v) of each pixel in the frame of p0 -> p1: u along (0 at p0), v across, positive
    toward the upper-left side."""
    (x0, y0), (x1, y1) = p0, p1
    L = np.hypot(x1 - x0, y1 - y0)
    dx, dy = (x1 - x0) / L, (y1 - y0) / L
    u = (XX - x0) * dx + (YY - y0) * dy
    nx, ny = dy, -dx  # rotate so that for an up-right axis n points up-left
    if nx > 0 or (nx == 0 and ny > 0):
        nx, ny = -nx, -ny
    v = (XX - x0) * nx + (YY - y0) * ny
    return u, v, L


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


def flat(m, body, hi=None, sh=None, k=2, klo=2):
    """Colors for a flat-shaded part: body, lit top-left rim and a bottom-right shade rim."""
    out = np.zeros((S, S, 3), np.float32)
    out[:] = C(body)
    if sh:
        out[dark(m, klo)] = C(sh)
    if hi:
        out[lit(m, k)] = C(hi)
    return out


def mottle(seed, cell, cols, weights, m=None, h=S, w=S, dither=0.25):
    """Flat palette patches from noise (Faithful-like soft mottling, little dither)."""
    t = noise(seed, cell, tile=False) * 0.65 + noise(seed + 1, cell / 2.5, tile=False) * 0.35
    t = (t - t.min()) / max(t.max() - t.min(), 1e-6)
    cum = np.cumsum(weights) / np.sum(weights)
    th = t + (pix(seed + 2, 2) - 0.5) * dither * 0.2
    idx = np.searchsorted(cum, np.clip(th, 0, 0.999))
    pal = np.stack([C(c) for c in cols])
    return pal[np.clip(idx, 0, len(cols) - 1)]


# ---------------------------------------------------------------------------- tools

HANDLE = dict(o="281e0b", main="493615", mid="684e1e", hi="896727")

# per tier: outline, dark, shade, body, light, highlight, seam
TIER = {
    "wooden": dict(o="20180a", d="372910", s="6b511f", b="755821", l="866526", h="866526",
                   m="594319"),
    "stone": dict(o="181818", d="494949", s="7f7f7f", b="898989", l="9a9a9a", h="9a9a9a",
                  m="6c6c6c"),
    "iron": dict(o="181818", d="444444", s="c1c1c1", b="d8d8d8", l="d8d8d8", h="ffffff",
                 m="969696"),
    "golden": dict(o="3f2e0e", d="825d16", s="e9b115", b="eaee57", l="eaee57", h="fdff76",
                   m="dc9613"),
    # Clear light cyan (a greener teal looked murky green in the hand).
    "diamond": dict(o="0a2c38", d="13586a", s="36b4cc", b="55d6ea", l="55d6ea", h="a8f4fc",
                    m="2a8fa6"),
}
# sword blades: light half, blade, fuller, guard, guard light, guard highlight, outline
SWORD = {
    "wooden": ("866526", "755821", "6b511f", "372910", "473614", "594319", "20180a"),
    "stone": ("b3b1af", "95918d", "878582", "494949", "5a5a5a", "787777", "212121"),
    "iron": ("ffffff", "d8d8d8", "bebebe", "444444", "6b6b6b", "969696", "181818"),
    "golden": ("ffffff", "fdff76", "eaee57", "825d16", "dc9613", "e9b115", "3f2e0e"),
    "diamond": ("c8f8fe", "5fdcee", "40c0d8", "13586a", "2a8fa6", "36b4cc", "0a2c38"),
}


def paint_handle(img: Img, p0, p1, w=9.0, seed=0, flare=None):
    """Thin wooden handle p0 (bottom left) -> p1, lit stripe on its upper-left side, dark
    outline on its lower-right side, a little grain along it."""
    u, v, L = axis(p0, p1)
    m = (u >= 0) & (u <= L) & (np.abs(v) <= w / 2)
    m |= disk(p0[1], p0[0], w / 2) | disk(p1[1], p1[0], w / 2)
    if flare is not None:
        m |= flare
    col = np.zeros((S, S, 3), np.float32)
    col[:] = C(HANDLE["main"])
    col[v > -w / 2 + 3.5] = C(HANDLE["mid"])
    col[v > w / 2 - 2.5] = C(HANDLE["hi"])
    # grain: short darker / lighter flecks along the handle
    g = pix(seed, 2)
    seg = ((u // 5).astype(int) * 7 + 3) % 11
    col[(seg < 2) & (np.abs(v) < 1.5) & m] = C(HANDLE["main"])
    col[(g > 0.9) & (v > 0) & (v < w / 2 - 2.5)] = C(HANDLE["hi"])
    img.put(drop(m), C(HANDLE["o"]))
    img.put(m, col)
    return m


def tool_pickaxe(tier):
    t = TIER[tier]

    def fn(seed):
        img = Img()
        paint_handle(img, (18, 114), (100, 34), seed=seed)
        pts = [(42, 25), (60, 26), (78, 29), (92, 36), (101, 46), (107, 60), (110, 76),
               (111, 94)]
        wid = [3, 5, 6, 7, 7, 6, 5, 3]
        m, side, _ = stroke(pts, wid)
        col = np.zeros((S, S, 3), np.float32)
        col[:] = C(t["b"])
        inner = m & (side < 0) & ~shrink(m, 1)
        col[m & (side < 0) & band(m, 1, -1, 3)] = C(t["s"])
        col[m & (side < 0) & (band(m, 1, 0, 2) | band(m, 0, -1, 2))] = C(t["s"])
        col[m & (side > 0) & (band(m, -1, 0, 2) | band(m, 0, 1, 2))] = C(t["h"])
        del inner
        rg = ring(m, 2)
        # dark edge on the inner (concave) side, the dark tone at the tips
        _, s2, tt = stroke(pts, [w + 4 for w in wid])
        img.put(rg & (s2 < 0), C(t["o"]))
        img.put(rg & (s2 > 0) & ((tt < 0.06) | (tt > 0.94)), C(t["d"]))
        img.put(m, col)
        return img.a
    return fn


def tool_axe(tier):
    t = TIER[tier]

    def fn(seed):
        img = Img()
        paint_handle(img, (18, 114), (96, 38), seed=seed)
        # blade: a quarter disc on the upper left of the handle top
        blade = ellipse(49, 89, 31, 31) & (YY < 49) & (XX < 89)
        blade |= rect(18, 82, 49, 89)
        blade &= ~(XX + YY < 88)  # cut the pointy corner a little
        col = np.zeros((S, S, 3), np.float32)
        col[:] = C(t["s"])
        rim = blade & ~ellipse(49, 89, 26, 26)
        col[rim] = C(t["h"])
        col[blade & (XX > 84)] = C(t["b"])
        col[rim & (XX > 84)] = C(t["h"])
        img.put(drop(blade) & (YY >= 48), C(t["o"]))
        img.put(blade, col)
        # poll: the rounded back of the head (lower right)
        poll, _, _ = rrect(97, 57, 8.5, 8.5, 0, 6)
        pc = flat(poll, t["b"], t["h"], t["s"])
        img.put(drop(poll), C(t["o"]))
        img.put(poll, pc)
        # collar around the handle top
        coll, cu, cv = rrect(89, 50, 11, 8, -45, 4)
        cc = flat(coll, t["b"], None, t["s"])
        cc[band(coll, -1, 0, 1)] = C(t["h"])
        cc[coll & (np.abs(cu) < 1.2)] = C(t["m"])
        img.put(drop(coll) & ~blade & ~poll, C(t["o"]))
        img.put(coll, cc)
        return img.a
    return fn


def tool_shovel(tier):
    t = TIER[tier]

    def fn(seed):
        img = Img()
        flare = polygon([(15, 103), (31, 119), (36, 108), (26, 98)])
        # Handle and blade on one straight 45 degree line.
        paint_handle(img, (26, 106), (80, 52), seed=seed, flare=flare)
        u, v, _ = axis((78, 54), (110, 22))
        qv = np.abs(v)
        blade = (u >= 2) & (u <= 44) & (qv <= 16)
        corner = (u > 44 - 10) & (qv > 16 - 10)
        blade &= ~corner | ((u - 34) ** 2 + (qv - 6) ** 2 <= 100)
        col = flat(blade, t["s"], t["h"], None, k=3)
        col[band(blade, 0, 1, 3) & (YY < 44)] = C(t["h"])
        col[band(blade, 1, 1, 3) & (YY >= 36)] = C(t["b"])
        img.put(drop(blade), C(t["o"]))
        img.put(blade, col)
        return img.a
    return fn


def tool_sword(tier):
    hi, body, fuller, guard, guard_l, guard_h, o = SWORD[tier]

    def fn(seed):
        img = Img()
        # grip and pommel
        paint_handle(img, (22, 110), (44, 88), w=9, seed=seed)
        pom = disk(118, 10, 8)
        pc = flat(pom, guard_l, guard_h, guard, k=3, klo=3)
        img.put(drop(pom), C(o))
        img.put(pom, pc)
        # blade narrowing to a point over its last 18 px
        u, v, L = axis((48, 80), (121, 7))
        half = 7.5 * np.clip((L - u) / 18.0, 0.0, 1.0)
        blade = (u >= -6) & (u <= L) & (np.abs(v) <= np.maximum(half, 0.8))
        bc = np.zeros((S, S, 3), np.float32)
        bc[:] = C(body)
        bc[v > 0] = C(hi)
        bc[(np.abs(v) < 1.3) & ((u // 3).astype(int) % 2 == 0) & (u > 4) & (u < L - 6)] = \
            C(fuller)
        img.put(drop(blade), C(o))
        img.put(blade, bc)
        # curved cross guard: a spike to the upper left, a hooked arm to the lower right
        up = stroke([(58, 84), (44, 74), (30, 64), (18, 56)], [16, 12, 7, 3])[0]
        low = stroke([(50, 88), (60, 94), (68, 102), (72, 111)], [13, 10, 7, 3])[0]
        gm = up | low | disk(86, 50, 9)
        gc = np.zeros((S, S, 3), np.float32)
        gc[:] = C(guard)
        gc[up] = C(guard_l)
        gc[up & lit(up, 3)] = C(guard_h)
        img.put(drop(gm, 2), C(o))
        img.put(gm, gc)
        return img.a
    return fn


# ---------------------------------------------------------------------------- materials


def stick(seed):
    img = Img()
    paint_handle(img, (19, 115), (113, 21), w=9, seed=seed)
    return img.a


def lumps(m, seed, n, pal, base_of=None, speck=0.12):
    """Rounded lumps (cells) inside `m`, each flat with a lighter top-left rim, a darker
    bottom-right rim and dark seams between them. `pal` is dark -> light; `base_of(ids)`
    optionally gives each pixel's base palette index."""
    g = rng(seed)
    ys, xs = np.nonzero(m)
    pts = np.stack([g.uniform(xs.min(), xs.max(), n), g.uniform(ys.min(), ys.max(), n)], 1)
    wy = (noise(seed + 3, 16, tile=False) - 0.5) * 16
    wx = (noise(seed + 4, 16, tile=False) - 0.5) * 16
    d = np.stack([np.hypot(XX + wx - x, YY + wy - y) * (1 + 0.25 * g.random())
                  for x, y in pts])
    ids = d.argmin(0)
    idx = np.full((S, S), 2.0) if base_of is None else base_of(ids).astype(np.float32)
    idx = idx + (g.random(n)[ids] > 0.6)
    tl = (np.roll(ids, (3, 3), (0, 1)) != ids)
    br = (np.roll(ids, (-3, -3), (0, 1)) != ids)
    seam = (np.roll(ids, (1, 1), (0, 1)) != ids) | (np.roll(ids, 1, 0) != ids)
    idx = idx + tl * 1.0 - br * 1.0
    idx = idx + (pix(seed + 7, 2) > 1 - speck) * 1.0 - (pix(seed + 8, 2) < speck) * 1.0
    cy, cx = ys.mean(), xs.mean()
    idx = idx + ((XX - cx) + (YY - cy) < -30) * 1.0 - ((XX - cx) + (YY - cy) > 30) * 1.0
    idx[seam] -= 1.5
    pal_a = np.stack([C(c) for c in pal])
    return pal_a[np.clip(idx, 0, len(pal) - 1).astype(int)]


def coal(seed):
    img = Img()
    m = smooth_poly(COAL_PTS, 3)
    col = lumps(m, seed, 11, ["1f1721", "252525", "2e2e2e", "323232", "363636", "393e46"])
    col[lit(m, 2)] = C("363636")
    col[dark(m, 3)] = C("1c1c1e")
    img.put(drop(m), C("101015"))
    img.put(m, col)
    return img.a


COAL_PTS = [(52, 18), (70, 18), (86, 28), (98, 44), (110, 66), (114, 88), (108, 106),
            (92, 116), (72, 119), (48, 116), (30, 104), (20, 86), (18, 66), (24, 46), (36, 26)]
CHAR_PTS = [(38, 12), (86, 10), (104, 20), (114, 44), (118, 80), (112, 104), (94, 118),
            (60, 120), (32, 114), (18, 98), (16, 60), (22, 30)]


def charcoal(seed):
    img = Img()
    m = smooth_poly(CHAR_PTS, 2)
    ridge = (YY - 18) + np.abs(XX - 70) * 0.35 + noise(seed + 9, 16, tile=False) * 16

    def base(ids):
        return np.where(ridge < 40, 5, np.where(ridge < 50, 3, 2))
    col = lumps(m, seed, 12, ["13110d", "1d1a14", "231f18", "2b261d", "312b22", "423b2f",
                              "4e4536", "605543", "7d6f58"], base, speck=0.1)
    col[lit(m, 2)] = C("312b22")
    col[dark(m, 3)] = C("13110d")
    img.put(drop(m), C("0c0a07"))
    img.put(m, col)
    return img.a


def bar(top, end, front, cols, seed, texture=None):
    """A 3/4 view bar (ingot, brick): top face, small left end face, long front face."""
    ctop, cend, cfront, chi, cfront_hi, cfront_lo, cout = cols
    img = Img()
    T, E, F = polygon(top), polygon(end), polygon(front)
    E &= ~T
    F &= ~T & ~E
    whole = T | E | F
    img.put(drop(whole), C(cout))
    img.put(ring(whole, 1) & ~drop(whole) & (YY > 40), C(cout))
    img.put(F, C(cfront))
    img.put(band(F, -1, 0, 3) & ~E, C(cfront_hi))
    img.put(band(F, 1, 0, 2) | band(F, 0, 1, 2), C(cfront_lo))
    img.put(E, C(cend))
    img.put(band(E, 1, 0, 2), C(cfront_lo))
    img.put(T, C(ctop))
    # bright edges: along the top's front and end edges
    img.put(band(T, 1, 0, 2) | band(T, 0, -1, 2), C(chi))
    if texture is not None:
        texture(img, T, E, F)
    return img.a


INGOT = (
    [(2, 47), (91, 17), (126, 47), (35, 78)],
    [(2, 47), (35, 78), (35, 110), (2, 81)],
    [(35, 78), (126, 47), (126, 79), (35, 110)],
)


def ingot(cols):
    def fn(seed):
        return bar(*INGOT, cols, seed)
    return fn


def brick(seed):
    def tex(img, T, E, F):
        g = pix(seed, 2)
        img.put(T & (g > 0.93), C("c76245"))
        img.put(F & (g > 0.94), C("612f22"))
        img.put(F & (g < 0.04), C("8e4631"))
    return bar(*INGOT, ("b75a40", "8e4631", "7f3e2c", "c76245", "8e4631", "612f22", "2d1610"),
               seed, tex)


def diamond(seed):
    img = Img()
    vy = (YY - 65) / 49.5
    hw = 42 * np.clip(1 - np.abs(vy) ** 2.2, 0, 1) ** (1 / 2.2) * (1 - 0.14 * vy)
    m = (np.abs(XX - 64) <= hw) & (np.abs(vy) <= 1) & (YY >= 17)
    col = np.zeros((S, S, 3), np.float32)
    col[:] = C("4aedd9")
    lower = (YY - 64) + (XX - 64) * 0.2 > 0
    col[lower] = C("20c5b5")
    # facets: cell borders of a few points, light lines above, darker lines below
    g = rng(seed + 3)
    pts = [(34, 34), (64, 32), (94, 34), (28, 58), (50, 56), (76, 56), (100, 58), (36, 84),
           (58, 82), (82, 82), (100, 84), (64, 106)]
    d = np.stack([np.hypot(XX - (x + g.uniform(-3, 3)), YY - (y + g.uniform(-3, 3)))
                  for x, y in pts])
    ids = d.argmin(0)
    edge = np.zeros((S, S), bool)
    for sh in ((0, 1), (1, 0), (1, 1), (0, 2), (2, 0)):
        edge |= ids != np.roll(ids, sh, (0, 1))
    edge &= m
    col[edge & ~lower] = C("a1fbe8")
    col[edge & lower] = C("1aaaa7")
    # a couple of bright facets upper left
    col[(ids == 4) & ~edge] = C("a1fbe8") * 0 + C("4aedd9")
    col[(ids == 0) & ~edge & ~lower] = C("a1fbe8")
    col[stroke([(54, 46), (64, 36), (74, 28)], [5, 6, 5])[0] & ~edge] = C("a1fbe8")
    col[lit(m, 2)] = C("d5fff6")
    col[lit(m, 2) & (YY < 50) & (XX < 60)] = C("ffffff")
    col[dark(m, 2)] = C("11727a")
    img.put(drop(m), C("145e53"))
    img.put(ring(m, 1) & ~drop(m), C("145e53"))
    img.put(m, col)
    return img.a


def iron_nugget(seed):
    img = Img()
    m = smooth_poly([(36, 58), (44, 49), (58, 48), (74, 53), (90, 62), (76, 70), (60, 78),
                     (46, 81), (37, 74)], 2)
    col = np.zeros((S, S, 3), np.float32)
    col[:] = C("d9dfe7")
    col[(YY - 62) + (XX - 60) * 0.2 > 4] = C("bdcadb")
    col[lit(m, 3)] = C("f2f2f2")
    col[dark(m, 2)] = C("a2b0be")
    col[m & rect(66, 40, 70, 48)] = C("f2f2f2")
    img.put(drop(m), C("585f68"))
    img.put(ring(m, 1) & ~drop(m), C("738d8d"))
    img.put(m, col)
    return img.a


def clay_ball(seed):
    img = Img()
    m = smooth_poly([(58, 28), (80, 29), (98, 40), (108, 60), (106, 84), (94, 101), (72, 109),
                     (48, 106), (30, 94), (21, 74), (24, 50), (38, 34)], 3)
    col = mottle(seed, 12, ["757d90", "9499a4", "a1a7b1", "acaebd"], [1.5, 3, 3, 1.2])
    shadow = (XX - 64) * 0.6 + (YY - 68) * 0.8 + noise(seed + 4, 10, tile=False) * 14 > 26
    col[shadow & m] = mottle(seed + 2, 10, ["5e6c8d", "757d90", "9499a4"], [2, 3, 1])[shadow & m]
    col[lit(m, 2) & (YY < 70)] = C("afb9d6")
    img.put(drop(m), C("373944"))
    img.put(ring(m, 1) & ~drop(m), C("40445a"))
    img.put(m, col)
    return img.a


# ---------------------------------------------------------------------------- containers

B_TOP = 34


def bucket_body(img: Img):
    t = np.clip((YY - B_TOP) / 76, 0, 1)
    hw = 42 - 15 * t**2.2
    body = (np.abs(XX - 64) <= hw) & (YY >= B_TOP) & (YY <= 110)
    body |= ellipse(109, 64, 6, 26)
    top = ellipse(B_TOP, 64, 18, 42)
    body |= top
    # vertical bands across the cylinder (following its taper): light left, dark right
    x = (XX - 64) / np.maximum(hw, 1)
    col = np.zeros((S, S, 3), np.float32)
    col[:] = C("a8a8a8")
    col[x < -0.12] = C("d8d8d8")
    col[(x >= 0.05) & (x < 0.28)] = C("969696")
    col[(x >= 0.28) & (x < 0.5)] = C("a8a8a8")
    col[x >= 0.5] = C("727272")
    col[x >= 0.78] = C("5f5f5f")
    col[band(body, 0, -1, 3)] = C("a8a8a8")
    col[(x > -0.78) & (x < -0.68) & (YY > 58) & (YY < 84)] = C("ffffff")
    col[band(body, 1, 0, 3) & (YY > 100)] = C("727272")
    img.put(drop(body), C("353535"))
    img.put(ring(body, 1) & ~drop(body), C("545454"))
    img.put(body, col)
    # thin rim with a dark edge
    img.put(top, C("a8a8a8"))
    img.put(top & (XX < 50), C("d8d8d8"))
    img.put(top & (XX > 88), C("727272"))
    img.put(band(top, -1, 0, 2), C("353535"))
    hole = ellipse(B_TOP + 1, 64, 15, 39)
    return hole


def bucket(content=None):
    def fn(seed):
        img = Img()
        hole = bucket_body(img)
        if content is None:
            col = np.zeros((S, S, 3), np.float32)
            col[:] = C("727272")
            col[XX < 50] = C("969696")
            col[XX > 72] = C("5f5f5f")
            col[XX > 90] = C("545454")
            col[band(hole, -1, 0, 3)] = C("353535")
            col[band(hole, 1, 0, 2)] = C("353535")
            img.put(hole, col)
        elif content == "water":
            hole = shrink(ellipse(B_TOP, 64, 18, 42), 2)
            col = mottle(seed, 10, ["234fcc", "2e58d3", "345fda", "446fe9", "5a82f3"],
                         [1, 3, 3, 2, 1])
            col[band(hole, -1, 0, 2)] = C("234fcc")
            img.put(hole, col)
            img.put(band(hole, 1, 0, 2), C("353535"))
        else:
            hole = shrink(ellipse(B_TOP, 64, 18, 42), 2)
            lava = hole | stroke([(96, 40), (97, 52), (95, 66), (95, 76)], [8, 6, 5, 4])[0]
            g = rng(seed)
            pts = [(x + g.uniform(-4, 4), y + g.uniform(-3, 3))
                   for y in (22, 34, 46) for x in range(26 + (y // 12) % 2 * 8, 106, 16)]
            d = np.stack([np.hypot(XX - x, (YY - y) * 1.5) for x, y in pts])
            ds = np.sort(d, 0)
            gap = ds[1] - ds[0]
            col = np.zeros((S, S, 3), np.float32)
            col[:] = C("e4d25c")
            col[gap < 7] = C("e38c3f")
            col[gap < 3] = C("cc4628")
            col[~hole] = C("e38c3f")
            col[~hole & (XX > 96)] = C("cc4628")
            col[band(lava, 1, 0, 2) & ~hole] = C("cc4628")
            img.put(lava, col)
            img.put(drop(lava) & ~hole & (YY > B_TOP + 10), C("7f3e2c"))
        return img.a
    return fn


def bottle_shape():
    body = disk(85, 64, 34)
    neck = rect(30, 56, 52, 72)
    shoulder = polygon([(56, 48), (72, 48), (89, 62), (39, 62)])
    return body | neck | shoulder


def bottle(seed):
    img = Img()
    m = bottle_shape()
    line = m & ~shrink(m, 2)
    line &= ~rect(0, 0, 32, 128)  # the neck top is under the cork
    col = np.zeros((S, S, 3), np.float32)
    col[:] = C("8badd0")
    col[(XX > 72) | (YY > 106)] = C("5d8fc2")
    col[(XX < 50) & (YY < 96)] = C("b3cfec")
    img.put(line, col)
    # glints inside: short curved streaks upper left, a small one lower right
    g1 = stroke([(44, 84), (47, 74), (53, 66)], [2, 2, 2])[0]
    g2 = stroke([(50, 84), (53, 76), (57, 71)], [2, 2, 2])[0]
    g3 = stroke([(86, 96), (84, 104), (79, 110)], [2, 2, 2])[0]
    img.put(g1, C("d4e5f7"))
    img.put(g2, C("b3cfec"))
    img.put(g3, C("8badd0"))
    # cork
    cork = rrect(65, 26, 10, 8, 0, 5)[0]
    cc = np.zeros((S, S, 3), np.float32)
    cc[:] = C("a94725")
    cc[band(cork, 1, 0, 3) | band(cork, 0, 1, 2)] = C("973716")
    cc[lit(cork, 2)] = C("d46d49")
    cc[cork & (YY >= 30) & (YY < 32)] = C("ccafa5")
    img.put(cork, cc)
    img.put(rect(34, 52, 36, 76), C("b3cfec"))
    return img.a


def potion_overlay(seed):
    img = Img()
    m = disk(86, 64, 30)
    col = np.zeros((S, S, 3), np.float32)
    col[:] = C("c5c5c5")
    col[(XX - 64) + (YY - 88) * 0.8 > 8] = C("a6a6a6")
    col[(XX - 64) + (YY - 88) * 0.8 > 26] = C("9a9a9a")
    col[stroke([(76, 70), (84, 78), (87, 90)], [5, 6, 4])[0]] = C("a6a6a6")
    col[stroke([(46, 76), (52, 68), (58, 64)], [4, 4, 3])[0]] = C("ffffff")
    col[stroke([(44, 84), (47, 74), (53, 66)], [2, 2, 2])[0]] = C("636363")
    col[stroke([(86, 96), (84, 104), (79, 110)], [2, 2, 2])[0]] = C("636363")
    col[band(m, -1, 0, 2) & (XX < 64)] = C("ffffff")
    img.put(m, col)
    return img.a


# ---------------------------------------------------------------------------- food


def slab(cx, cy, hl, hw, deg, r, thick):
    """A rounded chop seen from above at an angle: top face and a lower side band."""
    top, u, v = rrect(cx, cy, hl, hw, deg, r)
    below = shift(top, thick, 0) | shift(top, thick // 2, 0)
    side = below & ~top
    return top, side, u, v


PORK_RAW = {"main": "ef7070", "warm": "ff7777", "lit": "ff8c8c", "fat": "ffadad",
            "fat2": "ffc6c6", "dark": "a75353", "side": "a75353", "side2": "853e3e",
            "o": "512626"}
# Roasted: the same chop, browned; the fat caramelized to a light golden brown.
PORK_COOKED = {"main": "a4582f", "warm": "b3643a", "lit": "c47a47", "fat": "d89a62",
               "fat2": "e8b682", "dark": "6e3a1f", "side": "7a4022", "side2": "5a2e18",
               "o": "3a1d10"}


def porkchop_like(c):
    def fn(seed):
        img = Img()
        top = smooth_poly([(16, 78), (22, 64), (36, 52), (52, 48), (64, 34), (80, 22),
                           (98, 18), (112, 23), (119, 37), (116, 56), (104, 76), (84, 92),
                           (62, 104), (46, 108), (30, 104), (18, 94)], 2)
        side = (shift(top, 8, 0) | shift(top, 4, 0)) & ~top
        whole = top | side
        col = np.zeros((S, S, 3), np.float32)
        col[:] = C(c["main"])
        col[(XX - 70) * 0.5 + (YY - 60) < -4] = C(c["warm"])
        col[lit(top, 3)] = C(c["lit"])
        # marbling: pale curling fat bands
        for pts, w in [([(30, 96), (40, 84), (46, 70), (58, 62), (72, 64), (80, 74), (78, 86)],
                        [6, 8, 10, 10, 9, 7, 4]),
                       ([(66, 44), (82, 38), (98, 42), (106, 54), (102, 66)], [4, 7, 8, 6, 4]),
                       ([(56, 84), (64, 78), (70, 82)], [4, 4, 3])]:
            s_, sd, _ = stroke(pts, w)
            s_ &= shrink(top, 3)
            col[s_] = C(c["lit"])
            col[s_ & (sd > 0) & ~band(s_, -1, -1, 2)] = C(c["fat"])
            col[s_ & band(s_, -1, -1, 1) & (sd > 0)] = C(c["fat2"])
        col[dark(top, 2)] = C(c["dark"])
        img.put(side, C(c["side"]))
        img.put(band(side, 1, 0, 3) & side, C(c["side2"]))
        img.put(drop(whole), C(c["o"]))
        img.put(top, col)
        return img.a
    return fn


porkchop = porkchop_like(PORK_RAW)
cooked_porkchop = porkchop_like(PORK_COOKED)


MUTTON_PTS = [(98, 8), (104, 20), (107, 40), (107, 66), (103, 90), (92, 108), (72, 119),
              (46, 122), (26, 116), (19, 104), (24, 92), (40, 83), (60, 74), (76, 58),
              (86, 36), (92, 16)]


def mutton_like(c):
    def fn(seed):
        img = Img()
        m = smooth_poly(MUTTON_PTS, 3)
        col = np.zeros((S, S, 3), np.float32)
        col[:] = C(c["main"])
        col[(XX - 70) * 0.4 + (YY - 60) * -0.2 + noise(seed, 16, tile=False) * 10 < 0] = \
            C(c["light"])
        # a pale seam curving across the base, and a lit rim inside the upper-left edge
        side = band(m, 1, 0, 9) | band(m, 0, 1, 4)
        top = m & ~side
        loop = shrink(top, 5) & ~shrink(top, 8)
        seam = stroke([(66, 62), (76, 70), (84, 82), (88, 96)], [3, 3, 3, 3])[0] & shrink(top, 5)
        col[loop | seam] = C(c["fat"])
        col[(loop | seam) & band(loop | seam, -1, -1, 1)] = C(c["fat2"])
        col[lit(m, 3)] = C(c["hi"])
        col[side] = C(c["side"])
        col[band(m, 1, 0, 4) | band(m, 0, 1, 2)] = C(c["dark"])
        img.put(drop(m), C(c["o"]))
        img.put(m, col)
        return img.a
    return fn


MUTTON = dict(main="d12e26", light="dd3830", fat="e2625a", fat2="e27269", hi="e88a82",
              side="ad332e", dark="96211b", o="470a06")
COOKED_MUTTON = dict(main="82483a", light="884f40", fat="9d6147", fat2="9c6854", hi="a3705a",
                     side="6b3a2a", dark="522f1f", o="2a160d")


def egg_mask():
    vy = (YY - 70) / 52
    half = 44 * np.sqrt(np.clip(1 - vy**2, 0, 1)) * (1 + 0.12 * np.clip(vy, -1, 1))
    return (np.abs(XX - 64) <= half) & (np.abs(vy) <= 1)


def pig_spawn_egg(seed):
    img = Img()
    m = egg_mask()
    col = np.zeros((S, S, 3), np.float32)
    col[:] = C("ec8985")
    col[(XX - 50) ** 2 + (YY - 50) ** 2 < 34**2] = C("f19e98")
    col[(XX - 76) + (YY - 76) * 0.6 > 30] = C("e6716f")
    col[(XX - 48) ** 2 + (YY - 42) ** 2 < 14**2] = C("ffb9b5")
    col[lit(m, 3)] = C("f19e98")
    col[dark(m, 3)] = C("c6615a")
    # snout
    sn = rect(66, 46, 89, 82)
    col[sn] = C("ec8985")
    col[sn & ~shrink(sn, 2)] = C("a64b4b")
    for cx in (57, 71):
        col[rect(74, cx - 1, 82, cx + 3) | rect(76, cx - 3, 80, cx + 5)] = C("894746")
    img.put(drop(m), C("914040"))
    img.put(m, col)
    return img.a


def sheep_spawn_egg(seed):
    img = Img()
    m = egg_mask()
    col = np.zeros((S, S, 3), np.float32)
    col[:] = C("ececec")
    col[(XX - 76) + (YY - 76) * 0.6 > 26] = C("c7c7c7")
    col[(XX - 48) ** 2 + (YY - 40) ** 2 < 16**2] = C("ffffff")
    col[lit(m, 3)] = C("ffffff")
    # the sheep's face and brown lower part
    face = rrect(64, 73, 21, 24, 90, 6)[0]
    col[face] = C("bc9a81")
    col[face & (XX > 76)] = C("ad8a71")
    col[face & lit(face, 2)] = C("e0b89d")
    nose = ellipse(86, 62, 6, 8)
    col[nose] = C("ffb8b8")
    col[nose & ~shift(nose, 2, 2)] = C("ffd0d0")
    col[rect(92, 44, 94, 84)] = C("9c7960")
    bottom = m & (YY > 106)
    col[bottom] = C("9c7960")
    col[bottom & (XX > 80)] = C("87634a")
    col[dark(m, 2) & (YY < 104)] = C("c7c7c7")
    img.put(drop(m), C("5e4534"))
    img.put(drop(m) & (YY < 96), C("757575"))
    img.put(m, col)
    return img.a


# ---------------------------------------------------------------------------- block items


def lantern(seed):
    img = Img()
    # two chain links on top
    hook = (ellipse(21, 66, 6, 4.5) & ~ellipse(21, 66, 3, 1.5))
    hook |= (ellipse(31, 66, 5, 4) & ~ellipse(31, 66, 2, 1.5)) | rect(34, 64, 40, 68)
    img.put(drop(hook, 1), C("2a2f3a"))
    img.put(hook, C("3e4453"))
    img.put(lit(hook, 1), C("495065"))
    cap = polygon([(49, 39), (79, 39), (84, 52), (44, 52)])
    img.put(cap, C("3e4453"))
    img.put(band(cap, -1, 0, 3) | band(cap, 0, -1, 2), C("495065"))
    img.put(rect(47, 44, 51, 86), C("814023"))
    img.put(rect(47, 44, 49, 86), C("8b5230"))
    body = rect(52, 38, 112, 88)
    img.put(body, C("3e4453"))
    img.put(band(body, 0, -1, 2) | band(body, -1, 0, 2), C("495065"))
    glass = rect(60, 42, 104, 81)
    col = np.zeros((S, S, 3), np.float32)
    col[:] = C("f9c966")
    core = rrect(61, 83, 15, 16, 0, 7)[0]
    col[core] = C("fdfd8b")
    col[stroke([(54, 92), (62, 84), (70, 76)], [3, 4, 3])[0] & core] = C("ffffd5")
    col[stroke([(58, 96), (70, 86), (76, 76)], [2, 2, 2])[0] & core] = C("ffffd5")
    col[band(glass, 0, -1, 4) | band(glass, -1, 0, 4) | band(glass, 1, 0, 4)
        | band(glass, 0, 1, 4)] = C("f09149")
    col[band(glass, 0, -1, 2) | band(glass, -1, 0, 2) | band(glass, 1, 0, 2)
        | band(glass, 0, 1, 2)] = C("c36322")
    img.put(glass, col)
    img.put(rect(104, 38, 112, 88), C("3e4453"))
    img.put(rect(81, 84, 104, 88), C("2f3441"))
    img.put(drop(body | cap, 1), C("2a2f3a"))
    return img.a


def oak_door(seed):
    img = Img()
    door = rect(16, 30, 124, 104)
    col = np.zeros((S, S, 3), np.float32)
    col[:] = C("9f844d")
    # faint vertical grain streaks
    gr = noise(seed + 1, 6, tile=False)[:, :1] * np.ones((1, S))
    streak = (noise(seed + 2, 12, tile=False) > 0.55) & (((XX + (gr * 20).astype(int)) % 9) < 2)
    col[streak] = C("967441")
    img.put(door, col)
    img.put(dark(door, 2), C("7e6237"))
    img.put(drop(door), C("695433"))
    # lower panels (raised), upper window panes (transparent)
    for x0, x1 in ((40, 62), (72, 94)):
        for y0, y1 in ((74, 94), (98, 118)):
            p = rect(y0, x0, y1, x1)
            img.put(p, C("967441"))
            img.put(ring(p, 2) & door, C("7e6237"))
            img.put(band(p, 1, 0, 2) | band(p, 0, 1, 2), C("9f844d"))
        for y0, y1 in ((24, 40), (48, 68)):
            p = rect(y0, x0, y1, x1)
            img.put(ring(p, 2), C("695433"))
            img.put(ring(p, 2) & (shift(p, 2, 0) | shift(p, 0, 2)) & ~p, C("7e6237"))
            img.a[p] = 0
    # hinges and handle
    for y0 in (28, 62, 100):
        h = rect(y0, 28, y0 + 8, 34)
        img.put(h, C("6b6f7a"))
        img.put(lit(h, 1), C("808b95"))
    hd = rect(64, 84, 70, 96)
    img.put(hd, C("6b6f7a"))
    img.put(lit(hd, 1), C("808b95"))
    return img.a


def red_bed(seed):
    img = Img()
    L, B, R, F = (10, 47), (65, 19), (100, 49), (45, 77)
    H = 20
    top = polygon([L, B, R, F])
    front = polygon([L, F, (F[0], F[1] + H), (L[0], L[1] + H)])
    right = polygon([F, R, (R[0], R[1] + H), (F[0], F[1] + H)])
    # legs
    for lg in (rect(64, 10, 86, 17), rect(94, 36, 112, 46), rect(60, 108, 74, 118)):
        c = np.zeros((S, S, 3), np.float32)
        c[:] = C("695433")
        c[band(lg, 0, -1, 2)] = C("9f844d")
        c[band(lg, 0, 1, 2)] = C("413421")
        img.put(lg, c)
    # pillow at the head end (upper right) and its grey side
    pil = polygon([(68, 17), (86, 8), (119, 38), (101, 48)])
    pside = polygon([(101, 48), (119, 38), (119, 58), (101, 68)])
    img.put(pside, C("727272"))
    img.put(band(pside, -1, 0, 2), C("bebebe"))
    img.put(pil, C("ebebeb"))
    img.put(band(pil, 1, 0, 2) | band(pil, 0, 1, 2), C("bebebe"))
    # blanket
    img.put(front | right, C("720000"))
    img.put(band(front | right, -1, 0, 3), C("810707"))
    img.put(top, C("8c1515"))
    inner = shrink(top, 5) & ~shrink(top, 7)
    img.put(inner, C("9c2626"))
    img.put(band(top, -1, 0, 2) | band(top, 0, -1, 2), C("9c2626"))
    # the frame under the blanket
    fr = (polygon([(L[0], L[1] + H), (F[0], F[1] + H), (F[0], F[1] + H + 3),
                   (L[0], L[1] + H + 3)]) |
          polygon([(F[0], F[1] + H), (R[0], R[1] + H), (R[0], R[1] + H + 3),
                   (F[0], F[1] + H + 3)]))
    img.put(fr, C("695433"))
    img.put(band(fr, 1, 0, 1), C("413421"))
    img.put(drop(img.m, 1) & ~img.m, C("413421"))
    return img.a


def shears(seed):
    img = Img()
    # handle: a red-brown spring bow from the blades around to a ring
    a1, s1, _ = stroke([(78, 17), (64, 28), (52, 42), (43, 58), (39, 76), (38, 86)],
                       [7, 9, 10, 10, 9, 8])
    a2, s2, _ = stroke([(46, 94), (62, 92), (80, 84), (96, 72), (111, 57)], [9, 10, 10, 8, 6])
    for a, s in ((a1, s1), (a2, s2)):
        c = np.zeros((S, S, 3), np.float32)
        c[:] = C("8b4336")
        c[(s > 0) & lit(a, 3)] = C("a64f3f")
        c[dark(a, 3)] = C("6f352b")
        img.put(drop(a), C("5c2d1f"))
        img.put(a, c)
    # ring
    rm = disk(94, 30, 14) & ~disk(94, 30, 8)
    c = np.zeros((S, S, 3), np.float32)
    c[:] = C("707070")
    c[(XX - 30) + (YY - 94) < -6] = C("a8a8a8")
    c[(XX - 30) + (YY - 94) > 8] = C("424242")
    img.put(rm, c)
    # blades: two parallel bright strips
    for pts in ([(56, 46), (84, 16), (104, 16), (66, 50)],
                [(78, 60), (112, 26), (112, 46), (86, 70)]):
        b = polygon(pts)
        c = np.zeros((S, S, 3), np.float32)
        c[:] = C("d5d5d5")
        c[band(b, 1, 0, 2) | band(b, 0, 1, 2)] = C("a8a8a8")
        img.put(b, c)
    return img.a


# ---------------------------------------------------------------------------- table

TEXTURES = {
    "item/stick": stick,
    "item/coal": coal,
    "item/charcoal": charcoal,
    "item/iron_ingot": ingot(("d8d8d8", "a8a8a8", "727272", "ffffff", "828282", "5e5e5e",
                              "353535")),
    "item/gold_ingot": ingot(("fdf55f", "fad64a", "dc9613", "fffde0", "e9b115", "b26411",
                              "752802")),
    "item/diamond": diamond,
    "item/iron_nugget": iron_nugget,
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
    "item/lantern": lantern,
    "item/oak_door": oak_door,
    "item/red_bed": red_bed,
    "item/shears": shears,
}
for _tier in TIER:
    TEXTURES[f"item/{_tier}_pickaxe"] = tool_pickaxe(_tier)
    TEXTURES[f"item/{_tier}_axe"] = tool_axe(_tier)
    TEXTURES[f"item/{_tier}_shovel"] = tool_shovel(_tier)
    TEXTURES[f"item/{_tier}_sword"] = tool_sword(_tier)
