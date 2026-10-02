"""Crafted blocks in the game's flat, clean style (see BRIEF.md): planks, glass, bricks, stone
bricks, crafting table, furnace, storage blocks, torch, lantern, chain, door, wool, bed and
the block-breaking cracks.

Everything is a few flat colour areas drawn with `flat.Canvas` (anti-aliased edges), lit from
the top left (light rims top/left, shadow rims bottom/right), rounded corners, no noise.
"""

from __future__ import annotations

import numpy as np

import flat
from flat import Canvas, hexc, minus, moved_shape, shift, tiled, tones, union

S = 128


# ---------------------------------------------------------------------------- fast shapes
# The flat shape makers evaluate over the whole canvas; these wrap the non-wrapping ones so
# they are only evaluated inside their bounding box (keeps the build quick).


def bounded(shape, y0, x0, y1, x1):
    def f(y, x):
        out = np.zeros(y.shape, bool)
        ys, xs = y[:, 0], x[0, :]
        a, b = np.searchsorted(ys, y0), np.searchsorted(ys, y1)
        c, d = np.searchsorted(xs, x0), np.searchsorted(xs, x1)
        if a < b and c < d:
            out[a:b, c:d] = shape(y[a:b, c:d], x[a:b, c:d])
        return out
    return f


def box(y0, x0, y1, x1, r: float = 0.0, tile: bool = False):
    s = flat.box(y0, x0, y1, x1, r=r, tile=tile)
    return s if tile else bounded(s, y0 - 1, x0 - 1, y1 + 1, x1 + 1)


def capsule(p0, p1, width: float):
    s = flat.capsule(p0, p1, width)
    h = width / 2 + 1
    return bounded(s, min(p0[1], p1[1]) - h, min(p0[0], p1[0]) - h,
                   max(p0[1], p1[1]) + h, max(p0[0], p1[0]) + h)


def disk(cy, cx, r, tile: bool = True):
    s = flat.disk(cy, cx, r, tile)
    return s if tile else bounded(s, cy - r - 1, cx - r - 1, cy + r + 1, cx + r + 1)


def ellipse(cy, cx, ry, rx, angle: float = 0.0, tile: bool = True):
    s = flat.ellipse(cy, cx, ry, rx, angle, tile)
    m = max(ry, rx) + 1
    return s if tile else bounded(s, cy - m, cx - m, cy + m, cx + m)


def poly(points):
    xs, ys = [p[0] for p in points], [p[1] for p in points]
    return bounded(flat.poly(points), min(ys) - 1, min(xs) - 1, max(ys) + 1, max(xs) + 1)


def ring(cy, cx, r0, r1, tile: bool = True):
    return minus(disk(cy, cx, r1, tile), disk(cy, cx, r0, tile))

# ---------------------------------------------------------------------------- palette

OAK = tones("#b98a55")          # dark .. light
OAK_GAP = hexc("#7a5530")
BRICK_T = tones("#b5553f")
MORTAR_T = tones("#d8cfc0", 3, 0.08)
STONE_T = tones("#8d8f96")
STONE_CREASE = hexc("#62656e")
STONE_GAP = hexc("#4c4f58")
IRON = tones("#c9ccd2", 4, 0.12)
COPPER_T = tones("#c8743f")
PATINA = tones("#4fa38f", 3, 0.1)
GOLD = tones("#f0c043", 4, 0.13)
DIAMOND = tones("#4fd8cf", 4, 0.14)
COAL = tones("#2f2f35", 4, 0.12)
WOOL = [hexc("#d3ccbf"), hexc("#e0dacf"), hexc("#eeeae2"), hexc("#f6f3ed")]
RED = tones("#c8443a", 4, 0.13)
PILLOW = [hexc("#cfc8bb"), hexc("#e2ddd2"), hexc("#f2efe8"), hexc("#fbfaf6")]
STICK = tones("#8a6038", 3, 0.12)
DARK_IRON = tones("#3d4048", 4, 0.12)
WARM = [hexc("#e89a3c"), hexc("#ffc062"), hexc("#ffd27a"), hexc("#fff0bf")]
GLASS = [hexc("#b3c3cf"), hexc("#dfe8ef"), hexc("#f6fafc")]


def rim(c: Canvas, shape, light, dark, w: float = 2.0, mask=None) -> None:
    """Only the light (top/left) and dark (bottom/right) rims of `shape`, over what is there."""
    m = c.mask(shape)
    if mask is not None:
        m &= c.mask(mask)
    if light is not None:
        c.fill(m & ~c.mask(moved_shape(shape, w, w)), light)
    if dark is not None:
        c.fill(m & ~c.mask(moved_shape(shape, -w, -w)), dark)


def rows_of(heights):
    out, y = [], 0.0
    for h in heights:
        out.append((y, y + h))
        y += h
    return out


# ---------------------------------------------------------------------------- planks


def planks(c: Canvas, seed: int, heights, seams, gap: float = 2.5, knots: bool = True) -> None:
    """Horizontal boards (rows `heights`, one board per row ending at its seam `seams[i]`),
    each a flat tone, raised, with a couple of long grain lines and the odd knot."""
    r = np.random.default_rng(seed)
    c.fill(np.ones_like(c.alpha, bool), OAK_GAP)
    for i, ((y0, y1), sx) in enumerate(zip(rows_of(heights), seams)):
        k = (0.0, 0.035, -0.03, 0.02)[i % 4]
        base = shift(OAK[2], dv=k)
        lt, dk, grain = shift(base, dv=0.09, ds=-0.06), shift(base, dv=-0.14, ds=0.05), shift(base, dv=-0.08, ds=0.04)
        cx = sx + 64.0
        board = tiled(box(y0 + gap / 2, cx - 64 + gap / 2, y1 - gap / 2, cx + 64 - gap / 2, r=3))
        c.fill(board, base)
        # grain: one or two long thin lines, slightly darker
        for g in range(int(r.integers(1, 3))):
            gy = r.uniform(y0 + 7, y1 - 7)
            gx = r.uniform(0, 128)
            ln = r.uniform(34, 70)
            c.fill(lambda y, x, gy=gy, gx=gx, ln=ln: tiled(capsule((gx, gy), (gx + ln, gy), 2.2))(y, x)
                   & board(y, x), grain)
        if knots and r.random() < 0.45:
            ky, kx = (y0 + y1) / 2 + r.uniform(-3, 3), r.uniform(0, 128)
            kn = tiled(ellipse(ky, kx, 3.2, 5.5, tile=False))
            c.fill(lambda y, x: kn(y, x) & board(y, x), grain)
            c.fill(lambda y, x: tiled(ellipse(ky, kx, 1.4, 2.6, tile=False))(y, x) & board(y, x), dk)
        rim(c, board, lt, dk, 2.0)


