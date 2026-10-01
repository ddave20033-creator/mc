"""Item sprites in the game's own flat style (see BRIEF.md).

Every item is a few chunky flat shapes on a transparent 128 x 128 canvas: 2-3 flat tones per
material (a light rim on the top/left, a shade rim on the bottom/right: light from the top
left), one small white-ish gloss spot, and a 3 px outline in a dark tone of the item's own
colour. Alpha is all or nothing (the game extrudes item sprites into 3D from their alpha),
so silhouettes are solid with no thin slivers.

Tools lie on the diagonal: handle from the bottom left, head at the top right. They are laid
out in a local frame along that diagonal (`u` towards the top right, `v` across it towards
the bottom right) and share their shapes across tiers; only the head's material differs.
"""

from __future__ import annotations

import colorsys

import numpy as np
from PIL import Image, ImageDraw
from scipy.ndimage import distance_transform_edt

import flat
from flat import SS, Canvas, hexc, lerp, moved_shape, shift, union

R2 = float(np.sqrt(0.5))
WHITE = np.array([255.0, 253.0, 246.0], np.float32)


# ---------------------------------------------------------------------------- shapes


def bounded(shape, y0, x0, y1, x1):
    """`shape` evaluated only inside its bounding box (fast; the sample grid is regular)."""

    def f(y, x):
        ys, xs = y[:, 0], x[0, :]
        i0, i1 = np.searchsorted(ys, y0 - 1), np.searchsorted(ys, y1 + 1)
        j0, j1 = np.searchsorted(xs, x0 - 1), np.searchsorted(xs, x1 + 1)
        out = np.zeros(y.shape, bool)
        if i1 > i0 and j1 > j0:
            out[i0:i1, j0:j1] = shape(y[i0:i1, j0:j1], x[i0:i1, j0:j1])
        return out

    return f


def D(cy, cx, r):
    return bounded(flat.disk(cy, cx, r, tile=False), cy - r, cx - r, cy + r, cx + r)


def E(cy, cx, ry, rx, ang=0.0):
    m = max(ry, rx)
    return bounded(flat.ellipse(cy, cx, ry, rx, ang, tile=False), cy - m, cx - m, cy + m, cx + m)


def B(y0, x0, y1, x1, r=0.0):
    return bounded(flat.box(y0, x0, y1, x1, r), y0, x0, y1, x1)


def capsule(p0, p1, w):
    h = w * 0.5
    return bounded(flat.capsule(p0, p1, w), min(p0[1], p1[1]) - h, min(p0[0], p1[0]) - h,
                   max(p0[1], p1[1]) + h, max(p0[0], p1[0]) + h)


def P(points):
    """A filled polygon of (x, y) points that follows the sample coordinates (so it can be
    moved with `moved_shape`, unlike `flat.poly`)."""
    pts = [(float(px), float(py)) for px, py in points]

    def f(y, x):
        h, w = y.shape
        oy, ox = float(y[0, 0]) - 0.5 / SS, float(x[0, 0]) - 0.5 / SS
        im = Image.new("L", (w, h), 0)
        ImageDraw.Draw(im).polygon([((px - ox) * SS, (py - oy) * SS) for px, py in pts],
                                   fill=255)
        return np.array(im) > 127

    return f


def RP(points, r):
    """The polygon grown by `r` with round corners (`points` is the inset outline)."""
    inner = P(points)

    def f(y, x):
        m = inner(y, x)
        return distance_transform_edt(~m) <= r * SS

    return f


def inter(*shapes):
    return lambda y, x: np.logical_and.reduce([s(y, x) for s in shapes])


def minus(a, b):
    return lambda y, x: a(y, x) & ~b(y, x)


def half(y0=None, y1=None, x0=None, x1=None):
    """The axis-aligned region y0 <= y < y1, x0 <= x < x1 (open sides when None)."""

    def f(y, x):
        m = np.ones(y.shape, bool)
        if y0 is not None:
            m &= y >= y0
        if y1 is not None:
            m &= y < y1
        if x0 is not None:
            m &= x >= x0
        if x1 is not None:
            m &= x < x1
        return m

    return f


def above_line(p0, p1):
    """The side of the line p0 -> p1 ((x, y) points) on its left (y up is left of +x)."""
    (x0, y0), (x1, y1) = p0, p1

    def f(y, x):
        return (x1 - x0) * (y - y0) - (y1 - y0) * (x - x0) < 0

    return f


def egg(cy, cx, ry, rx, top=0.82):
    """An egg: an ellipse narrower at the top (`top` x the width there)."""
    k = (1.0 - top)

    def f(y, x):
        t = (y - cy) / ry
        w = rx * (1.0 + k * 0.5 * np.clip(t, -1, 1)) + 1e-6
        return ((x - cx) / w) ** 2 + t * t <= 1.0

    return f


# ---------------------------------------------------------------------------- colour


