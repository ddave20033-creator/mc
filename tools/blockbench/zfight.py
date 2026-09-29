"""Keeps a model's faces from flickering through each other (z-fighting): where two cubes'
faces lie in one plane, face the same way, overlap and can be seen (nothing is right in front
of them), one of them is drawn back into its cube a little (`INSET` pixels, less for a thin
one), so the other one is always in front. The cubes stay where they are to the eye; only which face wins changes.

Used by `bbmodel_to_rust.py` on every model it turns into game data (the .bbmodel files are
not changed). Run on its own to list what it finds:
    python tools/blockbench/zfight.py tools/blockbench/pistol.bbmodel
"""
import math
import numpy as np

# Closer than this (pixels), two faces count as one plane; the face that gives way goes this
# far back (small and far away, a model's faces must still be told apart).
PLANE = 0.03
INSET = 0.05
MIN_AREA = 1e-3
FACE_OF = {(0, 1): "east", (0, -1): "west", (1, 1): "up", (1, -1): "down", (2, 1): "south", (2, -1): "north"}


def rot_zyx(r):
    x, y, z = [math.radians(v) for v in r]
    rx = np.array([[1, 0, 0], [0, math.cos(x), -math.sin(x)], [0, math.sin(x), math.cos(x)]])
    ry = np.array([[math.cos(y), 0, math.sin(y)], [0, 1, 0], [-math.sin(y), 0, math.cos(y)]])
    rz = np.array([[math.cos(z), -math.sin(z), 0], [math.sin(z), math.cos(z), 0], [0, 0, 1]])
    return rz @ ry @ rx


def about(o, r):
    """Turn by `r` (degrees, ZYX) about `o`, as a 4x4 matrix."""
    m = np.eye(4)
    m[:3, :3] = rot_zyx(r)
    t, ti = np.eye(4), np.eye(4)
    t[:3, 3], ti[:3, 3] = o, -np.array(o, float)
    return t @ m @ ti


def bone_matrices(bones):
    """Each bone's rest transform (bones: dicts with parent, origin, rot; parents first)."""
    mats = []
    for b in bones:
        parent = mats[b["parent"]] if b["parent"] >= 0 else np.eye(4)
        mats.append(parent @ about(b["origin"], b["rot"]))
    return mats


def _faces(e, m):
    a, b = np.array(e["from"], float), np.array(e["to"], float)
    lo, hi = np.minimum(a, b), np.maximum(a, b)
    for ax in range(3):
        for side in (-1, 1):
            f = e.get("faces", {}).get(FACE_OF[(ax, side)])
            if not f or f.get("texture") is None:
                continue
            o = [k for k in range(3) if k != ax]
            pts = []
            for u, w in ((0, 0), (1, 0), (1, 1), (0, 1)):
                p = np.zeros(3)
                p[ax] = hi[ax] if side > 0 else lo[ax]
                p[o[0]] = hi[o[0]] if u else lo[o[0]]
                p[o[1]] = hi[o[1]] if w else lo[o[1]]
                pts.append((m @ np.append(p, 1))[:3])
            n = m[:3, :3] @ np.eye(3)[ax] * side
            yield ax, side, np.array(pts), n / np.linalg.norm(n)


def _area(poly):
    return sum(p[0] * q[1] - q[0] * p[1] for p, q in zip(poly, poly[1:] + poly[:1])) / 2 if len(poly) > 2 else 0.0


def _clip(poly, a, b):
    """The part of `poly` left of the edge a->b (Sutherland-Hodgman)."""
    side = lambda p: (b[0] - a[0]) * (p[1] - a[1]) - (b[1] - a[1]) * (p[0] - a[0])
    out = []
    for p, q in zip(poly, poly[1:] + poly[:1]):
        sp, sq = side(p), side(q)
        if sq >= -1e-9:
            if sp < -1e-9:
                out.append(p + (q - p) * (sp / (sp - sq)))
            out.append(q)
        elif sp >= -1e-9:
            out.append(p + (q - p) * (sp / (sp - sq)))
    return out


def _overlap(pa, pb, n):
    """How much of two faces in one plane overlap (area), and the middle of it."""
    u = np.cross(n, [1, 0, 0]) if abs(n[0]) < 0.9 else np.cross(n, [0, 1, 0])
    u /= np.linalg.norm(u)
    v = np.cross(n, u)
    flat = lambda pts: [np.array([p @ u, p @ v]) for p in pts]
    ccw = lambda poly: poly if _area(poly) > 0 else poly[::-1]
    a, b = ccw(flat(pa)), ccw(flat(pb))
    poly = a
    for p, q in zip(b, b[1:] + b[:1]):
        poly = _clip(poly, p, q)
        if not poly:
            return 0.0, None
    c = sum(poly) / len(poly)
    return abs(_area(poly)), c[0] * u + c[1] * v + (n @ pa[0]) * n