def paint_oak_planks(seed):
    c = Canvas()
    planks(c, seed, (30, 34, 28, 36), (18, 86, 44, 112))
    return c.finish(opaque=True)


def paint_bed_down(seed):
    c = Canvas()
    planks(c, seed + 3, (32, 32, 32, 32), (60, 4, 92, 30), knots=False)
    return c.finish(opaque=True)


# ---------------------------------------------------------------------------- glass


def paint_glass(seed):
    c = Canvas(tile=False)
    outer = box(0, 0, 128, 128)
    inner = box(5, 5, 123, 123, r=3)
    frame = minus(outer, inner)
    c.fill(frame, GLASS[1])
    c.fill(box(0, 0, 2, 128), GLASS[2])
    c.fill(box(0, 0, 128, 2), GLASS[2])
    c.fill(box(126, 2, 128, 128), GLASS[0])
    c.fill(box(2, 126, 128, 128), GLASS[0])
    # inner lip of the frame: shadow at the top/left inside edge, light at the bottom/right
    lip = minus(box(5, 5, 123, 123, r=3), box(6.5, 6.5, 121.5, 121.5, r=2))
    c.fill(lambda y, x: lip(y, x) & ((y < 20) | (x < 20)) & (y + x < 128), GLASS[0])
    # diagonal glints (top left: a broad and a thin one; bottom right: a short one)
    win = box(9, 9, 119, 119, r=2)

    def band(o, w):
        return lambda y, x: (np.abs((x + y) - o) <= w / 2) & win(y, x)

    c.fill(lambda y, x: band(52, 9)(y, x) & (x - y > -34) & (x - y < 34), GLASS[2])
    c.fill(lambda y, x: band(70, 3.5)(y, x) & (x - y > -26) & (x - y < 26), GLASS[2])
    c.fill(lambda y, x: band(190, 4)(y, x) & (x - y > -18) & (x - y < 18), GLASS[2])
    return c.finish(cutout=True)


# ---------------------------------------------------------------------------- bricks


def paint_bricks(seed):
    c = Canvas()
    c.fill(np.ones_like(c.alpha, bool), MORTAR_T[1])
    r = np.random.default_rng(seed)
    gap = 3.0
    for row in range(8):
        y0 = row * 16.0
        off = 16.0 if row % 2 else 0.0
        for b in range(4):
            x0 = off + b * 32.0
            base = BRICK_T[2] if r.random() < 0.55 else (BRICK_T[1] if r.random() < 0.5 else shift(BRICK_T[2], dv=0.04))
            sh = box(y0 + gap / 2, x0 + gap / 2, y0 + 16 - gap / 2, x0 + 32 - gap / 2, r=2.5)
            s = tiled(sh)
            c.fill(s, base)
            rim(c, s, shift(base, dv=0.1, ds=-0.08), shift(base, dv=-0.14, ds=0.05), 1.8)
    # the mortar's own shade: a darker lower lip under each brick
    return c.finish(opaque=True)


# ---------------------------------------------------------------------------- stone


def stone_plates(c: Canvas, seed: int, mask, count: int = 30) -> None:
    """The stone's flat-shaded facets (subdued), inside `mask`."""
    soft = [flat.lerp(STONE_T[1], STONE_T[2], 0.45), STONE_T[2] * 0.96, STONE_T[2],
            flat.lerp(STONE_T[2], STONE_T[3], 0.35)]
    c.facets(seed, count, soft, crease=STONE_CREASE, crease_w=1.0, mask=mask, tilt=1.0)


STONE_COURSES = [(0, 32, (10, 74)), (32, 64, (42, 100)), (64, 96, (0, 56, 92)),
                 (96, 128, (30, 82))]


def paint_stone_bricks(seed):
    """Ashlar: four courses of blocks of different lengths, each faceted stone, raised."""
    c = Canvas()
    c.fill(np.ones_like(c.alpha, bool), STONE_GAP)
    gap = 3.0
    blocks = []
    for y0, y1, xs in STONE_COURSES:
        xs = list(xs) + [xs[0] + 128]
        for a, b in zip(xs[:-1], xs[1:]):
            blocks.append(tiled(box(y0 + gap / 2, a + gap / 2, y1 - gap / 2, b - gap / 2, r=3)))
    allb = union(*blocks)
    stone_plates(c, seed, allb, 42)
    for b in blocks:
        rim(c, b, STONE_T[3], STONE_T[0], 2.0)
    return c.finish(opaque=True)


# ---------------------------------------------------------------------------- crafting table
# A sturdy dark-oak workbench: a thick top of boards with a 3 x 3 grid carved into it and iron
# brackets on its corners; legs and rails round a pegboard with tools hanging on it (side)
# or drawers and a rack of chisels (front); lumber on the shelf at the bottom.

BENCH = tones("#6e4529", 4, 0.13)        # dark oak: dark .. light
BENCH_GAP = hexc("#2b1a10")
PEGBOARD = tones("#9a6a3e", 3, 0.1)
BRASS = tones("#c99a3e", 3, 0.12)
HANDLE_RED = tones("#b4483a", 3, 0.12)


def metal(c, shape, t=IRON, w=1.6):
    c.fill(shape, t[2])
    rim(c, shape, t[3], t[0], w)


def bench_board(c: Canvas, shape, k: int = 2) -> None:
    """A dark-oak part, raised: tone `k` with light and shadow rims."""
    c.fill(shape, BENCH[k])
    rim(c, shape, BENCH[min(k + 1, 3)], BENCH[max(k - 1, 0)], 2.0)


def bench_body(lumber) -> Canvas:
    """Side/front base: the worktop's edge on top, a leg down each side, a rail under the top
    and a shelf at the bottom with `lumber(c)` lying on it; the middle (y 30..98, x 16..112)
    is left for the face's own things, on a dark gap."""
    c = Canvas(tile=False)
    c.fill(box(0, 0, 128, 128), BENCH_GAP)
    # the shelf board and what lies on it
    lumber(c)
    bench_board(c, box(118, 14, 128, 114), 1)
    # legs, the rail under the top, the worktop's thick edge (overhanging the legs)
    for x0, x1 in ((0, 16), (112, 128)):
        bench_board(c, box(18, x0, 128, x1), 1)
        c.fill(box(18, x1 - 3 if x0 else x0, 128, x1 if x0 == 0 else x0 + 3), BENCH[0])
    bench_board(c, box(18, 14, 28, 114), 1)
    top = box(0, 0, 18, 128)
    c.fill(top, BENCH[2])
    c.fill(box(0, 0, 3, 128), BENCH[3])
    c.fill(box(14, 0, 18, 128), BENCH[0])
    c.fill(box(28, 16, 31, 112), BENCH_GAP)  # shadow under the rail
    # iron straps where the top meets the legs, with rivets
    for x in (2, 112):
        strap = box(4, x, 26, x + 14, r=2)
        metal(c, strap, DARK_IRON, 1.4)
        for ry in (9, 21):
            c.fill(disk(ry, x + 7, 1.8, tile=False), DARK_IRON[3])
    return c


