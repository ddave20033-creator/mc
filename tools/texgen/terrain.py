"""Terrain textures: natural blocks, ores, logs, leaves, fluids and plants.

The look follows Faithful 64x (see BRIEF.md), redrawn at 128x128: flat colors from small
per-material palettes, 1-2 px detail, light from the top left. Nothing here reads the
reference images; palettes and layouts were picked by eye.

Every block texture tiles (all noise wraps, shapes use wrapped distances) and the fluid
animations loop (fields morph around a circle in time or move by whole tile periods).
"""

from __future__ import annotations

import zlib

import numpy as np
from scipy import ndimage

from common import S, Ramp, fbm, grid, hexc, noise, pix, rgba, rng

TAU = 2.0 * np.pi
YY, XX = grid()


# ---------------------------------------------------------------------------- helpers


def seed_of(path: str) -> int:
    return zlib.crc32(path.encode()) & 0x7FFFFFFF


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


# ---------------------------------------------------------------------------- stone & ores

STONE_C = pal("#686868", "#747474", "#7f7f7f", "#8f8f8f")


def stone_rgb(seed: int) -> np.ndarray:
    f = streaks(seed, 4, 14, 2) * 0.7 + streaks(seed + 1, 2, 6, 2) * 0.3
    f = f + (pix(seed + 2, 1) - 0.5) * 0.03
    return out_rgb(STONE_C, levels(f, STONE_C, [7, 33, 42, 18]))


STONE_SEED = seed_of("block/stone")


def paint_stone(seed):
    return opaque_rgba(stone_rgb(seed))


def ore(seed: int, nuggets, cols, shadow="#5c5c5c", halo="#686868", p=2.0, inner=None,
        edge_w=1.6, hi=(0.55, 0.8), jitter=0.18):
    """Stone with mineral nuggets. `nuggets`: (cx, cy, w, h[, rot]) in pixels; `cols`: dark
    rim, mid, bright, highlight."""
    out = stone_rgb(STONE_SEED)
    C = pal(*cols)
    sh, ha = hexc(shadow), hexc(halo)
    mott = streaks(seed + 7, 3, 6, 2)
    for i, n in enumerate(nuggets):
        cx, cy, w, h = n[:4]
        rot = n[4] if len(n) > 4 else 0.0
        rx, ry = w / 2, h / 2
        rmin = min(rx, ry)
        r, dx, dy = blob(cx, cy, rx, ry, rot, p, jitter=jitter, jseed=seed + i, jcell=4)
        br = np.clip((dx / rx + dy / ry) * 0.7 + 0.3, 0, 1)  # bottom-right side
        ring = (r > 1) & (r <= 1 + (0.8 + 1.8 * br) / rmin)
        out[ring & (br > 0.35)] = sh
        out[ring & (br <= 0.35)] = ha
        inside = r <= 1
        light = -(dx / rx + dy / ry) * 0.5
        t = (1 - r) * 0.6 + light * 0.5 + (mott - 0.5) * 0.7
        idx = bands(t, [0.2, hi[0], hi[1]]) + 1
        rim = r > 1 - (edge_w + 2.2 * br) / rmin
        idx = np.where(rim, 0, idx)
        col = C[np.clip(idx, 0, len(C) - 1)]
        out[inside] = col[inside]
        if inner is not None:
            spk = inside & ~rim & (pix(seed + 80 + i, 2) > 0.8) & (idx == 1)
            out[spk] = hexc(inner)
    return opaque_rgba(out)


def paint_coal_ore(seed):
    nug = [(64, 28, 44, 24, 0.15), (104, 22, 16, 12), (16, 34, 16, 12), (93, 56, 24, 16),
           (42, 67, 38, 24, -0.1), (113, 84, 20, 16), (33, 102, 38, 20), (91, 102, 24, 20),
           (57, 115, 20, 12)]
    return ore(seed, nug, ("#252525", "#2e2e2e", "#393c36", "#494b3f"), "#5c5c5c", "#5c5c5c",
               inner="#363636", hi=(0.45, 0.78), jitter=0.3)


def paint_iron_ore(seed):
    nug = [(96, 14, 32, 12, -0.1), (26, 24, 36, 16, 0.1), (66, 42, 68, 20, -0.08),
           (112, 44, 16, 8), (88, 71, 32, 14, -0.15), (36, 76, 48, 16, 0.1),
           (91, 101, 54, 20, -0.1), (43, 100, 18, 8), (18, 110, 28, 12, 0.1)]
    return ore(seed, nug, ("#af8e77", "#d8af93", "#d8af93", "#e2c0aa"), "#77674f", "#887455",
               p=1.4)


def paint_gold_ore(seed):
    nug = [(35, 13, 10, 10), (56, 42, 40, 52, 0.6), (108, 29, 24, 22), (112, 60, 16, 8),
           (25, 73, 22, 16), (92, 94, 46, 32, -0.25), (32, 107, 34, 24, 0.2), (57, 100, 10, 8),
           (115, 117, 10, 12)]
    return ore(seed, nug, ("#9c7020", "#eb9d0e", "#fcee4b", "#ffffb5"), "#5c5c5c", "#686868",
               p=1.2, hi=(0.62, 0.95))


def paint_diamond_ore(seed):
    nug = [(38, 20, 12, 8), (105, 24, 24, 10, -0.2), (66, 28, 20, 8), (49, 49, 40, 16, -0.15),
           (92, 47, 18, 14), (19, 67, 20, 12, -0.2), (77, 74, 38, 16, -0.1), (38, 84, 12, 8),
           (91, 100, 52, 24, -0.25), (36, 108, 24, 8)]
    return ore(seed, nug, ("#239698", "#1ed0d6", "#77e7d1", "#d5fff6"), "#676767", "#8dadb1",
               p=1.4, hi=(0.45, 0.8))


