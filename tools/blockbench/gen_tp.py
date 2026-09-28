"""Generates the third-person gun rigs for Blockbench: tp_pistol.bbmodel, tp_revolver.bbmodel and
tp_ak.bbmodel: how a player holds each gun as the others see them, and how they move it.

Run:  python tools/blockbench/gen_tp.py
then: python tools/blockbench/bbmodel_to_rust.py tools/blockbench/tp_pistol.bbmodel (and the
      others)

Each rig is a player of Minecraft's proportions (32 pixels tall, standing on the origin, facing
-Z, its shoulders at (+-5, 22)) holding the gun: the gun's own Blockbench model brought in at the
size it is drawn on the player (and made thicker, so that seen from the side it is not a stick).
The game only reads two bones of it:
- `gun`: where the gun is, its origin at the middle of the right fist on the grip (the gun's
  muzzle toward -Z, its top up);
- `left_hand` (inside `gun`, so it goes where the gun goes): where the left hand holds it (the
  handguard, or cupping the right hand), and where it goes while reloading.
The game points the arms at the grip and at the left hand, as they are here (the arms in the
rig are only turned to show that), and turns all of it with where the head looks.

Animations, added up by the game over the rest pose (the gun held at the hip, ready):
  idle (a slow breath), walk (one step cycle, as the legs swing), sprint (held while running),
  crouch (held while sneaking), aim (held up to the eye), shoot (the kick of a shot) and
  reload (at the same moments as the first-person one: the pistol's and the AK's 2 seconds,
  the revolver's 2.3).
"""
import base64, io, json, math, os, struct, uuid, zlib

HERE = os.path.dirname(os.path.abspath(__file__))
SHOULDER_R, SHOULDER_L = (5.0, 22.0, 0.0), (-5.0, 22.0, 0.0)
EYE = (1.6, 27.5, -11.2)      # where a sight is brought to when aiming (the right eye, ahead)

# ---------- small maths: rotations as the game does them (Z.Y.X, degrees) ----------
def rx(a):
    c, s = math.cos(a), math.sin(a); return [[1, 0, 0], [0, c, -s], [0, s, c]]
def ry(a):
    c, s = math.cos(a), math.sin(a); return [[c, 0, s], [0, 1, 0], [-s, 0, c]]
def rz(a):
    c, s = math.cos(a), math.sin(a); return [[c, -s, 0], [s, c, 0], [0, 0, 1]]
def mm(a, b): return [[sum(a[i][k] * b[k][j] for k in range(3)) for j in range(3)] for i in range(3)]
def mv(a, v): return [sum(a[i][k] * v[k] for k in range(3)) for i in range(3)]
def euler(r):
    x, y, z = (math.radians(v) for v in r)
    return mm(rz(z), mm(ry(y), rx(x)))
def add(a, b): return [p + q for p, q in zip(a, b)]
def sub(a, b): return [p - q for p, q in zip(a, b)]

def reach(shoulder, target):
    """The arm's rotation (degrees, x and y) pointing an arm hanging from `shoulder` at
    `target`, as the game's `player::reach` does."""
    d = sub(target, shoulder)
    n = math.sqrt(sum(v * v for v in d)) or 1.0
    d = [v / n for v in d]
    return [math.degrees(math.acos(max(-1.0, min(1.0, -d[1])))), math.degrees(math.atan2(-d[0], -d[2])), 0.0]

# ---------- keyframes (as the other rigs: rotation x/y and position x given mirrored) ----------
def kf(channel, t, v, interp="catmullrom"):
    return {"channel": channel, "data_points": [{"x": v[0], "y": v[1], "z": v[2]}], "uuid": str(uuid.uuid4()),
            "time": t, "color": -1, "interpolation": interp}
def R(v): return [-v[0], -v[1], v[2]]
def P(v): return [-v[0], v[1], v[2]]

