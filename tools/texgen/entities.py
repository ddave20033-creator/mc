"""Entity atlases (chest, pig, sheep, wolf) and particles (flame, smoke) in the game's own flat
look (see BRIEF.md and flat.py). (The player's skin, the game's own character, is in
`player.py`.)

Atlases use Minecraft's box UV layout (`box_uv`, see `src/content/mobs/` and
`src/textures/skin_pages.rs`) in a 64 unit wide atlas at 8 px per unit (chest and pig
512 x 512; sheep, its wool and the wolf 512 x 256). Every face is painted on its own small
`Canvas` and put at its exact place; space between faces stays transparent.

Orientation notes (they decide where light and shadow go):
- The chest model is stored upside down: its side faces are painted the right way up and
  flipped vertically into the atlas (`src/textures/from_pack.rs` flips them back). A double
  chest half has no frame on the seam edge (the game finds that edge by it).
- The pig's, sheep's and wolf's bodies (and the wolf's mane) are boxes turned on their side:
  their `front` face is the belly, `back` the back, `top` the neck end, `bottom` the rear;
  on the `right` face the animal's back is the left edge and the belly the right edge (the
  `left` face mirrored). Their side faces run from the head (top) to the rear (bottom).
- Head side faces: the `right` face's right edge touches the face (`front`), the `left`
  face's left edge does; the `top` face's lower edge does.
"""

from __future__ import annotations

import numpy as np

from flat import Canvas, box, capsule, disk, ellipse, hexc, minus, poly, union

K = 8  # px per model unit

# ---------------------------------------------------------------------------- helpers


def box_uv(u, v, dx, dy, dz):
    """Minecraft's box UV faces (x, y, w, h in units) for a cube at texture offset (u, v)."""
    return {
        "top": (u + dz, v, dx, dz),
        "bottom": (u + dz + dx, v, dx, dz),
        "right": (u, v + dz, dz, dy),
        "front": (u + dz, v + dz, dx, dy),
        "left": (u + dz + dx, v + dz, dz, dy),
        "back": (u + 2 * dz + dx, v + dz, dx, dy),
    }


def C(h: str) -> np.ndarray:
    return hexc(h)


def full(y, x):
    return np.ones(y.shape, bool)


def D(cy, cx, r):
    return disk(cy, cx, r, tile=False)


def E(cy, cx, ry, rx, a=0.0):
    return ellipse(cy, cx, ry, rx, a, tile=False)


def B(y0, x0, y1, x1, r=0.0):
    return box(y0, x0, y1, x1, r)


def inter(*shapes):
    return lambda y, x: np.logical_and.reduce([s(y, x) for s in shapes])


def below(p0, p1):
    """The half plane under the line through (x, y) points p0 -> p1."""
    (x0, y0), (x1, y1) = p0, p1
    return lambda y, x: (y - y0) * (x1 - x0) - (x - x0) * (y1 - y0) > 0


def paint(w, h, fn, cutout=False) -> np.ndarray:
    """A face `w` x `h` px drawn by `fn(canvas)`, finished (opaque unless `cutout`)."""
    c = Canvas(w=int(round(w)), h=int(round(h)), tile=False)
    fn(c)
    return c.finish(cutout=cutout, opaque=not cutout)


class Atlas:
    def __init__(self, wu: int, hu: int):
        self.a = np.zeros((hu * K, wu * K, 4), np.float32)

    def put(self, img, xu, yu, flip_v=False, flip_h=False):
        if flip_v:
            img = img[::-1]
        if flip_h:
            img = img[:, ::-1]
        h, w = img.shape[:2]
        y, x = int(round(yu * K)), int(round(xu * K))
        self.a[y : y + h, x : x + w] = img

    def face(self, rect, fn, **kw):
        x, y, w, h = rect
        self.put(paint(w * K, h * K, fn), x, y, **kw)


def curls(c, tn, r=9.0, seed=0, region=None):
    """Wool: rows of round curls, each row overlapping the one above (scallops hanging down),
    every curl a raised disk lit from the top left. `tn` = (shadow, base, light)."""
    rr = np.random.default_rng(seed)
    dy, dx = r * 1.1, r * 1.75
    y, row = -r * 0.3, 0
    clip = full if region is None else region
    while y < c.h + r:
        x = -dx + (row % 2) * dx * 0.5
        while x < c.w + dx:
            jy, jx = rr.uniform(-1, 1, 2) * r * 0.12
            rad = r * rr.uniform(0.94, 1.06)
            c.raised(inter(D(y + jy, x + jx, rad), clip), tn[1], tn[2], tn[0], rim=max(1.5, r * 0.2))
            x += dx
        y += dy
        row += 1


def spiral(cy, cx, r0, r1, turns, width, a0=0.0):
    """A stroke along a spiral from radius r0 out to r1."""
    n = int(24 * turns)
    pts = []
    for i in range(n + 1):
        t = i / n
        a = a0 + t * turns * 2 * np.pi
        rad = r0 + (r1 - r0) * t
        pts.append((cx + np.cos(a) * rad, cy + np.sin(a) * rad))
    return union(*[capsule(p, q, width) for p, q in zip(pts[:-1], pts[1:])])


# ---------------------------------------------------------------------------- chest

WOOD = dict(base=C("#b98a55"), alt=C("#b2834f"), light=C("#d3a76f"), dark=C("#966a3e"),
            gap=C("#6e4a2b"), under=C("#835c38"))
INNER = dict(base=C("#7c5634"), alt=C("#76512f"), light=C("#8e6540"), dark=C("#664529"),
             gap=C("#4a321f"), under=C("#4a321f"))
IRON = dict(base=C("#3d4048"), light=C("#5f6470"), dark=C("#25272d"), rivet=C("#767c88"),
            rivet_l=C("#a3a9b3"))
BAND_V, BAND_H = 10, 8  # iron band widths (px) on the vertical / horizontal edges


