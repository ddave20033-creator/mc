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

from common import S, fbm, grid, hexc, noise, pix, rgba, rng

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


# ---------------------------------------------------------------------------- stone & ores

STONE_C = pal("#686868", "#747474", "#7f7f7f", "#8f8f8f")


def stone_rgb(seed: int) -> np.ndarray:
    f = streaks(seed, 7, 24, 3) * 0.7 + streaks(seed + 1, 3, 8, 2) * 0.3
    f = f + (q(pix(seed + 2, 1), 1, 2) - 0.5) * 0.05
    return out_rgb(STONE_C, levels(f, STONE_C, [7, 33, 42, 18]))


STONE_SEED = seed_of("block/stone")


def paint_stone(seed):
    return opaque_rgba(stone_rgb(seed))


def ore(seed: int, nuggets, cols, shadow="#5c5c5c", halo="#686868", p=2.0, inner=None,
        edge_w=1.6, hi=(0.55, 0.8), jitter=0.18, alt=None, blend=None, grain=0.4):
    """Stone with mineral nuggets. `nuggets`: (cx, cy, w, h[, rot[, alt share]]) in pixels;
    `cols`: dark rim, mid, bright, highlight. With `alt` (a second 4-color palette) part of
    each nugget (its alt share, split across a random direction) is the other mineral, with
    `blend` along the boundary. The interior is mottled with 2 px speckle."""
    out = stone_rgb(STONE_SEED)
    C = pal(*cols)
    A = pal(*alt) if alt is not None else None
    sh = hexc(shadow)
    ha = hexc(halo) if halo is not None else None
    mott = streaks(seed + 7, 3, 6, 2) * (1 - grain) + pix(seed + 8, 2) * grain
    rr_ = rng(seed + 9)
    for i, n in enumerate(nuggets):
        cx, cy, w, h = n[:4]
        rot = n[4] if len(n) > 4 else 0.0
        share = n[5] if len(n) > 5 else 0.0
        rx, ry = w / 2, h / 2
        rmin = min(rx, ry)
        r, dx, dy = blob(cx, cy, rx, ry, rot, p, jitter=jitter, jseed=seed + i, jcell=4)
        br = np.clip((dx / rx + dy / ry) * 0.7 + 0.3, 0, 1)  # bottom-right side
        ring = (r > 1) & (r <= 1 + (0.8 + 1.8 * br) / rmin)
        out[ring & (br > 0.35)] = sh
        if ha is not None:
            out[ring & (br <= 0.35)] = ha
        inside = r <= 1
        light = -(dx / rx + dy / ry) * 0.5
        t = (1 - r) * 0.6 + light * 0.5 + (mott - 0.5) * 0.7
        idx = bands(t, [0.2, hi[0], hi[1]]) + 1
        rim = r > 1 - (edge_w + 2.2 * br) / rmin
        idx = np.where(rim, 0, idx)
        col = C[np.clip(idx, 0, len(C) - 1)]
        if A is not None and share > 0:
            ang = rr_.uniform(0, TAU)
            sd = (dx * np.cos(ang) + dy * np.sin(ang)) / max(rx, ry)
            sd = sd + (noise(seed + 50 + i, 4) - 0.5) * 0.5
            th = np.quantile(sd[inside], 1 - share) if inside.any() else 9
            am = sd > th
            col = np.where(am[..., None], A[np.clip(idx, 0, 3)], col)
            if blend is not None:
                bd = np.abs(sd - th) < 0.08
                col = np.where((bd & ~rim)[..., None], hexc(blend), col)
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
               inner="#363636", hi=(0.45, 0.78), jitter=0.3, grain=0.25)


def paint_iron_ore(seed):
    # Slanted flakes rising to the right, tan with pale speckle and a brown lower rim.
    nug = [(25, 22, 32, 14, -0.3), (95, 12, 30, 13, -0.25), (66, 42, 64, 22, -0.28),
           (113, 44, 16, 10, -0.3), (35, 75, 38, 18, -0.22), (88, 70, 34, 15, -0.3),
           (43, 98, 20, 10, -0.3), (18, 111, 26, 11, -0.35), (92, 102, 44, 20, -0.3)]
    return ore(seed, nug, ("#887455", "#af8e77", "#d8af93", "#e2c0aa"), "#77674f", None,
               p=1.15, hi=(0.5, 0.78), edge_w=1.2)


