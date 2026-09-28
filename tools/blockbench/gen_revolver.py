"""Generates revolver.bbmodel (+ revolver.png): a first-person revolver view model for Blockbench,
built the same way as the pistol (gen_pistol.py).

Run:  python tools/blockbench/gen_revolver.py

Model space: 1 unit = 1 Blockbench pixel, the first-person camera at (0, 0, 0) looking -Z
(the whole rig is moved there by OFF). Every face gets its own painted spot on one texture
atlas (D texels per unit). Round parts are built from boxes turned about their axis (the
cylinder is a 12-sided prism of six crossed boxes), rounded edges from 45-degree chamfer
strips; parts that touch are sunk slightly into each other so no two visible faces share a
plane (checked by `check_coplanar` at the end).

A six-shot double-action revolver: a swing-out cylinder on a crane (to the left), a full-length
underlug with the ejector rod's shroud, a ventilated rib, adjustable rear sight, hammer with a
knurled spur. No attachments.
"""
import base64, json, math, os, random, struct, uuid, zlib

random.seed(11)
D = 8                 # texels per model unit
K = D / 4             # painter scale (painters were tuned at 4 texels per unit)
ATLAS_W = 1024
HERE = os.path.dirname(os.path.abspath(__file__))

MATS = {
    "frame": (42, 44, 50), "strap": (46, 48, 54), "barrel": (48, 50, 56), "cyl": (56, 59, 66),
    "grip": (34, 35, 38), "metal": (112, 117, 124), "hammer": (84, 87, 94), "sight": (20, 20, 22),
    "bore": (12, 12, 13), "skin": (196, 140, 102), "sleeve": (52, 58, 70), "brass": (190, 150, 62),
    "copper": (178, 104, 66), "loader": (30, 31, 34), "lead": (118, 121, 128),
}
MAT_ALPHA = {}

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

# ---------- detail painters (u runs along a face, v down it, as seen from outside) ----------
# east face: u = 0 at the back (+z); west face: u = 0 at the front (-z);
# north face: u = 0 at +x; south face: u = 0 at -x; up face: u along +x, v = 0 at the front.

def lettering(u_from_front, v, n=10):
    """A line of engraved lettering, `u_from_front` units from the face's front end."""
    def f(px, w, h, face):
        if face not in ("east", "west"): return
        u = U(u_from_front) if face == "west" else w - U(u_from_front) - n * 5
        for k in range(n):
            glyph = random.getrandbits(15) | 0b100000000000001
            for gy in range(5):
                for gx in range(3):
                    if glyph >> (gy * 3 + gx) & 1:
                        rect(px, u + gx, U(v) + gy, u + gx + 1, U(v) + gy + 1, lambda c: shade(c, 34))
            u += 5
    return f

def screws(*spots):
    """Screw heads with a slot on both sides: (units from the front, units down)."""
    def f(px, w, h, face):
        if face not in ("east", "west"): return
        for uf, v in spots:
            u = U(uf) if face == "west" else w - U(uf)
            disc(px, u, U(v), U(0.26), lambda c: shade(c, -30))
            disc(px, u, U(v), U(0.2), (118, 122, 128))
            rect(px, u - U(0.18), U(v) - 1, u + U(0.18), U(v) + 1, (40, 42, 46))
    return f

def sight_dot(v=0.5, col=(238, 238, 228)):
    """A dot on a sight's back face (toward the eye), with a thin dark ring round it."""
    def f(px, w, h, face):
        if face == "south":
            cx, cy = w / 2, h * v
            disc(px, cx, cy, 0.2 * DENSITY[0], (10, 10, 11))
            disc(px, cx, cy, 0.15 * DENSITY[0], col)
    return f

def red_insert(px, w, h, face):
    """The front sight's red insert, seen from behind and from the sides."""
    if face == "south":
        rect(px, U(0.12), U(0.15), w - U(0.12), U(0.55), (206, 34, 26))
        rect(px, U(0.12), U(0.15), w - U(0.12), U(0.15) + 1, (250, 96, 80))
    elif face in ("east", "west"):
        rect(px, U(0.1), U(0.15), w - U(0.1), U(0.45), (176, 28, 22))

def stipple(px, w, h, face):
    if face not in ("east", "west", "south"): return
    b = U(0.25)
    for j in range(b, h - b):
        for i in range(b, w - b):
            r = random.random()
            if r < 0.33: px[j][i] = shade(px[j][i], -13)
            elif r < 0.52: px[j][i] = shade(px[j][i], 12)
    if face in ("east", "west") and w > U(2.5):  # moulded medallion
        cx, cy = w / 2, h * 0.22
        for j in range(h):
            for i in range(w):
                r = math.hypot(i + 0.5 - cx, j + 0.5 - cy)
                if abs(r - U(0.6)) < 0.7:
                    px[j][i] = shade(MATS["grip"], 30)
                elif r < U(0.3):
                    px[j][i] = shade(MATS["grip"], 18)

def finger_grooves(px, w, h, face):
    if face != "north": return
    for k in range(1, 4):
        v = int(h * k / 4)
        rect(px, 0, v - 1, w, v + 1, lambda c: shade(c, -24))
        rect(px, 0, v + 1, w, v + 2, lambda c: shade(c, 14))