def boards(c, y0, y1, n, pal=WOOD):
    """`n` horizontal boards between rows y0..y1, running past both edges (no ends seen)."""
    c.fill(B(y0, -20, y1, c.w + 20), pal["gap"])
    hgt = (y1 - y0) / n
    for i in range(n):
        a = y0 + i * hgt
        base = pal["base"] if i % 2 == 0 else pal["alt"]
        c.raised(B(a + 1, -20, a + hgt - 1, c.w + 20, r=2), base, pal["light"], pal["dark"], rim=2)


def iron_frame(c, top, bottom, left, right, pal=IRON, r=5.0, hb=BAND_H):
    """Dark iron bands along the chosen edges (`hb` px wide at the bottom), their inner
    corners rounded."""
    out = -40
    inner = B(BAND_H if top else out, BAND_V if left else out,
              c.h - (hb if bottom else out), c.w - (BAND_V if right else out), r=r)
    c.raised(minus(full, inner), pal["base"], pal["light"], pal["dark"], rim=2)


def rivet(c, y, x, r=2.6):
    c.raised(D(y, x, r), IRON["rivet"], IRON["rivet_l"], IRON["dark"], rim=1.0)


def chest_lid_side(w, left=True, right=True):
    """A lid side face (world orientation, top up): 5 units tall."""
    def fn(c):
        boards(c, BAND_H, c.h - 6, 1)
        iron_frame(c, True, True, left, right, r=4, hb=6)
        for x, on in ((BAND_V / 2, left), (c.w - BAND_V / 2, right)):
            if on:
                rivet(c, BAND_H / 2, x)
    return paint(w, 5 * K, fn)


def chest_body_side(w, left=True, right=True):
    """A body side face (world orientation): 10 units tall, the lid's shadow along its top."""
    def fn(c):
        boards(c, 0, c.h - BAND_H, 3)
        c.fill(B(-2, -20, 3, c.w + 20), WOOD["under"])
        iron_frame(c, False, True, left, right)
        for x, on in ((BAND_V / 2, left), (c.w - BAND_V / 2, right)):
            if on:
                rivet(c, c.h - BAND_H / 2, x)
                rivet(c, 26, x)
    return paint(w, 10 * K, fn)


def chest_square(w, left=True, right=True, kind="top"):
    """A 14 unit deep face: the lid top / body bottom (`top`), the lid's inside (`inside`) or
    the open body seen from above (`open`)."""
    def fn(c):
        if kind == "top":
            boards(c, 0, c.h, 4)
            iron_frame(c, True, True, left, right)
            for x, on in ((BAND_V / 2, left), (c.w - BAND_V / 2, right)):
                if on:
                    rivet(c, BAND_H / 2, x)
                    rivet(c, c.h - BAND_H / 2, x)
        elif kind == "inside":
            boards(c, 0, c.h, 4, INNER)
        else:
            boards(c, 0, c.h, 4, INNER)
            out = -40
            inner = B(8, 8 if left else out, c.h - 8, c.w - (8 if right else out), r=4)
            c.raised(minus(full, inner), WOOD["base"], WOOD["light"], WOOD["dark"], rim=2)
    return paint(w, 14 * K, fn)


def latch(at: Atlas, wu):
    """The latch box (0, 0) wu x 4 x 1: dark iron, a raised plate with a keyhole in front."""
    faces = box_uv(0, 0, wu, 4, 1)
    for n, rect in faces.items():
        if n == "front":
            continue
        at.face(rect, lambda c: c.raised(B(-4, -4, c.h + 4, c.w + 4), IRON["base"], None, None))
    def front(c):
        c.fill(full, IRON["dark"])
        c.raised(B(1, 1, c.h - 1, c.w - 1, r=3), IRON["base"], IRON["light"], IRON["dark"], rim=2)
        cx = c.w / 2
        c.fill(union(D(c.h * 0.45, cx, min(2.6, c.w * 0.2)), B(c.h * 0.45, cx - 1.1, c.h * 0.62, cx + 1.1)),
               C("#15161a"))
    at.face(faces["front"], front)


def chest_single(seed: int) -> np.ndarray:
    at = Atlas(64, 64)
    latch(at, 2)
    for x in (0, 14, 28, 42):
        at.put(chest_lid_side(14 * K), x, 14, flip_v=True)
        at.put(chest_body_side(14 * K), x, 33, flip_v=True)
    at.put(chest_square(14 * K, kind="inside"), 14, 0)
    at.put(chest_square(14 * K), 28, 0)
    at.put(chest_square(14 * K), 14, 19)
    at.put(chest_square(14 * K, kind="open"), 28, 19)
    return at.a


def chest_double(seed: int, left: bool) -> np.ndarray:
    """One half of a double chest (long faces 15 units) with no frame on the seam edge: the
    left half's squares and face 14 open on the left, face 43 on the right (the right half
    the other way round)."""
    at = Atlas(64, 64)
    latch(at, 1)
    for x in (0, 29):  # the ends: framed on both sides
        at.put(chest_lid_side(14 * K), x, 14, flip_v=True)
        at.put(chest_body_side(14 * K), x, 33, flip_v=True)
    for x, open_left in ((14, left), (43, not left)):
        at.put(chest_lid_side(15 * K, not open_left, open_left), x, 14, flip_v=True)
        at.put(chest_body_side(15 * K, not open_left, open_left), x, 33, flip_v=True)
    sl, sr = not left, left  # squares: framed sides
    at.put(chest_square(15 * K, sl, sr, "inside"), 14, 0)
    at.put(chest_square(15 * K, sl, sr), 29, 0)
    at.put(chest_square(15 * K, sl, sr), 14, 19)
    at.put(chest_square(15 * K, sl, sr, "open"), 29, 19)
    return at.a


# ---------------------------------------------------------------------------- pig

