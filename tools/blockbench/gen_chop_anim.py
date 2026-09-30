"""Generates chop.bbmodel: the game's player (`model/player.rs`, the same rig as
`gen_tp_anim.py`: elbows and knees) holding an axe in both hands, a tree's trunk in front of
him to aim at, a `chop` animation (a level swing from the right side into the trunk: drawn back, swung
round, the edge biting in, stuck a moment, pulled out) and a `stump` one (the axe raised over
the head and brought straight down into a stump on the ground) to work on in Blockbench.

Run:  python tools/blockbench/gen_chop_anim.py

Model pixels as in the game (1 px = 1.8 / 32 = 0.05625 blocks), the player standing on the
origin facing -Z. The axe hangs from the right fist (`axe`, pivot (6, 13.2, 0)): its handle
along its +Y from the grip (0) to the head (14 px up), its edge toward its -Z. The trunk
(`tree`, only to see where the axe goes; not part of the player) is the game's round trunk,
0.88 blocks across.
"""
import base64, json, math, os, struct, uuid, zlib

import gen_tp_anim as tp
from gen_tp import add, sub, mv, mm, euler, kf, R, P
from gen_tp_anim import Pose, part, group, ik, aim_limb, cols, norm, cross, scale, dot, cubes_of_player, skin_png

HERE = os.path.dirname(os.path.abspath(__file__))
PX = 1.8 / 32.0

# The rig: the gun's bone becomes the axe's.
BONES = tp.BONES
BONES.pop("gun", None)
BONES["axe"] = ("right_hand", (6, 13.2, 0))

# ---------- a small texture: the handle's wood, the head's steel, the trunk's bark and rings ----------
COLORS = [(122, 86, 52), (176, 180, 188), (96, 72, 44), (184, 150, 100), (228, 230, 236)]
WOOD, STEEL, BARK, RINGS, EDGE = range(5)

def colors_png():
    W, H = len(COLORS), 1
    row = b"\x00" + b"".join(bytes(c) + b"\xff" for c in COLORS)
    def chunk(t, d): return struct.pack(">I", len(d)) + t + d + struct.pack(">I", zlib.crc32(t + d) & 0xffffffff)
    return (b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", W, H, 8, 6, 0, 0, 0))
            + chunk(b"IDAT", zlib.compress(row, 9)) + chunk(b"IEND", b"")), W, H

def box(name, frm, to, color, origin=None, rotation=(0, 0, 0), faces=None):
    """A cube painted one color of `COLORS` (per face if `faces` is given: north, south, east,
    west, up, down)."""
    names = ("north", "south", "east", "west", "up", "down")
    faces = faces or [color] * 6
    f = {n: {"uv": [c, 0, c + 1, 1], "texture": 0} for n, c in zip(names, faces)}
    return {"name": name, "type": "cube", "uuid": str(uuid.uuid4()), "box_uv": False, "rescale": False,
            "from": list(frm), "to": list(to), "origin": list(origin or [(a + b) / 2 for a, b in zip(frm, to)]),
            "rotation": list(rotation), "faces": f}

GRIP = BONES["axe"][1]

def axe_cubes():
    """The axe at rest, its grip on the fist's middle: the handle up along +Y, the head at its
    top with the blade toward -Z (its edge brighter) and the poll behind."""
    g = GRIP
    def at(x0, y0, z0, x1, y1, z1): return [g[0] + x0, g[1] + y0, g[2] + z0], [g[0] + x1, g[1] + y1, g[2] + z1]
    return [
        box("handle", *at(-0.6, -2.5, -0.6, 0.6, 15.0, 0.6), WOOD),
        box("head", *at(-0.9, 11.0, -3.2, 0.9, 14.5, 1.0), STEEL),
        box("blade", *at(-0.5, 10.0, -5.6, 0.5, 15.5, -3.2), STEEL,
            faces=[EDGE, STEEL, STEEL, STEEL, STEEL, STEEL]),
        box("poll", *at(-1.0, 11.6, 1.0, 1.0, 13.9, 2.2), STEEL),
    ]

