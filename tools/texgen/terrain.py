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


# ---------------------------------------------------------------------------- old flora
# Old flora painters, still imported by flora.py until it is rewritten (the old pixel-art
# helpers and painters; to be deleted at merge, not used by the terrain painters above).

from common import fbm, grid, noise, pix, rgba, rng  # noqa: E402

YY, XX = grid()


# ---------------------------------------------------------------------------- helpers




def anoise(seed: int, cy: float, cx: float, h: int = S, w: int = S) -> np.ndarray:
    """Tiling value noise with different feature sizes along y and x (streaks)."""
    gh = max(1, int(round(h / cy)))
    gw = max(1, int(round(w / cx)))
    g = rng(seed).random((gh + 1, gw + 1)).astype(np.float32)
    g[gh, :] = g[0, :]
    g[:, gw] = g[:, 0]
    ys = np.arange(h, dtype=np.float32) * gh / h
    xs = np.arange(w, dtype=np.float32) * gw / w
    y0 = np.floor(ys).astype(int)
    x0 = np.floor(xs).astype(int)
    fy = ys - y0
    fx = xs - x0
    fy = fy * fy * (3 - 2 * fy)
    fx = fx * fx * (3 - 2 * fx)
    a = g[y0][:, x0]
    b = g[y0][:, x0 + 1]
    c = g[y0 + 1][:, x0]
    d = g[y0 + 1][:, x0 + 1]
    top = a + (b - a) * fx[None, :]
    bot = c + (d - c) * fx[None, :]
    return top + (bot - top) * fy[:, None]


def streaks(seed: int, cy: float, cx: float, octaves: int = 3) -> np.ndarray:
    """Fractal anisotropic noise (0..1-ish)."""
    tot, amp, norm = np.zeros((S, S), np.float32), 1.0, 0.0
    for o in range(octaves):
        tot += anoise(seed * 7 + o, max(1.0, cy / 2 ** o), max(1.0, cx / 2 ** o)) * amp
        norm += amp
        amp *= 0.5
    return tot / norm


def shear(a: np.ndarray, k: float) -> np.ndarray:
    """Row y rolled by round(k*y) pixels: slanted features that still tile when k*S is whole."""
    out = np.empty_like(a)
    for y in range(a.shape[0]):
        out[y] = np.roll(a[y], int(round(k * y)))
    return out


def pal(*colors: str) -> np.ndarray:
    return np.stack([hexc(c) for c in colors])


def levels(field: np.ndarray, colors: np.ndarray, fracs, mask=None) -> np.ndarray:
    """Palette index per pixel by quantiles of `field`: the darkest color gets the lowest
    fracs[0] share of the pixels, and so on (flat colors, no dithering)."""
    f = field if mask is None else field[mask]
    cum = np.cumsum(fracs)[:-1] / float(np.sum(fracs))
    th = np.quantile(f, cum)
    return np.searchsorted(th, field)


def bands(t: np.ndarray, thresholds) -> np.ndarray:
    """Palette index by fixed thresholds on t."""
    return np.searchsorted(np.asarray(thresholds, np.float32), t)


def wdelta(cy: float, cx: float):
    """Wrapped offsets (dy, dx) of every pixel from a point."""
    dy = (YY - cy + S / 2) % S - S / 2
    dx = (XX - cx + S / 2) % S - S / 2
    return dy, dx


def wgrow(mask: np.ndarray, r: int) -> np.ndarray:
    if r <= 0:
        return mask.astype(bool)
    p = np.pad(mask.astype(bool), r, mode="wrap")
    g = ndimage.binary_dilation(p, np.ones((2 * r + 1, 2 * r + 1), bool))
    return g[r:-r, r:-r]


def q(a: np.ndarray, ky: int = 2, kx: int | None = None) -> np.ndarray:
    """Snaps a field to ky x kx pixel clusters (pixel-art grain)."""
    kx = kx or ky
    h, w = a.shape[:2]
    return np.repeat(np.repeat(a[::ky, ::kx], ky, 0), kx, 1)[:h, :w]


def spread(seed: int, n: int, cand: int = 12):
    """n points (x, y) spread evenly over the wrapping tile (best-candidate sampling)."""
    r = rng(seed)
    pts = [(r.random() * S, r.random() * S)]
    while len(pts) < n:
        best, bd = None, -1.0
        P = np.array(pts)
        for _ in range(cand):
            c = (r.random() * S, r.random() * S)
            dx = np.abs(P[:, 0] - c[0])
            dy = np.abs(P[:, 1] - c[1])
            d = np.min(np.hypot(np.minimum(dx, S - dx), np.minimum(dy, S - dy)))
            if d > bd:
                best, bd = c, d
        pts.append(best)
    return pts


def stroke_mask(x0, y0, x1, y1, w):
    """A short straight stroke (round ends, width w) as (index window, mask) on the wrapped
    tile - cheap for many small strokes."""
    R = int(np.ceil(max(abs(x1 - x0), abs(y1 - y0)) + w)) + 1
    idx, dy, dx = window(y0, x0, R)
    vx, vy = x1 - x0, y1 - y0
    L2 = max(vx * vx + vy * vy, 1e-6)
    t = np.clip((dx * vx + dy * vy) / L2, 0, 1)
    d = np.hypot(dx - t * vx, dy - t * vy)
    return idx, d <= w / 2, t


def window(cy: float, cx: float, R: int):
    """Wrapped index window around a point: (index, dy, dx) for stamping small shapes."""
    iy = np.arange(int(np.floor(cy)) - R, int(np.floor(cy)) + R + 1)
    ix = np.arange(int(np.floor(cx)) - R, int(np.floor(cx)) + R + 1)
    dy = (iy + 0.5 - cy)[:, None] * np.ones((1, len(ix)), np.float32)
    dx = (ix + 0.5 - cx)[None, :] * np.ones((len(iy), 1), np.float32)
    return np.ix_(iy % S, ix % S), dy, dx


def stamp(canvas, idx, mask, val):
    """Writes `val` (scalar or window-shaped array) into canvas[idx] where mask."""
    sub = canvas[idx]
    v = np.broadcast_to(np.asarray(val), sub.shape)
    sub[mask] = v[mask]
    canvas[idx] = sub