def boards_lying(c: Canvas) -> None:
    """Three planks lying on the shelf, seen from their long side."""
    for i, (y0, x0, x1) in enumerate(((100, 22, 98), (106, 30, 106), (112, 18, 102))):
        b = box(y0, x0, y0 + 6, x1, r=1.5)
        t = shift(OAK[2], dv=(0.0, 0.04, -0.03)[i])
        c.fill(b, t)
        rim(c, b, shift(t, dv=0.09, ds=-0.06), shift(t, dv=-0.16, ds=0.05), 1.4)


def board_ends(c: Canvas) -> None:
    """Planks stacked on the shelf, seen end on: two rows of short rectangles."""
    for row, y0 in enumerate((100, 109)):
        for k in range(4):
            x0 = 20 + k * 23 + (0 if row else 6)
            if x0 + 20 > 110:
                continue
            b = box(y0, x0, y0 + 8, x0 + 20, r=1.5)
            t = shift(OAK[2], dv=(0.03, -0.02, 0.0, 0.05)[k])
            c.fill(b, t)
            rim(c, b, shift(t, dv=0.09, ds=-0.06), shift(t, dv=-0.16, ds=0.05), 1.4)
            c.fill(box(y0 + 3.5, x0 + 4, y0 + 4.5, x0 + 16), shift(t, dv=-0.1))


def paint_crafting_table_side(seed):
    """A pegboard with a hammer, a screwdriver and a wrench hanging on it."""
    c = bench_body(boards_lying)
    panel = box(33, 18, 97, 110, r=2)
    c.fill(panel, PEGBOARD[1])
    rim(c, panel, PEGBOARD[2], PEGBOARD[0], 1.6)
    for hy in range(40, 96, 10):
        for hx in range(25, 108, 10):
            c.fill(disk(hy, hx, 1.3, tile=False), PEGBOARD[0])
    # hammer: handle down, head across the top, a claw on its right
    hnd = capsule((38, 48), (38, 92), 7)
    c.fill(hnd, STICK[1])
    rim(c, hnd, STICK[2], STICK[0], 1.6)
    head = union(box(39, 28, 50, 50, r=2), box(41, 22, 48, 29, r=1))
    metal(c, head, IRON, 1.8)
    # screwdriver: a red grip and a steel shaft
    c.fill(disk(37, 64, 2.4, tile=False), DARK_IRON[1])
    grip = capsule((64, 42), (64, 64), 10)
    c.fill(grip, HANDLE_RED[1])
    rim(c, grip, HANDLE_RED[2], HANDLE_RED[0], 1.6)
    shaft = box(64, 62.5, 90, 65.5, r=1)
    metal(c, shaft, IRON, 1.0)
    # wrench: an open jaw on top, a ring at the bottom
    jaw = minus(disk(48, 90, 8, tile=False), box(36, 87, 49, 93))
    ring_ = minus(disk(86, 90, 6, tile=False), disk(86, 90, 3, tile=False))
    wrench = union(jaw, box(52, 87, 82, 93, r=1.5), ring_)
    metal(c, wrench, IRON, 1.6)
    c.fill(disk(37, 90, 2.4, tile=False), DARK_IRON[1])
    return c.finish(opaque=True)


def paint_crafting_table_front(seed):
    """Two drawers with brass pulls, and a rack of chisels under them."""
    c = bench_body(board_ends)
    for x0, x1 in ((18, 63), (65, 110)):
        dr = box(33, x0, 58, x1, r=3)
        bench_board(c, dr, 2)
        c.fill(box(37, x0 + 4, 54, x1 - 4, r=2), BENCH[1])
        c.fill(box(37, x0 + 4, 39, x1 - 4), BENCH[0])
        pull = capsule(((x0 + x1) / 2 - 8, 45.5), ((x0 + x1) / 2 + 8, 45.5), 4.5)
        c.fill(moved_shape(pull, 1.5, 1.5), BENCH[0])
        c.fill(pull, BRASS[1])
        rim(c, pull, BRASS[2], BRASS[0], 1.2)
    # the chisel rack: a bar with four chisels hanging through it, blades down
    c.fill(box(61, 18, 98, 110), BENCH[0])
    for k, x in enumerate((32, 52, 74, 96)):
        blade = poly([(x - 3, 72), (x + 3, 72), (x + 3.5 - k * 0.3, 92 - k * 2), (x - 3.5 + k * 0.3, 92 - k * 2)])
        metal(c, blade, IRON, 1.3)
        grip = capsule((x, 63), (x, 73), 8)
        t = (STICK, HANDLE_RED, STICK, BRASS)[k]
        c.fill(grip, t[1])
        rim(c, grip, t[2], t[0], 1.4)
    bar = box(66, 18, 72, 110, r=1)
    bench_board(c, bar, 2)
    return c.finish(opaque=True)


def paint_crafting_table_top(seed):
    """The worktop: dark-oak boards between two end rails, a 3 x 3 grid carved into the
    middle (sunk cells: shadow on their top/left, light on their bottom/right) and iron
    brackets on the corners."""
    c = Canvas(tile=False)
    c.fill(box(0, 0, 128, 128), BENCH_GAP)
    for i, (y0, y1) in enumerate(rows_of((32, 32, 32, 32))):
        b = box(y0 + 1, 14, y1 - 1, 114, r=1.5)
        t = shift(BENCH[2], dv=(0.0, 0.03, -0.025, 0.015)[i])
        c.fill(b, t)
        rim(c, b, shift(t, dv=0.08, ds=-0.05), shift(t, dv=-0.12, ds=0.04), 1.6)
        gy = y0 + (11, 21, 14, 19)[i]
        c.fill(capsule((20 + i * 9, gy), (64 + i * 9, gy), 1.8), shift(t, dv=-0.06))
    for x0, x1 in ((0, 14), (114, 128)):
        bench_board(c, box(0, x0, 128, x1), 1)
    # the carved grid
    c.fill(box(26, 26, 102, 102, r=4), BENCH[0])
    for gy in range(3):
        for gx in range(3):
            y0, x0 = 29 + gy * 24.33, 29 + gx * 24.33
            cell = box(y0, x0, y0 + 21.33, x0 + 21.33, r=2)
            c.fill(cell, shift(BENCH[1], dv=0.02))
            c.fill(cell, shift(BENCH[1], dv=0.02))
            c.fill(lambda y, x, cell=cell: cell(y, x) & ~moved_shape(cell, 2.0, 2.0)(y, x), BENCH_GAP)
            c.fill(lambda y, x, cell=cell: cell(y, x) & ~moved_shape(cell, -1.5, -1.5)(y, x), BENCH[3])
    # corner brackets: an iron L on each corner, with rivets
    for cy, cx, sy, sx in ((0, 0, 1, 1), (0, 128, 1, -1), (128, 0, -1, 1), (128, 128, -1, -1)):
        def span(a, b, s):
            return (a, b) if s > 0 else (b, a)
        ya, yb = span(cy + sy * 3, cy + sy * 24, sy)
        xa, xb = span(cx + sx * 3, cx + sx * 10, sx)
        yc, yd = span(cy + sy * 3, cy + sy * 10, sy)
        xc, xd = span(cx + sx * 3, cx + sx * 24, sx)
        bracket = union(box(ya, xa, yb, xb, r=1.5), box(yc, xc, yd, xd, r=1.5))
        metal(c, bracket, DARK_IRON, 1.4)
        for ry, rx in ((cy + sy * 6.5, cx + sx * 6.5), (cy + sy * 19, cx + sx * 6.5), (cy + sy * 6.5, cx + sx * 19)):
            c.fill(disk(ry, rx, 1.7, tile=False), DARK_IRON[3])
    return c.finish(opaque=True)