def with_value(rgb, v, ds=0.0):
    r, g, b = (np.asarray(rgb, np.float32) / 255.0).tolist()
    h, s, _ = colorsys.rgb_to_hsv(r, g, b)
    return np.array(colorsys.hsv_to_rgb(h, min(max(s + ds, 0.0), 1.0), v), np.float32) * 255


class Pal:
    """A material's flat tones: base, light (lit rim), shade (shadow rim), dark (deep
    shadow), line (outline) and gloss."""

    def __init__(self, base, lv=0.10, dv=0.13, line_v=0.30, gloss=0.78):
        b = hexc(base) if isinstance(base, str) else np.asarray(base, np.float32)
        self.base = b
        self.light = shift(b, dv=lv, ds=-0.07, dh=0.008)
        self.shade = shift(b, dv=-dv, ds=0.05, dh=-0.012)
        self.dark = shift(b, dv=-2.1 * dv, ds=0.09, dh=-0.02)
        self.line = with_value(b, line_v, ds=0.12)
        self.gloss = lerp(b, WHITE, gloss)


# ---------------------------------------------------------------------------- painting


def new():
    return Canvas(tile=False)


def rims(c: Canvas, shape, light=None, dark=None, rim=2.5, clip=None):
    """Only the lit (top/left) and shaded (bottom/right) rims of `shape`."""
    m = c.mask(shape)
    if clip is not None:
        m &= c.mask(clip)
    if light is not None:
        c.fill(m & ~c.mask(moved_shape(shape, rim, rim)), light)
    if dark is not None:
        c.fill(m & ~c.mask(moved_shape(shape, -rim, -rim)), dark)


def raised(c: Canvas, shape, pal: Pal, rim=2.5, base=None):
    c.fill(shape, pal.base if base is None else base)
    rims(c, shape, pal.light, pal.shade, rim)


def outline(c: Canvas, parts, width=3.0):
    """A `width` px outline round the drawing, each ring sample coloured like the part
    (shape, colour) its nearest drawn sample belongs to (earlier parts win; a None shape
    takes the rest)."""
    m = c.alpha > 0.5
    d, (iy, ix) = distance_transform_edt(~m, return_indices=True)
    ring_ = ~m & (d <= width * SS)
    owner = np.full(m.shape, len(parts) - 1, np.int32)
    for i in reversed(range(len(parts))):
        if parts[i][0] is not None:
            owner[c.mask(parts[i][0]) & m] = i
    cols = np.stack([np.asarray(col, np.float32) for _, col in parts])
    c.rgb[ring_] = cols[owner[iy, ix]][ring_]
    c.alpha[ring_] = 1.0


def done(c: Canvas, line, width=3.0):
    if isinstance(line, list):
        outline(c, line, width)
    else:
        outline(c, [(None, line)], width)
    return c.finish(cutout=True)


# ---------------------------------------------------------------------------- tools

# local frame along the diagonal: u towards the top right, v across towards the bottom right


def W(u, v):
    return (64.0 + (u + v) * R2, 64.0 + (v - u) * R2)


def LP(pts):
    return P([W(u, v) for u, v in pts])


def LRP(pts, r):
    return RP([W(u, v) for u, v in pts], r)


def LC(u0, v0, u1, v1, w):
    return capsule(W(u0, v0), W(u1, v1), w)


def LE(u, v, ru, rv):
    x, y = W(u, v)
    return E(y, x, rv, ru, -np.pi / 4)


def local(fn):
    """A shape given in the tools' local frame: `fn(u, v)` -> bool."""

    def f(y, x):
        dx, dy = x - 64.0, y - 64.0
        return fn((dx - dy) * R2, (dx + dy) * R2)

    return f


def local_disk(c0, r):
    return local(lambda u, v: (u - c0[0]) ** 2 + (v - c0[1]) ** 2 <= r * r)


def pie(apex, r, spread):
    """A fan from `apex` towards -v (the top left), `r` long, +-`spread` wide: an axe blade."""

    def fn(u, v):
        du, dv = u - apex[0], v - apex[1]
        ang = np.abs(np.arctan2(du, -dv))
        return (du * du + dv * dv <= r * r) & (ang <= spread)

    return local(fn)


def sweep(fn, n=48, caps=True):
    """A band along a local curve: `fn(t)` for t in -1..1 gives (u, v, half width)."""
    ts = np.linspace(-1.0, 1.0, n)
    pts = np.array([fn(t)[:2] for t in ts])
    hw = np.array([fn(t)[2] for t in ts])
    tan = np.gradient(pts, axis=0)
    tan /= np.linalg.norm(tan, axis=1, keepdims=True)
    nor = np.stack([-tan[:, 1], tan[:, 0]], 1)
    a = pts + nor * hw[:, None]
    b = pts - nor * hw[:, None]
    shape = LP([tuple(p) for p in np.concatenate([a, b[::-1]])])
    if caps:
        ends = [LC(*pts[0], *pts[0], 2 * hw[0]), LC(*pts[-1], *pts[-1], 2 * hw[-1])]
        shape = union(shape, *ends)
    return shape


