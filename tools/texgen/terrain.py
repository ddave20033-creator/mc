"""Terrain textures in the game's own flat style (see BRIEF.md): natural blocks, ores and the
two fluids.

Every texture is a few flat colour areas with clean anti-aliased edges, drawn with `flat`:
rock is low-poly facets (stone, bedrock, obsidian, sandstone top, glowstone, ice), loose
material is packed raised pebbles (cobblestone, gravel), soft ground is a flat colour with a
few raised shapes (dirt, grass, snow, sand ripples, clay). Ores are the stone texture with
clean faceted crystals or nuggets of the mineral on top. The fluids are flat bands from
periodic functions of space and frame index, so they tile and loop.

All blocks tile seamlessly (shapes wrap round the edges); all randomness comes from `seed`.
"""

from __future__ import annotations

import zlib

import numpy as np
from PIL import Image, ImageDraw
from scipy import ndimage

import flat
from flat import SS, Canvas, disk, ellipse, hexc, lerp, shift, tones

S = flat.S
TAU = 2.0 * np.pi
LIGHT = np.array([-0.7071, -0.7071])  # (x, y): light from the top left


def seed_of(path: str) -> int:
    return zlib.crc32(path.encode()) & 0x7FFFFFFF


# ---------------------------------------------------------------------------- helpers


def wpoly(points, size: int = S):
    """A filled polygon of (x, y) texture points that wraps round the tile's edges (drawn
    once per neighbouring tile copy). Follows the sample grid it is given, so it can be
    moved (`Canvas.raised`)."""
    pts = np.asarray(points, np.float32)

    def f(y, x):
        h, w = y.shape
        oy = float(y[0, 0]) - 0.5 / SS
        ox = float(x[0, 0]) - 0.5 / SS
        im = Image.new("L", (w, h), 0)
        d = ImageDraw.Draw(im)
        for ty in (-size, 0, size):
            for tx in (-size, 0, size):
                q = (pts + [tx - ox, ty - oy]) * SS
                if q[:, 0].max() < 0 or q[:, 0].min() > w or q[:, 1].max() < 0 or q[:, 1].min() > h:
                    continue
                d.polygon([tuple(p) for p in q], fill=255)
        return np.array(im) > 127

    return f


def roll_s(a: np.ndarray, dy: float, dx: float) -> np.ndarray:
    """A sample array moved by (dy, dx) texture pixels, wrapping (a tiling canvas)."""
    return np.roll(a, (int(round(dy * SS)), int(round(dx * SS))), (0, 1))


def rounded(mask: np.ndarray, r: float) -> np.ndarray:
    """`mask` (samples, tiling) with its corners rounded by `r` px (a morphological opening)."""
    p = int(r * SS) + 2
    m = np.pad(mask, p, mode="wrap")
    inner = ndimage.distance_transform_edt(m) > r * SS
    back = ndimage.distance_transform_edt(~inner) <= r * SS
    return back[p:-p, p:-p] & mask


def raised_cells(c: Canvas, ids: np.ndarray, inside: np.ndarray, base_of, light_of, dark_of,
                 rim: float = 2.0) -> None:
    """Raised stones from a cell map: each cell's `inside` part in its base colour, a light
    rim where its top-left neighbour is outside it and a dark rim on its bottom right."""
    key = np.where(inside, ids, -1)
    ul = roll_s(key, rim, rim)
    dr = roll_s(key, -rim, -rim)
    c.fill(inside, base_of[ids])
    c.fill(inside & (ul != key), light_of[ids])
    c.fill(inside & (dr != key), dark_of[ids])


def lwin(c: Canvas, cx: float, cy: float, R: float):
    """The wrapped sample window of radius `R` px round (cx, cy): cheap small shapes."""
    iy = np.arange(int(np.floor((cy - R) * SS)), int(np.ceil((cy + R) * SS))) % (c.h * SS)
    ix = np.arange(int(np.floor((cx - R) * SS)), int(np.ceil((cx + R) * SS))) % (c.w * SS)
    return np.ix_(iy, ix)


def lfill(c: Canvas, win, m: np.ndarray, color) -> None:
    sub = c.rgb[win]
    sub[m] = np.asarray(color, np.float32)
    c.rgb[win] = sub
    a = c.alpha[win]
    a[m] = 1.0
    c.alpha[win] = a