# ---------------------------------------------------------------------------- furnace

FURNACE_SEED = 5151
# The game cuts the front's two openings out between their dark outlines (r + g + b < 60, see
# src/textures/synth.rs `synth_furnace_cut`) and puts the model's hollows behind them: the
# mouth above (texture rows ~20..60) and the firebox below (rows ~88..125), x 14..114. So the
# openings sit there, outlined in OUTLINE, and nothing else on the front is that dark.
OUTLINE = hexc("#111113")
HOLE = hexc("#2a292e")
HOLE_FLOOR = hexc("#211f24")


def furnace_frame(c: Canvas) -> None:
    """A darker raised stone frame round a furnace face."""
    fr = minus(box(0, 0, 128, 128), box(7, 7, 121, 121, r=3))
    c.fill(fr, STONE_T[1])
    c.fill(box(0, 0, 2.5, 128), STONE_T[2])
    c.fill(box(0, 0, 128, 2.5), STONE_T[2])
    c.fill(box(125.5, 0, 128, 128), STONE_T[0])
    c.fill(box(0, 125.5, 128, 128), STONE_T[0])
    c.fill(minus(box(7, 7, 121, 121, r=3), box(9, 9, 121, 121, r=3)), STONE_GAP)


def furnace_side_base(seed: int) -> Canvas:
    c = Canvas(tile=False)
    stone_plates(c, seed, box(0, 0, 128, 128), 26)
    furnace_frame(c)
    return c


MOUTH = box(27, 26, 55, 102, r=11)
FIREBOX = union(box(104, 24, 120, 104), ellipse(104, 64, 13, 40, tile=False))


def hole(c: Canvas, shape, floor_rows) -> None:
    """An opening: dark outline 2.5 px, a dark back wall, a darker floor."""
    c.fill(shape, OUTLINE)
    inner = lambda y, x: shape(y, x) & shape(y + 2.5, x) & shape(y - 2.5, x) & shape(y, x + 2.5) \
        & shape(y, x - 2.5)
    c.fill(inner, HOLE)
    c.fill(lambda y, x: inner(y, x) & (y >= floor_rows), HOLE_FLOOR)


_FRONT: list = []


def furnace_front_base() -> Canvas:
    """The unlit front (a fresh copy each call; lit and unlit share it exactly)."""
    import copy
    if not _FRONT:
        _FRONT.append(_furnace_front())
    return copy.deepcopy(_FRONT[0])


def _furnace_front() -> Canvas:
    c = furnace_side_base(FURNACE_SEED)
    # the mouth's raised surround with a lintel
    sur = box(19, 17, 63, 111, r=15)
    stone_plates(c, FURNACE_SEED + 3, sur, 9)
    rim(c, sur, STONE_T[3], STONE_T[0], 2.2)
    hole(c, MOUTH, 49)
    # the firebox's arch of wedge stones and its sill
    arch = union(box(102, 14, 121, 114), ellipse(104, 64, 23, 50, tile=False))
    c.fill(arch, STONE_T[2])
    for a in np.linspace(0.0, np.pi, 7)[1:-1]:
        x0, y0 = 64 - np.cos(a) * 40, 104 - np.sin(a) * 13
        x1, y1 = 64 - np.cos(a) * 52, 104 - np.sin(a) * 25
        c.fill(lambda y, x, p=(x0, y0), q=(x1, y1): capsule(p, q, 1.6)(y, x) & arch(y, x),
               STONE_CREASE)
    for yy in (104, 112):
        for x0, x1 in ((14, 24), (104, 114)):
            c.fill(box(yy - 0.8, x0, yy + 0.8, x1), STONE_CREASE)
    rim(c, arch, STONE_T[3], STONE_T[0], 2.2)
    hole(c, FIREBOX, 114)
    # a step between the two
    st = box(66, 30, 74, 98, r=3)
    c.fill(st, STONE_T[2])
    rim(c, st, STONE_T[3], STONE_T[0], 2.0)
    return c


def paint_furnace_front(seed):
    return furnace_front_base().finish(opaque=True)


def paint_furnace_front_on(seed):
    """The same front with a flat fire in the firebox: three tongues over glowing coals
    (drawn only inside the opening's outline)."""
    c = furnace_front_base()
    opm = c.mask(lambda y, x: FIREBOX(y, x) & FIREBOX(y + 2.5, x) & FIREBOX(y - 2.5, x)
                 & FIREBOX(y, x + 2.5) & FIREBOX(y, x - 2.5))
    op = lambda y, x: opm
    c.fill(op, hexc("#6a2a1c"))
    fire = [hexc("#e2522a"), hexc("#ff8a2a"), hexc("#ffcf5a"), hexc("#fff1b8")]
    base_y = 116.0
    for cx, top, w in ((44, 98, 9.5), (64, 93, 12), (84, 99, 9)):
        for sc, col in ((1.0, fire[0]), (0.72, fire[1]), (0.45, fire[2])):
            ww = w * sc
            tt = top + (base_y - top) * (1 - sc) * 0.75
            t = poly([(cx - ww, base_y), (cx - ww * 0.8, tt + (base_y - tt) * 0.45),
                      (cx, tt), (cx + ww * 0.8, tt + (base_y - tt) * 0.45), (cx + ww, base_y)])
            e = ellipse(base_y - 2, cx, 5 * sc + 1.5, ww, tile=False)
            c.fill(lambda y, x, t=t, e=e: (t(y, x) | e(y, x)) & op(y, x), col)
    for i, cx in enumerate((34, 48, 62, 76, 90)):
        co = ellipse(117, cx, 3.5, 7.5, tile=False)
        c.fill(lambda y, x, co=co: co(y, x) & op(y, x), fire[0] if i % 2 else hexc("#b53b22"))
        c.fill(lambda y, x, cx=cx: ellipse(116, cx - 2, 1.3, 3.2, tile=False)(y, x) & op(y, x), fire[2])
    return c.finish(opaque=True)