TIERS = {
    "wooden": "#b98a55", "stone": "#8d8f96", "copper": "#c8743f",
    "iron": "#c9ccd2", "golden": "#f0c043", "diamond": "#4fd8cf",
}
WOOD = Pal("#8a6038", lv=0.12, dv=0.12, line_v=0.24)


def head_pal(tier) -> Pal:
    if tier == "iron":
        return Pal(TIERS[tier], lv=0.12, dv=0.16, line_v=0.33)
    if tier == "golden":
        return Pal(TIERS[tier], lv=0.06, dv=0.15, line_v=0.36, gloss=0.85)
    if tier == "diamond":
        return Pal(TIERS[tier], lv=0.10, dv=0.17, line_v=0.30)
    if tier == "stone":
        return Pal(TIERS[tier], lv=0.12, dv=0.14, line_v=0.25)
    return Pal(TIERS[tier], lv=0.10, dv=0.14, line_v=0.28)


def paint_handle(c: Canvas, u0, u1, w=12.0):
    shape = LC(u0, 0, u1, 0, w)
    raised(c, shape, WOOD, rim=2.5)
    return shape


def paint_head(c: Canvas, shape, tier, seed, rim=3.0):
    pal = head_pal(tier)
    if tier == "stone":
        c.facets(seed, 14, [pal.shade, pal.base, pal.light], crease=pal.dark,
                 crease_w=1.4, mask=shape, jitter=0.8)
        rims(c, shape, pal.light, pal.shade, rim)
    else:
        raised(c, shape, pal, rim)
    return pal


def gloss(c: Canvas, shape, pal: Pal, clip=None):
    m = c.mask(shape)
    if clip is not None:
        m &= c.mask(clip)
    c.fill(m, pal.gloss)


def tool_pickaxe(tier):
    def fn(seed):
        c = new()
        paint_handle(c, -56, 30)
        head = sweep(lambda t: (35 - 17 * t * t, 47 * t, 10.5 * (1 - 0.55 * t * t)))
        pal = paint_head(c, head, tier, seed)
        # a collar binding the head to the handle
        collar = LRP([(22, -8), (40, -8), (40, 8), (22, 8)], 2.5)
        c.fill(collar, pal.shade)
        rims(c, collar, pal.base, pal.dark, 2.0)
        gloss(c, LE(32, -27, 3.4, 9), pal)
        return done(c, [(union(head, collar), pal.line), (None, WOOD.line)])

    return fn


def tool_axe(tier):
    def fn(seed):
        c = new()
        paint_handle(c, -56, 20)
        uc, reach = 4.0, 42.0

        def blade_fn(u, v):
            t = np.clip((-v - 9.0) / 30.0, 0.0, 1.0)
            hw = 7.0 + 15.0 * t ** 1.6
            return (v < -2.0) & (np.abs(u - uc) <= hw) & ((u - uc) ** 2 + (v + 1) ** 2 <= reach ** 2)

        blade = local(blade_fn)
        eye = LRP([(-3, -5), (11, -5), (10, 9), (-2, 9)], 4.0)
        head = union(blade, eye)
        pal = paint_head(c, head, tier, seed)
        # the sharpened edge: a lighter band along the round cutting edge
        c.fill(minus(blade, local_disk((uc, -1.0), reach - 7)), pal.light)
        rims(c, head, None, pal.shade, 3.0)
        gloss(c, LE(15, -27, 2.8, 7), pal)
        return done(c, [(head, pal.line), (None, WOOD.line)])

    return fn


def tool_shovel(tier):
    def fn(seed):
        c = new()
        paint_handle(c, -56, 18)
        scoop = union(LRP([(22, -13), (42, -13), (42, 13), (22, 13)], 6.0),
                      LE(42, 0, 18, 19))
        neck = LC(6, 0, 22, 0, 12)
        pal = paint_head(c, scoop, tier, seed)
        # the scoop's fold: the half towards the light is lit, a crease down the middle
        if tier != "stone":
            c.fill(inter(scoop, lambda y, x: (x - 64) + (y - 64) < 0), pal.light)
            c.fill(inter(scoop, LC(22, 0, 54, 0, 4.0)), pal.shade)
            rims(c, scoop, None, pal.shade, 3.0)
        c.fill(neck, pal.shade)
        rims(c, neck, pal.base, pal.dark, 2.0)
        gloss(c, LE(38, -11, 7, 3.0), pal)
        return done(c, [(union(scoop, neck), pal.line), (None, WOOD.line)])

    return fn


