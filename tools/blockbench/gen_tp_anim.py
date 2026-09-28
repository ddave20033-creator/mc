"""Generates tp_<gun>_anim.bbmodel: the game's player (`model/player.rs`) with elbows and knees,
holding a gun, to pose and animate in Blockbench.

Run:  python tools/blockbench/gen_tp_anim.py            (the pistol)
      python tools/blockbench/gen_tp_anim.py revolver   (or ak)

The player (model pixels, 1 px = 1.8 / 32 = 0.05625 blocks; standing on the origin, facing -Z),
the game's sizes, the arms and legs split in two halves:
  head       8 x 8 x 8   y 24 .. 32, turns about the neck      (0, 24, 0)
  torso      8 x 12 x 4  y 12 .. 24, turns about the hips      (0, 12, 0)
  right_arm  4 x 6 x 4   y 18 .. 24, turns about the shoulder  (5, 22, 0)
  right_hand 4 x 6 x 4   y 12 .. 18, bends at the elbow        (5, 18, 0)
  left_arm / left_hand   the same at x -8 .. -4               (-5, 22, 0), (-5, 18, 0)
  right_leg  4 x 6 x 4   y 6 .. 12, turns about the hip        (1.9, 12, 0)
  right_foot 4 x 6 x 4   y 0 .. 6, bends at the knee           (1.9, 6, 0)
  left_leg / left_foot   the same at x -3.9 .. 0.1             (-1.9, 12, 0), (-1.9, 6, 0)
  gun        in the right fist, carried by the right forearm (6, 13.2, 0)
The whole body hangs from `body` (the hips) so it can be lowered and moved.
"""
import base64, json, math, os, struct, sys, uuid, zlib

from gen_tp import GUNS, gun_cubes, euler, mv, mm, add, sub, kf, R, P

HERE = os.path.dirname(os.path.abspath(__file__))
PX = 1.8 / 32.0
LENGTH = {"pistol": 10.5, "revolver": 11.0, "ak": 22.0}     # the gun's length on the player, px

def gun_length(cfg):
    """The gun's length (z) in its own model's pixels."""
    cubes, _, _, _ = gun_cubes(cfg["src"], cfg["root"], [0, 0, 0], 1.0, 1.0)
    return max(max(e["from"][2], e["to"][2]) for e in cubes) - min(min(e["from"][2], e["to"][2]) for e in cubes)

# ---------- the skin: the game's character textures (`procedural::character`), one texel a cell ----------
HAIR, SKIN, SHIRT, PANTS = (66, 42, 24), (206, 150, 112), (40, 150, 162), (56, 64, 150)
SHOE, WHITE, IRIS, MOUTH = (86, 86, 92), (238, 238, 240), (62, 84, 170), (120, 66, 52)
SLOTS = ["face", "head_side", "hair", "skin", "shirt_front", "shirt", "arm", "sleeve", "leg", "sole"]

def shade(c, v): return tuple(max(0, min(255, round(x * v))) for x in c)

def cell(slot, cx, cy):
    n = 0.95 + 0.07 * (((cx * 7 + cy * 13 + SLOTS.index(slot) * 5) % 5) / 4.0)
    if slot == "face":
        if cy <= 1: c = shade(HAIR, 1.0)
        elif cy == 2 and cx in (0, 7): c = shade(HAIR, 0.9)
        elif cy == 4 and cx in (1, 6): return WHITE
        elif cy == 4 and cx in (2, 5): return IRIS
        elif cy == 5 and cx in (3, 4): c = shade(SKIN, 0.86)
        elif cy == 6 and cx in (3, 4): c = MOUTH
        else: c = SKIN
    elif slot == "head_side":
        if cy <= 1: c = HAIR
        elif (cy == 2 and cx >= 4) or (cy == 3 and cx >= 6): c = shade(HAIR, 0.92)
        elif (cx, cy) == (3, 4): c = shade(SKIN, 0.84)
        else: c = shade(SKIN, 0.97)
    elif slot == "hair": c = shade(HAIR, 0.95)
    elif slot == "skin": c = SKIN
    elif slot == "shirt_front": c = shade(SKIN, 0.95) if cy == 0 and cx in (3, 4) else SHIRT
    elif slot == "shirt": c = shade(SHIRT, 0.95)
    elif slot == "arm": c = shade(SHIRT, 0.95) if cy <= 2 else SKIN
    elif slot == "sleeve": c = shade(SHIRT, 0.9)
    elif slot == "leg": c = PANTS if cy <= 6 else SHOE
    else: c = shade(SHOE, 0.8)
    return shade(c, n)