def paint_furnace_side(seed):
    return furnace_side_base(FURNACE_SEED + 11).finish(opaque=True)


def paint_furnace_top(seed):
    c = furnace_side_base(FURNACE_SEED + 21)
    lid = box(28, 28, 100, 100, r=8)
    stone_plates(c, FURNACE_SEED + 22, lid, 9)
    rim(c, lid, STONE_T[3], STONE_T[0], 2.5)
    c.fill(disk(64, 64, 11, tile=False), HOLE)
    c.fill(ring(64, 64, 11, 14, tile=False), STONE_T[1])
    rim(c, ring(64, 64, 11, 14, tile=False), STONE_T[3], STONE_T[0], 1.6)
    return c.finish(opaque=True)


# ---------------------------------------------------------------------------- storage blocks


def bevel_plate(c: Canvas, t, y0, x0, y1, x1, bevel=6.0, r=4.0):
    """A plate with wide flat bevels: top/left light, bottom/right dark, flat face."""
    outer = box(y0, x0, y1, x1, r=r)
    face = box(y0 + bevel, x0 + bevel, y1 - bevel, x1 - bevel, r=max(r - 2, 1))
    c.fill(outer, t[1])
    c.fill(poly([(x0, y0), (x1, y0), (x1 - bevel, y0 + bevel), (x0 + bevel, y0 + bevel),
                 (x0 + bevel, y1 - bevel), (x0, y1)]), t[3])
    c.fill(lambda y, x: outer(y, x) & poly([(x0, y0), (x1, y0), (x1 - bevel, y0 + bevel),
                                            (x0 + bevel, y0 + bevel), (x0 + bevel, y1 - bevel),
                                            (x0, y1)])(y, x), t[3])
    c.fill(lambda y, x: outer(y, x) & poly([(x1, y0), (x1, y1), (x0, y1), (x0 + bevel, y1 - bevel),
                                            (x1 - bevel, y1 - bevel), (x1 - bevel, y0 + bevel)])(y, x),
           t[0])
    # the top-right and bottom-left corners split the two (a mid tone)
    c.fill(face, t[2])
    return face


def rivet(c, cy, cx, t, r=4.0):
    d = disk(cy, cx, r, tile=False)
    c.fill(d, t[1])
    c.fill(disk(cy - 0.8, cx - 0.8, r - 1.4, tile=False), t[3])
    c.fill(disk(cy - 0.4, cx - 0.4, r - 2.4, tile=False), t[2])


def paint_iron_block(seed):
    """A riveted plate: wide bevels, a rivet in each corner, two pressed grooves."""
    c = Canvas(tile=False)
    bevel_plate(c, IRON, 0, 0, 128, 128, bevel=7, r=0)
    for cy in (40, 86):
        g = box(cy, 20, cy + 4, 108, r=2)
        c.fill(g, IRON[1])
        c.fill(box(cy + 2.5, 20, cy + 4, 108, r=1.5), IRON[3])
    for cy in (17, 111):
        for cx in (17, 111):
            rivet(c, cy, cx, IRON)
    gl = poly([(20, 70), (56, 20), (66, 20), (30, 70)])
    c.fill(lambda y, x: gl(y, x) & ~box(39, 0, 45, 128)(y, x), shift(IRON[2], dv=0.05))
    return c.finish(opaque=True)


def paint_copper_block(seed):
    """One heavy bevelled copper slab: rivets along the frame, two flat brushed highlight
    bands across the face, soft teal patina creeping in from the lower left corner."""
    c = Canvas(tile=False)
    bevel_plate(c, COPPER_T, 0, 0, 128, 128, bevel=8, r=0)
    face = box(8, 8, 120, 120)
    # brushed highlight bands (diagonal, flat)
    for o, w in ((70, 16), (100, 6)):
        c.fill(lambda y, x, o=o, w=w: face(y, x) & (np.abs(x + y - o * 1.0) <= w / 2)
               & (x - y > -60) & (x - y < 60), shift(COPPER_T[2], dv=0.05, ds=-0.06))
    # patina: overlapping flat patches in the lower left corner and along the bottom edge
    pat = union(ellipse(128, 0, 30, 34, tile=False), ellipse(125, 46, 4.5, 28, tile=False),
                ellipse(80, 2.5, 26, 4, tile=False))
    inner = union(ellipse(129, -2, 17, 20, tile=False), ellipse(126.5, 40, 2, 16, tile=False))
    c.fill(pat, PATINA[1])
    c.fill(inner, PATINA[2])
    # rivets along the frame (not where the patina lies)
    for cy, cx in ((4, 20), (4, 64), (4, 108), (64, 124), (124, 108), (20, 124),
                   (108, 124), (20, 4)):
        rivet(c, cy, cx, COPPER_T, 3.0)
    return c.finish(opaque=True)


def paint_gold_block(seed):
    """A bevelled gold slab with a raised diamond-shaped boss in the middle."""
    c = Canvas(tile=False)
    bevel_plate(c, GOLD, 0, 0, 128, 128, bevel=8, r=0)
    c.fill(box(16, 16, 112, 112, r=4), GOLD[1])
    c.fill(box(18, 18, 112, 112, r=4), GOLD[2])
    boss = [(64, 28), (100, 64), (64, 100), (28, 64)]
    c.fill(poly(boss), GOLD[1])
    c.fill(poly([(64, 28), (64, 64), (28, 64)]), GOLD[3])
    c.fill(poly([(64, 28), (100, 64), (64, 64)]), shift(GOLD[2], dv=0.03))
    c.fill(poly([(28, 64), (64, 64), (64, 100)]), GOLD[2])
    c.fill(poly([(64, 64), (100, 64), (64, 100)]), GOLD[0])
    c.fill(poly([(64, 44), (84, 64), (64, 84), (44, 64)]), shift(GOLD[2], dv=0.06, ds=-0.05))
    c.fill(poly([(40, 60), (56, 44), (60, 48), (44, 64)]), hexc("#fff3c4"))
    return c.finish(opaque=True)