def sample(keys, t):
    """A channel's value at `t` (linear between keys: good enough for the arms' preview)."""
    if not keys:
        return [0.0, 0.0, 0.0]
    if t <= keys[0][0]:
        return list(keys[0][1])
    for (t0, a), (t1, b) in zip(keys, keys[1:]):
        if t0 <= t <= t1:
            k = (t - t0) / max(1e-6, t1 - t0)
            return [p + (q - p) * k for p, q in zip(a, b)]
    return list(keys[-1][1])

# ---------- the player (only to see it by; the game draws its own) ----------
def skin_png():
    """A plain skin: a face tone, a teal shirt, blue trousers, in 8x8 swatches."""
    cols = [(196, 140, 102), (40, 160, 170), (54, 60, 150), (70, 50, 40)]
    W, H = 32, 8
    img = [[(0, 0, 0, 255)] * W for _ in range(H)]
    for k, c in enumerate(cols):
        for y in range(H):
            for x in range(k * 8, k * 8 + 8):
                shade = 10 if (x + y) % 3 == 0 else 0
                img[y][x] = tuple(min(255, v + shade) for v in c) + (255,)
    raw = b"".join(b"\x00" + b"".join(bytes(p) for p in row) for row in img)
    def chunk(t, d): return struct.pack(">I", len(d)) + t + d + struct.pack(">I", zlib.crc32(t + d) & 0xffffffff)
    return (b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", W, H, 8, 6, 0, 0, 0))
            + chunk(b"IDAT", zlib.compress(raw, 9)) + chunk(b"IEND", b""))

def box(name, frm, to, swatch):
    u = swatch * 8
    faces = {f: {"uv": [u + 1, 1, u + 7, 7], "texture": 1} for f in ("north", "east", "south", "west", "up", "down")}
    return {"name": name, "type": "cube", "uuid": str(uuid.uuid4()), "box_uv": False, "rescale": False,
            "from": frm, "to": to, "origin": [(a + b) / 2 for a, b in zip(frm, to)], "rotation": [0, 0, 0],
            "faces": faces}

def group(name, children, origin, rot=(0, 0, 0)):
    """A group of cubes (given as the elements; the outliner lists their uuids) and groups."""
    kids = [c["uuid"] if c.get("type") == "cube" else c for c in children]
    return {"name": name, "uuid": str(uuid.uuid4()), "origin": list(origin), "rotation": list(rot),
            "isOpen": True, "visibility": True, "export": True, "children": kids}

# ---------- a gun's own model, brought in ----------
SKIP = {"right_arm_mesh", "left_arm_mesh", "left_arm", "spent_case", "silencer", "scope", "laser",
        "flashlight", "mag_extended", "speedloader", "loose_round"}

def gun_cubes(src, root, grip, scale, thick):
    """The cubes of the group `root` of the gun model `src` (without its arms, attachments and
    hidden parts), moved so its right fist is at `grip` and scaled by `scale` (and its width,
    x, by `thick` more); its texture; and where its left fist is, from the right one."""
    m = json.load(open(os.path.join(HERE, src), encoding="utf-8"))
    elements = {e["uuid"]: e for e in m["elements"]}
    fist = left = None
    def find(n):
        nonlocal fist, left
        if isinstance(n, str):
            return None
        if n["name"] == "right_arm_mesh":
            fist = n["origin"]
        if n["name"] == "left_arm_mesh":
            left = n["origin"]
        found = n if n["name"] == root else None
        for c in n.get("children", []):
            sub_found = find(c)
            found = found or sub_found
        return found
    g = None
    for n in m["outliner"]:
        f = find(n)
        g = g or f
    out = []
    def collect(n):
        if isinstance(n, str):
            e = elements.get(n)
            if e and e.get("visibility", True):
                out.append(e)
            return
        if n["name"] in SKIP or not n.get("visibility", True):
            return
        for c in n.get("children", []):
            collect(c)
    collect(g)
    s = [scale * thick, scale, scale]
    def place(p): return [round(grip[k] + (p[k] - fist[k]) * s[k], 4) for k in range(3)]
    cubes = []
    for e in out:
        e = json.loads(json.dumps(e))
        e["uuid"] = str(uuid.uuid4())
        e["from"], e["to"], e["origin"] = place(e["from"]), place(e["to"]), place(e.get("origin", e["from"]))
        for f in e["faces"].values():
            if f.get("texture") is not None:
                f["texture"] = 0
        cubes.append(e)
    left_off = [(left[k] - fist[k]) * s[k] for k in range(3)]
    return cubes, m["textures"][0], m["resolution"], left_off

