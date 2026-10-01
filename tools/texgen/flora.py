"""Trees and plants in the game's flat style (see BRIEF.md): bark, round log ends, leaves,
cactus, flowers, grass and saplings.

- Log sides wrap round a round trunk (u three times round a trunk, once round a branch), so
  they tile both ways: oak has long lens-shaped furrows between ridges, spruce round
  overlapping scales, birch white bark with dark marks.
- Log tops: the game shows the circle of radius 0.47 * 128 as the cut end: circular rings,
  a pith, one radial crack and a bark rim.
- Leaves are tinted by the game (neutral greys): overlapping flat leaves, each one tone with
  a light rim and a vein, on a dark shadow tone, with real holes between the clusters.
- Plants are sprites standing on the bottom edge, with a thin outline in a dark tone of
  their own colour.
"""

from __future__ import annotations

import numpy as np
from scipy import ndimage

from flat import (
    S,
    SS,
    Canvas,
    _d,
    box,
    capsule,
    disk,
    ellipse,
    hexc,
    lerp,
    moved_shape,
    poly,
    ring,
    scatter,
    shift,
    tones,
)

C = S * 0.5

# ---------------------------------------------------------------------------- palette

OAK_WOOD, OAK_RING, OAK_BARK = "#c99a5e", "#a8773f", "#6b4a30"
SPRUCE_WOOD, SPRUCE_BARK = "#a87850", "#4a3426"
BIRCH_BARK, BIRCH_MARK, BIRCH_WOOD = "#e8e2d6", "#3d3a36", "#e2c48f"
CACTUS, CACTUS_DARK, CACTUS_LIGHT = "#5f9a45", "#3f7a35", "#8cc56a"
POPPY, DANDELION, DEAD_BUSH = "#d8423a", "#f2c935", "#8a6a40"
STEM = "#5a9440"  # flower stems and sapling leaves (not tinted)


def grey(v: float) -> np.ndarray:
    return np.array([v, v, v], np.float32)


# ---------------------------------------------------------------------------- shapes


def leaf(cx, cy, length, width, angle, tile=True, round_=0.75):
    """A flat leaf from its base (cx, cy) towards `angle` (radians, 0 = right, y down):
    rounded at the base, pointed at the tip. `round_` < 1 fattens it."""
    ca, sa = np.cos(angle), np.sin(angle)

    def f(y, x):
        dx, dy = _d(x, cx, S, tile), _d(y, cy, S, tile)
        u = dx * ca + dy * sa
        v = -dx * sa + dy * ca
        t = np.clip(u / length, 0.0, 1.0)
        half = 0.5 * width * np.sin(np.pi * t ** 0.8) ** round_
        return (u >= 0) & (u <= length) & (np.abs(v) <= half)

    return f


def half_leaf(cx, cy, length, width, angle, side, tile=True, round_=0.75):
    """One side (+1 / -1) of `leaf` (its other half is drawn in another tone)."""
    ca, sa = np.cos(angle), np.sin(angle)
    whole = leaf(cx, cy, length, width, angle, tile, round_)

    def f(y, x):
        dx, dy = _d(x, cx, S, tile), _d(y, cy, S, tile)
        v = -dx * sa + dy * ca
        return whole(y, x) & (v * side >= 0)

    return f


def wedge(cx, cy, r0, r1, angle, w0, w1):
    """A tapering radial strip (a crack) from radius r0 (w0 wide) to r1 (w1 wide)."""
    ca, sa = np.cos(angle), np.sin(angle)
    nx, ny = -sa, ca
    pts = [
        (cx + ca * r0 + nx * w0 / 2, cy + sa * r0 + ny * w0 / 2),
        (cx + ca * r1 + nx * w1 / 2, cy + sa * r1 + ny * w1 / 2),
        (cx + ca * r1 - nx * w1 / 2, cy + sa * r1 - ny * w1 / 2),
        (cx + ca * r0 - nx * w0 / 2, cy + sa * r0 - ny * w0 / 2),
    ]
    return poly(pts)


def blade(x0, y0, x1, y1, w0, bend=0.0, n=12):
    """A tapering curved blade from a base (x0, y0) of width w0 to a point (x1, y1), bowed
    sideways by `bend` px (not wrapping: sprites)."""
    ts = np.linspace(0, 1, n)
    dx, dy = x1 - x0, y1 - y0
    ln = max(np.hypot(dx, dy), 1e-6)
    nx, ny = -dy / ln, dx / ln
    cx = x0 + dx * ts + nx * bend * np.sin(np.pi * ts * 0.9)
    cy = y0 + dy * ts + ny * bend * np.sin(np.pi * ts * 0.9)
    hw = w0 * 0.5 * (1 - ts) ** 0.9
    left = [(cx[i] + nx * hw[i], cy[i] + ny * hw[i]) for i in range(n)]
    right = [(cx[i] - nx * hw[i], cy[i] - ny * hw[i]) for i in range(n)]
    return poly(left + right[::-1])