def skin_png():
    W, H = 8 * len(SLOTS), 8
    rows = []
    for y in range(H):
        row = b"\x00"
        for x in range(W):
            row += bytes(cell(SLOTS[x // 8], x % 8, y)) + b"\xff"
        rows.append(row)
    def chunk(t, d): return struct.pack(">I", len(d)) + t + d + struct.pack(">I", zlib.crc32(t + d) & 0xffffffff)
    return (b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", W, H, 8, 6, 0, 0, 0))
            + chunk(b"IDAT", zlib.compress(b"".join(rows), 9)) + chunk(b"IEND", b"")), W, H

def uv(slot, flip=False, rows=(0, 8)):
    u = SLOTS.index(slot) * 8
    return [u + 8, rows[0], u, rows[1]] if flip else [u, rows[0], u + 8, rows[1]]

def part(name, frm, to, faces, rows=(0, 8)):
    """A cube of the player: `faces` the skin slot of north (front), south, east, west, up, down
    (None: no face, where it joins the next part); its sides show `rows` of the skin."""
    names = ("north", "south", "east", "west", "up", "down")
    f = {}
    for fname, slot in zip(names, faces):
        if slot is None:
            f[fname] = {"uv": [0, 0, 0, 0], "texture": None}
            continue
        flip = fname == "east" and slot == "head_side"      # the hair toward the back on both sides
        f[fname] = {"uv": uv(slot, flip, rows if fname in ("north", "south", "east", "west") else (0, 8)), "texture": 1}
    return {"name": name, "type": "cube", "uuid": str(uuid.uuid4()), "box_uv": False, "rescale": False,
            "from": frm, "to": to, "origin": [(a + b) / 2 for a, b in zip(frm, to)], "rotation": [0, 0, 0],
            "faces": f}

def group(name, children, origin):
    kids = [c["uuid"] if c.get("type") == "cube" else c for c in children]
    return {"name": name, "uuid": str(uuid.uuid4()), "origin": list(origin), "rotation": [0, 0, 0],
            "isOpen": True, "visibility": True, "export": True, "children": kids}

# ---------- small maths: 3x3 rotations (game space: right-handed, Y up, facing -Z) ----------
def T(m): return [[m[j][i] for j in range(3)] for i in range(3)]
def norm(v):
    n = math.sqrt(sum(x * x for x in v)) or 1.0
    return [x / n for x in v]
def dot(a, b): return sum(p * q for p, q in zip(a, b))
def cross(a, b): return [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
def scale(v, k): return [x * k for x in v]
def cols(x, y, z): return [[x[0], y[0], z[0]], [x[1], y[1], z[1]], [x[2], y[2], z[2]]]

def to_euler(m):
    """Degrees (x, y, z) with m = Rz.Ry.Rx, as Blockbench and the game turn a group."""
    y = math.asin(max(-1.0, min(1.0, -m[2][0])))
    if abs(m[2][0]) < 0.9999:
        x, z = math.atan2(m[2][1], m[2][2]), math.atan2(m[1][0], m[0][0])
    else:
        x, z = math.atan2(-m[1][2], m[1][1]), 0.0
    return [math.degrees(x), math.degrees(y), math.degrees(z)]

def aim_limb(down, front):
    """The turn that points a limb's -Y (along it, from its joint) along `down`, its -Z (its front
    face) as near `front` as it can."""
    y = scale(norm(down), -1.0)
    z = norm(sub(scale(y, dot(front, y)), front))       # -front, square to the limb
    x = cross(y, z)
    return cols(x, y, z)

def ik(root, target, l1, l2, pole):
    """Two bones from `root` reaching for `target` (lengths l1, l2), the joint bending toward
    `pole`: where the joint goes."""
    d = sub(target, root)
    dist = max(1e-4, min(l1 + l2 - 1e-3, math.sqrt(dot(d, d))))
    dn = norm(d)
    a = (l1 * l1 + dist * dist - l2 * l2) / (2 * l1 * dist)      # cos of the angle at the root
    a = max(-1.0, min(1.0, a))
    side = norm(sub(pole, scale(dn, dot(pole, dn))))
    return add(root, add(scale(dn, l1 * a), scale(side, l1 * math.sqrt(1 - a * a))))

# ---------- the rig: the game's player with its arms and legs split at the elbows and knees ----------
# Every bone: its parent and its pivot.
BONES = {
    "body":        (None,          (0, 12, 0)),
    "torso":       ("body",        (0, 12, 0)),
    "head":        ("torso",       (0, 24, 0)),
    "right_arm":   ("torso",       (5, 22, 0)),
    "right_hand":  ("right_arm",   (5, 18, 0)),
    "left_arm":    ("torso",       (-5, 22, 0)),
    "left_hand":   ("left_arm",    (-5, 18, 0)),
    "right_leg":   ("body",        (1.9, 12, 0)),
    "right_foot":  ("right_leg",   (1.9, 6, 0)),
    "left_leg":    ("body",        (-1.9, 12, 0)),
    "left_foot":   ("left_leg",    (-1.9, 6, 0)),
    "gun":         ("right_hand",  (6, 13.2, 0)),
}
ARM_UP, ARM_LOW = 4.0, 5.0          # shoulder to elbow, elbow to the middle of the fist
LEG_UP, LEG_LOW = 6.0, 6.0          # hip to knee, knee to the sole
FIST = {"right": [1.0, 0, 0], "left": [-1.0, 0, 0]}   # the fist's middle, from the arm's line
GUN_REST = [-90, 0, 0]              # the gun lies along the hanging forearm at rest

OVERLAP = 2.0          # how far an arm's or a leg's upper half reaches past the joint
INSET = 0.04           # the lower half a hair thinner, so the two never flicker where they overlap

def limb(name, x0, x1, top, bottom, joint, lower, sides, cap_top, cap_bottom):
    """One half of an arm or a leg (12 long, from `top` down to `bottom`). The upper half reaches
    OVERLAP past the joint, filling the gap that opens on the outside of the bend; the lower one
    starts right at the joint, so nothing of it sticks out when it bends. The sides show their
    own rows of the skin."""
    if lower:
        y0, y1, i = bottom, joint, INSET
    else:
        y0, y1, i = joint - OVERLAP, top, 0.0
    rows = ((top - y1) / (top - bottom) * 8, (top - y0) / (top - bottom) * 8)
    return part(name, [x0 + i, y0, -2 + i], [x1 - i, y1, 2 - i], [sides] * 4 + [cap_top, cap_bottom], rows)

def cubes_of_player():
    return {
        "torso": part("torso", [-4, 12, -2], [4, 24, 2], ["shirt_front", "shirt", "shirt", "shirt", "shirt", "leg"]),
        "head": part("head", [-4, 24, -4], [4, 32, 4], ["face", "hair", "head_side", "head_side", "hair", "skin"]),
        "right_arm": limb("right_arm", 4, 8, 24, 12, 18, False, "arm", "sleeve", "skin"),
        "right_hand": limb("right_hand", 4, 8, 24, 12, 18, True, "arm", "skin", "skin"),
        "left_arm": limb("left_arm", -8, -4, 24, 12, 18, False, "arm", "sleeve", "skin"),
        "left_hand": limb("left_hand", -8, -4, 24, 12, 18, True, "arm", "skin", "skin"),
        "right_leg": limb("right_leg", -0.1, 3.9, 12, 0, 6, False, "leg", "leg", "leg"),
        "right_foot": limb("right_foot", -0.1, 3.9, 12, 0, 6, True, "leg", "leg", "sole"),
        "left_leg": limb("left_leg", -3.9, 0.1, 12, 0, 6, False, "leg", "leg", "leg"),
        "left_foot": limb("left_foot", -3.9, 0.1, 12, 0, 6, True, "leg", "leg", "sole"),
    }

class Pose:
    """World rotations and positions of the bones, from the root down, turned into what
    Blockbench keys: each bone's rotation against its parent and how far its pivot moved."""
    def __init__(self):
        self.rot, self.at = {}, {}              # world rotation, world pivot

    def set(self, bone, rot, at=None):
        parent, pivot = BONES[bone]
        if at is None:                          # carried by its parent
            at = self.point(parent, pivot)
        self.rot[bone], self.at[bone] = rot, at

    def point(self, bone, p):
        """A point of `bone` (given at rest) where it is now."""
        if bone is None:
            return list(p)
        return add(self.at[bone], mv(self.rot[bone], sub(p, BONES[bone][1])))

    def keys(self):
        out = {}
        for bone, (parent, pivot) in BONES.items():
            if bone not in self.rot:
                continue
            prot = self.rot[parent] if parent else [[1, 0, 0], [0, 1, 0], [0, 0, 1]]
            local = mm(T(prot), self.rot[bone])
            e = to_euler(local)
            if bone == "gun":
                e = [((v - r + 180) % 360) - 180 for v, r in zip(e, GUN_REST)]
            # where the pivot is against where the parent carries it
            carried = self.point(parent, pivot) if parent else list(pivot)
            moved = mv(T(prot), sub(self.at[bone], carried))
            out[bone] = (moved, e)
        return out

def rot_y(deg): return euler([0, deg, 0])

# ---------- the poses ----------
# The upper body holds the pistol as the gun mods do (TaCZ's pistol, MrCrayfish's guns): a Weaver
# stance, the body a little turned, the gun side's shoulder back, the head facing ahead; the
# right hand on the grip, the left one cupping it, both elbows bent and out. Aimed, the sights
# come up to the right eye and the head tips toward them; running, the pistol comes down to the
# chest, pointed at the ground ahead. The legs step with bending knees; the timing is the game's:
# one full stride every 3.33 blocks walked at its speeds (walking 4.3, running 5.6, sneaking 1.3;
# aiming slows to 0.6 of them).
STRIDE = 2.0 / 0.6
def gait(speed): return STRIDE / speed

# name, length, stride (how far a foot goes forward and back, px), crouch, aim, sprint
ANIMS = [
    ("idle",            3.0,               0.0, 0, 0, 0),
    ("idle_aim",        3.0,               0.0, 0, 1, 0),
    ("walk",            gait(4.3),         4.5, 0, 0, 0),
    ("walk_aim",        gait(4.3 * 0.6),   3.0, 0, 1, 0),
    ("sprint",          gait(5.6),         6.5, 0, 0, 1),
    ("sprint_aim",      gait(5.6),         5.0, 0, 1, 1),   # the game stops running to aim
    ("crouch_walk",     gait(1.3),         3.0, 1, 0, 0),
    ("crouch_walk_aim", gait(1.3 * 0.6),   2.5, 1, 1, 0),
]
SAMPLES = 12

def upper(cfg, c, a, s):
    """The held pose: the torso's turn and lean, where the grip goes (from the chest), the gun's
    turn, the head's tilt."""
    sight = cfg["sight"] * cfg["scale"]
    if a:
        return dict(turn=-14, lean=4 + 6 * c, grip=[1.8, 27.6 - sight, -9.0], gun=[0, 0, 0], tilt=-7)
    if s:
        return dict(turn=-6, lean=14, grip=[0.6, 16.2, -5.5], gun=[-50, 6, 0], tilt=0)
    return dict(turn=-16, lean=5 + 6 * c, grip=[1.0, 19.2, -7.0], gun=[-14, 0, 0], tilt=0)

def solve(cfg, support, t, length, stride, c, a, s):
    ph = 2 * math.pi * t / length
    moving = stride > 0
    sin, cos = math.sin(ph), math.cos(ph)
    breath = 0.0 if moving else 0.5 - 0.5 * math.cos(ph)
    p = Pose()

    # the feet: planted a little apart (the right one back), stepping in turn; the hips drop so
    # the legs never have to stretch, and bob with the steps
    lift = 1.5 + 1.5 * s
    feet = {}
    for side, sx, off, base in (("right", 1.9, 0.0, 1.2), ("left", -1.9, math.pi, -1.2)):
        q = ph + off
        z = -stride * math.cos(q) if moving else base
        y = lift * max(0.0, math.sin(q)) if moving else 0.0
        feet[side] = [sx * 1.15, y, z + (base * 0.5 if moving else 0)]
    crouch_drop = 3.6 * c
    reach_z = max(abs(feet["right"][2]), abs(feet["left"][2]))
    hip_y = min(11.6, math.sqrt(max(0.0, 144 - reach_z * reach_z)) - 0.3) - crouch_drop
    if moving:
        hip_y -= (0.2 + 0.4 * s) * (0.5 + 0.5 * math.cos(2 * ph))
    hip_y += 0.1 * breath
    body_at = [0, hip_y, 0.4 * c]
    p.set("body", rot_y(0), body_at)

    # the torso: turned into the stance, leaning forward, twisting a little with the steps
    u = upper(cfg, c, a, s)
    twist = (2.0 + 3.0 * s) * sin if moving else 0.0
    torso = mm(rot_y(u["turn"] + twist), euler([-u["lean"], 0, 0]))
    p.set("torso", torso)

    # the head: looking ahead whatever the torso does, tipped toward the sights when aiming
    p.set("head", euler([0, 0, u["tilt"]]))

    # the gun: where the grip goes, from the chest (the torso's top as it would be, unturned)
    chest_y = p.point("torso", (0, 24, 0))[1] - 24
    grip = add(u["grip"], [0, chest_y + (hip_y - 12) * 0.0, 0])
    if moving:                                  # the steps shake it a little
        k = 0.35 if a else 1.0
        grip = add(grip, [0.3 * k * sin, -(0.2 + 0.3 * s) * k * (1 - math.cos(2 * ph)), 0])
    grip = add(grip, [0, 0.12 * breath, 0])
    grot = list(u["gun"])
    if moving:
        k = 0.4 if a else 1.0
        grot = add(grot, [-(1.5 + 2 * s) * k * math.sin(2 * ph), 0, -(2 + 2 * s) * k * sin])
    gun = euler(grot)
    up = mv(gun, [0, 1, 0])

    # the arms: two bones each, the fists on the gun, the elbows bent down and out
    for side, target, out in (("right", grip, 1.0), ("left", add(grip, mv(gun, support)), -1.0)):
        arm, hand = f"{side}_arm", f"{side}_hand"
        shoulder = p.point("torso", BONES[arm][1])
        fist_off = FIST[side]
        end = target
        for _ in range(4):                      # the fist's middle is off the arm's line
            pole = [out * 1.0, -1.2, 0.4]
            elbow = ik(shoulder, end, ARM_UP, ARM_LOW, pole)
            ra = aim_limb(sub(elbow, shoulder), up)
            rh = aim_limb(sub(end, elbow), up)
            end = sub(target, mv(rh, fist_off))
        p.set(arm, ra, shoulder)
        p.set(hand, rh)
    # the gun in the right fist, as it is aimed
    p.set("gun", gun, p.point("right_hand", BONES["gun"][1]))

    # the legs: two bones each, the knees forward
    for side in ("right", "left"):
        leg, foot = f"{side}_leg", f"{side}_foot"
        hip = p.point("body", BONES[leg][1])
        sole = feet[side]
        knee = ik(hip, sole, LEG_UP, LEG_LOW, [0.15 * (1 if side == "right" else -1), 0, -1])
        p.set(leg, aim_limb(sub(knee, hip), [0, 0, -1]), hip)
        p.set(foot, aim_limb(sub(sole, knee), [0, 0, -1]))
    return p


def build(name, with_anims):
    cfg = dict(GUNS[name])
    cfg["scale"] = LENGTH[name] / gun_length(cfg)
    grip = list(BONES["gun"][1])
    cubes, tex, res, support = gun_cubes(cfg["src"], cfg["root"], grip, cfg["scale"], cfg["thick"])

    parts = cubes_of_player()
    groups = {}
    def make(bone):
        kids = [make(b) for b, (par, _) in BONES.items() if par == bone]
        own = [parts[bone]] if bone in parts else []
        if bone == "gun":
            own = cubes
        g = group(bone, own + kids, BONES[bone][1])
        if bone == "gun":
            g["rotation"] = list(GUN_REST)
        groups[bone] = g
        return g
    root = make("body")
    elements = list(parts.values()) + cubes

    anims = []
    for (an, length, stride, c, a, s) in (ANIMS if with_anims else ANIMS[:1]):
        times = [round(length * i / SAMPLES, 4) for i in range(SAMPLES + 1)]
        keys = {}
        for t in times:
            for bone, (moved, rot) in solve(cfg, support, t, length, stride, c, a, s).keys().items():
                k = keys.setdefault(bone, [])
                if bone == "body" or any(abs(v) > 1e-3 for v in moved):
                    k.append(kf("position", t, P([round(v, 3) for v in moved])))
                k.append(kf("rotation", t, R([round(v, 3) for v in rot])))
        animators = {groups[b]["uuid"]: {"name": b, "type": "bone", "keyframes": k} for b, k in keys.items() if k}
        anims.append({"uuid": str(uuid.uuid4()), "name": an, "loop": "loop", "override": False,
                      "length": round(length, 4), "snapping": 60, "selected": an == "idle",
                      "anim_time_update": "", "blend_weight": "", "start_delay": "", "loop_delay": "",
                      "animators": animators})

    png, W, H = skin_png()
    model = {
        "meta": {"format_version": "4.10", "model_format": "free", "box_uv": False},
        "name": f"tp_{name}_anim",
        "resolution": res,
        "elements": elements,
        "outliner": [root],
        "textures": [
            dict(tex, uuid=str(uuid.uuid4())),
            {"name": "player.png", "id": "1", "uuid": str(uuid.uuid4()), "folder": "", "namespace": "",
             "width": W, "height": H, "uv_width": W, "uv_height": H, "particle": False,
             "render_mode": "default", "visible": True, "saved": False,
             "source": "data:image/png;base64," + base64.b64encode(png).decode()},
        ],
        "animations": anims,
    }
    out = os.path.join(HERE, f"tp_{name}_anim.bbmodel")
    with open(out, "w") as f:
        json.dump(model, f, indent=1)
    print(f"{os.path.basename(out)}: {len(cubes)} gun cubes, {len(anims)} animations")

if __name__ == "__main__":
    args = [a for a in sys.argv[1:] if not a.startswith("-")]
    for name in args or ["pistol"]:
        build(name, "--all" in sys.argv)
