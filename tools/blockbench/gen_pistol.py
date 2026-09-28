"""Generates pistol.bbmodel (+ pistol.png): a first-person pistol view model for Blockbench.

Run:  python tools/blockbench/gen_pistol.py

Model space: 1 unit = 1 Blockbench pixel, the first-person camera at (0, 0, 0) looking -Z
(the whole rig is moved there by OFF). Every face gets its own painted spot on one texture
atlas (D texels per unit). Rounded edges are built from 45-degree chamfer strips and arcs of
rotated segments; parts that touch are sunk slightly into each other so no two visible faces
share a plane (checked by `check_coplanar` at the end).
"""
import base64, json, math, os, random, struct, uuid, zlib

random.seed(7)
D = 8                 # texels per model unit
K = D / 4             # painter scale (painters were tuned at 4 texels per unit)
ATLAS_W = 1024
HERE = os.path.dirname(os.path.abspath(__file__))

MATS = {
    "slide": (52, 55, 60), "frame": (30, 31, 34), "grip": (34, 35, 38), "metal": (112, 117, 124),
    "barrel": (70, 72, 76), "sight": (20, 20, 22), "mag": (26, 27, 29), "magsteel": (38, 40, 44),
    "bore": (12, 12, 13), "skin": (196, 140, 102), "sleeve": (52, 58, 70), "brass": (190, 150, 62),
    "copper": (178, 104, 66), "can": (38, 40, 43), "laser": (44, 46, 50), "lens": (225, 36, 28),
    "scope": (30, 32, 36), "glass": (150, 196, 220), "ext": (26, 27, 29), "spring": (146, 150, 156),
    "lamp": (36, 38, 42), "lamp_lens": (236, 234, 214),
}
# How opaque each material's texels are (255 unless given): the lenses let the view through.
MAT_ALPHA = {"glass": 96}

# Texels per unit of the face being painted: D, or more on the small parts that need it.
DENSITY = [D]

def U(v):
    """Model units to texels."""
    return int(round(v * DENSITY[0]))

def clamp(v): return max(0, min(255, int(v)))
def shade(c, d): return tuple(clamp(v + d) for v in c)

def rect(px, x0, y0, x1, y1, col):
    for j in range(max(0, int(y0)), min(len(px), int(y1))):
        for i in range(max(0, int(x0)), min(len(px[0]), int(x1))):
            px[j][i] = col(px[j][i]) if callable(col) else col

def disc(px, cx, cy, r, col):
    for j in range(len(px)):
        for i in range(len(px[0])):
            if math.hypot(i + 0.5 - cx, j + 0.5 - cy) < r:
                px[j][i] = col(px[j][i]) if callable(col) else col

def paint_face(face, mat, w, h, painters):
    base = MATS[mat]
    px = [[None] * w for _ in range(h)]
    for j in range(h):
        for i in range(w):
            d = random.randint(-3, 3)
            if face == "up": d += 10
            elif face == "down": d -= 14
            elif face in ("north", "south"): d -= 4
            # worn, lit edges: light along the top, dark along the bottom
            if j == 0: d += 18
            elif j == 1 and h > 6: d += 7
            elif j == h - 1: d -= 16
            if i == 0 or i == w - 1: d -= 6
            px[j][i] = shade(base, d)
    for p in painters:
        p(px, w, h, face)
    return px

# ---------- detail painters (u runs along a face, v down it; see face_size) ----------
def serrations(px, w, h, face):
    # the rear of the slide (+z) is at u=0 on the east face and at u=w on the west face
    if face not in ("east", "west"): return
    for k in range(8):
        u = U(0.4 + k * 0.6)
        if face == "west": u = w - u - 3
        rect(px, u, U(0.4), u + 2, h - U(0.4), lambda c: shade(c, -28))
        rect(px, u + 2, U(0.4), u + 3, h - U(0.4), lambda c: shade(c, 12))

def slide_marking(px, w, h, face):
    if face == "west":  # a line of engraved lettering near the front (u from -z)
        u = U(1.2)
        for k in range(9):
            glyph = random.getrandbits(15) | 0b100000000000001
            for gy in range(5):
                for gx in range(3):
                    if glyph >> (gy * 3 + gx) & 1:
                        rect(px, u + gx, U(1.1) + gy, u + gx + 1, U(1.1) + gy + 1, lambda c: shade(c, 34))
            u += 5
    if face == "south":  # striker plate with its pin
        cx, cy = w // 2, h // 2 + U(0.3)
        rect(px, cx - U(0.6), cy - U(0.8), cx + U(0.6), cy + U(0.8), lambda c: shade(c, -20))
        rect(px, cx - U(0.6), cy - U(0.8), cx + U(0.6), cy - U(0.8) + 1, lambda c: shade(c, 25))
        disc(px, cx, cy, U(0.22), (150, 152, 156))

def sight_dot(v=0.5):
    """A small white dot on a sight's back face (toward the eye), with a thin dark ring round
    it: 0.15 units across its middle, `v` of the way down the face."""
    def f(px, w, h, face):
        if face == "south":
            cx, cy = w / 2, h * v
            disc(px, cx, cy, 0.2 * DENSITY[0], (10, 10, 11))
            disc(px, cx, cy, 0.15 * DENSITY[0], (238, 238, 228))
    return f

def stipple(px, w, h, face):
    if face not in ("east", "west", "south"): return
    b = U(0.25)
    for j in range(b, h - b):
        for i in range(b, w - b):
            r = random.random()
            if r < 0.33: px[j][i] = shade(px[j][i], -13)
            elif r < 0.52: px[j][i] = shade(px[j][i], 12)
    if face in ("east", "west") and w > U(2.5):  # moulded logo ring
        cx, cy = w / 2, h * 0.2
        for j in range(h):
            for i in range(w):
                if abs(math.hypot(i + 0.5 - cx, j + 0.5 - cy) - U(0.6)) < 0.7:
                    px[j][i] = shade(MATS["grip"], 30)

def finger_grooves(px, w, h, face):
    if face != "north": return
    for k in range(1, 4):
        v = int(h * k / 4)
        rect(px, 0, v - 1, w, v + 1, lambda c: shade(c, -24))
        rect(px, 0, v + 1, w, v + 2, lambda c: shade(c, 14))

def rail_slots(px, w, h, face):
    for k in range(3):
        a = U(0.6 + k * 1.75)
        if face == "down":
            rect(px, 1, a, w - 1, a + U(0.6), lambda c: shade(c, -22))
        elif face in ("east", "west"):
            rect(px, a, 0, a + U(0.6), h, lambda c: shade(c, -18))