def strokes(segs, width, tile=True):
    """Many straight round-ended strokes (x, y, dx, dy) at once (needles)."""
    sg = np.asarray(segs, np.float32)

    def f(y, x):
        px = _d(x[None], sg[:, 0, None, None], S, tile)
        py = _d(y[None], sg[:, 1, None, None], S, tile)
        dx, dy = sg[:, 2, None, None], sg[:, 3, None, None]
        t = np.clip((px * dx + py * dy) / np.maximum(dx * dx + dy * dy, 1e-6), 0, 1)
        return (((px - t * dx) ** 2 + (py - t * dy) ** 2) <= (width * 0.5) ** 2).any(0)

    return f


def stroke(c: Canvas, pts, width, color, tile=False):
    """A polyline of round capsules."""
    for a, b in zip(pts[:-1], pts[1:]):
        c.fill(capsule(a, b, width, tile), color)


def lens(cx, cy, ry, rx, p=0.8, tile=True):
    """An upright lens: round in the middle, pointed at the top and bottom."""

    def f(y, x):
        dy, dx = _d(y, cy, S, tile) / ry, _d(x, cx, S, tile) / rx
        return np.abs(dx) <= np.clip(1 - dy * dy, 0, 1) ** p

    return f


# ---------------------------------------------------------------------------- windows
# Shapes evaluated only in a window round them (much faster than the whole canvas); on a
# tiling canvas the window wraps round the edges.


def window(c: Canvas, cx, cy, rx, ry=None):
    ry = rx if ry is None else ry

    def axis(lo, hi, n):
        a, b = int(np.floor(lo * SS)), int(np.ceil(hi * SS))
        if b - a >= n * SS:
            a, b = 0, n * SS
        elif not c.tile:
            a, b = max(a, 0), min(b, n * SS)
        idx = np.arange(a, b)
        return idx % (n * SS), (idx.astype(np.float32) + 0.5) / SS

    (rows, ys), (cols, xs) = axis(cy - ry, cy + ry, c.h), axis(cx - rx, cx + rx, c.w)
    yy, xx = np.meshgrid(ys, xs, indexing="ij")
    return rows, cols, yy, xx


def wmask(c: Canvas, win, shape) -> np.ndarray:
    """`shape` in the window, at unwrapped coordinates (so a shape near the window need
    not wrap itself: shapes made with tile=False are fine and faster)."""
    return shape(win[2], win[3])


def wfill(c: Canvas, win, m: np.ndarray, color) -> None:
    rows, cols = win[0], win[1]
    rr = np.broadcast_to(rows[:, None], m.shape)[m]
    cc = np.broadcast_to(cols[None, :], m.shape)[m]
    c.rgb[rr, cc] = np.asarray(color, np.float32)
    c.alpha[rr, cc] = 1.0


# ---------------------------------------------------------------------------- bark


def paint_oak_log(seed: int) -> np.ndarray:
    """Vertical ridges split by long lens-shaped dark furrows in staggered columns (a
    diamond network); each furrow's right edge (the next ridge's face turned to the light)
    lit, a few ridges a tone lighter or darker."""
    t = tones(OAK_BARK, 5, 0.13)
    c = Canvas(bg=t[2])
    rng = np.random.default_rng(seed)
    cols = 6
    cw = S / cols
    # broad ridges in slightly different tones
    for i in range(cols):
        if rng.random() < 0.5:
            x0 = (i + 0.5) * cw
            c.fill(box(-10, x0 + 2, S + 10, x0 + cw - 2, tile=True), t[1] if rng.random() < 0.5 else t[3])
    furrows = []
    for i in range(cols):
        y = rng.uniform(0, S)
        end = y + S
        while y < end - 10:
            ln = min(rng.uniform(40, 70), end - y)
            cx = i * cw + rng.uniform(-2.5, 2.5)
            furrows.append((cx, y + ln / 2, ln / 2 + 4, rng.uniform(3.2, 4.6)))
            y += ln * rng.uniform(0.7, 0.85)
            # a short side furrow joining the neighbour column now and then
            if rng.random() < 0.45:
                fy = y - ln * 0.4
                side = 1 if rng.random() < 0.5 else -1
                furrows.append((cx + side * cw * 0.5, fy, rng.uniform(12, 18), 2.4))
    for cx, cy, ry, rx in furrows:
        win = window(c, cx, cy, rx + 4, ry + 2)
        wfill(c, win, wmask(c, win, lens(cx + 2.2, cy + 1.0, ry, rx, p=0.7)), t[4])
    for cx, cy, ry, rx in furrows:
        win = window(c, cx, cy, rx + 4, ry + 2)
        wfill(c, win, wmask(c, win, lens(cx, cy, ry, rx, p=0.7)), t[0])
    return c.finish(opaque=True)


