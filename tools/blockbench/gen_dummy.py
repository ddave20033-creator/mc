"""Generates dummy.bbmodel (+ dummy.png): the wooden target dummy for Blockbench.

Run:  python tools/blockbench/gen_dummy.py
then: python tools/blockbench/bbmodel_to_rust.py tools/blockbench/dummy.bbmodel

Two root groups: `base`, the crossed feet it stands on (on the origin, up +Y), and `body`,
everything above them, which rocks on the top of the post's foot (its origin) when it is hit:
the post, a burlap sack of a torso with a target painted on its front, a crossbar through it
for arms with straw tufts at the ends, a rope round its waist and neck, and a sack of a head
with a stitched face. Its front faces +Z. Every face gets its own painted spot on one texture
atlas (D texels per unit).
"""
import base64, json, math, os, random, struct, uuid, zlib

random.seed(11)
D = 7
ATLAS_W = 1024
HERE = os.path.dirname(os.path.abspath(__file__))

MATS = {
    "wood": (138, 102, 60), "wood_dark": (96, 70, 42), "burlap": (188, 158, 108),
    "straw": (214, 188, 98), "rope": (168, 138, 88),
}
RED = (176, 40, 34)
CREAM = (232, 222, 196)
THREAD = (74, 52, 34)

def clamp(v): return max(0, min(255, int(v)))
def shade(c, d): return tuple(clamp(v + d) for v in c)
def U(v): return int(round(v * D))

def rect(px, x0, y0, x1, y1, col):
    for j in range(max(0, int(y0)), min(len(px), int(y1))):
        for i in range(max(0, int(x0)), min(len(px[0]), int(x1))):
            px[j][i] = col(px[j][i]) if callable(col) else col

def paint_face(face, mat, w, h, painters):
    base = MATS[mat]
    px = [[None] * w for _ in range(h)]
    for j in range(h):
        for i in range(w):
            d = random.randint(-5, 5)
            if face == "up": d += 10
            elif face == "down": d -= 18
            elif face in ("east", "west"): d -= 6
            px[j][i] = shade(base, d)
    for p in painters:
        p(px, w, h, face)
    return px

# ---------- painters ----------
def grain(along):
    """Wood grain: dark streaks along the board (`along`: "v" up the faces, "h" across)."""
    def f(px, w, h, face):
        vertical = along == "v" and face not in ("up", "down")
        n = w if vertical else h
        for k in range(n):
            if random.random() < 0.28:
                d = -random.randint(10, 22)
                if vertical:
                    rect(px, k, 0, k + 1, h, lambda c: shade(c, d))
                else:
                    rect(px, 0, k, w, k + 1, lambda c: shade(c, d))
        # a knot now and then
        if w > 8 and h > 8 and random.random() < 0.6:
            cx, cy = random.randint(2, w - 3), random.randint(2, h - 3)
            rect(px, cx - 1, cy - 1, cx + 2, cy + 2, lambda c: shade(c, -34))
    return f

