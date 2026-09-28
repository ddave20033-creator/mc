"""Turns a Blockbench view model (.bbmodel) into data the game draws: a Rust source file with
its bones, cubes and animations, and a PNG of 128x128 texture pages (one game texture layer
each) holding every face's piece of the model's texture.

Run after changing a model in Blockbench (and saving it), then rebuild the game:
    python tools/blockbench/bbmodel_to_rust.py [path/to/model.bbmodel]

Models (by file name):
    pistol.bbmodel       -> src/model/pistol_vm_data.rs, src/model/pistol_vm.png
    gun_station.bbmodel  -> src/model/gun_station_data.rs, src/model/gun_station.png
    grenades.bbmodel     -> src/model/grenade_data.rs, src/model/grenade.png

Conventions kept from Blockbench so the game moves exactly like the Blockbench preview:
- positions in Blockbench pixels, the first-person camera at (0, 0, 0) looking -Z;
- a group turns about its origin, Euler order ZYX, degrees;
- files saved before format 5.0 store animation position x and rotation x/y negated
  (Blockbench flips them when loading such a file), so they are flipped here too;
- keyframes interpolate like Blockbench: linear, catmullrom, or step.
Groups named in NOT_DRAWN keep their bones (the game uses where they are) but their cubes are
left out: the game draws the player's own arms there, and throws a real case instead of the
placeholder one.
"""
import base64, io, json, os, sys
from PIL import Image

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(os.path.dirname(HERE))
# The model: given on the command line (e.g. the copy you work on), or the one next to this.
SRC = sys.argv[1] if len(sys.argv) > 1 else os.path.join(HERE, "pistol.bbmodel")
base = os.path.basename(SRC)
KIND = "gun_station" if base.startswith("gun_station") else "grenades" if base.startswith("grenade") else "pistol"
OUT_NAME = {"pistol": "pistol_vm", "gun_station": "gun_station", "grenades": "grenade"}[KIND]
OUT_RS = os.path.join(ROOT, "src", "model", OUT_NAME + "_data.rs")
OUT_PNG = os.path.join(ROOT, "src", "model", OUT_NAME + ".png")
PAGE = 128
NOT_DRAWN = {"right_arm_mesh", "left_arm_mesh", "spent_case"}
# game face order (world::mesh::FACE_N): +X, -X, +Y, -Y, +Z, -Z
FACES = ["east", "west", "up", "down", "south", "north"]

m = json.load(open(SRC, encoding="utf-8"))
ver = tuple(int(p) for p in str(m["meta"].get("format_version", "5.0")).split(".")[:2])
flip_old = ver < (5, 0)

tex = m["textures"][0]
atlas = Image.open(io.BytesIO(base64.b64decode(tex["source"].split(",", 1)[1]))).convert("RGBA")
sx = atlas.width / tex.get("uv_width", m["resolution"]["width"])
sy = atlas.height / tex.get("uv_height", m["resolution"]["height"])
elements = {e["uuid"]: e for e in m["elements"]}

# ---- bones and cubes ----
bones, cubes, bone_of = [], [], {}
def walk(node, parent, hidden):
    if isinstance(node, str):
        e = elements.get(node)
        if e and e.get("type", "cube") == "cube" and not hidden and e.get("visibility", True):
            cubes.append((parent, e))
        return
    i = len(bones)
    bone_of[node["uuid"]] = i
    bones.append(dict(name=node["name"], parent=parent, origin=node.get("origin", [0, 0, 0]),
                      rot=node.get("rotation", [0, 0, 0])))
    hide = hidden or node["name"] in NOT_DRAWN
    for c in node.get("children", []):
        walk(c, i, hide)
for n in m["outliner"]:
    walk(n, -1, False)