def paint_spruce_log(seed: int) -> np.ndarray:
    """Round flaky scales overlapping like shingles (each one over the top of the one
    below), with a cast shadow under each scale's rounded lower edge."""
    t = tones(SPRUCE_BARK, 5, 0.12)
    c = Canvas(bg=t[0])
    rng = np.random.default_rng(seed)
    rows, cols = 8, 5
    rh, cw = S / rows, S / cols
    jit = rng.uniform(-2.0, 2.0, (rows, cols, 2))
    tone = rng.random((rows, cols))

    def draw(j, i, only_top=False):
        cx = (i + 0.5 + (j % 2) * 0.5) * cw + jit[j, i, 0]
        cy = (j + 0.5) * rh + jit[j, i, 1]
        rx, ry = cw * 0.62, rh * 0.95
        win = window(c, cx, cy + 2, rx + 3, ry + 4)
        top = (lambda y, x: y < 64) if only_top else (lambda y, x: np.ones_like(y, bool))
        sh = ellipse(cy + 2.4, cx + 0.6, ry, rx)
        ycan = np.broadcast_to((win[0][:, None] + 0.5) / SS, win[2].shape)
        m = wmask(c, win, sh) & top(ycan, None)
        wfill(c, win, m, t[0])
        sc = ellipse(cy, cx, ry, rx)
        m = wmask(c, win, sc) & top(ycan, None)
        base = t[3] if tone[j, i] < 0.3 else t[2] if tone[j, i] < 0.8 else t[1]
        wfill(c, win, m, base)
        lit = m & ~wmask(c, win, moved_shape(sc, 1.8, 1.8))
        wfill(c, win, lit, t[4])

    for j in range(rows - 1, -1, -1):
        for i in range(cols):
            draw(j, i, only_top=(j == 0))
    for i in range(cols):  # the bottom row again where it wraps over the top row
        draw(rows - 1, i, only_top=True)
    return c.finish(opaque=True)


def paint_birch_log(seed: int) -> np.ndarray:
    """White bark: a few greyer peeling bands, two dark branch scars with their 'brows',
    short dark horizontal lenticels."""
    t = tones(BIRCH_BARK, 4, 0.06)
    c = Canvas(bg=t[2])
    rng = np.random.default_rng(seed)
    mark = hexc(BIRCH_MARK)
    mark2 = lerp(mark, hexc(BIRCH_BARK), 0.35)
    for y in rng.uniform(0, S, 3):
        h = rng.uniform(6, 14)
        c.fill(box(y, -10, y + h, S + 10, tile=True), t[1])
        c.fill(box(y, -10, y + 1.6, S + 10, tile=True), t[3])
    for x, y in scatter(seed + 1, 2, 60):
        rx, ry = rng.uniform(13, 19), rng.uniform(3.2, 4.6)
        c.fill(ellipse(y, x, ry, rx), mark)
        c.fill(capsule((x - rx * 1.4, y - ry * 1.6), (x, y - ry * 0.5), 2.6, tile=True), mark2)
        c.fill(capsule((x + rx * 1.4, y - ry * 1.6), (x, y - ry * 0.5), 2.6, tile=True), mark2)
    for x, y in scatter(seed + 2, 16, 18):
        ln = rng.uniform(6, 18)
        w = rng.uniform(2.6, 3.6)
        c.fill(capsule((x - ln / 2, y), (x + ln / 2, y), w, tile=True), mark)
    return c.finish(opaque=True)


# ---------------------------------------------------------------------------- log ends


