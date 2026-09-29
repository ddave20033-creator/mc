"""Generates gun_station.bbmodel (+ gun_station.png): the gun station block for Blockbench, or
with --rifle rifle_station.bbmodel (+ rifle_station.png): the big one, for the rifles.

Run:  python tools/blockbench/gen_gun_station.py [--rifle]
then: python tools/blockbench/bbmodel_to_rust.py tools/blockbench/gun_station.bbmodel
      (or rifle_station.bbmodel)

A gunsmith's bench two blocks wide (the rifle station three): a steel cabinet under a
butcher-block top with a rubber cleaning mat on it, a drawer of cleaning tools and boxes of
rounds that slides out while the station is used (the "open" animation), and ammo cans and
toolboxes on the shelf below (the rifle station a long rifle case too).

Model space: 1 unit = 1 Blockbench pixel, the left block from (-8, 0, -8) to (8, 16, 8), the
others to its right (+X), its front (the drawer, toward the player using it) facing +Z
(south). The worktop's top is at y = 16 exactly: the game lays guns and parts on it. Every
face gets its own painted spot on one texture atlas (D texels per unit).
"""
import base64, json, math, os, random, struct, sys, uuid, zlib

RIFLE = "--rifle" in sys.argv
NAME = "rifle_station" if RIFLE else "gun_station"
# How far right of the small station everything on the right side sits (one more block).
W = 16 if RIFLE else 0

random.seed(11)
D = 8                 # texels per model unit
ATLAS_W = 1024
HERE = os.path.dirname(os.path.abspath(__file__))

MATS = {
    "steel": (62, 70, 82), "steel_dark": (40, 45, 53), "inside": (26, 28, 32),
    "drawer": (84, 93, 106), "alu": (176, 180, 186), "wood": (150, 104, 62), "mat": (40, 58, 50),
    "olive": (82, 88, 54), "red": (156, 40, 34), "black": (22, 22, 24), "cardboard": (176, 136, 86),
    "oil": (70, 92, 46), "cap": (190, 40, 30), "bristle": (226, 220, 200), "brass": (190, 150, 62),
    "metal": (130, 134, 140), "rubber": (30, 30, 32),
    "gunmetal": (50, 52, 58), "copper": (178, 104, 66), "lamp_green": (120, 255, 120),
}

if RIFLE:
    # the rifle station's cabinet is painted olive drab
    MATS.update({"steel": (64, 72, 56), "steel_dark": (42, 48, 38), "drawer": (86, 94, 72)})

def clamp(v): return max(0, min(255, int(v)))
def shade(c, d): return tuple(clamp(v + d) for v in c)
def U(v): return int(round(v * D))

def rect(px, x0, y0, x1, y1, col):
    for j in range(max(0, int(y0)), min(len(px), int(y1))):
        for i in range(max(0, int(x0)), min(len(px[0]), int(x1))):
            px[j][i] = col(px[j][i]) if callable(col) else col

def disc(px, cx, cy, r, col):
    for j in range(len(px)):
        for i in range(len(px[0])):
            if math.hypot(i + 0.5 - cx, j + 0.5 - cy) < r:
                px[j][i] = col(px[j][i]) if callable(col) else col

def paint_face(face, mat, w, h, painters, noise=4):
    base = MATS[mat]
    px = [[None] * w for _ in range(h)]
    for j in range(h):
        for i in range(w):
            d = random.randint(-noise, noise)
            if face == "up": d += 8
            elif face == "down": d -= 16
            elif face in ("east", "west"): d -= 6
            # worn, lit edges: light along the top, dark along the bottom
            if j == 0: d += 16
            elif j == h - 1: d -= 14
            if i == 0 or i == w - 1: d -= 6
            px[j][i] = shade(base, d)
    for p in painters:
        p(px, w, h, face)
    return px

# ---------- detail painters (u runs along a face, v down it) ----------
def powder(px, w, h, face):
    """Powder coat: a fine speckle."""
    for j in range(h):
        for i in range(w):
            if random.random() < 0.12:
                px[j][i] = shade(px[j][i], random.choice((-9, 7)))