# ---- every face's texels onto 128x128 pages (1 texel border copied from the edge) ----
pieces = []  # (cube index, face index, image, flip_u, flip_v)
for ci, (_, e) in enumerate(cubes):
    for fi, name in enumerate(FACES):
        f = e["faces"].get(name)
        if not f or f.get("texture") is None or not f.get("uv"):
            continue
        u1, v1, u2, v2 = f["uv"]
        x0, x1 = sorted((u1 * sx, u2 * sx))
        y0, y1 = sorted((v1 * sy, v2 * sy))
        box = (int(round(x0)), int(round(y0)), max(int(round(x1)), int(round(x0)) + 1),
               max(int(round(y1)), int(round(y0)) + 1))
        img = atlas.crop(box)
        if img.width > PAGE - 2 or img.height > PAGE - 2:
            k = min((PAGE - 2) / img.width, (PAGE - 2) / img.height)
            img = img.resize((max(1, int(img.width * k)), max(1, int(img.height * k))), Image.NEAREST)
        if f.get("rotation"):
            raise SystemExit(f"{e['name']} {name}: rotated face UVs are not supported")
        pieces.append((ci, fi, img, u2 < u1, v2 < v1))

pieces.sort(key=lambda p: (-p[2].height, -p[2].width))
pages, placed = [], {}
shelves = []  # per page: list of [y, height, x]
def place(w, h):
    for pi, sh in enumerate(shelves):
        for s in sh:
            if s[1] >= h and s[2] + w <= PAGE:
                x = s[2]; s[2] += w
                return pi, x, s[0]
        top = sum(s[1] for s in sh)
        if top + h <= PAGE:
            sh.append([top, h, w])
            return pi, 0, top
    shelves.append([[0, h, w]])
    pages.append(Image.new("RGBA", (PAGE, PAGE), (0, 0, 0, 0)))
    return len(pages) - 1, 0, 0
for ci, fi, img, fu, fv in pieces:
    w, h = img.width + 2, img.height + 2
    pi, x, y = place(w, h)
    padded = Image.new("RGBA", (w, h))
    padded.paste(img.resize((w, h), Image.NEAREST), (0, 0))   # edges stretched into the border
    padded.paste(img, (1, 1))
    pages[pi].paste(padded, (x, y))
    a, b = (x + 1) / PAGE, (x + 1 + img.width) / PAGE
    c, d = (y + 1) / PAGE, (y + 1 + img.height) / PAGE
    if fu: a, b = b, a
    if fv: c, d = d, c
    placed[(ci, fi)] = (pi, (a, c, b, d))

sheet = Image.new("RGBA", (PAGE, PAGE * max(1, len(pages))), (0, 0, 0, 0))
for i, p in enumerate(pages):
    sheet.paste(p, (0, i * PAGE))
sheet.save(OUT_PNG, optimize=True)

# ---- animations ----
def num(v):
    try:
        return float(v)
    except (TypeError, ValueError):
        raise SystemExit(f"keyframe value {v!r}: only plain numbers are supported")

anims = []
for a in m.get("animations", []):
    channels = []
    for uuid, an in a.get("animators", {}).items():
        if uuid not in bone_of:
            continue
        by = {}
        for k in an.get("keyframes", []):
            ch = k["channel"]
            if ch not in ("position", "rotation", "scale"):
                continue
            dp = k["data_points"][0]
            v = [num(dp.get(ax, 0)) for ax in "xyz"]
            if flip_old and ch in ("position", "rotation"):
                v[0] = -v[0]
            if flip_old and ch == "rotation":
                v[1] = -v[1]
            interp = {"linear": 0, "catmullrom": 1, "step": 2}.get(k.get("interpolation", "linear"), 0)
            by.setdefault(ch, []).append((float(k["time"]), v, interp))
        for ch, keys in by.items():
            keys.sort(key=lambda k: k[0])
            channels.append((bone_of[uuid], ("position", "rotation", "scale").index(ch), keys))
    loop = {"once": 0, "hold": 1, "loop": 2}.get(a.get("loop", "once"), 0)
    anims.append((a["name"], float(a["length"]), loop, channels))

# ---- Rust ----
def fl(x):
    s = f"{float(x):.4f}".rstrip("0")
    return s + "0" if s.endswith(".") else s
def v3(v): return "[" + ", ".join(fl(x) for x in v) + "]"

out = [
    f"// Generated by tools/blockbench/bbmodel_to_rust.py from tools/blockbench/{KIND}.bbmodel.",
    "// Do not edit: change the model in Blockbench and run the script again.",
    "use crate::model::viewmodel::{Anim, Bone, Channel, Cube, Face, Key};",
    "",
    f"/// Texture pages (128x128 layers) in `{OUT_NAME}.png`, from top to bottom.",
    f"pub const PAGES: u32 = {max(1, len(pages))};",
    "",
    "pub static BONES: &[Bone] = &[",
]
for b in bones:
    out.append(f"    Bone {{ name: {json.dumps(b['name'])}, parent: {b['parent']}, origin: {v3(b['origin'])}, rot: {v3(b['rot'])} }},")