def blob(cx, cy, rx, ry, rot=0.0, p=2.0, jitter=0.0, jseed=0, jcell=8):
    """Wrapped superellipse: (normalised radius r, dx, dy). r <= 1 is inside."""
    dy, dx = wdelta(cy, cx)
    c, s = np.cos(rot), np.sin(rot)
    u = dx * c + dy * s
    v = -dx * s + dy * c
    r = (np.abs(u / rx) ** p + np.abs(v / ry) ** p) ** (1.0 / p)
    if jitter:
        r = r + (noise(jseed, jcell) - 0.5) * jitter
    return r, dx, dy


def bumps(seed: int, items, jitter=0.12, want_xy=False):
    """Height field of wrapped domes (cx, cy, rx, ry[, h]) - the highest wins - plus the id of
    the dome on top, its light term (+ lit from the top left) and normalised radius R. With
    `want_xy` also the normalised offsets (NX, NY) inside the top dome."""
    NX = np.zeros((S, S), np.float32)
    NY = np.zeros((S, S), np.float32)
    H = np.full((S, S), -1.0, np.float32)
    ids = np.full((S, S), -1, np.int32)
    L = np.zeros((S, S), np.float32)
    R = np.ones((S, S), np.float32) * 9
    for i, it in enumerate(items):
        cx, cy, rx, ry = it[:4]
        hgt = it[4] if len(it) > 4 else 1.0
        r, dx, dy = blob(cx, cy, rx, ry, jitter=jitter, jseed=seed + i, jcell=8)
        h = (1 - np.clip(r, 0, 1.5) ** 2) * hgt
        top = (r <= 1) & (h > H)
        H = np.where(top, h, H)
        ids = np.where(top, i, ids)
        L = np.where(top, -(dx / rx + dy / ry) * 0.5, L)
        R = np.where(top, r, R)
        if want_xy:
            NX = np.where(top, dx / rx, NX)
            NY = np.where(top, dy / ry, NY)
    if want_xy:
        return H, ids, L, R, NX, NY
    return H, ids, L, R


def out_rgb(colors: np.ndarray, idx: np.ndarray) -> np.ndarray:
    return colors[np.clip(idx, 0, len(colors) - 1)]


def opaque_rgba(rgb: np.ndarray) -> np.ndarray:
    return rgba(rgb.astype(np.float32))


def bark(seed: int, C: np.ndarray, fracs=(3, 12, 29, 46, 9, 2)) -> np.ndarray:
    """Braided vertical bark: vertical streaks bent into zigzags that differ from column to
    column, so dark grooves and light ridges weave. C: 6 colors dark to light."""
    base = anoise(seed, 16, 3.2) * 0.6 + anoise(seed + 1, 8, 2) * 0.4
    ph = anoise(seed + 2, 128, 21.3)[0]  # phase per column
    per = S / 3
    warp = 1.8 * np.sin(TAU * (YY / per + ph[None, :] * 1.2))
    f = ndimage.map_coordinates(base, [YY - 0.5, XX - 0.5 + warp], order=1, mode="grid-wrap")
    f = f + (pix(seed + 4, 1) - 0.5) * 0.08
    return out_rgb(C, levels(f, C, fracs))


def log_top(seed: int, bark_c: np.ndarray, ring_c: np.ndarray, border=8, radii=None,
            bark_rgb=None) -> np.ndarray:
    """Growth rings as rounded squares at irregular spacing inside a bark rim. ring_c: dark
    ring line, its softer edge, base wood, light band, highlight. Each ring is a dark line
    with a softer mid tone on its inner side and a light band just outside it."""
    c = S / 2
    r = rng(seed)
    if radii is None:
        radii = [6, 15, 24, 34, 44]
    ay, ax = np.abs(YY - c), np.abs(XX - c)
    d = (ay ** 12 + ax ** 12) ** (1 / 12)  # rounded square
    d = d + (noise(seed, 32) - 0.5) * 1.6 + (noise(seed + 5, 8) - 0.5) * 0.6
    grain = pix(seed + 2, 1)
    idx = np.full((S, S), 2, np.int32)
    idx = np.where((noise(seed + 1, 8) > 0.62) & (grain > 0.5), 3, idx)
    idx = np.where(grain > 0.97, 1, idx)
    for k, R in enumerate(radii):
        R = R + r.uniform(-0.8, 0.8)
        e = d - R
        w = 0.8 if k % 2 else 1.1
        idx = np.where((e > w) & (e < w + 2.0), np.maximum(idx, 3), idx)
        idx = np.where((e > -w - 1.2) & (e <= -w), np.minimum(idx, 1), idx)
        dark = np.where(noise(seed + 9 + k, 8) > 0.4, 0, 1)
        idx = np.where(np.abs(e) <= w, dark, idx)
    idx = np.where(d < 2.2, 1, idx)
    # Light rim just inside the bark, darker line under it.
    edge = np.maximum(ay, ax)
    idx = np.where((edge >= c - border - 2) & (edge < c - border), 3, idx)
    idx = np.where((edge >= c - border - 2) & (edge < c - border) & (grain > 0.7), 4, idx)
    idx = np.where((edge >= c - border - 4) & (edge < c - border - 2), 1, idx)
    out = out_rgb(ring_c, idx)
    bm = edge >= c - border
    if bark_rgb is None:
        bf = streaks(seed + 3, 6, 6, 2) + (pix(seed + 4, 1) - 0.5) * 0.25
        bark_rgb = out_rgb(bark_c, levels(bf, bark_c, [2, 4, 3]))
    out[bm] = bark_rgb[bm]
    return opaque_rgba(out)


OAK_BARK_C = pal("#382b18", "#4c3d26", "#5f4a2b", "#745a36", "#917142", "#987849")
OAK_RING_C = pal("#967441", "#9f844d", "#af8f55", "#b8945f", "#c29d62")
SPRUCE_BARK_C = pal("#2e1608", "#2e1c0a", "#311e0b", "#3b2713", "#4d3317", "#553a1f")
SPRUCE_RING_C = pal("#5a4424", "#70522e", "#7a5a34", "#82613a", "#886539")


def paint_oak_log(seed):
    return opaque_rgba(bark(seed, OAK_BARK_C))


def paint_oak_log_top(seed):
    return log_top(seed, OAK_BARK_C[1:4], OAK_RING_C, radii=[6, 15, 24, 33, 43])


def paint_spruce_log(seed):
    return opaque_rgba(bark(seed, SPRUCE_BARK_C))