# ---------- the swing ----------
# The axe's state: which way its handle points (yaw round the body, 0 ahead, + to the left;
# pitch up), how far out from the chest the grip is, and the torso's turn. Its edge faces
# square to the handle, level (the way the head sweeps).
def axe_frame(yaw, pitch):
    y, p = math.radians(yaw), math.radians(pitch)
    h = [-math.sin(y) * math.cos(p), math.sin(p), -math.cos(y) * math.cos(p)]
    edge = norm([h[2], 0.0, -h[0]])          # level, square to the handle, leading the sweep
    return h, edge

CHEST = [0.5, 17.0, 0.0]

# The swing's key poses: (time, (handle yaw, handle pitch, grip out from the chest, grip
# height, torso turn, hips turn, hips down, weight toward the left foot)). The body leads
# the arms: the hips start round first, the torso follows, the axe comes last and fastest.
KEYS = [
    (0.00, (-22.0, 52.0, 6.0, 15.0, 0.0, 0.0, 0.0, 0.0)),      # held at rest, head up in front
    (0.16, (-70.0, 36.0, 6.8, 17.5, -16.0, -5.0, 0.3, -0.3)),  # drawing back
    (0.28, (-102.0, 22.0, 7.2, 18.5, -27.0, -9.0, 0.6, -0.6)), # drawn back, weight on the right
    (0.33, (-50.0, 10.0, 7.8, 18.0, -8.0, 0.0, 0.8, 0.0)),     # swinging round
    (0.38, (26.0, 3.0, 8.0, 17.5, 12.0, 6.0, 0.9, 0.6)),       # bites into the trunk
    (0.41, (24.0, 3.5, 7.9, 17.5, 10.5, 5.5, 0.9, 0.55)),      # the jolt of it
    (0.52, (25.0, 3.0, 8.0, 17.5, 11.0, 5.5, 0.85, 0.55)),     # stuck
    (0.62, (14.0, 10.0, 7.0, 17.0, 6.0, 3.0, 0.5, 0.3)),       # pulled out
    (0.80, (-22.0, 52.0, 6.0, 15.0, 0.0, 0.0, 0.0, 0.0)),      # back at rest
]

def state_at(t):
    """The swing's pose at `t` seconds: between the keys smoothly (a Catmull-Rom curve through
    them), the stroke into the trunk speeding up."""
    for i, ((t0, a), (t1, b)) in enumerate(zip(KEYS, KEYS[1:])):
        if t <= t1:
            k = (t - t0) / (t1 - t0)
            if (t0, t1) == (0.33, 0.38):
                k = k ** 1.6
            pa = KEYS[max(i - 1, 0)][1]
            pb = KEYS[min(i + 2, len(KEYS) - 1)][1]
            k2, k3 = k * k, k * k * k
            return [0.5 * (2 * y + (-x + z) * k + (2 * x - 5 * y + 4 * z - w) * k2 + (-x + 3 * y - 3 * z + w) * k3)
                    for x, y, z, w in zip(pa, a, b, pb)]
    return list(KEYS[-1][1])

LENGTH, SAMPLES = 0.8, 40