out += ["];", "", "pub static CUBES: &[Cube] = &["]
for ci, (bone, e) in enumerate(cubes):
    faces = []
    for fi in range(6):
        if (ci, fi) in placed:
            pi, uv = placed[(ci, fi)]
            faces.append(f"Face {{ page: {pi}, uv: {v3(uv)} }}")
        else:
            faces.append("Face::NONE")
    out.append(f"    Cube {{ name: {json.dumps(e['name'])}, bone: {bone}, from: {v3(e['from'])}, to: {v3(e['to'])}, origin: {v3(e.get('origin', [0, 0, 0]))}, "
               f"rot: {v3(e.get('rotation', [0, 0, 0]))}, faces: [{', '.join(faces)}] }},")
out += ["];", ""]

# ---- points the game needs, found from the cubes (bone index, point) ----
def cubes_named(pred):
    found = [(b, e) for b, e in cubes if pred(e["name"])]
    if not found:
        raise SystemExit("no cube for a point: check the cube names")
    return found
def bbox(found):
    lo = [min(min(e["from"][k], e["to"][k]) for _, e in found) for k in range(3)]
    hi = [max(max(e["from"][k], e["to"][k]) for _, e in found) for k in range(3)]
    return lo, hi
points = {}
if KIND == "pistol":
  barrel = cubes_named(lambda n: n.startswith("barrel_") and n != "barrel_hood" and not n.startswith("barrel_hood"))
  lo, hi = bbox(barrel)
  points["MUZZLE"] = (barrel[0][0], [(lo[0] + hi[0]) / 2, (lo[1] + hi[1]) / 2, lo[2]])
  case = cubes_named(lambda n: n == "chamber_case")
  lo, hi = bbox(case)
  points["EJECT"] = (case[0][0], [(lo[k] + hi[k]) / 2 for k in range(3)])
  can = cubes_named(lambda n: n.startswith("silencer_"))
  lo, hi = bbox(can)
  points["SILENCED"] = (can[0][0], [(lo[0] + hi[0]) / 2, (lo[1] + hi[1]) / 2, lo[2]])
  lens = cubes_named(lambda n: n == "laser_lens")
  lo, hi = bbox(lens)
  points["LASER"] = (lens[0][0], [(lo[0] + hi[0]) / 2, (lo[1] + hi[1]) / 2, lo[2]])
  lamp = cubes_named(lambda n: n == "flashlight_lens")
  lo, hi = bbox(lamp)
  points["LIGHT"] = (lamp[0][0], [(lo[0] + hi[0]) / 2, (lo[1] + hi[1]) / 2, lo[2]])
  docs = {"MUZZLE": "The middle of the barrel's front end.",
          "EJECT": "The round in the chamber: where a spent case comes out.",
          "SILENCED": "The middle of the silencer's front end (where the bullet leaves with one).",
          "LASER": "The laser sight's lens: where its beam starts.",
          "LIGHT": "The weapon light's lens: where its light comes from."}
  for k, (b, p) in points.items():
      out.append(f"/// {docs[k]} (bone, point)")
      out.append(f"pub const {k}: (usize, [f32; 3]) = ({b}, {v3(p)});")
out += ["", "pub static ANIMS: &[Anim] = &["]
for name, length, loop, channels in anims:
    out.append(f"    Anim {{ name: {json.dumps(name)}, length: {fl(length)}, looping: {loop}, channels: &[")
    for bone, kind, keys in channels:
        ks = ", ".join(f"Key {{ t: {fl(t)}, v: {v3(v)}, interp: {i} }}" for t, v, i in keys)
        out.append(f"        Channel {{ bone: {bone}, kind: {kind}, keys: &[{ks}] }}, // {bones[bone]['name']}")
    out.append("    ] },")
out += ["];", ""]
open(OUT_RS, "w", encoding="utf-8", newline="\n").write("\n".join(out))
print(f"{len(bones)} bones, {len(cubes)} cubes, {len(pieces)} faces on {len(pages)} pages, "
      f"{len(anims)} animations (format {'.'.join(map(str, ver))}{', old: flipped' if flip_old else ''})")