PIG = dict(light=C("#f8c0bb"), base=C("#f0a0a0"), shade=C("#d9707a"), deep=C("#b9566a"),
           snout=C("#f5b0ae"), nostril=C("#a94d61"), hoof=C("#93505e"), hoof_l=C("#a9606c"),
           eye=C("#2e2228"), white=C("#ffffff"))


def eye(c, cy, cx, r, color, glint=C("#ffffff")):
    c.fill(D(cy, cx, r), color)
    c.fill(D(cy - r * 0.35, cx - r * 0.35, r * 0.36), glint)


def pig(seed: int) -> np.ndarray:
    at = Atlas(64, 64)
    P = PIG
    hd = box_uv(0, 0, 8, 8, 8)

    # ---- head
    def head_top(c):
        c.fill(full, P["base"])
        c.fill(E(24, 22, 22, 16), P["light"])

    def head_side(c):  # the right side: the face is on the right edge
        c.fill(full, P["base"])
        c.fill(E(78, 32, 26, 50), P["shade"])  # the jowl
        c.fill(E(10, 16, 10, 20), P["light"])
        # a floppy ear folding forward over the side's front top
        ear = E(16, 44, 9, 15, -0.55)
        c.raised(ear, P["shade"], P["base"], P["deep"], rim=2)
        c.fill(E(17.5, 45.5, 4.5, 9, -0.55), P["deep"])

    def head_front(c):
        c.fill(full, P["base"])
        c.fill(E(4, 18, 14, 26), P["light"])
        c.fill(E(78, 32, 26, 50), P["shade"])
        for x in (11, 53):
            eye(c, 24, x, 5.5, P["eye"])

    def head_back(c):
        c.fill(full, P["base"])
        c.fill(E(76, 32, 22, 46), P["shade"])

    at.face(hd["top"], head_top)
    at.face(hd["bottom"], lambda c: c.fill(full, P["shade"]))
    at.face(hd["right"], head_side)
    at.put(paint(64, 64, head_side), *hd["left"][:2], flip_h=True)
    at.face(hd["front"], head_front)
    at.face(hd["back"], head_back)

    # ---- snout (16, 16) 4 x 3 x 1: a soft round snout with two nostrils
    sn = box_uv(16, 16, 4, 3, 1)

    def snout_front(c):
        c.fill(full, P["shade"])
        c.raised(B(0.5, 0.5, c.h - 0.5, c.w - 0.5, r=8), P["snout"], P["light"], P["shade"], rim=2)
        for x in (10.5, 21.5):
            c.fill(E(12.5, x, 4.6, 3.0), P["nostril"])
            c.fill(E(11, x - 0.8, 1.6, 1.1), P["deep"])

    at.face(sn["front"], snout_front)
    for n in ("top", "right", "left"):
        at.face(sn[n], lambda c: c.fill(full, P["snout"]))
    at.face(sn["bottom"], lambda c: c.fill(full, P["shade"]))
    at.face(sn["back"], lambda c: c.fill(full, P["base"]))

    # ---- body (28, 8) 10 x 16 x 8, on its side
    bd = box_uv(28, 8, 10, 16, 8)

    def flank(c):  # right flank: back on the left edge, belly on the right
        c.fill(full, P["shade"])
        c.fill(E(64, 10, 80, 40), P["base"])
        c.fill(E(64, -2, 56, 18), P["light"])

    def belly(c):
        c.fill(full, P["shade"])

    def back(c):
        c.fill(full, P["base"])
        c.fill(E(64, 34, 54, 22), P["light"])

    def neck(c):
        c.fill(full, P["base"])
        c.fill(E(66, 40, 18, 46), P["shade"])

    def rear(c):
        c.fill(full, P["base"])
        c.fill(E(68, 40, 16, 46), P["shade"])
        c.fill(D(30, 40, 4.0), P["shade"])
        c.fill(spiral(30, 40, 2.0, 11.0, 1.6, 3.6, a0=np.pi), P["deep"])

    at.face(bd["right"], flank)
    at.put(paint(64, 128, flank), *bd["left"][:2], flip_h=True)
    at.face(bd["front"], belly)
    at.face(bd["back"], back)
    at.face(bd["top"], neck)
    at.face(bd["bottom"], rear)

    # ---- legs (0, 16) 4 x 6 x 4: a hoof at the bottom
    lg = box_uv(0, 16, 4, 6, 4)

    def leg(split):
        def fn(c):
            c.fill(full, P["base"])
            c.fill(B(-4, c.w - 9, c.h + 4, c.w + 4), P["shade"])
            c.raised(B(c.h - 10, -4, c.h + 4, c.w + 4, r=3), P["hoof"], P["hoof_l"], None, rim=2)
            if split:
                c.fill(B(c.h - 8, c.w / 2 - 1, c.h + 1, c.w / 2 + 1), C("#6f3a47"))
        return fn

    for n in ("right", "left", "back"):
        at.face(lg[n], leg(False))
    at.face(lg["front"], leg(True))
    at.face(lg["top"], lambda c: c.fill(full, P["base"]))

    def sole(c):
        c.fill(full, P["hoof"])
        c.fill(B(-2, c.w / 2 - 1, c.h + 2, c.w / 2 + 1), C("#6f3a47"))

    at.face(lg["bottom"], sole)
    return at.a


# ---------------------------------------------------------------------------- sheep

WOOL = (C("#d8d1c4"), C("#eeeae2"), C("#fbf9f4"))
WOOL_DIM = (C("#c9c1b3"), C("#ddd7cc"), C("#ebe6dd"))
SKIN = (C("#d9c6b4"), C("#e6d6c7"), C("#f0e5da"))
FACE = dict(base=C("#c9b9a5"), light=C("#d9cbb9"), dark=C("#a8957f"), deep=C("#8d7a66"),
            nose=C("#8a6a5e"), eye=C("#2b2420"), hoof=C("#5f544b"), hoof_l=C("#766a5f"))