# ---------- the guns ----------
MODEL = 0.835 * 1.15          # a gun model's pixels to the old gun space, and that to its size
GUNS = {
    "pistol": dict(src="pistol.bbmodel", root="pistol", arm_scale=0.45, thick=1.7,
                   grip=[1.2, 18.8, -9.0], sight=9.8, reload=2.0),
    "revolver": dict(src="revolver.bbmodel", root="revolver", arm_scale=0.45, thick=1.6,
                     grip=[1.2, 18.8, -9.0], sight=9.4, reload=2.3),
    "ak": dict(src="ak.bbmodel", root="rifle", arm_scale=0.2, thick=2.5,
               grip=[3.0, 17.4, -9.2], sight=10.6, reload=2.0),
}

# ---------- how each gun is held (model pixels; the body turned, + facing more to the left) ----------
def turn_y(p, deg):
    """`p` turned about the body's vertical axis."""
    a = math.radians(deg)
    return [p[0] * math.cos(a) + p[2] * math.sin(a), p[1], -p[0] * math.sin(a) + p[2] * math.cos(a)]

def rot_about(rot, pivot, p):
    """`p` turned by `rot` (degrees) about `pivot`."""
    return add(pivot, mv(euler(rot), sub(p, pivot)))

# A handgun's left fist, cupping the right one: beside it, a little lower (the fists 4 wide).
CUP = [-2.8, -0.7, -0.3]
# Where a rifle's stock goes: into the pocket of the right shoulder, inside the arm, near its
# top (on the body, before it turns).
POCKET = [2.4, 23.2, -2.2]
REACH = 8.8          # shoulder to the fist's middle, the elbow a little bent

def held_poses(name, cfg, butt, sight):
    """The rest, aimed and running poses: (grip, gun rotation, body turn). A rifle rests its
    stock in the shoulder, muzzle down, and is aimed level from there, the body bladed (the left
    shoulder forward) so the left hand reaches the handguard; a handgun is held out in both
    fists in a Weaver stance, the sights brought up to the right eye."""
    if name == "ak":
        turn = -40.0
        pocket = turn_y(POCKET, turn)
        aim = sub(pocket, butt)
        down = [-30.0, 0, 0]
        rest = rot_about(down, pocket, aim)
        return dict(rest=(rest, down, turn), aim=(aim, [0, 0, 0], turn),
                    # running: across the chest (port arms), the muzzle up to the left, the stock
                    # down in front of the right hip, clear of the right forearm
                    sprint=([1.5, 16.5, -5.5], [35.0, 50.0, 0.0], -10.0))
    turn = -15.0
    shoulder = turn_y([5.0, 22.0, 0.0], turn)
    def at(x, y):
        dx, dy = x - shoulder[0], y - shoulder[1]
        return [x, y, shoulder[2] - math.sqrt(max(0.0, REACH * REACH - dx * dx - dy * dy))]
    return dict(rest=(at(1.0, 19.0), [-20.0, 0, 0], turn),
                aim=(at(1.6, EYE[1] - sight), [0, 0, 0], turn),
                sprint=([2.0, 16.5, -4.5], [-45.0, 10.0, 0], -5.0))