def find(cubes, bones):
    """The flickering pairs: (area, the cube that should give way and the other one, each as
    (index, axis, side) of its face). `cubes`: (bone index, element) pairs, as drawn."""
    mats = bone_matrices(bones)
    placed = [mats[bi] @ about(e.get("origin", [0, 0, 0]), e.get("rotation", [0, 0, 0])) for bi, e in cubes]
    solids = []
    for (bi, e), m in zip(cubes, placed):
        a, b = np.array(e["from"], float), np.array(e["to"], float)
        solids.append((np.linalg.inv(m), np.minimum(a, b), np.maximum(a, b)))
    faces = [(ci, ax, side, pts, n) for ci, ((_, e), m) in enumerate(zip(cubes, placed)) for ax, side, pts, n in _faces(e, m)]

    def covered(p, skip):
        for k, (inv, lo, hi) in enumerate(solids):
            if k in skip:
                continue
            q = (inv @ np.append(p, 1))[:3]
            if np.all(q > lo + 1e-4) and np.all(q < hi - 1e-4):
                return True
        return False

    hits = []
    for i in range(len(faces)):
        ci, axi, si, pi, ni = faces[i]
        di = ni @ pi[0]
        for j in range(i + 1, len(faces)):
            cj, axj, sj, pj, nj = faces[j]
            if ci == cj or ni @ nj < 0.9999 or abs(di - ni @ pj[0]) > PLANE:
                continue
            area, mid = _overlap(pi, pj, ni)
            if area <= MIN_AREA or covered(mid + ni * 0.04, (ci, cj)):
                continue
            # The smaller face gives way (it is the one mostly inside the other).
            a, b = (ci, axi, si), (cj, axj, sj)
            hits.append((area, a, b) if _area_of(pi) <= _area_of(pj) else (area, b, a))
    return hits


def _area_of(pts):
    return np.linalg.norm(np.cross(pts[1] - pts[0], pts[3] - pts[0]))


def _draw_back(cubes, face):
    """Moves a cube's face into it (a thin part by a share of its thickness). False if it is
    too thin for that."""
    ci, ax, side = face
    e = cubes[ci][1]
    a, b = e["from"], e["to"]
    # (neighbours, like the pieces of a ring, go back by different amounts, or they would
    # meet in one plane again)
    step = min(INSET * (1.0 + 0.5 * (ci % 3)), abs(b[ax] - a[ax]) * 0.3)
    if step < 0.005:
        return False
    key = "to" if (side > 0) == (b[ax] > a[ax]) else "from"
    e[key] = list(e[key])
    e[key][ax] += -step * side
    return True


def resolve(cubes, bones, rounds=10):
    """Draws back the faces that would flicker (changes the elements' from/to in place):
    the smaller of each pair, or the other one when that one is too thin. Returns how many
    were drawn back."""
    done = 0
    for _ in range(rounds):
        moved = set()
        for _, first, second in find(cubes, bones):
            if first in moved or second in moved:
                continue
            for face in (first, second):
                if _draw_back(cubes, face):
                    moved.add(face)
                    done += 1
                    break
        if not moved:
            break
    return done


if __name__ == "__main__":
    import json, sys
    for path in sys.argv[1:]:
        m = json.load(open(path, encoding="utf-8"))
        els = {e["uuid"]: e for e in m["elements"]}
        bones, cubes = [], []
        def walk(node, parent, hidden):
            if isinstance(node, str):
                e = els.get(node)
                if e and e.get("type", "cube") == "cube" and not hidden and e.get("visibility", True):
                    cubes.append((parent, e))
                return
            i = len(bones)
            bones.append(dict(parent=parent, origin=node.get("origin", [0, 0, 0]), rot=node.get("rotation", [0, 0, 0])))
            for c in node.get("children", []):
                walk(c, i, hidden or node["name"] in ("right_arm_mesh", "left_arm_mesh", "spent_case") or node.get("visibility") is False)
        for n in m["outliner"]:
            walk(n, -1, False)
        hits = find(cubes, bones)
        print(f"{path}: {len(hits)} flickering")
        for area, (ci, ax, side), (other, _, _) in sorted(hits, key=lambda h: -h[0])[:40]:
            print(f"  {area:6.2f}px2  {cubes[ci][1]['name']} ({'xyz'[ax]}{'+' if side > 0 else '-'}) <-> {cubes[other][1]['name']}")