def flank_faces(at: Atlas, bd, paint_world, du, lu):
    """The right/left faces of a body on its side from a world-oriented painter (`du` units
    tall: back to belly, `lu` long: head to rear)."""
    world = paint(lu * K, du * K, paint_world)
    at.put(world.transpose(1, 0, 2), *bd["right"][:2])
    at.put(world.transpose(1, 0, 2)[:, ::-1], *bd["left"][:2])


def sheep(seed: int) -> np.ndarray:
    at = Atlas(64, 32)
    F = FACE
    hd = box_uv(0, 0, 6, 6, 8)

    def head_front(c):
        c.fill(full, F["base"])
        c.fill(E(2, 14, 14, 22), F["light"])
        for x in (12.5, 35.5):
            eye(c, 21, x, 4.4, F["eye"])
        # nose: a soft rounded triangle and a little Y of a mouth
        c.fill(union(poly([(18, 31), (30, 31), (24, 38.5)]), D(32.5, 19.5, 2.2), D(32.5, 28.5, 2.2)),
               F["nose"])
        c.fill(capsule((24, 37), (24, 41), 1.8), F["deep"])
        c.fill(capsule((24, 41), (20.5, 43.5), 1.8), F["deep"])
        c.fill(capsule((24, 41), (27.5, 43.5), 1.8), F["deep"])

    def head_side(c):  # right side, 8 x 6 units: the face's 2 units on the right edge
        c.fill(full, WOOL[1])
        curls(c, WOOL, 8.0, seed + 1, region=lambda y, x: x < 48)
        c.fill(B(-4, 47, c.h + 4, c.w + 4, r=6), F["base"])
        c.fill(B(c.h - 10, 47, c.h + 4, c.w + 4), F["dark"])
        # the ear: a leaf pointing back, its inside darker
        c.raised(E(17, 45, 5.5, 12, 0.35), F["base"], F["light"], F["dark"], rim=2)
        c.fill(E(17.5, 44, 2.6, 8, 0.35), F["dark"])

    def head_top(c):  # 6 x 8: the face's 2 units on the lower edge
        c.fill(full, WOOL[1])
        curls(c, WOOL, 8.0, seed + 2, region=lambda y, x: y < 48)
        c.fill(B(47, -4, c.h + 4, c.w + 4, r=6), F["base"])

    at.face(hd["front"], head_front)
    at.face(hd["right"], head_side)
    at.put(paint(64, 48, head_side), *hd["left"][:2], flip_h=True)
    at.face(hd["top"], head_top)
    at.face(hd["bottom"], lambda c: c.fill(full, F["dark"]))
    at.face(hd["back"], lambda c: (c.fill(full, WOOL[1]), curls(c, WOOL, 8.0, seed + 3)))

    # ---- body (28, 8) 8 x 16 x 6, sheared: cropped cream skin with faint short curls
    bd = box_uv(28, 8, 8, 16, 6)

    def skin(tn, r=6.0, s=0):
        return lambda c: (c.fill(full, tn[1]), curls(c, tn, r, seed + 10 + s))

    flank_faces(at, bd, skin(SKIN, s=1), 6, 16)
    at.face(bd["back"], skin(SKIN, s=2))
    at.face(bd["front"], skin((C("#c9b5a3"), C("#d8c6b6"), C("#e2d3c5")), s=3))
    at.face(bd["top"], skin(SKIN, s=4))
    at.face(bd["bottom"], skin(SKIN, s=5))

    # ---- legs (0, 16) 4 x 12 x 4: woolly tops, the face colour below, dark hooves
    lg = box_uv(0, 16, 4, 12, 4)

    def leg(i):
        def fn(c):
            c.fill(full, F["base"])
            c.fill(B(-4, c.w - 8, c.h + 4, c.w + 4), F["dark"])
            c.raised(B(c.h - 9, -4, c.h + 4, c.w + 4, r=2.5), F["hoof"], F["hoof_l"], None, rim=2)
            c.fill(B(-4, -4, 26, c.w + 4), WOOL[1])
            for x in range(-4, c.w + 8, 10):  # the wool's scalloped hem
                c.raised(D(26, x + (i % 2) * 5, 6.0), WOOL[1], WOOL[2], WOOL[0], rim=1.8)
        return fn

    for i, n in enumerate(("right", "front", "left", "back")):
        at.face(lg[n], leg(i))
    at.face(lg["top"], lambda c: (c.fill(full, WOOL[1]), curls(c, WOOL, 7.0, seed + 20)))
    at.face(lg["bottom"], lambda c: c.fill(full, F["hoof"]))
    return at.a


def sheep_wool(seed: int) -> np.ndarray:
    """The wool coat (tinted by the sheep's colour): cream curls everywhere, dimmer under the
    belly and under the head and legs."""
    at = Atlas(64, 32)

    def wool(tn=WOOL, r=9.0, s=0):
        return lambda c: (c.fill(full, tn[1]), curls(c, tn, r, seed + s))

    for i, (n, rect) in enumerate(box_uv(0, 0, 6, 6, 6).items()):
        at.face(rect, wool(WOOL_DIM if n == "bottom" else WOOL, 9.5, i))
    bd = box_uv(28, 8, 8, 16, 6)
    flank_faces(at, bd, wool(WOOL, 11.0, 10), 6, 16)
    at.face(bd["back"], wool(WOOL, 9.0, 11))
    at.face(bd["front"], wool(WOOL_DIM, 9.0, 12))
    at.face(bd["top"], wool(WOOL, 9.0, 13))
    at.face(bd["bottom"], wool(WOOL, 9.0, 14))
    for i, (n, rect) in enumerate(box_uv(0, 16, 4, 6, 4).items()):
        at.face(rect, wool(WOOL_DIM if n == "bottom" else WOOL, 8.0, 20 + i))
    return at.a