def trigger_blade(px, w, h, face):
    if face == "north":
        rect(px, w // 2 - 1, 1, w // 2 + 1, h - 1, lambda c: shade(c, 40))

def dark(*faces):
    """Faces that look into a cavity: nearly black."""
    def f(px, w, h, face):
        if face in faces:
            rect(px, 0, 0, w, h, lambda c: shade(MATS["bore"], random.randint(-2, 2)))
    return f

def bore_face(r):
    """The front of a tube with a hole of radius `r` (units) in its middle."""
    def f(px, w, h, face):
        if face == "north":
            disc(px, w / 2, h / 2, r * DENSITY[0] + 0.8 * K, lambda c: shade(c, -25))
            disc(px, w / 2, h / 2, r * DENSITY[0], (8, 8, 9))
            # rifling: a few lighter lands inside the bore
            for k in range(6):
                a = k * math.pi / 3
                disc(px, w / 2 + math.cos(a) * r * DENSITY[0] * 0.8, h / 2 + math.sin(a) * r * DENSITY[0] * 0.8,
                     0.6 * K, (26, 26, 28))
    return f

def knurl(px, w, h, face):
    """Fine cross-hatching (the hammer spur, the latch)."""
    if face in ("up", "east", "west", "south"):
        for j in range(h):
            for i in range(w):
                if (i + j) % 3 == 0 or (i - j) % 3 == 0:
                    px[j][i] = shade(px[j][i], -26)

def latch_grooves(px, w, h, face):
    if face in ("east", "west"):
        for k in range(2, w - 1, 3):
            rect(px, k, 1, k + 1, h - 1, lambda c: shade(c, -30))

def rib_vents(px, w, h, face):
    """A ventilated rib: open slots along its sides, fine grooves across its top."""
    if face in ("east", "west"):
        for k in range(9):
            u = U(0.9 + k * 1.2)
            rect(px, u, U(0.18), u + U(0.7), h - U(0.2), (10, 10, 11))
            rect(px, u, h - U(0.2), u + U(0.7), h - U(0.2) + 1, lambda c: shade(c, 20))
    if face == "up":
        for k in range(2, h - 1, 2):
            rect(px, 0, k, w, k + 1, lambda c: shade(c, -20))

def strap_groove(px, w, h, face):
    """The top strap's sighting groove, along its top."""
    if face == "up":
        rect(px, w // 2 - U(0.18), 0, w // 2 + U(0.18), h, lambda c: shade(c, -22))
        for k in range(1, h, 2):
            rect(px, w // 2 - U(0.18), k, w // 2 + U(0.18), k + 1, lambda c: shade(c, -12))

def rod_tip(px, w, h, face):
    """The ejector rod's knurled head seen at the front of the underlug."""
    if face == "north":
        cx, cy = w / 2, h * 0.52
        disc(px, cx, cy, U(0.32), (14, 14, 15))
        disc(px, cx, cy, U(0.26), (120, 124, 130))
        disc(px, cx, cy, U(0.1), (80, 84, 90))

def case_head(px, w, h, face):
    if face != "south": return
    cx, cy = w / 2, h / 2
    for j in range(h):
        for i in range(w):
            r = math.hypot(i + 0.5 - cx, j + 0.5 - cy)
            if r < U(0.17): px[j][i] = (200, 200, 205)                     # primer
            elif r < U(0.2): px[j][i] = shade(px[j][i], -20)
            elif abs(r - U(0.34)) < 0.6: px[j][i] = shade(px[j][i], 26)     # headstamp ring
            elif r > min(w, h) / 2 - 1: px[j][i] = shade(px[j][i], -30)   # rim edge

def case_shine(px, w, h, face):
    if face in ("east", "west", "up"):
        rect(px, 0, 1, w, 1 + K, lambda c: shade(c, 40))

def bullet_tip(px, w, h, face):
    if face == "north":
        disc(px, w / 2, h / 2, U(0.14), lambda c: shade(c, 40))
    elif face in ("east", "west", "up", "down"):
        for i in range(w):
            t = i / max(1, w - 1)
            k = 1 - t if face == "east" else t
            rect(px, i, 0, i + 1, h, lambda c, k=k: shade(c, int(-26 * (1 - k))))

def loader_knob(px, w, h, face):
    if face in ("east", "west", "up", "down"):
        for k in range(1, w if face in ("east", "west") else h, 2):
            if face in ("east", "west"): rect(px, k, 0, k + 1, h, lambda c: shade(c, -20))
            else: rect(px, 0, k, w, k + 1, lambda c: shade(c, -20))
    if face == "south":
        disc(px, w / 2, h / 2, min(w, h) * 0.3, lambda c: shade(c, 16))

# ---------- model building ----------
OFF = (7, -24, -34)   # the rig sits at the hip, down-right of the camera
# The gun is built 1.1 times as big as its numbers here, about the grip's pivot (GP below):
# against the pistol it is then as big as a Ruger GP100 4.2" is against a Glock 17 (241 x
# 145 x 40 mm against 202 x 139 x 34). The arms are not.
SCALE = 1.1
PIVOT = (0, 8, 2.5)
ARMS = [False]        # building an arm: not scaled
def sp(p):
    """A point of the gun, scaled about the pivot."""
    return p if ARMS[0] else [c + SCALE * (a - c) for a, c in zip(p, PIVOT)]
def sh(p): return [round(a + o, 4) for a, o in zip(sp(p), OFF)]

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

def tube(name, axis, r, z0, z1, mat, painters=(), front=(), density=None, one_end=False):
    """A round tube along z (octagonal): two crossed boxes and four 45-degree corner strips.
    `front` paints the front (north) face of the widest box. With `one_end`, only the widest
    box has its ends drawn, the others set further in (small parts seen close up, whose
    stacked ends would flicker)."""
    x, y = axis
    n = r * 0.414
    d = density or D
    sides = ("east", "west", "up", "down") if one_end else None
    inset = 0.02 if one_end else 0.003
    parts = [
        cube(name + "_h", [x - r, y - n, z0], [x + r, y + n, z1], mat, tuple(painters) + tuple(front), density=d),
        cube(name + "_v", [x - n, y - r, z0 + inset], [x + n, y + r, z1 - inset], mat, painters, density=d, only=sides),
    ]
    for i, (sx, sy) in enumerate(((1, 1), (1, -1), (-1, 1), (-1, -1))):
        c = chamfer(f"{name}_c{i}", "z", (x + sx * n, y + sy * n), r - n, (z0 + 2 * inset, z1 - 2 * inset), mat, painters)
        if one_end:
            for f in ("north", "south"):
                e = next(e for e in elements if e["uuid"] == c)
                e["faces"][f]["texture"] = None      # (in place: its paint job is dropped too)
                e["faces"][f]["uv"] = [0, 0, 0, 0]
        parts.append(c)
    return parts

def group(name, children, origin=(0, 0, 0), rot=(0, 0, 0), visible=True):
    return {"name": name, "uuid": str(uuid.uuid4()), "origin": sh(origin), "rotation": list(rot),
            "isOpen": True, "visibility": visible, "export": True, "children": children}

# ---- the layout ----
CY = 10.15                  # cylinder axis height (x = 0)
CA = 1.95                   # cylinder: distance to its flats (12 of them)
CH = 1.2                    # chambers: distance from the axis
CHR = 0.45                  # chamber radius (the rim, 0.5 at real size, covers it)
BORE = (0.0, CY + CH)       # the top chamber lines up with the barrel
CZ = (-3.05, 1.2)           # cylinder body, front and back
BEVEL = (-3.2, -3.04)       # the narrower ring at its front
CHAMBERS = [math.radians(90 + 60 * k) for k in range(6)]
GP, GR = [0, 8, 2.5], (-22, 0, 0)     # grip pivot and tilt (as the pistol's)
SIGHT = 13.9                          # height of the sight line
GB = 0.0                              # the grip's bottom (shorter than the pistol's)

# ---- the cylinder: a 12-sided prism of six boxes turned 30 degrees apart ----
def cyl_end(theta, a, hy, front):
    """Paints a cylinder box's front or back: where each texel is on the cylinder's end
    (turned back by the box's angle), with the chambers, their bullets or their bores."""
    ct, st = math.cos(theta), math.sin(theta)
    def f(px, w, h, face):
        if face not in ("north", "south"): return
        if (face == "north") != front: return
        for j in range(h):
            for i in range(w):
                # the same steel all over the end (the boxes' ends lie one behind another)
                px[j][i] = shade(MATS["cyl"], -4 + random.randint(-2, 2))
        for j in range(h):
            for i in range(w):
                lx = (a - (i + 0.5) * 2 * a / w) if face == "north" else (-a + (i + 0.5) * 2 * a / w)
                ly = hy - (j + 0.5) * 2 * hy / h
                X, Y = lx * ct - ly * st, lx * st + ly * ct
                rc = math.hypot(X, Y)
                for ph in CHAMBERS:
                    r = math.hypot(X - CH * math.cos(ph), Y - CH * math.sin(ph))
                    if r < CHR:
                        if front and r < 0.34:     # the bullet's lead nose, down in the chamber
                            px[j][i] = shade(MATS["lead"], int(-40 * r / 0.34) + (24 if r < 0.2 else 0))
                        else:
                            px[j][i] = shade(MATS["bore"], random.randint(-2, 2))
                    elif r < CHR + 0.1:
                        px[j][i] = shade(px[j][i], -22 if front else 22)   # the chamber's mouth
                if front and rc < 0.3:                # the ejector rod's hole
                    px[j][i] = (14, 14, 15)
                if not front and rc < 0.62:           # the ejector star
                    px[j][i] = shade(px[j][i], 16)
                if rc > a * 0.985:
                    px[j][i] = shade(px[j][i], 20)
    return f

def flute(px, w, h, face):
    """A flute along the cylinder between two chambers (east face: u = 0 at the back)."""
    if face not in ("east", "west"): return
    z_front, z_back = 0.75, 1.25                   # units kept plain at each end
    u0, u1 = (U(z_front), w - U(z_back)) if face == "west" else (U(z_back), w - U(z_front))
    for j in range(h):
        t = (j + 0.5) / h
        if 0.16 < t < 0.84:
            depth = 1 - abs(t - 0.5) / 0.34
            rect(px, u0, j, u1, j + 1, lambda c, d=depth: shade(c, int(-14 - 26 * d)))
            # rounded ends
            rect(px, u0 - 1, j, u0, j + 1, lambda c: shade(c, -18))
            rect(px, u1, j, u1 + 1, j + 1, lambda c: shade(c, -18))
    rect(px, u0, int(h * 0.16), u1, int(h * 0.16) + 1, lambda c: shade(c, 26))   # lit upper lip

def stop_notch(px, w, h, face):
    """The cylinder stop's notch near the back, over each chamber, and the turn line."""
    if face not in ("east", "west"): return
    zb = 0.5                                        # notch centre from the back
    u = U(zb) if face == "east" else w - U(zb)
    rect(px, u - U(0.22), int(h * 0.28), u + U(0.22), int(h * 0.72), (16, 16, 18))
    rect(px, u - U(0.22), int(h * 0.72), u + U(0.22), int(h * 0.72) + 1, lambda c: shade(c, 24))
    # the faint ring the cylinder stop polishes round it
    lu = U(0.9) if face == "east" else w - U(0.9)
    rect(px, lu, 0, lu + 1, h, lambda c: shade(c, 14))

def cylinder_prism(name, a, z0, z1, painters_flat, painters_side, ends, density, step=0.03):
    """Six boxes, each giving two of the 12 flats; box k turned k*30 degrees. Their ends lie
    `step` apart, one behind another (far enough not to flicker)."""
    hy = a * math.tan(math.radians(15)) + 0.004
    parts = []
    for k in range(6):
        th = k * 30.0
        side = painters_flat if k % 2 == 0 else painters_side
        # ends: the box's own front/back painted as the cylinder's end would be there
        e = []
        if ends:
            e = [cyl_end(math.radians(th), a, hy, True), cyl_end(math.radians(th), a, hy, False)]
        dz = step * k
        parts.append(cube(f"{name}_{k}", [-a, CY - hy, z0 + dz], [a, CY + hy, z1 - dz], "cyl",
                          tuple(side) + tuple(e), [0, CY, (z0 + z1) / 2], (0, 0, th), density=density,
                          only=("east", "west", "north", "south")))
    return parts

cylinder_body = (
    cylinder_prism("cylinder", CA, CZ[0], CZ[1], [flute], [stop_notch], True, 2 * D)
    + cylinder_prism("cylinder_bevel", CA - 0.17, BEVEL[0], BEVEL[1] + 0.02, [], [], True, 2 * D, step=0.014)
)
# the ejector rod runs forward from the cylinder, inside the underlug when it is shut
ejector_rod = [
    cube("ejector_rod", [-0.22, CY - 0.22, -8.0], [0.22, CY + 0.22, BEVEL[0] + 0.02], "metal"),
    cube("ejector_rod_head", [-0.3, CY - 0.3, -8.6], [0.3, CY + 0.3, -7.98], "metal", [knurl], density=2 * D),
]

def dent(px, w, h, face):
    """A fired primer: the firing pin's dent in it."""
    if face == "south":
        disc(px, w / 2, h / 2, min(w, h) * 0.5, (70, 66, 60))
        disc(px, w / 2, h / 2, min(w, h) * 0.28, (30, 28, 26))

def swc_nose(px, w, h, face):
    """A semi-wadcutter's flat nose (the meplat) with the lube-dark edge round it."""
    if face == "north":
        disc(px, w / 2, h / 2, min(w, h) * 0.5 - 0.5, lambda c: shade(c, -14))
        disc(px, w / 2, h / 2, min(w, h) * 0.34, lambda c: shade(c, 22))
    elif face in ("east", "west", "up", "down"):
        rect(px, 0, 0, w, h, lambda c: shade(c, random.randint(-6, 6)))

def round_cap(inner=0.0):
    """A round's end cut out round from its square: the rim's head (with the headstamp and
    the primer, `case_head`), or its front face, a ring (`inner`: the part inside, taken up
    by the case, left out too). Outside the circle clear."""
    def f(px, w, h, face):
        if face not in ("south", "north"): return
        if face == "south":
            case_head(px, w, h, face)
        cx, cy, r = w / 2, h / 2, min(w, h) / 2
        for j in range(h):
            for i in range(w):
                d = math.hypot(i + 0.5 - cx, j + 0.5 - cy)
                if d > r or d < inner * r:
                    px[j][i] = px[j][i][:3] + (0,)
    return f

def rim(name, x, y, r, z0, z1):
    """A round's rim: the octagonal sides of `tube` without their ends, and each end one flat
    square cut round, so no two ends lie one behind another."""
    parts = tube(name, (x, y), r, z0, z1, "brass", [], density=2 * D, one_end=True)
    h = next(e for e in elements if e["uuid"] == parts[0])
    for f in ("north", "south"):
        h["faces"][f]["texture"] = None
        h["faces"][f]["uv"] = [0, 0, 0, 0]
    parts.append(cube(name + "_cap", [x - r, y - r, z1 - 0.001], [x + r, y + r, z1], "brass", [round_cap()],
                      density=4 * D, only=("south",)))
    parts.append(cube(name + "_front", [x - r, y - r, z0], [x + r, y + r, z0 + 0.001], "brass", [round_cap(0.8)],
                      density=4 * D, only=("north",)))
    return parts

def cartridge(name, x, y, z_back, with_dent=True):
    """A .357 Magnum round with its head (rim) at `z_back`, pointing forward (-z): the long
    rimmed case (a fired one is only this), its lead semi-wadcutter bullet (a driving band,
    then the narrower flat nose), and the dent a fired primer has; as three lists of cubes
    (case, bullet, dent). Built smaller by the gun's scale, so it comes out real size (as the
    pistol's 9 mm): 40 mm long, a 33 mm case, 9.6 mm across, the rim 11.2 mm."""
    f = 1 / SCALE
    def box(n, r, z0, z1, mat, painters, **k):
        return cube(n, [x - r * f, y - r * f, z_back + z0 * f], [x + r * f, y + r * f, z_back + z1 * f], mat, painters, **k)
    case = [box(f"{name}_case", 0.43, -2.96, 0.01, "brass", [case_shine])]
    case += rim(f"{name}_rim", x, y, 0.5 * f, z_back, z_back + 0.14 * f)
    bullet = [
        box(f"{name}_bullet", 0.41, -3.2, -2.95, "lead", [], density=2 * D),
        box(f"{name}_nose", 0.31, -3.62, -3.19, "lead", [swc_nose], density=2 * D),
    ]
    dented = [box(f"{name}_dent", 0.13, 0.14, 0.19, "bore", [dent], density=4 * D, only=("south",))] if with_dent else []
    return case, bullet, dented

def chamber_pos(k):
    ph = CHAMBERS[k]
    return CH * math.cos(ph), CY + CH * math.sin(ph)

RIM_Z = CZ[1] - 0.06          # where a round's head sits in its chamber
def chamber_group(k):
    """What is in chamber k: a round (its case and rim; its bullet while it is live; the dent
    once it is fired). The game shows each as the chamber is."""
    x, y = chamber_pos(k)
    case, bullet, dented = cartridge(f"round{k}", x, y, RIM_Z)
    return group(f"chamber{k}", case + [
        group(f"bullet{k}", bullet, (x, y, RIM_Z - 3.6)),
        group(f"dent{k}", dented, (x, y, RIM_Z + 0.15)),
    ], (x, y, RIM_Z - 1.5))

# a speedloader: its body behind the rims, the knob to turn, and the rounds it holds (as many
# as it has), each lined up with a chamber when it is pushed on
LOADER_Z = CZ[1] + 0.13
def loader_round(k):
    x, y = chamber_pos(k)
    case, bullet, _ = cartridge(f"loader_round{k}", x, y, RIM_Z, with_dent=False)
    return group(f"loader_round{k}", case + bullet, (x, y, RIM_Z - 1.5))
speedloader = (
    tube("loader_body", (0, CY), 1.62, LOADER_Z, LOADER_Z + 0.9, "loader", [], density=D)
    + tube("loader_knob", (0, CY), 0.6, LOADER_Z + 0.88, LOADER_Z + 2.2, "loader", [loader_knob], density=2 * D)
)

# the round the left hand pushes into a chamber when loading one at a time: where chamber 0
# is at rest (the one under the hammer; the cylinder turns the next empty one there)
LOOSE = chamber_pos(0)
loose_case, loose_bullet, _ = cartridge("loose_round", LOOSE[0], LOOSE[1], RIM_Z, with_dent=False)

# the mainspring inside the grip frame, pushing the hammer: a coil on its strut (tilted with
# the grip)
def coils(px, w, h, face):
    if face in ("east", "west", "north", "south"):
        for k in range(0, h, max(2, U(0.45))):
            rect(px, 0, k, w, k + 1, lambda c: shade(c, -60))


# ---- frame ----
FZ0 = -4.7                  # its front, where the barrel screws in
frame = [
    # under the cylinder, down to the trigger guard
    cube("frame_lower", [-1.3, 7.5, FZ0], [1.3, 8.15, 3.8], "frame", [screws((9.6, 0.33))]),
    # the front of the window: round the barrel's shank, above the crane
    cube("frame_front", [-1.25, 9.25, FZ0], [1.25, 12.3, -3.32], "frame"),
    # behind the cylinder: the recoil shield
    cube("recoil_shield", [-1.25, 8.12, CZ[1] + 0.1], [1.25, 12.25, 2.69], "frame", [screws((1.3, 2.6)), dark("north")]),
    # the cheeks either side of the hammer, their top rear corner rounded
    cube("hammer_cheek_l", [-1.24, 8.13, 2.65], [-0.47, 11.5, 3.9], "frame"),
    cube("hammer_cheek_r", [0.47, 8.13, 2.65], [1.24, 11.5, 3.9], "frame"),
    cube("hammer_cheek_l_top", [-1.23, 11.49, 2.66], [-0.48, 12.2, 3.2], "frame"),
    cube("hammer_cheek_r_top", [0.48, 11.49, 2.66], [1.23, 12.2, 3.2], "frame"),
    chamfer("hammer_cheek_l_round", "x", (11.5, 3.2), 0.7, (-1.235, -0.475), "frame"),
    chamfer("hammer_cheek_r_round", "x", (11.5, 3.2), 0.7, (0.475, 1.235), "frame"),
    cube("hammer_slot_floor", [-0.48, 8.14, 2.66], [0.48, 9.0, 3.89], "frame", [dark("up")]),
    # the grip frame rising behind the hammer
    cube("grip_frame", [-1.2, 7.45, 3.85], [1.2, 9.8, 5.2], "frame"),
    cube("grip_frame_top", [-1.19, 9.79, 3.86], [1.19, 10.4, 4.6], "frame"),
    chamfer("grip_frame_round", "x", (9.8, 4.6), 0.6, (-1.18, 1.18), "frame"),
    # the top strap over the cylinder, its deck and the chamfers rounding it into the sides
    cube("top_strap", [-1.2, 12.2, FZ0 + 0.01], [1.2, 12.6, 2.7], "strap"),
    cube("top_strap_deck", [-0.8, 12.55, FZ0 + 0.02], [0.8, 13.0, 2.3], "strap", [strap_groove]),
    chamfer("top_strap_chamfer_l", "z", (-0.8, 12.6), 0.4, (FZ0 + 0.03, 2.3), "strap"),
    chamfer("top_strap_chamfer_r", "z", (0.8, 12.6), 0.4, (FZ0 + 0.03, 2.3), "strap"),
    chamfer("top_strap_chamfer_rear", "x", (12.6, 2.3), 0.4, (-0.8, 0.8), "strap"),
    # the cylinder latch on the left, pushed forward to swing the cylinder out
    cube("cylinder_latch", [-1.52, 10.35, 1.45], [-1.24, 11.15, 2.95], "metal", [latch_grooves], density=2 * D),
    cube("latch_plate", [-1.33, 10.1, 1.35], [-1.23, 11.4, 3.3], "frame"),
    # the pin the hammer turns on, and the trigger's
    cube("hammer_pin_l", [-1.3, 9.35, 3.05], [-1.23, 9.85, 3.55], "metal", density=2 * D),
    cube("hammer_pin_r", [1.23, 9.35, 3.05], [1.3, 9.85, 3.55], "metal", density=2 * D),
    cube("trigger_pin_l", [-1.36, 7.68, -2.45], [-1.29, 8.0, -2.13], "metal", density=2 * D),
    cube("trigger_pin_r", [1.29, 7.68, -2.45], [1.36, 8.0, -2.13], "metal", density=2 * D),
]

# the rear sight: an adjustable block on the strap, a square notch, a white dot either side
rear_sight = [
    cube("rear_sight_base", [-0.98, 12.98, 0.9], [0.98, 13.32, 2.5], "sight", density=2 * D),
    cube("rear_sight_l", [-0.97, 13.3, 1.5], [-0.3, SIGHT, 2.49], "sight", [sight_dot(0.5)], density=2 * D),
    cube("rear_sight_r", [0.3, 13.3, 1.5], [0.97, SIGHT, 2.49], "sight", [sight_dot(0.5)], density=2 * D),
    cube("rear_sight_l_step", [-0.96, 13.31, 0.91], [-0.31, 13.58, 1.51], "sight", density=2 * D),
    cube("rear_sight_r_step", [0.31, 13.31, 0.91], [0.96, 13.58, 1.51], "sight", density=2 * D),
    cube("rear_sight_screw", [1.0, 13.0, 1.55], [1.08, 13.28, 1.85], "metal", density=2 * D),
]

# ---- the barrel: round, with a ventilated rib on top and a full-length underlug ----
BZ = (-16.0, FZ0 + 0.05)
barrel = (
    tube("barrel", BORE, 0.85, BZ[0], BZ[1], "barrel", front=[bore_face(0.36)])
    + [
        cube("rib", [-0.55, 12.05, BZ[0]], [0.55, 12.9, BZ[1] + 0.02], "barrel", [rib_vents]),
        cube("lug", [-0.78, 9.45, BZ[0] + 0.2], [0.78, 11.25, BZ[1] + 0.01], "barrel",
             [lettering(1.3, 0.45, 9), rod_tip]),
        cube("lug_bottom", [-0.38, 9.05, BZ[0] + 0.21], [0.38, 9.5, BZ[1]], "barrel"),
        chamfer("lug_round_l", "z", (-0.38, 9.45), 0.4, (BZ[0] + 0.22, BZ[1] - 0.01), "barrel"),
        chamfer("lug_round_r", "z", (0.38, 9.45), 0.4, (BZ[0] + 0.22, BZ[1] - 0.01), "barrel"),
        # the front sight: a ramped blade pinned into the rib, a red insert at its back
        cube("front_sight", [-0.26, 12.85, BZ[0] + 0.35], [0.26, SIGHT, BZ[0] + 1.1], "sight", [red_insert], density=2 * D),
        cube("front_sight_ramp", [-0.25, 12.86, BZ[0] + 0.02], [0.25, 13.45, BZ[0] + 0.36], "sight", density=2 * D),
    ]
)

# ---- trigger guard and trigger (as the pistol's) ----
def guard_arc():
    R, cy, cz = 1.0, 5.3, -3.7
    parts = [
        cube("guard_front", [-0.55, cy, cz - R - 0.3], [0.55, 7.55, cz - R + 0.3], "frame"),
        cube("guard_bottom", [-0.56, cy - R - 0.3, cz], [0.56, cy - R + 0.3, 1.6], "frame"),
        cube("guard_rear", [-0.54, cy - R - 0.2, 1.3], [0.54, 7.55, 1.9], "frame"),
    ]
    for i, phi in enumerate((11.25, 33.75, 56.25, 78.75)):
        p = math.radians(phi)
        y, z = cy - R * math.sin(p), cz - R * math.cos(p)
        hw = 0.535 if i % 2 else 0.545
        parts.append(cube(f"guard_arc_{i}", [-hw, y - 0.25, z - 0.3], [hw, y + 0.25, z + 0.3], "frame",
                          origin=[0, y, z], rot=(-phi, 0, 0)))
    return parts
frame += guard_arc()

TJ = (7.5 - 1.15 * math.cos(math.radians(5)), -2.2 - 1.15 * math.sin(math.radians(5)))
trigger = [
    cube("trigger_upper", [-0.34, 6.35, -2.6], [0.34, 7.5, -1.8], "metal", [trigger_blade],
         origin=[0, 7.5, -2.2], rot=(5, 0, 0)),
    cube("trigger_lower", [-0.33, TJ[0] - 1.25, TJ[1] - 0.35], [0.33, TJ[0] + 0.1, TJ[1] + 0.35], "metal",
         [trigger_blade], origin=[0, TJ[0], TJ[1]], rot=(30, 0, 0)),
]

# ---- hammer: turns back on its pin when cocked; a knurled spur to thumb it ----
HP = (0, 9.6, 3.3)
hammer = [
    cube("hammer_body", [-0.45, 9.1, 2.8], [0.45, 12.35, 3.75], "hammer"),
    # the spur reaching back over the grip frame, knurled on top, its end rounded down
    cube("hammer_spur", [-0.5, 11.85, 3.7], [0.5, 12.55, 4.7], "hammer", [knurl]),
    chamfer("hammer_spur_round", "x", (12.2, 4.7), 0.35, (-0.49, 0.49), "hammer"),
    cube("hammer_nose", [-0.14, 11.2, 2.55], [0.14, 11.5, 2.81], "metal", density=2 * D),
]

# ---- grip: the pistol's stepped profile, rounded at the butt ----
grip = [
    cube("grip_core", [-1.45, GB, 0.75], [1.45, 8, 4.25], "grip", [stipple], GP, GR),
    cube("grip_mid", [-1.28, GB + 0.01, 0.55], [1.28, 7.99, 4.45], "grip", [stipple], GP, GR),
    cube("grip_front", [-1.0, GB + 0.02, 0.4], [1.0, 7.98, 4.6], "grip", [stipple, finger_grooves], GP, GR),
    cube("grip_backstrap", [-1.05, GB + 0.3, 4.55], [1.05, 7.6, 5.05], "grip", [stipple], GP, GR),
    cube("grip_backstrap_round", [-0.75, GB + 0.31, 5.0], [0.75, 7.59, 5.22], "grip", [stipple], GP, GR),
    # the butt, stepped in so it reads round
    cube("grip_butt", [-1.32, GB - 0.35, 0.45], [1.32, GB + 0.05, 5.0], "grip", [], GP, GR),
    cube("grip_butt_bottom", [-1.05, GB - 0.6, 0.75], [1.05, GB - 0.3, 4.7], "grip", [], GP, GR),
]

mainspring = [
    cube("mainspring_coil", [-0.38, 1.6, 4.05], [0.38, 7.2, 4.8], "metal", [coils], GP, GR),
    cube("mainspring_strut", [-0.16, 1.2, 4.24], [0.16, 8.1, 4.6], "hammer", [], GP, GR),
    cube("mainspring_seat", [-0.5, 1.2, 3.95], [0.5, 1.6, 4.9], "hammer", [], GP, GR),
]

# ---- arms: Minecraft proportions at twice the size, the fist around the grip ----
FR = (0, 4.5, 3.6)       # right fist centre
FL = (-4.2, 2.6, 2.4)    # left fist centre, cupping under the right hand
def arm(side, f, rot):
    x, y, z = f
    ARMS[0] = True
    g = group(side + "_arm_mesh", [
        cube(side + "_hand", [x - 3.5, y - 3.5, z - 3.5], [x + 3.5, y + 3.5, z + 13], "skin"),
        cube(side + "_sleeve", [x - 3.8, y - 3.8, z + 13], [x + 3.8, y + 3.8, z + 26], "sleeve"),
    ], f, rot)
    ARMS[0] = False
    return g

# ---- hierarchy ----
CRANE = (-0.95, 8.75, -1.0)   # the crane's hinge, along z under the cylinder's left side
g_rounds = group("cylinder_rounds", [chamber_group(k) for k in range(6)], (0, CY, 0))
g_loader = group("speedloader", speedloader + [loader_round(k) for k in range(6)], (0, CY, LOADER_Z + 1), visible=True)
g_cylinder = group("cylinder", cylinder_body + ejector_rod + [g_rounds, g_loader], (0, CY, 0))
g_loose_bullet = group("loose_bullet", loose_bullet, (LOOSE[0], LOOSE[1], RIM_Z - 3.6))
g_loose = group("loose_round", loose_case + [g_loose_bullet], (LOOSE[0], LOOSE[1], RIM_Z - 1.5))
g_crane = group("crane", [
    cube("crane_yoke", [-1.24, 8.1, FZ0 + 0.01], [1.24, 9.26, -3.33], "frame", [dark("south")]),
    cube("crane_arm", [-0.9, 8.4, -3.34], [0.3, 9.3, BEVEL[0] + 0.01], "frame"),
    g_cylinder, g_loose], CRANE)
g_mainspring = group("mainspring", mainspring, (0, 4.5, 4.4))
g_hammer = group("hammer", hammer, HP)
g_trigger = group("trigger", trigger, (0, 7.5, -2.2))
g_rear = group("rear_sight", rear_sight, (0, 13.5, 1.7))
g_frame = group("frame", frame + [g_hammer, g_trigger, g_rear], (0, 8, 0))
g_barrel = group("barrel", barrel, (0, BORE[1], -9))
g_grip = group("grip", grip, GP)
g_larm_mesh = arm("left", FL, (15, -35, 0))
ARMS[0] = True
g_larm = group("left_arm", [g_larm_mesh], FL)
ARMS[0] = False
g_revolver = group("revolver", [g_frame, g_crane, g_barrel, g_grip, g_mainspring, g_larm], GP)
g_rarm_mesh = arm("right", FR, (15, 20, 0))
ARMS[0] = True
g_rarm = group("right_arm", [g_rarm_mesh, g_revolver], FR)
g_root = group("viewmodel", [g_rarm], FR)
ARMS[0] = False

# the speedloader only shows while reloading, the loose round while loading one
g_loader["visibility"] = False
g_loose["visibility"] = False

# ---------- animations ----------
def kf(channel, t, x=0, y=0, z=0, interp="linear"):
    return {"channel": channel, "data_points": [{"x": x, "y": y, "z": z}], "uuid": str(uuid.uuid4()),
            "time": t, "color": -1, "interpolation": interp}

# Blockbench keyframes: rotation x/y and position x are given mirrored (Bedrock convention)
def R(x=0, y=0, z=0): return dict(x=-x, y=-y, z=z)
def P(x=0, y=0, z=0): return dict(x=-x, y=y, z=z)
def rot(t, interp="catmullrom", **k): return kf("rotation", t, interp=interp, **R(**k))
def pos(t, interp="catmullrom", **k): return kf("position", t, interp=interp, **P(**k))
def lrot(t, **k): return rot(t, "linear", **k)
def lpos(t, **k): return pos(t, "linear", **k)
def scl(t, v): return kf("scale", t, v, v, v, interp="step")
def show(t, on): return scl(t, 1 if on else 0)

def animator(g, *keys):
    return g["uuid"], {"name": g["name"], "type": "bone", "keyframes": list(keys)}

def animation(name, length, loop, *animators):
    return {"uuid": str(uuid.uuid4()), "name": name, "loop": loop, "override": False,
            "length": length, "snapping": 60, "selected": False, "anim_time_update": "",
            "blend_weight": "", "start_delay": "", "loop_delay": "", "animators": dict(animators)}

# Double action: the trigger pull cocks the hammer and turns the cylinder one chamber
# (a sixth of a turn, anticlockwise seen from behind), then the hammer falls. The cylinder
# looks the same after a sixth of a turn, so it jumps back to 0 unseen.
shoot = animation("shoot", 0.4, "loop",
    animator(g_trigger,
             # pulled back: its lower end toward the grip (+Z), turning about its pin
             lrot(0), lrot(0.08, x=-24), lrot(0.2, x=-24), lrot(0.3), lrot(0.4)),
    animator(g_hammer,
             lrot(0), lrot(0.08, x=38), lrot(0.095), lrot(0.4)),
    animator(g_cylinder,
             lrot(0), lrot(0.075, z=60), kf("rotation", 0.4, **R(z=60), interp="step")),
    animator(g_revolver,  # the kick: muzzle up, back into the hand
             rot(0), rot(0.095), rot(0.14, x=9), rot(0.3), rot(0.4),
             pos(0), pos(0.095), pos(0.14, z=1.2), pos(0.3), pos(0.4)),
)
shoot["selected"] = True

walk = animation("walk", 0.8, "loop",
    animator(g_root,
             pos(0), pos(0.2, x=0.8, y=-0.6), pos(0.4), pos(0.6, x=-0.8, y=-0.6), pos(0.8),
             rot(0), rot(0.2, z=-1.5), rot(0.4), rot(0.6, z=1.5), rot(0.8)),
)

SPR = dict(x=-25, y=30, z=15)
sprint = animation("sprint", 0.5, "loop",
    animator(g_root,
             pos(0, x=-2, y=-3, z=2), pos(0.125, x=-0.8, y=-4.4, z=2), pos(0.25, x=-2, y=-3, z=2),
             pos(0.375, x=-3.2, y=-4.4, z=2), pos(0.5, x=-2, y=-3, z=2),
             rot(0, **SPR), rot(0.125, x=-27, y=30, z=13), rot(0.25, **SPR), rot(0.375, x=-27, y=30, z=17), rot(0.5, **SPR)),
)

# aiming down the sights: the rear sight's notch comes to the middle of the view
aim = animation("aim", 0.25, "hold",
    animator(g_root, pos(0), pos(0.25, x=-7, y=24 - (PIVOT[1] + SCALE * (SIGHT - PIVOT[1])) - 0.2, z=12), rot(0), rot(0.25)),
)

# Reload, in three parts the game plays as the reload goes (see `revolver_view`):
# - 0 .. 0.95 open: the gun turned up and in, the cylinder swung out to the left on its
#   crane, the ejector pushed (the cases come out at 0.65: the game throws them from there),
#   the left hand back under the cylinder (HOLD);
# - 0.95 .. 1.62 speedloader: the left hand brings it to the chambers, turns its knob (the
#   rounds are let go at 1.4) and takes it away, back to HOLD;
# - 1.62 .. 2.3 close: the cylinder swung shut, the gun brought back.
# Loading one round at a time plays "load_round" instead of the speedloader, held at 0.95.
TILT = dict(x=30, y=10, z=-35)
LIFT = dict(x=-6, y=12, z=2)
SWING = 80
OUT = 1.35                        # how far the ejector pushes the cases back
# the left fist behind the swung-out cylinder, from FL (found by eye in Blockbench)
HAND_AT = dict(x=2.2, y=2.0, z=2.5)
HOLD = dict(x=-1.0, y=-1.0, z=5.0)
reload = animation("reload", 2.3, "once",
    animator(g_rarm,
             rot(0), lrot(0.3, **TILT), lrot(1.95, **TILT), rot(2.3),
             pos(0), lpos(0.3, **LIFT), lpos(1.95, **LIFT), pos(2.3)),
    animator(g_crane,  # swung out by the left thumb, held out, shut again with a flick
             lrot(0), lrot(0.3), lrot(0.48, z=SWING), lrot(1.65, z=SWING), lrot(1.8), lrot(2.3)),
    animator(g_rounds,  # pushed back out of the chambers by the ejector
             lpos(0), lpos(0.55), lpos(0.65, z=OUT), lpos(0.66),
             show(0, True), show(0.655, False), show(0.7, True)),
    animator(g_loader,
             show(0, False), show(1.0, True), show(1.62, False),
             lpos(0), lpos(1.0, z=7), lpos(1.3, z=1.2), lpos(1.4), lpos(1.47), lpos(1.62, z=6),
             lrot(0), lrot(1.4), lrot(1.47, z=-40), lrot(1.62, z=-40)),
    animator(g_larm,  # thumbs the latch, pushes the ejector rod, holds the cylinder, fetches the loader
             pos(0), pos(0.3, x=0.5, y=2.5, z=1), pos(0.45, x=0.5, y=2.5, z=1),
             pos(0.58, x=2.0, y=3.9, z=-12.4), pos(0.68, x=2.0, y=3.9, z=-11.0), pos(0.78, x=2.0, y=3.9, z=-12.4),
             pos(0.95, **HOLD), pos(1.05, x=HAND_AT["x"], y=HAND_AT["y"], z=HAND_AT["z"] + 5.8),
             pos(1.3, **HAND_AT), pos(1.47, **HAND_AT), pos(1.62, **HOLD),
             pos(1.72, x=-1.5, y=1.5, z=1), pos(1.95), pos(2.3),
             rot(0), rot(0.95), rot(1.3), rot(1.47, z=-25), rot(1.62), rot(2.3)),
    animator(g_hammer, lrot(0)),
)

# One round pushed into the chamber at the top of the swung-out cylinder by the left hand:
# brought in from the side at a slant (not lined up behind the chamber, where from the eye it
# would look as if it were in already), straightened just behind it and pushed in (seated at
# 0.45: the game shows the chamber full from there), then the cylinder turned on to the next
# one. Added to the reload held at 0.95. The hand stays below the round, not over it.
LOOSE_FROM = dict(x=-3.1, y=0.4, z=6.0)   # in the crane's frame: behind and below the chamber
load_round = animation("load_round", 0.6, "once",
    animator(g_loose,
             show(0, True), show(0.45, False),
             lpos(0, **LOOSE_FROM), pos(0.25, x=-1.6, y=0.3, z=3.0), lpos(0.37, z=1.4), lpos(0.45),
             rot(0, y=-35), rot(0.25, y=-20), lrot(0.37), lrot(0.45)),
    animator(g_cylinder,
             lrot(0), lrot(0.45), lrot(0.57, z=60), kf("rotation", 0.6, **R(z=60), interp="step")),
    animator(g_larm,
             pos(0), pos(0.15, x=-0.5, y=1, z=3), pos(0.3, x=-0.6, y=2.0, z=2.8), pos(0.42, x=-0.2, y=2.4, z=1.4),
             pos(0.5, x=-0.5, y=1.8, z=2.2), pos(0.6)),
)

# ---------- at the gun station ----------
# Taken apart, lying on its side on the table: each part in its own window of time (the
# game plays each window backwards to put that part in). The frame stays; the parts end up
# laid beside it in the gun's side plane (y, z), where the table is.
# cylinder (with its crane) 0 .. 0.8, hammer 0.8 .. 1.3, mainspring 1.3 .. 1.8,
# barrel 1.8 .. 2.5.
GRIP_DOWN = (0, -math.cos(math.radians(22)), math.sin(math.radians(22)))
def down_grip(d): return dict(zip("xyz", [c * d for c in GRIP_DOWN]))
strip = animation("strip", 2.5, "hold",
    animator(g_crane,  # swung out, pulled forward off the frame, laid above it
             lrot(0), lrot(0.25, z=45), lrot(0.55, z=45), rot(0.8),
             lpos(0), lpos(0.25), lpos(0.55, z=-6), pos(0.8, y=7.5)),
    animator(g_hammer,  # cocked, lifted out of its slot, laid behind the frame
             rot(0.8), rot(0.95, x=40), rot(1.3),
             pos(0.8), pos(0.95), pos(1.1, y=3, z=0.5), pos(1.3, y=4.5, z=4)),
    animator(g_mainspring,  # out through the bottom of the grip
             pos(1.3), pos(1.45, **down_grip(3)), pos(1.8, **down_grip(11))),
    animator(g_barrel,  # unscrewed, and off forward
             rot(1.8), lrot(2.2, z=720),
             pos(1.8), pos(2.2, z=-2), pos(2.5, z=-5)),
)

# ---------- checks ----------
def check_coplanar():
    """Visible faces of two unrotated cubes lying in the same plane and overlapping flicker
    (z-fighting). Report every such pair."""
    # (the speedloader's rounds and the loose one are never seen where the chambers' are)
    boxes = [(e["name"], e["from"], e["to"], e["faces"]) for e in elements
             if not any(e["rotation"]) and not e["name"].startswith(("loader_round", "loose_round"))]
    # (only faces that are drawn: x -/+ west/east, y down/up, z north/south)
    face_of = {(0, -1): "west", (0, 1): "east", (1, -1): "down", (1, 1): "up", (2, -1): "north", (2, 1): "south"}
    drawn = lambda faces, ax, sign: faces[face_of[(ax, sign)]].get("texture") is not None
    bad = []
    for i in range(len(boxes)):
        for j in range(i + 1, len(boxes)):
            (na, fa, ta, xa), (nb, fb, tb, xb) = boxes[i], boxes[j]
            for ax in range(3):
                o = [k for k in range(3) if k != ax]
                ov = all(min(ta[k], tb[k]) - max(fa[k], fb[k]) > 1e-3 for k in o)
                if not ov: continue
                for pa, sa in ((fa[ax], -1), (ta[ax], 1)):
                    for pb, sb in ((fb[ax], -1), (tb[ax], 1)):
                        if abs(pa - pb) < 1e-4 and sa == sb and drawn(xa, ax, sa) and drawn(xb, ax, sb):
                            bad.append((na, nb, "xyz"[ax], pa))
    return bad

# ---------- texture: pack faces tallest first, paint, and point the UVs at them ----------
jobs = [j for j in jobs if j[0].get("texture") is not None]
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
            t = src[min(max(i, 0), tw - 1)]
            row[ax + 1 + i] = t if len(t) == 4 else t + (MAT_ALPHA.get(mat, 255),)
    face_ref["uv"] = [ax + 1, ay + 1, ax + 1 + tw, ay + 1 + th]

raw = b"".join(b"\x00" + b"".join(bytes(p) for p in row) for row in img)
def chunk(t, d): return struct.pack(">I", len(d)) + t + d + struct.pack(">I", zlib.crc32(t + d) & 0xffffffff)
png = (b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", ATLAS_W, TEX_H, 8, 6, 0, 0, 0))
       + chunk(b"IDAT", zlib.compress(raw, 9)) + chunk(b"IEND", b""))

# The gun's own parts move as far as it is scaled (not the arms, nor what holds it).
def subtree_uuids(g):
    out = {g["uuid"]}
    for c in g["children"]:
        if isinstance(c, dict):
            out |= subtree_uuids(c)
    return out
scaled_bones = subtree_uuids(g_revolver) - subtree_uuids(g_larm)
for an in (shoot, reload, load_round, walk, sprint, aim, strip):
    for uid, a in an["animators"].items():
        if uid in scaled_bones:
            for k in a["keyframes"]:
                if k["channel"] == "position":
                    for dp in k["data_points"]:
                        for ax in "xyz":
                            dp[ax] = dp[ax] * SCALE

model = {
    "meta": {"format_version": "4.10", "model_format": "free", "box_uv": False},
    "name": "revolver_viewmodel",
    "resolution": {"width": ATLAS_W, "height": TEX_H},
    "elements": elements,
    "outliner": [g_root],
    "textures": [{
        "name": "revolver.png", "id": "0", "uuid": str(uuid.uuid4()), "folder": "", "namespace": "",
        "width": ATLAS_W, "height": TEX_H, "uv_width": ATLAS_W, "uv_height": TEX_H,
        "particle": False, "render_mode": "default", "visible": True, "saved": False,
        "source": "data:image/png;base64," + base64.b64encode(png).decode(),
    }],
    "animations": [shoot, reload, load_round, walk, sprint, aim, strip],
}
with open(os.path.join(HERE, "revolver.bbmodel"), "w") as f:
    json.dump(model, f, indent=1)
with open(os.path.join(HERE, "revolver.png"), "wb") as f:
    f.write(png)

bad = check_coplanar()
for b in bad:
    print("coplanar:", *b)
print(f"ok: {len(elements)} cubes, texture {ATLAS_W}x{TEX_H}, {len(bad)} coplanar pairs")