COBBLE_C = pal("#525252", "#616161", "#6e6d6d", "#888788", "#a6a6a6", "#b5b5b5")
COBBLES = [(21, 15, 34, 32), (67, 15, 42, 32), (106, 1, 42, 34), (96, 40, 40, 34),
           (35, 46, 38, 30), (3, 53, 28, 28), (75, 67, 40, 28), (25, 77, 38, 26),
           (116, 89, 40, 30), (66, 101, 42, 28), (21, 111, 44, 32)]


def paint_cobblestone(seed):
    items = [(cx, cy, w / 2 + 1.5, h / 2 + 1.5) for cx, cy, w, h in COBBLES]
    H, ids, L, R = bumps(seed, items, jitter=0.12)
    stone = ids >= 0
    # Mortar between the stones: mid greys in streaks, dark shadow under/right of stones.
    mort = streaks(seed + 3, 4, 10, 2)
    idx = np.where(mort > 0.55, 2, 1)
    shadow = ~stone & np.roll(np.roll(stone, 2, 0), 2, 1)
    idx = np.where(shadow, 0, idx)
    # Stones: bright on the top left, a darker rim on the bottom right, speckled surface.
    t = L * 0.9 + (1 - R) * 0.5 + (streaks(seed + 4, 3, 6, 2) - 0.5) * 0.5
    t += (pix(seed + 5, 1) - 0.5) * 0.12
    sidx = bands(t, [-0.05, 0.32, 0.7]) + 2
    sidx = np.where((R > 0.86) & (L < 0.1), 2, sidx)
    idx = np.where(stone, sidx, idx)
    return opaque_rgba(out_rgb(COBBLE_C, idx))


# ---------------------------------------------------------------------------- dirt & grass

DIRT_C = pal("#593d29", "#79553a", "#966c4a", "#b9855c")
PEBBLE_C = pal("#6c6c6c", "#878787")
PEBBLES = [(50, 18), (10, 26), (93, 35), (38, 50), (118, 84), (66, 94), (54, 122)]


def dirt_rgb(seed: int) -> np.ndarray:
    r = rng(seed)
    items = []
    # Lumpy clods on a jittered grid: 7 x 7 domes of 16-24 px.
    n = 8
    for i in range(n):
        for j in range(n):
            cx = (j + 0.5 + (r.random() - 0.5) * 0.8) * S / n
            cy = (i + 0.5 + (r.random() - 0.5) * 0.8) * S / n
            rad = r.uniform(9, 13)
            items.append((cx, cy, rad * r.uniform(1.0, 1.25), rad, r.uniform(0.7, 1.0)))
    H, ids, L, R = bumps(seed, items, jitter=0.2)
    t = L * 0.8 + (1 - R) * 0.35 + (noise(seed + 2, 4) - 0.5) * 0.4
    t += (pix(seed + 3, 1) - 0.5) * 0.25
    crev = ((R > 0.88) & (L < 0.1)) | (ids < 0)
    t = np.where(crev, -9, t)
    idx = levels(t, DIRT_C, [13, 38, 32, 13])
    out = out_rgb(DIRT_C, idx)
    for k, (cx, cy) in enumerate(PEBBLES):
        pr, dx, dy = blob(cx, cy, 6, 6, jitter=0.25, jseed=seed + 90 + k, jcell=4)
        m = pr <= 1
        lit = -(dx + dy) / 12 + (pix(seed + 100 + k, 2) - 0.5) * 0.6
        col = np.where((lit > 0.1)[..., None], PEBBLE_C[1], PEBBLE_C[0])
        out[m] = col[m]
    return out


def paint_dirt(seed):
    return opaque_rgba(dirt_rgb(seed))


def paint_grass_block_side(seed):
    return opaque_rgba(dirt_rgb(seed_of("block/dirt")))


GRASS_C = pal("#797979", "#868686", "#939393", "#9c9c9c", "#ababab", "#c0c0c0")


def grass_top_field(seed: int) -> np.ndarray:
    """Short diagonal blade strokes seen from above, criss-crossing in patches."""
    A = shear(anoise(seed, 5, 2.5), 1)
    B = shear(anoise(seed + 1, 5, 2.5), -1)
    sel = noise(seed + 2, 8)
    f = np.where(sel > 0.5, A * 0.8 + B * 0.2, B * 0.8 + A * 0.2)
    f = f + (noise(seed + 3, 8) - 0.5) * 0.3 + (pix(seed + 4, 1) - 0.5) * 0.18
    return f


def paint_grass_block_top(seed):
    f = grass_top_field(seed)
    return opaque_rgba(out_rgb(GRASS_C, levels(f, GRASS_C, [7, 24, 29, 21, 16, 3])))