def paint_spruce_log_top(seed):
    return log_top(seed, pal("#443321", "#553a1f", "#3b2713"), SPRUCE_RING_C,
                   radii=[6, 16, 25, 34, 44])


BIRCH_C = pal("#36342a", "#605e54", "#bebeae", "#d8d8ce", "#ded3d5", "#eef1ea", "#f0eeeb",
              "#ffffff")
# Dark lenticel marks: (x, y, length, thickness, tilt) - thick in the middle, tapering to
# points, the right end a little lower (like Faithful's).
BIRCH_MARKS = [(46, 30, 38, 11, 0.18), (120, 42, 40, 11, 0.12), (116, 108, 40, 10, -0.2),
               (12, 118, 30, 10, 0.35)]


def birch_rgb(seed: int) -> np.ndarray:
    # White bark crossed by soft zigzag bands (grey and pinkish), each row's zigzag shifted
    # differently, broken up in places.
    shift = anoise(seed, 42.7, 128)[:, :1] * 128 / 3 + (noise(seed + 1, 32) - 0.5) * 6
    per = 128 / 3
    zig = np.abs(((XX + shift) / per) % 1.0 - 0.5) * 2 * 7  # 0..7 px
    rowf = (YY + zig) / 16.0
    dd = np.abs(rowf - np.round(rowf)) * 16
    gate = noise(seed + 4, 16) * 0.7 + pix(seed + 7, 4) * 0.3 > 0.36
    idx = np.full((S, S), 7, np.int32)
    wf = noise(seed + 2, 16) * 0.7 + pix(seed + 3, 2) * 0.3
    idx = np.where(wf < 0.45, 6, idx)
    idx = np.where((wf > 0.45) & (wf < 0.5), 5, idx)
    tone = np.where(noise(seed + 5, 8) > 0.6, 4, 3)
    idx = np.where((dd < 1.4) & gate, tone, idx)
    idx = np.where((dd >= 1.4) & (dd < 2.6) & gate, 5, idx)
    out = out_rgb(BIRCH_C, idx)
    for k, (cx, cy, L, T, tilt) in enumerate(BIRCH_MARKS):
        dy, dx = wdelta(cy, cx)
        c, s_ = np.cos(tilt), np.sin(tilt)
        u = (dx * c + dy * s_) / (L / 2)
        v = (-dx * s_ + dy * c)
        half = T / 2 * np.clip(1 - u * u, 0, 1) ** 0.9 * (1 + 0.2 * u)
        vv = v + (u * u) * T * 0.3 + (noise(seed + 30 + k, 4) - 0.5) * 1.4  # ends droop
        core = (np.abs(u) < 1) & (np.abs(vv) < half)
        halo = (np.abs(u) < 1.12) & (np.abs(vv) < half + 2.2) & ~core
        rim = core & (np.abs(vv) > half - 1.8) & (vv > 0)
        out[halo & (vv < 0)] = BIRCH_C[2]
        out[halo & (vv >= 0)] = BIRCH_C[3]
        out[core] = BIRCH_C[0]
        out[rim] = BIRCH_C[1]
    return out


def paint_birch_log(seed):
    return opaque_rgba(birch_rgb(seed))


def paint_birch_log_top(seed):
    # Bark rim: white and pale grey, with dark lenticel spots along each side.
    n = noise(seed + 6, 8)
    rim = np.where((n > 0.45)[..., None], hexc("#ffffff"), hexc("#ebebe7"))
    r = rng(seed + 7)
    spots = np.zeros((S, S), bool)
    k = 0
    for side_ in range(4):
        pos = r.uniform(0, 16)
        while pos < S - 4:
            ln, th, dep = r.uniform(5, 11), r.uniform(3.5, 5.0), r.uniform(2.5, 4.5)
            cx, cy = [(pos, dep), (S - dep, pos), (pos, S - dep), (dep, pos)][side_]
            rx, ry = (ln / 2, th / 2) if side_ in (0, 2) else (th / 2, ln / 2)
            rr, _, _ = blob(cx, cy, rx, ry, p=2.2, jitter=0.3, jseed=seed + 60 + k, jcell=4)
            spots |= rr <= 1
            pos += ln + r.uniform(12, 28)
            k += 1
    halo = wgrow(spots, 1) & ~spots
    rim[halo] = hexc("#999682")
    rim[spots] = hexc("#514f47")
    rings = pal("#ae9f76", "#b8a875", "#c8b77a", "#d7c185", "#d7cb8d")
    return log_top(seed, None, rings, bark_rgb=rim, radii=[6, 15, 24, 33.5, 44])


# ---------------------------------------------------------------------------- leaves
# Faithful-style foliage: distinct, fairly large leaf shapes (oak: lobed leaves hanging from
# thin arching twigs; birch: scattered pointed ovals; spruce: little fir sprigs), lit on the
# side facing the top left, with dark veins and bottom-right rims. A darker back layer of
# leaves fills part of the gaps; the rest (about 20 %) are real transparent holes.

LEAF_C = pal("#3e3c3e", "#4f4d4f", "#5f5c5f", "#6d6b6d", "#7c7b7c", "#8b8c8b", "#9a9b9a",
             "#a9aba9", "#b9bcb9", "#c9cbc9")


def wshift(m: np.ndarray, dy: int, dx: int) -> np.ndarray:
    return np.roll(np.roll(m, dy, 0), dx, 1)


def leaf_frame(cx, cy, ang):
    """Wrapped leaf-local coordinates around a base point: u along `ang`, v across."""
    dy, dx = wdelta(cy, cx)
    c, s = np.cos(ang), np.sin(ang)
    return dx * c + dy * s, -dx * s + dy * c


def lit_sign(ang: float) -> float:
    """Which side (sign of v) of a leaf pointing along `ang` faces the top-left light."""
    return 1.0 if (np.sin(ang) - np.cos(ang)) > 0 else -1.0


def leaf_under(cidx: np.ndarray, m: np.ndarray, drop: int = 2, amt: int = 1):
    """Before stamping a leaf mask `m`: a shadow on the leaves below its bottom-right side and
    a dark seam along its top-left edge where it lies over other leaves."""
    below = cidx >= 0
    sh = wshift(m, drop, drop) & ~m & below
    cidx[sh] = np.maximum(cidx[sh] - amt, 1)
    seam = (wshift(m, -1, -1) | wshift(m, -1, 0) | wshift(m, 0, -1)) & ~m & below
    cidx[seam] = np.minimum(cidx[seam], 4)