def weave(px, w, h, face):
    """Burlap: a coarse weave of lighter and darker threads."""
    for j in range(h):
        for i in range(w):
            if (i // 2 + j // 2) % 2 == 0:
                px[j][i] = shade(px[j][i], 9)
            if (i + j * 3) % 7 == 0:
                px[j][i] = shade(px[j][i], -14)

def seams(px, w, h, face):
    """Stitched seams down the sack's sides (the edges of its front and back)."""
    if face not in ("north", "south", "east", "west"):
        return
    for j in range(1, h - 1, 3):
        rect(px, 0, j, 1, j + 2, THREAD)
        rect(px, w - 1, j, w, j + 2, THREAD)

def target(px, w, h, face):
    """A target painted on the front: red and cream rings round a red bull's-eye."""
    if face != "south":
        return
    cx, cy = w / 2, h * 0.45
    r_out = min(w, h) * 0.42
    for j in range(h):
        for i in range(w):
            d = math.hypot(i + 0.5 - cx, j + 0.5 - cy) / r_out
            if d > 1.0:
                continue
            ring = int(d * 5)  # 0 = bull's-eye .. 4 = outer ring
            col = RED if ring % 2 == 0 else CREAM
            # worn paint: the weave shows through here and there
            px[j][i] = shade(col, random.randint(-10, 6) - (8 if random.random() < 0.12 else 0))

def face_painter(px, w, h, face):
    """The head's front: two stitched crosses for eyes and a stitched mouth."""
    if face != "south":
        return
    s = max(2, w // 7)
    for ex in (w * 0.3, w * 0.7):
        ey = h * 0.4
        for k in range(-s, s + 1):
            for x, y in ((ex + k, ey + k), (ex + k, ey - k)):
                rect(px, x, y, x + 1, y + 1, THREAD)
    my = int(h * 0.72)
    for x in range(int(w * 0.25), int(w * 0.75), 3):
        rect(px, x, my, x + 2, my + 1, THREAD)

def wrap(px, w, h, face):
    """Rope: twisted strands."""
    for j in range(h):
        for i in range(w):
            if (i + j) % 4 == 0:
                px[j][i] = shade(px[j][i], -24)
            elif (i + j) % 4 == 1:
                px[j][i] = shade(px[j][i], 12)

def stalks(px, w, h, face):
    """A straw bundle lying along X: stalks running its length on the sides, their cut ends
    (light dots in dark gaps) on the end faces."""
    if face in ("east", "west"):
        for j in range(h):
            for i in range(w):
                px[j][i] = shade(px[j][i], -38)
        for j in range(0, h, 2):
            for i in range((j // 2) % 2, w, 2):
                px[j][i] = shade(MATS["straw"], random.randint(-8, 22))
        return
    for j in range(h):
        d = random.choice((-34, -18, -6, 0, 10, 20))
        rect(px, 0, j, w, j + 1, lambda c: shade(c, d))
        # a stalk's joint now and then
        if random.random() < 0.3:
            k = random.randint(0, w - 1)
            rect(px, k, j, k + 1, j + 1, lambda c: shade(c, -26))

# ---------- model building ----------
FACES = ("north", "east", "south", "west", "up", "down")
def face_size(face, s):
    sx, sy, sz = s
    return {"north": (sx, sy), "south": (sx, sy), "east": (sz, sy), "west": (sz, sy),
            "up": (sx, sz), "down": (sx, sz)}[face]

elements, jobs = [], []

def cube(name, frm, to, mat, painters=(), rot=(0, 0, 0), origin=None):
    size = [b - a for a, b in zip(frm, to)]
    assert all(s > 0 for s in size), name
    faces = {}
    for f in FACES:
        fw, fh = face_size(f, size)
        faces[f] = {"uv": None, "texture": 0}
        jobs.append((faces[f], f, mat, max(1, U(fw)), max(1, U(fh)), tuple(painters)))
    centre = [(a + b) / 2 for a, b in zip(frm, to)]
    elements.append({
        "name": name, "type": "cube", "uuid": str(uuid.uuid4()), "box_uv": False, "rescale": False,
        "from": [round(v, 4) for v in frm], "to": [round(v, 4) for v in to],
        "origin": [round(v, 4) for v in (origin or centre)], "rotation": list(rot), "faces": faces,
    })
    return elements[-1]["uuid"]

base = [
    cube("foot_x", [-8, 0, -2], [8, 2, 2], "wood_dark", [grain("h")]),
    cube("foot_z", [-2, 0.02, -8], [2, 1.98, 8], "wood_dark", [grain("h")]),
]
body = [
    cube("post", [-1.5, 2, -1.5], [1.5, 13, 1.5], "wood", [grain("v")]),
    cube("torso", [-5, 12, -3], [5, 23, 3], "burlap", [weave, seams, target]),
    cube("waist_rope", [-5.25, 13, -3.25], [5.25, 14, 3.25], "rope", [wrap]),
    cube("crossbar", [-9.6, 19.5, -1.2], [9.6, 21.5, 1.2], "wood", [grain("h")]),
    cube("neck_rope", [-1.6, 23, -1.6], [1.6, 24, 1.6], "rope", [wrap]),
    cube("head", [-3, 24, -3], [3, 30, 3], "burlap", [weave, face_painter]),
    cube("head_knot", [-1, 30, -1], [1, 31, 1], "rope", [wrap]),
]

def hand(side):
    """A straw bundle for a hand at the crossbar's end (`side` 1: +X, -1: -X): tied on with
    rope, narrow where it is tied and spreading out, a few loose stalks sticking out."""
    name = "right" if side > 0 else "left"
    def xs(a, b):
        return (a * side, b * side) if side > 0 else (b * side, a * side)
    parts = []
    x0, x1 = xs(8.8, 9.6)
    parts.append(cube(f"hand_{name}_rope", [x0, 19.3, -1.4], [x1, 21.7, 1.4], "rope", [wrap]))
    x0, x1 = xs(9.6, 10.8)
    parts.append(cube(f"hand_{name}_neck", [x0, 19.55, -1.1], [x1, 21.45, 1.1], "straw", [stalks]))
    x0, x1 = xs(10.8, 12.6)
    parts.append(cube(f"hand_{name}_bundle", [x0, 19.1, -1.5], [x1, 21.9, 1.5], "straw", [stalks]))
    for k, (y, z, rz, ry) in enumerate(((21.4, 0.9, 14, -8), (19.7, -0.8, -12, 10), (20.6, 0.1, 4, 16))):
        x0, x1 = xs(12.3, 13.9 - 0.3 * k)
        pivot = [12.4 * side, y, z]
        parts.append(cube(f"hand_{name}_stalk{k}", [x0, y - 0.13, z - 0.13], [x1, y + 0.13, z + 0.13], "straw",
                          [stalks], rot=(0, ry * side, rz * side), origin=pivot))
    return parts

body += hand(1) + hand(-1)

def group(name, children, origin=(0, 0, 0)):
    return {"name": name, "uuid": str(uuid.uuid4()), "origin": list(origin), "rotation": [0, 0, 0],
            "isOpen": True, "visibility": True, "export": True, "children": children}

outliner = [group("base", base), group("body", body, (0, 2, 0))]

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
for (face_ref, face, mat, tw, th, painters), ax, ay in placed:
    px = paint_face(face, mat, tw, th, painters)
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
    "name": "dummy",
    "resolution": {"width": ATLAS_W, "height": TEX_H},
    "elements": elements,
    "outliner": outliner,
    "textures": [{
        "name": "dummy.png", "id": "0", "uuid": str(uuid.uuid4()), "folder": "", "namespace": "",
        "width": ATLAS_W, "height": TEX_H, "uv_width": ATLAS_W, "uv_height": TEX_H,
        "particle": False, "render_mode": "default", "visible": True, "saved": False,
        "source": "data:image/png;base64," + base64.b64encode(png).decode(),
    }],
    "animations": [],
}
with open(os.path.join(HERE, "dummy.bbmodel"), "w") as f:
    json.dump(model, f, indent=1)
with open(os.path.join(HERE, "dummy.png"), "wb") as f:
    f.write(png)
print(f"ok: {len(elements)} cubes, texture {ATLAS_W}x{TEX_H}")