def _log_top(seed: int, wood: str, ring_col, bark: str, radii, rim_light=None,
             rim_marks=None) -> np.ndarray:
    rng = np.random.default_rng(seed)
    bt = tones(bark, 4, 0.12)
    wt = tones(wood, 4, 0.08)
    c = Canvas(tile=False, bg=bt[1])
    R = 0.47 * S  # visible cut end
    WR = R - 6.0  # wood inside the bark rim
    # bark rim: lit top-left half, shadow bottom-right half
    c.fill(lambda y, x: (x - C) + (y - C) < 0, bt[2] if rim_light is None else rim_light)
    if rim_marks is not None:
        for a in rng.uniform(0, 2 * np.pi, 9):
            ln = rng.uniform(0.12, 0.3)
            pts = [(C + np.cos(a + k) * (R - 2.5), C + np.sin(a + k) * (R - 2.5))
                   for k in np.linspace(0, ln, 5)]
            stroke(c, pts, 2.4, rim_marks)
    c.fill(disk(C, C, WR + 1.5, tile=False), bt[0])  # thin dark line between bark and wood
    c.fill(disk(C, C, WR, tile=False), wt[3])  # light sapwood band
    c.fill(disk(C, C, WR - 4.0, tile=False), wt[2])
    ring_col = hexc(ring_col) if isinstance(ring_col, str) else ring_col
    for k, r in enumerate(radii):
        w = 2.4 if k % 2 == 0 else 1.6
        c.fill(ring(C, C, r - w / 2, r + w / 2, tile=False), ring_col)
    c.fill(disk(C, C, 3.4, tile=False), ring_col)
    # one radial crack
    a = rng.uniform(0.15, 0.6) * np.pi + (np.pi if rng.random() < 0.5 else 0)
    c.fill(wedge(C, C, 2.0, WR - 6.0, a, 3.6, 0.6), shift(ring_col, dv=-0.22))
    return c.finish(opaque=True)


def paint_oak_log_top(seed: int) -> np.ndarray:
    return _log_top(seed, OAK_WOOD, OAK_RING, OAK_BARK, [9, 17, 24.5, 32, 39.5, 46])


def paint_spruce_log_top(seed: int) -> np.ndarray:
    ring_ = shift(hexc(SPRUCE_WOOD), dv=-0.17, ds=0.08)
    return _log_top(seed, SPRUCE_WOOD, ring_, SPRUCE_BARK, [7, 13, 19, 25, 31, 37, 43])


def paint_birch_log_top(seed: int) -> np.ndarray:
    ring_ = shift(hexc(BIRCH_WOOD), dv=-0.14, ds=0.1)
    return _log_top(seed, BIRCH_WOOD, ring_, BIRCH_BARK, [11, 21, 31, 41],
                    rim_light=shift(hexc(BIRCH_BARK), dv=0.04), rim_marks=hexc(BIRCH_MARK))


# ---------------------------------------------------------------------------- leaves

# leaf greys: shadow inside the crown, then three leaf tones, a rim and a vein
LEAF_SHADOW = grey(112)
LEAF_TONES = [grey(150), grey(176), grey(200)]
LEAF_RIM = grey(222)
LEAF_DARK_RIM = grey(132)


def _tone_pair(k: int):
    lo = LEAF_TONES[k - 1] if k > 0 else lerp(LEAF_SHADOW, LEAF_TONES[0], 0.5)
    return lo, LEAF_TONES[k]


def _draw_leaf(c: Canvas, x, y, ln, wd, a, k, round_=0.75):
    """A leaf: two halves in neighbouring tones (the half turned away from the top-left
    light darker), a light rim on its top-left edge, a dark one bottom right, a vein.
    Returns (window, mask)."""
    lo, hi = _tone_pair(k)
    win = window(c, x + np.cos(a) * ln / 2, y + np.sin(a) * ln / 2, ln / 2 + 3)
    whole = leaf(x, y, ln, wd, a, tile=False, round_=round_)
    m = wmask(c, win, whole)
    wfill(c, win, m, hi)
    nx, ny = -np.sin(a), np.cos(a)  # the +v side of the leaf
    side = 1 if nx * -0.7 + ny * -0.7 < 0 else -1
    wfill(c, win, wmask(c, win, half_leaf(x, y, ln, wd, a, side, tile=False, round_=round_)), lo)
    wfill(c, win, m & ~wmask(c, win, moved_shape(whole, 1.6, 1.6)), LEAF_RIM)
    wfill(c, win, m & ~wmask(c, win, moved_shape(whole, -1.3, -1.3)), LEAF_DARK_RIM)
    ca, sa = np.cos(a), np.sin(a)
    vein = capsule((x + ca * ln * 0.1, y + sa * ln * 0.1),
                   (x + ca * ln * 0.7, y + sa * ln * 0.7), 1.5)
    wfill(c, win, wmask(c, win, vein), lerp(lo, LEAF_SHADOW, 0.55))
    return win, m