def paint_diamond_block(seed):
    """A bevelled frame round a big cut gem seen from above: an octagon of flat facets."""
    c = Canvas(tile=False)
    bevel_plate(c, DIAMOND, 0, 0, 128, 128, bevel=7, r=0)
    c.fill(box(14, 14, 114, 114, r=3), DIAMOND[1])
    # octagon table with crown facets
    R, r_ = 46.0, 24.0
    ang = np.pi / 8 + np.arange(8) * np.pi / 4
    outer = [(64 + R * np.cos(a), 64 + R * np.sin(a)) for a in ang]
    inner = [(64 + r_ * np.cos(a), 64 + r_ * np.sin(a)) for a in ang]
    for k in range(8):
        a = ang[k] + np.pi / 8
        lit = -(np.cos(a) + np.sin(a)) * 0.7  # facing the top left
        col = DIAMOND[3] if lit > 0.5 else DIAMOND[2] if lit > -0.2 else DIAMOND[1] if lit > -0.8 else DIAMOND[0]
        q = [outer[k], outer[(k + 1) % 8], inner[(k + 1) % 8], inner[k]]
        c.fill(poly(q), col)
    c.fill(poly(inner), DIAMOND[2])
    c.fill(poly([inner[4], inner[5], inner[6], (64, 64)]), shift(DIAMOND[2], dv=0.08, ds=-0.1))
    # crease lines
    for k in range(8):
        c.fill(capsule(outer[k], inner[k], 1.0), DIAMOND[0])
    c.fill(poly([(46, 36), (52, 32), (40, 50), (36, 52)]), hexc("#effffd"))
    # small corner gems
    for cy in (24, 104):
        for cx in (24, 104):
            c.fill(poly([(cx, cy - 6), (cx + 6, cy), (cx, cy + 6), (cx - 6, cy)]), DIAMOND[2])
            c.fill(poly([(cx, cy - 6), (cx, cy), (cx - 6, cy)]), DIAMOND[3])
            c.fill(poly([(cx + 6, cy), (cx, cy + 6), (cx, cy)]), DIAMOND[0])
    return c.finish(opaque=True)


def paint_coal_block(seed):
    """Packed coal: big flat-shaded facets in near-black tones with sharp light creases, a
    couple of glossy glints."""
    c = Canvas()
    t = [COAL[1], flat.lerp(COAL[1], COAL[2], 0.5), COAL[2], shift(COAL[3], dv=-0.05)]
    c.facets(seed, 20, t, crease=hexc("#17171b"), crease_w=1.8, jitter=0.85, tilt=1.1)
    for (x, y) in flat.scatter(seed + 3, 4, 46):
        c.fill(tiled(ellipse(y, x, 1.5, 5.5, angle=-0.7, tile=False)), hexc("#80828d"))
    return c.finish(opaque=True)


# ---------------------------------------------------------------------------- torch


def paint_torch(seed):
    """Stick x 56..72 from y 64 down, glowing head y 48..64 (the game cuts the top fifth)."""
    c = Canvas(tile=False)
    c.fill(box(64, 56, 128, 72), STICK[1])
    c.fill(box(64, 56, 128, 60), STICK[2])
    c.fill(box(64, 67, 128, 72), STICK[0])
    c.fill(box(64, 56, 68, 72), shift(STICK[0], dv=-0.06))  # charred under the head
    c.fill(box(48, 56, 64, 72), hexc("#ff8a2a"))
    c.fill(box(48, 59, 61, 69, r=2), hexc("#ffcf5a"))
    c.fill(box(51, 61, 57, 65, r=1.5), hexc("#fff3c4"))
    return c.finish(cutout=True)


# ---------------------------------------------------------------------------- lantern / chain


def iron_part(c, shape, w=1.6, t=DARK_IRON):
    c.fill(shape, t[1])
    rim(c, shape, t[3], t[0], w)


def link(c, y0, x0, y1, x1, wire, clip=None, t=None):
    """A chain link / ring seen face on: a rounded rectangular loop of wire."""
    t = t or CHAIN
    outer = box(y0, x0, y1, x1, r=min(y1 - y0, x1 - x0) / 2)
    inner = box(y0 + wire, x0 + wire, y1 - wire, x1 - wire, r=max(min(y1 - y0, x1 - x0) / 2 - wire, 0.5))
    lp = minus(outer, inner)
    if clip is not None:
        lp0 = lp
        lp = lambda y, x: lp0(y, x) & clip(y, x)
    c.fill(lp, t[1])
    # light on the outer top/left and inner bottom/right edges, dark opposite
    c.fill(lambda y, x: lp(y, x) & ~outer(y - 1.6, x - 1.6), t[3])
    c.fill(lambda y, x: lp(y, x) & ~outer(y + 1.6, x + 1.6), t[0])
    c.fill(lambda y, x: lp(y, x) & inner(y + 1.4, x + 1.4) & ~inner(y, x), t[0])


CHAIN = tones("#5b606b", 4, 0.12)


def paint_lantern(seed):
    """Minecraft's lantern UV layout (8 px units), drawn flat: a dark iron cap and frame with
    a warm glowing glass, the lid, the hanging ring and the standing handle."""
    c = Canvas(tile=False)
    # cap sides (1,0)-(5,2)
    cap = box(0, 8, 16, 40)
    iron_part(c, cap, 2.0)
    c.fill(box(6, 10, 9, 38), DARK_IRON[0])
    # body sides (0,2)-(6,9): iron bands top and bottom, glass between two posts
    body = box(16, 0, 72, 48)
    c.fill(body, DARK_IRON[1])
    c.fill(box(24, 4, 64, 44), WARM[2])
    c.fill(box(24, 4, 64, 8), WARM[1])
    c.fill(box(24, 4, 27, 44), WARM[1])
    c.fill(poly([(12, 60), (30, 28), (36, 28), (18, 60)]), WARM[3])
    c.fill(disk(52, 30, 5, tile=False), WARM[3])
    for y0 in (16, 64):
        b = box(y0, 0, y0 + 8, 48)
        iron_part(c, b, 1.6)
    for x0 in (0, 44):
        p = box(24, x0, 64, x0 + 4)
        iron_part(c, p, 1.2)
    # body top/bottom (0,9)-(6,15) with the cap top (1,10)-(5,14) in the middle
    tb = box(72, 0, 120, 48)
    iron_part(c, tb, 2.0)
    lid = box(80, 8, 112, 40, r=5)
    iron_part(c, lid, 2.0)
    c.fill(disk(96, 24, 5, tile=False), DARK_IRON[0])
    c.fill(disk(95, 23, 2.5, tile=False), DARK_IRON[3])
    # hanging ring (11,1)-(14,5) and standing handle (11,10)-(14,12)
    link(c, 9, 89, 39, 111, 5.5, t=DARK_IRON)
    link(c, 81, 89, 111, 111, 5.5, clip=box(80, 88, 96, 112), t=DARK_IRON)
    return c.finish(cutout=True)


def paint_iron_chain(seed):
    """Minecraft's chain layout: two 24 px strips (the two crossed planes); face-on links
    alternate with edge-on ones, staggered between the strips; tiles vertically."""
    c = Canvas(tile=False)
    for x0, phase in ((0, 0), (24, 32)):
        for k in range(-1, 3):
            y = phase + k * 64
            for oy in (0, 128, -128):
                yy0 = y + oy
                if yy0 > 128 or yy0 + 40 < 0:
                    continue
                link(c, yy0 + 2, x0 + 3, yy0 + 38, x0 + 21, 5.5)
                # the edge-on link between: a thin rounded bar
                bar = box(yy0 + 30, x0 + 9.5, yy0 + 72, x0 + 14.5, r=2.5)
                c.fill(bar, CHAIN[1])
                c.fill(box(yy0 + 30, x0 + 9.5, yy0 + 72, x0 + 11.5, r=1), CHAIN[3])
                c.fill(box(yy0 + 30, x0 + 13, yy0 + 72, x0 + 14.5, r=1), CHAIN[0])
    return c.finish(cutout=True)