# ---------------------------------------------------------------------------- wolf

WOLF = dict(base=C("#b9b4ad"), dark=C("#7d756c"), mid=C("#9d968d"), light=C("#d6d1c9"),
            cream=C("#e6e1d8"), nose=C("#2d2a28"), nose_l=C("#57524e"), eye=C("#2b2622"),
            amber=C("#e2a646"), red=C("#c63c2e"), red_d=C("#8f2a22"), ear_in=C("#c4b3a8"),
            pad=C("#4a4440"), white=C("#ffffff"), mouth=C("#5b4f49"))
WOLF_BOXES = {
    "head": box_uv(0, 0, 6, 6, 4), "ear": box_uv(16, 14, 2, 2, 1), "snout": box_uv(0, 10, 3, 3, 4),
    "body": box_uv(18, 14, 6, 9, 6), "mane": box_uv(21, 0, 8, 6, 7), "leg": box_uv(0, 18, 2, 8, 2),
    "tail": box_uv(9, 18, 2, 8, 2),
}


def wolf_eye(c, cx, mood, side):
    """An eye on the head's front (48 x 48) at column `cx`; `side` -1 left, +1 right of the
    face (the nose is towards the middle)."""
    W = WOLF
    cy = 20.0
    inward = -side  # towards the nose, in x
    if mood == "tame":  # round, dark, a big glint, a soft light brow above
        c.fill(E(cy - 7.5, cx, 2.6, 6.0), W["light"])
        c.fill(D(cy, cx, 4.6), W["eye"])
        c.fill(D(cy - 1.6, cx - 1.5, 1.9), W["white"])
        c.fill(D(cy + 1.8, cx + 1.8, 0.9), W["white"])
    elif mood == "angry":  # narrowed under a dark slanted brow, a red rim
        lid = below((cx - 7, cy - 4.5 + inward * 2.2), (cx + 7, cy - 4.5 - inward * 2.2))
        lid_lo = below((cx - 7, cy - 1.8 + inward * 2.2), (cx + 7, cy - 1.8 - inward * 2.2))
        c.fill(inter(E(cy, cx, 4.6, 7.0), lid), W["red"])
        c.fill(inter(E(cy + 0.3, cx, 3.2, 5.4), lid_lo), W["amber"])
        c.fill(inter(E(cy + 0.4, cx + inward * 0.8, 3.0, 1.5), lid_lo), W["eye"])
        brow = minus(B(cy - 12, cx - 8, cy + 2, cx + 8, r=2), lid)
        c.fill(inter(brow, lambda y, x: y > cy - 9.5 + inward * (x - cx) * -0.32 - 3.0), W["dark"])
    else:  # wild: an almond amber eye with a dark rim and pupil
        c.fill(E(cy, cx, 3.8, 6.4), W["eye"])
        c.fill(E(cy, cx, 2.5, 4.8), W["amber"])
        c.fill(D(cy, cx + inward * 0.6, 2.0), W["eye"])
        c.fill(D(cy - 1.0, cx - 1.6, 0.9), W["white"])