def rims(m: np.ndarray):
    """(bottom-right edge, top-left edge) pixels of a mask."""
    out_br = m & (~wshift(m, -1, -1) | ~wshift(m, -1, 0) | ~wshift(m, 0, -1))
    out_tl = m & (~wshift(m, 1, 1) | ~wshift(m, 1, 0) | ~wshift(m, 0, 1))
    return out_br, out_tl


def oak_leaf(cidx, cx, cy, ang, L, W, shade, seed, droop=0.22):
    """A lobed oak leaf from its stalk at (cx, cy) along `ang`, drooping a little: a broad lit
    upper part and a mid-grey lower margin with fingered lobes."""
    u, v = leaf_frame(cx, cy, ang)
    f = u / L
    fc = np.clip(f, 0, 1)
    v = v - droop * np.cos(ang) * L * fc * fc  # the leaf's midline sags under gravity
    env = np.sin(np.pi * fc ** 0.75) ** 0.5 * (0.6 + 0.45 * fc)
    nl = 3.3
    wave = np.minimum(1.0, np.abs(np.sin(np.pi * nl * f + 0.35)) * 1.7)
    sg = lit_sign(ang)
    # Rounded lobes with V notches, deeper on the shaded (lower) side.
    lobe = np.where(v * sg > 0, 0.42 + 0.58 * wave, 0.6 + 0.4 * wave)
    half = W * env * lobe
    rag = (pix(seed, 2) - 0.5) * 0.8
    m = (f >= 0.03) & (f <= 1.0) & (np.abs(v) <= half + rag)
    if not m.any():
        return
    leaf_under(cidx, m)
    lit = v * sg < 0.35 * W * env
    t = np.where(lit, 8, 6) + shade
    sp = pix(seed + 1, 1)
    dash = q(pix(seed + 2, 1), 1, 2) if abs(np.cos(ang)) > 0.7 else q(pix(seed + 2, 1), 2, 1)
    t = np.where(lit & (dash > 0.86) & (np.abs(v) > 2), t - 1, t)
    t = np.where(~lit & (sp < 0.1), t + 1, t)
    # Veins: a midrib and faint side veins angled forward into each lobe.
    per = L / nl
    g = (u + 0.7 * np.abs(v) - 0.1 * L) / per
    dv = np.abs(g - np.round(g)) * per / 1.22
    side = (dv < 0.7) & (np.abs(v) > 1.5) & (np.abs(v) < half * 0.75) & (f < 0.9)
    t = np.where(side, t - 1, t)
    t = np.where(side & ~lit, t - 1, t)
    mid = (np.abs(v) < 1.0) & (f < 0.85)
    t = np.where(mid, 5 + shade, t)
    br, tl = rims(m)
    t = np.where(br, t - 2, t)
    t = np.where(tl & ~br & lit, 9 + shade, t)
    cidx[m] = np.clip(t, 0, 9)[m]


def oval_leaf(cidx, cx, cy, ang, L, W, shade, seed, stalk=4.0):
    """A pointed oval (birch) leaf with a short stalk at (cx, cy): a lit half with a pale
    streak beside the midrib, a darker half with a thick dark margin."""
    u, v = leaf_frame(cx, cy, ang)
    uu = u - stalk
    f = uu / L
    fc = np.clip(f, 0, 1)
    half = W * np.sin(np.pi * fc) ** 0.7 * (1.1 - 0.3 * fc)
    m = (f >= 0) & (f <= 1) & (np.abs(v) <= half + (pix(seed, 2) - 0.5) * 0.7)
    st = (u >= 0) & (u < stalk + 1) & (np.abs(v) < 1.1)
    mm = m | st
    if not mm.any():
        return
    leaf_under(cidx, mm)
    sg = lit_sign(ang)
    sv = v * sg  # < 0 on the lit half
    t = np.where(sv < 0.5, 6, 5) + shade
    # Pale streak along the lit side of the midrib, toothed by the side veins.
    per = L / 4.0
    g = (uu - 1.2 * np.abs(v)) / per
    vein = np.abs(g - np.round(g)) * per / 1.6 < 0.6
    streak = (sv < -1.2) & (sv > -half * 0.6) & (f > 0.12) & (f < 0.8) & ~vein
    t = np.where(streak, 8 + shade, t)
    t = np.where(vein & (np.abs(v) > 1.5) & (np.abs(v) < half - 1.5), t - 1, t)
    sp = pix(seed + 1, 1)
    t = np.where((sv < 0) & (sp > 0.92), t + 1, t)
    t = np.where((sv >= 0) & (sp < 0.1), t + 1, t)
    t = np.where((np.abs(v) < 0.9) & (f < 0.9), 4 + shade, t)
    br, tl = rims(m)
    t = np.where((sv > 0) & (sv > half - 2.4), 3 + shade, t)
    t = np.where(br, np.minimum(t, 4 + shade), t)
    t = np.where(tl & ~br & (sv < 0), 7 + shade, t)
    t = np.where(st & ~m, 3 + shade, t)
    cidx[mm] = np.clip(t, 0, 9)[mm]


def wline(cidx, x0, y0, x1, y1, w, val):
    """A wrapped straight stroke of width w painted with index `val`."""
    dy, dx = wdelta(y0, x0)
    vx, vy = x1 - x0, y1 - y0
    L2 = max(vx * vx + vy * vy, 1e-6)
    t = np.clip((dx * vx + dy * vy) / L2, 0, 1)
    d = np.hypot(dx - t * vx, dy - t * vy)
    m = d <= w / 2
    cidx[m] = val
    return m


def leaves_out(cidx: np.ndarray) -> np.ndarray:
    m = cidx >= 0
    out = rgba(LEAF_C[np.clip(cidx, 0, len(LEAF_C) - 1)], np.where(m, 255, 0))
    out[~m, :3] = 0
    return out