def pose_at(t):
    yaw, pitch, reach, height, turn, hips, dip, weight = state_at(t)
    h, edge = axe_frame(yaw, pitch)
    p = Pose()
    p.set("body", euler([0, hips, 0]), [weight, 12 - dip, 0])
    p.set("torso", euler([2.0 + dip, turn - hips, 0]))
    # The head calm: it keeps looking at the trunk (a little down) whatever the body does,
    # only nodding a touch as the axe bites.
    nod = 1.5 * max(0.0, 1.0 - abs(t - 0.40) / 0.06)
    p.set("head", euler([-6.0 - nod, -turn * 0.9, 0]))
    # The grip: out from the chest the way the handle points (level), at the height asked.
    flat = norm([h[0], 0.0, h[2]])
    grip = add([CHEST[0] + weight, height - dip, CHEST[2]], scale(flat, reach))
    # The axe's own turn: its +Y along the handle, its -Z along the edge.
    y_ax = norm(h)
    z_ax = scale(edge, -1.0)
    x_ax = cross(y_ax, z_ax)
    rot = cols(x_ax, y_ax, z_ax)
    # Both hands on the handle: the right at the grip's end, the left higher up.
    left_on = add(grip, scale(y_ax, 5.0))
    for side, target, out in (("right", grip, 1.0), ("left", left_on, -1.0)):
        arm, hand = f"{side}_arm", f"{side}_hand"
        shoulder = p.point("torso", BONES[arm][1])
        fist_off = tp.FIST[side]
        end = target
        for _ in range(4):
            pole = [out * 1.0, -1.0, 0.6]
            elbow = ik(shoulder, end, tp.ARM_UP, tp.ARM_LOW, pole)
            ra = aim_limb(sub(elbow, shoulder), y_ax)
            rh = aim_limb(sub(end, elbow), y_ax)
            end = sub(target, mv(rh, fist_off))
        p.set(arm, ra, shoulder)
        p.set(hand, rh)
    p.set("axe", rot, grip)
    # The feet planted apart (the left one forward), the knees giving a little as the hips go
    # down.
    for side, foot_at in (("right", [2.6, 0, 1.5]), ("left", [-2.6, 0, -1.5])):
        leg, foot = f"{side}_leg", f"{side}_foot"
        hip = p.point("body", BONES[leg][1])
        knee = ik(hip, foot_at, tp.LEG_UP, tp.LEG_LOW, [0, 0, -1])
        p.set(leg, aim_limb(sub(knee, hip), [0, 0, -1]), hip)
        p.set(foot, aim_limb(sub(foot_at, knee), [0, 0, -1]))
    return p, grip, rot

# ---------- the stump: raised overhead and brought straight down into it ----------
# The handle in the body's middle plane (a touch of `yaw`), `pitch` up from level ahead
# (past 90 it leans back over the head). Swung down, the edge leads: square to the handle,
# in the same upright plane.
def stump_frame(yaw, pitch):
    y, p = math.radians(yaw), math.radians(pitch)
    h = [-math.sin(y) * math.cos(p), math.sin(p), -math.cos(y) * math.cos(p)]
    edge = [-math.sin(y) * math.sin(p), -math.cos(p), -math.cos(y) * math.sin(p)]
    return h, edge

# (time, (handle yaw, handle pitch, grip ahead of the chest, grip height, the torso's lean
# forward, hips down, the hands' gap along the handle, the grip to the right)). From the
# chop's rest, the axe goes up over the right shoulder, beside the head (the body leaning back
# a little), stops a blink there, then comes down straight and fast in front, the body
# bending into it; the edge bites into the stump and the axe is worked out and back to rest.
STUMP_KEYS = [
    (0.00, (-22.0, 52.0, 6.0, 15.0, 0.0, 0.0, 5.0, 0.5)),     # the chop's rest
    (0.20, (-14.0, 125.0, 2.0, 27.0, -6.0, 0.0, 4.5, 4.0)),   # going up by the head
    (0.30, (-10.0, 150.0, 0.5, 29.0, -9.0, 0.0, 4.0, 4.5)),   # raised over the right shoulder
    (0.36, (-4.0, 80.0, 5.0, 23.0, 6.0, 0.4, 3.2, 2.0)),      # coming down
    (0.42, (0.0, -38.0, 8.0, 14.5, 24.0, 1.4, 2.4, 0.5)),     # into the stump
    (0.45, (0.0, -35.0, 7.8, 14.8, 23.0, 1.5, 2.4, 0.5)),     # the jolt of it
    (0.58, (0.0, -36.0, 7.9, 14.6, 23.5, 1.4, 2.4, 0.5)),     # stuck
    (0.68, (-8.0, 12.0, 7.0, 16.0, 10.0, 0.7, 3.5, 0.5)),     # worked out
    (0.90, (-22.0, 52.0, 6.0, 15.0, 0.0, 0.0, 5.0, 0.5)),     # back at rest
]
STUMP_LENGTH, STUMP_SAMPLES = 0.9, 45

