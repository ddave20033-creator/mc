"""The game's own player skin (`entity/player/wide/rustcraft`): an original character, the
RustCraft explorer - copper-red hair, brass goggles pushed up on the forehead, a green
neckerchief, a rust-brown leather vest over a linen shirt with rolled sleeves, leather bracers,
a belt with a brass buckle, olive trousers and brown boots. Drawn in the pack's flat style
(`flat.py`): flat areas in three tones per material, clean anti-aliased edges, light from the
top left, seams as clean lines.

Minecraft's 64x64 skin layout (wide arms) at 8 px per unit (512 x 512). The base layer holds
the body and the clothes worn close (fully opaque); the outer layer (hat, jacket, sleeves: 32
units right of the head's faces, 16 below the others') holds what sits on top - the goggles and
their strap, the neckerchief, the vest, the belt and the bracers - with all-or-nothing alpha.
The game draws the outer layer over the base (`src/textures/from_pack.rs`); it uses the front
of each arm and leg on all their sides, the body's right side on both sides and the top, and
the head's bottom for the hands' ends, so those faces are drawn to work there too.

    python tools/texgen/player.py   # tools/texgen/out/player_check.png: the views put together
"""

from __future__ import annotations

import numpy as np

from flat import Canvas, box, capsule, disk, ellipse, hexc, minus, moved_shape, union

K = 8  # px per unit


def T(*hexes):
    """A material's flat tones: [dark, mid, light]."""
    return [hexc(h) for h in hexes]


SKIN = T("#c98563", "#e3a682", "#f2c3a0")
FRECKLE = hexc("#c47b58")
HAIR = T("#86351b", "#b3532a", "#d97a3f")
LINEN = T("#c2b28f", "#e2d6b5", "#f4eedb")
VEST = T("#6b3320", "#91492b", "#b5653a")
SCARF = T("#2c5c3f", "#3e7f55", "#5fa270")
STRAP = T("#36241a", "#4f3726", "#6c4c34")
BRASS = T("#a06a2a", "#d29b46", "#f3cd7c")
LENS = T("#2b6466", "#4a948f", "#a6dccf")
CANVAS = T("#4b5233", "#646d44", "#7f8a56")
BOOT = T("#3c2518", "#5a3a25", "#7b5335")
SOLE = hexc("#2a1c14")
EYE_WHITE = hexc("#f7f4ec")
IRIS = T("#22432a", "#3f7a47", "#6aa765")
GLINT = hexc("#ffffff")
MOUTH = hexc("#9b4636")


# ---------------------------------------------------------------------------- helpers


def cv(w, h, bg=None) -> Canvas:
    """A face of `w` x `h` px (not tiling)."""
    return Canvas(w, h, tile=False, bg=bg)


def poly(points):
    """A filled polygon of (x, y) points that, unlike `flat.poly`, honours the coordinates it
    is asked about (so it can be moved: rims, cast shadows)."""
    pts = [(float(x), float(y)) for x, y in points]

    def f(y, x):
        inside = np.zeros(y.shape, bool)
        for (x1, y1), (x2, y2) in zip(pts, pts[1:] + pts[:1]):
            if y1 == y2:
                continue
            cross = (y1 > y) != (y2 > y)
            xi = (x2 - x1) * (y - y1) / (y2 - y1) + x1
            inside ^= cross & (x < xi)
        return inside

    return f


def rect(y0, x0, y1, x1, r=0.0):
    return box(y0, x0, y1, x1, r=r)


def circ(cy, cx, r):
    return disk(cy, cx, r, tile=False)


def oval(cy, cx, ry, rx, angle=0.0):
    return ellipse(cy, cx, ry, rx, angle, tile=False)


def line(p0, p1, w=1.4):
    return capsule(p0, p1, w)


def raised(c: Canvas, shape, t, rim=1.6):
    c.raised(shape, t[1], t[2], t[0], rim)


def shadow(c: Canvas, caster, color, dy=2.5, dx=1.5, on=None):
    """The shadow `caster` throws down and right (light from the top left), on `on` only."""
    m = c.mask(moved_shape(caster, dy, dx)) & ~c.mask(caster)
    if on is not None:
        m &= c.mask(on)
    c.fill(m, color)