def _holes(c: Canvas, seed: int, front: np.ndarray, n: int, min_dist: float,
           target: float = 0.15) -> None:
    """Real holes: irregular blobs (a few overlapping disks) cut where the front leaves do
    not cover them, so their edges are leaf silhouettes. The blobs are scaled until the holes
    cover about `target` of the texture."""
    rng = np.random.default_rng(seed)
    blobs = []
    for x, y in scatter(seed, n, min_dist):
        parts = [(x, y, 1.0)]
        for _ in range(2):
            a = rng.uniform(0, 2 * np.pi)
            parts.append((x + np.cos(a) * 6, y + np.sin(a) * 6, rng.uniform(0.6, 0.9)))
        blobs.append(parts)
    d = np.full(c.alpha.shape, 1e9, np.float32)  # distance in blob radii
    for parts in blobs:
        for x, y, s in parts:
            rows, cols, yy, xx = window(c, x, y, 26 * s)
            dy, dx = yy - y, xx - x
            d[np.ix_(rows, cols)] = np.minimum(d[np.ix_(rows, cols)],
                                               np.sqrt(dy * dy + dx * dx) / s)
    free = ~_morph(front, 2.5, ndimage.binary_closing)  # no holes between needles
    lo, hi = 1.0, 25.0
    for _ in range(12):
        r = (lo + hi) / 2
        if ((d <= r) & free).mean() < target:
            lo = r
        else:
            hi = r
    c.erase(_morph((d <= (lo + hi) / 2) & free, 1.5, ndimage.binary_opening))


def _morph(m: np.ndarray, px: float, op) -> np.ndarray:
    """A morphological `op` with a disk `px` texture pixels wide, wrapping round."""
    r = int(round(px * SS))
    yy, xx = np.mgrid[-r : r + 1, -r : r + 1]
    k = yy * yy + xx * xx <= r * r
    return op(np.pad(m, r + 1, mode="wrap"), k)[r + 1 : -r - 1, r + 1 : -r - 1]


def _leaves(seed: int, n: int, size: tuple, wr: tuple, min_dist: float, round_: float,
            per: int, spread: float, holes: tuple) -> np.ndarray:
    """Clusters of `per` leaves fanning out of a point, scattered so they tile, in two
    layers (the holes are cut between them)."""
    rng = np.random.default_rng(seed)
    c = Canvas(bg=LEAF_SHADOW)
    front = np.zeros_like(c.alpha, bool)
    pts = scatter(seed + 3, n, min_dist)
    for idx, (x, y) in enumerate(pts):
        is_front = idx >= len(pts) * 0.55
        base = rng.uniform(0, 2 * np.pi)
        for j in range(per):
            a = base + (j - (per - 1) / 2) * spread + rng.uniform(-0.2, 0.2)
            ln = rng.uniform(*size)
            wd = ln * rng.uniform(*wr)
            k = int(rng.integers(1, 3)) if is_front else int(rng.integers(0, 2))
            win, m = _draw_leaf(c, x, y, ln, wd, a, k, round_)
            rows, cols = win[0], win[1]
            sub = front[np.ix_(rows, cols)]
            if is_front:
                sub |= m
            else:
                sub &= ~m
            front[np.ix_(rows, cols)] = sub
    _holes(c, seed + 9, front, *holes)
    return c.finish(cutout=True)


def paint_oak_leaves(seed: int) -> np.ndarray:
    return _leaves(seed, 40, (22, 30), (0.62, 0.72), 15, 0.6, 3, 0.95, (9, 34))


def paint_birch_leaves(seed: int) -> np.ndarray:
    return _leaves(seed, 64, (15, 20), (0.5, 0.58), 11, 0.7, 3, 0.85, (11, 30))


def paint_spruce_leaves(seed: int) -> np.ndarray:
    """Needle sprays: a twig with paired needles slanting forward (chevrons)."""
    rng = np.random.default_rng(seed)
    c = Canvas(bg=LEAF_SHADOW)
    front = np.zeros_like(c.alpha, bool)
    pts = scatter(seed + 3, 44, 14)
    for idx, (x, y) in enumerate(pts):
        is_front = idx >= len(pts) * 0.5
        a = rng.uniform(0, 2 * np.pi)
        a = a + 0.35 * np.sin(a - np.pi / 2) * -1  # a little towards hanging down
        ln = rng.uniform(24, 34)
        k = int(rng.integers(1, 3)) if is_front else int(rng.integers(0, 2))
        lo, hi = _tone_pair(k)
        ca, sa = np.cos(a), np.sin(a)
        nx, ny = -sa, ca
        ndl = 12.0
        win = window(c, x + ca * ln / 2, y + sa * ln / 2, ln / 2 + ndl + 3)
        spray = np.zeros((len(win[0]), len(win[1])), bool)
        for side in (1, -1):
            lit = (nx * side * -0.7 + ny * side * -0.7) > 0
            segs = []
            for s in np.arange(2.0, ln, 4.6):
                px, py = x + ca * s, y + sa * s
                taper = 1.0 - 0.5 * (s / ln)
                segs.append((px, py, (ca * 0.85 + nx * side * 0.8) * ndl * taper,
                             (sa * 0.85 + ny * side * 0.8) * ndl * taper))
            m = wmask(c, win, strokes(segs, 2.6, tile=False))
            wfill(c, win, m, hi if lit else lo)
            spray |= m
        stem = wmask(c, win, capsule((x, y), (x + ca * (ln + 3), y + sa * (ln + 3)), 3.0))
        wfill(c, win, stem, LEAF_RIM if k == 2 else lerp(LEAF_RIM, hi, 0.45))
        spray |= stem
        rows, cols = win[0], win[1]
        sub = front[np.ix_(rows, cols)]
        sub = (sub | spray) if is_front else (sub & ~spray)
        front[np.ix_(rows, cols)] = sub
    _holes(c, seed + 9, front, 12, 28, 0.15)
    return c.finish(cutout=True)