def wolf(mood="wild"):
    def painter(seed: int) -> np.ndarray:
        at = Atlas(64, 32)
        W = WOLF
        X = WOLF_BOXES

        # ---- head (0, 0) 6 x 6 x 4
        def head_front(c):
            c.fill(full, W["base"])
            # a darker forehead mark running down between the eyes
            c.fill(union(E(-2, 24, 12, 13), poly([(15, 6), (33, 6), (24, 18)])), W["mid"])
            # the light mask: cheeks and the muzzle's surround (the snout covers the middle)
            c.fill(union(E(44, 10, 13, 13), E(44, 38, 13, 13), B(28, 8, 52, 40)), W["light"])
            for cx, side in ((12.0, -1), (36.0, 1)):
                wolf_eye(c, cx, mood, side)

        def head_side(c):  # right side 4 x 6: the face on the right edge
            c.fill(full, W["base"])
            c.fill(E(4, 10, 10, 16), W["mid"])
            c.fill(E(50, 22, 16, 22), W["light"])

        def head_top(c):
            c.fill(full, W["base"])
            c.fill(E(4, 24, 18, 14), W["mid"])

        at.face(X["head"]["front"], head_front)
        at.face(X["head"]["right"], head_side)
        at.put(paint(32, 48, head_side), *X["head"]["left"][:2], flip_h=True)
        at.face(X["head"]["top"], head_top)
        at.face(X["head"]["bottom"], lambda c: c.fill(full, W["light"]))
        at.face(X["head"]["back"], lambda c: (c.fill(full, W["base"]), c.fill(E(0, 24, 20, 30), W["mid"])))

        # ---- ears (16, 14) 2 x 2 x 1: dark, a light inside on the front
        def ear_front(c):
            c.fill(full, W["dark"])
            c.fill(poly([(4, 15), (12, 15), (8, 3.5)]), W["ear_in"])

        at.face(X["ear"]["front"], ear_front)
        for n in ("top", "bottom", "right", "left", "back"):
            at.face(X["ear"][n], lambda c: c.fill(full, W["dark"]))

        # ---- snout (0, 10) 3 x 3 x 4: light, the black nose on its front top
        def snout_front(c):  # 24 x 24
            c.fill(full, W["cream"])
            c.fill(union(E(4, 12, 6.0, 8.0), B(-4, 4, 4, 20)), W["nose"])
            c.fill(E(2.6, 9.5, 1.6, 2.8), W["nose_l"])
            if mood == "angry":  # bared teeth along the bottom
                c.fill(B(15, 1, c.h + 2, c.w - 1, r=2), W["mouth"])
                for x in (5.5, 12, 18.5):
                    c.fill(poly([(x - 2.4, 15), (x + 2.4, 15), (x, 21.5)]), W["white"])
            else:
                c.fill(capsule((12, 10), (12, 17), 1.6), W["mouth"])
                c.fill(capsule((12, 17), (5, 20), 1.6), W["mouth"])
                c.fill(capsule((12, 17), (19, 20), 1.6), W["mouth"])

        def snout_side(c):  # right side 4 x 3, the front on the right edge
            c.fill(full, W["light"])
            c.fill(B(-4, -4, 7, c.w + 4), W["base"])
            c.fill(capsule((6, 18), (c.w + 2, 17), 1.8), W["mouth"])

        def snout_top(c):  # 3 x 4, the front on the lower edge
            c.fill(full, W["light"])
            c.fill(E(0, 12, 14, 9), W["base"])
            c.fill(B(c.h - 5, 4, c.h + 4, c.w - 4, r=3), W["nose"])

        at.face(X["snout"]["front"], snout_front)
        at.face(X["snout"]["right"], snout_side)
        at.put(paint(32, 24, snout_side), *X["snout"]["left"][:2], flip_h=True)
        at.face(X["snout"]["top"], snout_top)
        at.face(X["snout"]["bottom"], lambda c: c.fill(full, W["cream"]))
        at.face(X["snout"]["back"], lambda c: c.fill(full, W["base"]))

        # ---- body (18, 14) 6 x 9 x 6 on its side: a dark saddle, a light belly
        bd = X["body"]

        def flank(c):  # world orientation: 6 units tall (back up), 9 long (head left)
            c.fill(full, W["base"])
            c.fill(union(E(-4, 36, 18, 44), B(-4, -4, 9, c.w + 4)), W["dark"])
            c.fill(E(52, 36, 14, 40), W["light"])

        flank_faces(at, bd, flank, 6, 9)

        def saddle(c):
            c.fill(full, W["mid"])
            c.fill(E(36, 24, 40, 18), W["dark"])

        at.face(bd["back"], saddle)
        at.face(bd["front"], lambda c: (c.fill(full, W["light"]), c.fill(E(36, 24, 30, 14), W["cream"])))
        at.face(bd["top"], lambda c: (c.fill(full, W["base"]), c.fill(B(-4, -4, 8, 52), W["dark"])))

        def rump(c):
            c.fill(full, W["base"])
            c.fill(B(-4, -4, 8, 52), W["dark"])
            c.fill(E(48, 24, 10, 18), W["light"])
            c.fill(D(20, 24, 8), W["mid"])

        at.face(bd["bottom"], rump)

        # ---- mane (21, 0) 8 x 6 x 7 on its side, shaggier: tufts along its rear edge
        mn = X["mane"]

        def mane_flank(c):  # world: 8 units tall (back up), 7 long (head left)
            c.fill(full, W["base"])
            c.fill(union(E(-6, 28, 22, 40), B(-4, -4, 10, c.w + 4)), W["dark"])
            c.fill(E(62, 24, 16, 30), W["light"])
            for y in range(-4, c.h + 8, 12):  # fur tufts hanging over the body
                c.fill(poly([(c.w + 2, y), (c.w + 2, y + 12), (c.w - 9, y + 6)]), W["mid"])

        flank_faces(at, mn, mane_flank, 7, 6)

        def mane_back(c):
            c.fill(full, W["dark"])
            for x in range(-4, c.w + 8, 12):
                c.fill(poly([(x, c.h + 2), (x + 12, c.h + 2), (x + 6, c.h - 9)]), W["mid"])

        def mane_chest(c):
            c.fill(full, W["light"])
            for x in range(-4, c.w + 8, 12):
                c.fill(poly([(x, c.h + 2), (x + 12, c.h + 2), (x + 6, c.h - 9)]), W["cream"])

        at.face(mn["back"], mane_back)
        at.face(mn["front"], mane_chest)
        at.face(mn["top"], lambda c: (c.fill(full, W["base"]), c.fill(B(-4, -4, 10, c.w + 4), W["dark"]),
                                      c.fill(B(c.h - 12, -4, c.h + 4, c.w + 4), W["light"])))
        at.face(mn["bottom"], lambda c: (c.fill(full, W["mid"]), c.fill(B(-4, -4, 10, c.w + 4), W["dark"])))

        # ---- legs (0, 18) 2 x 8 x 2: light socks
        lg = X["leg"]

        def leg(c):
            c.fill(full, W["base"])
            c.fill(B(-4, c.w - 5, c.h + 4, c.w + 4), W["mid"])
            c.fill(B(c.h - 14, -4, c.h + 4, c.w + 4, r=3), W["light"])

        for n in ("right", "front", "left", "back"):
            at.face(lg[n], leg)
        at.face(lg["top"], lambda c: c.fill(full, W["mid"]))
        at.face(lg["bottom"], lambda c: (c.fill(full, W["light"]), c.fill(D(8, 8, 5), W["pad"])))

        # ---- tail (9, 18) 2 x 8 x 2: root at the top, a light tip at the bottom
        tl = X["tail"]

        def tail(dark):
            def fn(c):
                c.fill(full, W["dark"] if dark else W["base"])
                c.fill(B(c.h - 16, -4, c.h + 4, c.w + 4, r=3), W["cream"])
            return fn

        for n in ("right", "front", "left"):
            at.face(tl[n], tail(False))
        at.face(tl["back"], tail(True))
        at.face(tl["top"], lambda c: c.fill(full, W["dark"]))
        at.face(tl["bottom"], lambda c: c.fill(full, W["cream"]))
        return at.a

    return painter