def paint_copper_ore(seed):
    # Rhombic chunks of green-teal patina and salmon-orange copper, olive where they meet.
    nug = [(26, 18, 18, 9, -0.3, 0.0), (96, 20, 16, 9, -0.35, 1.0),
           (46, 42, 50, 24, -0.25, 0.65), (96, 62, 44, 28, -0.1, 0.4),
           (28, 76, 38, 22, -0.15, 0.1), (58, 100, 38, 22, -0.2, 0.55),
           (22, 106, 20, 12, -0.2, 0.0), (110, 104, 20, 12, 0.0, 0.0)]
    return ore(seed, nug, ("#3a685a", "#3a7663", "#599581", "#4fba98"), "#5c5c5c", "#a2a2a2",
               p=1.05, hi=(0.35, 0.7), alt=("#c16746", "#e0734d", "#e0734d", "#f38268"),
               blend="#818058", edge_w=1.4, jitter=0.12)


def paint_gold_ore(seed):
    nug = [(35, 13, 10, 10), (48, 42, 62, 28, -1.05), (108, 29, 24, 22), (112, 60, 16, 8),
           (25, 73, 24, 16), (92, 94, 46, 30, -0.3), (32, 107, 36, 24, 0.2), (57, 100, 10, 8),
           (115, 117, 10, 14)]
    return ore(seed, nug, ("#9c7020", "#eb9d0e", "#fcee4b", "#ffffb5"), "#5c5c5c", "#a2a2a2",
               p=1.15, hi=(0.55, 0.95))


def paint_diamond_ore(seed):
    nug = [(38, 20, 12, 8), (105, 24, 24, 10, -0.2), (66, 28, 20, 8), (49, 49, 40, 16, -0.15),
           (92, 47, 18, 14), (19, 67, 20, 12, -0.2), (77, 74, 38, 16, -0.1), (38, 84, 12, 8),
           (91, 100, 50, 22, -0.25), (36, 108, 24, 8)]
    return ore(seed, nug, ("#239698", "#1ed0d6", "#77e7d1", "#d5fff6"), "#676767", "#8dadb1",
               p=1.25, hi=(0.45, 0.8))


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
    # Lumpy clods on a jittered grid: 7 x 7 domes of 20-30 px.
    n = 7
    for i in range(n):
        for j in range(n):
            cx = (j + 0.5 + 0.5 * (i % 2) + (r.random() - 0.5) * 0.6) * S / n
            cy = (i + 0.5 + (r.random() - 0.5) * 0.6) * S / n
            rad = r.uniform(10, 13.5)
            items.append((cx, cy, rad * r.uniform(1.0, 1.25), rad, r.uniform(0.7, 1.0)))
    H, ids, L, R = bumps(seed, items, jitter=0.2)
    # Lit on the upper left rim (a light curl), darker below, 2 px speckle.
    t = L * 0.8 * np.sqrt(np.clip(R, 0, 1)) + (1 - np.clip(R, 0, 1)) * 0.2 + (noise(seed + 2, 8) - 0.5) * 0.35
    t += (pix(seed + 3, 2) - 0.5) * 0.3 + (pix(seed + 4, 1) - 0.5) * 0.08
    crev = ((R > 0.84) & (L < 0.1)) | (ids < 0)
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


def grass_top_idx(seed: int) -> np.ndarray:
    """Seen from above: mottled grass made of many short blade dashes (2 px wide, mostly
    diagonal, lying in patches of similar direction) - light blades with the odd pale tip and
    dark gaps between them."""
    r = rng(seed)
    patch = noise(seed + 1, 32) * 0.6 + noise(seed + 4, 16) * 0.4
    grain = pix(seed + 2, 2)
    t = patch * 0.7 + grain * 0.3
    cidx = np.where(t > 0.4, 2, 1).astype(np.int32)
    dirf = noise(seed + 3, 32)
    for k in range(1100):
        x, y = r.random() * S, r.random() * S
        dsel = dirf[int(y) % S, int(x) % S]
        a0 = 0.8 if (dsel + r.uniform(-0.45, 0.45)) > 0.5 else -0.8
        ang = a0 + r.uniform(-0.35, 0.35)
        ln = r.uniform(5, 10)
        x1, y1 = x + np.cos(ang) * ln, y + np.sin(ang) * ln
        idx, m, tt = stroke_mask(x, y, x1, y1, 2.0)
        light = patch[int(y) % S, int(x) % S] + r.uniform(-0.3, 0.3)
        u = r.random()
        if u < 0.28:
            val = 0 if r.random() < 0.45 else 1  # a dark gap between blades
        elif light > 0.66:
            val = np.where(tt > 0.7, 5 if r.random() < 0.3 else 4, 4)
        else:
            val = np.where(tt > 0.7, 4, 3)
        stamp(cidx, idx, m, val)
    return cidx


