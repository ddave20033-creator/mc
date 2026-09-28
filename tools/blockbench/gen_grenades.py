"""Generates grenades.bbmodel (+ grenades.png): the frag and the smoke grenade for Blockbench.

Run:  python tools/blockbench/gen_grenades.py
then: python tools/blockbench/bbmodel_to_rust.py tools/blockbench/grenades.bbmodel

Two root groups, `frag` and `smoke`, each standing on the origin (its bottom's middle), up
+Y, its lever on the +X side. A frag grenade: a segmented olive body (the old pineapple kind),
its fuse on top, the spoon lever down its side and the pin's ring. A smoke grenade: a grey can
with a coloured band, the same fuse, lever and ring. Every face gets its own painted spot on
one texture atlas (D texels per unit).
"""
import base64, json, math, os, random, struct, uuid, zlib

random.seed(5)
D = 12
ATLAS_W = 1024
HERE = os.path.dirname(os.path.abspath(__file__))

MATS = {
    "olive": (92, 104, 58), "olive_dark": (62, 70, 40), "steel": (150, 154, 160),
    "steel_dark": (96, 100, 106), "can": (118, 124, 118), "red": (196, 52, 40),
    "yellow": (218, 186, 70), "black": (28, 28, 30),
}

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
            d = random.randint(-4, 4)
            if face == "up": d += 10
            elif face == "down": d -= 16
            elif face in ("east", "west"): d -= 5
            if j == 0: d += 14
            elif j == h - 1: d -= 12
            px[j][i] = shade(base, d)
    for p in painters:
        p(px, w, h, face)
    return px

# ---------- painters ----------
def segments(px, w, h, face):
    """The pineapple's grooves: rows and columns of raised squares."""
    if face in ("up", "down"):
        return
    step = U(1.0)
    for j in range(0, h, step):
        rect(px, 0, j, w, j + 2, lambda c: shade(c, -34))
        rect(px, 0, j + 2, w, j + 3, lambda c: shade(c, 16))
    for i in range(0, w, step):
        rect(px, i, 0, i + 2, h, lambda c: shade(c, -30))

def band(col, stripe=None):
    """A painted band round a can (and a thinner stripe in it)."""
    def f(px, w, h, face):
        if face in ("up", "down"):
            return
        rect(px, 0, int(h * 0.30), w, int(h * 0.62), col)
        if stripe:
            rect(px, 0, int(h * 0.44), w, int(h * 0.50), stripe)
    return f

def knurl(px, w, h, face):
    if face in ("up", "down"):
        return
    for i in range(0, w, 3):
        rect(px, i, 0, i + 1, h, lambda c: shade(c, -26))

def stencil(px, w, h, face):
    """A few stencilled marks on a can."""
    if face not in ("north", "south"):
        return
    y = int(h * 0.72)
    x = U(0.3)
    while x < w - U(0.4):
        n = random.randint(2, 4)
        rect(px, x, y, x + n, y + U(0.35), (30, 30, 30))
        x += n + 2

# ---------- model building ----------
FACES = ("north", "east", "south", "west", "up", "down")
def face_size(face, s):
    sx, sy, sz = s
    return {"north": (sx, sy), "south": (sx, sy), "east": (sz, sy), "west": (sz, sy),
            "up": (sx, sz), "down": (sx, sz)}[face]

elements, jobs = [], []

def cube(name, frm, to, mat, painters=(), origin=None, rot=(0, 0, 0)):
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

def vtube(name, r, y0, y1, mat, painters=(), x=0.0, z=0.0):
    """A round (octagonal) upright tube: two crossed boxes and four 45-degree corner strips."""
    n = r * 0.414
    parts = [
        cube(name + "_a", [x - r, y0, z - n], [x + r, y1, z + n], mat, painters),
        cube(name + "_b", [x - n, y0 + 0.004, z - r], [x + n, y1 - 0.004, z + r], mat, painters),
    ]
    s = (r - n) * math.sqrt(2) / 2
    for i, (sx, sz) in enumerate(((1, 1), (1, -1), (-1, 1), (-1, -1))):
        cx, cz = x + sx * n, z + sz * n
        parts.append(cube(f"{name}_c{i}", [cx - s, y0 + 0.008, cz - s], [cx + s, y1 - 0.008, cz + s], mat, painters,
                          [cx, (y0 + y1) / 2, cz], (0, 45, 0)))
    return parts