def stump_state_at(t):
    for i, ((t0, a), (t1, b)) in enumerate(zip(STUMP_KEYS, STUMP_KEYS[1:])):
        if t <= t1:
            k = (t - t0) / (t1 - t0)
            if (t0, t1) == (0.36, 0.42):
                k = k ** 1.6
            pa = STUMP_KEYS[max(i - 1, 0)][1]
            pb = STUMP_KEYS[min(i + 2, len(STUMP_KEYS) - 1)][1]
            k2, k3 = k * k, k * k * k
            return [0.5 * (2 * y + (-x + z) * k + (2 * x - 5 * y + 4 * z - w) * k2 + (-x + 3 * y - 3 * z + w) * k3)
                    for x, y, z, w in zip(pa, a, b, pb)]
    return list(STUMP_KEYS[-1][1])

def stump_pose_at(t):
    yaw, pitch, reach, height, lean, dip, gap, right = stump_state_at(t)
    h, edge = stump_frame(yaw, pitch)
    p = Pose()
    p.set("body", euler([0, 0, 0]), [0, 12 - dip, 0])
    # (the game's x turn tips the top back: forward is negative)
    p.set("torso", euler([-lean, 0, 0]))
    p.set("head", euler([-6.0 - lean * 0.5, 0, 0]))
    # The grip ahead of the chest, where the torso has brought it.
    chest = p.point("torso", [0.5, 17.0, 0.0])
    grip = [right, height - dip, chest[2] - reach]
    y_ax = norm(h)
    z_ax = scale(edge, -1.0)
    x_ax = cross(y_ax, z_ax)
    rot = cols(x_ax, y_ax, z_ax)
    left_on = add(grip, scale(y_ax, gap))
    for side, target, out in (("right", grip, 1.0), ("left", left_on, -1.0)):
        arm, hand = f"{side}_arm", f"{side}_hand"
        shoulder = p.point("torso", BONES[arm][1])
        fist_off = tp.FIST[side]
        end = target
        for _ in range(4):
            pole = [out * 1.2, -1.0, 0.3]
            elbow = ik(shoulder, end, tp.ARM_UP, tp.ARM_LOW, pole)
            ra = aim_limb(sub(elbow, shoulder), y_ax)
            rh = aim_limb(sub(end, elbow), y_ax)
            end = sub(target, mv(rh, fist_off))
        p.set(arm, ra, shoulder)
        p.set(hand, rh)
    p.set("axe", rot, grip)
    # Feet square and apart, the knees giving as the hips go down.
    for side, foot_at in (("right", [2.8, 0, 0.5]), ("left", [-2.8, 0, -0.5])):
        leg, foot = f"{side}_leg", f"{side}_foot"
        hip = p.point("body", BONES[leg][1])
        knee = ik(hip, foot_at, tp.LEG_UP, tp.LEG_LOW, [0, 0, -1])
        p.set(leg, aim_limb(sub(knee, hip), [0, 0, -1]), hip)
        p.set(foot, aim_limb(sub(foot_at, knee), [0, 0, -1]))
    return p, grip, rot

