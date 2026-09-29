"""Generates fishing_rod.bbmodel (+ fishing_rod.png): the spinning rod and its bobber for
Blockbench.

Run:  python tools/blockbench/gen_fishing_rod.py
then: python tools/blockbench/bbmodel_to_rust.py tools/blockbench/fishing_rod.bbmodel

Model space (Blockbench pixels, 16 to a block): the rod lies along +Z from its butt at the
origin, up +Y, its spinning reel hanging under it with the crank on the +X side (for the left hand:
held right-handed, +X is the angler's left).
LENGTH_PX long, butt to tip.

Groups (bones):
- `rod`: the butt (EVA end cap, cork rear grip), the reel seat with its hoods, the cork fore
  grip and the winding check. Stiff.
  - `reel`: the foot, stem, gear box, spool with line wound on it and the drag knob.
    - `rotor`: the rotor cup with the bail arm and line roller (turns about Z, the reel's
      own axis, a few times per crank turn).
    - `crank`: the crank's shaft, arm and knob (turns about X through the gear box).
  - `blank0`..`blank7`: the tapered graphite blank, each segment a child of the one before
    (turned about its start, the rod bends progressively), with the line guides on their
    underside, getting smaller toward the tip, each on a red thread wrap trimmed in gold.
    - `tip`: the tip top ring (its middle is where the line leaves the rod).
- `bobber`: a red-top/white-bottom float, its middle at the origin, a small antenna on top
  with the eyelet the line is tied to.

Every face gets its own painted spot on one texture atlas (D texels per pixel), hand painted
with numpy in the game's pixel style: flat palettes, a little grain, no blur.
"""
import base64, io, json, math, os, uuid

import numpy as np
from PIL import Image

D = 8
ATLAS_W = 512
HERE = os.path.dirname(os.path.abspath(__file__))
RNG = np.random.default_rng(7)

LENGTH_PX = 35.2
BLANK_Z0 = 10.3
SEGMENTS = 8
REEL_Y = -2.6   # the reel's axis (spool, rotor) under the rod
GEAR_Z = 6.0    # the crank's axis goes through the gear box here (along X)

def U(v): return int(round(v * D))

# ---------------------------------------------------------------------------- palettes
MATS = {
    "carbon": (34, 36, 43), "cork": (190, 150, 98), "eva": (36, 36, 39),
    "seat": (54, 57, 64), "gold": (198, 158, 62), "gold_knurl": (190, 150, 58),
    "wrap_red": (160, 30, 32), "reel": (68, 72, 81), "reel_dark": (38, 40, 46),
    "spool": (176, 180, 188), "line": (170, 212, 196), "chrome": (196, 200, 208),
    "ring": (64, 66, 74), "knob": (30, 30, 32),
    "bob_red": (212, 40, 34), "bob_white": (236, 234, 226), "bob_mid": (212, 40, 34),
    "bob_black": (26, 26, 28), "antenna": (246, 96, 30),
}

def grain(h, w, k):
    return RNG.integers(-k, k + 1, (h, w))[..., None].astype(np.float32)