def paint_grass_block_side_overlay(seed):
    r = rng(seed)
    h = np.full(S, 16.0, np.float32)
    h += (np.repeat(r.random(S // 2), 2) - 0.5) * 3
    h += (anoise(seed + 1, 1, 16, 1, S)[0] - 0.5) * 6
    x = int(r.integers(0, 6))
    while x < S:
        w = r.uniform(3, 6)
        ln = r.uniform(6, 20)
        for k in range(-int(w) - 1, int(w) + 2):
            h[(x + k) % S] = max(h[(x + k) % S], 16 + ln * (1 - abs(k) / w))
        x += int(r.integers(6, 12))
    h = np.round(h)
    mask = YY < h[None, :]
    f = anoise(seed + 2, 16, 2) * 0.6 + anoise(seed + 3, 6, 1.5) * 0.25
    f = f + (pix(seed + 4, 1) - 0.5) * 0.15
    f = np.where(YY > h[None, :] - 3, f - 0.2, f)
    idx = levels(f, GRASS_C, [12, 23, 25, 26, 14], mask)
    out = rgba(out_rgb(GRASS_C, idx), np.where(mask, 255, 0))
    out[~mask, :3] = 0
    return out


SNOW_C = pal("#b2d5d5", "#d7efef", "#f0fdfd", "#f7fefe", "#ffffff")


def snow_field(seed: int) -> np.ndarray:
    return streaks(seed, 8, 16, 3) * 0.8 + (pix(seed + 1, 1) - 0.5) * 0.1


def paint_snow(seed):
    f = snow_field(seed)
    return opaque_rgba(out_rgb(SNOW_C[2:], levels(f, SNOW_C[2:], [20, 41, 38])))


def paint_grass_block_snow(seed):
    out = dirt_rgb(seed_of("block/dirt"))
    r = rng(seed)
    # The dirt's top edge is a row of jagged peaks; snow fills everything above it.
    xs = np.arange(S) + 0.5
    edge = np.full(S, 50.0)
    x = 0.0
    while x < S + 20:
        w = r.uniform(20, 34)
        pk = r.uniform(6, 11)
        for k in range(S):
            d = abs(((xs[k] - x + S / 2) % S) - S / 2)
            if d < w / 2:
                edge[k] = min(edge[k], 50 - pk * (1 - 2 * d / w))
        x += w * r.uniform(0.7, 1.0)
    edge = np.round(edge + (np.repeat(r.random(S // 2), 2) - 0.5) * 2)
    e = edge[None, :]
    snow = YY < e
    f = snow_field(seed_of("block/snow"))
    sidx = levels(f, SNOW_C[2:], [20, 41, 38], snow) + 2
    depth = e - YY
    sidx = np.where(snow & (depth < 12) & (f < np.quantile(f, 0.75)), 1, sidx)
    sidx = np.where(snow & (depth < 4) & (pix(seed + 3, 1) > 0.3), 1, sidx)
    sidx = np.where(snow & (depth < 2), 0, sidx)
    col = SNOW_C[np.clip(sidx, 0, 4)]
    out[snow] = col[snow]
    # A thin darker (frozen) line along the dirt's top edge.
    lip = (YY >= e) & (YY < e + 2)
    out[lip] = hexc("#78554d")
    lip2 = (YY >= e + 2) & (YY < e + 4) & (pix(seed + 4, 1) > 0.5)
    out[lip2] = hexc("#956c5d")
    return opaque_rgba(out)


# ---------------------------------------------------------------------------- sand & co


def cells(seed: int, count: int, jitter: float = 0.9, warp: float = 0.0, warp_cell: float = 16):
    """Tiling Voronoi with a noise warp: (ids, d1, border, dy, dx) where (dy, dx) is the
    pixel's offset from its cell's point."""
    r = rng(seed)
    side = int(np.ceil(np.sqrt(count)))
    pts = []
    for i in range(side):
        for j in range(side):
            if len(pts) >= count:
                break
            pts.append(((i + 0.5 + (r.random() - 0.5) * jitter) * S / side,
                        (j + 0.5 + (r.random() - 0.5) * jitter) * S / side))
    yy, xx = YY.copy(), XX.copy()
    if warp:
        yy = yy + (noise(seed + 101, warp_cell) - 0.5) * 2 * warp
        xx = xx + (noise(seed + 102, warp_cell) - 0.5) * 2 * warp
    best = np.full((S, S), 1e9, np.float32)
    second = np.full((S, S), 1e9, np.float32)
    ids = np.zeros((S, S), np.int32)
    bdy = np.zeros((S, S), np.float32)
    bdx = np.zeros((S, S), np.float32)
    for k, (py, px) in enumerate(pts):
        dy = (yy - py + S / 2) % S - S / 2
        dx = (xx - px + S / 2) % S - S / 2
        d = np.hypot(dy, dx)
        closer = d < best
        second = np.where(closer, best, np.minimum(second, d))
        ids = np.where(closer, k, ids)
        bdy = np.where(closer, dy, bdy)
        bdx = np.where(closer, dx, bdx)
        best = np.where(closer, d, best)
    return ids, best, (second - best) * 0.5, bdy, bdx


SAND_C = pal("#d1ba8a", "#d5c496", "#dacfa3", "#e3dbb0", "#e7e4bb", "#edebcb")


def sand_field(seed: int) -> np.ndarray:
    return (pix(seed, 1) * 0.2 + pix(seed + 3, 2) * 0.3 + noise(seed + 1, 4) * 0.25
            + shear(anoise(seed + 2, 6, 2), 1) * 0.25)


def paint_sand(seed):
    f = sand_field(seed)
    return opaque_rgba(out_rgb(SAND_C, levels(f, SAND_C, [7, 26, 36, 25, 1, 5])))


GRAVEL_GREY = pal("#645b5b", "#726b69", "#817f7f", "#979797", "#b0aeae")
GRAVEL_WARM = pal("#645b5b", "#726b69", "#89817e", "#968e8e", "#b1a2a2")


def paint_gravel(seed):
    r = rng(seed)
    items = []
    n = 9
    for i in range(n):
        for j in range(n):
            cx = (j + 0.5 + (i % 2) * 0.5 + (r.random() - 0.5) * 0.5) * S / n
            cy = (i + 0.5 + (r.random() - 0.5) * 0.5) * S / n
            rad = r.uniform(7, 10)
            items.append((cx, cy, rad * r.uniform(1.0, 1.2), rad, r.uniform(0.6, 1.0)))
    H, ids, L, R = bumps(seed, items, jitter=0.3)
    t = L * 0.7 + (1 - R) * 0.2 + (pix(seed + 2, 1) - 0.5) * 0.35
    t += (noise(seed + 5, 4) - 0.5) * 0.4
    t += (rng(seed + 3).random(len(items) + 1)[ids] - 0.5) * 0.5
    idx = bands(t, [-0.35, 0.0, 0.4, 0.75]).astype(np.int32)
    idx = np.where(((R > 0.9) & (L < 0.2)) | (ids < 0), 1, idx)
    idx = np.where((R > 0.95) & (L < -0.2), 0, idx)
    warm = (rng(seed + 4).random(len(items) + 1) < 0.3)[ids]
    out = np.where(warm[..., None], out_rgb(GRAVEL_WARM, idx), out_rgb(GRAVEL_GREY, idx))
    return opaque_rgba(out)


CLAY_C = pal("#9499a4", "#9ca1ac", "#9aa3b3", "#a1a7b1", "#acaebd", "#afb9d6")


def scales(seed: int, rows: int = 8, w: float = 18, h: float = 14):
    """Overlapping fish-scale bumps in offset rows (clay, obsidian)."""
    r = rng(seed)
    items = []
    cols = int(round(S / w))
    for i in range(rows):
        for j in range(cols):
            cx = (j + 0.5 * (i % 2) + (r.random() - 0.5) * 0.4) * S / cols
            cy = (i + (r.random() - 0.5) * 0.4) * S / rows
            items.append((cx, cy, w * r.uniform(0.55, 0.7), h * r.uniform(0.6, 0.75),
                          1 + i * 0.01 + r.random() * 0.2))
    return bumps(seed, items, jitter=0.15)


def paint_clay(seed):
    H, ids, L, R = scales(seed, 11, 15, 12)
    f = 0.5 + (noise(seed + 1, 16) - 0.5) * 0.4 + (pix(seed + 2, 1) - 0.5) * 0.25
    idx = np.where(f > 0.62, 4, np.where(f > 0.42, 3, 1))
    rim = (R > 0.86) & (L < 0.1)
    idx = np.where(rim, np.where(pix(seed + 3, 1) > 0.4, 0, 1), idx)
    idx = np.where((R < 0.7) & (L > 0.3) & (pix(seed + 4, 1) > 0.6), 2, idx)
    idx = np.where(pix(seed + 5, 1) > 0.992, 5, idx)
    return opaque_rgba(out_rgb(CLAY_C, idx))


ICE_C = pal("#86aefd", "#8cb3fe", "#92b9fe", "#a1c3ff", "#bcd4ff", "#c8dcff")


def paint_ice(seed):
    f = shear(anoise(seed, 16, 6), -2) * 0.3 + noise(seed + 4, 16) * 0.6
    f = f + (pix(seed + 1, 1) - 0.5) * 0.08
    idx = levels(f, ICE_C, [17, 29, 37, 17])
    # A few light streaks rising to the right.
    st = shear(anoise(seed + 2, 16, 5), -2) * 0.6 + noise(seed + 3, 32) * 0.4
    idx = np.where(st > 0.75, 3, idx)
    idx = np.where(st > 0.81, 4, idx)
    return opaque_rgba(out_rgb(ICE_C, idx))


BEDROCK_C = pal("#222222", "#333333", "#575757", "#636363", "#979797")


def paint_bedrock(seed):
    f = streaks(seed, 5, 16, 3) * 0.75 + streaks(seed + 1, 2, 6, 2) * 0.25
    return opaque_rgba(out_rgb(BEDROCK_C, levels(f, BEDROCK_C, [6, 28, 32, 25, 9])))


OBSIDIAN_C = pal("#000001", "#06030b", "#100c1c", "#271e3d", "#3b2754")


def paint_obsidian(seed):
    r = rng(seed)
    items = []
    for i in range(6):
        for j in range(5):
            rad = r.uniform(9, 13)
            cx = (j + 0.5 * (i % 2) + (r.random() - 0.5) * 0.6) * S / 5
            cy = (i + (r.random() - 0.5) * 0.5) * S / 6
            items.append((cx, cy, rad * r.uniform(1.3, 1.7), rad, r.uniform(0.6, 1.0)))
    H, ids, L, R, NX, NY = bumps(seed, items, jitter=0.15, want_xy=True)
    t = (1 - R) * 0.5 + (noise(seed + 1, 8) - 0.5) * 0.4
    idx = bands(t, [0.02, 0.18]).astype(np.int32)
    idx = np.where(ids < 0, 0, idx)
    # Purple crescents along the upper rims of the lumps.
    cy_ = NY + 0.35 * NX
    cres = (R > 0.45) & (R < 0.95) & (cy_ < -0.2)
    idx = np.where(cres, 3, idx)
    idx = np.where(cres & (R > 0.62) & (R < 0.86) & (cy_ < -0.5) & (NX < 0.3), 4, idx)
    # Dark gap under each lump.
    idx = np.where((R > 0.9) & (NY > 0.2), 0, idx)
    return opaque_rgba(out_rgb(OBSIDIAN_C, idx))


SANDSTONE_C = pal("#c6ae71", "#d1ba8a", "#d5c496", "#dad2a3", "#e3dbb0", "#e7e4bb", "#edebcb")


def paint_sandstone(seed):
    r = rng(seed)
    items = []
    rows = [(42, 3, 0.1), (72, 3, 0.6), (100, 3, 0.2), (126, 3, 0.75)]
    for cy, n, off in rows:
        for j in range(n):
            w = S / n
            cx = (j + 0.5 + off + (r.random() - 0.5) * 0.3) * w
            items.append((cx, cy + (r.random() - 0.5) * 8, w * r.uniform(0.55, 0.68),
                          r.uniform(16, 20), r.uniform(0.8, 1.0)))
    H, ids, L, R, NX, NY = bumps(seed, items, jitter=0.1, want_xy=True)
    t = 0.55 + L * 0.35 + (1 - R) * 0.15 + (streaks(seed + 1, 3, 8, 2) - 0.5) * 0.35
    t += (pix(seed + 2, 1) - 0.5) * 0.1
    idx = bands(t, [0.4, 0.72, 0.92]) + 2
    idx = np.where((R > 0.8) & (L < 0.15), 2, idx)
    idx = np.where((R > 0.9) | (ids < 0), 1, idx)
    idx = np.where((ids < 0) & (pix(seed + 5, 1) > 0.5), 0, idx)
    seam = (ids != np.roll(ids, 1, 0)) | (ids != np.roll(ids, 1, 1))
    seam = seam | np.roll(seam, -1, 0) | np.roll(seam, -1, 1)
    idx = np.where(seam, 1, idx)
    # Smooth top band with a darker lower edge.
    band = YY < 22
    bt = 0.6 + (streaks(seed + 3, 4, 16, 2) - 0.5) * 0.5 + (pix(seed + 4, 1) - 0.5) * 0.1
    bidx = bands(bt, [0.35, 0.5, 0.7]) + 2
    bidx = np.where(YY < 3, bidx + 1, bidx)
    bidx = np.where(YY >= 19, 2, bidx)
    idx = np.where(band, bidx, idx)
    idx = np.where((YY >= 22) & (YY < 25), 0, idx)
    idx = np.where((YY >= 25) & (YY < 27), 1, idx)
    return opaque_rgba(out_rgb(SANDSTONE_C, idx))


def paint_sandstone_top(seed):
    wy = (noise(seed, 16) - 0.5) * 22
    wx = (noise(seed + 1, 16) - 0.5) * 22
    a = np.abs(np.sin(TAU * (XX + YY + wx) * 5 / (2 * S)))
    b = np.abs(np.sin(TAU * (XX - YY + wy) * 5 / (2 * S)))
    lines = np.minimum(a, b)
    f = 0.6 + (noise(seed + 2, 16) - 0.5) * 0.4 + (pix(seed + 3, 1) - 0.5) * 0.15
    f = f + (noise(seed + 5, 8) - 0.5) * 0.3
    f = np.where((lines < 0.14) & (noise(seed + 4, 8) > 0.45), f - 0.2, f)
    f = np.where((lines >= 0.14) & (lines < 0.3), f - 0.08, f)
    C = SANDSTONE_C[2:]
    return opaque_rgba(out_rgb(C, levels(f, C, [2, 12, 32, 50, 3])))


GLOW_C = pal("#6f4522", "#734e26", "#855029", "#886839", "#cc8654", "#fbda74", "#fff0da",
             "#ffffff")
GLOWS = [(11, 12, 26, 24), (56, 3, 32, 24), (100, 12, 40, 24), (38, 43, 42, 40),
         (80, 36, 32, 24), (118, 42, 28, 28), (2, 72, 34, 32), (85, 74, 32, 36),
         (40, 78, 32, 20), (62, 100, 34, 24), (24, 112, 32, 32), (103, 107, 32, 24)]


def paint_glowstone(seed):
    items = [(cx, cy, w / 2 + 1, h / 2 + 1) for cx, cy, w, h in GLOWS]
    H, ids, L, R, NX, NY = bumps(seed, items, jitter=0.1, want_xy=True)
    bg = streaks(seed + 1, 6, 10, 2)
    idx = np.where(bg > 0.5, 1, 0)
    halo = (ids < 0) & wgrow(ids >= 0, 3)
    idx = np.where(halo, np.where(pix(seed + 2, 1) > 0.5, 3, 2), idx)
    blobm = ids >= 0
    br = np.clip(-L * 1.4 + 0.3, 0, 1)
    rim = blobm & (R > 1 - (0.12 + 0.25 * br))
    core = blobm & (np.hypot(NX + 0.2, NY + 0.22) < 0.6)
    spot = blobm & (np.hypot(NX + 0.35, NY + 0.35) < 0.22)
    idx = np.where(blobm, 5, idx)
    idx = np.where(core, 6, idx)
    idx = np.where(spot, 7, idx)
    idx = np.where(rim, 4, idx)
    return opaque_rgba(out_rgb(GLOW_C, idx))


# ---------------------------------------------------------------------------- logs


def bark(seed: int, C: np.ndarray, fracs=(3, 12, 29, 46, 9, 2)) -> np.ndarray:
    """Braided vertical bark: vertical streaks bent into zigzags that differ from column to
    column, so dark grooves and light ridges weave. C: 6 colors dark to light."""
    base = anoise(seed, 12, 3.2) * 0.6 + anoise(seed + 1, 6, 2) * 0.4
    ph = anoise(seed + 2, 128, 21.3)[0]  # phase per column
    per = S / 3
    warp = 3.0 * np.sin(TAU * (YY / per + ph[None, :] * 1.2))
    f = ndimage.map_coordinates(base, [YY - 0.5, XX - 0.5 + warp], order=1, mode="grid-wrap")
    f = f + (pix(seed + 4, 1) - 0.5) * 0.08
    return out_rgb(C, levels(f, C, fracs))


def log_top(seed: int, bark_c: np.ndarray, ring_c: np.ndarray, border=9, spacing=11.0,
            bark_rgb=None) -> np.ndarray:
    """Square growth rings inside a bark rim. ring_c: 6 colors dark to light."""
    c = S / 2
    wob = (noise(seed, 16) - 0.5) * 1.5
    d = np.maximum(np.abs(YY - c), np.abs(XX - c)) + wob
    ring = (d / spacing) % 1.0
    t = 0.7 + (noise(seed + 1, 8) - 0.5) * 0.25 + (pix(seed + 2, 1) - 0.5) * 0.1
    t = np.where(ring < 0.22, 0.25, t)
    t = np.where((ring >= 0.22) & (ring < 0.35), 0.45, t)
    t = np.where(d < 3.5, 0.3, t)
    idx = bands(t, [0.3, 0.5, 0.62, 0.78, 0.92]).astype(np.int32)
    out = out_rgb(ring_c, idx)
    edge = np.maximum(np.abs(YY - c), np.abs(XX - c))
    inner = (edge >= c - border - 2) & (edge < c - border)
    out[inner] = ring_c[1]
    bm = edge >= c - border
    if bark_rgb is None:
        bf = streaks(seed + 3, 6, 6, 2) + (pix(seed + 4, 1) - 0.5) * 0.12
        bark_rgb = out_rgb(bark_c, levels(bf, bark_c, [2, 4, 3]))
    out[bm] = bark_rgb[bm]
    return opaque_rgba(out)


OAK_BARK_C = pal("#382b18", "#4c3d26", "#5f4a2b", "#745a36", "#917142", "#987849")
OAK_RING_C = pal("#967441", "#9f844d", "#af8f55", "#b8945f", "#af8f55", "#c29d62")
SPRUCE_BARK_C = pal("#2e1608", "#2e1c0a", "#311e0b", "#3b2713", "#4d3317", "#553a1f")
SPRUCE_RING_C = pal("#5a4424", "#614b2e", "#70522e", "#7a5a34", "#7a5a34", "#82613a")


def paint_oak_log(seed):
    return opaque_rgba(bark(seed, OAK_BARK_C))


def paint_oak_log_top(seed):
    return log_top(seed, OAK_BARK_C[1:4], OAK_RING_C)


def paint_spruce_log(seed):
    return opaque_rgba(bark(seed, SPRUCE_BARK_C))


def paint_spruce_log_top(seed):
    return log_top(seed, pal("#443321", "#553a1f", "#3b2713"), SPRUCE_RING_C)


BIRCH_C = pal("#36342a", "#605e54", "#bebeae", "#d8d8ce", "#ded3d5", "#eef1ea", "#f0eeeb",
              "#ffffff")
BIRCH_MARKS = [(50, 34, 38, 12, 0.3), (128, 45, 40, 14, -0.1), (112, 108, 38, 12, -0.35),
               (10, 114, 26, 12, 0.45)]


def birch_rgb(seed: int) -> np.ndarray:
    # Shingle pattern: shallow chevrons of faint pinkish-grey lines on white.
    wob = (noise(seed, 32) - 0.5) * 8
    u = (XX + wob) / 32.0
    zig = np.abs((u % 1.0) - 0.5) * 16  # 0..8 px up and down
    rows = (YY + zig + (noise(seed + 1, 16) - 0.5) * 6) % (S / 6)
    lines = rows < 1.6
    t = 0.7 + (streaks(seed + 2, 4, 16, 2) - 0.5) * 0.6 + (pix(seed + 3, 1) - 0.5) * 0.15
    idx = bands(t, [0.45, 0.6, 0.72]) + 4
    idx = np.where(idx == 4, 5, idx)
    idx = np.where(lines & (noise(seed + 4, 8) > 0.3), np.where(pix(seed + 5, 1) > 0.5, 4, 3),
                   idx)
    idx = np.where(np.roll(lines, 1, 0) & ~lines & (noise(seed + 4, 8) > 0.3), 2, idx)
    out = out_rgb(BIRCH_C, idx)
    for k, (cx, cy, w, h, rot) in enumerate(BIRCH_MARKS):
        rr, dx, dy = blob(cx, cy, w / 2, h / 2, rot, 1.2, jitter=0.15, jseed=seed + 20 + k,
                          jcell=4)
        m = rr <= 1
        out[m] = BIRCH_C[0]
        rim = (rr > 1) & (rr <= 1.18) & (dy > 0)
        out[rim] = BIRCH_C[1]
    return out


def paint_birch_log(seed):
    return opaque_rgba(birch_rgb(seed))


def paint_birch_log_top(seed):
    # Bark rim: white with dark lenticels.
    bf = pix(seed + 5, 1) * 0.1 + noise(seed + 6, 4) * 0.9
    rim = np.where((bf > 0.66)[..., None], hexc("#514f47"),
                   np.where((bf > 0.6)[..., None], hexc("#999682"),
                            np.where((bf > 0.3)[..., None], hexc("#ebebe7"), hexc("#ffffff"))))
    rings = pal("#ae9f76", "#b8a875", "#c8b77a", "#c8b77a", "#c8b77a", "#d7c185")
    return log_top(seed, None, rings, bark_rgb=rim)


# ---------------------------------------------------------------------------- leaves
# (First-round leaves, preferred by the user: small sprays of pointed leaves over a dark
# inner canopy, 12-15 % holes mostly in the gaps between leaves.)

LEAF = Ramp("#2c2c2c", "#3c3c3c", "#4d4d4d", "#5f5f5f", "#717171", "#838383", "#969696",
            "#a9a9a9")


def q(a: np.ndarray, ky: int = 2, kx: int | None = None) -> np.ndarray:
    """Snaps a field to ky x kx pixel clusters (pixel-art grain)."""
    kx = kx or ky
    h, w = a.shape[:2]
    return np.repeat(np.repeat(a[::ky, ::kx], ky, 0), kx, 1)[:h, :w]


def window(cy: float, cx: float, R: int):
    """Wrapped index window around a point: (rows, cols, dy, dx) for stamping small shapes."""
    iy = np.arange(int(np.floor(cy)) - R, int(np.floor(cy)) + R + 1)
    ix = np.arange(int(np.floor(cx)) - R, int(np.floor(cx)) + R + 1)
    dy = (iy + 0.5 - cy)[:, None] * np.ones((1, len(ix)), np.float32)
    dx = (ix + 0.5 - cx)[None, :] * np.ones((len(iy), 1), np.float32)
    return np.ix_(iy % S, ix % S), dy, dx


def stamp_leaf(t, cov, cy, cx, ang, L, W, base, vein=True):
    """Draws one pointed leaf (shade values into `t`, coverage into `cov`), lit top left."""
    R = int(np.ceil(L)) + 1
    idx, dy, dx = window(cy, cx, R)
    u = dx * np.cos(ang) + dy * np.sin(ang)
    v = -dx * np.sin(ang) + dy * np.cos(ang)
    f = np.clip(1 - (u / L) ** 2, 0, 1)
    wv = W * f ** 0.7
    m = np.abs(v) <= wv + 0.15
    if not m.any():
        return
    lit = -(dy + dx) / (1.4142 * L)
    tl = base + lit * 0.25
    rim = (np.abs(v) > wv - 1.1) | (np.abs(u) > L - 1.1)
    tl = np.where(rim & (dy + dx > 0), tl - 0.3, tl)
    tl = np.where(rim & (dy + dx <= 0), tl + 0.08, tl)
    if vein:
        tl = np.where((np.abs(v) < 0.6) & (np.abs(u) < L - 1.5), tl - 0.1, tl)
    sub = t[idx]
    sub[m] = tl[m]
    t[idx] = sub
    if cov is not None:
        c = cov[idx]
        c[m] = True
        cov[idx] = c


def leaves(seed: int, kind: str, holes: float = 0.14, bright: float = 0.0):
    r = rng(seed)
    # Holes: scattered clumps of 2-6 px, the rest is dark inner canopy.
    hn = q(noise(seed + 1, 8) * 0.45 + pix(seed + 2, 4) * 0.4 + pix(seed + 5, 2) * 0.15, 2)
    t = np.full((S, S), 0.12, np.float32) + (pix(seed + 3, 2) - 0.5) * 0.1
    n_sprays = {"oak": 150, "birch": 200, "spruce": 210}[kind]
    for i in range(n_sprays):
        sy, sx = r.random() * S, r.random() * S
        a0 = r.uniform(0, TAU)
        count = int(r.integers(4, 7)) if kind != "spruce" else int(r.integers(6, 10))
        for j in range(count):
            if kind == "spruce":
                # Needles on both sides of a short twig.
                twig = a0
                along = (j // 2) * 3.0 - 6
                side = 1 if j % 2 else -1
                ang = twig + side * r.uniform(0.6, 0.9)
                L, W = r.uniform(3.5, 5.5), r.uniform(1.0, 1.4)
                cy = sy + np.sin(twig) * along + np.sin(ang) * L * 0.9
                cx = sx + np.cos(twig) * along + np.cos(ang) * L * 0.9
            else:
                ang = a0 + j * TAU / count + r.uniform(-0.3, 0.3)
                if kind == "birch":
                    L, W = r.uniform(3.5, 4.8), r.uniform(2.2, 3.0)
                else:
                    L, W = r.uniform(4.5, 6.5), r.uniform(2.4, 3.3)
                cy = sy + np.sin(ang) * L * 0.85
                cx = sx + np.cos(ang) * L * 0.85
            stamp_leaf(t, None, cy, cx, ang, L, W, r.uniform(0.42, 0.78) + bright,
                       vein=kind != "spruce")
    # Holes where the hole noise is low, preferably in the gaps between leaves.
    score = hn + (t > 0.3) * 0.35
    cov = score > np.quantile(score, holes)
    t += (pix(seed + 4, 2) - 0.5) * 0.08
    out = rgba(LEAF.shade(t, 0.4), np.where(cov, 255, 0))
    out[~cov, :3] = 0
    return out


def paint_oak_leaves(seed):
    return leaves(seed, "oak", 0.14)


def paint_spruce_leaves(seed):
    return leaves(seed, "spruce", 0.12, bright=-0.06)


def paint_birch_leaves(seed):
    return leaves(seed, "birch", 0.15, bright=0.05)


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
    out = rgba(out_rgb(CACTUS_C, idx))
    out[:, :8, 3] = 0
    out[:, 120:, 3] = 0
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
    r = rng(seed)
    cidx = np.full((S, S), -1, np.int32)
    blades = []
    for k in range(40):
        x0 = r.uniform(10, 118)
        central = 1 - abs(x0 - 64) / 70
        h = r.uniform(40, 60) + central * r.uniform(20, 52)
        lean = (x0 - 64) * r.uniform(0.35, 0.7) + r.uniform(-8, 8)
        lean = float(np.clip(x0 + lean, 3, 125) - x0)
        blades.append((r.random() * 0.5 + (1 - h / 112), x0, h, lean))
    # Tall ones behind, short ones in front.
    for depth, x0, h, lean in sorted(blades):
        pts = [(x0, 128), (x0 + lean * 0.25, 128 - h * 0.5), (x0 + lean * 0.65, 128 - h * 0.85),
               (x0 + lean, 128 - h)]
        m, lt = curve(pts, r.uniform(5, 7), 1.2)
        base = 2 if depth > 0.5 else 1
        tones = (base - 1, base + 1, base + 3)
        val = np.where(lt > 0.3, tones[0], np.where(lt < -0.2, tones[2], tones[1]))
        # Lower parts sit in the shade of the tuft.
        val = np.where(YY > 128 - h * 0.35, np.maximum(val - 1, 0), val)
        cidx[m] = val[m]
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
    cidx = np.full((S, S), -1, np.int32)
    trunk = [(64, 128), (62, 96), (58, 64), (56, 36), (54, 12)]
    twigs = [((62, 108), (34, 94)), ((63, 100), (94, 88)), ((60, 86), (30, 74)),
             ((59, 74), (98, 54)), ((58, 60), (38, 50)), ((57, 50), (84, 28)),
             ((56, 36), (40, 14)), ((57, 26), (76, 6)), ((60, 80), (82, 78)),
             ((58, 66), (22, 54))]
    for p0, p1 in twigs:
        m, lt = seg(p0, p1, 8, 1.5)
        draw(cidx, m, lt, (0, 1, 2))
    m, lt = curve(trunk, 18, 4)
    val = np.where(lt < -0.2, 2, np.where(lt > 0.4, 0, 1))
    val = np.where((lt < -0.4) & (YY < 70), 3, val)
    val = np.where((YY < 60) & (lt < 0.3) & (val == 1), 2, val)
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


def loop_field(a: np.ndarray, b: np.ndarray, p: float) -> np.ndarray:
    """Blend of two fields around a circle in time: loops when p goes 0 -> 1."""
    return 0.5 + (a - 0.5) * np.cos(TAU * p) + (b - 0.5) * np.sin(TAU * p)


WATER_C = pal("#a5a5a5", "#aeaeae", "#c2c2c2", "#d3d3d3", "#ffffff")


def paint_water_still(seed):
    A1, A2 = streaks(seed, 4, 28, 2), streaks(seed + 1, 4, 28, 2)
    H1 = anoise(seed + 2, 3, 26)
    H2 = anoise(seed + 3, 3, 21.3)
    frames = []
    for f in range(32):
        p = f / 32.0
        base = loop_field(A1, A2, p)
        base = np.roll(base, 4 * f, 1)
        idx = np.where(base > np.quantile(base, 0.45), 1, 0)
        h = np.maximum(np.roll(H1, 4 * f, 1), np.roll(np.roll(H2, -4 * f, 1), 4 * f, 0))
        idx = np.where(h > 0.84, 2, idx)
        idx = np.where(h > 0.91, 3, idx)
        idx = np.where(h > 0.955, 4, idx)
        frames.append(opaque_rgba(out_rgb(WATER_C, idx)))
    return frames, 2


LAVA_C = pal("#c73405", "#cc4108", "#d3530d", "#d96415", "#df7c23", "#e59433", "#ebad44",
             "#f2cd5b")


def paint_lava_still(seed):
    A1 = fbm(seed, 32, 2)
    A2 = fbm(seed + 1, 32, 2)
    B = np.tile(fbm(seed + 2, 16, 2, 64, 64), (2, 2))
    r = rng(seed + 3)
    spots = [(r.random() * S, r.random() * S, r.uniform(9, 13), r.random()) for _ in range(11)]
    frames = []
    for f in range(32):
        p = f / 32.0
        a = loop_field(A1, A2, p)
        b = np.roll(np.roll(B, 2 * f, 0), 2 * f, 1)
        field = a * 0.75 + b * 0.25
        # Bright bubbles swelling and fading.
        for cx, cy, rad, ph in spots:
            hgt = 0.5 + 0.5 * np.sin(TAU * (p + ph))
            rr, _, _ = blob(cx, cy, rad, rad)
            field = np.maximum(field, (1 - np.clip(rr, 0, 1) ** 2) * (0.35 + 0.6 * hgt)
                               + 0.25 * (rr < 1))
        frames.append(opaque_rgba(out_rgb(LAVA_C, levels(field, LAVA_C,
                                                          [3, 15, 31, 17, 17, 10, 5, 2]))))
    return frames, 3


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
    "block/oak_log": paint_oak_log,
    "block/oak_log_top": paint_oak_log_top,
    "block/spruce_log": paint_spruce_log,
    "block/spruce_log_top": paint_spruce_log_top,
    "block/birch_log": paint_birch_log,
    "block/birch_log_top": paint_birch_log_top,
    "block/oak_leaves": paint_oak_leaves,
    "block/spruce_leaves": paint_spruce_leaves,
    "block/birch_leaves": paint_birch_leaves,
    "block/cactus_side": paint_cactus_side,
    "block/cactus_top": paint_cactus_top,
    "block/short_grass": paint_short_grass,
    "block/poppy": paint_poppy,
    "block/dandelion": paint_dandelion,
    "block/dead_bush": paint_dead_bush,
    "block/oak_sapling": paint_oak_sapling,
    "block/birch_sapling": paint_birch_sapling,
    "block/spruce_sapling": paint_spruce_sapling,
    "block/water_still": paint_water_still,
    "block/lava_still": paint_lava_still,
}