def mirror(img):
    return img[:, ::-1].copy()


def opaque(c: Canvas):
    return c.finish(opaque=True)


def cutout(c: Canvas):
    return c.finish(cutout=True)


# ---------------------------------------------------------------------------- head

# The fringe: three big locks swept to the right (tips down-right), temples at the sides.
FRINGE = poly([(-4, -4), (68, -4), (68, 34), (61, 34), (58, 17), (55, 27), (41, 16),
               (36, 27), (23, 16), (17, 26), (7, 17), (4, 34), (-4, 34)])


def head_front(seed):
    c = cv(64, 64, SKIN[1])
    c.fill(rect(59, -1, 65, 65), SKIN[0])  # the jaw's shade
    # Hair and the shadow it throws on the forehead.
    shadow(c, FRINGE, SKIN[0], 3.0, 2.0)
    raised(c, FRINGE, HAIR, 2.0)
    for p in ([(49, 14), (55, 24), (52, 14)], [(30, 14), (36, 24), (33, 14)],
              [(12, 14), (17, 23), (15, 14)]):
        c.fill(poly(p), HAIR[2])
    # Brows.
    c.fill(line((11.5, 28.5), (23, 27), 3.0), HAIR[0])
    c.fill(line((41, 27), (52.5, 28.5), 3.0), HAIR[0])
    # Eyes: white, a round green iris with a darker top, a white glint, the upper lid.
    for x0 in (11, 41):
        eye = rect(32, x0, 41, x0 + 12, r=3.0)
        c.fill(eye, EYE_WHITE)
        cx = x0 + 6
        iris = lambda y, x, cx=cx, eye=eye: circ(36.8, cx, 4.3)(y, x) & eye(y, x)
        c.fill(iris, IRIS[1])
        c.fill(lambda y, x, iris=iris: iris(y, x) & (y < 34.6), IRIS[0])
        c.fill(circ(37.2, cx, 1.9), IRIS[0])
        c.fill(circ(35.4, cx - 1.6, 1.35), GLINT)
        c.fill(line((x0 + 0.5, 32.2), (x0 + 11.5, 32.2), 2.2), HAIR[0])
    # Nose: a shade on its right and under it.
    c.fill(poly([(33, 38), (35.5, 38), (36.5, 45.5), (29.5, 45.5), (29.5, 44), (33.5, 43.5)]),
           SKIN[0])
    # Freckles.
    for fy, fx in ((44, 15), (46.5, 19), (43.5, 22.5), (44, 49), (46.5, 45), (43.5, 41.5)):
        c.fill(circ(fy, fx, 1.3), FRECKLE)
    # A small smile.
    c.fill(line((27.5, 52.5), (36.5, 52.5), 2.8), MOUTH)
    c.fill(line((25.5, 50.8), (27.5, 52.5), 2.2), MOUTH)
    c.fill(line((36.5, 52.5), (38.5, 50.8), 2.2), MOUTH)
    return opaque(c)


SIDE_HAIR = poly([(-4, -4), (68, -4), (68, 25), (61, 19), (57, 26), (51, 19), (46, 23),
                  (45.5, 38), (40, 34), (39, 21), (32, 19), (27, 25), (26, 42), (23, 50),
                  (20, 47), (16, 58), (12, 51), (7, 59), (3, 53), (-4, 57)])


def head_side(seed):
    """The right side: the back of the head on the left, the face on the right."""
    c = cv(64, 64, SKIN[1])
    c.fill(rect(59, 26, 65, 65), SKIN[0])
    shadow(c, SIDE_HAIR, SKIN[0], 3.0, 2.0)
    raised(c, SIDE_HAIR, HAIR, 2.0)
    for p in ([(48, 4), (55, 4), (57, 20), (53, 16)], [(28, 4), (36, 4), (38, 16), (33, 14)],
              [(4, 8), (13, 8), (15, 36), (10, 46)]):
        c.fill(poly(p), HAIR[2])
    for x, y in ((51, 19), (32, 19), (12, 51), (21, 47)):
        c.fill(line((x, y), (x - 2, y - 12), 1.6), HAIR[0])
    # The ear.
    ear = rect(28, 29, 46, 38, r=4.5)
    shadow(c, ear, SKIN[0], 2.0, 2.0)
    raised(c, ear, SKIN, 1.6)
    c.fill(rect(32, 32, 42, 35, r=1.5), SKIN[0])
    return opaque(c)