def tool_sword(tier):
    def fn(seed):
        c = new()
        pal = head_pal(tier)
        paint_handle(c, -50, -24, 11.0)
        blade = LRP([(-22, -8), (36, -8), (54, 0), (36, 8), (-22, 8)], 2.0)
        paint_head(c, blade, tier, seed, rim=2.5)
        if tier != "stone":
            c.fill(inter(blade, lambda y, x: (x + y) < 128), pal.light)  # lit half
            rims(c, blade, None, pal.shade, 2.5)
        guard = LRP([(-29, -19), (-22, -19), (-22, 19), (-29, 19)], 3.5)
        pommel = LE(-54, 0, 8, 8)
        for s in (guard, pommel):
            c.fill(s, pal.shade)
            rims(c, s, pal.base, pal.dark, 2.0)
        gloss(c, LE(10, -3, 9, 1.8), pal)
        return done(c, [(union(blade, guard, pommel), pal.line), (None, WOOD.line)])

    return fn


# ---------------------------------------------------------------------------- materials


def stick(seed):
    c = new()
    s = union(capsule((24, 106), (104, 24), 12), capsule((58, 72), (44, 50), 9))
    raised(c, s, WOOD, 2.5)
    gloss(c, capsule((70, 54), (82, 42), 3.2), WOOD)
    return done(c, WOOD.line)


def coal_like(base, crease, seed_off, shape, line_v):
    def fn(seed):
        c = new()
        pal = Pal(base, lv=0.12, dv=0.08, line_v=line_v)
        c.facets(seed + seed_off, 9, [pal.shade, pal.base, pal.light], crease=crease,
                 crease_w=1.6, mask=shape, jitter=0.9)
        rims(c, shape, pal.light, pal.dark, 3.0)
        gloss(c, E(44, 48, 4, 7, -0.5), pal)
        return done(c, pal.line)

    return fn


def charcoal(seed):
    """A burnt chunk of wood: a short log piece with its ringed end showing."""
    c = new()
    pal = Pal("#4a3a30", lv=0.10, dv=0.08, line_v=0.13)
    body = capsule((36, 88), (82, 42), 46)
    raised(c, body, pal, 3.0)
    # cracks across the bark
    for a, b in [((40, 70), (58, 88)), ((56, 52), (72, 68))]:
        c.fill(inter(body, capsule(a, b, 3.5)), pal.dark)
    end = E(42, 82, 23, 16, -np.pi / 4)
    c.fill(end, shift(pal.base, dv=0.12, ds=-0.05))
    c.fill(inter(end, minus(E(42, 82, 15, 10, -np.pi / 4), E(42, 82, 11, 6.5, -np.pi / 4))),
           pal.shade)
    c.fill(E(42, 82, 4, 3, -np.pi / 4), pal.shade)
    rims(c, end, None, pal.dark, 2.0)
    gloss(c, E(30, 70, 2.6, 6, -np.pi / 4), pal)
    return done(c, pal.line)


def ingot(base, lv, dv, line_v, gloss_=0.8):
    def fn(seed):
        c = new()
        pal = Pal(base, lv=lv, dv=dv, line_v=line_v, gloss=gloss_)
        front = RP([(26, 66), (86, 66), (94, 94), (16, 94)], 2.0)
        top = RP([(26, 66), (86, 66), (104, 42), (46, 42)], 2.0)
        end = RP([(86, 66), (104, 42), (112, 70), (94, 94)], 2.0)
        c.fill(front, pal.base)
        c.fill(end, pal.shade)
        c.fill(top, pal.light)
        rims(c, front, None, pal.shade, 3.0)
        c.fill(capsule((44, 58), (54, 50), 4), pal.gloss)
        c.fill(capsule((60, 58), (64, 54), 4), pal.gloss)
        return done(c, pal.line)

    return fn


def brick(seed):
    c = new()
    pal = Pal("#b5553a", lv=0.10, dv=0.12, line_v=0.28, gloss=0.55)
    front = RP([(20, 54), (88, 54), (88, 94), (20, 94)], 3.0)
    top = RP([(20, 54), (88, 54), (108, 34), (40, 34)], 3.0)
    side = RP([(88, 54), (108, 34), (108, 74), (88, 94)], 3.0)
    c.fill(front, pal.base)
    c.fill(side, pal.shade)
    c.fill(top, pal.light)
    # a pressed frog (dent) on the top and a couple of firing marks
    c.fill(P([(44, 46), (74, 46), (86, 40), (56, 40)]), pal.base)
    c.fill(D(72, 38, 3.5), pal.shade)
    c.fill(D(82, 66, 3.0), pal.shade)
    c.fill(capsule((40, 40), (52, 40), 3.5), pal.gloss)
    return done(c, pal.line)