# ---------------------------------------------------------------------------- cactus


def paint_cactus_side(seed: int) -> np.ndarray:
    base, dark, light = hexc(CACTUS), hexc(CACTUS_DARK), hexc(CACTUS_LIGHT)
    mid_light = lerp(base, light, 0.5)
    c = Canvas(bg=dark)
    ribs = 4
    rw = S / ribs
    spine = hexc("#efe6c4")
    spine_dark = shift(dark, dv=-0.1)
    for i in range(ribs):
        x0 = i * rw
        rib = box(-20, x0 + 3, S + 20, x0 + rw - 3, r=0, tile=True)
        c.fill(rib, base)
        c.fill(box(-20, x0 + 3, S + 20, x0 + 9, tile=True), mid_light)
        c.fill(box(-20, x0 + 3, S + 20, x0 + 5.5, tile=True), light)
        c.fill(box(-20, x0 + rw - 7, S + 20, x0 + rw - 3, tile=True), lerp(base, dark, 0.5))
        # spines along the groove on the rib's right, staggered per rib
        for j in range(4):
            y = (j + 0.5 + (i % 2) * 0.5) * S / 4
            gx = x0 + rw
            c.fill(disk(y + 1.0, gx + 1.0, 3.0), spine_dark)
            c.fill(disk(y, gx, 2.6), spine)
    return c.finish(opaque=True)


def paint_cactus_top(seed: int) -> np.ndarray:
    """The square top: the side's rib grooves reach in a little from every edge, a lighter
    domed crown in the middle with eight short grooves, spines round it."""
    base, dark, light = hexc(CACTUS), hexc(CACTUS_DARK), hexc(CACTUS_LIGHT)
    spine = hexc("#efe6c4")
    c = Canvas(tile=False, bg=base)
    c.fill(box(0, 0, S, S), base)
    c.fill(lambda y, x: (x < 5) | (y < 5), lerp(base, light, 0.5))  # lit top/left rim
    c.fill(lambda y, x: (x > S - 5) | (y > S - 5), lerp(base, dark, 0.55))
    for k in range(5):
        p = k * S / 4
        for (x0, y0, x1, y1) in ((p, 0, p, 18), (p, S, p, S - 18), (0, p, 18, p), (S, p, S - 18, p)):
            c.fill(capsule((x0, y0), (x1, y1), 5.0), dark)
    c.fill(disk(C, C, 40, tile=False), lerp(base, dark, 0.35))
    c.fill(disk(C - 1.5, C - 1.5, 38, tile=False), lerp(base, light, 0.25))
    c.fill(disk(C - 1, C - 1, 22, tile=False), lerp(base, light, 0.55))
    for k in range(8):
        a = k * np.pi / 4 + np.pi / 8
        c.fill(wedge(C, C, 8, 38, a, 3.2, 1.2), lerp(base, dark, 0.6))
        sx, sy = C + np.cos(a) * 46, C + np.sin(a) * 46
        c.fill(disk(sy + 1, sx + 1, 3.0, tile=False), dark)
        c.fill(disk(sy, sx, 2.6, tile=False), spine)
    c.fill(disk(C, C, 6, tile=False), light)
    return c.finish(opaque=True)


# ---------------------------------------------------------------------------- sprites


def _sprite_done(c: Canvas, width: float = 2.0, dark: float = 0.45) -> np.ndarray:
    """A `width` px outline round the sprite, each outline sample a dark tone of the nearest
    drawn colour (so a stem gets a dark green edge, a petal a dark red one)."""
    m = c.alpha > 0.5
    dist, (iy, ix) = ndimage.distance_transform_edt(~m, return_indices=True)
    ring_ = (dist <= width * SS) & ~m
    col = c.rgb[iy[ring_], ix[ring_]]
    grey_ = col.mean(-1, keepdims=True)
    c.rgb[ring_] = np.clip((col + (col - grey_) * 0.35) * dark, 0, 255)
    c.alpha[ring_] = 1.0
    return c.finish(cutout=True)