BACK_HAIR = poly([(-4, -4), (68, -4), (68, 54), (60, 59), (53, 52), (45, 59), (38, 52),
                  (31, 60), (24, 52), (17, 59), (10, 52), (4, 58), (-4, 54)])


def head_back(seed):
    c = cv(64, 64, SKIN[1])
    shadow(c, BACK_HAIR, SKIN[0], 2.5, 0.0)
    raised(c, BACK_HAIR, HAIR, 2.0)
    # Big locks: a dark parting line up from each notch, a light sheen on the upper left ones.
    for x, y in ((53, 52), (38, 52), (24, 52), (10, 52)):
        c.fill(line((x, y), (x - 3, 30), 1.6), HAIR[0])
    for p in ([(4, 8), (13, 8), (16, 34), (12, 46)], [(19, 8), (27, 8), (30, 32), (26, 42)],
              [(34, 8), (40, 8), (42, 26), (39, 33)]):
        c.fill(poly(p), HAIR[2])
    return opaque(c)


def head_top(seed):
    """Long locks from the crown at the back (the top edge) towards the face (the bottom
    edge), lit on the left."""
    c = cv(64, 64, HAIR[1])
    for p in ([(2, -2), (14, -2), (16, 40), (6, 66)], [(22, -2), (31, -2), (33, 36), (27, 58)],
              [(40, -2), (46, -2), (48, 30), (44, 46)]):
        c.fill(poly(p), HAIR[2])
    for x0, x1 in ((19, 21), (37, 39.5), (55, 57)):
        c.fill(line((x0, -2), (x1, 66), 1.6), HAIR[0])
    return opaque(c)


def head_bottom(seed):
    """The chin's underside - and the ends of the hands (the game uses it there)."""
    return opaque(cv(64, 64, SKIN[0]))


# -- the hat layer: the goggles and their strap

STRAP_BAND = rect(8, -4, 17, 68)


def strap(c: Canvas):
    raised(c, STRAP_BAND, STRAP, 1.4)
    c.fill(line((-2, 14.6), (66, 14.6), 1.0), STRAP[0])


def goggles_front(seed):
    c = cv(64, 64)
    strap(c)
    c.fill(rect(9.5, 28, 15.5, 36, r=1.5), BRASS[0])  # the bridge
    c.fill(rect(9.5, 28, 11.5, 36, r=1.0), BRASS[1])
    for cx in (18, 46):
        frame = oval(12.5, cx, 7.8, 10.0)
        raised(c, frame, BRASS, 1.6)
        lens = oval(12.8, cx, 5.2, 7.2)
        c.fill(lens, LENS[1])
        c.fill(lambda y, x, lens=lens, cx=cx: lens(y, x) & (x - cx + (y - 12.8) * 0.9 > 2.5),
               LENS[0])
        c.fill(line((cx - 4.5, 12.8), (cx - 2.0, 9.6), 2.0), LENS[2])
        for rx in (cx - 8.7, cx + 8.7):
            c.fill(circ(12.5, rx, 1.1), BRASS[2] if rx < cx else BRASS[0])
    return cutout(c)


def strap_around(seed, buckle=False):
    """The hat layer round the side and the back: the goggles' strap (and its buckle)."""
    c = cv(64, 64)
    strap(c)
    if buckle:
        frame = rect(6, 26, 19, 38, r=2.0)
        raised(c, frame, BRASS, 1.5)
        c.fill(rect(9, 29, 16, 35, r=1.0), STRAP[0])
        c.fill(line((29.5, 12.5), (33.5, 12.5), 1.8), BRASS[2])
    return cutout(c)


# ---------------------------------------------------------------------------- body