def diamond(seed):
    c = new()
    pal = Pal("#4fd8cf", lv=0.10, dv=0.16, line_v=0.30)
    a, b = (40, 26), (88, 26)
    g0, g1, g2, g3 = (14, 50), (46, 50), (82, 50), (114, 50)
    t = (64, 114)
    faces = [
        ([g0, a, g1], pal.light), ([a, b, g2, g1], lerp(pal.light, WHITE, 0.35)),
        ([b, g3, g2], pal.base), ([g0, g1, t], pal.base), ([g1, g2, t], pal.light),
        ([g2, g3, t], pal.shade),
    ]
    for pts, col in faces:
        c.fill(RP(pts, 0.6), col)
    rims(c, union(*[P(p) for p, _ in faces]), None, pal.dark, 2.5)
    c.fill(P([(44, 32), (52, 32), (40, 46), (32, 46)]), WHITE)
    return done(c, pal.line)


def iron_nugget(seed):
    c = new()
    pal = Pal("#c9ccd2", lv=0.12, dv=0.16, line_v=0.33)
    for cy, cx, r in [(82, 84, 17), (78, 42, 18), (50, 64, 21)]:
        raised(c, D(cy, cx, r), pal, 3.0)
    c.fill(E(42, 56, 3, 6, -0.5), pal.gloss)
    return done(c, pal.line)


def clay_ball(seed):
    c = new()
    pal = Pal("#a3aab8", lv=0.10, dv=0.12, line_v=0.32)
    ball = E(66, 64, 44, 47)
    raised(c, ball, pal, 4.0)
    for cy, cx, ry, rx in [(74, 50, 7, 9), (60, 84, 6, 8)]:
        c.fill(E(cy, cx, ry, rx, -0.3), pal.shade)
        c.fill(E(cy - 1.6, cx - 1.6, ry - 2.2, rx - 2.2, -0.3), pal.base)
    c.fill(E(40, 44, 5, 9, -0.5), pal.gloss)
    return done(c, pal.line)


# ---------------------------------------------------------------------------- containers

BUCKET = Pal("#b8bec8", lv=0.10, dv=0.15, line_v=0.30)
BUCKET_TOP = 44.0


def bucket_rim():
    return E(BUCKET_TOP, 64, 10, 44)


def bucket_opening():
    return E(BUCKET_TOP, 64, 6.5, 38)


def bucket(content=None):
    def fn(seed):
        c = new()
        pal = BUCKET
        top = BUCKET_TOP
        handle = inter(minus(E(top + 2, 64, 34, 46), E(top + 2, 64, 28.5, 40.5)),
                       half(y1=top))
        c.fill(handle, pal.shade)
        rims(c, handle, pal.base, pal.dark, 2.0)
        body = RP([(30, top), (98, top), (88, 106), (40, 106)], 6.0)
        c.fill(body, pal.base)
        c.fill(inter(body, P([(0, 0), (46, 0), (54, 128), (0, 128)])), pal.light)
        c.fill(inter(body, P([(84, 0), (128, 0), (128, 128), (76, 128)])), pal.shade)
        band = inter(body, half(y0=76, y1=83))
        c.fill(band, pal.dark)
        rims(c, body, None, pal.dark, 3.0)
        c.fill(bucket_rim(), pal.light)
        rims(c, bucket_rim(), None, pal.shade, 2.0)
        opening = bucket_opening()
        if content is None:
            c.fill(opening, pal.dark)
            c.fill(inter(opening, minus(opening, moved_shape(opening, 3.5, 0))),
                   shift(pal.dark, dv=-0.12))
        elif content == "water":
            w = Pal("#3f7fe0", lv=0.12, dv=0.14)
            c.fill(opening, w.base)
            c.fill(inter(opening, E(top + 3, 70, 3.5, 22)), w.shade)
            c.fill(inter(opening, capsule((44, top - 1.5), (62, top - 1.5), 3.2)), w.gloss)
        else:
            lv = Pal("#f07a1e", lv=0.08, dv=0.16)
            c.fill(opening, lv.base)
            c.fill(inter(opening, E(top + 2.5, 76, 3.5, 14)), hexc("#c23a12"))
            c.fill(inter(opening, E(top - 1, 52, 3, 10)), hexc("#ffd34a"))
            c.fill(inter(opening, D(top + 1, 92, 2.8)), hexc("#ffd34a"))
        c.fill(capsule((40, 58), (44, 92), 4.5), pal.gloss)
        return done(c, pal.line)

    return fn


GLASS = Pal("#d4ecf2", lv=0.06, dv=0.12, line_v=0.38, gloss=0.9)
BOTTLE_C = (82.0, 64.0, 31.0)  # cy, cx, r of the round body


def potion_liquid():
    cy, cx, r = BOTTLE_C
    return inter(D(cy, cx, r - 5.0), half(y0=cy - 16.0))


def bottle(seed):
    c = new()
    cy, cx, r = BOTTLE_C
    body = union(D(cy, cx, r), B(26, 51, 60, 77, 3))
    lip = B(16, 46, 28, 82, 4)
    c.fill(body, GLASS.base)
    c.fill(inter(body, half(x1=56)), GLASS.light)
    rims(c, body, None, GLASS.shade, 4.0)
    c.fill(potion_liquid(), shift(GLASS.base, dv=-0.04))
    c.fill(lip, GLASS.shade)
    rims(c, lip, GLASS.base, GLASS.dark, 2.0)
    c.fill(capsule((47, 70), (42, 86), 4.2), WHITE)
    c.fill(D(62, 51, 2.4), WHITE)
    return done(c, GLASS.line)