def frame_detail(px, w, h, face):
    # frame spans z -11.5..8: east u = 8 - z, west u = z + 11.5
    if face not in ("east", "west"): return
    lu = U(8 + 4) if face == "east" else U(-4 + 11.5)       # takedown lever at z = -4
    rect(px, lu - U(0.4), 1, lu + U(0.4), h - 1, lambda c: shade(c, -16))
    rect(px, lu - U(0.4), 1, lu + U(0.4), 2, lambda c: shade(c, 20))
    pu = U(8 - 2) if face == "east" else U(2 + 11.5)       # trigger pin at z = 2
    disc(px, pu, h / 2, U(0.2), (120, 124, 130))

def trigger_blade(px, w, h, face):
    if face == "north":
        rect(px, w // 2 - 1, 1, w // 2 + 1, h - 1, lambda c: shade(c, 40))

def dark(*faces):
    """Faces that look into a cavity: nearly black."""
    def f(px, w, h, face):
        if face in faces:
            rect(px, 0, 0, w, h, lambda c: shade(MATS["bore"], random.randint(-2, 2)))
    return f

def case_head(px, w, h, face):
    if face != "south": return
    cx, cy = w / 2, h / 2
    for j in range(h):
        for i in range(w):
            r = math.hypot(i + 0.5 - cx, j + 0.5 - cy)
            if r < U(0.17): px[j][i] = (200, 200, 205)                     # primer
            elif r < U(0.2): px[j][i] = shade(px[j][i], -20)
            elif r > min(w, h) / 2 - 1: px[j][i] = shade(px[j][i], -30)   # rim edge

def case_shine(px, w, h, face):
    if face in ("east", "west", "up"):
        rect(px, 0, 1, w, 1 + K, lambda c: shade(c, 40))
    if face in ("east", "west"):  # extractor groove near the head (+z)
        g = U(0.25) if face == "east" else w - U(0.25) - 1
        rect(px, g, 0, g + 1, h, lambda c: shade(c, -30))

def bullet_tip(px, w, h, face):
    if face == "north":
        disc(px, w / 2, h / 2, U(0.14), lambda c: shade(c, 40))
    elif face in ("east", "west", "up", "down"):
        # the ogive darkens toward the nose (-z: u=w on east, u=0 on west/up/down)
        for i in range(w):
            t = i / max(1, w - 1)
            k = 1 - t if face == "east" else t
            rect(px, i, 0, i + 1, h, lambda c, k=k: shade(c, int(-26 * (1 - k))))

# witness holes on both sides of the magazine, toward its back; shared with the brass behind them
# The grip and the magazine as long as a Glock 17's are to its slide (139 mm high against
# 202 mm long): the grip's bottom at GB (it was -4.5, a third too long).
GB = -0.5
MAG_BODY = dict(y0=GB + 0.05, y1=6.2, z0=0.5, z1=4.3)
HOLE, HOLES = 0.45, [1.0 + k * 1.25 for k in range(5)]   # size, distance of each from the top
HOLE_U = (0.3, 0.75)                                     # from the magazine's back edge

def witness_holes(px, w, h, face):
    if face not in ("east", "west"): return
    for v in HOLES:
        u0 = U(HOLE_U[0]) if face == "east" else w - U(HOLE_U[1])
        rect(px, u0, U(v), u0 + U(HOLE), U(v + HOLE), (8, 8, 9))
        rect(px, u0, U(v + HOLE), u0 + U(HOLE), U(v + HOLE) + 1, lambda c: shade(c, 22))
    # stamped round count next to the lowest hole
    u = U(1.1) if face == "east" else w - U(1.1) - 3
    rect(px, u, U(HOLES[-1]), u + 3, U(HOLES[-1]) + 5, lambda c: shade(c, 30))

def base_plate(px, w, h, face):
    if face == "down":
        for k in range(4):
            v = U(0.7 + k * 1.3)
            rect(px, U(0.3), v, w - U(0.3), v + 2, lambda c: shade(c, -20))

def bore_face(r):
    """The front of a tube with a hole of radius `r` (units) in its middle."""
    def f(px, w, h, face):
        if face == "north":
            disc(px, w / 2, h / 2, r * DENSITY[0] + 0.8, lambda c: shade(c, -25))
            disc(px, w / 2, h / 2, r * DENSITY[0], (8, 8, 9))
    return f

def knurl(px, w, h, face):
    """Fine grooves around a tube (across its length)."""
    if face in ("east", "west", "up", "down"):
        along = w if face in ("east", "west") else h
        for k in range(1, along, 3):
            if face in ("east", "west"):
                rect(px, k, 0, k + 1, h, lambda c: shade(c, -22))
            else:
                rect(px, 0, k, w, k + 1, lambda c: shade(c, -22))

def can_bands(px, w, h, face):
    """A silencer's tube: two thin bands and a faint lengthwise sheen."""
    if face in ("east", "west"):
        for u in (U(1.4), w - U(1.6)):
            rect(px, u, 0, u + 1, h, lambda c: shade(c, -18))
            rect(px, u + 1, 0, u + 2, h, lambda c: shade(c, 10))
        rect(px, 0, h // 3, w, h // 3 + 1, lambda c: shade(c, 9))
    if face in ("up", "down"):
        for v in (U(1.4), h - U(1.6)):
            rect(px, 0, v, w, v + 1, lambda c: shade(c, -18))

def lens_glow(px, w, h, face):
    if face == "north":
        cx, cy = w / 2, h / 2
        for j in range(h):
            for i in range(w):
                r = math.hypot(i + 0.5 - cx, j + 0.5 - cy) / max(1.0, min(w, h) / 2)
                px[j][i] = (255, clamp(210 - 170 * r), clamp(190 - 180 * r))

def lamp_face(px, w, h, face):
    """A weapon light's front: a bright lens, the reflector's rings behind it and the LED in
    the middle."""
    if face != "north":
        return
    cx, cy = w / 2, h / 2
    for j in range(h):
        for i in range(w):
            r = math.hypot(i + 0.5 - cx, j + 0.5 - cy) / max(1.0, min(w, h) / 2)
            ringed = int(r * 5) % 2
            px[j][i] = shade(MATS["lamp_lens"], int(-60 * r) + (8 if ringed else -6))
    disc(px, cx, cy, U(0.22), (255, 252, 236))

def lamp_grip(px, w, h, face):
    """Cooling fins round the body: grooves across it."""
    if face in ("east", "west", "up", "down"):
        along = w if face in ("east", "west") else h
        for k in range(U(0.6), along - U(1.4), U(0.45)):
            if face in ("east", "west"):
                rect(px, k, 0, k + 1, h, lambda c: shade(c, -22))
            else:
                rect(px, 0, k, w, k + 1, lambda c: shade(c, -22))

def glass(px, w, h, face):
    """Clear glass: even, with a soft reflection streak across it."""
    if face in ("north", "south"):
        for j in range(h):
            for i in range(w):
                d = (i - j) / max(1, w + h) * 2
                px[j][i] = shade(MATS["glass"], int(55 * max(0.0, 1 - abs(d - 0.25) * 6)))

def turret_cap(px, w, h, face):
    if face in ("up", "east", "west", "north", "south"):
        for k in range(1, w, 2):
            rect(px, k, 0, k + 1, h, lambda c: shade(c, -16))

def ring_screws(px, w, h, face):
    if face in ("east", "west"):
        disc(px, w / 2, h * 0.25, U(0.14), (120, 124, 130))
        disc(px, w / 2, h * 0.75, U(0.14), (120, 124, 130))

def laser_switch(px, w, h, face):
    if face in ("east", "west"):
        rect(px, 1, 1, w - 1, h - 1, lambda c: shade(c, 18))

def coils(px, w, h, face):
    """A coil spring's turns: dark gaps across it every half unit, lit crests between."""
    if face in ("east", "west", "up", "down"):
        along = w if face in ("east", "west") else h
        step = max(2, U(0.5))
        for k in range(0, along, step):
            if face in ("east", "west"):
                rect(px, k, 0, k + 1, h, lambda c: shade(c, -60))
                rect(px, k + step // 2, 0, k + step // 2 + 1, h, lambda c: shade(c, 30))
            else:
                rect(px, 0, k, w, k + 1, lambda c: shade(c, -60))
                rect(px, 0, k + step // 2, w, k + step // 2 + 1, lambda c: shade(c, 30))

def ext_stripe(px, w, h, face):
    if face in ("east", "west", "north", "south"):
        rect(px, 0, h // 2 - 1, w, h // 2 + 1, (214, 104, 32))

# ---------- model building ----------
OFF = (7, -24, -34)   # the rig sits at the hip, down-right of the camera
def sh(p): return [round(a + o, 4) for a, o in zip(p, OFF)]

FACES = ("north", "east", "south", "west", "up", "down")
def face_size(face, s):
    sx, sy, sz = s
    return {"north": (sx, sy), "south": (sx, sy), "east": (sz, sy), "west": (sz, sy),
            "up": (sx, sz), "down": (sx, sz)}[face]

elements, jobs = [], []

def cube(name, frm, to, mat, painters=(), origin=None, rot=(0, 0, 0), density=D, only=None):
    size = [b - a for a, b in zip(frm, to)]
    assert all(s > 0 for s in size), name
    faces = {}
    for f in FACES:
        if only is not None and f not in only:
            faces[f] = {"uv": [0, 0, 0, 0], "texture": None}
            continue
        fw, fh = face_size(f, size)
        faces[f] = {"uv": None, "texture": 0}
        DENSITY[0] = density
        jobs.append((faces[f], f, mat, max(1, U(fw)), max(1, U(fh)), tuple(painters), density))
        DENSITY[0] = D
    centre = [(a + b) / 2 for a, b in zip(frm, to)]
    elements.append({
        "name": name, "type": "cube", "uuid": str(uuid.uuid4()), "box_uv": False, "rescale": False,
        "from": sh(frm), "to": sh(to), "origin": sh(origin or centre), "rotation": list(rot),
        "faces": faces,
    })
    return elements[-1]["uuid"]

def chamfer(name, axis, centre, half_diag, span, mat, painters=()):
    """A 45-degree strip along `axis` ('x' or 'z') centred on an edge: fills the corner
    between two faces so the edge reads as rounded. span = (from, to) along the axis."""
    s = half_diag * math.sqrt(2) / 2
    a, b = centre
    if axis == "z":   # centre = (x, y)
        return cube(name, [a - s, b - s, span[0]], [a + s, b + s, span[1]], mat, painters,
                    [a, b, (span[0] + span[1]) / 2], (0, 0, 45))
    else:             # centre = (y, z)
        return cube(name, [span[0], a - s, b - s], [span[1], a + s, b + s], mat, painters,
                    [(span[0] + span[1]) / 2, a, b], (45, 0, 0))

def tube(name, axis, r, z0, z1, mat, painters=(), front=(), density=None):
    """A round tube along z (octagonal): two crossed boxes and four 45-degree corner strips.
    `front` paints the front (north) face of the widest box."""
    x, y = axis
    n = r * 0.414
    d = density or D
    parts = [
        cube(name + "_h", [x - r, y - n, z0], [x + r, y + n, z1], mat, tuple(painters) + tuple(front), density=d),
        cube(name + "_v", [x - n, y - r, z0 + 0.003], [x + n, y + r, z1 - 0.003], mat, painters, density=d),
    ]
    for i, (sx, sy) in enumerate(((1, 1), (1, -1), (-1, 1), (-1, -1))):
        parts.append(chamfer(f"{name}_c{i}", "z", (x + sx * n, y + sy * n), r - n, (z0 + 0.006, z1 - 0.006), mat, painters))
    return parts

def ring(name, axis, r, t, z0, z1, mat, painters=()):
    """A hollow octagonal tube along z: eight walls `t` thick, `r` to their outside, dark
    inside."""
    x, y = axis
    w = 2 * r * math.tan(math.radians(22.5)) + 0.01
    parts = []
    for k in range(8):
        th = k * 45.0
        a = r - t / 2
        cx, cy = x + a * math.cos(math.radians(th)), y + a * math.sin(math.radians(th))
        parts.append(cube(f"{name}_w{k}", [cx - t / 2, cy - w / 2, z0], [cx + t / 2, cy + w / 2, z1], mat,
                          tuple(painters) + (dark("west"),), [cx, cy, (z0 + z1) / 2], (0, 0, th)))
    return parts

def glass_disc(name, axis, r, z0, z1):
    """A flat octagonal pane (`r` to the middle of its edges) across a tube: side by side
    boxes that do not overlap, only their front and back drawn (see-through glass would show
    every face behind another)."""
    x, y = axis
    diag = r * math.sqrt(2)
    # columns from the middle out, each as tall as the octagon lets its outer edge be
    edges = [0.0, r * 0.25, r * 0.45, r * 0.6, r * 0.72, r * 0.83, r * 0.92, r]
    parts = []
    for i in range(len(edges) - 1):
        a, b = edges[i], edges[i + 1]
        half = min(r, diag - b + (b - a) * 0.5)
        spans = [(-b, b)] if a == 0.0 else [(a, b), (-b, -a)]
        for j, (u0, u1) in enumerate(spans):
            parts.append(cube(f"{name}_{i}{j}", [x + u0, y - half, z0], [x + u1, y + half, z1], "glass",
                              [glass], density=2 * D, only=("north", "south")))
    return parts

def group(name, children, origin=(0, 0, 0), rot=(0, 0, 0), visible=True):
    return {"name": name, "uuid": str(uuid.uuid4()), "origin": sh(origin), "rotation": list(rot),
            "isOpen": True, "visibility": visible, "export": True, "children": children}

GP, GR = [0, 8, 2.5], (-18, 0, 0)     # grip pivot and tilt

# ---- slide: x +-1.5, y 9..12.9 (sides) with a chamfered deck up to 13.3, z -12..8 ----
# the ejection port is a real opening: z -1.5..2, the top open from x -0.1, the side from y 12.1
PORT = (-1.5, 2)
slide = [
    cube("slide_rear", [-1.5, 9, PORT[1]], [1.5, 12.9, 8], "slide", [serrations, slide_marking, dark("north")]),
    cube("slide_front", [-1.5, 9, -10], [1.5, 12.9, PORT[0]], "slide", [slide_marking, dark("south")]),
    cube("slide_port_floor", [-1.5, 9, PORT[0]], [1.5, 11.0, PORT[1]], "slide", [dark("up")]),
    cube("slide_port_wall", [-1.5, 11.0, PORT[0]], [-0.1, 12.9, PORT[1]], "slide", [dark("east")]),
    cube("slide_port_lip", [0.9, 11.0, PORT[0]], [1.5, 12.1, PORT[1]], "slide", [dark("up", "west")]),
    # nose: a frame around the opening the barrel sits in
    cube("slide_nose_top", [-1.5, 11.9, -12], [1.5, 12.9, -10], "slide"),
    cube("slide_nose_bottom", [-1.5, 9, -12], [1.5, 10.3, -10], "slide"),
    cube("slide_nose_l", [-1.5, 10.3, -12], [-0.8, 11.9, -10], "slide"),
    cube("slide_nose_r", [0.8, 10.3, -12], [1.5, 11.9, -10], "slide"),
    # the deck on top, and the chamfers that round its edges into the sides, front and back
    cube("slide_deck_front", [-1.1, 12.9, -11.6], [1.1, 13.3, PORT[0]], "slide"),
    cube("slide_deck_rear", [-1.1, 12.9, PORT[1]], [1.1, 13.3, 7.6], "slide"),
    cube("slide_deck_mid", [-1.1, 12.9, PORT[0]], [-0.1, 13.3, PORT[1]], "slide"),
    chamfer("slide_chamfer_l", "z", (-1.1, 12.9), 0.4, (-11.6, 7.6), "slide"),
    chamfer("slide_chamfer_r_front", "z", (1.1, 12.9), 0.4, (-11.6, PORT[0]), "slide"),
    chamfer("slide_chamfer_r_rear", "z", (1.1, 12.9), 0.4, (PORT[1], 7.6), "slide"),
    chamfer("slide_chamfer_front", "x", (12.9, -11.6), 0.4, (-1.1, 1.1), "slide"),
    chamfer("slide_chamfer_rear", "x", (12.9, 7.6), 0.4, (-1.1, 1.1), "slide"),
]
# Barrel: a hollow tube; the corner fillers make the bore octagonal (round-ish). Its own part:
# it stays when the slide flies back (sticking out of the slide's nose, as a real one does),
# and comes out of the gun when it is taken apart.
barrel = [
    cube("barrel_top", [-0.79, 11.5, -12.3], [0.79, 11.89, -9.99], "barrel"),
    cube("barrel_bottom", [-0.79, 10.31, -12.3], [0.79, 10.7, -9.99], "barrel"),
    cube("barrel_l", [-0.79, 10.7, -12.3], [-0.4, 11.5, -9.99], "barrel"),
    cube("barrel_r", [0.4, 10.7, -12.3], [0.79, 11.5, -9.99], "barrel"),
    cube("bore_end", [-0.4, 10.7, -10.1], [0.4, 11.5, -10.03], "bore"),
] + [
    chamfer(f"bore_round_{i}", "z", (x, y), 0.16, (-12.29, -10.04), "barrel")
    for i, (x, y) in enumerate([(-0.4, 10.7), (0.4, 10.7), (-0.4, 11.5), (0.4, 11.5)])
]
slide += [
    # Sights like a real pistol's. Front: one thin post, its front sloped, a white dot on its
    # back. Rear: one block dovetailed into the slide, a square notch in its middle, the two
    # ears either side of it with a white dot each and their fronts sloped. The post's top
    # and the ears' tops are on the sight line.
    # (twice the texels: they are small and seen close up when aiming)
    cube("front_sight", [-0.3, 13.27, -10.65], [0.3, 14.3, -10.0], "sight", [sight_dot(0.35)], density=2 * D),
    cube("front_sight_step", [-0.29, 13.28, -11.0], [0.29, 13.9, -10.64], "sight", density=2 * D),
    cube("rear_sight_base", [-1.3, 13.27, 5.7], [1.3, 13.72, 7.3], "sight", density=2 * D),
    cube("rear_sight_l", [-1.29, 13.7, 6.15], [-0.32, 14.3, 7.29], "sight", [sight_dot(0.5)], density=2 * D),
    cube("rear_sight_r", [0.32, 13.7, 6.15], [1.29, 14.3, 7.29], "sight", [sight_dot(0.5)], density=2 * D),
    cube("rear_sight_l_step", [-1.28, 13.71, 5.71], [-0.33, 14.0, 6.16], "sight", density=2 * D),
    cube("rear_sight_r_step", [0.33, 13.71, 5.71], [1.28, 14.0, 6.16], "sight", density=2 * D),
]

# ---- frame ----
def guard_arc():
    """Trigger guard: a front bar, a bottom bar, and a rounded corner of 4 rotated segments
    on a radius-1 arc around (y 5.3, z -3.7)."""
    R, cy, cz = 1.0, 5.3, -3.7
    parts = [
        cube("guard_front", [-0.55, cy, cz - R - 0.3], [0.55, 7.55, cz - R + 0.3], "frame"),
        cube("guard_bottom", [-0.56, cy - R - 0.3, cz], [0.56, cy - R + 0.3, 1.6], "frame"),
    ]
    for i, phi in enumerate((11.25, 33.75, 56.25, 78.75)):
        p = math.radians(phi)
        y, z = cy - R * math.sin(p), cz - R * math.cos(p)
        hw = 0.535 if i % 2 else 0.545
        parts.append(cube(f"guard_arc_{i}", [-hw, y - 0.25, z - 0.3], [hw, y + 0.25, z + 0.3], "frame",
                          origin=[0, y, z], rot=(-phi, 0, 0)))
    return parts

frame = [
    cube("frame", [-1.4, 7.5, -11.5], [1.4, 9, 8], "frame", [frame_detail]),
    cube("rail", [-1.2, 6.7, -11], [1.2, 7.55, -5], "frame", [rail_slots]),
    cube("beavertail", [-1.3, 7.6, 7.95], [1.3, 8.8, 8.9], "frame"),
    chamfer("beavertail_round", "x", (8.2, 8.9), 0.6, (-1.28, 1.28), "frame"),
    cube("slide_stop", [-1.65, 8.5, -3.5], [-1.37, 9.2, -1], "metal"),
    cube("mag_release", [-1.62, 6.4, 0.6], [-1.3, 7.2, 1.4], "metal"),
] + guard_arc()

# trigger: two segments, the lower one curling forward
TJ = (7.5 - 1.15 * math.cos(math.radians(5)), -2.2 - 1.15 * math.sin(math.radians(5)))
trigger = [
    cube("trigger_upper", [-0.3, 6.35, -2.6], [0.3, 7.5, -1.8], "metal", [trigger_blade],
         origin=[0, 7.5, -2.2], rot=(5, 0, 0)),
    cube("trigger_lower", [-0.29, TJ[0] - 1.25, TJ[1] - 0.35], [0.29, TJ[0] + 0.1, TJ[1] + 0.35], "metal",
         [trigger_blade], origin=[0, TJ[0], TJ[1]], rot=(30, 0, 0)),
]

# ---- recoil spring on its guide rod, under the barrel inside the slide ----
recoil_spring = [
    cube("spring_coil", [-0.42, 9.35, -11.2], [0.42, 10.19, -3.6], "spring", [coils]),
    cube("spring_rod_tip", [-0.18, 9.59, -11.7], [0.18, 9.95, -11.19], "metal"),
    cube("spring_flange", [-0.55, 9.22, -3.61], [0.55, 10.32, -3.2], "metal"),
]

# ---- grip: stepped profile so its edges read round (all tilted with the grip) ----
grip = [
    cube("grip_core", [-1.5, GB, 0.35], [1.5, 8, 4.65], "grip", [stipple], GP, GR),
    cube("grip_mid", [-1.32, GB + 0.01, 0.12], [1.32, 7.99, 4.88], "grip", [stipple], GP, GR),
    cube("grip_front", [-1.05, GB + 0.02, 0], [1.05, 7.98, 5], "grip", [stipple, finger_grooves], GP, GR),
    cube("grip_backstrap", [-1.1, GB + 0.3, 4.95], [1.1, 7.6, 5.55], "grip", [stipple], GP, GR),
    cube("grip_backstrap_round", [-0.8, GB + 0.31, 5.5], [0.8, 7.59, 5.72], "grip", [stipple], GP, GR),
]

# ---- magazine (inside the grip at rest) ----
def round_at(name, x0, y0):
    """A cartridge lying in the magazine, pointing forward (-z), against its back: 9x19 mm, a
    19 mm case and 30 mm long (0.9 across here)."""
    return [
        cube(name + "_case", [x0, y0, 2.36], [x0 + 0.9, y0 + 0.9, 4.1], "brass", [case_head, case_shine], GP, GR),
        cube(name + "_bullet", [x0 + 0.1, y0 + 0.1, 1.4], [x0 + 0.8, y0 + 0.8, 2.37], "copper", [bullet_tip], GP, GR),
    ]

mb = MAG_BODY
magazine = [
    cube("mag_body", [-1.2, mb["y0"], mb["z0"]], [1.2, mb["y1"], mb["z1"]], "magsteel", [witness_holes, dark("up")], GP, GR),
    cube("mag_feed_lip_l", [-1.2, 6.15, 0.9], [-0.85, 7.0, 4.3], "magsteel", [], GP, GR),
    cube("mag_feed_lip_r", [0.85, 6.15, 0.9], [1.2, 7.0, 4.3], "magsteel", [], GP, GR),
]
# the base plate of the standard magazine; the extended one reaches 3.8 further out of the grip
mag_standard = [
    cube("mag_base", [-1.7, GB - 0.9, -0.2], [1.7, GB + 0.05, 5.8], "mag", [], GP, GR),
    cube("mag_base_bottom", [-1.5, GB - 1.1, 0.05], [1.5, GB - 0.85, 5.55], "mag", [base_plate], GP, GR),
    cube("mag_base_lip", [-1.45, GB, 0.2], [1.45, GB + 0.4, 5.4], "mag", [], GP, GR),
]
EXT = 3.8
mag_extended = [
    cube("ext_body", [-1.2, GB + 0.05 - EXT, mb["z0"]], [1.2, GB + 0.1, mb["z1"]], "magsteel", [], GP, GR),
    cube("ext_base", [-1.7, GB - 0.9 - EXT, -0.2], [1.7, GB + 0.05 - EXT, 5.8], "ext", [ext_stripe], GP, GR),
    cube("ext_base_bottom", [-1.5, GB - 1.1 - EXT, 0.05], [1.5, GB - 0.85 - EXT, 5.55], "ext", [base_plate], GP, GR),
    cube("ext_base_lip", [-1.45, GB - EXT, 0.2], [1.45, GB + 0.4 - EXT, 5.4], "ext", [], GP, GR),
]
mag_rounds = round_at("mag_round_top", -0.85, 6.3) + round_at("mag_round_2", -0.05, 5.5)
# brass seen through every witness hole (hidden with the rounds when the magazine is empty)
for k, v in enumerate(HOLES):
    y1 = mb["y1"] - v
    y0 = y1 - HOLE
    # both faces put the holes HOLE_U from the magazine's back (+z) edge
    z = (mb["z1"] - HOLE_U[1], mb["z1"] - HOLE_U[0])
    mag_rounds.append(cube(f"witness_brass_r{k}", [1.2, y0, z[0]], [1.212, y1, z[1]], "brass", [], GP, GR))
    mag_rounds.append(cube(f"witness_brass_l{k}", [-1.212, y0, z[0]], [-1.2, y1, z[1]], "brass", [], GP, GR))

# ---- arms: Minecraft proportions at twice the size, the fist around the grip ----
FR = (0, 4.5, 3.6)       # right fist centre
FL = (-4.2, 2.6, 2.4)    # left fist centre, cupping under the right hand
def arm(side, f, rot):
    x, y, z = f
    return group(side + "_arm_mesh", [
        cube(side + "_hand", [x - 3.5, y - 3.5, z - 3.5], [x + 3.5, y + 3.5, z + 13], "skin"),
        cube(side + "_sleeve", [x - 3.8, y - 3.8, z + 13], [x + 3.8, y + 3.8, z + 26], "sleeve"),
    ], f, rot)

# ---- attachments: each its own group, shown by the game only when fitted ----
# Silencer on the barrel's end: an octagonal can, a knurled collar at its back where it
# screws on, and an end cap with the bore.
BORE = (0.0, 11.1)
silencer = (
    tube("silencer_can", BORE, 1.3, -21.0, -12.2, "can", [can_bands], density=D)
    + tube("silencer_collar", BORE, 1.38, -13.6, -12.15, "can", [knurl])
    + tube("silencer_cap", BORE, 1.15, -21.3, -20.9, "metal", [], front=[bore_face(0.42)])
)
# Laser sight on the rail under the dust cover: a body with a stepped nose and a red lens,
# jaws clamping it to the rail with a screw, and pressure pads at its back.
laser = [
    cube("laser_body", [-1.0, 5.0, -10.9], [1.0, 6.62, -6.9], "laser", [frame_detail]),
    cube("laser_nose", [-0.8, 5.15, -11.3], [0.8, 6.45, -10.89], "laser"),
    cube("laser_lens", [-0.45, 5.35, -11.36], [0.45, 6.25, -11.29], "lens", [lens_glow], density=2 * D),
    cube("laser_jaw_l", [-1.32, 5.9, -9.9], [-0.99, 7.35, -8.1], "laser"),
    cube("laser_jaw_r", [0.99, 5.9, -9.9], [1.32, 7.35, -8.1], "laser"),
    cube("laser_screw", [1.32, 6.6, -9.2], [1.44, 6.95, -8.8], "metal", density=2 * D),
    cube("laser_pad_l", [-1.12, 5.4, -7.6], [-0.99, 5.9, -7.1], "metal", [laser_switch], density=2 * D),
    cube("laser_pad_r", [0.99, 5.4, -7.6], [1.12, 5.9, -7.1], "metal", [laser_switch], density=2 * D),
]
# Weapon light on the accessory rail (instead of the laser: the rail holds one): a round body
# with cooling grooves, a wider bezel at the front round a bright lens, jaws clamping it to the
# rail with a screw, and the switch paddles at its back.
LAMP = (0.0, 5.5)
flashlight = (
    tube("flashlight_body", LAMP, 1.02, -11.0, -6.9, "lamp", [lamp_grip])
    + tube("flashlight_bezel", LAMP, 1.22, -11.9, -10.95, "lamp", [knurl])
    + [
        cube("flashlight_lens", [-0.86, 4.64, -11.96], [0.86, 6.36, -11.9], "lamp_lens", [lamp_face], density=2 * D),
        cube("flashlight_jaw_l", [-1.32, 6.2, -9.9], [-0.99, 7.35, -8.1], "lamp"),
        cube("flashlight_jaw_r", [0.99, 6.2, -9.9], [1.32, 7.35, -8.1], "lamp"),
        cube("flashlight_screw", [1.32, 6.6, -9.2], [1.44, 6.95, -8.8], "metal", density=2 * D),
        cube("flashlight_paddle_l", [-1.2, 5.2, -7.4], [-1.0, 6.1, -6.95], "metal", [laser_switch], density=2 * D),
        cube("flashlight_paddle_r", [1.0, 5.2, -7.4], [1.2, 6.1, -6.95], "metal", [laser_switch], density=2 * D),
    ]
)

# Scope on a base plate on the slide: two rings, the tube, the objective bell in front, the
# eyepiece at the back, the elevation and windage turrets and the lenses.
AX = (0.0, 15.5)
scope = [
    cube("scope_base", [-0.9, 13.28, -6.2], [0.9, 13.76, 4.2], "scope"),
] + [
    part
    for i, z in enumerate((-4.6, 1.6))
    for part in (
        cube(f"scope_ring{i}_foot", [-1.2, 13.74, z], [1.2, AX[1] - 0.95, z + 1.0], "scope", [ring_screws]),
        cube(f"scope_ring{i}_l", [-1.2, AX[1] - 0.95, z], [-0.93, AX[1] + 0.95, z + 1.0], "scope"),
        cube(f"scope_ring{i}_r", [0.93, AX[1] - 0.95, z], [1.2, AX[1] + 0.95, z + 1.0], "scope"),
        cube(f"scope_ring{i}_top", [-1.2, AX[1] + 0.95, z], [1.2, AX[1] + 1.2, z + 1.0], "scope", [ring_screws]),
    )
] + (
    # hollow: the tube, the objective bell in front with a shoulder down to the tube, the
    # eyepiece at the back with its shoulder; glass at both ends to look through
    ring("scope_tube", AX, 0.95, 0.18, -5.6, 3.3, "scope")
    + ring("scope_bell", AX, 1.3, 0.2, -9.2, -5.45, "scope")
    + ring("scope_bell_shoulder", AX, 1.28, 0.53, -5.59, -5.44, "scope")
    + ring("scope_eyepiece", AX, 1.15, 0.2, 3.2, 5.3, "scope", [knurl])
    + ring("scope_eyepiece_shoulder", AX, 1.13, 0.4, 3.19, 3.36, "scope")
    + glass_disc("scope_lens_front", AX, 1.1, -9.0, -8.94)
    + glass_disc("scope_lens_back", AX, 0.95, 5.12, 5.18)
    + [
        cube("scope_turret_top", [-0.5, AX[1] + 0.9, -1.6], [0.5, AX[1] + 1.55, -0.6], "scope", [turret_cap], density=2 * D),
        cube("scope_turret_side", [0.9, AX[1] - 0.5, -1.6], [1.55, AX[1] + 0.5, -0.6], "scope", [turret_cap], density=2 * D),
    ]
)

# ---- hierarchy ----
g_silencer = group("silencer", silencer, (0, 11.1, -16))
g_scope = group("scope", scope, (0, 15.5, 0))
# the rear sight on its own: a scope on the slide takes its place (the game hides it then)
names = {e["uuid"]: e["name"] for e in elements}
rear = [u for u in slide if names.get(u, "").startswith("rear_sight")]
g_rear = group("rear_sight", rear, (0, 13.7, 6.5))
g_slide = group("slide", [u for u in slide if u not in rear] + [g_rear, g_scope], (0, 11, 0))
g_barrel = group("barrel", barrel + [
    cube("barrel_hood", [-0.8, 10.4, -2.9], [0.8, 11.33, 1.95], "barrel"),
    # the tube between the muzzle and the chamber (inside the slide: seen with the gun apart)
    cube("barrel_tube", [-0.72, 10.38, -9.97], [0.72, 11.82, -2.88], "barrel"),
    g_silencer],
                 (0, 11.1, -5))
g_spring = group("recoil_spring", recoil_spring, (0, 9.8, -7))
g_trigger = group("trigger", trigger, (0, 7.5, -2.2))
g_laser = group("laser", laser, (0, 6, -9))
g_flashlight = group("flashlight", flashlight, (0, 6, -9))
g_frame = group("frame", frame + [g_trigger, g_laser, g_flashlight], (0, 8, 0))
g_grip = group("grip", grip, GP)
g_chamber_case = group("chamber_case", [
    cube("chamber_case", [-0.45, 11.32, 0.06], [0.45, 12.22, 1.8], "brass", [case_head, case_shine])], (0, 11.8, 0))
g_chamber_bullet = group("chamber_bullet", [
    cube("chamber_bullet", [-0.35, 11.42, -0.9], [0.35, 12.12, 0.07], "copper", [bullet_tip])], (0, 11.8, -0.4))
g_chamber = group("chambered_round", [g_chamber_case, g_chamber_bullet], (0, 11.8, 0))
# the spent case thrown out of the port when firing (hidden inside the chambered one)
g_spent = group("spent_case", [
    cube("spent_case", [-0.43, 11.34, 0.08], [0.43, 12.2, 1.78], "brass", [case_head, case_shine])], (0, 11.77, 0.93))
# the left arm hangs off the magazine, so the hand is always where the magazine is;
# its keyframes are offsets from the magazine
g_larm = group("left_arm", [arm("left", FL, (15, -35, 0))], FL)
g_mag_rounds = group("magazine_rounds", mag_rounds, GP)
g_mag_std = group("mag_standard", mag_standard, GP)
# Blockbench shows the standard one; the eye in the outliner switches to the extended one
g_mag_ext = group("mag_extended", mag_extended, GP, visible=False)
g_mag_mesh = group("magazine_mesh", magazine + [g_mag_std, g_mag_ext, g_mag_rounds], GP)   # hidden on its own; the hand stays
g_mag = group("magazine", [g_mag_mesh, g_larm], GP)
g_pistol = group("pistol", [g_slide, g_barrel, g_spring, g_frame, g_grip, g_mag, g_chamber, g_spent], GP)
g_rarm = group("right_arm", [arm("right", FR, (15, 20, 0)), g_pistol], FR)
g_root = group("viewmodel", [g_rarm], FR)

# ---------- animations ----------
def kf(channel, t, x=0, y=0, z=0, interp="linear"):
    return {"channel": channel, "data_points": [{"x": x, "y": y, "z": z}], "uuid": str(uuid.uuid4()),
            "time": t, "color": -1, "interpolation": interp}

# Blockbench keyframes: rotation x/y and position x are given mirrored (Bedrock convention)
def R(x=0, y=0, z=0): return dict(x=-x, y=-y, z=z)
def P(x=0, y=0, z=0): return dict(x=-x, y=y, z=z)
def rot(t, interp="catmullrom", **k): return kf("rotation", t, interp=interp, **R(**k))
def pos(t, interp="catmullrom", **k): return kf("position", t, interp=interp, **P(**k))
def scl(t, v): return kf("scale", t, v, v, v, interp="step")
def show(t, on): return scl(t, 1 if on else 0)

def animator(g, *keys):
    return g["uuid"], {"name": g["name"], "type": "bone", "keyframes": list(keys)}

def animation(name, length, loop, *animators):
    return {"uuid": str(uuid.uuid4()), "name": name, "loop": loop, "override": False,
            "length": length, "snapping": 60, "selected": False, "anim_time_update": "",
            "blend_weight": "", "start_delay": "", "loop_delay": "", "animators": dict(animators)}

shoot = animation("shoot", 0.4, "loop",
    animator(g_trigger,  # pulled back, held while the gun cycles, then released
             kf("rotation", 0), kf("rotation", 0.03, x=20), kf("rotation", 0.2, x=20),
             kf("rotation", 0.28), kf("rotation", 0.4)),
    animator(g_slide,  # fires once the trigger breaks: snaps back, then returns
             kf("position", 0), kf("position", 0.03), kf("position", 0.06, z=4), kf("position", 0.09, z=4),
             kf("position", 0.17), kf("position", 0.4)),
    # the bullet is gone when it fires; the slide pulls the case out (the spent case below);
    # a fresh round is in once the slide closes
    animator(g_chamber, show(0, True), show(0.03, False), show(0.16, True)),
    animator(g_spent,
             show(0, False), show(0.03, True), show(0.32, False),
             kf("position", 0), kf("position", 0.03), kf("position", 0.06, z=4, y=0.3),
             kf("position", 0.12, x=4, y=3.5, z=5), kf("position", 0.32, x=12, y=4, z=8),
             kf("rotation", 0), kf("rotation", 0.06), kf("rotation", 0.32, x=-200, y=90, z=-540)),
)
shoot["selected"] = True

# walking: figure-eight bob of the whole view model, a little roll with each step
walk = animation("walk", 0.8, "loop",
    animator(g_root,
             pos(0), pos(0.2, x=0.8, y=-0.6), pos(0.4), pos(0.6, x=-0.8, y=-0.6), pos(0.8),
             rot(0), rot(0.2, z=-1.5), rot(0.4), rot(0.6, z=1.5), rot(0.8)),
)

# sprinting: gun lowered and turned across the body, bouncing harder
SPR = dict(x=-25, y=30, z=15)
sprint = animation("sprint", 0.5, "loop",
    animator(g_root,
             pos(0, x=-2, y=-3, z=2), pos(0.125, x=-0.8, y=-4.4, z=2), pos(0.25, x=-2, y=-3, z=2),
             pos(0.375, x=-3.2, y=-4.4, z=2), pos(0.5, x=-2, y=-3, z=2),
             rot(0, **SPR), rot(0.125, x=-27, y=30, z=13), rot(0.25, **SPR), rot(0.375, x=-27, y=30, z=17), rot(0.5, **SPR)),
)

# aiming down the sights: the rear sight comes to the middle of the view
aim = animation("aim", 0.25, "hold",
    animator(g_root, pos(0), pos(0.25, x=-7, y=9.5, z=8), rot(0), rot(0.25)),
)

# reload from empty: slide locked back, empty chamber and magazine; drop it, the left hand
# brings a full one up into the grip, then racks the slide, which chambers a round
DROP = (0, -0.95, 0.31)          # down along the tilted grip (pistol frame)
def along(d): return dict(zip("xyz", [c * d for c in DROP]))
def neg(d): return {k: -v for k, v in d.items()}
G = (0, -10.8 + (GB + 4.5), 5.7)  # the left fist under the magazine's base plate (from FL)
TILT = dict(x=38, y=12, z=-28)
LIFT = dict(x=-4, y=15, z=2)
reload = animation("reload", 2.0, "once",
    animator(g_rarm,  # raised and brought in so the magazine change happens in view
             rot(0), rot(0.3, **TILT), rot(1.25, **TILT), rot(1.45, y=-6, z=-10), rot(1.75), rot(2.0),
             pos(0), pos(0.3, **LIFT), pos(1.25, **LIFT), pos(1.45, x=-1.5, y=4, z=3), pos(1.75), pos(2.0)),
    animator(g_mag,
             kf("position", 0), kf("position", 0.35), kf("position", 0.6, **along(24)),
             kf("position", 0.62, **along(30)), kf("position", 0.8, **along(30)),
             kf("position", 1.1, **along(2.5)), kf("position", 1.2), kf("position", 2.0)),
    animator(g_mag_mesh, show(0, True), show(0.6, False), show(0.8, True)),
    animator(g_larm,  # offsets from the magazine
             kf("position", 0), kf("position", 0.35),
             kf("position", 0.6, **neg(along(24))),            # stays put while the old one falls
             kf("position", 0.62, **neg(along(30))),
             kf("position", 0.8, **dict(zip("xyz", G))),        # reaches down, takes the new one
             kf("position", 1.2, **dict(zip("xyz", G))),        # ...and pushes it up into the grip
             kf("position", 1.32),
             kf("position", 1.45, x=0, y=8.5, z=5), kf("position", 1.55, x=0, y=8.5, z=9),  # racks the slide
             kf("position", 1.8), kf("position", 2.0)),
    animator(g_slide,  # locked back; pulled a touch further, then slams home
             kf("position", 0, z=4), kf("position", 1.45, z=4), kf("position", 1.55, z=4.4),
             kf("position", 1.6), kf("position", 2.0)),
    animator(g_chamber, show(0, False), show(1.6, True)),
    animator(g_spent, show(0, False)),
    animator(g_mag_rounds, show(0, False), show(0.62, True)),
)

# ---------- at the gun station ----------
# Taken apart (field stripped), lying on its side on the table: each part in its own window
# of time (the game plays each window backwards to put that part in). The frame stays; the
# parts end up laid beside it in the gun's side plane (y, z), where the table is.
STRIP = dict(magazine=(0.0, 0.5), slide=(0.5, 1.3), recoil_spring=(1.3, 1.8), barrel=(1.8, 2.4))
BARREL_OUT = [pos(1.8), pos(1.95, y=1.2, z=0.8), pos(2.4, y=-9, z=-2)]
BARREL_TURN = [rot(1.8), rot(1.95, x=-8), rot(2.4)]
strip = animation("strip", 2.4, "hold",
    animator(g_mag,  # out of the grip, then laid behind it
             pos(0), pos(0.08), pos(0.35, **along(14)), pos(0.5, y=-1, z=13)),
    animator(g_slide,  # pulled back a touch, run forward off the frame, laid above it
             pos(0.5), pos(0.62, z=1.2), pos(0.72, z=1.2), pos(1.0, z=-11), pos(1.3, y=5.5, z=-3)),
    animator(g_spring,  # its rod lifted out at the back, laid under the barrel
             pos(1.3), pos(1.42, y=0.8, z=1.0), pos(1.8, y=-12.5, z=-1.5),
             rot(1.3), rot(1.42, x=6), rot(1.8)),
    animator(g_barrel, *BARREL_OUT, *BARREL_TURN),
    animator(g_chamber, *BARREL_OUT, *BARREL_TURN),
)
# The attachments going on (the game plays them backwards to take one off).
fit_scope = animation("fit_scope", 0.5, "hold",
    animator(g_scope, pos(0, y=7), pos(0.3, y=0.5), pos(0.42), pos(0.5)))
fit_silencer = animation("fit_silencer", 0.7, "hold",
    animator(g_silencer,  # onto the barrel's thread, then screwed on
             kf("position", 0, **P(z=-7)), kf("position", 0.15, **P(z=-1.4)), kf("position", 0.6), kf("position", 0.7),
             kf("rotation", 0, **R(z=900)), kf("rotation", 0.15, **R(z=900)), kf("rotation", 0.6), kf("rotation", 0.7)))
fit_laser = animation("fit_laser", 0.5, "hold",
    animator(g_laser, pos(0, y=-3, z=-5), pos(0.2, z=-5), pos(0.45), pos(0.5)))
fit_flashlight = animation("fit_flashlight", 0.5, "hold",
    animator(g_flashlight, pos(0, y=-3, z=-5), pos(0.2, z=-5), pos(0.45), pos(0.5)))

# ---------- checks ----------
def check_coplanar():
    """Visible faces of two unrotated cubes lying in the same plane and overlapping flicker
    (z-fighting). Report every such pair."""
    boxes = [(e["name"], e["from"], e["to"]) for e in elements if not any(e["rotation"])]
    bad = []
    for i in range(len(boxes)):
        for j in range(i + 1, len(boxes)):
            (na, fa, ta), (nb, fb, tb) = boxes[i], boxes[j]
            for ax in range(3):
                o = [k for k in range(3) if k != ax]
                ov = all(min(ta[k], tb[k]) - max(fa[k], fb[k]) > 1e-3 for k in o)
                if not ov: continue
                for pa, sa in ((fa[ax], -1), (ta[ax], 1)):
                    for pb, sb in ((fb[ax], -1), (tb[ax], 1)):
                        if abs(pa - pb) < 1e-4 and sa == sb:
                            bad.append((na, nb, "xyz"[ax], pa))
    return bad

# ---------- texture: pack faces tallest first, paint, and point the UVs at them ----------
jobs.sort(key=lambda j: (-j[4], -j[3]))
x = y = row_h = 0
placed = []
for job in jobs:
    w, h = job[3] + 2, job[4] + 2          # 1 texel border copied from the edge
    if x + w > ATLAS_W:
        x, y, row_h = 0, y + row_h, 0
    placed.append((job, x, y))
    x += w
    row_h = max(row_h, h)
TEX_H = ((y + row_h + 15) // 16) * 16
img = [[(0, 0, 0, 0)] * ATLAS_W for _ in range(TEX_H)]
for (face_ref, face, mat, tw, th, painters, density), ax, ay in placed:
    DENSITY[0] = density
    px = paint_face(face, mat, tw, th, painters)
    DENSITY[0] = D
    for j in range(-1, th + 1):
        row = img[ay + 1 + j]
        src = px[min(max(j, 0), th - 1)]
        for i in range(-1, tw + 1):
            row[ax + 1 + i] = src[min(max(i, 0), tw - 1)] + (MAT_ALPHA.get(mat, 255),)
    face_ref["uv"] = [ax + 1, ay + 1, ax + 1 + tw, ay + 1 + th]

raw = b"".join(b"\x00" + b"".join(bytes(p) for p in row) for row in img)
def chunk(t, d): return struct.pack(">I", len(d)) + t + d + struct.pack(">I", zlib.crc32(t + d) & 0xffffffff)
png = (b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", ATLAS_W, TEX_H, 8, 6, 0, 0, 0))
       + chunk(b"IDAT", zlib.compress(raw, 9)) + chunk(b"IEND", b""))

model = {
    "meta": {"format_version": "4.10", "model_format": "free", "box_uv": False},
    "name": "pistol_viewmodel",
    "resolution": {"width": ATLAS_W, "height": TEX_H},
    "elements": elements,
    "outliner": [g_root],
    "textures": [{
        "name": "pistol.png", "id": "0", "uuid": str(uuid.uuid4()), "folder": "", "namespace": "",
        "width": ATLAS_W, "height": TEX_H, "uv_width": ATLAS_W, "uv_height": TEX_H,
        "particle": False, "render_mode": "default", "visible": True, "saved": False,
        "source": "data:image/png;base64," + base64.b64encode(png).decode(),
    }],
    "animations": [shoot, reload, walk, sprint, aim, strip, fit_scope, fit_silencer, fit_laser, fit_flashlight],
}
with open(os.path.join(HERE, "pistol.bbmodel"), "w") as f:
    json.dump(model, f, indent=1)
with open(os.path.join(HERE, "pistol.png"), "wb") as f:
    f.write(png)

bad = check_coplanar()
for b in bad:
    print("coplanar:", *b)
print(f"ok: {len(elements)} cubes, texture {ATLAS_W}x{TEX_H}, {len(bad)} coplanar pairs")