def vest_panels(w=64):
    """The vest's two front panels (open in the middle)."""
    left = poly([(-4, -4), (19, -4), (21, 10), (22.5, 78), (-4, 78)])
    right = poly([(w - x, y) for x, y in [(-4, -4), (19, -4), (21, 10), (22.5, 78), (-4, 78)]])
    return left, right


SCARF_BAND = rect(-4, 12, 7, 52)
SCARF_TIP = poly([(21.5, 4), (43.5, 4), (33.5, 27), (31.5, 27)])
BELT = rect(76, -4, 84, 68)


def trousers_top(c: Canvas, w):
    c.fill(rect(84, -1, 97, w + 1), CANVAS[1])
    c.fill(rect(84, -1, 87, w + 1), CANVAS[0])  # the belt's shadow


def body_base(seed, name):
    w = 64 if name in ("front", "back") else 32
    c = cv(w, 96, LINEN[1])
    trousers_top(c, w)
    if name == "front":
        # The open collar, the placket and its buttons, the shadows of what hangs over it.
        c.fill(poly([(23, -1), (41, -1), (32, 13)]), SKIN[1])
        c.fill(poly([(20, -1), (25, -1), (32, 13), (27, 12)]), LINEN[2])
        c.fill(poly([(44, -1), (39, -1), (32, 13), (37, 12)]), LINEN[0])
        c.fill(line((32.5, 14), (32.5, 82), 1.4), LINEN[0])
        for by in (30, 44, 58, 71):
            c.fill(circ(by, 35, 1.6), LINEN[0])
        left, right = vest_panels()
        shadow(c, union(left, right, SCARF_BAND, SCARF_TIP), LINEN[0], 2.5, 2.0)
        c.fill(line((32, 87), (32, 97), 1.4), CANVAS[0])
    elif name == "back":
        c.fill(line((32, 87), (32, 97), 1.4), CANVAS[0])
    return opaque(c)


def belt(c: Canvas, w, loops=()):
    raised(c, BELT, STRAP, 1.5)
    for lx in loops:
        lp = rect(74.5, lx, 85.5, lx + 4, r=1.0)
        raised(c, lp, STRAP, 1.2)


def body_over(seed, name):
    """The outer layer: the vest, the neckerchief and the belt."""
    if name == "front":
        c = cv(64, 96)
        left, right = vest_panels()
        for panel in (left, right):
            raised(c, panel, VEST, 2.0)
        # Seams along the opening.
        c.fill(line((16.5, 9), (18.2, 74), 1.2), VEST[0])
        c.fill(line((47.5, 9), (45.8, 74), 1.2), VEST[0])
        # A pocket with a flap on the left panel, brass buttons on the right one.
        c.fill(rect(47, 4, 63, 15.5, r=1.5), VEST[0])
        c.fill(rect(47, 5, 62, 14.5, r=1.2), VEST[1])
        c.fill(rect(47, 4, 52.5, 15.5, r=1.5), VEST[2])
        c.fill(rect(52.5, 4.5, 53.8, 15), VEST[0])
        raised(c, circ(50, 9.8, 1.8), BRASS, 0.9)
        for by in (34, 54):
            b = circ(by, 49, 2.4)
            shadow(c, b, VEST[0], 1.2, 1.0)
            raised(c, b, BRASS, 1.0)
        # The neckerchief: its shadow, the band, the tip with a fold, the knot.
        sc = union(SCARF_BAND, SCARF_TIP)
        shadow(c, sc, VEST[0], 2.5, 2.0, on=union(left, right))
        raised(c, sc, SCARF, 1.6)
        c.fill(line((32.5, 9), (32.5, 24), 1.4), SCARF[0])
        knot = rect(2, 27.5, 11, 37.5, r=3.0)
        shadow(c, knot, SCARF[0], 1.5, 1.2)
        raised(c, knot, SCARF, 1.6)
        # The belt and its buckle.
        belt(c, 64)
        frame = rect(73.5, 25.5, 86.5, 38.5, r=2.0)
        shadow(c, frame, STRAP[0], 1.5, 1.2, on=BELT)
        raised(c, frame, BRASS, 1.6)
        c.fill(rect(76.8, 28.8, 83.2, 35.2, r=1.0), STRAP[0])
        c.fill(line((28.5, 80), (33.5, 80), 1.8), BRASS[2])
        return cutout(c)
    if name == "back":
        c = cv(64, 96)
        vest = rect(-4, -4, 78, 68)
        raised(c, vest, VEST, 2.0)
        c.fill(line((-2, 13), (66, 13), 1.2), VEST[0])  # the yoke
        c.fill(line((32, 13), (32, 76), 1.2), VEST[0])
        cinch = rect(62, 16, 66.5, 48)
        shadow(c, cinch, VEST[0], 1.5, 1.0)
        raised(c, cinch, STRAP, 1.2)
        bk = rect(60.5, 29, 68, 35, r=1.0)
        raised(c, bk, BRASS, 1.2)
        band = rect(-4, 12, 5, 52)
        shadow(c, band, VEST[0], 2.0, 1.5)
        raised(c, band, SCARF, 1.4)
        belt(c, 64, loops=(12, 48))
        return cutout(c)
    # The right side (the game uses it for both sides and the top).
    c = cv(32, 96)
    raised(c, rect(-4, -4, 78, 36), VEST, 2.0)
    c.fill(line((16, -2), (16, 76), 1.2), VEST[0])
    belt(c, 32, loops=(14,))
    return cutout(c)