def paint_grass_block_top(seed):
    return opaque_rgba(out_rgb(GRASS_C, grass_top_idx(seed)))


def paint_grass_block_side_overlay(seed):
    r = rng(seed)
    # Fringe: a ragged band ~16 px deep with pointed drips of blades hanging below it, their
    # edges stepped in 2 px (Faithful's pixel steps at double resolution).
    h = np.full(S, 15.0, np.float32)
    h += (np.repeat(r.random(S // 2), 2) - 0.5) * 4
    x = float(r.integers(0, 6))
    while x < S:
        w = r.uniform(3.5, 7.5)
        ln = r.uniform(3, 16) if r.random() < 0.8 else r.uniform(16, 24)
        for k in range(-int(w) - 1, int(w) + 2):
            xi = int(x + k) % S
            h[xi] = max(h[xi], 14 + ln * max(0.0, 1 - abs(k) / w) ** 0.4)
        x += r.uniform(6, 13)
    h = np.round(h / 2) * 2
    mask = YY < h[None, :]
    # Inside: vertical blade streaks, lighter at the top, a darker shade line at the bottom.
    f = anoise(seed + 2, 24, 2) * 0.55 + anoise(seed + 3, 8, 2) * 0.3 + q(pix(seed + 4, 1), 3, 1) * 0.15
    f = f + np.clip((12 - YY) / 12, 0, 1) * 0.12
    idx = levels(f, GRASS_C, [10, 22, 27, 25, 16], mask)
    idx = np.where(mask & (YY > h[None, :] - 3), np.maximum(idx - 1, 0), idx)
    idx = np.where(mask & (YY > h[None, :] - 5) & (YY < h[None, :] - 2) & (h[None, :] > 20)
                   & (f > np.quantile(f, 0.5)), idx + 1, idx)
    out = rgba(out_rgb(GRASS_C, idx), np.where(mask, 255, 0))
    out[~mask, :3] = 0
    return out


SNOW_C = pal("#b2d5d5", "#d7efef", "#f0fdfd", "#f7fefe", "#ffffff")


def snow_field(seed: int) -> np.ndarray:
    # Soft drifts with faint wind streaks rising to the right.
    return (streaks(seed, 8, 16, 3) * 0.55 + shear_v(anoise(seed + 2, 4, 32), -1) * 0.35
            + (pix(seed + 1, 1) - 0.5) * 0.08)


def paint_snow(seed):
    f = snow_field(seed)
    return opaque_rgba(out_rgb(SNOW_C[2:], levels(f, SNOW_C[2:], [20, 41, 38])))


def paint_grass_block_snow(seed):
    out = dirt_rgb(seed_of("block/dirt"))
    r = rng(seed)
    # The dirt's top edge is a row of rounded clod tops; snow fills everything above it.
    xs = np.arange(S) + 0.5
    edge = np.full(S, 50.0)
    x = 0.0
    while x < S + 20:
        w = r.uniform(18, 32)
        pk = r.uniform(7, 14)
        for k in range(S):
            d = abs(((xs[k] - x + S / 2) % S) - S / 2)
            if d < w / 2:
                edge[k] = min(edge[k], 50 - pk * np.sqrt(1 - (2 * d / w) ** 2))
        x += w * r.uniform(0.9, 1.3)
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
    """Rounded pebbles of mixed size packed in a speckled darker matrix: each pebble mid grey
    with a lighter top-left cap and a dark bottom-right rim, some warm pinkish."""
    ids, d1, border, dy, dx = cells(seed, 52, jitter=0.95, warp=2.0, warp_cell=16)
    n = ids.max() + 1
    r = rng(seed + 1)
    mr = ndimage.mean(d1 + border, ids, np.arange(n))  # mean radius per cell
    R = (np.asarray(mr) * r.uniform(1.05, 1.25, n))[ids]
    wob = (noise(seed + 5, 4) - 0.5) * 2.0
    nr = (d1 + wob) / np.maximum(R, 1e-3)
    peb = (nr < 1.0) & (border > 0.7 + (pix(seed + 7, 1) > 0.5))
    lit = -(dx + dy) / np.maximum(R, 1) / 1.4
    bright = r.uniform(-0.12, 0.12, n)[ids]
    t = 0.5 + lit * 0.4 + bright + (pix(seed + 2, 2) - 0.5) * 0.2
    idx = bands(t, [0.1, 0.3, 0.64, 0.86]).astype(np.int32)
    idx = np.where(peb & (nr > 0.78) & (dx + dy > 0), np.minimum(idx, 1), idx)
    # Matrix between pebbles.
    mt = noise(seed + 3, 8) * 0.5 + pix(seed + 4, 2) * 0.5
    midx = np.where(mt > 0.55, 2, 1)
    midx = np.where(mt < 0.2, 0, midx)
    idx = np.where(peb, idx, midx)
    warm = (r.random(n) < 0.35)[ids] & peb
    warm |= ~peb & (noise(seed + 6, 16) > 0.55)
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
    """Smooth blue-grey clay: faint overlapping scallops (broken darker arcs along their lower
    edges), checker-dithered pale patches on their lit sides, bluish mottling and a few
    bright blue specks."""
    H, ids, L, R = scales(seed, 11, 15, 12)
    chk = ((YY.astype(int) // 2 + XX.astype(int) // 2) % 2 == 0)
    idx = np.full((S, S), 3, np.int32)
    blue = noise(seed + 1, 16) * 0.6 + pix(seed + 2, 2) * 0.4
    idx = np.where(blue > 0.62, 2, idx)
    idx = np.where((blue > 0.55) & (blue <= 0.62) & chk, 2, idx)
    pale = (L > 0.15) & (R > 0.35) & (R < 0.85) & (noise(seed + 3, 8) > 0.45)
    idx = np.where(pale & (chk | (pix(seed + 4, 2) > 0.6)), 4, idx)
    rim = (R > 0.74) & (L < 0.0) & (noise(seed + 5, 8) > 0.3)
    idx = np.where(rim, np.where(pix(seed + 6, 2) > 0.6, 0, 1), idx)
    idx = np.where(rim & (R > 0.95), 1, idx)
    r = rng(seed + 7)
    for _ in range(12):
        y, x = int(r.integers(0, S // 2)) * 2, int(r.integers(0, S // 2)) * 2
        idx[y:y + 2, x:x + 2] = 5
        if r.random() < 0.5:
            idx[y, (x + 2) % S] = 4
    return opaque_rgba(out_rgb(CLAY_C, idx))


ICE_C = pal("#7fa7f7", "#86aefd", "#8cb3fe", "#92b9fe", "#a1c3ff", "#bcd4ff", "#c8dcff")


def shear_v(a: np.ndarray, k: float) -> np.ndarray:
    """Column x rolled down by round(k*x) pixels (tiles when k is a whole number)."""
    out = np.empty_like(a)
    for x in range(a.shape[1]):
        out[:, x] = np.roll(a[:, x], int(round(k * x)))
    return out


def paint_ice(seed):
    r = rng(seed)
    # Soft broad bands rising to the right, a few pixels of grain.
    f = shear_v(anoise(seed, 11, 43), -1) * 0.45 + shear_v(anoise(seed + 5, 4, 21.3), -1) * 0.12
    f = f + noise(seed + 4, 32) * 0.45 + (pix(seed + 1, 1) - 0.5) * 0.04
    idx = levels(f, ICE_C, [5, 20, 32, 33, 10])
    # A handful of curved light streaks along the same direction, pale in the middle.
    for k in range(5):
        x, y = r.random() * S, r.random() * S
        ln = r.uniform(28, 56)
        ang = -0.7 + r.uniform(-0.12, 0.12)
        bend = r.uniform(-0.025, 0.025)
        n = int(ln // 3)
        w = r.uniform(2.0, 3.0)
        for i in range(n):
            a0 = ang + bend * i
            x1, y1 = x + np.cos(a0) * 3, y + np.sin(a0) * 3
            fpos = (i + 0.5) / n
            ww = w * (0.6 + 0.8 * np.sin(np.pi * fpos))
            ix, m, _ = stroke_mask(x, y, x1, y1, ww)
            stamp(idx, ix, m, 4)
            if 0.3 < fpos < 0.7:
                ix, m, _ = stroke_mask(x, y, x1, y1, max(1.0, ww - 1.4))
                stamp(idx, ix, m, 5 if (k % 3) else 6)
            x, y = x1, y1
    return opaque_rgba(out_rgb(ICE_C, idx))


BEDROCK_C = pal("#222222", "#333333", "#575757", "#636363", "#979797")


def paint_bedrock(seed):
    f = streaks(seed, 8, 24, 3) * 0.7 + streaks(seed + 1, 3, 10, 2) * 0.3
    f = f + (q(pix(seed + 2, 1), 2, 4) - 0.5) * 0.1
    return opaque_rgba(out_rgb(BEDROCK_C, levels(f, BEDROCK_C, [6, 28, 32, 25, 9])))


OBSIDIAN_C = pal("#000001", "#06030b", "#100c1c", "#271e3d", "#3b2754")


def paint_obsidian(seed):
    """Glassy black lumps overlapping like twisted rope, each with a purple crescent of light
    along its curved upper edge."""
    r = rng(seed)
    bgn = noise(seed + 1, 16) * 0.7 + pix(seed + 2, 2) * 0.3
    idx = np.where(bgn > 0.5, 1, 0).astype(np.int32)
    H = np.full((S, S), -1.0, np.float32)
    pts = spread(seed + 3, 17)
    for k, (cx, cy) in enumerate(pts):
        rx, ry = r.uniform(19, 25), r.uniform(10, 13)
        rot = r.uniform(-0.55, 0.55)
        bend = r.uniform(0.45, 0.85)
        hgt = r.random()
        dy, dx = wdelta(cy, cx)
        c, s_ = np.cos(rot), np.sin(rot)
        u = (dx * c + dy * s_) / rx
        v = (-dx * s_ + dy * c) / ry + bend * u * u  # ends droop: a banana/crescent lump
        v = v + (noise(seed + 10 + k, 8) - 0.5) * 0.25
        rr = u * u + v * v
        m = (rr < 1) & (hgt > H)
        if not m.any():
            continue
        H = np.where(m, hgt, H)
        t = np.full((S, S), 2, np.int32)
        t = np.where((v > 0.3) | ((u > 0.6) & (v > -0.1)), 1, t)  # shaded underside / end
        t = np.where((v > 0.8) & (u > 0), 0, t)
        cres = (v < -0.38) & (v > -0.94) & (np.abs(u) < 0.85 - 0.2 * np.abs(v))
        t = np.where(cres, 3, t)
        core = (v < -0.56) & (v > -0.86) & (np.abs(u) < 0.6 - 0.3 * np.abs(u))
        t = np.where(core, 4, t)
        idx = np.where(m, t, idx)
    # Dark seam where lumps overlap (top edge of each visible lump part).
    edge = (H >= 0) & (np.roll(H, 1, 0) > H + 0.001)
    idx = np.where(edge, 0, idx)
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
    bg = streaks(seed + 1, 6, 10, 2) * 0.8 + pix(seed + 3, 2) * 0.2
    idx = np.where(bg > 0.5, 1, 0)
    idx = np.where(bg > 0.66, 3, idx)
    blobm = ids >= 0
    # A thin warm glow ring around each nugget, darker brown shadow to its bottom right.
    halo = ~blobm & wgrow(blobm, 2)
    idx = np.where(halo, 2, idx)
    shadow = ~blobm & np.roll(np.roll(blobm, 2, 0), 2, 1)
    idx = np.where(shadow, 0, idx)
    br = np.clip(-L * 1.4 + 0.3, 0, 1)
    rim = blobm & (R > 1 - (0.08 + 0.3 * br))
    core = blobm & (np.hypot(NX + 0.3, NY + 0.32) < 0.55 + (pix(seed + 2, 2) - 0.5) * 0.12)
    spot = blobm & (np.hypot(NX + 0.42, NY + 0.42) < 0.2)
    idx = np.where(blobm, 5, idx)
    idx = np.where(core, 6, idx)
    idx = np.where(spot, 7, idx)
    idx = np.where(rim, 4, idx)
    return opaque_rgba(out_rgb(GLOW_C, idx))


# ---------------------------------------------------------------------------- logs


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


def loop_field(a: np.ndarray, b: np.ndarray, p: float) -> np.ndarray:
    """Blend of two fields around a circle in time: loops when p goes 0 -> 1."""
    return 0.5 + (a - 0.5) * np.cos(TAU * p) + (b - 0.5) * np.sin(TAU * p)


WATER_C = pal("#a5a5a5", "#aeaeae", "#c2c2c2", "#d3d3d3", "#ffffff")


def paint_water_still(seed):
    """Two flat greys in long horizontal step-edged ripples drifting sideways, and a few
    wave crests (lighter bands with a bright core) that swell and fade as they travel."""
    A1, A2 = anoise(seed, 6, 32), anoise(seed + 1, 6, 32)
    B1, B2 = anoise(seed + 5, 3, 16), anoise(seed + 6, 3, 16)
    r = rng(seed + 2)
    crests = [(r.random() * S, (i + r.uniform(0.2, 0.8)) * S / 5, r.uniform(26, 40),
               r.uniform(5, 8), r.random(), 1 if i % 2 else -1) for i in range(5)]
    C1, C2 = anoise(seed + 7, 2.5, 21.3), anoise(seed + 8, 2.5, 21.3)
    frames = []
    for f in range(32):
        p = f / 32.0
        base = loop_field(A1, A2, p) * 0.75 + loop_field(B1, B2, p) * 0.25
        base = np.roll(base, 4 * f, 1)
        # Snap the ripple edges to 2 px steps horizontally (pixel-art ripples).
        base = q(base, 1, 2)
        idx = np.where(base > np.quantile(base, 0.47), 1, 0)
        hi = np.roll(q(loop_field(C1, C2, p), 1, 2), 4 * f, 1)
        for cx, cy, rx, ry, ph, d in crests:
            life = np.sin(np.pi * ((p + ph) % 1.0))  # 0 -> 1 -> 0 over the loop
            if life < 0.1:
                continue
            x = cx + d * 4 * f
            rr, dx, dy = blob(x, cy, rx * (0.6 + 0.4 * life), ry)
            env = np.clip(1 - rr, 0, 1) ** 0.6 * life  # where this crest is, how strong
            v = hi + env * 0.9
            idx = np.where((v > 0.8) & (env > 0), np.maximum(idx, 2), idx)
            idx = np.where((v > 1.1) & (env > 0), 3, idx)
            idx = np.where((v > 1.34) & (env > 0), 4, idx)
        frames.append(opaque_rgba(out_rgb(WATER_C, idx)))
    return frames, 2


LAVA_C = pal("#c73405", "#cc4108", "#d3530d", "#d96415", "#df7c23", "#e59433", "#ebad44",
             "#f2cd5b")


def paint_lava_still(seed):
    """Blotchy molten surface: glowing blobs (yellow cores stepping out through orange
    rings) that drift in small circles and swell and dim, over an orange-red body with darker
    crusting patches that slowly morph. Palette steps are fixed across frames (no flicker)."""
    A1, A2 = fbm(seed, 32, 3), fbm(seed + 1, 32, 3)
    C1, C2 = fbm(seed + 4, 16, 2), fbm(seed + 5, 16, 2)
    r = rng(seed + 3)
    spots = []
    for i, (cx, cy) in enumerate(spread(seed + 6, 11)):
        spots.append((cx, cy, r.uniform(11, 21), r.random(), r.uniform(2, 5), r.random()))
    fields = []
    for f in range(32):
        p = f / 32.0
        field = loop_field(A1, A2, p) * 0.55 + loop_field(C1, C2, p) * 0.25
        glow = np.zeros((S, S), np.float32)
        for k, (cx, cy, rad, ph, orb, ph2) in enumerate(spots):
            hgt = 0.55 + 0.45 * np.sin(TAU * (p + ph))
            x = cx + orb * np.cos(TAU * (p + ph2))
            y = cy + orb * np.sin(TAU * (p + ph2))
            rr, _, _ = blob(x, y, rad, rad * 0.85, rot=ph * 3, jitter=0.25,
                            jseed=seed + 40 + k, jcell=8)
            glow = np.maximum(glow, np.clip(1 - rr, 0, 1) ** 1.1 * hgt)
        fields.append(field + glow * 0.8)
    th = np.quantile(fields[0], np.cumsum([3, 15, 31, 17, 17, 10, 5])[:] / 100.0)
    frames = [opaque_rgba(out_rgb(LAVA_C, np.searchsorted(th, fl))) for fl in fields]
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