def paint_oak_leaves(seed):
    r = rng(seed)
    cidx = np.full((S, S), -1, np.int32)
    # Back layer: darker leaves in the gaps.
    n = 4
    for i in range(n):
        for j in range(n):
            cx = (j + 0.5 * (i % 2) + 0.5 + (r.random() - 0.5) * 0.4) * S / n
            cy = (i + 0.5 + (r.random() - 0.5) * 0.4) * S / n
            ang = r.uniform(-0.2, 1.2) if r.random() < 0.7 else r.uniform(1.9, 3.0)
            oak_leaf(cidx, cx, cy, ang, r.uniform(26, 31), r.uniform(10, 12), -2,
                     seed + 200 + i * n + j)
    # Twigs running across in gentle arcs, the big leaves hang from them down to the right.
    rows, cols = 4, 4
    bases = []
    for i in range(rows):
        row = []
        for j in range(cols):
            x = (j + 0.5 * (i % 2) + r.uniform(-0.12, 0.12)) * S / cols
            y = (i + r.uniform(-0.12, 0.12)) * S / rows + (j % 2) * 5
            row.append((x, y))
        bases.append(row)
    for i in range(rows):
        for j in range(cols):
            (x0, y0), (x1, y1) = bases[i][j], bases[i][(j + 1) % cols]
            if j == cols - 1:
                x1 += S
            xm, ym = (x0 + x1) / 2, (y0 + y1) / 2 - 5
            for a, b in (((x0, y0), (xm, ym)), ((xm, ym), (x1, y1))):
                m = wline(np.zeros((S, S), np.int32), a[0], a[1], b[0], b[1], 2.2, 1) > 0
                cidx[m & (cidx < 0)] = 2
    for i in range(rows):
        for j in range(cols):
            x, y = bases[i][j]
            ang = r.uniform(0.15, 1.0) if r.random() < 0.75 else r.uniform(2.1, 2.9)
            oak_leaf(cidx, x, y, ang, r.uniform(36, 41), r.uniform(14, 16),
                     int(r.integers(-1, 1)), seed + 10 + i * cols + j)
    return leaves_out(cidx)


def paint_birch_leaves(seed):
    r = rng(seed)
    cidx = np.full((S, S), -1, np.int32)
    # Leaves scattered evenly in all directions: a darker back layer, then the front ones.
    pts = spread(seed + 1, 38)
    for k, (cx, cy) in enumerate(pts):
        back = k >= 22
        size = 0.85 if back else 1.0
        L, W = r.uniform(25, 30) * size, r.uniform(9, 10.5) * size
        ang = r.uniform(0, TAU)
        bx, by = cx - np.cos(ang) * (L / 2 + 4), cy - np.sin(ang) * (L / 2 + 4)
        pts[k] = (back, bx, by, ang, L, W)
    for k, (back, bx, by, ang, L, W) in sorted(enumerate(pts), key=lambda e: -e[1][0]):
        oval_leaf(cidx, bx, by, ang, L, W, -2 if back else 0, seed + 300 + k)
    return leaves_out(cidx)


def fir_sprig(cidx, cx, cy, H, Wd, shade, seed):
    """A little fir sprig (Faithful's spruce leaves) hanging from its top (cx, cy): a stem
    with a dark bare tip on top and two stacked tiers of needle branches slanting down and
    out - short near each tier's top, long at its bottom, so every tier is a jagged "^" -
    with forked, dark-tipped ends."""
    dy, dx = wdelta(cy + H / 2, cx)  # anchored mid-sprig: wrapped offsets reach +-64
    dy = dy + H / 2
    ax = np.abs(dx)
    rough = pix(seed, 2)
    slope, spacing = 0.55, 8.0
    th = (H - 8) / 2
    for k in range(2):
        ty = 8 + k * th * 0.95
        ly = dy - ty
        span = Wd * (0.78 + 0.22 * k)
        bl = ly - slope * ax  # 0 on the tier's top branch, growing down
        bmax = 3 * spacing + 4
        lim = span * (0.3 + 0.7 * np.clip(bl / bmax, 0, 1)) + rough * 3
        inside = (bl >= -1) & (bl <= bmax) & (ax <= lim)
        g = bl / spacing
        fr = g - np.floor(g)
        comb = (fr < 0.45 + rough * 0.15) | (ax < 3 + rough * 2)
        # Needles forking off below the outer part of each branch.
        fork = (fr >= 0.45) & (fr < 0.8) & (ax > lim * 0.45) & (((ax + 0.5 * bl) % 5) < 2)
        m = inside & (comb | fork)
        t = np.full((S, S), 5 + shade, np.int32)
        t = np.where((dx < 0) & (fr < 0.22), 6 + shade, t)  # lit upper edge on the left
        t = np.where((fr > 0.4) & (ax >= 3), 4 + shade, t)  # shaded underside and forks
        tips = ax > lim - 3 - rough * 2
        t = np.where(tips, np.where(rough > 0.5, 2, 3) + shade, t)
        cidx[m] = t[m]
    stem = (ax < 1.8) & (dy >= 8) & (dy < H - 8)
    cidx[stem] = np.where(dx < -0.5, 6, 5)[stem] + shade
    top = (ax < 1.8) & (dy >= 0) & (dy < 9)
    cidx[top] = np.where(dx > 0, 0, 1)[top] + shade


def paint_spruce_leaves(seed):
    r = rng(seed)
    cidx = np.full((S, S), -1, np.int32)
    cols, rows = 3, 3
    # Sprigs on a slanted lattice, top rows first so the lower sprigs' dark tips lie over the
    # ones above.
    for layer, shade in ((1, 0),):
        for i in range(rows):
            for j in range(cols):
                ox = -i / 3.0 + (0.0 if layer else 0.5)
                oy = 0.0 if layer else 0.5
                cx = (j + ox + r.uniform(-0.05, 0.05)) * S / cols
                cy = (i + oy + r.uniform(-0.05, 0.05)) * S / rows
                sc = 1.0 if layer else 0.75
                fir_sprig(cidx, cx, cy, r.uniform(54, 58) * sc, r.uniform(26, 28) * sc,
                          shade, seed + 50 + layer * 20 + i * cols + j)
    return leaves_out(cidx)


# ---------------------------------------------------------------------------- cactus

CACTUS_C = pal("#39581a", "#426520", "#527d26", "#5b8c2b", "#649832", "#9fa76d", "#babf95")