def paint_short_grass(seed: int) -> np.ndarray:
    rng = np.random.default_rng(seed)
    c = Canvas(tile=False)
    tg = [grey(150), grey(180), grey(206), grey(226)]
    # back blades darker, front ones lighter; each blade split along its length in two tones
    specs = [(-40, 0), (32, 0), (-10, 0), (-18, 1), (16, 1), (-4, 2), (40, 1), (-30, 2),
             (26, 2), (6, 2)]
    for k, (dx, layer) in enumerate(specs):
        x0 = C + dx * 0.9 + rng.uniform(-3, 3)
        h = rng.uniform(70, 104) - (abs(dx) * 0.4)
        lean = dx * 0.55 + rng.uniform(-8, 8)
        bend = rng.uniform(-10, 10)
        w = rng.uniform(13, 17)
        x1, y1 = x0 + lean, S - h
        c.fill(blade(x0, S + 2, x1, y1, w, bend), tg[layer])
        c.fill(blade(x0 - w * 0.22, S + 2, x1, y1, w * 0.5, bend), tg[layer + 1])
    c.erase(lambda y, x: y >= S)
    return _sprite_done(c)


def _leafy_stem(c, x0, y0, x1, y1, color, w=5.0, bend=0.0):
    pts = [(x0 + (x1 - x0) * t + bend * np.sin(np.pi * t), y0 + (y1 - y0) * t)
           for t in np.linspace(0, 1, 6)]
    stroke(c, pts, w, color)


def _sprite_leaf(c, x, y, ln, wd, a, col_hi, col_lo, round_=0.7, vein=None):
    c.fill(leaf(x, y, ln, wd, a, tile=False, round_=round_), col_hi)
    side = -1 if np.cos(a) > 0 else 1
    c.fill(half_leaf(x, y, ln, wd, a, side, tile=False, round_=round_), col_lo)
    if vein is not None:
        ca, sa = np.cos(a), np.sin(a)
        c.fill(capsule((x, y), (x + ca * ln * 0.7, y + sa * ln * 0.7), 1.6), vein)


def paint_poppy(seed: int) -> np.ndarray:
    c = Canvas(tile=False)
    st = tones(STEM, 4, 0.12)
    red = hexc(POPPY)
    red_d, red_l = shift(red, dv=-0.22, ds=0.05), shift(red, dv=0.08, ds=-0.12)
    _leafy_stem(c, C, S, C + 2, 44, st[1], 5.5, bend=-5)
    _sprite_leaf(c, C - 1, S - 22, 34, 14, -2.55, st[3], st[2], vein=st[1])
    _sprite_leaf(c, C + 1, S - 36, 30, 12, -0.6, st[3], st[2], vein=st[1])
    # the flower seen from the side: a cup of four round petals, a dark heart on top
    cy, cx = 36, C + 2
    c.fill(ellipse(cy + 3, cx - 15, 15, 14, tile=False), red_d)  # back petals
    c.fill(ellipse(cy + 3, cx + 15, 15, 14, tile=False), red_d)
    c.fill(ellipse(cy - 6, cx, 13, 22, tile=False), red_d)
    c.fill(ellipse(cy - 6.5, cx, 7, 15, tile=False), hexc("#3a2228"))  # the open heart
    c.fill(disk(cy - 7.5, cx - 4, 2.2, tile=False), hexc("#e7c25a"))
    c.fill(disk(cy - 7.5, cx + 4, 2.2, tile=False), hexc("#e7c25a"))
    c.fill(disk(cy - 9.5, cx, 2.2, tile=False), hexc("#e7c25a"))
    c.fill(ellipse(cy + 8, cx - 9, 13, 12, angle=0.3, tile=False), red)  # front petals
    c.fill(ellipse(cy + 8, cx + 9, 13, 12, angle=-0.3, tile=False), shift(red, dv=-0.08))
    c.fill(ellipse(cy + 3, cx - 12, 4.5, 3.4, angle=0.5, tile=False), red_l)  # gloss
    return _sprite_done(c)


def paint_dandelion(seed: int) -> np.ndarray:
    c = Canvas(tile=False)
    st = tones(STEM, 4, 0.12)
    yel = hexc(DANDELION)
    yel_d, yel_l = shift(yel, dv=-0.18, dh=-0.02, ds=0.1), shift(yel, dv=0.05, ds=-0.25)
    # long smooth leaves at the foot, wavy-edged by a second smaller leaf on each
    for a, ln in ((-2.85, 46), (-0.3, 44), (-2.3, 34), (-0.85, 32)):
        _sprite_leaf(c, C, S - 3, ln, ln * 0.32, a, st[3] if ln < 40 else st[2],
                     st[2] if ln < 40 else st[1], round_=0.9, vein=st[0])
    _leafy_stem(c, C, S - 4, C - 1, 46, st[1], 5.0, bend=4)
    # the head: a ring of rounded ray petals, a smaller inner ring, a light middle
    cy, cx = 40, C - 1
    for k in range(16):
        a = k * 2 * np.pi / 16
        c.fill(capsule((cx, cy), (cx + np.cos(a) * 21, cy + np.sin(a) * 18), 8.5), yel_d)
    for k in range(10):
        a = k * 2 * np.pi / 10 + 0.3
        c.fill(capsule((cx, cy), (cx + np.cos(a) * 15, cy + np.sin(a) * 13), 8.5), yel)
    c.fill(ellipse(cy - 1, cx - 1, 7, 8, tile=False), yel_l)
    c.fill(ellipse(cy - 3, cx - 3, 2.6, 3, tile=False), hexc("#fff4c8"))
    return _sprite_done(c)