def lshape(c: Canvas, cx: float, cy: float, R: float, shape, color) -> None:
    """Fills a small (wrapping) shape that lies within `R` px of (cx, cy)."""
    win = lwin(c, cx, cy, R)
    lfill(c, win, shape(c.y[win], c.x[win]), color)


def lraised(c: Canvas, cx: float, cy: float, R: float, shape, base, light, dark,
            rim: float = 2.0) -> None:
    """`Canvas.raised` for a small (wrapping) shape within `R` px of (cx, cy)."""
    win = lwin(c, cx, cy, R + rim + 1)
    y, x = c.y[win], c.x[win]
    m = shape(y, x)
    lfill(c, win, m, base)
    lfill(c, win, m & ~shape(y - rim, x - rim), light)
    lfill(c, win, m & ~shape(y + rim, x + rim), dark)


def cell_map(seed: int, count: int, jitter: float = 0.9):
    return flat.voronoi(seed, count, S, S, jitter)


def gem(c: Canvas, cx: float, cy: float, r: float, cols, rng, n: int = 6, squash: float = 1.0,
        angle: float = 0.0, table: float = 0.42, gloss=None, outline=None, ow: float = 1.6,
        pointy: float = 0.0) -> None:
    """A low-poly crystal / nugget: a convex polygon cut into flat facets round a raised top
    (the `table`), each facet one of `cols` (dark .. light) by how it faces the light, a
    `gloss` spot on the lit side and an `outline` ring of the mineral's darkest tone."""
    a0 = rng.uniform(0, TAU)
    ang = a0 + np.arange(n) * TAU / n + rng.uniform(-0.25, 0.25, n) * TAU / n
    rad = r * rng.uniform(0.82, 1.08, n)
    if pointy:
        rad = rad * (1 + pointy * np.abs(np.cos(ang - angle)) ** 4)
    ca, sa = np.cos(angle), np.sin(angle)

    def place(u, v):
        v = v * squash
        return cx + u * ca - v * sa, cy + u * sa + v * ca

    vx, vy = place(np.cos(ang) * rad, np.sin(ang) * rad)
    verts = np.stack([vx, vy], 1)
    # the raised top sits towards the light
    apex = np.array([cx, cy]) + LIGHT * r * 0.18
    if outline is not None:
        d = verts - [cx, cy]
        d = d / np.maximum(np.linalg.norm(d, axis=1, keepdims=True), 1e-6)
        c.fill(wpoly(verts + d * ow), outline)
    k = len(cols)
    for i in range(n):
        a, b = verts[i], verts[(i + 1) % n]
        mid = (a + b) * 0.5 - [cx, cy]
        nrm = mid / max(np.linalg.norm(mid), 1e-6)
        lit = float(nrm @ LIGHT)  # -1 .. 1
        lvl = int(np.clip(np.round((lit * 0.5 + 0.5) * (k - 1) * 1.05 - 0.1), 0, k - 1))
        c.fill(wpoly([apex, a, b]), cols[lvl])
    if table > 0:
        top = apex + (verts - apex) * table
        c.fill(wpoly(top), cols[min(k - 1, max(1, (k * 2) // 3))])
    if gloss is not None:
        g = apex + LIGHT * r * 0.42
        s = max(r * 0.2, 1.5)
        c.fill(wpoly([(g[0] - s, g[1]), (g[0], g[1] - s * 1.4), (g[0] + s, g[1]),
                      (g[0], g[1] + s * 0.9)]), gloss)


def periodic(seed: int, waves: int, f: float, kmax: int = 3, tmax: int = 0, kxmax=None):
    """A smooth tiling (and looping over `f` in 0..1) field: a sum of plane waves with whole
    spatial and temporal frequencies. Returns field(y, x) at sample coordinates."""
    r = np.random.default_rng(seed)
    comps = []
    for _ in range(waves):
        ky, kx = 0, 0
        while ky == 0 and kx == 0:
            kx_ = kmax if kxmax is None else kxmax
            ky, kx = int(r.integers(-kmax, kmax + 1)), int(r.integers(-kx_, kx_ + 1))
        kt = int(r.integers(-tmax, tmax + 1)) if tmax else 0
        comps.append((ky, kx, kt, r.uniform(0, TAU), r.uniform(0.5, 1.0)))

    def field(y, x):
        # y, x: a canvas's sample grid (rows of y, columns of x): separable, so cheap
        ys, xs = y[:, 0].astype(np.float64), x[0, :].astype(np.float64)
        out = np.zeros(y.shape, np.float32)
        for ky, kx, kt, ph, amp in comps:
            a = TAU * ky * ys / S
            b = TAU * kx * xs / S + TAU * kt * f + ph
            out += (amp * (np.outer(np.cos(a), np.cos(b)) - np.outer(np.sin(a), np.sin(b)))).astype(np.float32)
        return out

    return field


# ---------------------------------------------------------------------------- stone & ores

STONE_T = [hexc("#7f828b"), hexc("#8a8d95"), hexc("#9699a0"), hexc("#a3a5aa")]
STONE_CREASE = shift(hexc("#8d8f96"), dv=-0.2, ds=0.03)
STONE_SEED = seed_of("block/stone")


def paint_stone(seed):
    """Low-poly plates: about 14 flat-shaded facets with darker creases between them."""
    return stone_canvas().finish(opaque=True)


_STONE: list = []


def stone_canvas() -> Canvas:
    """A fresh canvas holding the stone texture (the ores draw over it)."""
    if not _STONE:
        c = Canvas()
        c.facets(STONE_SEED, 16, STONE_T, crease=STONE_CREASE, crease_w=1.2)
        _STONE.append((c.rgb.copy(), c.alpha.copy()))
    c = Canvas()
    c.rgb[:], c.alpha[:] = _STONE[0]
    return c


def ore(seed: int, clusters: int, per: tuple, size: tuple, cols, outline, gloss, n=6,
        squash=(0.75, 1.0), table=0.42, pointy=0.0, min_dist=46.0) -> np.ndarray:
    """The stone with `clusters` groups of `per` crystals of radius `size` drawn over it."""
    c = stone_canvas()
    r = np.random.default_rng(seed)
    centres = flat.scatter(seed, clusters, min_dist)
    for cx, cy in centres:
        k = int(r.integers(per[0], per[1] + 1))
        base_ang = r.uniform(0, TAU)
        for j in range(k):
            rr = r.uniform(*size) * (1.0 if j == 0 else 0.72)
            off = 0.0 if j == 0 else size[1] * 1.05
            a = base_ang + j * TAU / max(k, 1) + r.uniform(-0.4, 0.4)
            gem(c, cx + np.cos(a) * off, cy + np.sin(a) * off, rr, cols, r, n=n,
                squash=r.uniform(*squash), angle=r.uniform(0, TAU), table=table,
                gloss=gloss, outline=outline, pointy=pointy)
    return c.finish(opaque=True)


COAL_T = [hexc("#1c1c21"), hexc("#2a2a31"), hexc("#3a3a43"), hexc("#4a4a55")]
IRON_T = [hexc("#a87f5f"), hexc("#c99d79"), hexc("#d9b38c"), hexc("#ecd2b4")]
COPPER_T = [hexc("#8f4a24"), hexc("#b15f30"), hexc("#c8743f"), hexc("#e3965e")]
GOLD_T = [hexc("#b8801c"), hexc("#dca22e"), hexc("#f0c043"), hexc("#ffe07a")]
DIAMOND_T = [hexc("#1f9c98"), hexc("#35bfb8"), hexc("#4fd8cf"), hexc("#9ff1ea")]


def paint_coal_ore(seed):
    """Angular black coal lumps, matte, with a faint grey glint."""
    return ore(seed, 4, (2, 4), (10.5, 13.0), COAL_T, hexc("#15151a"), hexc("#7a7a86"), n=5,
               squash=(0.65, 0.95), table=0.0, min_dist=48.0)


def paint_iron_ore(seed):
    """Warm beige-pink faceted iron nuggets."""
    return ore(seed, 4, (2, 3), (11.5, 14.0), IRON_T, hexc("#7d5a44"), hexc("#fff6ea"), n=6)


def paint_copper_ore(seed):
    """Orange copper nuggets, some faces turned teal by patina."""
    c = stone_canvas()
    r = np.random.default_rng(seed)
    pat = [hexc("#2f7566"), hexc("#3f8f7c"), hexc("#4fa38f"), hexc("#7cc8b2")]
    for i, (cx, cy) in enumerate(flat.scatter(seed, 4, 46.0)):
        k = int(r.integers(2, 4))
        a0 = r.uniform(0, TAU)
        for j in range(k):
            rr = r.uniform(10.5, 13.0) * (1.0 if j == 0 else 0.72)
            off = 0.0 if j == 0 else 13.5
            a = a0 + j * TAU / k + r.uniform(-0.4, 0.4)
            cols = pat if (i + j) % 3 == 2 else COPPER_T
            gem(c, cx + np.cos(a) * off, cy + np.sin(a) * off, rr, cols, r, n=6,
                squash=r.uniform(0.75, 1.0), angle=r.uniform(0, TAU),
                gloss=hexc("#fff0dc"), outline=hexc("#5e2c14"))
    return c.finish(opaque=True)


def paint_gold_ore(seed):
    """Bright gold nuggets with a white gloss."""
    return ore(seed, 4, (2, 3), (11.0, 13.5), GOLD_T, hexc("#7a4f10"), hexc("#fffbe8"), n=7)


def paint_diamond_ore(seed):
    """Long pointed cyan crystals with clear cut faces."""
    return ore(seed, 4, (2, 3), (9.5, 11.5), DIAMOND_T, hexc("#0f5e5e"), hexc("#ffffff"), n=6,
               squash=(0.5, 0.6), table=0.38, pointy=0.9)


# ---------------------------------------------------------------------------- cobblestone & gravel

COBBLE_T = tones("#9c9ea5", 4, spread=0.09)
COBBLE_GAP = shift(hexc("#8d8f96"), dv=-0.25, ds=0.04)


def paint_cobblestone(seed):
    """Rounded, raised flat stones packed tightly, dark gaps between them; one lit facet cut
    on some stones."""
    c = Canvas(bg=COBBLE_GAP)
    ids, border = cell_map(seed, 13, 0.8)
    inside = rounded(border > 1.3, 4.0)
    r = np.random.default_rng(seed + 1)
    n = ids.max() + 1
    lvl = r.integers(1, 3, n)  # base tone 1 or 2
    base = np.stack([COBBLE_T[k] for k in lvl])
    light = np.stack([lerp(COBBLE_T[k], COBBLE_T[3], 0.75) for k in lvl])
    dark = np.stack([lerp(COBBLE_T[0], COBBLE_GAP, 0.3) for _ in lvl])
    raised_cells(c, ids, inside, base, light, dark, rim=2.2)
    return c.finish(opaque=True)


GRAVEL_T = tones("#8a8784", 4, spread=0.1)
GRAVEL_WARM = tones("#9a8270", 4, spread=0.1)
GRAVEL_GAP = hexc("#5d5856")


def paint_gravel(seed):
    """Small rounded pebbles of a few greys (and a few warm ones), packed loosely on dark
    grit, each raised with a light top-left rim."""
    c = Canvas(bg=GRAVEL_GAP)
    r = np.random.default_rng(seed)
    pts = flat.scatter(seed, 140, 9.5)
    for i, (x, y) in enumerate(pts):
        warm = r.random() < 0.16
        T = GRAVEL_WARM if warm else GRAVEL_T
        k = int(r.integers(1, 3))
        rad = r.uniform(5.0, 7.5)
        sh = ellipse(y, x, rad * r.uniform(0.7, 0.92), rad, r.uniform(0, np.pi))
        lraised(c, x, y, rad, sh, T[k], lerp(T[k], T[3], 0.8), T[0], rim=1.5)
    return c.finish(opaque=True)


# ---------------------------------------------------------------------------- dirt & grass

DIRT_T = tones("#8a5a3c", 4, spread=0.1)
PEBBLE_T = tones("#9c8a78", 4, spread=0.11)


def dirt_canvas(seed: int, pebbles: int = 6) -> Canvas:
    c = Canvas(bg=DIRT_T[2])
    r = np.random.default_rng(seed)
    # soft flat flecks: a few darker and lighter lumps of earth
    for i, (x, y) in enumerate(flat.scatter(seed + 5, 18, 22.0)):
        col = DIRT_T[1] if i % 3 else DIRT_T[3]
        lshape(c, x, y, 7, ellipse(y, x, r.uniform(2.0, 3.0), r.uniform(3.5, 6.0),
                                   r.uniform(0, np.pi)), col)
    # raised rounded pebbles
    for x, y in flat.scatter(seed + 9, pebbles, 34.0):
        rad = r.uniform(4.2, 6.0)
        sh = ellipse(y, x, rad * r.uniform(0.72, 0.9), rad, r.uniform(0, np.pi))
        lraised(c, x, y, rad, sh, PEBBLE_T[2], PEBBLE_T[3], PEBBLE_T[0], rim=1.5)
    return c


def paint_dirt(seed):
    """Flat warm brown with darker/lighter flat flecks and a few raised round pebbles."""
    return dirt_canvas(seed).finish(opaque=True)


def paint_grass_block_side(seed):
    """The dirt (the grass fringe is the overlay)."""
    return dirt_canvas(seed_of("block/dirt")).finish(opaque=True)


GRASS_T = [hexc("#8e8e8e"), hexc("#9e9e9e"), hexc("#ababab"), hexc("#bcbcbc"), hexc("#cacaca")]


def paint_grass_block_top(seed):
    """Flat light grey (tinted green by the game): a few big soft patches and clusters of
    slim tapered blades lying in a few directions, each with a faint shadow; low contrast."""
    c = Canvas(bg=GRASS_T[2])
    r = np.random.default_rng(seed)
    for i, (x, y) in enumerate(flat.scatter(seed + 3, 6, 44.0)):
        col = lerp(GRASS_T[2], GRASS_T[1] if i % 2 else GRASS_T[3], 0.6)
        a = r.uniform(0, np.pi)
        for k in range(3):
            c.fill(ellipse(y + r.uniform(-7, 7), x + r.uniform(-9, 9), r.uniform(7, 10),
                           r.uniform(11, 15), a + r.uniform(-0.4, 0.4)), col)
    # clusters of slim tapered blades lying in a few directions, each with a faint shadow
    for i, (x, y) in enumerate(flat.scatter(seed, 26, 22.0)):
        a0 = r.uniform(0, np.pi)
        light = GRASS_T[3] if i % 3 else lerp(GRASS_T[3], GRASS_T[4], 0.5)
        for k in range(int(r.integers(4, 7))):
            bx, by = x + r.uniform(-5, 5), y + r.uniform(-5, 5)
            a = a0 + r.uniform(-0.45, 0.45)
            L, w = r.uniform(4.5, 7.0), r.uniform(1.1, 1.5)
            lshape(c, bx, by, L + 3, ellipse(by + 1.0, bx + 0.8, w, L, a),
                   lerp(GRASS_T[1], GRASS_T[2], 0.3))
            lshape(c, bx, by, L + 3, ellipse(by, bx, w, L, a), light)
    return c.finish(opaque=True)


def fringe(seed: int, depth: float, drips: int) -> callable:
    """The wavy fringe hanging from the top edge: a band with rounded scallops and a few
    longer round drips (tiles horizontally)."""
    r = np.random.default_rng(seed)
    parts = [flat.box(-4, -2, depth - 12, S + 2)]
    n = 9
    for i in range(n):
        x = (i + 0.5 + r.uniform(-0.12, 0.12)) * S / n
        rr = r.uniform(7.5, 9.0)
        parts.append(disk(depth - rr + r.uniform(-3.0, 1.0), x, rr))
    for i in sorted(r.choice(n, drips, replace=False)):
        x = (i + 1.0) * S / n + r.uniform(-1.5, 1.5)  # between two scallops
        L = r.uniform(12, 20)
        w = r.uniform(5.5, 7.0)
        parts.append(flat.capsule((x, depth - 10), (x, depth - 6 + L), w, tile=True))
    return flat.union(*parts)


def paint_grass_block_side_overlay(seed):
    """A clean wavy grass fringe on top (rounded scallops and drips), lighter blade tips,
    a darker under-edge; transparent below (cutout)."""
    c = Canvas()
    m = fringe(seed, 34.0, 3)
    c.fill(m, GRASS_T[2])
    M = c.mask(m)
    c.fill(M & ~roll_s(M, -2.4, 0), GRASS_T[0])  # under-edge shadow
    c.fill(flat.box(-4, -2, 3.0, S + 2), GRASS_T[3])  # lit top edge
    return c.finish(cutout=True)


SNOW_T = [hexc("#b4c3d6"), hexc("#c9d6e6"), hexc("#dfe7f0"), hexc("#eef3f7"), hexc("#fafcfd")]


def paint_grass_block_snow(seed):
    """The dirt side with a thick white snow fringe on top, pale blue shading underneath."""
    c = dirt_canvas(seed_of("block/dirt"))
    m = fringe(seed, 38.0, 3)
    c.fill(m, SNOW_T[3])
    M = c.mask(m)
    c.fill(M & ~roll_s(M, -3.4, 0), SNOW_T[1])
    c.fill(M & ~roll_s(M, -1.4, 0), SNOW_T[0])
    c.fill(flat.box(-4, -2, 4.0, S + 2), SNOW_T[4])
    return c.finish(opaque=True)


def paint_snow(seed):
    """Soft wind-blown snow: a few gentle wavy drift crests, each a bright line with a pale
    blue shadow band under it, and a few sparkles."""
    c = Canvas(bg=SNOW_T[3])
    r = np.random.default_rng(seed)
    n = 3
    for i in range(n):
        cy = (i + r.uniform(0.35, 0.65)) * S / n
        k, ph, amp = 1 + i % 2, r.uniform(0, TAU), r.uniform(4.0, 6.0)
        c.fill(ripple(cy + 4.0, amp, k, ph, 6.0), SNOW_T[2])
        c.fill(ripple(cy + 1.8, amp, k, ph, 2.6), SNOW_T[1])
        c.fill(ripple(cy, amp, k, ph, 2.4), SNOW_T[4])
    for x, y in flat.scatter(seed + 1, 9, 26.0):
        s = r.uniform(1.4, 2.1)
        c.fill(wpoly([(x - s, y), (x, y - s * 1.6), (x + s, y), (x, y + s * 1.6)]), hexc("#ffffff"))
    return c.finish(opaque=True)


# ---------------------------------------------------------------------------- sand, sandstone, clay

SAND_T = tones("#e3cf9b", 4, spread=0.06)


def ripple(cy: float, amp: float, k: int, ph: float, w: float):
    """A wavy horizontal band (tiles: whole waves across the tile)."""

    def f(y, x):
        c = cy + amp * np.sin(TAU * k * x / S + ph) + amp * 0.25 * np.sin(TAU * (k + 1) * x / S + ph * 1.7)
        d = (y - c + S / 2) % S - S / 2
        return np.abs(d) <= w * 0.5

    return f


def paint_sand(seed):
    """Wind ripples: wavy lit crests with a soft shadow below, flat in between, a few grains."""
    c = Canvas(bg=SAND_T[2])
    r = np.random.default_rng(seed)
    n = 5
    for i in range(n):
        cy = (i + r.uniform(0.35, 0.65)) * S / n
        k, ph, amp = 1 + i % 2, r.uniform(0, TAU), r.uniform(3.0, 4.5)
        c.fill(ripple(cy + 2.6, amp, k, ph, 3.4), SAND_T[1])
        c.fill(ripple(cy, amp, k, ph, 2.6), SAND_T[3])
    for i, (x, y) in enumerate(flat.scatter(seed + 2, 14, 22.0)):
        c.fill(disk(y, x, r.uniform(1.0, 1.5)), SAND_T[0] if i % 2 else SAND_T[3])
    return c.finish(opaque=True)


SANDSTONE_T = tones("#d9bf86", 4, spread=0.07)


def paint_sandstone(seed):
    """Wavy sediment layers: flat bands of sandstone tones, each with a lit top line and a
    shadow line under it, a few small chipped pits."""
    c = Canvas(bg=SANDSTONE_T[2])
    r = np.random.default_rng(seed)
    bands = [0, 30, 58, 92]
    for i, y0 in enumerate(bands):
        ph, k = r.uniform(0, TAU), int(r.integers(1, 3))
        y1 = bands[(i + 1) % len(bands)] + (S if i == len(bands) - 1 else 0)
        h = y1 - y0
        col = [SANDSTONE_T[2], SANDSTONE_T[3], SANDSTONE_T[2], SANDSTONE_T[1]][i]
        c.fill(ripple(y0 + h / 2, 0, k, ph, h), col)  # straight-ish base band
        c.fill(ripple(y0, 2.0, k, ph, 3.2), SANDSTONE_T[0])
        c.fill(ripple(y0 + 2.6, 2.0, k, ph, 2.2), shift(SANDSTONE_T[3], dv=0.04))
    for x, y in flat.scatter(seed + 3, 7, 30.0):
        rx = r.uniform(2.5, 4.0)
        c.fill(ellipse(y, x, rx * 0.55, rx, 0), SANDSTONE_T[0])
        c.fill(ellipse(y - 0.9, x, rx * 0.4, rx * 0.85, 0), SANDSTONE_T[1])
    return c.finish(opaque=True)


def paint_sandstone_top(seed):
    """Big pale sandstone plates (low contrast facets) with thin light creases."""
    c = Canvas()
    T = [hexc("#cdb079"), hexc("#d6bb83"), hexc("#dfc690"), hexc("#e8d3a2")]
    c.facets(seed, 8, T, crease=hexc("#b89a63"), crease_w=1.4, jitter=0.7)
    return c.finish(opaque=True)


CLAY_T = tones("#9fa6b5", 4, spread=0.06)


def paint_clay(seed):
    """Smooth wet clay, as if smoothed by hand: long flat glossy smears with a soft shadow
    edge under them, and a few small round pores."""
    c = Canvas(bg=CLAY_T[2])
    r = np.random.default_rng(seed)
    for i, (x, y) in enumerate(flat.scatter(seed, 7, 36.0)):
        ry, rx, a = r.uniform(6.0, 8.0), r.uniform(26, 34), r.uniform(-0.2, 0.2)
        lshape(c, x, y, rx + 4, ellipse(y + 2.2, x + 1.2, ry, rx, a), lerp(CLAY_T[2], CLAY_T[1], 0.7))
        lshape(c, x, y, rx + 4, ellipse(y, x, ry, rx, a), lerp(CLAY_T[2], CLAY_T[3], 0.6))
    for x, y in flat.scatter(seed + 2, 6, 34.0):
        rr = r.uniform(1.3, 1.9)
        lshape(c, x, y, rr + 2, disk(y, x, rr), CLAY_T[0])
        lshape(c, x, y, rr + 2, flat.minus(disk(y, x, rr), disk(y - 0.9, x - 0.9, rr)), CLAY_T[1])
    return c.finish(opaque=True)


# ---------------------------------------------------------------------------- ice, bedrock, obsidian, glowstone


def paint_ice(seed):
    """Large clear plates with white cracks between them and two diagonal glints."""
    c = Canvas()
    T = tones("#a9d3f0", 4, spread=0.05)
    c.facets(seed, 7, T, crease=hexc("#e6f4fd"), crease_w=1.4, jitter=0.8)
    for x, y in ((24.0, 92.0), (88.0, 40.0)):
        for w, d in ((4.0, 0.0), (1.8, 7.0)):
            c.fill(flat.tiled(flat.capsule((x + d, y + 14), (x + d + 22, y - 8), w)),
                   hexc("#e1f1fc"))
    return c.finish(opaque=True)


def paint_bedrock(seed):
    """Rough, high-contrast small rock facets, nearly black creases."""
    c = Canvas()
    T = [hexc("#2e2f35"), hexc("#3d3e45"), hexc("#4a4b52"), hexc("#64656e"), hexc("#7b7c86")]
    c.facets(seed, 30, T, crease=hexc("#1c1d21"), crease_w=1.6, tilt=1.4)
    return c.finish(opaque=True)


def paint_obsidian(seed):
    """Glassy deep purple-black facets, a few lit violet ones, faint violet glints."""
    c = Canvas()
    T = [hexc("#1a1328"), hexc("#221932"), hexc("#2a1f3d"), hexc("#35274d"), hexc("#4b3570")]
    ids = c.facets(seed, 18, T, crease=hexc("#120c1d"), crease_w=1.0, tilt=1.2)
    r = np.random.default_rng(seed)
    for x, y in flat.scatter(seed + 4, 5, 40.0):
        L = r.uniform(9, 14)
        a = r.uniform(-0.3, 0.3) - 0.785
        c.fill(flat.capsule((x, y), (x + np.cos(a) * L, y + np.sin(a) * L), 2.2, tile=True),
               hexc("#7b5fb0"))
    return c.finish(opaque=True)


def paint_glowstone(seed):
    """Warm glowing crystal plates: bright facets with deep amber creases, a small bright
    core facet on each plate."""
    c = Canvas()
    T = [hexc("#d18a33"), hexc("#e8a94a"), hexc("#f5c95c"), hexc("#ffe9a3")]
    ids, border = cell_map(seed, 14, 0.9)
    c.facets(seed, 14, T, crease=hexc("#8a5320"), crease_w=2.6)
    # inner bright facet: the cell's core
    c.fill(border > 6.5, hexc("#fff4c9"))
    c.fill((border > 6.5) & (roll_s(border, 1.6, 1.6) <= 6.5), hexc("#ffffff"))
    return c.finish(opaque=True)


# ---------------------------------------------------------------------------- fluids


def paint_water_still(seed):
    """Light grey (the shader tints it blue) with flat lighter and darker wave bands that
    drift and morph; tiles and loops over 32 frames."""
    frames = []
    base, dark, light, top = hexc("#b8b8b8"), hexc("#a6a6a6"), hexc("#cfcfcf"), hexc("#e4e4e4")
    c = Canvas()
    every = np.ones(c.alpha.shape, bool)
    for f in range(32):
        t = f / 32.0
        c.fill(every, base)
        a = periodic(seed, 4, t, kmax=3, tmax=1, kxmax=1)(c.y, c.x)
        b = periodic(seed + 1, 3, t, kmax=4, tmax=1, kxmax=2)(c.y, c.x)
        v = a + 0.6 * b
        c.fill(v < -1.3, dark)
        c.fill(v > 1.2, light)
        c.fill(v > 2.1, top)
        frames.append(c.finish(opaque=True))
    return frames, 2


def paint_lava_still(seed):
    """Molten orange with flat yellow-hot veins and dark crust plates drifting slowly;
    tiles and loops over 32 frames."""
    frames = []
    crust, crust2 = hexc("#8a2a1a"), hexc("#b23d16")
    base, hot, white = hexc("#ff7a1a"), hexc("#ffa82a"), hexc("#ffd04a")
    c = Canvas()
    every = np.ones(c.alpha.shape, bool)
    for f in range(32):
        t = f / 32.0
        c.fill(every, base)
        a = periodic(seed, 4, t, kmax=2, tmax=1)(c.y, c.x)
        b = periodic(seed + 1, 3, t, kmax=3, tmax=1)(c.y, c.x)
        v = a + 0.5 * b
        c.fill(v < -1.25, crust2)
        c.fill(v < -1.9, crust)
        c.fill(np.abs(v - 0.9) < 0.35, hot)
        c.fill(np.abs(v - 0.9) < 0.14, white)
        frames.append(c.finish(opaque=True))
    return frames, 3


TEXTURES = {
    "block/grass_block_top": paint_grass_block_top,
    "block/grass_block_side": paint_grass_block_side,
    "block/grass_block_side_overlay": paint_grass_block_side_overlay,
    "block/grass_block_snow": paint_grass_block_snow,
    "block/dirt": paint_dirt,
    "block/stone": paint_stone,
    "block/snow": paint_snow,
    "block/cobblestone": paint_cobblestone,
    "block/coal_ore": paint_coal_ore,
    "block/iron_ore": paint_iron_ore,
    "block/copper_ore": paint_copper_ore,
    "block/gold_ore": paint_gold_ore,
    "block/diamond_ore": paint_diamond_ore,
    "block/sand": paint_sand,
    "block/gravel": paint_gravel,
    "block/clay": paint_clay,
    "block/ice": paint_ice,
    "block/bedrock": paint_bedrock,
    "block/obsidian": paint_obsidian,
    "block/sandstone": paint_sandstone,
    "block/sandstone_top": paint_sandstone_top,
    "block/glowstone": paint_glowstone,
    "block/water_still": paint_water_still,
    "block/lava_still": paint_lava_still,
}