def ring(name, cx, cy, r, t, mat):
    """The pin's ring, standing in the XY plane round (cx, cy): eight short bars."""
    parts = []
    L = 2 * r * math.tan(math.radians(22.5)) + 0.02
    for k in range(8):
        a = k * 45.0
        x, y = cx + r * math.cos(math.radians(a)), cy + r * math.sin(math.radians(a))
        parts.append(cube(f"{name}_{k}", [x - t / 2, y - L / 2, -t / 2], [x + t / 2, y + L / 2, t / 2], mat,
                          origin=[x, y, 0], rot=(0, 0, a)))
    return parts

def fuse(prefix, top):
    """The fuse on top (`top`: the body's top), the spoon lever down the +X side, the pin and
    its ring."""
    return (
        vtube(prefix + "_fuse", 0.95, top, top + 1.7, "steel", [knurl])
        + [
            cube(prefix + "_fuse_cap", [-0.7, top + 1.7, -0.7], [0.7, top + 2.1, 0.7], "steel_dark"),
            # the spoon: over the fuse, then down the side
            cube(prefix + "_spoon_top", [-0.2, top + 2.1, -0.55], [2.2, top + 2.4, 0.55], "steel"),
            cube(prefix + "_spoon_side", [2.2, top - 3.5, -0.55], [2.5, top + 2.4, 0.55], "steel"),
            # the pin through the fuse, and its ring off the -X side
            cube(prefix + "_pin", [-1.9, top + 1.05, -0.12], [1.0, top + 1.3, 0.12], "steel_dark"),
        ]
        + ring(prefix + "_ring", -2.9, top + 1.2, 1.0, 0.26, "steel")
    )

# ---- frag: a pineapple body on a short neck
frag = (
    vtube("frag_body", 2.9, 0.8, 7.2, "olive", [segments])
    + vtube("frag_bottom", 2.2, 0.0, 0.82, "olive_dark")
    + vtube("frag_shoulder", 2.2, 7.18, 8.0, "olive_dark")
    + fuse("frag", 8.0)
)
# ---- smoke: a grey can with a coloured band
smoke = (
    vtube("smoke_can", 2.4, 0.0, 8.4, "can", [band(MATS["red"], MATS["yellow"]), stencil])
    + vtube("smoke_rim", 2.5, 8.4, 8.8, "steel_dark")
    + fuse("smoke", 8.8)
)

def group(name, children, origin=(0, 0, 0)):
    return {"name": name, "uuid": str(uuid.uuid4()), "origin": list(origin), "rotation": [0, 0, 0],
            "isOpen": True, "visibility": True, "export": True, "children": children}

outliner = [group("frag", frag), group("smoke", smoke)]

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
    "name": "grenades",
    "resolution": {"width": ATLAS_W, "height": TEX_H},
    "elements": elements,
    "outliner": outliner,
    "textures": [{
        "name": "grenades.png", "id": "0", "uuid": str(uuid.uuid4()), "folder": "", "namespace": "",
        "width": ATLAS_W, "height": TEX_H, "uv_width": ATLAS_W, "uv_height": TEX_H,
        "particle": False, "render_mode": "default", "visible": True, "saved": False,
        "source": "data:image/png;base64," + base64.b64encode(png).decode(),
    }],
    "animations": [],
}
with open(os.path.join(HERE, "grenades.bbmodel"), "w") as f:
    json.dump(model, f, indent=1)
with open(os.path.join(HERE, "grenades.png"), "wb") as f:
    f.write(png)
print(f"ok: {len(elements)} cubes, texture {ATLAS_W}x{TEX_H}")