def paint_dead_bush(seed: int) -> np.ndarray:
    rng = np.random.default_rng(seed)
    c = Canvas(tile=False)
    t = tones(DEAD_BUSH, 4, 0.12)

    def branch(x, y, a, ln, w, depth):
        x1, y1 = x + np.cos(a) * ln, y + np.sin(a) * ln
        c.fill(capsule((x, y), (x1, y1), w), t[1] if depth % 2 == 0 else t[2])
        # lit top-left edge
        c.fill(capsule((x - 0.8, y - 0.8), (x1 - 0.8, y1 - 0.8), w * 0.4), t[3])
        if depth < 3:
            for s in (-1, 1):
                branch(x1, y1, a + s * rng.uniform(0.35, 0.6), ln * rng.uniform(0.55, 0.72),
                       max(w * 0.7, 2.6), depth + 1)

    branch(C, S + 2, -np.pi / 2 - 0.45, 34, 6.5, 0)
    branch(C, S + 2, -np.pi / 2 + 0.5, 32, 6.5, 0)
    branch(C, S + 2, -np.pi / 2 + 0.05, 26, 6.0, 1)
    c.erase(lambda y, x: y >= S)
    return _sprite_done(c)


def paint_oak_sapling(seed: int) -> np.ndarray:
    c = Canvas(tile=False)
    gt = tones("#5f9a3e", 4, 0.13)
    bark = tones(OAK_BARK, 3, 0.12)
    _leafy_stem(c, C, S, C + 1, 50, bark[1], 7.0, bend=-3)
    c.fill(capsule((C - 1.5, S), (C - 0.5, 56), 2.2), bark[2])
    for x, y, a, ln in ((C, 62, -2.6, 38), (C, 62, -0.55, 38), (C + 1, 54, -1.6, 40),
                        (C, 86, -2.9, 30), (C + 1, 80, -0.25, 30)):
        _sprite_leaf(c, x, y, ln, ln * 0.66, a, gt[3], gt[2], round_=0.6, vein=gt[1])
    return _sprite_done(c)


def paint_birch_sapling(seed: int) -> np.ndarray:
    c = Canvas(tile=False)
    gt = tones("#7aa84a", 4, 0.12)
    bark = hexc(BIRCH_BARK)
    _leafy_stem(c, C, S, C - 1, 40, bark, 6.0, bend=4)
    for y in (112, 92, 70):
        c.fill(capsule((C - 2, y), (C + 2.5, y), 2.6), hexc(BIRCH_MARK))
    for x, y, a, ln in ((C - 1, 44, -1.5, 30), (C, 50, -2.45, 28), (C, 50, -0.65, 28),
                        (C + 1, 70, -0.35, 26), (C + 1, 80, -2.8, 24), (C + 2, 96, -0.4, 20)):
        _sprite_leaf(c, x, y, ln, ln * 0.55, a, gt[3], gt[2], round_=0.7, vein=gt[1])
    return _sprite_done(c)


def paint_spruce_sapling(seed: int) -> np.ndarray:
    c = Canvas(tile=False)
    gt = tones("#3f7048", 4, 0.12)
    bark = tones(SPRUCE_BARK, 3, 0.12)
    c.fill(box(96, C - 4, S + 1, C + 4, r=2), bark[1])
    c.fill(box(96, C - 4, S + 1, C - 1.5, r=1), bark[2])
    # three stacked tiers, each a wide rounded chevron: left half lit, right half shaded
    for top, bot, hw in ((18, 54, 20), (38, 78, 30), (60, 102, 40)):
        left = [(C, top), (C - hw, bot - 6), (C - hw + 6, bot), (C, bot - 7)]
        right = [(C, top), (C + hw, bot - 6), (C + hw - 6, bot), (C, bot - 7)]
        c.fill(poly(left), gt[3])
        c.fill(poly(right), gt[2])
        # a darker underside notch
        c.fill(poly([(C - hw + 6, bot), (C, bot - 7), (C + hw - 6, bot), (C, bot - 3)]), gt[1])
    return _sprite_done(c)


TEXTURES = {
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
}