def cactus_spines(cidx, seed, n, x0, x1, y0, y1):
    """Small pale spines: little crosses and short diagonal dashes (2 px strokes)."""
    r = rng(seed)
    for k in range(n):
        x = int(r.uniform(x0 + 4, x1 - 8)) // 2 * 2
        y = int(r.uniform(y0 + 4, y1 - 8)) // 2 * 2
        if r.random() < 0.45:  # cross
            cidx[y + 2:y + 4, x:x + 6] = 5
            cidx[y:y + 6, x + 2:x + 4] = 5
            cidx[y + 2:y + 4, x + 2:x + 4] = 6
        else:  # short diagonal dash
            d = 1 if r.random() < 0.5 else -1
            for i in range(3):
                xx = x + (2 * i if d > 0 else 4 - 2 * i)
                cidx[y + 2 * i:y + 2 * i + 2, xx:xx + 2] = 6 if i == 0 else 5


def paint_cactus_side(seed):
    x = XX - 0.5
    t = 0.66 + (anoise(seed, 24, 4) - 0.5) * 0.35 + (pix(seed + 1, 1) - 0.5) * 0.1
    grooves = [8, 45, 83, 120]
    dist = np.min([np.abs(x - g) for g in grooves], axis=0)
    # Each rib: rounded, lit left of its centre.
    t += np.clip(dist / 18, 0, 1) * 0.35 - 0.2
    idx = bands(t, [0.25, 0.4, 0.55, 0.7]).astype(np.int32)
    idx = np.where(dist < 1.5, 0, np.where(dist < 3.5, np.minimum(idx, 1), idx))
    cactus_spines(idx, seed + 2, 16, 10, 118, 0, 124)
    alpha = np.full((S, S), 255.0, np.float32)
    alpha[:, :8] = 0
    alpha[:, 120:] = 0
    # Spines sticking out sideways past the edges: short pale ticks with a bent tip.
    r = rng(seed + 3)
    for side in (0, 1):
        y = r.uniform(2, 16)
        while y < S - 4:
            yi = int(y) // 2 * 2
            ln = int(r.integers(3, 5)) * 2
            x0 = 8 - ln if side == 0 else 120
            idx[yi:yi + 2, x0:x0 + ln] = 6
            tip = x0 if side == 0 else x0 + ln - 2
            idx[yi + 2:yi + 4, tip:tip + 2] = 5
            alpha[yi:yi + 2, x0:x0 + ln] = 255
            alpha[yi + 2:yi + 4, tip:tip + 2] = 255
            y += r.uniform(14, 26)
    out = rgba(out_rgb(CACTUS_C, idx), alpha)
    out[out[..., 3] == 0, :3] = 0
    return out


def paint_cactus_top(seed):
    t = 0.6 + (noise(seed, 16) - 0.5) * 0.4 + (pix(seed + 1, 1) - 0.5) * 0.1
    idx = bands(t, [0.25, 0.4, 0.55, 0.68]).astype(np.int32)
    # Segment lines at the thirds, darker border.
    for g in (44, 83):
        idx = np.where((np.abs(XX - g) < 1.5) | (np.abs(YY - g) < 1.5), 2, idx)
    edge = np.minimum(np.minimum(XX - 8, 120 - XX), np.minimum(YY - 8, 120 - YY))
    idx = np.where(edge < 6, np.minimum(idx, 2), idx)
    idx = np.where(edge < 3, 1, idx)
    idx = np.where(edge < 1.5, 0, idx)
    cactus_spines(idx, seed + 2, 14, 12, 116, 12, 116)
    out = rgba(out_rgb(CACTUS_C, idx))
    out[edge < 0, 3] = 0
    out[out[..., 3] == 0, :3] = 0
    return out


# ---------------------------------------------------------------------------- plants


def seg(p0, p1, w0, w1):
    """Tapered stroke from p0 to p1 (x, y) with widths w0 -> w1: (mask, light) where light is
    -1..1 across the stroke, + on the side facing the top left."""
    (x0, y0), (x1, y1) = p0, p1
    dx, dy = x1 - x0, y1 - y0
    L = max(np.hypot(dx, dy), 1e-6)
    t = np.clip(((XX - x0) * dx + (YY - y0) * dy) / (L * L), 0, 1)
    px, py = x0 + t * dx, y0 + t * dy
    w = w0 + (w1 - w0) * t
    d = np.hypot(XX - px, YY - py)
    side = ((XX - x0) * dy - (YY - y0) * dx) / L
    facing = -(dy - dx) / L / 1.4142  # normal (dy, -dx)/L dotted with (-1, -1)/sqrt2
    light = side / (w / 2 + 1e-6) * np.sign(facing if abs(facing) > 0.2 else 1.0)
    return d <= w / 2, np.clip(light, -1, 1)


def draw(cidx, mask, light, tones, split=(-0.3, 0.35)):
    """Paints a stroke into an index canvas: tones = (dark side, body, lit side)."""
    val = np.where(light < split[0], tones[2], np.where(light > split[1], tones[0], tones[1]))
    cidx[mask] = val[mask]


def sprite_out(C: np.ndarray, cidx: np.ndarray) -> np.ndarray:
    m = cidx >= 0
    out = rgba(C[np.clip(cidx, 0, len(C) - 1)], np.where(m, 255, 0))
    out[~m, :3] = 0
    return out


def curve(pts, w0, w1):
    """Polyline stroke through pts, tapering from w0 to w1: (mask, light)."""
    n = len(pts) - 1
    mask = np.zeros((S, S), bool)
    light = np.zeros((S, S), np.float32)
    for i in range(n):
        a = w0 + (w1 - w0) * i / n
        b = w0 + (w1 - w0) * (i + 1) / n
        m, lt = seg(pts[i], pts[i + 1], a, b)
        new = m & ~mask
        light = np.where(new, lt, light)
        mask |= m
    return mask, light


SHORT_GRASS_C = pal("#6c6c6c", "#767776", "#838183", "#919191", "#a0a0a0", "#b8b7b8")