def rivets(px, w, h, face):
    """Rivets along the panel's edges."""
    if face not in ("east", "west", "south", "north"): return
    for u in range(U(1), w - U(0.5), U(3)):
        for v in (U(0.8), h - U(0.8)):
            disc(px, u + 0.5, v, U(0.22), lambda c: shade(c, 28))
            disc(px, u + 1.0, v + 0.8, U(0.12), lambda c: shade(c, -20))

def butcher_block(px, w, h, face):
    """Glued strips of wood along the top's length, grain across each, a varnish shine."""
    strip = U(1.6)
    for i in range(w):
        k = (i // strip) % 3
        tone = (-10, 6, -2)[k]
        for j in range(h):
            g = int(6 * math.sin(i * 0.9 + j * 0.15 + k * 2))
            px[j][i] = shade(px[j][i], tone + g)
        if i % strip == 0:
            rect(px, i, 0, i + 1, h, lambda c: shade(c, -26))
    rect(px, 0, 0, w, 1, lambda c: shade(c, 30))

# The worktop's outline (x0, z0, x1, z1): the mat's tiles paint their part of one pattern.
TOP = (-8.3, -8.3, 24.3 + W, 8.4)

def mat_tile(x0, z0):
    """A piece of the top from (x0, z0): a rubber gun-cleaning mat on the wood, a thin wood
    margin round it, a grid of squares on it (lined up across all the pieces)."""
    def f(px, w, h, face):
        if face != "up":
            return
        m = 0.7
        wood = MATS["wood"]
        for j in range(h):
            for i in range(w):
                ax, az = x0 + (i + 0.5) / D, z0 + (j + 0.5) / D
                inner = TOP[0] + m < ax < TOP[2] - m and TOP[1] + m < az < TOP[3] - m
                if not inner:
                    strip = int((ax - TOP[0]) / 1.6) % 2
                    px[j][i] = shade(wood, random.randint(-6, 6) + (8 if strip else -4))
                    continue
                c = shade(MATS["mat"], random.randint(-3, 3))
                gx, gz = (ax - TOP[0] - m) % 2.0, (az - TOP[1] - m) % 2.0
                if gx < 1.0 / D or gz < 1.0 / D:
                    c = shade(c, 12)
                # the mat's raised rim: lit at the back, shadowed at the front
                if az < TOP[1] + m + 1.0 / D:
                    c = shade(c, -30)
                elif az > TOP[3] - m - 1.0 / D:
                    c = shade(c, 18)
                px[j][i] = c
    return f

def wood_grain(px, w, h, face):
    for j in range(h):
        g = int(7 * math.sin(j * 0.7 + random.random() * 0.3))
        for i in range(w):
            px[j][i] = shade(px[j][i], g)

def drawer_front(px, w, h, face):
    """Pressed panel with a label holder and a white card in it."""
    if face != "south": return
    b = U(0.5)
    rect(px, b, b, w - b, b + 1, lambda c: shade(c, -22))
    rect(px, b, h - b - 1, w - b, h - b, lambda c: shade(c, 18))
    rect(px, b, b, b + 1, h - b, lambda c: shade(c, -14))
    rect(px, w - b - 1, b, w - b, h - b, lambda c: shade(c, 12))
    cx, top = w // 2, U(2.4)
    rect(px, cx - U(1.4), top, cx + U(1.4), top + U(0.9), (170, 174, 180))
    rect(px, cx - U(1.2), top + 1, cx + U(1.2), top + U(0.9) - 1, (228, 224, 210))
    for k in range(4):
        rect(px, cx - U(1.0) + k * U(0.55), top + 3, cx - U(1.0) + k * U(0.55) + U(0.35), top + 4, (60, 60, 70))

def brushed(px, w, h, face):
    for j in range(h):
        s = random.randint(-8, 8)
        for i in range(w):
            px[j][i] = shade(px[j][i], s // 2)

def stencil(text_rows):
    """Yellow stencilled marks (a few short bars) on a can's front."""
    def f(px, w, h, face):
        if face != "south": return
        for r in range(text_rows):
            y = U(1.0) + r * U(0.9)
            x = U(0.8)
            while x < w - U(1.2):
                n = random.randint(2, 4)
                rect(px, x, y, x + n, y + U(0.4), (206, 184, 72))
                x += n + 2
    return f

def ammo_can_side(px, w, h, face):
    if face in ("east", "west", "south", "north"):
        rect(px, 0, U(0.9), w, U(0.9) + 1, lambda c: shade(c, -26))
        rect(px, 0, U(0.9) + 1, w, U(0.9) + 2, lambda c: shade(c, 14))

def toolbox_side(px, w, h, face):
    if face in ("east", "west", "south", "north"):
        rect(px, 0, U(1.0), w, U(1.0) + 1, lambda c: shade(c, -40))
        rect(px, 0, U(1.0) + 1, w, U(1.0) + 2, lambda c: shade(c, 20))
    if face == "south":
        rect(px, w // 2 - U(0.5), U(0.6), w // 2 + U(0.5), U(1.5), (170, 174, 180))

def label(col):
    def f(px, w, h, face):
        if face in ("south", "east", "west", "north"):
            rect(px, 0, h // 3, w, h * 2 // 3, col)
            rect(px, U(0.2), h // 3 + 2, w - U(0.2), h // 3 + 3, (60, 60, 60))
    return f

def cardboard_box(px, w, h, face):
    if face == "up":
        rect(px, 0, h // 2, w, h // 2 + 1, lambda c: shade(c, -30))
    if face == "south":
        rect(px, U(0.3), U(0.3), w - U(0.3), h - U(0.3), (234, 230, 214))
        rect(px, U(0.5), U(0.6), w - U(0.5), U(0.9), (160, 40, 30))

def dark(*faces):
    def f(px, w, h, face):
        if face in faces:
            rect(px, 0, 0, w, h, lambda c: shade(MATS["inside"], random.randint(-2, 2)))
    return f

# ---------- model building ----------
FACES = ("north", "east", "south", "west", "up", "down")
def face_size(face, s):
    sx, sy, sz = s
    return {"north": (sx, sy), "south": (sx, sy), "east": (sz, sy), "west": (sz, sy),
            "up": (sx, sz), "down": (sx, sz)}[face]

elements, jobs = [], []

def cube(name, frm, to, mat, painters=(), origin=None, rot=(0, 0, 0), only=None, noise=4):
    size = [b - a for a, b in zip(frm, to)]
    assert all(s > 0 for s in size), name
    faces = {}
    for f in FACES:
        if only is not None and f not in only:
            faces[f] = {"uv": [0, 0, 0, 0], "texture": None}
            continue
        fw, fh = face_size(f, size)
        faces[f] = {"uv": None, "texture": 0}
        jobs.append((faces[f], f, mat, max(1, U(fw)), max(1, U(fh)), tuple(painters), noise))
    centre = [(a + b) / 2 for a, b in zip(frm, to)]
    elements.append({
        "name": name, "type": "cube", "uuid": str(uuid.uuid4()), "box_uv": False, "rescale": False,
        "from": [round(v, 4) for v in frm], "to": [round(v, 4) for v in to],
        "origin": [round(v, 4) for v in (origin or centre)], "rotation": list(rot), "faces": faces,
    })
    return elements[-1]["uuid"]

def group(name, children, origin=(0, 0, 0), rot=(0, 0, 0)):
    return {"name": name, "uuid": str(uuid.uuid4()), "origin": list(origin), "rotation": list(rot),
            "isOpen": True, "visibility": True, "export": True, "children": children}

# ---- the cabinet: two blocks wide (x -8 .. 24; the rifle station's three, to 40), the left
# block's middle at x 0 ----
L, R = -8, 24 + W
cabinet = [
    cube("plinth", [L + 0.5, 0.02, -7.5], [R - 0.5, 1, 6.6], "steel_dark", [powder]),
    cube("side_l", [L, 0, -8], [L + 1, 14.5, 8], "steel", [powder, rivets, dark("east")]),
    cube("side_r", [R - 1, 0, -8], [R, 14.5, 8], "steel", [powder, rivets, dark("west")]),
    cube("back", [L + 1, 1, -8], [R - 1, 14.5, -7], "steel", [powder, rivets, dark("south")]),
    cube("shelf", [L + 1, 1, -7], [R - 1, 1.6, 7.6], "steel_dark", [powder]),
    cube("divider", [L + 1, 9.9, -7], [R - 1, 10.4, 7.6], "steel_dark", [powder]),
    cube("rail_top", [L + 1, 14.1, -7], [R - 1, 14.5, 7.6], "steel_dark", [powder]),
    cube("face_bottom", [L + 1, 1.6, 7.3], [R - 1, 2.2, 7.9], "steel", [powder]),
    cube("face_mid", [L + 1.02, 9.6, 7.3], [R - 1.02, 10.38, 7.9], "steel", [powder]),
    # corner posts at the front, slightly proud of the sides, and one in the middle below
    cube("post_l", [L - 0.2, 0.03, 7.6], [L + 1.2, 14.47, 8.2], "steel_dark", [powder]),
    cube("post_r", [R - 1.2, 0.03, 7.6], [R + 0.2, 14.47, 8.2], "steel_dark", [powder]),
    cube("post_mid", [7.3, 0.04, 7.32], [8.7, 9.6, 8.0], "steel_dark", [powder]),
] + ([cube("post_mid2", [23.3, 0.04, 7.32], [24.7, 9.6, 8.0], "steel_dark", [powder])] if RIFLE else []) + [
    cube("foot_bl", [L + 0.2, -0.01, -7.8], [L + 1.4, 0.4, -6.6], "rubber"),
    cube("foot_br", [R - 1.4, -0.01, -7.8], [R - 0.2, 0.4, -6.6], "rubber"),
]
# the top's face in tiles of about 8 pixels
TILES = 6 if RIFLE else 4
TILE_W = 8.15 if not RIFLE else (TOP[2] - TOP[0]) / TILES
# ---- the worktop: butcher block with a plain gridded rubber mat on top (flush, y = 16) ----
top = [
    cube("top", [L - 0.3, 14.5, -8.3], [R + 0.3, 15.99, 8.4], "wood", [butcher_block], noise=3,
         only=("north", "south", "east", "west", "down")),
] + [
    # its top face in pieces (one texture page each)
    cube(f"mat_{i}{j}", [TOP[0] + i * TILE_W, 15.99, TOP[1] + j * 8.35], [TOP[0] + (i + 1) * TILE_W, 16, TOP[1] + (j + 1) * 8.35],
         "mat", [mat_tile(TOP[0] + i * TILE_W, TOP[1] + j * 8.35)], only=("up",))
    for i in range(TILES) for j in range(2)
] + [
    cube("top_edge_front", [L - 0.3, 14.2, 8.2], [R + 0.3, 14.5, 8.4], "steel_dark"),
]
# ---- the drawer across the front, with the cleaning brush and other tools in it ----
DL, DR = L + 1.3, R - 1.3
drawer = [
    cube("drawer_front", [DL, 10.6, 7.55], [DR, 13.95, 8.2], "drawer", [powder, drawer_front]),
    cube("handle_bar", [3.6 + W / 2, 11.7, 8.55], [12.4 + W / 2, 12.2, 8.9], "alu", [brushed]),
    cube("handle_post_l", [3.62 + W / 2, 11.72, 8.19], [4.1 + W / 2, 12.18, 8.56], "alu"),
    cube("handle_post_r", [11.9 + W / 2, 11.72, 8.19], [12.38 + W / 2, 12.18, 8.56], "alu"),
    cube("drawer_floor", [DL + 0.3, 10.62, -6], [DR - 0.3, 10.95, 7.56], "inside"),
    cube("drawer_side_l", [DL + 0.3, 10.95, -6], [DL + 0.65, 13.6, 7.56], "drawer", [dark("east")]),
    cube("drawer_side_r", [DR - 0.65, 10.95, -6], [DR - 0.3, 13.6, 7.56], "drawer", [dark("west")]),
    cube("drawer_back", [DL + 0.65, 10.95, -6], [DR - 0.65, 13.6, -5.65], "drawer", [dark("south")]),
    cube("drawer_split", [9.1 + W, 10.95, -5.65], [9.3 + W, 12.3, 7.56], "drawer"),
] + ([
    # (the rifle station's loader has its own bay in the middle: split off from the tools too)
    cube("drawer_split_tools", [9.1, 10.95, -5.65], [9.3, 12.3, 7.56], "drawer"),
] if RIFLE else []) + [
    # the left side: cleaning (a rod, oil, punches, patches; the brush is its own group)
    cube("rod", [-5.6, 10.95, -4.8], [-5.2, 11.35, 5.2], "metal", [brushed]),
    cube("rod_handle", [-5.8, 10.95, 5.2], [-5.0, 11.75, 7.0], "wood", [wood_grain]),
    cube("oil_bottle", [4.4, 10.95, 3.4], [5.9, 12.7, 4.9], "oil", [label((236, 232, 210))]),
    cube("oil_cap", [4.85, 12.7, 3.85], [5.45, 13.3, 4.45], "cap"),
    cube("punch_0", [6.6, 10.95, -4.6], [6.9, 11.25, 1.8], "metal"),
    cube("punch_1", [7.3, 10.95, -4.6], [7.6, 11.25, 1.2], "metal"),
    cube("punch_2", [8.0, 10.95, -4.6], [8.3, 11.25, 0.6], "brass"),
    cube("patches", [3.4, 10.95, -4.6], [6.0, 11.35, -2.2], "bristle", noise=6),
]
# The right side: three boxes of rounds, open at the top, the rounds standing in them (the
# game shows as many rows as there are rounds: `ammo_<box>_<level>`, 1 a third full .. 3 full)
# and how many on a label on the front (the game writes the number there).
def cartridge_bases(px, w, h, face):
    """Rounds standing in a box seen from above, packed in staggered rows: each case's rim
    (brass, lit on one side), its head and the silver primer in the middle."""
    if face != "up":
        return
    rect(px, 0, 0, w, h, (46, 38, 26))
    step = U(0.85)
    r = step * 0.5
    row = 0
    j = r
    while j < h + r:
        off = r if row % 2 else 0
        i = r * 0.5 + off
        while i < w + r:
            tone = random.randint(-10, 10)
            disc(px, i, j, r * 0.98, lambda c: shade(MATS["brass"], tone - 34))
            disc(px, i - r * 0.12, j - r * 0.12, r * 0.84, lambda c: shade(MATS["brass"], tone + 6))
            disc(px, i - r * 0.3, j - r * 0.3, r * 0.25, lambda c: shade(MATS["brass"], tone + 40))
            disc(px, i, j, r * 0.34, (150, 150, 156))
            disc(px, i - r * 0.08, j - r * 0.08, r * 0.14, (196, 196, 204))
            i += step
        j += step * 0.87
        row += 1

def shaded(*faces):
    """Faces inside an open box: the same metal, in shadow."""
    def f(px, w, h, face):
        if face in faces:
            rect(px, 0, 0, w, h, lambda c: shade(c, -26))
    return f

def can_front(px, w, h, face):
    """An ammo can's front: pressed edges round a flat panel (the game stencils the count
    on it)."""
    if face == "south":
        b = U(0.25)
        rect(px, 0, 0, w, b, lambda c: shade(c, 16))
        rect(px, 0, h - b, w, h, lambda c: shade(c, -18))
        rect(px, 0, 0, b, h, lambda c: shade(c, -8))
        rect(px, w - b, 0, w, h, lambda c: shade(c, -8))

AMMO_X = [(10.0 + W, 13.6 + W), (14.2 + W, 17.8 + W), (18.4 + W, 22.0 + W)]
AMMO_Z = (-4.4, 3.4)
AMMO_Y = (10.95, 13.3)
ammo_boxes = []
for i, (x0, x1) in enumerate(AMMO_X):
    z0, z1 = AMMO_Z
    y0, y1 = AMMO_Y
    t = 0.25
    walls = [
        cube(f"ammo_box_{i}_floor", [x0, y0, z0], [x1, y0 + 0.25, z1], "olive"),
        cube(f"ammo_box_{i}_front", [x0, y0 + 0.25, z1 - t], [x1, y1, z1], "olive", [can_front, shaded("north")]),
        cube(f"ammo_box_{i}_back", [x0, y0 + 0.25, z0], [x1, y1, z0 + t], "olive", [ammo_can_side, shaded("south")]),
        cube(f"ammo_box_{i}_l", [x0, y0 + 0.25, z0 + t], [x0 + t, y1, z1 - t], "olive", [ammo_can_side, shaded("east")]),
        cube(f"ammo_box_{i}_r", [x1 - t, y0 + 0.25, z0 + t], [x1, y1, z1 - t], "olive", [ammo_can_side, shaded("west")]),
    ]
    levels = []
    for k, z_end in enumerate((z0 + t + (z1 - z0 - 2 * t) / 3, z0 + t + (z1 - z0 - 2 * t) * 2 / 3, z1 - t)):
        levels.append(group(f"ammo_{i}_{k + 1}", [
            cube(f"ammo_rounds_{i}_{k + 1}", [x0 + t + 0.01, y1 - 0.45 - 0.01 * k, z0 + t + 0.01], [x1 - t - 0.01, y1 - 0.2 - 0.01 * k, z_end - 0.01],
                 "brass", [cartridge_bases], only=("up",)),
        ], ((x0 + x1) / 2, y1, (z0 + z1) / 2)))
    ammo_boxes.append(group(f"ammo_box_{i}", walls + levels, ((x0 + x1) / 2, y0, z1)))

# The rifle station's magazine loader, in the middle of its drawer (the game shows it once one
# is put there): a base plate the magazine lies on on its side (the game draws it at
# `loader_mag`), its lips in the connector at the right end, and a lamp on top. Its "feed"
# animation blinks the lamp (`loader_round`: lit) while it loads.
def plate_marks(px, w, h, face):
    if face == "up":
        rect(px, U(0.4), U(0.4), w - U(0.4), U(0.4) + 1, lambda c: shade(c, -20))
        rect(px, U(0.4), h - U(0.4) - 1, w - U(0.4), h - U(0.4), lambda c: shade(c, -20))

if RIFLE:
    # (out at the front of the drawer, where it comes out from under the top: seen while it
    # is open)
    LX0, LX1 = 10.4, 24.6          # the loader across the drawer's middle
    LZ0, LZ1 = 1.5, 7.35
    CZ = 4.45                      # its middle, front to back
    LY = 10.95                     # the drawer's floor
    MAG_AT = (14.7, LY + 1.15, CZ) # where the magazine lies (its middle)
    loader = [
        cube("loader_base", [LX0 + 0.6, LY, LZ0 + 0.4], [21.4, LY + 0.6, LZ1 - 0.4], "gunmetal", [powder, plate_marks]),
        # the connector the magazine's lips go into, its dark mouth toward the magazine
        cube("loader_socket", [18.6, LY + 0.59, CZ - 1.9], [21.2, LY + 3.0, CZ + 1.9], "metal", [brushed]),
        cube("loader_mouth", [18.4, LY + 0.9, CZ - 1.3], [18.62, LY + 2.2, CZ + 1.3], "black"),
        cube("loader_lamp", [19.6, LY + 2.99, CZ + 0.6], [20.4, LY + 3.2, CZ + 1.4], "black"),
    ]
    # the lamp lit (blinking while it loads: the game shows it only then)
    lamp_on = [cube("loader_lamp_on", [19.65, LY + 3.19, CZ + 0.65], [20.35, LY + 3.25, CZ + 1.35], "lamp_green")]
    g_feed_round = group("loader_round", lamp_on, (20.0, LY + 3.2, CZ + 1.0))
    g_loader_mag = group("loader_mag", [], MAG_AT)
    g_loader = group("loader", loader + [g_feed_round, g_loader_mag], (15.5, LY, CZ))

# The scrubbing brush: a wooden block with a knob to hold it by and bristles under it. The
# game hides it here while someone holds it, and draws it in their hand.
brush = [
    cube("brush_body", [-3.0, 11.75, 0.2], [3.0, 12.65, 3.0], "wood", [wood_grain]),
    cube("brush_knob", [-1.6, 12.64, 0.9], [1.6, 13.35, 2.3], "wood", [wood_grain]),
    cube("brush_bristles", [-2.8, 10.96, 0.35], [2.8, 11.76, 2.85], "bristle", noise=10),
]
# ---- on the shelf: an ammo can, a toolbox and a cleaning kit ----
shelf = [
    cube("can_body", [-6.4, 1.6, -5], [-1.2, 6.8, 6.6], "olive", [ammo_can_side, stencil(3)]),
    cube("can_lid", [-6.6, 6.8, -5.2], [-1.0, 7.4, 6.8], "olive"),
    cube("can_handle", [-4.6, 7.4, -0.4], [-3.0, 7.75, 3.8], "black"),
    cube("can_latch", [-4.2, 5.4, 6.6], [-3.4, 7.2, 6.95], "black"),
    cube("box_body", [0.2, 1.6, -4.4], [6.6, 5.4, 6.5], "red", [toolbox_side]),
    cube("box_lid", [0.1, 5.4, -4.5], [6.7, 6.2, 6.6], "red"),
    cube("box_handle", [2.4, 6.2, 0.3], [4.4, 7.4, 1.3], "black"),
    cube("box_latch", [3.0, 4.6, 6.5], [3.8, 5.8, 6.8], "alu"),
    cube("kit_body", [10.0 + W, 1.6, -5.0], [21.5 + W, 4.6, 5.5], "black", [toolbox_side]),
    cube("kit_lid", [9.9 + W, 4.6, -5.1], [21.6 + W, 5.2, 5.6], "black"),
    cube("kit_label", [13.0 + W, 2.4, 5.5], [18.5 + W, 3.8, 5.65], "cardboard", [cardboard_box]),
    cube("can2_body", [16.2 + W, 5.2, -4.0], [20.4 + W, 9.2, 4.0], "olive", [ammo_can_side, stencil(2)]),
] + ([
    # a long hard case for a rifle on the shelf in the middle, its latches and handle
    cube("case_body", [8.2, 1.6, -5.2], [24.8, 4.4, 5.8], "olive", [ammo_can_side]),
    cube("case_lid", [8.1, 4.4, -5.3], [24.9, 5.0, 5.9], "olive"),
    cube("case_handle", [14.6, 5.0, 0.2], [18.4, 5.5, 1.4], "black"),
    cube("case_latch_l", [10.4, 3.6, 5.8], [11.4, 4.9, 6.1], "alu"),
    cube("case_latch_r", [21.6, 3.6, 5.8], [22.6, 4.9, 6.1], "alu"),
] if RIFLE else [])

g_brush = group("brush", brush, (0, 12, 1.6))
g_drawer = group("drawer", drawer + [g_brush] + ammo_boxes + ([g_loader] if RIFLE else []), (8 + W // 2, 12, 7.5))
g_root = group("gun_station", [
    group("cabinet", cabinet), group("top", top), g_drawer, group("shelf", shelf),
])

# ---------- animations ----------
def kf(channel, t, x=0, y=0, z=0, interp="catmullrom"):
    return {"channel": channel, "data_points": [{"x": x, "y": y, "z": z}], "uuid": str(uuid.uuid4()),
            "time": t, "color": -1, "interpolation": interp}

def animator(g, *keys):
    return g["uuid"], {"name": g["name"], "type": "bone", "keyframes": list(keys)}

def animation(name, length, loop, *animators):
    return {"uuid": str(uuid.uuid4()), "name": name, "loop": loop, "override": False,
            "length": length, "snapping": 60, "selected": False, "anim_time_update": "",
            "blend_weight": "", "start_delay": "", "loop_delay": "", "animators": dict(animators)}

# The drawer slides out toward the player, a little too far and back (the game plays it
# backwards to close it).
open_anim = animation("open", 0.5, "hold",
    animator(g_drawer, kf("position", 0), kf("position", 0.12, z=0.6), kf("position", 0.38, z=7.4),
             kf("position", 0.5, z=7.0)),
)
open_anim["selected"] = True
anims = [open_anim]
if RIFLE:
    # The loader at work: its lamp blinks, once for each round it pushes in.
    anims.append(animation("feed", 0.35, "loop",
        animator(g_feed_round, kf("scale", 0, 1, 1, 1, interp="step"), kf("scale", 0.18, 0, 0, 0, interp="step")),
    ))

# ---------- checks ----------
def check_coplanar():
    boxes = [(e["name"], e["from"], e["to"]) for e in elements if not any(e["rotation"])]
    bad = []
    for i in range(len(boxes)):
        for j in range(i + 1, len(boxes)):
            (na, fa, ta), (nb, fb, tb) = boxes[i], boxes[j]
            for ax in range(3):
                o = [k for k in range(3) if k != ax]
                if not all(min(ta[k], tb[k]) - max(fa[k], fb[k]) > 1e-3 for k in o):
                    continue
                for pa, sa in ((fa[ax], -1), (ta[ax], 1)):
                    for pb, sb in ((fb[ax], -1), (tb[ax], 1)):
                        if abs(pa - pb) < 1e-4 and sa == sb:
                            bad.append((na, nb, "xyz"[ax], pa))
    return bad

# ---------- texture ----------
jobs.sort(key=lambda j: (-j[4], -j[3]))
x = y = row_h = 0
placed = []
for job in jobs:
    w, h = job[3] + 2, job[4] + 2
    if x + w > ATLAS_W:
        x, y, row_h = 0, y + row_h, 0
    placed.append((job, x, y))
    x += w
    row_h = max(row_h, h)
TEX_H = ((y + row_h + 15) // 16) * 16
img = [[(0, 0, 0, 0)] * ATLAS_W for _ in range(TEX_H)]
for (face_ref, face, mat, tw, th, painters, noise), ax, ay in placed:
    px = paint_face(face, mat, tw, th, painters, noise)
    for j in range(-1, th + 1):
        row = img[ay + 1 + j]
        src = px[min(max(j, 0), th - 1)]
        for i in range(-1, tw + 1):
            row[ax + 1 + i] = src[min(max(i, 0), tw - 1)] + (255,)
    face_ref["uv"] = [ax + 1, ay + 1, ax + 1 + tw, ay + 1 + th]

raw = b"".join(b"\x00" + b"".join(bytes(p) for p in row) for row in img)
def chunk(t, d): return struct.pack(">I", len(d)) + t + d + struct.pack(">I", zlib.crc32(t + d) & 0xffffffff)
png = (b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", ATLAS_W, TEX_H, 8, 6, 0, 0, 0))
       + chunk(b"IDAT", zlib.compress(raw, 9)) + chunk(b"IEND", b""))

model = {
    "meta": {"format_version": "4.10", "model_format": "free", "box_uv": False},
    "name": NAME,
    "resolution": {"width": ATLAS_W, "height": TEX_H},
    "elements": elements,
    "outliner": [g_root],
    "textures": [{
        "name": NAME + ".png", "id": "0", "uuid": str(uuid.uuid4()), "folder": "", "namespace": "",
        "width": ATLAS_W, "height": TEX_H, "uv_width": ATLAS_W, "uv_height": TEX_H,
        "particle": False, "render_mode": "default", "visible": True, "saved": False,
        "source": "data:image/png;base64," + base64.b64encode(png).decode(),
    }],
    "animations": anims,
}
with open(os.path.join(HERE, NAME + ".bbmodel"), "w") as f:
    json.dump(model, f, indent=1)
with open(os.path.join(HERE, NAME + ".png"), "wb") as f:
    f.write(png)

bad = check_coplanar()
for b in bad:
    print("coplanar:", *b)
print(f"ok: {len(elements)} cubes, texture {ATLAS_W}x{TEX_H}, {len(bad)} coplanar pairs")