def trunk_cubes():
    """The trunk where the axe bites in at the hit: its edge a little way into the bark."""
    p, grip, rot = pose_at(0.38)
    edge_tip = add(grip, mv(rot, [0, 12.75, -5.6]))
    h, _ = axe_frame(*state_at(0.38)[:2])
    inward = norm([h[0], 0.0, h[2]])
    r = 0.44 * 32 / 1.8                     # the round trunk's radius in px
    c = add(edge_tip, scale(inward, r - 1.2))
    lo, hi = [c[0] - r, 0, c[2] - r], [c[0] + r, 48, c[2] + r]
    side = [BARK] * 4 + [RINGS, RINGS]
    # Two boxes, one turned 45 degrees: an eight-sided trunk.
    k = r * 0.83
    return [
        box("trunk", lo, hi, BARK, faces=side),
        box("trunk_45", [c[0] - k, 0.01, c[2] - k], [c[0] + k, 47.99, c[2] + k], BARK,
            origin=[c[0], 24, c[2]], rotation=[0, 45, 0], faces=side),
    ]

def build():
    parts = cubes_of_player()
    axe = axe_cubes()
    groups = {}
    def make(bone):
        kids = [make(b) for b, (par, _) in BONES.items() if par == bone]
        own = axe if bone == "axe" else ([parts[bone]] if bone in parts else [])
        g = group(bone, own + kids, BONES[bone][1])
        groups[bone] = g
        return g
    root = make("body")
    tree = trunk_cubes()
    tree_group = group("tree", tree, [0, 0, 0])
    tree_group["export"] = False

    anims = []
    for name, length, samples, fixed, at in (("chop", LENGTH, SAMPLES, None, pose_at), ("hold_axe", 2.0, 1, 0.0, pose_at),
                                             ("stump", STUMP_LENGTH, STUMP_SAMPLES, None, stump_pose_at)):
        keys = {}
        times = [round(length * i / samples, 4) for i in range(samples + 1)]
        for t in times:
            p, _, _ = at(t if fixed is None else fixed)
            for bone, (moved, rot) in p.keys().items():
                k = keys.setdefault(bone, [])
                if bone in ("body", "axe") or any(abs(v) > 1e-3 for v in moved):
                    k.append(kf("position", t, P([round(v, 3) for v in moved])))
                k.append(kf("rotation", t, R([round(v, 3) for v in rot])))
        animators = {groups[b]["uuid"]: {"name": b, "type": "bone", "keyframes": k} for b, k in keys.items() if k}
        anims.append({"uuid": str(uuid.uuid4()), "name": name, "loop": "hold" if name == "hold_axe" else "loop",
                      "override": False, "length": length, "snapping": 50, "selected": name == "chop",
                      "anim_time_update": "", "blend_weight": "", "start_delay": "", "loop_delay": "",
                      "animators": animators})

    skin, SW, SH = skin_png()
    pal, CW, CH = colors_png()
    model = {
        "meta": {"format_version": "4.10", "model_format": "free", "box_uv": False},
        "name": "chop",
        "resolution": {"width": CW, "height": CH},
        "elements": list(parts.values()) + axe + tree,
        "outliner": [root, tree_group],
        "textures": [
            {"name": "axe_tree.png", "id": "0", "uuid": str(uuid.uuid4()), "folder": "", "namespace": "",
             "width": CW, "height": CH, "uv_width": CW, "uv_height": CH, "particle": False,
             "render_mode": "default", "visible": True, "saved": False,
             "source": "data:image/png;base64," + base64.b64encode(pal).decode()},
            {"name": "player.png", "id": "1", "uuid": str(uuid.uuid4()), "folder": "", "namespace": "",
             "width": SW, "height": SH, "uv_width": SW, "uv_height": SH, "particle": False,
             "render_mode": "default", "visible": True, "saved": False,
             "source": "data:image/png;base64," + base64.b64encode(skin).decode()},
        ],
        "animations": anims,
    }
    out = os.path.join(HERE, "chop.bbmodel")
    with open(out, "w") as f:
        json.dump(model, f, indent=1)
    print(f"{os.path.basename(out)}: {len(model['elements'])} cubes, {len(anims)} animations")

if __name__ == "__main__":
    build()