def paint_short_grass(seed):
    """A tuft: ~30 blades curving up and outward from a dense base, each with a lit stripe,
    the outer and front ones shorter; the bases crowd into a darker mass."""
    r = rng(seed)
    cidx = np.full((S, S), -1, np.int32)
    blades = []
    for k in range(30):
        x0 = 10 + (k + r.uniform(0.0, 1.0)) * 108 / 30
        central = max(0.0, 1 - abs(x0 - 64) / 60)
        h = r.uniform(22, 44) + central ** 0.8 * r.uniform(30, 72)
        lean = (x0 - 64) * r.uniform(0.3, 0.6) + r.uniform(-6, 6)
        lean = float(np.clip(x0 + lean, 3, 125) - x0)
        blades.append((h, x0, lean))
    # Tall ones first (behind), short ones in front darken the base.
    for h, x0, lean in sorted(blades, reverse=True):
        pts = [(x0, 130), (x0 + lean * 0.12, 128 - h * 0.4), (x0 + lean * 0.45, 128 - h * 0.78),
               (x0 + lean, 128 - h)]
        m, lt = curve(pts, r.uniform(7, 9), 1.5)
        body = 3 if h > 60 else 2
        val = np.where(lt < -0.3, body + 2, np.where(lt > 0.4, body - 1, body))
        val = np.where(YY > 128 - h * 0.35, np.maximum(val - 1, 0), val)
        val = np.where((YY > 116) & (lt > -0.3), np.maximum(val - 1, 0), val)
        cidx[m] = np.clip(val, 0, 5)[m]
    return sprite_out(SHORT_GRASS_C, cidx)


POPPY_C = pal("#204626", "#265a25", "#2b702a", "#4a8f28", "#742303", "#9b221a", "#bf2529",
              "#ed302c")


def paint_poppy(seed):
    cidx = np.full((S, S), -1, np.int32)
    # Leaves: two long thin blades rising outwards from the base.
    for p1 in ((44, 104), (96, 104)):
        m, lt = seg((66, 124), p1, 7, 1.5)
        draw(cidx, m, lt, (0, 1, 2))
    m, lt = seg((66, 128), (66, 76), 7, 6)
    draw(cidx, m, lt, (1, 2, 3))
    # Sepal below the head.
    m, lt = seg((60, 79), (72, 78), 5, 4)
    draw(cidx, m, lt, (0, 1, 2))
    # Head: tilted cup, darker ring and a dark centre.
    r0, dx, dy = blob(64, 60, 24, 17, -0.2, 2.0, jitter=0.12, jseed=seed, jcell=8)
    light = -(dx + dy) / 34
    head = np.where(light > 0.1, 7, 6)
    head = np.where((r0 > 0.85) & (light < -0.2), 5, head)
    head = np.where((r0 < 0.62) & (r0 > 0.42), 6, head)
    r1, _, _ = blob(62, 58, 10, 6.5, -0.2)
    head = np.where(r1 < 1, 5, head)
    head = np.where(r1 < 0.55, 4, head)
    cidx[r0 <= 1] = head[r0 <= 1]
    return sprite_out(POPPY_C, cidx)


DANDELION_C = pal("#177c04", "#4a8f28", "#55ab2d", "#bd6a22", "#f19d25", "#fed639", "#ffec4f")


def paint_dandelion(seed):
    r = rng(seed)
    cidx = np.full((S, S), -1, np.int32)
    # Toothed leaves low on both sides.
    for p0, p1 in (((64, 122), (42, 98)), ((66, 124), (90, 104)), ((64, 116), (50, 106))):
        m, lt = seg(p0, p1, 6, 2)
        draw(cidx, m, lt, (0, 1, 2))
        vx, vy = p1[0] - p0[0], p1[1] - p0[1]
        for k in range(1, 4):
            fx, fy = p0[0] + vx * k / 4, p0[1] + vy * k / 4
            sgn = -1 if vx < 0 else 1
            m, lt = seg((fx, fy), (fx + sgn * 5, fy - 7), 3, 1.5)
            draw(cidx, m, lt, (0, 1, 2))
    m, lt = seg((66, 128), (66, 84), 7, 6)
    draw(cidx, m, lt, (0, 1, 2))
    # Fluffy head: wide ellipse with a ragged rim, orange heart at the bottom.
    r0, dx, dy = blob(64, 78, 24, 12.5, 0, 2.0)
    rag = (pix(seed + 1, 2) - 0.5) * 0.35
    head = (r0 + rag) <= 1
    t = -(dx + dy) / 30 + (pix(seed + 3, 1) - 0.5) * 0.6
    val = np.where(t > 0.05, 6, 5)
    heart, _, _ = blob(66, 84, 12, 6)
    val = np.where((heart < 1) & (pix(seed + 4, 1) > 0.35), 4, val)
    val = np.where((heart < 0.55) & (pix(seed + 5, 1) > 0.5), 3, val)
    cidx[head] = val[head]
    return sprite_out(DANDELION_C, cidx)


DEAD_BUSH_C = pal("#513d24", "#67502c", "#946428", "#b17a36")


def paint_dead_bush(seed):
    """A gnarled dry shrub: a thick trunk leaning a little, forking into crooked branches
    with thin side twigs, lit on the left."""
    cidx = np.full((S, S), -1, np.int32)
    twigs = [  # (points, base width, tip width), thin ones first
        ([(66, 106), (50, 98), (34, 94)], 7, 2.5), ([(68, 98), (86, 90), (106, 86)], 7, 2.5),
        ([(64, 84), (44, 76), (24, 74)], 7, 2.5), ([(70, 70), (94, 58), (118, 50)], 8, 2.5),
        ([(60, 60), (40, 52), (22, 44)], 7, 2.5), ([(106, 54), (112, 62)], 4, 2),
        ([(40, 52), (36, 62)], 4, 2), ([(74, 44), (96, 24), (106, 6)], 9, 3),
        ([(96, 24), (106, 30)], 4, 2), ([(58, 40), (44, 20), (36, 6)], 9, 3),
        ([(44, 22), (52, 14)], 4, 2), ([(64, 44), (68, 24), (64, 12)], 8, 3),
    ]
    for pts, w0, w1 in twigs:
        m, lt = curve(pts, w0, w1)
        val = np.where(lt < -0.2, 2, np.where(lt > 0.4, 0, 1))
        cidx[m] = val[m]
    trunk = [(70, 128), (70, 108), (66, 86), (64, 64), (62, 46), (58, 30)]
    m, lt = curve(trunk, 18, 9)
    val = np.where(lt < -0.15, 2, np.where(lt > 0.45, 0, 1))
    val = np.where((lt < -0.45) & (YY < 96), 3, val)
    val = np.where((YY > 100) & (val >= 2), val - 1, val)
    # A few bark cracks.
    crack = (np.abs(XX - 66 - (YY - 100) * 0.1) < 1) & (YY > 70) & (YY < 118) & (pix(seed, 2) > 0.3)
    val = np.where(crack & (val > 0), val - 1, val)
    cidx[m] = val[m]
    for pts, w0, w1 in [([(62, 46), (80, 38)], 6, 4), ([(60, 36), (54, 30)], 5, 4)]:
        m, lt = curve(pts, w0, w1)
        val = np.where(lt < -0.2, 3, np.where(lt > 0.4, 0, 2))
        cidx[m] = val[m]
    return sprite_out(DEAD_BUSH_C, cidx)