def body_top_bottom(seed):
    """The shoulders (the neck in the collar, the vest and the neckerchief round it) and the
    hips' underside."""
    top = cv(64, 32, LINEN[1])
    neck = oval(16, 32, 8, 10)
    top.fill(neck, SKIN[1])
    over = cv(64, 32)
    raised(over, union(rect(-4, -4, 36, 19), rect(-4, 45, 36, 68)), VEST, 2.0)
    ring = minus(oval(16, 32, 12.5, 15.5), neck)
    raised(over, ring, SCARF, 1.6)
    bottom = cv(64, 32, CANVAS[0])
    return opaque(top), cutout(over), opaque(bottom)


# ---------------------------------------------------------------------------- arms and legs


def arm_base(seed):
    """One side of an arm (all four look alike: the game uses the front for each): the linen
    sleeve rolled up to the elbow, the forearm, the hand at the end."""
    c = cv(32, 96, LINEN[1])
    c.fill(line((5, 6), (12, 20), 1.4), LINEN[0])  # a fold of the sleeve
    c.fill(line((22, 10), (26, 26), 1.4), LINEN[0])
    c.fill(rect(44, -1, 97, 33), SKIN[1])
    cuff = rect(33, -4, 44, 36)
    c.fill(rect(44, -1, 47, 33), SKIN[0])  # the cuff's shadow
    raised(c, cuff, LINEN, 1.8)
    c.fill(line((-2, 38.5), (34, 38.5), 1.2), LINEN[0])
    # The hand: the wrist's crease and the gaps between the fingers.
    c.fill(line((6, 86), (26, 86), 1.3), SKIN[0])
    for fx in (9.5, 16, 22.5):
        c.fill(line((fx, 90), (fx, 97), 1.3), SKIN[0])
    return opaque(c)


def arm_over(seed):
    """The outer layer of an arm: a leather bracer on the forearm with two straps and their
    brass buckles."""
    c = cv(32, 96)
    bracer = rect(58, -4, 82, 36)
    raised(c, bracer, VEST, 2.0)
    for sy in (63, 73):
        s = rect(sy, -4, sy + 4.5, 36)
        shadow(c, s, VEST[0], 1.5, 0.0)
        raised(c, s, STRAP, 1.1)
        bk = rect(sy - 1, 19, sy + 5.5, 25, r=1.0)
        raised(c, bk, BRASS, 1.1)
        c.fill(rect(sy + 1, 21, sy + 3.5, 23), STRAP[0])
    return cutout(c)


def arm_top(seed):
    return opaque(cv(32, 32, LINEN[2]))


