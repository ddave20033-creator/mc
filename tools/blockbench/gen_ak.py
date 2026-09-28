"""Generates ak.bbmodel (+ ak.png): a first-person AK-47 view model for Blockbench, built the
same way as the pistol (gen_pistol.py) and the revolver (gen_revolver.py).

Run:  python tools/blockbench/gen_ak.py
then: python tools/blockbench/bbmodel_to_rust.py tools/blockbench/ak.bbmodel

Model space: 1 unit = 1 Blockbench pixel (about a centimetre, as the pistol), the first-person
camera at (0, 0, 0) looking -Z (the whole rig is moved there by OFF). The right fist holds the
pistol grip where it holds the pistol's; the left hand is under the handguard. Every face gets
its own painted spot on one texture atlas (D texels per unit). Parts that touch are sunk
slightly into each other so no two visible faces share a plane (`check_coplanar`).

An AK-47: a stamped receiver with its dust cover, the bolt carrier and its charging handle on
the right (`slide`: it flies back when firing), the rear sight block with its tangent leaf, a
wooden handguard under and over the gas tube, the gas block, the front sight tower and a
slant muzzle brake, a curved 30-round magazine that rocks in and out, a pistol grip and a
wooden stock with a steel butt plate. No attachments.

The bones the game looks for are named as the pistol's: `slide`, `trigger`, `magazine`,
`magazine_mesh`, `magazine_rounds`, `chambered_round`, `spent_case`, `left_arm`, and so are the
animations (`shoot`, `reload` with the pistol's moments, `walk`, `sprint`, `aim`, `strip`).
"""
import base64, json, math, os, random, struct, uuid, zlib

random.seed(47)
D = 8                 # texels per model unit
K = D / 4             # painter scale
ATLAS_W = 1024
HERE = os.path.dirname(os.path.abspath(__file__))

MATS = {
    "steel": (44, 46, 51), "cover": (50, 52, 58), "barrel": (58, 60, 64), "metal": (112, 117, 124),
    "sight": (26, 27, 29), "bore": (12, 12, 13), "wood": (128, 64, 36), "wood_dark": (98, 46, 26),
    "grip": (86, 42, 24), "mag": (52, 50, 48), "skin": (196, 140, 102), "sleeve": (52, 58, 70),
    "brass": (190, 150, 62), "copper": (178, 104, 66), "spring": (146, 150, 156),
    "butt": (40, 41, 44),
}
MAT_ALPHA = {}
DENSITY = [D]

def U(v):
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