def animations(name, cfg, left_rest):
    """Each animation's keys: (name, length, loop, gun position keys, gun rotation keys, left
    hand position keys, the body's turn keys), in the game's convention (rotations in
    degrees, the muzzle up being + x)."""
    long = name == "ak"
    A = []
    poses = cfg["poses"]
    rest_p, rest_r, rest_t = poses["rest"]
    # held at rest (the pose itself, breathing)
    A.append(("idle", 3.0, "loop",
              [(0, [0, 0, 0]), (1.5, [0, 0.18, 0]), (3.0, [0, 0, 0])],
              [(0, rest_r), (1.5, add(rest_r, [1.5, 0, 0])), (3.0, rest_r)],
              [], [(0, rest_t), (3.0, rest_t)]))
    # one step cycle (left, right): bobbing, swaying and rolling a little
    A.append(("walk", 1.0, "loop",
              [(0, [0, 0, 0]), (0.25, [0.35, -0.45, 0]), (0.5, [0, 0, 0]), (0.75, [-0.35, -0.45, 0]), (1.0, [0, 0, 0])],
              [(0, [0, 0, 0]), (0.25, [-2, 0, -2.5]), (0.5, [0, 0, 0]), (0.75, [-2, 0, 2.5]), (1.0, [0, 0, 0])],
              [], []))
    # running: the pistol down by the chest, pointed at the ground ahead; the rifle across it
    sp, sr, st = poses["sprint"]
    spr_p, spr_r = sub(sp, rest_p), sub(sr, rest_r)
    A.append(("sprint", 0.5, "loop",
              [(0, spr_p), (0.125, add(spr_p, [0.3, -0.5, 0])), (0.25, spr_p), (0.375, add(spr_p, [-0.3, -0.5, 0])), (0.5, spr_p)],
              [(0, spr_r), (0.25, add(spr_r, [-3, 0, 0])), (0.5, spr_r)],
              [], [(0, st - rest_t), (0.5, st - rest_t)]))
    # sneaking: the shoulders are 3.2 lower, the gun with them, held a little closer
    A.append(("crouch", 1.0, "hold", [(0, [0, -3.2, 1.0]), (1.0, [0, -3.2, 1.0])], [(0, [4, 0, 0]), (1.0, [4, 0, 0])], [], []))
    # aimed: a rifle's stock in the shoulder, level; a handgun's sights up to the eye
    ap, ar, at = poses["aim"]
    A.append(("aim", 0.3, "hold", [(0, [0, 0, 0]), (0.3, sub(ap, rest_p))], [(0, [0, 0, 0]), (0.3, sub(ar, rest_r))],
              [], [(0, 0.0), (0.3, at - rest_t)]))
    # a shot: the muzzle kicks up and the gun comes back a little
    kick = 7.0 if long else 14.0
    A.append(("shoot", 0.3, "once",
              [(0, [0, 0, 0]), (0.03, [0, 0.3, 0.9]), (0.3, [0, 0, 0])],
              [(0, [0, 0, 0]), (0.03, [kick, 0, 0]), (0.3, [0, 0, 0])],
              [], []))
    # the reload
    if name == "revolver":
        # the cylinder swung out to the left (the gun rolled onto its right side, muzzle up),
        # the ejector pushed, a speedloader fetched from the belt and turned, the cylinder shut
        cyl = [-2.2 * cfg["arm_scale"] / 0.45, 1.8, -0.4]
        belt = [-3.0, -9.0, 6.0]
        lh = [(0, [0, 0, 0]), (0.3, cyl), (0.6, add(cyl, [0, 0, -1.2])), (0.7, cyl), (0.95, cyl),
              (1.15, belt), (1.35, add(cyl, [0, -0.5, 1.2])), (1.5, add(cyl, [0, 0, 0.3])), (1.62, cyl),
              (1.9, add(cyl, [0.6, 0, 0])), (2.3, [0, 0, 0])]
        A.append(("reload", 2.3, "once",
                  [(0, [0, 0, 0]), (0.3, [-1.0, 2.2, 1.5]), (1.62, [-1.0, 2.2, 1.5]), (2.0, [-0.4, 0.8, 0.6]), (2.3, [0, 0, 0])],
                  [(0, [0, 0, 0]), (0.3, [28, 0, 38]), (1.62, [28, 0, 38]), (1.9, [8, 0, 10]), (2.3, [0, 0, 0])],
                  lh, []))
    else:
        # the pistol's moments: the magazine out at 0.35 (fallen by 0.6), the new one fetched
        # (0.62) and pushed in (1.2), the hand off it (1.35); the slide or the charging handle
        # pulled (1.45 .. 1.55) and let go (1.6)
        if long:
            mag = [0.5, -2.3, 2.8]                  # the magazine's bottom, from the handguard
            handle = [1.15, 1.2, 4.1]               # the charging handle
            pull = add(handle, [0, 0, 1.1])
            gun_p, gun_r = [-1.2, 2.0, 1.2], [8, 0, 32]
        else:
            mag = [0.3, -1.6, 0.2]                  # under the grip
            handle = [0.2, 2.6, 1.6]                # over the slide's back
            pull = add(handle, [0, 0, 1.4])
            gun_p, gun_r = [-0.8, 2.4, 1.6], [22, 0, -24]
        belt = [-2.5, -9.5, 6.5]
        lh = [(0, [0, 0, 0]), (0.2, mag), (0.35, mag), (0.6, add(mag, [0, -5.5, 1.2])), (0.8, belt),
              (1.1, add(mag, [0, -0.6, 0])), (1.22, mag), (1.32, add(handle, [0, 0, -0.6])), (1.45, handle),
              (1.55, pull), (1.62, add(pull, [0, 0.8, 0])), (2.0, [0, 0, 0])]
        if "lh_shift" in cfg:
            lh = [(t, add(v, cfg["lh_shift"]) if any(v) else v) for t, v in lh]
        A.append(("reload", 2.0, "once",
                  [(0, [0, 0, 0]), (0.3, gun_p), (1.25, gun_p), (1.45, [x * 0.4 for x in gun_p]), (1.75, [0, 0, 0]), (2.0, [0, 0, 0])],
                  [(0, [0, 0, 0]), (0.3, gun_r), (1.25, gun_r), (1.45, [x * 0.35 for x in gun_r]), (1.75, [0, 0, 0]), (2.0, [0, 0, 0])],
                  lh, []))
    return A