def leg_base(seed):
    """One side of a leg (all alike, see `arm_base`): olive trousers with a knee patch, tucked
    into boots with a turned-down cuff and a strap."""
    c = cv(32, 96, CANVAS[1])
    c.fill(line((16, -2), (16, 34), 1.4), CANVAS[0])
    patch = rect(38, 7, 52, 25, r=2.5)
    shadow(c, patch, CANVAS[0], 1.5, 1.2)
    c.fill(patch, CANVAS[2])
    c.fill(minus(rect(40, 9, 50, 23, r=1.5), rect(41.2, 10.2, 48.8, 21.8, r=0.8)), CANVAS[0])
    c.fill(rect(58, -1, 62, 33), CANVAS[0])  # bunched over the boots
    # The boot: the shaft, the cuff, the strap with a buckle, the sole.
    c.fill(rect(62, -1, 97, 33), BOOT[1])
    c.fill(rect(72, -1, 74.5, 33), BOOT[0])
    raised(c, rect(61, -4, 72, 36), BOOT, 1.6)
    s = rect(78, -4, 82.5, 36)
    raised(c, s, STRAP, 1.1)
    raised(c, rect(77, 19, 83.5, 25, r=1.0), BRASS, 1.0)
    c.fill(rect(79, 21, 81.5, 23), STRAP[0])
    c.fill(rect(89, -1, 97, 33), SOLE)
    c.fill(rect(89, -1, 90.4, 33), BOOT[0])
    return opaque(c)


def leg_ends(seed):
    top = cv(32, 32, CANVAS[1])
    sole = cv(32, 32, SOLE)
    for y in (8, 16, 24):
        sole.fill(line((4, y), (28, y), 1.4), BOOT[0])
    return opaque(top), opaque(sole)


# ---------------------------------------------------------------------------- the atlas


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


def put(atlas, img, x, y):
    h, w = img.shape[:2]
    atlas[y * K : y * K + h, x * K : x * K + w] = img


def rustcraft(seed: int) -> np.ndarray:
    a = np.zeros((64 * K, 64 * K, 4), np.float32)
    # Head (0, 0) and its outer layer (32, 0).
    hd, hat = box_uv(0, 0, 8, 8, 8), box_uv(32, 0, 8, 8, 8)
    side = head_side(seed)
    put(a, head_top(seed), *hd["top"][:2])
    put(a, head_bottom(seed), *hd["bottom"][:2])
    put(a, head_front(seed), *hd["front"][:2])
    put(a, side, *hd["right"][:2])
    put(a, mirror(side), *hd["left"][:2])
    put(a, head_back(seed), *hd["back"][:2])
    around = strap_around(seed)
    put(a, goggles_front(seed), *hat["front"][:2])
    put(a, around, *hat["right"][:2])
    put(a, mirror(around), *hat["left"][:2])
    put(a, strap_around(seed, buckle=True), *hat["back"][:2])
    # Body (16, 16) and the jacket layer (16, 32).
    bd, jk = box_uv(16, 16, 8, 12, 4), box_uv(16, 32, 8, 12, 4)
    for name in ("front", "back", "right", "left"):
        base = body_base(seed, "right" if name == "left" else name)
        over = body_over(seed, "right" if name == "left" else name)
        if name == "left":
            base, over = mirror(base), mirror(over)
        put(a, base, *bd[name][:2])
        put(a, over, *jk[name][:2])
    top, top_over, bottom = body_top_bottom(seed)
    put(a, top, *bd["top"][:2])
    put(a, top_over, *jk["top"][:2])
    put(a, bottom, *bd["bottom"][:2])
    # Arms: the right (40, 16) with its sleeve layer (40, 32), the left (32, 48) with (48, 48).
    arm_img, arm_o, arm_t, hand = arm_base(seed), arm_over(seed), arm_top(seed), head_bottom(seed)
    for (u, v), (ou, ov), flip in (((40, 16), (40, 32), False), ((32, 48), (48, 48), True)):
        arm, sleeve = box_uv(u, v, 4, 12, 4), box_uv(ou, ov, 4, 12, 4)
        for name in ("front", "right", "back", "left"):
            put(a, mirror(arm_img) if flip else arm_img, *arm[name][:2])
            put(a, mirror(arm_o) if flip else arm_o, *sleeve[name][:2])
        put(a, arm_t, *arm["top"][:2])
        put(a, hand[: 4 * K, : 4 * K], *arm["bottom"][:2])
    # Legs: the right (0, 16), the left (16, 48); their outer layers stay empty.
    leg_img = leg_base(seed)
    top, sole = leg_ends(seed)
    for (u, v), flip in (((0, 16), False), ((16, 48), True)):
        leg = box_uv(u, v, 4, 12, 4)
        for name in ("front", "right", "back", "left"):
            put(a, mirror(leg_img) if flip else leg_img, *leg[name][:2])
        put(a, top, *leg["top"][:2])
        put(a, sole, *leg["bottom"][:2])
    return a