def potion_overlay(seed):
    c = new()
    cy, cx, r = BOTTLE_C
    liq = potion_liquid()
    c.fill(liq, np.full(3, 212.0))
    c.fill(inter(liq, half(y1=cy - 8.0)), np.full(3, 236.0))
    rims(c, liq, None, np.full(3, 170.0), 3.5)
    c.fill(D(cy + 6, cx - 12, 3.0), np.full(3, 250.0))
    c.fill(D(cy + 13, cx - 4, 2.2), np.full(3, 250.0))
    return c.finish(cutout=True)


# ---------------------------------------------------------------------------- food


def chop_shape():
    return union(E(54, 72, 36, 44, -0.4), E(82, 42, 26, 28))


def porkchop_like(cooked):
    def fn(seed):
        c = new()
        if cooked:
            meat, fat, line_v = Pal("#a8643a", lv=0.10, dv=0.12), Pal("#dca25a"), 0.26
        else:
            meat, fat, line_v = Pal("#ec8e8c", lv=0.07, dv=0.12), Pal("#f6e2cf"), 0.36
        shape = chop_shape()
        c.fill(shape, fat.base)
        rims(c, shape, fat.light, fat.shade, 2.5)
        inner = inter(shape, moved_shape(shape, 9, -8))
        c.fill(inner, meat.base)
        rims(c, shape, None, meat.shade, 3.5, clip=inner)
        if cooked:
            for k in range(3):
                o = k * 16
                c.fill(inter(inner, capsule((46 + o, 96), (78 + o, 44), 5)), meat.shade)
            c.fill(capsule((48, 50), (60, 42), 4), meat.gloss)
        else:
            c.fill(inter(inner, capsule((60, 70), (88, 52), 5)), fat.base)
            c.fill(capsule((48, 50), (60, 42), 4), meat.gloss)
        # the bone
        bone = D(84, 42, 10)
        c.fill(bone, hexc("#f2ead8"))
        rims(c, bone, None, hexc("#cdbf9f"), 2.0)
        c.fill(D(84, 42, 3.6), hexc("#c98f7c") if not cooked else hexc("#8a5a3a"))
        return done(c, with_value(meat.base, line_v, ds=0.15))

    return fn


def mutton_like(cooked):
    def fn(seed):
        c = new()
        if cooked:
            meat, fat, line_v = Pal("#8f4e2c", lv=0.10, dv=0.10), Pal("#c88a48"), 0.24
        else:
            meat, fat, line_v = Pal("#c9484d", lv=0.10, dv=0.12), Pal("#f1dcc4"), 0.30
        bonec = Pal("#efe7d4", lv=0.04, dv=0.12)
        shaft = capsule((44, 84), (26, 102), 11)
        knobs = union(D(99, 22, 8), D(106, 30, 8))
        bone = union(shaft, knobs)
        raised(c, bone, bonec, 2.0)
        leg = E(52, 72, 34, 44, -0.75)
        c.fill(leg, fat.base)
        rims(c, leg, fat.light, None, 3.0)
        meatshape = minus(E(56, 70, 30, 40, -0.75), E(28, 82, 9, 30, -0.75))
        c.fill(inter(leg, meatshape), meat.base)
        rims(c, inter(leg, meatshape), None, meat.shade, 3.5)
        c.fill(inter(leg, capsule((54, 76), (82, 52), 6)), meat.shade)
        if cooked:
            c.fill(inter(leg, capsule((48, 60), (64, 46), 4)), meat.gloss)
            c.fill(inter(leg, capsule((60, 88), (92, 64), 4)), meat.light)
        else:
            c.fill(capsule((64, 32), (80, 30), 4), fat.gloss)
        rims(c, leg, None, meat.dark, 2.5)
        return done(c, [(bone, bonec.line), (None, with_value(meat.base, line_v, ds=0.1))])

    return fn