# ---------------------------------------------------------------------------- door

DOOR = tones("#a8794a", 4, 0.14)
DOOR_SEED = 777


def door_half(seed: int, top: bool) -> Canvas:
    """Door coordinates y 0..256 (top half 0..128): a frame of stiles and rails round three
    vertical boards; a round window with a cross bar high up, a Z brace low down, dark iron
    strap hinges on the left, a ring pull on the right."""
    c = Canvas(tile=False)
    oy = 0 if top else -128  # door y = texture y - oy

    def D(shape_fn):  # shape in door coordinates
        return lambda y, x: shape_fn(y - oy, x)

    c.fill(box(0, 0, 128, 128), OAK_GAP)
    # three vertical boards
    for k, (x0, x1) in enumerate(((10, 48), (48, 82), (82, 118))):
        b = D(box(0, x0 + 1.2, 256, x1 - 1.2, r=0))
        col = shift(DOOR[2], dv=(0.0, 0.03, -0.02)[k])
        c.fill(b, col)
        c.fill(D(box(0, x0 + 1.2, 256, x0 + 3.2)), shift(col, dv=0.07, ds=-0.05))
        c.fill(D(box(0, x1 - 3.2, 256, x1 - 1.2)), shift(col, dv=-0.1))
    # grain lines on the boards
    r = np.random.default_rng(seed)
    for k, (x0, x1) in enumerate(((10, 48), (48, 82), (82, 118))):
        for _ in range(2):
            gx = r.uniform(x0 + 8, x1 - 8)
            gy = r.uniform(0, 200)
            c.fill(D(capsule((gx, gy), (gx, gy + r.uniform(30, 60)), 2)), shift(DOOR[2], dv=-0.08))
    # Z brace (bottom half)
    brace = union(D(box(150, 12, 166, 116, r=3)), D(box(226, 12, 242, 116, r=3)),
                  D(poly([(16, 226), (32, 230), (112, 166), (96, 162)])))
    c.fill(brace, DOOR[1])
    rim(c, brace, DOOR[2], DOOR[0], 2.0)
    # outer stiles and rails
    frame = union(D(box(0, 0, 256, 12)), D(box(0, 116, 256, 128)), D(box(0, 0, 12, 128)),
                  D(box(244, 0, 256, 128)), D(box(122, 0, 136, 128)))
    c.fill(frame, DOOR[1])
    for s in (D(box(0, 0, 256, 12)), D(box(0, 116, 256, 128))):
        rim(c, s, DOOR[2], DOOR[0], 2.0)
    for s in (D(box(0, 0, 12, 128)), D(box(244, 0, 256, 128)), D(box(122, 0, 136, 128))):
        rim(c, s, DOOR[2], DOOR[0], 2.0, mask=lambda y, x: D(box(0, 12, 256, 116))(y, x))
    # round window
    win_c = (58.0, 64.0)
    wr = 30.0
    c.fill(D(disk(*win_c, wr + 6, tile=False)), DOOR[1])
    rim(c, D(disk(*win_c, wr + 6, tile=False)), DOOR[2], DOOR[0], 2.2)
    glass = D(disk(*win_c, wr, tile=False))
    c.erase(glass)
    bars = union(D(box(win_c[0] - 2.5, 30, win_c[0] + 2.5, 98)), D(box(24, 61.5, 92, 66.5)))
    c.fill(lambda y, x: bars(y, x) & glass(y, x), DOOR[1])
    c.fill(lambda y, x: D(box(win_c[0] + 1, 30, win_c[0] + 2.5, 98))(y, x) & glass(y, x), DOOR[0])
    c.fill(lambda y, x: D(box(24, 65, 92, 66.5))(y, x) & glass(y, x), DOOR[0])
    # hinges: dark iron straps on the left with a round end and two bolts
    for hy in (22, 222):
        st = union(D(box(hy, 0, hy + 10, 34, r=0)), D(disk(hy + 5, 34, 6.5, tile=False)))
        iron_part(c, st, 1.6)
        for bx in (14, 33):
            c.fill(D(disk(hy + 5, bx, 1.8, tile=False)), DARK_IRON[3])
    # ring pull on the right, at the middle of the door (in the top half)
    c.fill(D(disk(112, 104, 5, tile=False)), DARK_IRON[1])
    rim(c, D(disk(112, 104, 5, tile=False)), DARK_IRON[3], DARK_IRON[0], 1.4)
    pull = D(ring(119, 104, 4.5, 8, tile=False))
    c.fill(lambda y, x: pull(y, x) & (y - oy > 114), DARK_IRON[2])
    c.fill(lambda y, x: pull(y, x) & (y - oy > 122), DARK_IRON[0])
    return c


def paint_oak_door_top(seed):
    return door_half(DOOR_SEED, True).finish(cutout=True)


def paint_oak_door_bottom(seed):
    return door_half(DOOR_SEED, False).finish(cutout=True)


# ---------------------------------------------------------------------------- wool


def paint_white_wool(seed):
    """Knitted wool: columns of interlocking V stitches (two soft slanted loops each), in
    close cream tones, the loops lit on their upper left; tiles seamlessly."""
    c = Canvas()
    c.fill(np.ones_like(c.alpha, bool), WOOL[0])
    cw, rh = 16.0, 14.2222  # 8 columns, 9 rows
    for row in range(9):
        cy = row * rh + rh / 2
        for col in range(8):
            cx = col * cw + cw / 2
            for side, ang in ((-1, -0.5), (1, 0.5)):
                lx = cx + side * 3.7
                leg = tiled(ellipse(cy, lx, 8.6, 3.9, angle=ang, tile=False))
                c.fill(leg, WOOL[1])
                top = tiled(ellipse(cy - 0.9, lx - 0.6, 7.4, 3.0, angle=ang, tile=False))
                c.fill(top, WOOL[2])
                c.fill(minus(top, tiled(ellipse(cy + 0.6, lx + 0.7, 7.4, 3.0, angle=ang,
                                                tile=False))), WOOL[3])
    return c.finish(opaque=True)


# ---------------------------------------------------------------------------- bed

BED_WOOD = OAK