def grain(px, w, h, face):
    """Lacquered wood: long streaks along the part (its length runs along z), darker grain
    lines, a knot or two, a soft sheen."""
    along_u = face in ("east", "west")
    n = h if along_u else w
    for k in range(n):
        if random.random() < 0.35:
            d = -random.randint(8, 22)
            if along_u:
                rect(px, 0, k, w, k + 1, lambda c: shade(c, d))
            else:
                rect(px, k, 0, k + 1, h, lambda c: shade(c, d))
    if face in ("east", "west") and h > 10:
        rect(px, 0, max(1, h // 4), w, max(1, h // 4) + 1, lambda c: shade(c, 16))
        if random.random() < 0.7:
            cx, cy = random.randint(3, max(4, w - 4)), random.randint(2, max(3, h - 3))
            disc(px, cx, cy, U(0.3), lambda c: shade(c, -26))

def rivets(*spots):
    """Rivet heads on both sides of the receiver: (units from the front, units down)."""
    def f(px, w, h, face):
        if face not in ("east", "west"): return
        for uf, v in spots:
            u = U(uf) if face == "west" else w - U(uf)
            disc(px, u, U(v), U(0.2), lambda c: shade(c, 26))
            disc(px, u + 1, U(v) + 1, U(0.12), lambda c: shade(c, -16))
    return f

def stamping(px, w, h, face):
    """The receiver's stamped side: the magazine well's dimple and a faint seam along it."""
    if face not in ("east", "west"): return
    rect(px, 0, U(0.6), w, U(0.6) + 1, lambda c: shade(c, -14))
    rect(px, 0, U(0.6) + 1, w, U(0.6) + 2, lambda c: shade(c, 10))
    # the oval dimple over the magazine well
    cu = U(5.5) if face == "west" else w - U(5.5)
    for j in range(h):
        for i in range(w):
            if ((i + 0.5 - cu) / U(1.6)) ** 2 + ((j + 0.5 - U(1.6)) / U(0.7)) ** 2 < 1:
                px[j][i] = shade(px[j][i], -12)

def cover_ribs(px, w, h, face):
    """The dust cover's stiffening ribs across its top."""
    if face == "up":
        for k in range(U(1.2), h - U(0.8), U(1.3)):
            rect(px, 1, k, w - 1, k + 1, lambda c: shade(c, -20))
            rect(px, 1, k + 1, w - 1, k + 2, lambda c: shade(c, 16))

def mag_ribs(px, w, h, face):
    """A steel magazine's pressed ribs down its sides and the stamped lines on its back."""
    if face in ("east", "west"):
        for u in (U(1.2), w - U(1.2)):
            rect(px, u, 0, u + 2, h, lambda c: shade(c, -18))
            rect(px, u + 2, 0, u + 3, h, lambda c: shade(c, 14))
        rect(px, w // 2 - 1, U(0.4), w // 2 + 1, h - U(0.4), lambda c: shade(c, 10))
    if face == "south":
        rect(px, w // 2 - 1, 0, w // 2 + 1, h, lambda c: shade(c, -16))

def floor_plate(px, w, h, face):
    if face == "down":
        rect(px, w // 2 - U(0.35), U(0.8), w // 2 + U(0.35), U(1.5), lambda c: shade(c, -28))

def grip_grooves(px, w, h, face):
    """The pistol grip's moulded grooves (sides) and finger ribs (front)."""
    if face in ("east", "west"):
        for k in range(U(0.8), h - U(0.6), U(0.55)):
            rect(px, U(0.4), k, w - U(0.4), k + 1, lambda c: shade(c, -18))
    if face == "north":
        for k in range(1, 5):
            v = int(h * k / 5)
            rect(px, 0, v, w, v + 1, lambda c: shade(c, -22))

def dark(*faces):
    def f(px, w, h, face):
        if face in faces:
            rect(px, 0, 0, w, h, lambda c: shade(MATS["bore"], random.randint(-2, 2)))
    return f

def bore_face(r):
    def f(px, w, h, face):
        if face == "north":
            disc(px, w / 2, h / 2, r * DENSITY[0] + 0.8 * K, lambda c: shade(c, -25))
            disc(px, w / 2, h / 2, r * DENSITY[0], (8, 8, 9))
    return f

def brake_ports(px, w, h, face):
    """The slant brake: its cut-away port on the top-left, the vent holes on the right."""
    if face == "up":
        rect(px, 0, U(0.2), w // 2, h - U(0.2), (14, 14, 15))
    if face == "east":
        for k in range(2):
            disc(px, U(0.6) + k * U(0.7), h / 2, U(0.18), (14, 14, 15))

def sight_notch(px, w, h, face):
    """The rear sight leaf: range numbers down its top, the notch's dark edge."""
    if face == "up":
        for k in range(1, 7):
            v = int(h * k / 8)
            rect(px, w // 2 - 1, v, w // 2 + 1, v + 1, lambda c: shade(c, 40))

def selector_face(px, w, h, face):
    if face == "east":
        rect(px, U(0.4), h // 2 - 1, w - U(0.4), h // 2, lambda c: shade(c, 24))
        for k in range(3):
            rect(px, w - U(0.8) - k * 3, 0, w - U(0.8) - k * 3 + 1, U(0.4), lambda c: shade(c, 30))

def coils(px, w, h, face):
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

def case_head(px, w, h, face):
    if face != "south": return
    cx, cy = w / 2, h / 2
    for j in range(h):
        for i in range(w):
            r = math.hypot(i + 0.5 - cx, j + 0.5 - cy)
            if r < U(0.17): px[j][i] = (200, 200, 205)
            elif r < U(0.2): px[j][i] = shade(px[j][i], -20)
            elif abs(r - U(0.38)) < 0.6: px[j][i] = shade(px[j][i], 26)

def case_shine(px, w, h, face):
    if face in ("east", "west", "up"):
        rect(px, 0, 1, w, 1 + K, lambda c: shade(c, 40))

def bullet_tip(px, w, h, face):
    if face == "north":
        disc(px, w / 2, h / 2, U(0.12), lambda c: shade(c, 40))
    elif face in ("east", "west", "up", "down"):
        for i in range(w):
            t = i / max(1, w - 1)
            k = 1 - t if face == "east" else t
            rect(px, i, 0, i + 1, h, lambda c, k=k: shade(c, int(-30 * (1 - k))))

def knob(px, w, h, face):
    if face in ("east", "north", "south"):
        for k in range(1, h, 2):
            rect(px, 0, k, w, k + 1, lambda c: shade(c, -22))

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
    s = half_diag * math.sqrt(2) / 2
    a, b = centre
    if axis == "z":
        return cube(name, [a - s, b - s, span[0]], [a + s, b + s, span[1]], mat, painters,
                    [a, b, (span[0] + span[1]) / 2], (0, 0, 45))
    else:
        return cube(name, [span[0], a - s, b - s], [span[1], a + s, b + s], mat, painters,
                    [(span[0] + span[1]) / 2, a, b], (45, 0, 0))

def tube(name, axis, r, z0, z1, mat, painters=(), front=(), density=None):
    """A round tube along z (octagonal): two crossed boxes and four 45-degree corner strips."""
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

def group(name, children, origin=(0, 0, 0), rot=(0, 0, 0), visible=True):
    return {"name": name, "uuid": str(uuid.uuid4()), "origin": sh(origin), "rotation": list(rot),
            "isOpen": True, "visibility": visible, "export": True, "children": children}

# ---- the layout ----
GP, GR = [0, 8, 2.5], (-20, 0, 0)     # grip pivot and tilt (as the pistol's)
BY = 10.9                             # the bore's height
SIGHT = 15.1                          # the sight line: the front post's top, the notch's bottom
RZ = (-17.0, 9.5)                     # the receiver, front and back
PORT = (-8.0, -1.0)                   # the ejection port on its right side

# ---- receiver (the frame: it stays; the barrel, sights, handguards and stock are fixed to it) ----
receiver = [
    cube("rec_lower", [-1.55, 7.7, RZ[0]], [1.55, 11.3, RZ[1]], "steel",
         [stamping, rivets((1.2, 0.9), (2.4, 0.9), (13.0, 2.2), (21.5, 2.2), (24.8, 1.2))]),
    cube("rec_wall_l", [-1.54, 11.28, RZ[0] + 0.01], [-1.12, 12.45, RZ[1] - 0.01], "steel", [dark("east")]),
    cube("rec_wall_r_front", [1.12, 11.28, RZ[0] + 0.01], [1.54, 12.45, PORT[0]], "steel", [dark("west")]),
    cube("rec_wall_r_back", [1.12, 11.28, PORT[1]], [1.54, 12.45, RZ[1] - 0.01], "steel", [dark("west")]),
    cube("rec_rear_block", [-1.45, 7.75, RZ[1] - 0.05], [1.45, 12.35, RZ[1] + 0.6], "steel"),
    # the trunnion and rear sight block in front of it, where the barrel and the gas tube meet
    cube("trunnion", [-1.52, 7.8, -22.6], [1.52, 12.2, RZ[0] + 0.05], "steel", [rivets((1.4, 1.0), (3.6, 1.0))]),
    cube("rs_block", [-1.35, 12.18, -22.4], [1.35, 14.15, -17.2], "steel"),
    cube("rs_block_top", [-1.1, 14.13, -22.2], [1.1, 14.4, -17.4], "steel"),
    # the tangent leaf and its slider, the notch's ears at its back end
    cube("rs_leaf", [-0.75, 14.38, -21.8], [0.75, 14.65, -17.9], "sight", [sight_notch], density=2 * D),
    cube("rs_slider", [-0.95, 14.39, -20.6], [0.95, 14.82, -19.6], "sight", density=2 * D),
    cube("rs_ear_l", [-0.9, 14.6, -18.55], [-0.24, SIGHT + 0.35, -17.95], "sight", density=2 * D),
    cube("rs_ear_r", [0.24, 14.6, -18.55], [0.9, SIGHT + 0.35, -17.95], "sight", density=2 * D),
    cube("rs_notch_floor", [-0.25, 14.61, -18.54], [0.25, SIGHT - 0.05, -17.96], "sight", density=2 * D),
    # the magazine catch behind the well, the selector lever down the right side
    cube("mag_catch", [-0.7, 6.6, -7.7], [0.7, 7.75, -6.95], "metal"),
    cube("mag_catch_paddle", [-0.9, 6.3, -7.35], [0.9, 6.65, -6.6], "metal"),
    cube("selector", [1.53, 10.35, -6.6], [1.78, 11.2, 4.2], "metal", [selector_face]),
    cube("selector_tab", [1.6, 9.6, 3.2], [1.9, 10.4, 4.15], "metal"),
]
# the trigger guard: a flat strip from the well to the grip
guard = [
    cube("guard_front", [-0.5, 5.65, -7.0], [0.5, 7.74, -6.45], "steel"),
    cube("guard_bottom", [-0.51, 5.6, -6.9], [0.51, 6.1, 0.8], "steel"),
]
# the trigger hangs from its pin, its blade curving forward at the bottom
trigger = [
    cube("trigger_upper", [-0.3, 6.4, -4.4], [0.3, 7.72, -3.7], "metal", [], [0, 7.7, -4.0], (6, 0, 0)),
    cube("trigger_lower", [-0.29, 6.25, -4.9], [0.29, 6.8, -4.2], "metal", [], [0, 6.5, -4.5], (32, 0, 0)),
]

# ---- barrel, gas block, front sight, muzzle brake ----
barrel = (
    tube("barrel", (0, BY), 0.62, -60.0, -22.45, "barrel")
    + tube("brake", (0, BY), 0.78, -62.4, -59.9, "barrel", [brake_ports], front=[bore_face(0.35)])
    + [
        # the front sight block round the barrel, its tower and the hood's ears round the post
        cube("fs_block", [-0.95, 9.95, -56.4], [0.95, 11.9, -53.2], "steel"),
        cube("fs_tower", [-0.6, 11.88, -56.0], [0.6, 13.6, -54.0], "steel"),
        cube("fs_ear_l", [-0.92, 12.6, -55.5], [-0.58, SIGHT + 0.45, -54.3], "steel"),
        cube("fs_ear_r", [0.58, 12.6, -55.5], [0.92, SIGHT + 0.45, -54.3], "steel"),
        cube("fs_post", [-0.13, 13.58, -55.05], [0.13, SIGHT, -54.75], "sight", density=2 * D),
        # the gas block and the bayonet lug under it
        cube("gas_block", [-1.0, 9.9, -45.2], [1.0, 13.9, -42.0], "steel"),
        cube("gas_block_lug", [-0.5, 9.3, -44.8], [0.5, 9.95, -42.8], "steel"),
        # the cleaning rod under the barrel, its head at the front sight
        cube("cleaning_rod", [-0.2, 9.35, -56.3], [0.2, 9.75, -22.4], "metal"),
        cube("cleaning_rod_head", [-0.32, 9.23, -57.0], [0.32, 9.87, -56.28], "metal"),
    ]
)

# ---- the lower handguard (wood) and its retainers ----
HG = (-36.0, -22.5)
handguard_lower = [
    cube("hg_lower", [-1.85, 7.5, HG[0]], [1.85, 11.75, HG[1]], "wood", [grain]),
    chamfer("hg_lower_round_l", "z", (-1.85, 7.5), 0.55, (HG[0] + 0.02, HG[1] - 0.02), "wood", [grain]),
    chamfer("hg_lower_round_r", "z", (1.85, 7.5), 0.55, (HG[0] + 0.02, HG[1] - 0.02), "wood", [grain]),
    cube("hg_retainer", [-2.0, 7.3, HG[0] - 0.7], [2.0, 11.9, HG[0] + 0.1], "steel"),
    cube("hg_rear_ferrule", [-1.95, 7.45, HG[1] - 0.2], [1.95, 11.9, HG[1] + 0.5], "steel"),
]

# ---- grip: bakelite, tilted as the pistol's, grooved ----
grip = [
    cube("grip_core", [-1.15, 1.0, 0.9], [1.15, 8.45, 4.7], "grip", [grip_grooves], GP, GR),
    cube("grip_front", [-0.9, 1.05, 0.62], [0.9, 8.4, 4.95], "grip", [grip_grooves], GP, GR),
    cube("grip_cap", [-1.2, 0.6, 0.8], [1.2, 1.05, 4.8], "steel", [], GP, GR),
]

# ---- stock: a wooden stock dropping to a steel butt plate ----
STOCK_Z = (RZ[1] + 0.4, 31.0)
stock = [
    # the comb along the top, tilted down toward the butt (it also fills the wrist)
    cube("stock_comb", [-1.3, 8.0, STOCK_Z[0]], [1.3, 12.0, STOCK_Z[1]], "wood", [grain],
         [0, 12.0, STOCK_Z[0]], (4, 0, 0)),
    # the belly: its bottom line falling from the wrist to the toe of the butt
    cube("stock_belly", [-1.27, 7.6, 13.0], [1.27, 11.4, 30.2], "wood", [grain],
         [0, 7.6, 13.0], (13.0, 0, 0)),
    cube("butt_plate", [-1.42, 2.9, STOCK_Z[1] - 0.35], [1.42, 10.9, STOCK_Z[1] + 0.3], "butt", [],
         [0, 10.9, STOCK_Z[1]], (6, 0, 0)),
    cube("sling_swivel", [-0.2, 6.8, 23.6], [0.2, 7.65, 25.0], "metal", [], [0, 7.6, 13.0], (13.0, 0, 0)),
]

# ---- dust cover and the recoil spring under it (one part at the gun station) ----
CZ0 = -17.15      # the cover's front, against the rear sight block
dust_cover = [
    cube("cover_top", [-1.1, 13.2, CZ0], [1.1, 13.75, RZ[1] - 0.1], "cover", [cover_ribs]),
    cube("cover_side_l", [-1.5, 12.4, CZ0 + 0.05], [-1.1, 13.5, RZ[1] - 0.15], "cover"),
    cube("cover_side_r", [1.1, 12.4, CZ0 + 0.05], [1.5, 13.5, RZ[1] - 0.15], "cover"),
    chamfer("cover_round_l", "z", (-1.1, 13.5), 0.4, (CZ0 + 0.1, RZ[1] - 0.2), "cover"),
    chamfer("cover_round_r", "z", (1.1, 13.5), 0.4, (CZ0 + 0.1, RZ[1] - 0.2), "cover"),
    # its back end, closed round the spring guide's button
    cube("cover_back", [-1.49, 12.36, RZ[1] - 0.45], [1.49, 13.72, RZ[1] - 0.08], "cover"),
    # the recoil spring's guide button sticking out of the back of the cover
    cube("cover_button", [-0.35, 12.65, RZ[1] - 0.2], [0.35, 13.15, RZ[1] + 0.45], "metal"),
]
recoil_spring = [
    cube("spring_coil", [-0.36, 11.7, -3.0], [0.36, 12.35, RZ[1] - 0.3], "spring", [coils]),
    cube("spring_guide", [-0.15, 11.87, -4.0], [0.15, 12.17, -2.9], "metal"),
]

# ---- bolt carrier (the "slide": back when firing, pulled by its handle), its piston ----
carrier = [
    cube("carrier_body", [-1.08, 11.45, -9.5], [1.08, 12.38, 0.5], "metal", [dark("south")]),
    cube("carrier_bolt", [-0.62, 10.35, -9.68], [0.62, 11.5, -8.0], "metal"),
    cube("charging_handle", [1.07, 11.6, -7.6], [2.55, 12.1, -6.8], "metal"),
    cube("charging_knob", [2.5, 11.35, -7.9], [3.25, 12.35, -6.5], "metal", [knob], density=2 * D),
    # the carrier's front, up to the piston
    cube("carrier_nose", [-0.45, 12.36, -9.45], [0.45, 13.2, -7.5], "metal"),
]
piston = tube("piston", (0, 12.95), 0.28, -17.0, -9.3, "metal")

# ---- gas tube and the upper handguard over it (one part at the gun station) ----
gas_tube = (
    tube("gas_tube", (0, 12.95), 0.72, -42.1, -22.3, "steel")
    + [
        cube("hg_upper", [-1.3, 12.05, -36.2], [1.3, 14.25, -22.4], "wood", [grain]),
        chamfer("hg_upper_round_l", "z", (-1.3, 14.25), 0.45, (-36.18, -22.42), "wood", [grain]),
        chamfer("hg_upper_round_r", "z", (1.3, 14.25), 0.45, (-36.18, -22.42), "wood", [grain]),
        cube("gas_tube_lever", [1.3, 12.5, -22.35], [1.85, 13.4, -21.2], "steel"),
    ]
)

# ---- magazine: curved, five pressed segments and a floor plate ----
MAG_TOP = (9.4, -10.5)            # (y, z): the middle of its top, inside the well
MAG_SEG, MAG_DEPTH = 2.85, 5.3
MAG_ANGLES = [0, 6, 12, 18, 24]
# Witness holes down both sides near the back, two on each segment (a hole's middle this far
# above and below the segment's middle, this far in from its back edge, this big); the brass
# behind each shows while the rounds reach down to it (`pistol_view::bench::mag_cube_shown`).
HOLE_OFF, HOLE_IN, HOLE_R = 0.7, 0.55, 0.2
def mag_holes(px, w, h, face):
    if face not in ("east", "west"): return
    top = MAG_SEG / 2 + 0.05
    for o in (HOLE_OFF, -HOLE_OFF):
        v = top - o
        u0 = U(HOLE_IN - HOLE_R) if face == "east" else w - U(HOLE_IN + HOLE_R)
        rect(px, u0, U(v - HOLE_R), u0 + U(2 * HOLE_R), U(v + HOLE_R), (8, 8, 9))
        rect(px, u0, U(v + HOLE_R), u0 + U(2 * HOLE_R), U(v + HOLE_R) + 1, lambda c: shade(c, 22))
mag_brass = []
def mag_chain():
    parts = []
    y, z = MAG_TOP
    for k, a in enumerate(MAG_ANGLES):
        r = math.radians(a)
        dy, dz = -math.cos(r), -math.sin(r)
        cy, cz = y + dy * MAG_SEG / 2, z + dz * MAG_SEG / 2
        w = 1.06 - 0.004 * k     # a hair narrower each, so their overlapping sides do not flicker
        parts.append(cube(f"mag_body{k}", [-w, cy - MAG_SEG / 2 - 0.05, cz - MAG_DEPTH / 2],
                          [w, cy + MAG_SEG / 2 + 0.05, cz + MAG_DEPTH / 2], "mag",
                          [mag_ribs, mag_holes] + ([dark("up")] if k == 0 else []), [0, cy, cz], (a, 0, 0)))
        # the brass behind its two holes, on both sides (counted from the top)
        zc = cz + MAG_DEPTH / 2 - HOLE_IN
        for j, o in enumerate((HOLE_OFF, -HOLE_OFF)):
            n = 2 * k + j
            for side, (x0, x1) in (("r", (w, w + 0.012)), ("l", (-w - 0.012, -w))):
                mag_brass.append(cube(f"witness_brass_{side}{n}", [x0, cy + o - HOLE_R, zc - HOLE_R],
                                      [x1, cy + o + HOLE_R, zc + HOLE_R], "brass", [], [0, cy, cz], (a, 0, 0)))
        y, z = y + dy * MAG_SEG, z + dz * MAG_SEG
    a = MAG_ANGLES[-1]
    r = math.radians(a)
    cy, cz = y - math.cos(r) * 0.25, z - math.sin(r) * 0.25
    parts.append(cube("mag_floor", [-1.2, cy - 0.28, cz - MAG_DEPTH / 2 - 0.2],
                      [1.2, cy + 0.28, cz + MAG_DEPTH / 2 + 0.2], "mag", [floor_plate], [0, cy, cz], (a, 0, 0)))
    return parts

magazine = mag_chain() + [
    cube("mag_lug", [-0.5, 8.7, -13.55], [0.5, 9.4, -13.1], "mag"),
    cube("mag_feed_lip_l", [-1.05, 9.35, -12.6], [-0.72, 10.05, -8.3], "mag"),
    cube("mag_feed_lip_r", [0.72, 9.35, -12.6], [1.05, 10.05, -8.3], "mag"),
]

def cartridge(name, x, y, z_back):
    """A 7.62x39 round with its head at `z_back`, pointing forward: the bottlenecked case, its
    neck, the copper-jacketed bullet. To the pistol's scale (its 9 mm round, 30 mm, is 2.7
    long): 56 mm long in all, the case 39 mm, 11.3 mm across."""
    return [
        cube(name + "_case", [x - 0.51, y - 0.51, z_back - 2.92], [x + 0.51, y + 0.51, z_back], "brass",
             [case_head, case_shine]),
        cube(name + "_neck", [x - 0.37, y - 0.37, z_back - 3.5], [x + 0.37, y + 0.37, z_back - 2.9], "brass", [case_shine]),
        cube(name + "_bullet", [x - 0.36, y - 0.36, z_back - 5.04], [x + 0.36, y + 0.36, z_back - 3.48], "copper",
             [bullet_tip]),
    ]
mag_rounds = cartridge("mag_round_top", 0.26, 9.82, -8.05) + cartridge("mag_round_2", -0.26, 9.35, -8.15) + mag_brass

# ---- arms: Minecraft proportions at twice the size ----
FR = (0, 4.5, 3.6)         # right fist centre (on the grip, as the pistol's)
FL = (-2.4, 5.9, -28.0)    # left fist centre, under the handguard
def arm(side, f, rot):
    x, y, z = f
    return group(side + "_arm_mesh", [
        cube(side + "_hand", [x - 3.5, y - 3.5, z - 3.5], [x + 3.5, y + 3.5, z + 13], "skin"),
        cube(side + "_sleeve", [x - 3.8, y - 3.8, z + 13], [x + 3.8, y + 3.8, z + 26], "sleeve"),
    ], f, rot)

# ---- hierarchy ----
g_trigger = group("trigger", trigger, (0, 7.7, -4.0))
g_frame = group("frame", receiver + guard + barrel + handguard_lower + stock + [g_trigger], (0, 10, 0))
g_grip = group("grip", grip, GP)
g_cover = group("dust_cover", dust_cover + [group("recoil_spring", recoil_spring, (0, 12, 3))], (0, 12.9, 0))
g_slide = group("slide", carrier + piston, (0, 11.8, -4.5))
g_gas = group("gas_tube", gas_tube, (0, 12.95, -30))
g_mag_rounds = group("magazine_rounds", mag_rounds, (0, 9.6, -10.5))
g_mag_mesh = group("magazine_mesh", magazine + [g_mag_rounds], (0, 9.4, -13.3))
g_mag = group("magazine", [g_mag_mesh], (0, 9.4, -13.3))
g_chamber = group("chambered_round", [
    group("chamber_case", [cube("chamber_case", [-0.51, BY - 0.51, -13.2], [0.51, BY + 0.51, -9.7], "brass",
                                [case_head, case_shine])], (0, BY, -11.4)),
    group("chamber_bullet", [cube("chamber_bullet", [-0.36, BY - 0.36, -14.74], [0.36, BY + 0.36, -13.18], "copper",
                                  [bullet_tip])], (0, BY, -14)),
], (0, BY, -12))
g_spent = group("spent_case", [
    cube("spent_case", [-0.49, BY - 0.49, -13.15], [0.49, BY + 0.49, -9.75], "brass", [case_head, case_shine])],
    (0, BY, -11.4))
g_larm = group("left_arm", [arm("left", FL, (22, -28, 0))], FL)
g_rifle = group("rifle", [g_frame, g_grip, g_cover, g_slide, g_gas, g_mag, g_chamber, g_spent, g_larm], GP)
g_rarm = group("right_arm", [arm("right", FR, (15, 20, 0)), g_rifle], FR)
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

# Firing (automatic: the next shot starts it again): the bolt carrier slams back and forward
# within a tenth of a second, the round is gone while it is open, a fresh one after.
shoot = animation("shoot", 0.3, "once",
    animator(g_trigger, lrot(0), lrot(0.02, x=-18), lrot(0.16, x=-18), lrot(0.24), lrot(0.3)),
    animator(g_slide, lpos(0), lpos(0.01), lpos(0.035, z=5.2), lpos(0.05, z=5.2), lpos(0.09), lpos(0.3)),
    animator(g_chamber, show(0, True), show(0.012, False), show(0.08, True)),
    animator(g_spent,
             show(0, False), show(0.012, True), show(0.28, False),
             lpos(0), lpos(0.035, z=5), lpos(0.1, x=5, y=2.5, z=3), lpos(0.28, x=13, y=1, z=4),
             lrot(0), lrot(0.035), lrot(0.28, x=-160, y=80, z=-420)),
)
shoot["selected"] = True

walk = animation("walk", 0.8, "loop",
    animator(g_root,
             pos(0), pos(0.2, x=0.9, y=-0.7), pos(0.4), pos(0.6, x=-0.9, y=-0.7), pos(0.8),
             rot(0), rot(0.2, z=-1.8), rot(0.4), rot(0.6, z=1.8), rot(0.8)),
)

# sprinting: the rifle swung across the chest, muzzle down and to the left
SPR = dict(x=-22, y=38, z=18)
sprint = animation("sprint", 0.5, "loop",
    animator(g_root,
             pos(0, x=-3, y=-2, z=4), pos(0.125, x=-1.8, y=-3.4, z=4), pos(0.25, x=-3, y=-2, z=4),
             pos(0.375, x=-4.2, y=-3.4, z=4), pos(0.5, x=-3, y=-2, z=4),
             rot(0, **SPR), rot(0.125, x=-24, y=38, z=16), rot(0.25, **SPR), rot(0.375, x=-24, y=38, z=20),
             rot(0.5, **SPR)),
)

# aiming down the sights: the rear notch to the middle of the view, the stock in the shoulder
AIM_Z = 12.0
aim = animation("aim", 0.3, "hold",
    animator(g_root, pos(0), pos(0.3, x=-OFF[0], y=-OFF[1] - SIGHT, z=AIM_Z), rot(0), rot(0.3)),
)

# Reload, at the pistol's moments (`pistol_view`): the old magazine rocked forward out of the
# well (out at 0.35, gone at 0.6), a new one brought up by the left hand (it has it at 0.62),
# hooked in front and rocked back in (in at 1.2, the hand off it at 1.35), then the left hand
# reaches over to the charging handle (1.32), pulls it (1.45 .. 1.55) and lets it go (1.6):
# the carrier chambers a round. Back under the handguard by 2.0.
TILT = dict(x=10, y=10, z=-40)
LIFT = dict(x=-5, y=8, z=1)
MAG_DOWN = dict(y=-9, z=-3)              # where the new magazine is brought from (tilted)
TO_MAG = dict(x=0.8, y=-13.5, z=13.5)      # the left fist from under the handguard to the magazine's front
TO_HANDLE = dict(x=5.6, y=5.8, z=19.0)    # ...and over to the charging handle
def plus(a, b): return {k: a.get(k, 0) + b.get(k, 0) for k in "xyz"}
reload = animation("reload", 2.0, "once",
    animator(g_rarm,
             rot(0), rot(0.3, **TILT), rot(1.25, **TILT), rot(1.45, x=4, y=4, z=-10), rot(1.75), rot(2.0),
             pos(0), pos(0.3, **LIFT), pos(1.25, **LIFT), pos(1.45, x=-1, y=3, z=3), pos(1.75), pos(2.0)),
    animator(g_mag,
             lrot(0), lrot(0.2), lrot(0.35, x=22), lrot(0.6, x=30), lrot(0.8, x=26), lrot(1.1, x=22), lrot(1.2),
             lrot(2.0),
             lpos(0), lpos(0.35, y=-0.8), lpos(0.5, y=-4, z=-1), lpos(0.6, y=-16, z=-2),
             lpos(0.8, **MAG_DOWN), lpos(1.1, y=-0.4), lpos(1.2), lpos(2.0)),
    animator(g_mag_mesh, show(0, True), show(0.6, False), show(0.8, True)),
    animator(g_larm,
             pos(0), pos(0.2, **TO_MAG), pos(0.35, **TO_MAG),
             pos(0.6, **plus(TO_MAG, dict(y=-7))),                     # lets it fall, reaches down
             pos(0.8, **plus(TO_MAG, MAG_DOWN)),                       # takes the new one
             pos(1.1, **plus(TO_MAG, dict(y=-0.4))), pos(1.22, **TO_MAG),  # ...and rocks it in
             pos(1.32, **plus(TO_HANDLE, dict(z=-1))),
             pos(1.45, **TO_HANDLE), pos(1.55, **plus(TO_HANDLE, dict(z=5.2))),   # pulls the handle
             pos(1.62, **plus(TO_HANDLE, dict(y=1.5, z=4))), pos(2.0),
             rot(0), rot(0.2, x=-12, y=6), rot(1.22, x=-12, y=6), rot(1.4, x=8, y=22, z=8),
             rot(1.62, x=8, y=22, z=8), rot(2.0)),
    animator(g_slide, lpos(0), lpos(1.45), lpos(1.55, z=5.2), lpos(1.6), lpos(2.0)),
    animator(g_chamber, show(0, False), show(1.6, True)),
    animator(g_spent, show(0, False)),
    animator(g_mag_rounds, show(0, False), show(0.62, True)),
)

# ---------- at the gun station ----------
# Taken apart (field stripped), lying on its side on the table: each part in its own window of
# time (the game plays each window backwards to put that part in). The frame stays; the parts
# end up laid beside it in the gun's side plane (y, z), where the table is.
# magazine 0 .. 0.5, dust cover with the spring 0.5 .. 1.1, bolt carrier 1.1 .. 1.8,
# gas tube with the upper handguard 1.8 .. 2.4.
strip = animation("strip", 2.4, "hold",
    animator(g_mag,  # rocked out, then laid under the stock
             rot(0), rot(0.2, x=24), rot(0.5, x=-90),
             pos(0), pos(0.2, y=-1.5), pos(0.5, y=-14, z=22)),
    animator(g_cover,  # lifted off at the back, laid just above the receiver and the stock
             pos(0.5), pos(0.65, y=0.4, z=0.6), pos(0.8, y=2.5, z=4), pos(1.1, y=3.6, z=12),
             rot(0.5), rot(0.65, x=-6), rot(1.1)),
    animator(g_slide,  # run back and lifted out, laid above the barrel
             pos(1.1), pos(1.35, z=6), pos(1.5, y=2.5, z=6), pos(1.8, y=6.2, z=-18)),
    animator(g_gas,  # its lever turned, lifted off the barrel, laid under the handguard
             pos(1.8), pos(1.95, y=1.2), pos(2.1, y=3, z=-2), pos(2.4, y=-10.5, z=-4),
             rot(1.8), rot(1.95, x=-6), rot(2.4)),
)

# ---------- checks ----------
def check_coplanar():
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
    w, h = job[3] + 2, job[4] + 2
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
    "name": "ak_viewmodel",
    "resolution": {"width": ATLAS_W, "height": TEX_H},
    "elements": elements,
    "outliner": [g_root],
    "textures": [{
        "name": "ak.png", "id": "0", "uuid": str(uuid.uuid4()), "folder": "", "namespace": "",
        "width": ATLAS_W, "height": TEX_H, "uv_width": ATLAS_W, "uv_height": TEX_H,
        "particle": False, "render_mode": "default", "visible": True, "saved": False,
        "source": "data:image/png;base64," + base64.b64encode(png).decode(),
    }],
    "animations": [shoot, reload, walk, sprint, aim, strip],
}
with open(os.path.join(HERE, "ak.bbmodel"), "w") as f:
    json.dump(model, f, indent=1)
with open(os.path.join(HERE, "ak.png"), "wb") as f:
    f.write(png)

bad = check_coplanar()
for b in bad:
    print("coplanar:", *b)
print(f"ok: {len(elements)} cubes, texture {ATLAS_W}x{TEX_H}, {len(bad)} coplanar pairs")