def leaf_sprite(cidx, bx, by, ang, L, W, tones):
    """A pointed oval leaf from (bx, by) along ang: tones (dark, body, light)."""
    u = (XX - bx) * np.cos(ang) + (YY - by) * np.sin(ang)
    v = -(XX - bx) * np.sin(ang) + (YY - by) * np.cos(ang)
    f = np.clip(u / L, 0, 1)
    m = (u >= 0) & (u <= L) & (np.abs(v) <= W * np.sin(np.pi * f) ** 0.8)
    face = -np.sign(np.sin(ang) - np.cos(ang) + 1e-3)
    val = np.where(v * face > 0, tones[2], tones[1])
    val = np.where(np.abs(v) > W * np.sin(np.pi * f) ** 0.8 - 1.5, tones[0], val)
    cidx[m] = val[m]


OAK_SAPLING_C = pal("#105210", "#1f6519", "#408f2f", "#57ad3f", "#4c3214", "#5a3f1e",
                    "#70532e", "#7f6139")


def paint_oak_sapling(seed):
    cidx = np.full((S, S), -1, np.int32)
    back = [(30, 44, -2.7), (74, 26, -1.2), (98, 34, -0.4), (46, 72, -2.4), (100, 70, 0.2)]
    for bx, by, a in back:
        leaf_sprite(cidx, bx, by, a, 26, 6, (0, 0, 1))
    branches = [([(64, 128), (64, 100), (58, 78), (46, 52), (36, 36)], 20, 9),
                ([(64, 100), (72, 80), (84, 54), (96, 40)], 16, 8),
                ([(60, 84), (40, 78), (22, 82)], 11, 6),
                ([(70, 86), (92, 76), (104, 70)], 11, 6)]
    for pts, w0, w1 in branches:
        m, lt = curve(pts, w0, w1)
        val = np.where(lt < -0.3, 7, np.where(lt > 0.4, 4, np.where(lt > 0.0, 5, 6)))
        cidx[m] = val[m]
    front = [(34, 36, -2.3, 22), (40, 34, -0.9, 20), (96, 40, -0.3, 22), (90, 38, -1.9, 18),
             (22, 82, 2.8, 22), (22, 82, -2.2, 18), (104, 70, 0.9, 22), (104, 70, -0.8, 18),
             (68, 80, -0.3, 20), (54, 60, 2.2, 18), (80, 60, 1.2, 18)]
    for bx, by, a, L in front:
        leaf_sprite(cidx, bx, by, a, L * 1.25, 6, (1, 2, 3))
    return sprite_out(OAK_SAPLING_C, cidx)


BIRCH_SAPLING_C = pal("#51742d", "#5a7e33", "#6c9e38", "#acbf62", "#c9d7a5", "#e3ddba")


def paint_birch_sapling(seed):
    r = rng(seed)
    cidx = np.full((S, S), -1, np.int32)
    # Narrow, tall crown: a cone of foliage clumps from y 4 to 116.
    for k in range(160):
        y = r.uniform(6, 112)
        half = 6 + 32 * np.sin(np.pi * (y - 2) / 124) ** 0.8 * (0.4 + 0.6 * (y - 6) / 106)
        x = 64 + r.uniform(-half, half)
        rad = r.uniform(3.5, 6)
        rr, dx, dy = blob(x, y, rad, rad * 0.8, jitter=0.6, jseed=seed + k % 16, jcell=2)
        m = rr <= 1
        lt = -(dx + dy) / (2 * rad)
        val = np.where(lt > 0.15, 3, np.where(lt < -0.25, 0, np.where(r.random() < 0.5, 1, 2)))
        cidx[m] = val[m]
    # Pale trunk with branches reaching into the crown.
    m, lt = seg((66, 128), (64, 10), 9, 3)
    draw(cidx, m, lt, (4, 4, 5))
    for y0, dx0 in ((96, -26), (80, 24), (62, -20), (44, 18), (28, -12)):
        m, lt = seg((65, y0), (65 + dx0, y0 - 16), 4, 2)
        draw(cidx, m, lt, (4, 4, 5))
    return sprite_out(BIRCH_SAPLING_C, cidx)


SPRUCE_SAPLING_C = pal("#130803", "#2e1d0a", "#50361a", "#223522", "#2e492e", "#395a39")


def paint_spruce_sapling(seed):
    r = rng(seed)
    cidx = np.full((S, S), -1, np.int32)
    m, lt = seg((64, 128), (62, 16), 9, 3)
    draw(cidx, m, lt, (0, 1, 2))
    # Shaggy cone of needle clumps, jagged drooping tier edges.
    for k in range(260):
        y = r.uniform(14, 118)
        tier = (y - 14) % 16 / 16
        half = (5 + (y - 14) / 104 * 30) * (0.7 + 0.3 * tier)
        x = 63 + r.uniform(-half, half)
        rad = r.uniform(2.5, 4.5)
        rr, dx, dy = blob(x, y, rad * 1.3, rad * 0.7, rot=0.5 * np.sign(x - 63),
                          jitter=0.5, jseed=seed + k % 16, jcell=2)
        mm = rr <= 1
        lt = -(dx + dy) / (2 * rad)
        val = np.where(lt > 0.2, 5, np.where(lt < -0.2, 3, 4))
        cidx[mm] = val[mm]
    # Dark branch lines and trunk glimpses inside the crown.
    for y0, sgn in ((40, 1), (58, -1), (76, 1), (92, -1), (104, 1)):
        m, lt = seg((63, y0), (63 + sgn * 16, y0 + 8), 3, 1.5)
        cidx[m & (pix(seed + y0, 2) > 0.3)] = 1
    m, lt = seg((63, 112), (63, 128), 8, 8)
    draw(cidx, m, lt, (0, 1, 2))
    return sprite_out(SPRUCE_SAPLING_C, cidx)


# ---------------------------------------------------------------------------- fluids


# ---------------------------------------------------------------------------- table

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