def build(name, cfg):
    scale = cfg["arm_scale"] * MODEL
    # where the stock's end and the sights are, from the grip, and the left hand on the gun
    cubes, _, _, left_off = gun_cubes(cfg["src"], cfg["root"], [0, 0, 0], scale, cfg["thick"])
    zmax = max(max(e["from"][2], e["to"][2]) for e in cubes)
    rear = [e for e in cubes if max(e["from"][2], e["to"][2]) > zmax - 0.8]
    ys = [v for e in rear for v in (e["from"][1], e["to"][1])]
    butt = [0.0, (min(ys) + max(ys)) / 2, zmax]
    sight = cfg["sight"] * scale
    if name != "ak":
        left_off = CUP
    else:
        # the left hand under the handguard, holding it up from below (the fist's middle a
        # little under its bottom, half-way along it), not beside it
        hg = [e for e in cubes if e["name"].startswith("hg_")]
        bottom = min(min(e["from"][1], e["to"][1]) for e in hg)
        front = min(min(e["from"][2], e["to"][2]) for e in hg)
        back = max(max(e["from"][2], e["to"][2]) for e in hg)
        under = [-0.3, bottom - 1.6, (front + back) / 2]
        # the reload's moves of the left hand were made from where it held before
        cfg = dict(cfg, lh_shift=sub(left_off, under))
        left_off = under
    cfg = dict(cfg, poses=held_poses(name, cfg, butt, sight))
    grip = [round(v, 4) for v in cfg["poses"]["rest"][0]]
    cubes, tex, res, _ = gun_cubes(cfg["src"], cfg["root"], grip, scale, cfg["thick"])
    left_rest = add(grip, left_off)

    # the player to see it by: body, head and legs; the arms (turned toward the hands)
    body = [box("body", [-4, 12, -2], [4, 24, 2], 1), box("head", [-4, 24, -4], [4, 32, 4], 0),
            box("right_leg", [-0.1, 0, -2], [3.9, 12, 2], 2), box("left_leg", [-3.9, 0, -2], [0.1, 12, 2], 2)]
    box_r = box("right_arm_box", [3, 12, -2], [7, 24, 2], 0)
    box_l = box("left_arm_box", [-7, 12, -2], [-3, 24, 2], 0)
    g_right = group("right_arm", [box_r], SHOULDER_R)
    g_left = group("left_arm", [box_l], SHOULDER_L)
    g_left_hand = group("left_hand", [], left_rest)
    g_gun = group("gun", cubes + [g_left_hand], grip)
    # the body's turn (only its rotation about the vertical is read)
    g_torso = group("torso", [], (0, 12, 0))
    g_root = group("player", body + [g_right, g_left, g_gun, g_torso], (0, 0, 0))
    elements = body + [box_r, box_l] + cubes

    anims = []
    rest_right = reach(SHOULDER_R, grip)
    rest_left = reach(SHOULDER_L, left_rest)
    for (an, length, loop, gpos, grot, lpos, tturn) in animations(name, cfg, left_rest):
        animators = {
            g_gun["uuid"]: {"name": "gun", "type": "bone", "keyframes":
                            [kf("position", t, P(v)) for t, v in gpos] + [kf("rotation", t, R(v)) for t, v in grot]},
        }
        if tturn:
            animators[g_torso["uuid"]] = {"name": "torso", "type": "bone", "keyframes":
                                          [kf("rotation", t, R([0, v, 0])) for t, v in tturn]}
        if lpos:
            animators[g_left_hand["uuid"]] = {"name": "left_hand", "type": "bone", "keyframes":
                                              [kf("position", t, P(v)) for t, v in lpos]}
        # the arms, pointed at the hands as the game points them (only to see it by)
        times = sorted({t for t, _ in gpos} | {t for t, _ in grot} | {t for t, _ in lpos})
        rk, lk = [], []
        for t in times:
            at = add(grip, sample(gpos, t))
            m = euler(sample(grot, t))
            hand = add(at, mv(m, add(left_off, sample(lpos, t))))
            r, l = reach(SHOULDER_R, at), reach(SHOULDER_L, hand)
            rk.append(kf("rotation", t, R(r), "linear"))
            lk.append(kf("rotation", t, R(l), "linear"))
        animators[g_right["uuid"]] = {"name": "right_arm", "type": "bone", "keyframes": rk}
        animators[g_left["uuid"]] = {"name": "left_arm", "type": "bone", "keyframes": lk}
        anims.append({"uuid": str(uuid.uuid4()), "name": an, "loop": loop, "override": False, "length": length,
                      "snapping": 60, "selected": an == "idle", "anim_time_update": "", "blend_weight": "",
                      "start_delay": "", "loop_delay": "", "animators": animators})
    # at rest the arms already reach for the gun
    g_right["rotation"] = [0, 0, 0]
    skin = skin_png()
    model = {
        "meta": {"format_version": "4.10", "model_format": "free", "box_uv": False},
        "name": f"tp_{name}",
        "resolution": res,
        "elements": elements,
        "outliner": [g_root],
        "textures": [
            dict(tex, uuid=str(uuid.uuid4())),
            {"name": "player.png", "id": "1", "uuid": str(uuid.uuid4()), "folder": "", "namespace": "",
             "width": 32, "height": 8, "uv_width": 32, "uv_height": 8, "particle": False,
             "render_mode": "default", "visible": True, "saved": False,
             "source": "data:image/png;base64," + base64.b64encode(skin).decode()},
        ],
        "animations": anims,
    }
    with open(os.path.join(HERE, f"tp_{name}.bbmodel"), "w") as f:
        json.dump(model, f, indent=1)
    print(f"tp_{name}: {len(cubes)} gun cubes, rest arms {rest_right[:2]} {rest_left[:2]}")

if __name__ == "__main__":
    for name, cfg in GUNS.items():
        build(name, cfg)