def cod_like(cooked):
    def fn(seed):
        c = new()
        if cooked:
            back, belly, line_v = Pal("#c47c3c", lv=0.08, dv=0.12), Pal("#ebbd78"), 0.28
        else:
            back, belly, line_v = Pal("#8f8a68", lv=0.10, dv=0.12), Pal("#e2d8bd"), 0.28
        ang = -0.42
        body = E(60, 66, 21, 44, ang)
        tail = RP([(30, 74), (17, 58), (19, 95)], 3.0)
        fin_top = RP([(54, 40), (70, 26), (80, 34)], 2.5)
        fin_low = RP([(52, 82), (64, 92), (66, 80)], 2.0)
        for s in (tail, fin_top, fin_low):
            c.fill(s, back.shade)
            rims(c, s, back.base, back.dark, 2.0)
        c.fill(body, back.base)
        c.fill(inter(body, lambda y, x: (y - 60) * np.cos(ang) - (x - 66) * np.sin(ang) > 5),
               belly.base)
        rims(c, body, back.light, None, 3.0)
        rims(c, body, None, belly.shade, 3.0, clip=lambda y, x: y > 60)
        if cooked:
            for k in range(3):
                o = k * 17
                c.fill(inter(body, capsule((34 + o, 82), (52 + o, 42), 5)), back.shade)
        else:
            for cy, cx in [(52, 52), (46, 72), (60, 64)]:
                c.fill(D(cy, cx, 3.2), back.shade)
        # gill and eye
        c.fill(inter(body, capsule((86, 36), (90, 58), 3.5)), back.shade)
        c.fill(D(42, 98, 5.5), WHITE if not cooked else hexc("#efe4cc"))
        c.fill(D(42, 99, 3.0), hexc("#24252a") if not cooked else hexc("#6b4a30"))
        c.fill(capsule((60, 44), (74, 38), 3.5), back.gloss)
        return done(c, with_value(back.base, line_v, ds=0.1))

    return fn


# ---------------------------------------------------------------------------- spawn eggs


SPOTS = [(34, 70, 9), (52, 44, 11), (60, 86, 8), (84, 64, 13), (98, 36, 9), (100, 94, 8)]


def spawn_egg(base, spot):
    def fn(seed):
        c = new()
        pal = Pal(base, lv=0.05, dv=0.10)
        sp = Pal(spot, lv=0.06, dv=0.10)
        shape = egg(66, 64, 50, 39)
        c.fill(shape, pal.base)
        for cy, cx, r in SPOTS:
            c.fill(inter(shape, D(cy, cx, r)), sp.base)
        # the round shading: a shadow crescent at the bottom right
        crescent = minus(shape, moved_shape(shape, -6, -5))
        c.fill(crescent, pal.shade)
        for cy, cx, r in SPOTS:
            c.fill(inter(crescent, D(cy, cx, r)), sp.shade)
        c.fill(E(38, 48, 5, 9, -0.7), pal.gloss)
        return done(c, with_value(sp.base, 0.30, ds=0.1))

    return fn


# ---------------------------------------------------------------------------- objects

IRON_DARK = Pal("#4d535c", lv=0.12, dv=0.08, line_v=0.16)


def lantern(seed):
    c = new()
    ring = minus(D(25, 64, 10), D(25, 64, 5.0))
    c.fill(ring, IRON_DARK.base)
    rims(c, ring, IRON_DARK.light, IRON_DARK.dark, 2.0)
    glow = Pal("#ffc24a", lv=0.08, dv=0.10, gloss=0.6)
    body = B(44, 38, 100, 90, 3)
    c.fill(body, glow.base)
    c.fill(E(72, 64, 18, 12), lerp(glow.base, WHITE, 0.55))
    c.fill(E(76, 64, 9, 6), WHITE)
    for x0 in (34, 82):
        bar = B(42, x0, 102, x0 + 12, 3)
        raised(c, bar, IRON_DARK, 2.0)
    cap = RP([(44, 40), (84, 40), (90, 46), (38, 46)], 3)
    raised(c, cap, IRON_DARK, 2.0)
    knob = B(30, 54, 40, 74, 3)
    raised(c, knob, IRON_DARK, 2.0)
    base = B(100, 30, 112, 98, 4)
    raised(c, base, IRON_DARK, 2.0)
    c.fill(capsule((48, 54), (48, 66), 3.2), lerp(glow.base, WHITE, 0.8))
    return done(c, IRON_DARK.line)


def oak_door(seed):
    c = new()
    wood = Pal("#b4834f", lv=0.08, dv=0.12, line_v=0.26)
    door = B(12, 32, 116, 96, 5)
    c.fill(door, wood.base)
    for x in (52, 74):
        c.fill(inter(door, B(14, x, 116, x + 3)), wood.shade)
    c.fill(inter(door, B(62, 32, 70, 96)), wood.shade)
    glass = Pal("#bfe1ec", lv=0.06, dv=0.12)
    for x0 in (40, 68):
        pane = B(22, x0, 52, x0 + 20, 4)
        c.fill(pane, glass.base)
        rims(c, pane, glass.shade, glass.light, 2.5)
        c.fill(capsule((x0 + 6, 30), (x0 + 6, 38), 3.2), WHITE)
    rims(c, door, wood.light, wood.dark, 3.0)
    for y0 in (24, 94):
        hinge = B(y0, 28, y0 + 9, 42, 2.5)
        raised(c, hinge, IRON_DARK, 2.0)
    knob = D(82, 84, 4.8)
    c.fill(knob, hexc("#f0c043"))
    rims(c, knob, None, hexc("#b9852a"), 1.8)
    return done(c, [(B(20, 30, 104, 44), IRON_DARK.line), (None, wood.line)])