def quilt(c: Canvas, mask) -> None:
    """The blanket: flat red with diagonal quilting stitches (a diamond grid every 32 px)."""
    m = c.mask(mask)
    c.fill(m, RED[2])
    st = lambda y, x: ((np.abs(((x + y) % 32) - 16) < 0.9) | (np.abs(((x - y) % 32) - 16) < 0.9))
    c.fill(m & st(c.y, c.x), RED[1])
    # little lit puffs inside each diamond (upper left side of the cell)
    puff = lambda y, x: (((x + y) % 32 > 3) & ((x + y) % 32 < 10) & ((x - y) % 32 > 19) & ((x - y) % 32 < 29))
    c.fill(m & puff(c.y, c.x), RED[3])


def paint_red_bed_foot_up(seed):
    c = Canvas()
    quilt(c, box(0, 0, 128, 128))
    # the hem at the foot end
    c.fill(box(112, 0, 128, 128), RED[1])
    c.fill(box(112, 0, 114.5, 128), RED[3])
    c.fill(box(125.5, 0, 128, 128), RED[0])
    return c.finish(opaque=True)


def paint_red_bed_head_up(seed):
    c = Canvas(tile=False)
    # sheet under the pillow
    c.fill(box(0, 0, 128, 128), PILLOW[1])
    quilt(c, box(48, 0, 128, 128))
    # the blanket's turned-down white edge, then the blanket's rolled top
    c.fill(box(48, 0, 60, 128), PILLOW[2])
    c.fill(box(48, 0, 50.5, 128), PILLOW[3])
    c.fill(box(57.5, 0, 60, 128), PILLOW[0])
    c.fill(box(60, 0, 63, 128), RED[0])
    # pillow
    p = box(6, 14, 44, 114, r=12)
    c.fill(moved_shape(p, 2.5, 2.5), PILLOW[0])
    c.fill(p, PILLOW[2])
    rim(c, p, PILLOW[3], PILLOW[1], 3.0)
    c.fill(ellipse(25, 64, 3, 30, tile=False), PILLOW[1])
    return c.finish(opaque=True)


def bed_side(seed: int, sheet: str | None, legs) -> np.ndarray:
    """A bed side 9/16 tall: blanket (rows 56..88; on the head end the white sheet), the
    wooden frame (88..104) and legs (104..128, 24 px) at the outer corners."""
    c = Canvas(tile=False)
    band = box(56, 0, 88, 128)
    c.fill(band, RED[2])
    c.fill(box(56, 0, 59, 128), RED[3])
    c.fill(box(82, 0, 88, 128), RED[1])
    c.fill(box(86, 0, 88, 128), RED[0])
    # a stitched seam along the blanket's side
    for x in range(4, 128, 12):
        c.fill(box(70.2, x, 71.8, x + 6, r=0.8), RED[1])
    if sheet:
        x0, x1 = {"right": (80, 128), "left": (0, 48), "all": (0, 128)}[sheet]
        sh = box(56, x0, 88, x1)
        c.fill(sh, PILLOW[2])
        c.fill(box(56, x0, 59, x1), PILLOW[3])
        c.fill(box(84, x0, 88, x1), PILLOW[0])
        if sheet == "right":
            c.fill(box(56, 78, 88, 80.5), RED[0])
        elif sheet == "left":
            c.fill(box(56, 47.5, 88, 50), RED[0])
    # the frame rail runs on into the next piece: lit top, shaded bottom, no end rims
    c.fill(box(88, 0, 104, 128), OAK[2])
    c.fill(box(88, 0, 90.2, 128), OAK[3])
    c.fill(box(101.8, 0, 104, 128), OAK[0])
    for side in legs:
        x0 = 0 if side == "left" else 104
        lg = box(103, x0, 128, x0 + 24, r=0)
        c.fill(lg, OAK[1])
        rim(c, lg, OAK[2], OAK[0], 2.2)
        c.fill(box(103, x0, 105, x0 + 24), OAK[0])
    return c.finish(cutout=True)


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

_CRACKS: list = []
STAGE_AT = [4, 10, 20, 30, 42, 55, 68, 82, 98, 1e9]


def crack_segments():
    """Straight-segment crack lines: (p0, p1, width, time). Main cracks run out from a point
    near the middle and fork; later, cross cracks join neighbouring arms (a shattered web).
    A segment appears at the first stage whose limit reaches its time."""
    if _CRACKS:
        return _CRACKS
    r = np.random.default_rng(4646)
    segs = []
    arms = []  # per main arm: list of (point, time)
    start = np.array([62.0, 60.0])
    n_arms = 5
    base = r.uniform(0, 2 * np.pi)

    def grow(p, a, t, depth, width, arm):
        for step in range(20):
            ln = r.uniform(11, 17)
            a += r.uniform(-0.32, 0.32)
            q = p + ln * np.array([np.cos(a), np.sin(a)])
            if not (-6 < q[0] < 134 and -6 < q[1] < 134):
                segs.append((p, q, width, t))
                return
            segs.append((p, q, width, t))
            t += ln
            p = q
            if arm is not None:
                arm.append((p, t))
            if depth < 1 and step >= 1 and r.random() < 0.4:
                grow(p, a + r.choice([-1, 1]) * r.uniform(0.7, 1.1), t + 6, depth + 1,
                     width * 0.8, None)
            if depth > 0 and step > 1 + r.integers(0, 3):
                return

    for k in range(n_arms):
        a = base + k * 2 * np.pi / n_arms + r.uniform(-0.25, 0.25)
        arm = [(start, 0.0)]
        grow(start, a, (0.0, 6.0, 3.0, 12.0, 9.0)[k % 5], 0, 3.0, arm)
        arms.append(arm)
    # cross cracks: join points of neighbouring arms at similar distance (the web)
    for k in range(n_arms):
        A, B = arms[k], arms[(k + 1) % n_arms]
        for idx in (2, 3, 5):
            if idx < len(A) and idx < len(B):
                pa, ta = A[idx]
                pb, tb = B[min(idx + int(r.integers(-1, 2)), len(B) - 1)]
                mid = (pa + pb) / 2 + r.uniform(-4, 4, 2)
                t = max(ta, tb) + 14 + idx * 5
                segs.append((pa, mid, 2.2, t))
                segs.append((mid, pb, 2.2, t))
    _CRACKS.extend(segs)
    return _CRACKS


_CRACK_T: list = []


def crack_time() -> np.ndarray:
    """Per canvas sample: the earliest time a crack covers it (inf: never)."""
    if not _CRACK_T:
        c = Canvas(tile=False)
        tmin = np.full(c.alpha.shape, np.inf, np.float32)
        for p, q, w, t in crack_segments():
            m = c.mask(capsule(tuple(p), tuple(q), w))
            tmin[m] = np.minimum(tmin[m], t)
        _CRACK_T.append(tmin)
    return _CRACK_T[0]


def destroy(k):
    def paint_stage(seed):
        c = Canvas(tile=False)
        c.fill(crack_time() < STAGE_AT[k], np.array([60.0, 60.0, 60.0]))
        return c.finish(cutout=True)
    return paint_stage


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