TEXTURES = {
    "entity/player/wide/rustcraft": rustcraft,
}


# ---------------------------------------------------------------------------- preview


def _preview():
    """tools/texgen/out/player_check.png: the faces put together the way the game shows them
    (outer layer over the base): front, back and right side, the head's top, the body's side
    and top, at 2x."""
    import zlib
    from pathlib import Path

    from PIL import Image

    out = Path(__file__).parent / "out"
    out.mkdir(exist_ok=True)
    path = "entity/player/wide/rustcraft"
    a = rustcraft(zlib.crc32(path.encode()) & 0x7FFFFFFF)

    def comp(x, y, w, h, ox, oy):
        b = a[y * K : (y + h) * K, x * K : (x + w) * K].copy()
        o = a[(y + oy) * K : (y + oy + h) * K, (x + ox) * K : (x + ox + w) * K]
        al = o[..., 3:4] / 255.0
        b[..., :3] = o[..., :3] * al + b[..., :3] * (1 - al)
        return b

    def figure(parts):
        fig = np.zeros((32 * K, 16 * K, 4), np.float32)
        for img, x, y in parts:
            hh, ww = img.shape[:2]
            fig[y * K : y * K + hh, x * K : x * K + ww] = img
        return fig

    arm = comp(44, 20, 4, 12, 0, 16)
    leg = comp(4, 20, 4, 12, 0, 16)
    front = figure([(comp(8, 8, 8, 8, 32, 0), 4, 0), (comp(20, 20, 8, 12, 0, 16), 4, 8),
                    (arm, 0, 8), (arm[:, ::-1], 12, 8), (leg, 4, 20), (leg[:, ::-1], 8, 20)])
    back = figure([(comp(24, 8, 8, 8, 32, 0), 4, 0), (comp(32, 20, 8, 12, 0, 16), 4, 8),
                   (arm, 0, 8), (arm[:, ::-1], 12, 8), (leg, 4, 20), (leg[:, ::-1], 8, 20)])
    side = figure([(comp(0, 8, 8, 8, 32, 0), 4, 0), (comp(16, 20, 4, 12, 0, 16), 4, 8),
                   (arm, 8, 8), (leg, 6, 20)])
    extras = figure([(comp(8, 0, 8, 8, 32, 0), 4, 0), (comp(16, 20, 4, 12, 0, 16), 6, 9),
                     (comp(20, 16, 8, 4, 0, 16), 4, 22), (comp(44, 16, 4, 4, 0, 16), 6, 27)])

    def show(img, scale):
        h, w = img.shape[:2]
        bg = np.where((np.indices((h, w)).sum(0) // 8 % 2)[..., None] == 0, 70, 95).repeat(3, 2)
        al = img[..., 3:4] / 255.0
        rgb = np.clip(img[..., :3] * al + bg * (1 - al), 0, 255).astype(np.uint8)
        return Image.fromarray(rgb, "RGB").resize((w * scale, h * scale), Image.LANCZOS)

    figs = [show(f, 2) for f in (front, back, side, extras)]
    heads = [show(comp(x, 8, 8, 8, 32, 0), 4) for x in (8, 24, 0)]
    heads.append(show(comp(8, 0, 8, 8, 32, 0), 4))
    sheet = Image.new("RGB", (4 * 266 + 10, 512 + 266), (30, 30, 34))
    for i, f in enumerate(figs):
        sheet.paste(f, (10 + i * 266, 0))
    for i, f in enumerate(heads):
        sheet.paste(f, (10 + i * 266, 522))
    sheet.save(out / "player_check.png")
    show(a, 1).save(out / "player.png")
    print(out / "player_check.png")


if __name__ == "__main__":
    _preview()