def red_bed(seed):
    c = new()
    wood = Pal("#a06a3c", lv=0.10, dv=0.12, line_v=0.24)
    red = Pal("#cf3b3b", lv=0.10, dv=0.12, line_v=0.27)
    pillow = Pal("#f2efe8", lv=0.03, dv=0.10)
    for x0 in (16, 100):
        leg = B(84, x0, 110, x0 + 12, 2.5)
        raised(c, leg, wood, 2.0)
    head = B(28, 12, 100, 26, 4)
    raised(c, head, wood, 2.5)
    frame = B(74, 14, 92, 114, 3)
    raised(c, frame, wood, 2.5)
    pil = B(46, 24, 72, 52, 8)
    raised(c, pil, pillow, 3.0)
    blanket = RP([(54, 52), (110, 52), (110, 78), (50, 78)], 4)
    raised(c, blanket, red, 3.0)
    c.fill(B(48, 48, 82, 60, 4), red.light)  # the folded-back top
    rims(c, B(48, 48, 82, 60, 4), None, red.shade, 2.5)
    c.fill(capsule((70, 54), (98, 54), 3.5), red.gloss)
    return done(c, [(union(blanket, B(48, 48, 82, 60, 4)), red.line),
                    (pil, with_value(pillow.base, 0.45)), (None, wood.line)])


def shears(seed):
    c = new()
    metal = Pal("#c9ccd2", lv=0.12, dv=0.16, line_v=0.33)
    grip = Pal("#c0483a", lv=0.10, dv=0.12, line_v=0.28)
    pivot = (60, 64)
    blades = [
        RP([(56, 62), (66, 56), (108, 18), (90, 26)], 3.0),
        RP([(60, 70), (66, 60), (110, 34), (100, 46)], 3.0),
    ]
    for b in blades:
        raised(c, b, metal, 2.5)
    loops = [((27, 90), (52, 70)), ((46, 101), (62, 76))]
    handles = []
    for (lx, ly), (ax, ay) in loops:
        loop = minus(D(ly, lx, 14), D(ly, lx, 7.0))
        arm = capsule((lx, ly), (ax, ay), 11)
        h = union(minus(arm, D(ly, lx, 7.0)), loop)
        handles.append(h)
        raised(c, h, grip, 2.5)
    c.fill(D(pivot[1], pivot[0], 5), IRON_DARK.base)
    c.fill(D(pivot[1] - 1, pivot[0] - 1, 2.0), IRON_DARK.light)
    c.fill(capsule((76, 44), (92, 30), 3.0), metal.gloss)
    return done(c, [(union(*handles), grip.line), (None, metal.line)])


def bone(seed):
    c = new()
    pal = Pal("#ece4cf", lv=0.03, dv=0.12, line_v=0.38)
    shape = union(
        capsule((36, 92), (92, 36), 14),
        D(98, 22, 10), D(106, 30, 10), D(22, 98, 10), D(30, 106, 10),
    )
    raised(c, shape, pal, 3.0)
    c.fill(capsule((48, 72), (68, 52), 3.2), pal.gloss)
    return done(c, pal.line)


# ---------------------------------------------------------------------------- table


TEXTURES = {
    "item/stick": stick,
    "item/coal": coal_like("#4a4c56", hexc("#2a2b31"), 3,
                           RP([(36, 40), (72, 26), (100, 48), (96, 86), (64, 102), (30, 86)],
                              6), 0.12),
    "item/charcoal": charcoal,
    "item/iron_ingot": ingot("#c9ccd2", 0.12, 0.16, 0.33),
    "item/gold_ingot": ingot("#f0c043", 0.06, 0.15, 0.36, 0.85),
    "item/copper_ingot": ingot("#c8743f", 0.10, 0.14, 0.28),
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
    "item/porkchop": porkchop_like(False),
    "item/cooked_porkchop": porkchop_like(True),
    "item/mutton": mutton_like(False),
    "item/cooked_mutton": mutton_like(True),
    "item/pig_spawn_egg": spawn_egg("#f0a0a0", "#d9707a"),
    "item/sheep_spawn_egg": spawn_egg("#eeeae2", "#c9b9a5"),
    "item/wolf_spawn_egg": spawn_egg("#b9b4ad", "#7d756c"),
    "item/lantern": lantern,
    "item/oak_door": oak_door,
    "item/red_bed": red_bed,
    "item/shears": shears,
    "item/bone": bone,
    "item/cod": cod_like(False),
    "item/cooked_cod": cod_like(True),
}
for _tier in TIERS:
    TEXTURES[f"item/{_tier}_pickaxe"] = tool_pickaxe(_tier)
    TEXTURES[f"item/{_tier}_axe"] = tool_axe(_tier)
    TEXTURES[f"item/{_tier}_shovel"] = tool_shovel(_tier)
    TEXTURES[f"item/{_tier}_sword"] = tool_sword(_tier)