def wolf_collar(seed: int) -> np.ndarray:
    """Only the collar, in light neutral greys (the game tints it): a band round the top of
    the mane's four side faces (its neck end), a frame round the mane's front end (its `top`
    face) and a small round tag hanging under the chest. Transparent elsewhere."""
    at = Atlas(64, 32)
    G = dict(base=C("#e2e2e2"), light=C("#f6f6f6"), dark=C("#b4b4b4"), deep=C("#8e8e8e"))
    x, y, w, h = WOLF_BOXES["mane"]["top"]

    def frame(c):
        c.raised(minus(full, B(4, 4, c.h - 4, c.w - 4, r=2)), G["base"], G["light"], G["dark"], rim=1.5)

    at.put(paint(w * K, h * K, frame, cutout=True), x, y)
    x0 = WOLF_BOXES["mane"]["right"][0]
    bx, by, bw, _ = WOLF_BOXES["mane"]["back"]
    fx, _, fw, _ = WOLF_BOXES["mane"]["front"]
    span = bx + bw - x0

    def band(c):
        c.fill(full, G["base"])
        c.fill(B(-2, -4, 1.5, c.w + 4), G["light"])
        c.fill(B(6.5, -4, 10, c.w + 4), G["dark"])
        cx = (fx + fw / 2 - x0) * K  # the buckle under the chin
        c.raised(B(0.5, cx - 6, 7.5, cx + 6, r=2), G["dark"], G["light"], G["deep"], rim=1.2)
        c.fill(B(2.5, cx - 3.5, 5.5, cx + 3.5, r=1), G["base"])

    at.put(paint(span * K, K, band, cutout=True), x0, by)

    def tag(c):  # the tag below the band on the chest (mane front face, 8 x 6)
        cx = c.w / 2
        c.fill(capsule((cx, 0), (cx, 4), 2.0), G["dark"])
        c.raised(D(9, cx, 5.2), G["base"], G["light"], G["dark"], rim=1.2)

    tag_img = paint(fw * K, 2 * K, tag, cutout=True)
    ty = int((by + 1) * K)
    region = at.a[ty : ty + tag_img.shape[0], fx * K : (fx + fw) * K]
    on = tag_img[..., 3] > 0
    region[on] = tag_img[on]
    return at.a


# ---------------------------------------------------------------------------- particles


def drop(tip_y, cy, r, cx=64.0, lean=0.0, sharp=1.5):
    """A teardrop: round bottom (centre cy, radius r), pointed tip at `tip_y`, the tip leaning
    by `lean` px."""
    def f(y, x):
        t = np.clip((y - tip_y) / (cy - tip_y), 0, 1)
        mid = cx + lean * (1 - t) ** 2
        hw_top = r * t ** sharp * np.sqrt(np.clip(2 - t, 0, None))
        hw_bot = r * np.sqrt(np.clip(1 - ((y - cy) / r) ** 2, 0, None))
        hw = np.where(y < cy, np.minimum(hw_top, r), hw_bot)
        return (np.abs(x - mid) <= hw) & (y >= tip_y)
    return f


def flame(seed: int) -> np.ndarray:
    """A clean flat flame: a red tip, an orange body and a pale yellow core low inside it."""
    def fn(c):
        c.fill(drop(6, 84, 38, lean=8, sharp=1.25), C("#e5452c"))
        c.fill(drop(26, 86, 36, lean=6, sharp=1.2), C("#ff8a2a"))
        c.fill(drop(40, 90, 29, lean=4, sharp=1.15), C("#ffbe55"))
        c.fill(drop(58, 96, 19, lean=2, sharp=1.1), C("#ffe9a3"))
    return paint(128, 128, fn, cutout=True)


# The puffs' size per stage (radius in px, 128 px sprite).
SMOKE_R = [6, 13, 20, 27, 34, 41, 49, 58]
# A puff's lobes: (dy, dx, r) as fractions of its size, back to front.
LOBES = [(-0.32, -0.12, 0.5), (-0.2, 0.34, 0.42), (0.22, -0.46, 0.46), (0.26, 0.46, 0.44),
         (0.12, 0.0, 0.62)]
SMOKE = (C("#c7c7c7"), C("#e2e2e2"), C("#f6f6f6"))


def smoke(i: int):
    """Flat light grey puffs, small to large: a cluster of round lobes, each lit from the top
    left (one disk for the smallest)."""
    def painter(seed: int) -> np.ndarray:
        R = SMOKE_R[i]

        def fn(c):
            cy, cx = 66.0, 64.0
            if i < 2:
                c.raised(D(cy, cx, R), SMOKE[1], SMOKE[2], SMOKE[0], rim=max(1.0, R * 0.16))
                return
            for dy, dx, rr in LOBES:
                c.raised(D(cy + dy * R, cx + dx * R, rr * R), SMOKE[1], SMOKE[2], SMOKE[0],
                         rim=max(1.5, R * 0.1))
        return paint(128, 128, fn, cutout=True)

    return painter


TEXTURES = {
    "entity/chest/normal": chest_single,
    "entity/chest/normal_left": lambda s: chest_double(s, True),
    "entity/chest/normal_right": lambda s: chest_double(s, False),
    "entity/pig/pig_temperate": pig,
    "entity/sheep/sheep": sheep,
    "entity/sheep/sheep_wool": sheep_wool,
    "entity/wolf/wolf": wolf("wild"),
    "entity/wolf/wolf_tame": wolf("tame"),
    "entity/wolf/wolf_angry": wolf("angry"),
    "entity/wolf/wolf_collar": wolf_collar,
    "particle/flame": flame,
    **{f"particle/generic_{i}": smoke(i) for i in range(8)},
}


# ---------------------------------------------------------------------------- debug previews