def paint(mat, face, w, h, along):
    """One face's texels (h, w, 3): `along` says which texture axis runs down the part's
    long axis ('u' or 'v', None on its ends)."""
    base = np.array(MATS[mat], np.float32)
    px = np.zeros((h, w, 3), np.float32) + base
    yy, xx = np.mgrid[0:h, 0:w]
    a = xx if along == "u" else yy          # along the part
    c = yy if along == "u" else xx          # across it
    n_along = w if along == "u" else h
    if mat == "carbon":
        px += grain(h, w, 2)
        # the graphite's cross wraps: faint diagonal glints
        px[(a + 2 * c) % 9 == 0] += 9
        px[(a + 2 * c) % 9 == 1] += 4
        if face == "up":
            px += 6
    elif mat == "cork":
        px += grain(h, w, 6)
        # the rings of cork, a little different each
        ring = (a // 3) % 4
        px += np.where(ring == 1, 7, np.where(ring == 3, -6, 0))[..., None]
        r = RNG.random((h, w))
        px[r < 0.13] = (128, 90, 54)
        px[(r > 0.13) & (r < 0.19)] = (150, 110, 66)
        px[r > 0.9] = (216, 180, 126)
    elif mat == "eva":
        px += grain(h, w, 4)
        px[(a % 4 == 0)] -= 8
    elif mat == "seat":
        px += grain(h, w, 2)
        if along:
            px[c % 5 == 0] += 10
    elif mat in ("gold", "gold_knurl"):
        px += grain(h, w, 5)
        if along:
            # a bright band round the middle, a dark one at each edge
            mid = n_along // 2
            px[a == mid] += 38
            px[(a == 0) | (a == n_along - 1)] -= 34
        if mat == "gold_knurl" and along:
            px[c % 2 == 0] -= 30
    elif mat == "wrap_red":
        px += grain(h, w, 3)
        if along:
            # the thread's turns (across the rod) and the gold trim at both ends
            px[a % 2 == 0] -= 18
            trim = (a == 0) | (a == n_along - 1)
            px[trim] = (212, 172, 72)
            px[trim & (c % 2 == 0)] = (170, 130, 48)
    elif mat in ("reel", "reel_dark"):
        px += grain(h, w, 3)
        if along:
            px[c % 4 == 1] += 9     # brushed
        if face == "up":
            px += 16
    elif mat == "spool":
        px += grain(h, w, 3)
        if along:
            px[a == 0] += 30
            px[a == n_along - 1] -= 40
    elif mat == "line":
        # the line wound on the spool: turns across it, crossing
        px += grain(h, w, 3)
        if along:
            px[a % 2 == 0] -= 22
            px[(a + c) % 5 == 0] += 16
    elif mat == "chrome":
        px += grain(h, w, 4)
        if along:
            px[c % 3 == 0] += 30
            px[c % 3 == 2] -= 44
        else:
            px[(xx + yy) % 3 == 0] += 20
    elif mat == "ring":
        px += grain(h, w, 3)
        px[(xx + yy) % 4 == 0] += 34
    elif mat == "knob":
        px += grain(h, w, 3)
        px[(yy % 3 == 0)] += 8
    elif mat in ("bob_red", "bob_white", "bob_black", "antenna"):
        px += grain(h, w, 3)
        if mat == "bob_red" and face == "up":
            px += 14
    elif mat == "bob_mid":
        # the float's widest band: red above the middle, white below, a black line between
        px += grain(h, w, 3)
        if face in ("north", "south", "east", "west"):
            half = h // 2
            px[yy >= half] = np.array(MATS["bob_white"], np.float32)
            px[yy >= half] += grain(h, w, 3)[yy >= half]
            px[(yy == half) | (yy == half - 1)] = MATS["bob_black"]
        elif face == "down":
            px[:] = MATS["bob_white"]
    # face shading (the game shades too, lightly)
    px += {"up": 8, "down": -12, "east": -3, "west": -3}.get(face, 0)
    return np.clip(px, 0, 255).astype(np.uint8)

# ---------------------------------------------------------------------------- model building
FACES = ("north", "east", "south", "west", "up", "down")
# the model axes a face's texture u and v run along
FACE_AXES = {"north": (0, 1), "south": (0, 1), "east": (2, 1), "west": (2, 1), "up": (0, 2), "down": (0, 2)}

elements, jobs = [], []

def cube(name, frm, to, mat, long=None, origin=None, rot=(0, 0, 0)):
    size = [b - a for a, b in zip(frm, to)]
    assert all(s > 0 for s in size), (name, frm, to)
    if long is None:
        long = int(np.argmax(size))
    faces = {}
    for f in FACES:
        ua, va = FACE_AXES[f]
        along = "u" if ua == long else "v" if va == long else None
        faces[f] = {"uv": None, "texture": 0}
        jobs.append((faces[f], f, mat, max(1, U(size[ua])), max(1, U(size[va])), along))
    centre = [(a + b) / 2 for a, b in zip(frm, to)]
    elements.append({
        "name": name, "type": "cube", "uuid": str(uuid.uuid4()), "box_uv": False, "rescale": False,
        "from": [round(v, 4) for v in frm], "to": [round(v, 4) for v in to],
        "origin": [round(v, 4) for v in (origin or centre)], "rotation": list(rot), "faces": faces,
    })
    return elements[-1]["uuid"]

def group(name, children, origin=(0, 0, 0)):
    return {"name": name, "uuid": str(uuid.uuid4()), "origin": [round(v, 4) for v in origin],
            "rotation": [0, 0, 0], "isOpen": True, "visibility": True, "export": True,
            "children": children}

def tube(name, axis, r, a0, a1, p, q, mat, step=0.04, octagon=True):
    """A round (octagonal) tube along `axis` (0 x, 1 y, 2 z) from a0 to a1 round (p, q) (the
    other two coordinates in order): two crossed boxes and four 45-degree corner strips, each
    standing back a little at the ends (their ends must not lie in one plane). A thin one is
    a plain square."""
    others = [k for k in range(3) if k != axis]
    def box(n, lo_a, hi_a, e0, e1, origin=None, rot=(0, 0, 0)):
        frm, to = [0.0] * 3, [0.0] * 3
        frm[axis], to[axis] = lo_a, hi_a
        frm[others[0]], to[others[0]] = e0[0], e1[0]
        frm[others[1]], to[others[1]] = e0[1], e1[1]
        return cube(n, frm, to, mat, long=axis, origin=origin, rot=rot)
    if not octagon:
        return [box(name, a0, a1, (p - r, q - r), (p + r, q + r))]
    n = r * math.tan(math.radians(22.5))
    step = min(step, (a1 - a0) / 6)
    parts = [
        box(name + "_a", a0, a1, (p - r, q - n), (p + r, q + n)),
        box(name + "_b", a0 + step, a1 - step, (p - n, q - r), (p + n, q + r)),
    ]
    s = (r - n) * math.sqrt(2) / 2
    rot = [0, 0, 0]
    rot[axis] = 45
    for i, (sp, sq) in enumerate(((1, 1), (1, -1), (-1, 1), (-1, -1))):
        cp, cq = p + sp * n, q + sq * n
        o = [0.0] * 3
        o[axis] = (a0 + a1) / 2
        o[others[0]], o[others[1]] = cp, cq
        parts.append(box(f"{name}_c{i}", a0 + 2 * step, a1 - 2 * step, (cp - s, cq - s), (cp + s, cq + s),
                         origin=o, rot=tuple(rot)))
    return parts

def ring(name, cx, cy, z, r, t, mat, bars=8, from_deg=0.0, to_deg=360.0):
    """A ring standing in the XY plane round (cx, cy) at `z`: short tangent bars (only those
    between from_deg and to_deg: an arc)."""
    parts = []
    step = 360.0 / bars
    L = 2 * r * math.tan(math.radians(step / 2)) + t * 0.6
    count = bars if to_deg - from_deg >= 360 else int(round((to_deg - from_deg) / step)) + 1
    for k in range(count):
        a = from_deg + k * step
        x, y = cx + r * math.cos(math.radians(a)), cy + r * math.sin(math.radians(a))
        parts.append(cube(f"{name}_{k}", [x - t / 2, y - L / 2, z - t / 2], [x + t / 2, y + L / 2, z + t / 2], mat,
                          long=1, origin=[x, y, z], rot=(0, 0, round(a % 360, 3))))
    return parts

def blank_r(z):
    """The blank's radius at z: from 0.24 at the fore grip to 0.075 at the tip."""
    t = (z - BLANK_Z0) / (LENGTH_PX - BLANK_Z0)
    return 0.24 + (0.075 - 0.24) * t

# ---- the butt: EVA end cap, rear cork grip, reel seat, fore grip, winding check
butt = (
    tube("butt_end", 2, 0.4, 0.0, 0.3, 0, 0, "eva")
    + tube("butt_cap", 2, 0.5, 0.2, 1.0, 0, 0, "eva")
    + tube("grip_rear", 2, 0.44, 0.95, 4.2, 0, 0, "cork")
    + tube("grip_rear_mid", 2, 0.47, 1.6, 3.5, 0, 0, "cork")
    + tube("seat_rear_hood", 2, 0.42, 4.15, 4.9, 0, 0, "gold")
    + tube("seat", 2, 0.34, 4.8, 8.1, 0, 0, "seat")
    + tube("seat_nut", 2, 0.43, 7.4, 8.1, 0, 0, "gold_knurl")
    + tube("seat_nut_ring", 2, 0.4, 8.05, 8.35, 0, 0, "gold")
    + tube("grip_fore", 2, 0.4, 8.3, 10.2, 0, 0, "cork")
    + tube("winding_check", 2, 0.3, 10.15, 10.5, 0, 0, "gold")
)

# ---- the reel, hanging under the seat
reel_parts = (
    [cube("reel_foot", [-0.24, -0.5, 4.9], [0.24, -0.3, 7.5], "reel", long=2),
     cube("reel_stem", [-0.17, REEL_Y + 0.5, 5.9], [0.17, -0.45, 6.6], "reel", long=1),
     cube("reel_stem_front", [-0.12, REEL_Y + 0.9, 6.55], [0.12, -0.5, 6.85], "reel", long=1)]
    # the gear box: round, sideways (the crank's axis through it)
    + tube("reel_body", 0, 0.82, -0.5, 0.5, REEL_Y, GEAR_Z, "reel")
    + tube("reel_body_rear", 2, 0.5, 5.0, 5.9, 0, REEL_Y + 0.05, "reel_dark")
    # the anti-reverse cap on the right (-X; the crank is on the left)
    + tube("reel_cap", 0, 0.42, -0.66, -0.45, REEL_Y, GEAR_Z, "gold")
    # the spool with the line on it, its lips and the drag knob
    + tube("spool_rear_lip", 2, 0.72, 7.55, 7.8, 0, REEL_Y, "spool")
    + tube("spool_line", 2, 0.64, 7.75, 8.95, 0, REEL_Y, "line")
    + tube("spool_front_lip", 2, 0.74, 8.9, 9.15, 0, REEL_Y, "spool")
    + tube("drag_knob", 2, 0.34, 9.1, 9.7, 0, REEL_Y, "reel_dark")
    + tube("drag_knob_cap", 2, 0.22, 9.65, 9.8, 0, REEL_Y, "gold")
)
rotor_parts = (
    tube("rotor", 2, 0.8, 6.5, 7.6, 0, REEL_Y, "reel_dark")
    + tube("rotor_ring", 2, 0.84, 7.35, 7.6, 0, REEL_Y, "gold")
    # the bail: an arm up each side, and the wire round under the spool
    + [cube("bail_arm_r", [0.8, REEL_Y - 0.1, 7.2], [0.92, REEL_Y + 0.1, 8.75], "chrome", long=2),
       cube("bail_arm_l", [-0.92, REEL_Y - 0.1, 7.2], [-0.8, REEL_Y + 0.1, 8.75], "chrome", long=2),
       cube("line_roller", [-1.02, REEL_Y - 0.14, 8.45], [-0.78, REEL_Y + 0.14, 8.8], "gold", long=0)]
    + ring("bail", 0, REEL_Y, 8.62, 0.86, 0.1, "chrome", bars=8, from_deg=180, to_deg=360.1)
)
crank_parts = (
    [cube("crank_shaft", [0.45, REEL_Y - 0.13, GEAR_Z - 0.13], [0.95, REEL_Y + 0.13, GEAR_Z + 0.13], "gold", long=0),
     cube("crank_arm", [0.93, REEL_Y - 1.75, GEAR_Z - 0.15], [1.15, REEL_Y + 0.2, GEAR_Z + 0.15], "reel", long=1),
     cube("crank_knob_stem", [1.13, REEL_Y - 1.72, GEAR_Z - 0.07], [1.35, REEL_Y - 1.48, GEAR_Z + 0.07], "gold", long=0)]
    # a T knob (a paddle) the left hand turns
    + [cube("crank_knob", [1.33, REEL_Y - 1.86, GEAR_Z - 0.34], [1.95, REEL_Y - 1.34, GEAR_Z + 0.34], "knob", long=0),
       cube("crank_knob_cap", [1.9, REEL_Y - 1.72, GEAR_Z - 0.2], [2.0, REEL_Y - 1.48, GEAR_Z + 0.2], "gold", long=2)]
)
KNOB = [1.64, REEL_Y - 1.6, GEAR_Z]

g_rotor = group("rotor", rotor_parts, (0, REEL_Y, 7.2))
g_crank = group("crank", crank_parts, (0, REEL_Y, GEAR_Z))
g_reel = group("reel", reel_parts + [g_rotor, g_crank], (0, -0.4, 6.2))

# ---- the blank in segments, with the guides
GUIDES = [  # (z, ring radius, stand-off stem length)
    (13.4, 0.30, 0.34), (17.2, 0.25, 0.28), (20.8, 0.21, 0.23),
    (24.3, 0.18, 0.19), (27.7, 0.15, 0.16), (31.0, 0.13, 0.14),
]
TIP = [0, -0.17, LENGTH_PX]
seg_len = (LENGTH_PX - BLANK_Z0) / SEGMENTS
seg_z = [BLANK_Z0 + i * seg_len for i in range(SEGMENTS + 1)]

def guide(k, z, R, stem):
    r = blank_r(z)
    t = max(0.05, R * 0.22)
    cy = -(r + stem + R)
    parts = tube(f"guide{k}_wrap", 2, r + 0.045, z - 0.42, z + 0.42, 0, 0, "wrap_red", octagon=r > 0.15)
    parts.append(cube(f"guide{k}_foot", [-0.09, -(r + 0.1), z - 0.34], [0.09, -r + 0.02, z + 0.34], "chrome", long=2))
    # the frame: from the foot down to the ring, leaning back a little (two legs for the
    # bigger ones)
    top = -(r + 0.02)
    bot = cy + R * 0.9
    if R > 0.4:
        for side in (-1, 1):
            x = side * R * 0.45
            parts.append(cube(f"guide{k}_leg{'lr'[side > 0]}", [x - 0.05, bot, z - 0.05], [x + 0.05, top, z + 0.05], "chrome",
                              long=1, origin=[x, top, z], rot=(0, 0, round(side * 14, 1))))
    else:
        parts.append(cube(f"guide{k}_stem", [-0.05, bot, z - 0.05], [0.05, top, z + 0.05], "chrome", long=1))
    parts += ring(f"guide{k}_ring", 0, cy, z, R, t, "ring", bars=8)
    parts += ring(f"guide{k}_frame", 0, cy, z + t * 0.35, R + t * 0.8, t * 0.5, "chrome", bars=8)
    return parts

segments = []
for i in range(SEGMENTS):
    z0, z1 = seg_z[i], seg_z[i + 1]
    r = blank_r(z0 + seg_len * 0.5)
    # each segment reaches a little into the next (no gap where they bend)
    last = i == SEGMENTS - 1
    end = z1 if last else z1 + r * 0.9
    parts = tube(f"blank{i}_tube", 2, r, z0, end, 0, 0, "carbon", step=0.02, octagon=r > 0.15)
    for k, (gz, R, stem) in enumerate(GUIDES):
        if z0 <= gz < z1:
            parts += guide(k, gz, R, stem)
    segments.append((parts, z0))

tip_parts = (
    tube("tip_tube", 2, 0.1, LENGTH_PX - 0.4, LENGTH_PX + 0.05, 0, 0, "chrome", octagon=False)
    + ring("tip_ring", 0, TIP[1], LENGTH_PX, 0.12, 0.06, "chrome", bars=6)
)
node = group("tip", tip_parts, (0, 0, LENGTH_PX))
for i in reversed(range(SEGMENTS)):
    parts, z0 = segments[i]
    node = group(f"blank{i}", parts + [node], (0, 0, z0))
g_rod = group("rod", butt + [g_reel, node], (0, 0, 0))

# ---- the bobber: a float round its middle, red above, white below, an antenna on top
bob = (
    tube("bob_bottom_tip", 1, 0.16, -1.6, -1.1, 0, 0, "bob_white", octagon=False)
    + tube("bob_low2", 1, 0.45, -1.15, -0.85, 0, 0, "bob_white")
    + tube("bob_low1", 1, 0.8, -0.9, -0.45, 0, 0, "bob_white")
    + tube("bob_mid", 1, 1.0, -0.5, 0.5, 0, 0, "bob_mid")
    + tube("bob_high1", 1, 0.8, 0.45, 0.9, 0, 0, "bob_red")
    + tube("bob_high2", 1, 0.45, 0.85, 1.15, 0, 0, "bob_red")
    + tube("bob_antenna", 1, 0.1, 1.1, 1.45, 0, 0, "antenna", octagon=False)
    + [cube("bob_eye_l", [-0.13, 1.4, -0.04], [-0.05, 1.6, 0.04], "chrome", long=1),
       cube("bob_eye_r", [0.05, 1.4, -0.04], [0.13, 1.6, 0.04], "chrome", long=1),
       cube("bob_eye_top", [-0.13, 1.52, -0.04], [0.13, 1.6, 0.04], "chrome", long=0)]
)
g_bobber = group("bobber", bob, (0, 0, 0))

outliner = [g_rod, g_bobber]

# ---------------------------------------------------------------------------- texture
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
img = np.zeros((TEX_H, ATLAS_W, 4), np.uint8)
for (face_ref, face, mat, tw, th, along), ax, ay in placed:
    px = paint(mat, face, tw, th, along)
    padded = np.pad(px, ((1, 1), (1, 1), (0, 0)), mode="edge")
    img[ay:ay + th + 2, ax:ax + tw + 2, :3] = padded
    img[ay:ay + th + 2, ax:ax + tw + 2, 3] = 255
    face_ref["uv"] = [ax + 1, ay + 1, ax + 1 + tw, ay + 1 + th]

buf = io.BytesIO()
Image.fromarray(img, "RGBA").save(buf, "PNG", optimize=True)
png = buf.getvalue()

model = {
    "meta": {"format_version": "4.10", "model_format": "free", "box_uv": False},
    "name": "fishing_rod",
    "resolution": {"width": ATLAS_W, "height": TEX_H},
    "elements": elements,
    "outliner": outliner,
    "textures": [{
        "name": "fishing_rod.png", "id": "0", "uuid": str(uuid.uuid4()), "folder": "", "namespace": "",
        "width": ATLAS_W, "height": TEX_H, "uv_width": ATLAS_W, "uv_height": TEX_H,
        "particle": False, "render_mode": "default", "visible": True, "saved": False,
        "source": "data:image/png;base64," + base64.b64encode(png).decode(),
    }],
    "animations": [],
}
with open(os.path.join(HERE, "fishing_rod.bbmodel"), "w") as f:
    json.dump(model, f, indent=1)
with open(os.path.join(HERE, "fishing_rod.png"), "wb") as f:
    f.write(png)
print(f"ok: {len(elements)} cubes, texture {ATLAS_W}x{TEX_H}; knob {KNOB}, tip {TIP}")