def _previews():
    """Full-size atlases with their UV faces boxed (out/uv_*.png) and the chest faces put
    together like the game does (out/game_chest*.png): python tools/texgen/entities.py"""
    import zlib
    from pathlib import Path

    from PIL import Image, ImageDraw

    out = Path(__file__).parent / "out"
    out.mkdir(exist_ok=True)

    def show(img, scale):
        h, w = img.shape[:2]
        bg = np.where((np.indices((h, w)).sum(0) // 4 % 2)[..., None] == 0, 60, 90).repeat(3, 2)
        al = img[..., 3:4] / 255.0
        rgb = (img[..., :3] * al + bg * (1 - al)).astype(np.uint8)
        return Image.fromarray(rgb, "RGB").resize((w * scale, h * scale), Image.NEAREST)

    boxes = {
        "entity/chest/normal": [box_uv(0, 0, 2, 4, 1), box_uv(0, 0, 14, 5, 14), box_uv(0, 19, 14, 10, 14)],
        "entity/chest/normal_left": [box_uv(0, 0, 1, 4, 1), box_uv(0, 0, 15, 5, 14), box_uv(0, 19, 15, 10, 14)],
        "entity/pig/pig_temperate": [box_uv(0, 0, 8, 8, 8), box_uv(16, 16, 4, 3, 1), box_uv(28, 8, 10, 16, 8),
                                     box_uv(0, 16, 4, 6, 4)],
        "entity/sheep/sheep": [box_uv(0, 0, 6, 6, 8), box_uv(28, 8, 8, 16, 6), box_uv(0, 16, 4, 12, 4)],
        "entity/sheep/sheep_wool": [box_uv(0, 0, 6, 6, 6), box_uv(28, 8, 8, 16, 6), box_uv(0, 16, 4, 6, 4)],
        **{f"entity/wolf/{n}": list(WOLF_BOXES.values()) for n in ("wolf", "wolf_angry", "wolf_tame",
                                                                    "wolf_collar")},
    }
    for path, bl in boxes.items():
        img = TEXTURES[path](zlib.crc32(path.encode()) & 0x7FFFFFFF)
        w = img.shape[1]
        im = show(img, 2)
        d = ImageDraw.Draw(im)
        u = w * 2 / 64
        for bx in bl:
            for name, (x, y, fw, fh) in bx.items():
                d.rectangle([x * u, y * u, (x + fw) * u - 1, (y + fh) * u - 1], outline=(255, 0, 255))
        im.save(out / f"uv_{path.split('/')[-1]}.png")

    def region(img, x, y, w, h):
        k = img.shape[1] // 64
        return img[y * k : (y + h) * k, x * k : (x + w) * k]

    def face(img, lid_x, wdt=14):
        k = img.shape[1] // 64
        f = np.zeros((16 * k, wdt * k, 4), np.float32)
        lid = region(img, lid_x, 14, wdt, 5)[::-1]
        lid = np.array(Image.fromarray(lid.astype(np.uint8), "RGBA").resize((wdt * k, 4 * k), Image.NEAREST),
                       np.float32)
        f[2 * k : 6 * k] = lid
        f[6 * k : 16 * k] = region(img, lid_x, 33, wdt, 10)[::-1]
        return f

    c = TEXTURES["entity/chest/normal"](1)
    le = TEXTURES["entity/chest/normal_left"](2)
    ri = TEXTURES["entity/chest/normal_right"](3)
    gap = np.zeros((128, 16, 4), np.float32)
    row = np.concatenate([face(c, 42), gap, face(c, 0), gap, np.pad(region(c, 28, 0, 14, 14), ((0, 16), (0, 0), (0, 0))), gap,
                          np.pad(region(c, 1, 1, 2, 4), ((0, 96), (0, 0), (0, 0)))], 1)
    show(row, 2).save(out / "game_chest.png")
    def seam_right(img):  # the game's test: the seam column looks like the wood 2 units in
        diff = lambda a, b: np.abs(img[:, a, :3] - img[:, b, :3]).sum()  # noqa: E731
        return diff(-1, -17) < diff(0, 16)

    def oriented(img, want_right):
        return img if seam_right(img) == want_right else img[:, ::-1]

    pad = ((0, 16), (0, 0), (0, 0))
    dbl = np.concatenate([oriented(face(le, 43, 15), True), oriented(face(ri, 43, 15), False), gap,
                          oriented(face(le, 14, 15), True), oriented(face(ri, 14, 15), False), gap,
                          np.pad(oriented(region(le, 29, 0, 15, 14), True), pad),
                          np.pad(oriented(region(ri, 29, 0, 15, 14), False), pad)], 1)
    show(dbl, 2).save(out / "game_double_chest.png")

    # the mobs' faces as seen from the front: head front with the snout (and ears) on it
    def front(path, head, snout=None, s_at=None, ears=None):
        img = TEXTURES[path](zlib.crc32(path.encode()) & 0x7FFFFFFF)
        x, y, w, h = head
        top = 16 if ears else 0
        f = np.zeros((h * K + top, w * K, 4), np.float32)
        f[top:] = img[y * K : (y + h) * K, x * K : (x + w) * K]
        if snout:
            sx, sy, sw, sh = snout
            f[top + s_at[1] : top + s_at[1] + sh * K, s_at[0] : s_at[0] + sw * K] =                 img[sy * K : (sy + sh) * K, sx * K : (sx + sw) * K]
        if ears:
            ex, ey, ew, eh = ears
            e = img[ey * K : (ey + eh) * K, ex * K : (ex + ew) * K]
            f[:16, 0:16] = e
            f[:16, w * K - 16 :] = e
        return f

    faces = [front("entity/pig/pig_temperate", box_uv(0, 0, 8, 8, 8)["front"], box_uv(16, 16, 4, 3, 1)["front"],
                   (16, 32)),
             front("entity/sheep/sheep", box_uv(0, 0, 6, 6, 8)["front"])]
    for n in ("wolf", "wolf_tame", "wolf_angry"):
        faces.append(front(f"entity/wolf/{n}", WOLF_BOXES["head"]["front"], WOLF_BOXES["snout"]["front"], (12, 24),
                           WOLF_BOXES["ear"]["front"]))
    hmax = max(f.shape[0] for f in faces)
    row = np.concatenate([np.pad(f, ((hmax - f.shape[0], 0), (8, 8), (0, 0))) for f in faces], 1)
    show(row, 3).save(out / "game_faces.png")


if __name__ == "__main__":
    _previews()
