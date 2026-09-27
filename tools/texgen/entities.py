"""Entity atlases (chest, player, pig, sheep) and particles (flame, smoke), in the look of
Faithful 64x redrawn at our sizes.

Atlases use Minecraft's box UV layout (`ModelPart.Cube`, see `src/entity/mob.rs`) in a 64 unit
wide atlas: chest and player at 8 px per unit (512 x 512), pig and sheep at 2 px per unit.
Faithful's atlases are 4 px per unit, so shapes measured on them ("ref px") are scaled by
`k / 4` here. Faces are painted in atlas orientation (the chest is stored upside down).
"""

from __future__ import annotations

import numpy as np
from scipy import ndimage

from common import Ramp, blank, ellipse, grid, grow, hexc, noise, paint, pix, polygon, rng, thick_line

# ---------------------------------------------------------------------------- helpers


def box_uv(u, v, dx, dy, dz):
    """Minecraft's box UV faces (x, y, w, h in units) for a cube at texture offset (u, v)."""
    return {
        "top": (u + dz, v, dx, dz),
        "bottom": (u + dz + dx, v, dx, dz),
        "right": (u, v + dz, dz, dy),
        "front": (u + dz, v + dz, dx, dy),
        "left": (u + dz + dx, v + dz, dz, dy),
        "back": (u + 2 * dz + dx, v + dz, dx, dy),
    }


def cols(*hexes) -> np.ndarray:
    return np.stack([hexc(h) for h in hexes])


def aniso(seed: int, h: int, w: int, cy: float, cx: float) -> np.ndarray:
    """Smooth non-tiling noise 0..1 with features about cy x cx pixels."""
    gh, gw = int(np.ceil(h / cy)) + 3, int(np.ceil(w / cx)) + 3
    g = rng(seed).random((gh, gw)).astype(np.float32)
    z = ndimage.zoom(g, (cy, cx), order=3, mode="nearest")
    oy, ox = int(cy), int(cx)
    return np.clip(z[oy : oy + h, ox : ox + w], 0, 1)


def pick(seed, h, w, colors, weights=None, size=2) -> np.ndarray:
    """A random color from `colors` per `size` x `size` cluster (RGB image)."""
    c = cols(*colors)
    wts = np.ones(len(colors)) if weights is None else np.asarray(weights, np.float64)
    cum = np.cumsum(wts) / wts.sum()
    idx = np.clip(np.searchsorted(cum, pix(seed, size, h, w)), 0, len(colors) - 1)
    return c[idx]


def refgrid(h, w, s):
    """Pixel centers in reference pixels (`s` of our pixels per reference pixel)."""
    yy, xx = grid(h, w)
    return yy / s, xx / s


def canvas(h, w, rgb=None):
    img = blank(h, w)
    if rgb is not None:
        img[..., :3] = rgb
        img[..., 3] = 255
    return img


def put(atlas, img, x, y, k):
    h, w = img.shape[:2]
    atlas[int(y * k) : int(y * k) + h, int(x * k) : int(x * k) + w] = img


# ---------------------------------------------------------------------------- chest

FR_DARK = ("#2a251d", "#27231e", "#28241d", "#211d17", "#2b261f")
FR_GREY = ("#332e25", "#352f26", "#362f25", "#373127", "#2a251d")
FR_LIGHT = ("#443c30", "#463e32", "#453c2f", "#413b2f")
SH_F = ("#84622f", "#7f5f22", "#90662a", "#956c2e", "#825b26")
SH_T = ("#90662a", "#926323", "#7f5f22", "#956c2e")
SH_R1 = ("#775a30", "#695229", "#73562e", "#755429")
SH_R2 = ("#8e6126", "#926323")
EDGES = {
    "F": [(4, FR_DARK, (12, 2, 2, 1, 1)), (4, SH_F, (2, 3, 3, 2, 1))],
    "L": [(4, FR_LIGHT, (8, 3, 2, 1))],
    "D": [(4, FR_DARK, (12, 2, 2, 1, 1))],
    "R": [(8, FR_GREY, (2, 3, 10, 6, 0)), (4, SH_R1, None), (4, SH_R2, (4, 1))],
    "T": [(8, FR_GREY, (2, 3, 10, 6, 0)), (4, SH_T, None)],
}
IRON = Ramp("#767676", "#868686", "#919191", "#a5a5a5", "#c2c2c2", "#cdcdcd")


def planks(seed, h, w, seams, dark=False):
    """Horizontal oak boards (Faithful's chest wood): two tones in long patches, wavy grain
    lines and 4 px seams starting at the rows in `seams`."""
    if dark:
        light, mid, grain, s1, s2 = "#482c00", "#442900", "#392600", "#482a00", "#432600"
    else:
        light, mid, grain, s1, s2 = "#ab792d", "#a47227", "#8f691d", "#a76e1f", "#a26b23"
    n1 = aniso(seed, h, w, 3, 34)
    rgb = np.where((n1 > 0.5)[..., None], hexc(light), hexc(mid))
    r = rng(seed + 4)
    g = hexc(grain)
    # grain: short wavy dashes, 2 px thick, with little steps (Faithful's "~" marks)
    for _ in range(int(h * w / 200)):
        y, x = r.integers(0, h), r.integers(-8, w)
        L = int(r.integers(6, 20))
        ph = r.uniform(0, 6.3)
        th = int(r.integers(2, 4))
        for j in range(L):
            yy = int(y + round(np.sin(ph + j * 0.4) * 1.5))
            if 0 <= x + j < w:
                rgb[max(0, yy) : max(0, yy + th - (j in (0, L - 1))), x + j] = g
    # vertical "stems" with side branches at the middle of some boards
    bounds = [0] + list(seams) + [h]
    for bi, (a, b) in enumerate(zip(bounds[:-1], bounds[1:])):
        if b - a < 20 or bi % 2 == int(seed) % 2:
            continue
        x = w // 2 - 1 + int(r.integers(-3, 4))
        y0, y1 = a + 6, b - 4
        rgb[y0:y1, x : x + 3] = g
        for yb in range(y0 + 2, y1 - 4, 7):
            L = int(r.integers(4, 9))
            rgb[yb : yb + 3, x - L : x] = g
            rgb[yb + 1 : yb + 4, x + 3 : x + 3 + L] = g
    for s in seams:
        rgb[s : s + 2] = hexc(s1)
        rgb[s + 2 : s + 4] = np.where((pix(seed + 5 + s, 2, 2, w) > 0.35)[..., None], hexc(s2), hexc(s1))
    return canvas(h, w, rgb)


def band_mask(h, w, side, o, wd):
    m = np.zeros((h, w), bool)
    if side == "t":
        m[o : o + wd] = True
    elif side == "b":
        m[h - o - wd : h - o] = True
    elif side == "l":
        m[:, o : o + wd] = True
    else:
        m[:, w - o - wd : w - o] = True
    return m


def panel(seed, h, w, edges, seams, dark=False, hollow=False):
    """A chest face in atlas orientation. `edges` maps t/b/l/r to an EDGES kind (None: the open
    seam of a double chest half)."""
    img = planks(seed, h, w, seams, dark)
    depth = {s: sum(b[0] for b in EDGES[e]) if e else 0 for s, e in edges.items()}
    if hollow:
        img[depth["t"] : h - depth["b"], depth["l"] : w - depth["r"], :3] = 0
    # inner bands first, then the frames on top so they win at the corners
    for outer in (False, True):
        for i, (side, e) in enumerate(edges.items()):
            if not e:
                continue
            o = 0
            for j, (wd, colors, wts) in enumerate(EDGES[e]):
                if (j == 0) == outer:
                    m = band_mask(h, w, side, o, wd)
                    img[m, :3] = pick(seed + 11 * i + j, h, w, colors, wts)[m]
                o += wd
    return img


def latch_face(h, w, kind):
    """Iron latch, atlas orientation (lit towards the bottom = the top in the world)."""
    yy, xx = grid(h, w)
    v, u = yy / h, xx / w
    if kind in ("top", "bottom"):
        t = 0.95 - u * 0.45
    elif kind == "left":
        t = 0.05 + v**3 * 0.45
    elif kind == "right":
        t = 0.08 + v * 0.55 + (1 - u) * 0.08
    else:
        t = 0.02 + v * 0.8 + u * 0.3 - (u > 0.85) * 0.25
        t = np.where(u < 0.22, 0.02 + v * 0.45, t)
    return canvas(h, w, IRON.shade(t, 0.0))


def latch_shadows(lid, body, cx):
    """The dark stain around the latch on the front: under the lid rim and on the body top.
    `cx` is the latch center column (a double chest's latch sits on the seam)."""
    for img, rows, core_c, rim_c, out_c in (
        (lid, (8, 18), "#543e16", "#614116", "#734f1e"),
        (body, (58, 72), "#45351a", "#614116", "#77501c"),
    ):
        h, w = img.shape[:2]
        yy, xx = grid(h, w)
        y0, y1 = rows
        if img is lid:
            core = (((xx - cx) / 8.5) ** 2 + (np.clip(yy - (y1 - 7), 0, None) / 7) ** 2 < 1) & (yy >= y0)
        else:
            core = (np.abs(xx - cx) < 8) & (yy >= y0) & (yy < y1)
        rim = grow(core, 2) & ~core & (yy >= y0)
        out = grow(core, 4) & ~core & ~rim & (yy >= y0)
        if img is body:
            out = (np.abs(xx - cx) < 11) & (yy >= y0 - 2) & (yy < y0)
            img[(np.abs(xx - cx) < 12) & (yy >= 72), :3] = hexc("#211d17")
        img[out, :3] = hexc(out_c)
        img[rim, :3] = hexc(rim_c)
        img[core, :3] = hexc(core_c)


LID = {"t": "T", "b": "L", "l": "D", "r": "L"}
BODY = {"t": "F", "b": "R", "l": "F", "r": "L"}
SQUARE = {"t": "F", "b": "F", "l": "F", "r": "F"}


def chest_single(seed: int) -> np.ndarray:
    k = 8
    a = blank(512, 512)
    for name, (x, y, w, h) in box_uv(0, 0, 2, 4, 1).items():
        put(a, latch_face(h * k, w * k, name), x, y, k)
    for i, x in enumerate((0, 14, 28, 42)):
        lid = panel(seed + 10 + i, 40, 112, LID, [18])
        body = panel(seed + 20 + i, 80, 112, BODY, [24, 56])
        if x == 42:
            latch_shadows(lid, body, 57)
        put(a, lid, x, 14, k)
        put(a, body, x, 33, k)
    put(a, panel(seed + 30, 112, 112, SQUARE, [24, 56, 88], dark=True), 14, 0, k)
    put(a, panel(seed + 31, 112, 112, SQUARE, [24, 56, 88]), 28, 0, k)
    put(a, panel(seed + 32, 112, 112, SQUARE, [24, 56, 88]), 14, 19, k)
    put(a, panel(seed + 33, 112, 112, SQUARE, [24, 56, 88], hollow=True), 28, 19, k)
    return a


def chest_double(seed: int, left: bool) -> np.ndarray:
    """One half of a double chest (15 units long) with no frame on the seam edge (like
    Faithful: the left half's squares and face 14 open on the left, face 43 on the right)."""
    k = 8
    a = blank(512, 512)
    for name, (x, y, w, h) in box_uv(0, 0, 1, 4, 1).items():
        put(a, latch_face(h * k, w * k, name), x, y, k)
    sq_open = "l" if left else "r"
    f14_open = "l" if left else "r"
    f43_open = "r" if left else "l"

    def opened(spec, side):
        return {s: (None if s == side else e) for s, e in spec.items()}

    def flipped(spec):  # the frame kinds of a face seen from the other side
        return {"t": spec["t"], "b": spec["b"], "l": spec["r"], "r": spec["l"]}

    end_x = 29 if left else 0
    put(a, panel(seed + 10, 40, 112, LID, [18]), end_x, 14, k)
    put(a, panel(seed + 20, 80, 112, BODY, [24, 56]), end_x, 33, k)
    for i, (x, side) in enumerate(((14, f14_open), (43, f43_open))):
        lid_spec = opened(LID if side == "r" else flipped(LID), side)
        body_spec = opened(BODY if side == "r" else flipped(BODY), side)
        lid = panel(seed + 11 + i, 40, 120, lid_spec, [18])
        body = panel(seed + 21 + i, 80, 120, body_spec, [24, 56])
        if x == 43:
            latch_shadows(lid, body, 120 if side == "r" else 0)
        put(a, lid, x, 14, k)
        put(a, body, x, 33, k)
    put(a, panel(seed + 30, 112, 120, opened(SQUARE, sq_open), [24, 56, 88], dark=True), 14, 0, k)
    put(a, panel(seed + 31, 112, 120, opened(SQUARE, sq_open), [24, 56, 88]), 29, 0, k)
    put(a, panel(seed + 32, 112, 120, opened(SQUARE, sq_open), [24, 56, 88]), 14, 19, k)
    put(a, panel(seed + 33, 112, 120, opened(SQUARE, sq_open), [24, 56, 88], hollow=True), 29, 19, k)
    return a


# ---------------------------------------------------------------------------- player

HAIR = Ramp("#241808", "#2b1e0d", "#332411", "#3f2a15")
SK = {  # skin tones, light to dark
    "hi": "#b7836b", "f": "#b3795e", "e": "#aa7259", "d": "#9b6349", "c": "#94603e",
    "a": "#90593f", "b": "#8f5e3e", "8": "#815339", "2": "#764b33",
}
BEARD = ("#492510", "#421d0a")
SHIRT = {"hi": "#00cccc", "hi2": "#0abcbc", "5": "#00afaf", "4": "#00a4a4", "2": "#049595",
         "0": "#058888", "d1": "#037a7a", "d2": "#007f7f", "d3": "#006868"}
PANTS = {"hi": "#463aa5", "m": "#41359b", "d": "#3a3189"}
SHOE = {"m": "#363636", "d": "#282828", "hi": "#454545"}


def hair(seed, h, w):
    t = aniso(seed, h, w, 4, 2.2) * 0.5 + pix(seed + 1, 1, h, w) * 0.25 + aniso(seed + 2, h, w, 12, 8) * 0.25
    return canvas(h, w, HAIR.shade(np.clip((t - 0.2) * 1.6, 0, 1), 0.0))


def set_(img, mask, color):
    img[mask, :3] = hexc(color) if isinstance(color, str) else color[mask]
    img[mask, 3] = 255


def head_front(seed):
    img = hair(seed, 64, 64)
    ry, rx = refgrid(64, 64, 2)
    d = np.abs(rx - 16)
    top = 8 + np.interp(d, [0, 10, 12, 13, 14, 15, 16], [0, 0, 1, 2, 3, 4, 5])
    skin = ry >= top
    right = (rx > 21.5 - np.clip(ry - 12, 0, None) * 0.7) | (ry >= 20)
    set_(img, skin, SK["f"])
    set_(img, skin & right, SK["e"])
    hi = ellipse(21, 26, 4, 7, 64, 64) & (pix(seed + 3, 2, 64, 64) > 0.35) & skin
    set_(img, hi, SK["hi"])
    hairm = ~skin
    side_rim = skin & (np.roll(hairm, 2, 1) | np.roll(hairm, -2, 1) | np.roll(hairm, 3, 0))
    set_(img, side_rim, SK["d"])
    set_(img, skin & ((rx < 1) | (rx > 31)) & (ry > 13), SK["d"])
    # eyes (white outside, iris inside), nose, mouth
    for x0, x1, c in ((4, 8, "#ffffff"), (8, 12, "#523d89"), (20, 24, "#523d89"), (24, 28, "#ffffff")):
        set_(img, (rx >= x0) & (rx < x1) & (ry >= 16) & (ry < 20), c)
    set_(img, (rx >= 8) & (rx < 12) & (ry >= 19) & (ry < 20), "#46337a")
    set_(img, (rx >= 20) & (rx < 24) & (ry >= 19) & (ry < 20), "#46337a")
    set_(img, (rx >= 12) & (rx < 20) & (ry >= 20) & (ry < 24), "#6a4030")
    set_(img, (rx >= 12) & (rx < 20) & (ry >= 23.5) & (ry < 24), "#5e3829")
    # lower cheeks: mottled darker skin
    low = (ry >= 26) & ((rx < 8) | (rx >= 24))
    set_(img, low, pick(seed + 4, 64, 64, (SK["b"], SK["a"], SK["8"], SK["d"], SK["c"]), (4, 3, 2, 2, 1)))
    set_(img, (ry >= 24) & (ry < 26) & ((rx < 2) | (rx >= 30)), SK["d"])
    # stubble beard around the mouth
    beard = ((ry >= 24) & (ry < 26) & (((rx >= 9) & (rx < 12)) | ((rx >= 20) & (rx < 23))))
    beard |= (ry >= 25) & (((rx >= 8) & (rx < 12)) | ((rx >= 20) & (rx < 24)))
    beard |= (ry >= 28) & (rx >= 8) & (rx < 24)
    set_(img, beard, pick(seed + 5, 64, 64, BEARD, (3, 2), 2))
    set_(img, (rx >= 12) & (rx < 20) & (ry >= 24) & (ry < 28), "#774235")
    set_(img, (rx >= 12) & (rx < 20) & (ry >= 24) & (ry < 25), "#6a3a2e")
    return img


def head_side(seed):
    """The right side (back of the head on the left, face on the right)."""
    img = hair(seed, 64, 64)
    ry, rx = refgrid(64, 64, 2)
    L = np.interp(ry, [16, 18, 20, 22, 24, 26, 28, 29, 30], [22, 20, 19, 17, 15, 12, 7, 3, 0])
    R = np.interp(ry, [16, 17, 18, 19, 20, 21, 21.9, 22], [25, 26, 27, 28, 28, 29, 29, 33])
    skin = (ry >= 16) & (rx >= L) & (rx < R)
    set_(img, skin, SK["d"])
    rim = skin & (np.roll(~skin, 2, 1) | np.roll(~skin, 2, 0))
    set_(img, rim, pick(seed + 1, 64, 64, (SK["a"], SK["8"]), (2, 1), 1))
    corner = skin & (rx > 23) & (ry > 26 - (rx - 23) * 0.3)
    set_(img, corner, pick(seed + 2, 64, 64, (SK["b"], SK["8"], SK["d"]), (3, 2, 1)))
    return img


def head_bottom(seed):
    ry, rx = refgrid(64, 64, 2)
    img = canvas(64, 64, pick(seed, 64, 64, (SK["8"],), None))
    inner = ((np.abs(rx - 16) / 12) ** 4 + (np.abs(ry - 13) / 10) ** 4) < 1
    set_(img, inner, SK["2"])
    corners = (ry > 20) & ~inner & (np.abs(rx - 16) > 9)
    set_(img, corners, SK["a"])
    low = ry >= 27
    set_(img, low, pick(seed + 1, 64, 64, (SK["8"], SK["b"], SK["a"]), (3, 2, 1)))
    beard = (np.abs(rx - 16) < 9) & (ry >= 28)
    set_(img, beard, BEARD[1])
    set_(img, (np.abs(rx - 16) < 8) & (ry >= 28.5), BEARD[0])
    return img


def shirt_tex(seed, h, w, base="5", fold="4"):
    n = aniso(seed, h, w, 18, 3.5)
    rgb = np.where((n < 0.36)[..., None], hexc(SHIRT[fold]), hexc(SHIRT[base]))
    fine = aniso(seed + 1, h, w, 5, 1.5) > 0.82
    rgb[fine] = hexc(SHIRT[fold])
    return canvas(h, w, rgb)


def pants_tex(seed, h, w, lines=True):
    n = aniso(seed, h, w, 14, 6)
    rgb = np.where((n > 0.64)[..., None], hexc(PANTS["hi"]), hexc(PANTS["m"]))
    img = canvas(h, w, rgb)
    if lines:  # a few vertical folds
        r = rng(seed + 1)
        yy, xx = grid(h, w)
        for _ in range(max(1, w // 14)):
            x0, y0 = r.uniform(3, w - 3), r.uniform(0, h * 0.5)
            L, bend = r.uniform(h * 0.25, h * 0.6), r.uniform(-3, 3)
            t = np.clip((yy - y0) / L, 0, 1)
            m = (np.abs(xx - (x0 + bend * t * t)) < 1) & (yy >= y0) & (yy < y0 + L)
            set_(img, m, PANTS["d"])
    return img


def hem(img, seed, y0, tail=None):
    """Trousers below row `y0`; `tail` = (x0, x1, tip_x, depth): a shirt tail over them."""
    h, w = img.shape[:2]
    shirt = img.copy()
    img[y0:] = pants_tex(seed + 1, h - y0, w, lines=False)
    img[y0 : y0 + 2, :, :3] = hexc(PANTS["d"])
    if tail:
        x0, x1, tx, dep = tail
        m = polygon([(x0, y0 - 1), (x1, y0 - 1), (x1, y0 + dep * 0.4), (tx, y0 + dep), (x0, y0 + 1)], h, w)
        img[m] = shirt[np.clip(np.where(m)[0] - 12, 0, None), np.where(m)[1]]
        edge = m & ~ndimage.binary_erosion(m, np.ones((3, 3), bool)) & (grid(h, w)[0] > y0)
        set_(img, edge, SHIRT["0"])
        below = np.roll(m, 2, 0) & ~m & (grid(h, w)[0] > y0)
        set_(img, below, PANTS["d"])


def spikes(img, seed, cy, colors=("0", "d1")):
    """Faithful's dark zigzag band across the shirt's sides."""
    h, w = img.shape[:2]
    r = rng(seed)
    for layer, c in enumerate(colors):
        x = -2.0
        while x < w:
            bw = r.uniform(3, 6) * (1 - layer * 0.35)
            up = r.uniform(10, 26) * (1 - layer * 0.4)
            dn = r.uniform(10, 24) * (1 - layer * 0.4)
            m = polygon([(x, cy), (x + bw / 2, cy - up), (x + bw, cy), (x + bw / 2, cy + dn)], h, w)
            set_(img, m, SHIRT[c])
            x += bw * r.uniform(0.7, 1.1)


def body_faces(seed):
    k = 8
    f = {}
    # top: shirt shoulders with the neck opening
    top = shirt_tex(seed, 32, 64)
    ry, rx = refgrid(32, 64, 2)
    hole = ((rx - 16) / 13.0) ** 2 + ((ry - 8) / 9.5) ** 2 < 1
    set_(top, grow(hole, 2) & ~hole & (rx < 16), SHIRT["hi"])
    set_(top, hole, SK["d"])
    set_(top, ((rx - 16) / 9.5) ** 2 + ((ry - 8.5) / 7.5) ** 2 < 1, SK["a"])
    set_(top, hole & ~(((rx - 16.5) / 12.2) ** 2 + ((ry - 8.5) / 9) ** 2 < 1) & (rx > 16), SK["e"])
    f["top"] = top
    # front: V neck, folds, trousers at the bottom
    fr = shirt_tex(seed + 1, 96, 64)
    ry, rx = refgrid(96, 64, 2)
    set_(fr, (rx < 2) | ((rx < 4) & (ry > 30)), SHIRT["2"])
    hw = np.interp(ry, [0, 4, 6, 8.5], [10, 6, 3, 0])
    vn = np.abs(rx - 16) < hw
    set_(fr, grow(vn, 3) & ~vn, SHIRT["2"])
    set_(fr, grow(vn, 1) & ~vn & (ry > 2), SHIRT["0"])
    set_(fr, vn, SK["d"])
    set_(fr, vn & (np.abs(rx - 16) > hw - 1) & (rx > 16), SK["e"])
    hem(fr, seed + 2, 80, (30, 60, 46, 12))
    f["front"] = fr
    for name, s in (("right", 3), ("left", 4)):
        sd = shirt_tex(seed + s, 96, 32, "2", "0")
        spikes(sd, seed + s + 10, 49)
        hem(sd, seed + s + 20, 80)
        f[name] = sd if name == "right" else sd[:, ::-1].copy()
    bk = shirt_tex(seed + 5, 96, 64)
    ry, rx = refgrid(96, 64, 2)
    set_(bk, rx >= 30, SHIRT["2"])
    spikes(bk, seed + 15, 49, ("4",))
    hem(bk, seed + 6, 80, (4, 34, 18, 10))
    f["back"] = bk
    f["bottom"] = canvas(32, 64, np.broadcast_to(hexc(PANTS["d"]), (32, 64, 3)).copy())
    return f


def creases(img, seed, n, y_range, color):
    """Short vertical skin creases, some forking (Faithful's arm lines)."""
    h, w = img.shape[:2]
    r = rng(seed)
    yy, xx = grid(h, w)
    for _ in range(n):
        x, y0 = r.uniform(2, w - 2), r.uniform(*y_range)
        L = r.uniform(8, 22)
        set_(img, (np.abs(xx - x) < 1) & (yy >= y0) & (yy < min(h - 1, y0 + L)), color)
        if r.random() < 0.6:
            d = r.choice([-1, 1])
            set_(img, thick_line((x, y0 + L * 0.5), (x + d * 4, y0 + L * 0.5 - 6), 2, h, w), color)


def arm_faces(seed, inner_face="left"):
    f = {}
    for i, name in enumerate(("right", "front", "left", "back")):
        inner = name == inner_face
        img = canvas(96, 32, np.broadcast_to(hexc(SK["d"] if inner else SK["e"]), (96, 32, 3)).copy())
        creases(img, seed + i, 5, (40, 80), SK["a"] if inner else SK["d"])
        sleeve = shirt_tex(seed + 10 + i, 96, 32, "d2" if inner else "5", "d3" if inner else "4")
        img[:36] = sleeve[:36]
        set_(img, (grid(96, 32)[0] >= 33) & (grid(96, 32)[0] < 36), SHIRT["d3"] if inner else SHIRT["2"])
        set_(img, (grid(96, 32)[0] >= 36) & (grid(96, 32)[0] < 38), SK["a"] if inner else SK["d"])
        if inner:
            set_(img, (np.abs(grid(96, 32)[1] - 16) < 7) & (grid(96, 32)[0] > 12) & (grid(96, 32)[0] < 34), SHIRT["d3"])
        else:
            for k in range(3):
                x0 = 4 + k * 9 + i * 2
                set_(img, thick_line((x0, 26), (x0 + 5, 8), 2, 96, 32), SHIRT["hi2"])
        f[name] = img
    top = shirt_tex(seed + 30, 32, 32)
    set_(top, thick_line((5, 26), (26, 6), 2.5, 32, 32), SHIRT["hi2"])
    set_(top, thick_line((4, 14), (14, 4), 2, 32, 32), SHIRT["hi2"])
    f["top"] = top
    hand = canvas(32, 32, np.broadcast_to(hexc(SK["e"]), (32, 32, 3)).copy())
    for y in (6, 13, 20):
        set_(hand, (np.abs(grid(32, 32)[0] - y) < 1) & (grid(32, 32)[1] > 20), SK["d"])
    f["bottom"] = hand
    return f


def leg_faces(seed):
    f = {}
    for i, name in enumerate(("right", "front", "left", "back")):
        img = pants_tex(seed + i, 96, 32)
        yy, xx = grid(96, 32)
        if name == "front":
            smile = (np.abs(np.hypot((xx - 16) / 1.3, (yy - 30) * 1.0) - 12) < 1) & (yy > 32)
            set_(img, smile, PANTS["d"])
        # shoes: bottom 2 units with a rounded top edge
        top = 80 + 3 * (np.abs(xx - 16) / 16) ** 2
        sh = yy >= top
        set_(img, sh, pick(seed + 10 + i, 96, 32, (SHOE["m"], SHOE["hi"]), (12, 1)))
        set_(img, sh & (yy < top + 2), SHOE["d"])
        set_(img, sh & (yy >= 94), SHOE["d"])
        f[name] = img
    f["top"] = canvas(32, 32, np.broadcast_to(hexc(PANTS["d"]), (32, 32, 3)).copy())
    sole = canvas(32, 32, np.broadcast_to(hexc(SHOE["m"]), (32, 32, 3)).copy())
    yy, xx = grid(32, 32)
    set_(sole, ((yy.astype(int) // 2) % 3 == 0), SHOE["d"])
    f["bottom"] = sole
    return f


def steve(seed: int) -> np.ndarray:
    k = 8
    a = blank(512, 512)
    hd = box_uv(0, 0, 8, 8, 8)
    put(a, hair(seed + 1, 64, 64), *hd["top"][:2], k)
    put(a, hair(seed + 2, 64, 64), *hd["back"][:2], k)
    put(a, head_front(seed + 3), *hd["front"][:2], k)
    put(a, head_side(seed + 4), *hd["right"][:2], k)
    put(a, head_side(seed + 5)[:, ::-1], *hd["left"][:2], k)
    put(a, head_bottom(seed + 6), *hd["bottom"][:2], k)
    for name, img in body_faces(seed + 10).items():
        put(a, img, *box_uv(16, 16, 8, 12, 4)[name][:2], k)
    for s, (u, v) in enumerate(((40, 16), (32, 48))):
        for name, img in arm_faces(seed + 40 + s * 50, "left" if s == 0 else "right").items():
            put(a, img, *box_uv(u, v, 4, 12, 4)[name][:2], k)
    for s, (u, v) in enumerate(((0, 16), (16, 48))):
        for name, img in leg_faces(seed + 150 + s * 50).items():
            put(a, img, *box_uv(u, v, 4, 12, 4)[name][:2], k)
    return a


# ---------------------------------------------------------------------------- pig

PIG = Ramp("#be504d", "#c6615a", "#e4686a", "#e67973", "#e68583", "#e6918b", "#f19e98", "#eea5a4")


def pig_face(w, h, field):
    """A pig face: `field(u, v)` (0..1 over the face) quantized to the pig palette."""
    yy, xx = grid(h, w)
    return canvas(h, w, PIG.shade(np.clip(field(xx / w, yy / h), 0, 1), 0.0))


def P(i):
    return PIG.at(i)


def pig(seed: int) -> np.ndarray:
    k = 2
    a = blank(128, 128)
    hd = box_uv(0, 0, 8, 8, 8)

    def lit(u, v, base=5, top=1.2, edge=1.0):
        """Rounded pillow: lighter toward the top center, darker to the sides/bottom (in steps)."""
        return (base + top * np.clip(0.6 - v, 0, 1) * (1 - np.abs(u - 0.5) * 1.6)
                - edge * np.clip(np.abs(u - 0.5) * 2 - 0.75, 0, 1) * 4) / 7

    put(a, pig_face(16, 16, lambda u, v: lit(u, v, 5.4, 3.0) + np.sin(u * 22) * 0.02), *hd["top"][:2], k)
    put(a, pig_face(16, 16, lambda u, v: (1.4 + 0 * u - 0.3 * v) / 7), *hd["bottom"][:2], k)
    put(a, pig_face(16, 16, lambda u, v: (5.3 - 1.5 * np.clip(np.hypot(u - 0.5, v - 0.1) - 0.6, 0, 1) * 4) / 7),
        *hd["back"][:2], k)

    def front(u, v):
        t = 5.4 + 1.6 * np.clip(0.45 - v, 0, 1) * 2 * (np.abs(u - 0.5) < 0.3)
        t -= 1.0 * (np.abs(u - 0.5) > 0.44) * (v > 0.5)
        # rounded lower cheeks (only the lower half: the top of the face stays smooth)
        t -= 2.0 * np.clip(np.hypot(np.abs(u - 0.5) - 0.3, v - 0.75) - 0.18, 0, 1) * 5 * (v > 0.5)
        return t / 7

    fr = pig_face(16, 16, front)
    fr[8:10, 4:12, :3] = P(4)  # the snout's base between the eyes
    fr[7:8, 4:12, :3] = P(6)
    fr[8:10, 0:2, :3] = hexc("#020001")
    fr[8:10, 2:4, :3] = 255
    fr[8:10, 12:14, :3] = 255
    fr[8:10, 14:16, :3] = hexc("#020001")
    put(a, fr, *hd["front"][:2], k)

    def side(u, v):
        d = np.hypot(u - 1.0, v)
        t = 5.3 + 1.2 * (v < 0.35) * (u > 0.5) - np.clip((d - 0.9) / 0.35, 0, 1) * 4.4
        return t / 7

    sd = pig_face(16, 16, side)
    ear_o = ellipse(8.5, 10, 3.5, 1.8, 16, 16)
    ear_i = ellipse(8.5, 10, 2.5, 0.9, 16, 16)
    sd[ear_o, :3] = P(2)
    sd[ear_i, :3] = P(1)
    put(a, sd, *hd["right"][:2], k)
    put(a, sd[:, ::-1].copy(), *hd["left"][:2], k)

    sn = box_uv(16, 16, 4, 3, 1)
    snf = pig_face(8, 6, lambda u, v: (6.3 - 0.8 * (v > 0.8)) / 7 + 0 * u)
    snf[2:4, 1:3, :3] = P(0)
    snf[2:4, 5:7, :3] = P(0)
    snf[2:3, 2:3, :3] = P(1)
    snf[2:3, 5:6, :3] = P(1)
    put(a, snf, *sn["front"][:2], k)
    put(a, pig_face(8, 2, lambda u, v: 7 / 7 + 0 * u), *sn["top"][:2], k)
    put(a, pig_face(8, 2, lambda u, v: 3 / 7 + 0 * u), *sn["bottom"][:2], k)
    for n in ("right", "left"):
        put(a, pig_face(2, 6, lambda u, v: (5 - v) / 7 + 0 * u), *sn[n][:2], k)
    put(a, pig_face(8, 6, lambda u, v: 4 / 7 + 0 * u), *sn["back"][:2], k)

    bd = box_uv(28, 8, 10, 16, 8)
    r = rng(seed)

    def flank(to_belly):
        def fn(u, v):
            b = u if to_belly > 0 else 1 - u
            t = 6.6 - 1.6 * np.clip(b - 0.55, 0, 1) * 2.5
            t -= 1.5 * np.clip(np.abs(v - 0.5) * 2 - 0.8, 0, 1) * 5
            return t / 7
        img = pig_face(16, 32, fn)
        for _ in range(3):
            x = int(r.integers(2, 13))
            y0 = int(r.integers(3, 12))
            img[y0 : y0 + int(r.integers(10, 18)), x, :3] = P(5)
        return img

    put(a, flank(1), *bd["right"][:2], k)
    put(a, flank(-1), *bd["left"][:2], k)
    belly = pig_face(20, 32, lambda u, v: (2.3 - 1.2 * np.clip(np.abs(u - 0.5) * 2 - 0.8, 0, 1) * 5
                                          + 0.9 * (np.abs(u - 0.5) > 0.35) * (np.abs(v - 0.5) < 0.42)) / 7)
    yy, xx = grid(32, 20)
    band = (np.abs(xx - 10) < 1.2) & (yy > 5) & (yy < 28)
    for cy in (8, 14.5, 21, 26):
        band |= (np.abs(xx - 10) / 3.2 + np.abs(yy - cy) / 2.6) < 1
    belly[band, :3] = P(1)
    put(a, belly, *bd["front"][:2], k)

    def back(u, v):
        t = 6.8 - 1.2 * np.clip(np.abs(u - 0.5) * 2 - 0.6, 0, 1) * 2.5
        t -= 1.5 * np.clip(np.abs(v - 0.5) * 2 - 0.85, 0, 1) * 6
        return t / 7

    bk = pig_face(20, 32, back)
    for x in (4, 9, 15):
        bk[4 : 4 + int(r.integers(12, 22)), x, :3] = P(5)
    put(a, bk, *bd["back"][:2], k)
    put(a, pig_face(20, 16, lambda u, v: (3.5 + 1.8 * np.clip(0.9 - np.hypot(u - 0.5, (v - 1.0) * 0.8) * 1.4, 0, 1)) / 7),
        *bd["top"][:2], k)
    rear = pig_face(20, 16, lambda u, v: (4.8 + 0.8 * (v < 0.5) - 1.5 * np.clip(np.abs(u - 0.5) * 2 - 0.85, 0, 1) * 5) / 7)
    yy, xx = grid(16, 20)
    cx, cy = 10.5, 7.5
    ang = np.arctan2(yy - cy, xx - cx)
    rad = np.hypot(yy - cy, xx - cx)
    spiral = (np.abs(((rad - (ang + np.pi) / (2 * np.pi) * 2.6) % 2.6) - 1.3) < 0.5) & (rad < 5.2) & (rad > 0.6)
    rear[spiral, :3] = P(0)
    put(a, rear, *bd["bottom"][:2], k)

    lg = box_uv(0, 16, 4, 6, 4)
    for i, n in enumerate(("right", "front", "left", "back")):
        m = aniso(seed + 5 + i, 12, 8, 3, 3)
        leg = canvas(12, 8, PIG.shade(np.clip((4.4 + (m > 0.6) * 1.5 - (m < 0.3) * 1.0) / 7, 0, 1), 0.0))
        leg[11, 1:7:2, :3] = P(0)
        put(a, leg, *lg[n][:2], k)
    put(a, pig_face(8, 8, lambda u, v: 5.5 / 7 + 0 * u), *lg["top"][:2], k)
    hoof = pig_face(8, 8, lambda u, v: 5 / 7 + 0 * u)
    for x in (1, 3, 5, 7):
        hoof[1:7, x - 1 if x == 7 else x, :3] = P(0)
    put(a, hoof, *lg["bottom"][:2], k)
    return a


# ---------------------------------------------------------------------------- sheep

FLEECE = Ramp("#d2d2d2", "#dedede", "#ececec", "#f8f6f5")
WOOL = Ramp("#d4d4d4", "#dfdfdf", "#efefef", "#f8f8f8", "#ffffff")
TAN = {"d": "#af886b", "m": "#b7947b", "l": "#c09e86", "hoof": "#57463a"}


def fleece(seed, h, w, ramp=FLEECE, bias=0.0):
    t = noise(seed, 4, h, w, tile=False) * 0.5 + pix(seed + 1, 1, h, w) * 0.5 + bias
    return canvas(h, w, ramp.shade(np.clip(t, 0, 1), 0.0))


def chevrons(img, seed, density, colors, mask=None):
    """Faithful's fur marks: little V strokes."""
    h, w = img.shape[:2]
    r = rng(seed)
    n = int(h * w * density / 4)
    c = cols(*colors)
    for _ in range(n):
        y, x = int(r.integers(0, h - 1)), int(r.integers(0, w - 2))
        if mask is not None and not mask[y, x]:
            continue
        col = c[r.integers(0, len(c))]
        if r.random() < 0.6:  # V
            pts = [(y, x), (y + 1, x + 1), (y, x + 2)]
        else:  # slash
            pts = [(y, x), (y + 1, x + 1)] if r.random() < 0.5 else [(y + 1, x), (y, x + 1)]
        for py, px in pts:
            if py < h and px < w:
                img[py, px, :3] = col


def streaks(img, seed, density, colors):
    """Fleece stubble on sheared skin: little upward strokes and carets, 1 px wide."""
    h, w = img.shape[:2]
    r = rng(seed)
    c = cols(*colors)
    shapes = [[(0, 0), (1, 0)], [(0, 0), (1, 0), (2, 0)], [(0, 1), (1, 0), (1, 2)],
              [(0, 1), (1, 0), (1, 2), (2, 0)], [(0, 1), (1, 0)], [(0, 0), (1, 1)]]
    for _ in range(int(h * w * density / 3)):
        y, x = int(r.integers(0, h)), int(r.integers(0, w))
        col = c[r.integers(0, len(c))]
        for dy, dx in shapes[r.integers(0, len(shapes))]:
            if y + dy < h and x + dx < w:
                img[y + dy, x + dx, :3] = col


def tan(seed, h, w):
    n = noise(seed, 6, h, w, tile=False) * 0.7 + pix(seed + 1, 1, h, w) * 0.3
    rgb = np.where((n > 0.7)[..., None], hexc(TAN["l"]), np.where((n < 0.3)[..., None], hexc(TAN["d"]), hexc(TAN["m"])))
    return canvas(h, w, rgb)


def sheep(seed: int) -> np.ndarray:
    k = 2
    a = blank(64, 128)
    hd = box_uv(0, 0, 6, 6, 8)
    for i, n in enumerate(("top", "bottom", "right", "left", "back")):
        put(a, fleece(seed + i, hd[n][3] * k, hd[n][2] * k), *hd[n][:2], k)
    # face: a rounded tan patch with eyes and a pink nose
    fr = fleece(seed + 9, 12, 12)
    yy, xx = grid(12, 12)
    face = (np.abs(xx - 6) < 5.4 - np.clip(3 - yy, 0, None) * 1.0 - np.clip(yy - 9.5, 0, None) * 1.2) & (yy > 0.5)
    fr[face, :3] = hexc(TAN["m"])
    rim = face & ~ndimage.binary_erosion(face)
    fr[rim & (yy > 6), :3] = hexc(TAN["d"])
    fr[4:6, 0:2, :3] = 0
    fr[4:6, 2:4, :3] = 255
    fr[4:6, 8:10, :3] = 255
    fr[4:6, 10:12, :3] = 0
    fr[8:10, 4:8, :3] = hexc("#ffb8b8")
    fr[9:10, 5:7, :3] = hexc("#e69494")
    fr[8:9, 4:5, :3] = hexc("#e69494")
    fr[8:9, 7:8, :3] = hexc("#e69494")
    put(a, fr, *hd["front"][:2], k)
    # ears on the head sides next to the face (leaf shapes)
    for n, flip in (("right", False), ("left", True)):
        x, y = hd[n][0] * k, hd[n][1] * k
        ear = polygon([(9, 2), (13.5, 2.5), (13, 6), (11, 7.5)], 12, 16)
        inner = polygon([(10, 2.7), (12.8, 3), (12, 4.8)], 12, 16)
        img = a[y : y + 12, x : x + 16]
        e, ii = (ear[:, ::-1], inner[:, ::-1]) if flip else (ear, inner)
        img[e, :3] = hexc(TAN["m"])
        img[e & ~ndimage.binary_erosion(e), :3] = hexc(TAN["d"])
        img[ii, :3] = hexc("#ffb8b8")
    bd = box_uv(28, 8, 8, 16, 6)
    for i, (n, (x, y, w, h)) in enumerate(bd.items()):
        img = tan(seed + 20 + i, h * k, w * k)
        streaks(img, seed + 30 + i, 0.3, ("#f8f6f5", "#ececec", "#dedede", "#f8f6f5"))
        put(a, img, x, y, k)
    lg = box_uv(0, 16, 4, 12, 4)
    for i, n in enumerate(("right", "front", "left", "back")):
        x, y, w, h = lg[n]
        img = tan(seed + 40 + i, 24, 8)
        img[:8] = fleece(seed + 50 + i, 8, 8)
        img[23, :, :3] = hexc(TAN["hoof"])
        put(a, img, x, y, k)
    put(a, fleece(seed + 60, 8, 8), *lg["top"][:2], k)
    sole = tan(seed + 61, 8, 8)
    sole[:, 0, :3] = sole[:, 7, :3] = sole[7, :, :3] = hexc(TAN["hoof"])
    put(a, sole, *lg["bottom"][:2], k)
    return a


def wool_face(seed, h, w, shade):
    """White fleece with fur marks; `shade(u, v)` darkens it (0 bright .. 1 grey)."""
    yy, xx = grid(h, w)
    t = noise(seed, 3, h, w, tile=False) * 0.3 + pix(seed + 1, 1, h, w) * 0.15 + 0.62
    # spiky edges on the grey areas (fur hanging over them)
    jv = (pix(seed + 3, 1, 1, w)[0][None, :] - 0.5) * 0.35
    ju = (pix(seed + 4, 1, h, 1)[:, 0][:, None] - 0.5) * 0.25
    ju = ju + (pix(seed + 5, 1, h, w) - 0.5) * 0.12
    sh = shade(np.clip(xx / w + ju, 0, 1), np.clip(yy / h + jv, 0, 1))
    t -= sh
    img = canvas(h, w, WOOL.shade(np.clip(t, 0, 1), 0.0))
    marks = img.copy()
    chevrons(marks, seed + 2, 0.22, ("#000000",))
    m = marks[..., 0] < 1
    darker = WOOL.shade(np.clip(t - 0.3, 0, 1), 0.0)
    img[m, :3] = darker[m]
    return img


def sheep_wool(seed: int) -> np.ndarray:
    k = 2
    a = blank(64, 128)
    flat = lambda u, v: 0.0 * u  # noqa: E731
    grey = lambda u, v: 0.85 + 0.0 * u  # noqa: E731
    hd = box_uv(0, 0, 6, 6, 6)
    for i, (n, (x, y, w, h)) in enumerate(hd.items()):
        sh = {"bottom": grey, "back": lambda u, v: 0.55 + 0.0 * u}.get(n, lambda u, v: 0.4 * np.clip(v - 0.75, 0, 1) * 4)
        put(a, wool_face(seed + i, h * k, w * k, sh), x, y, k)
    bd = box_uv(28, 8, 8, 16, 6)
    for i, (n, (x, y, w, h)) in enumerate(bd.items()):
        if n == "front":  # the belly: a grey band along the middle
            sh = lambda u, v: 0.8 * np.clip(1.7 - np.abs(u - 0.45) * 4.5, 0, 1) + 0.05  # noqa: E731
        elif n in ("top", "bottom"):
            sh = lambda u, v: 0.6 * (v > 0.62)  # noqa: E731
        elif n == "back":
            sh = flat
        else:
            sh = lambda u, v: 0.3 * np.clip(np.abs(u - 0.5) * 2 - 0.5, 0, 1) * 2  # noqa: E731
        put(a, wool_face(seed + 10 + i, h * k, w * k, sh), x, y, k)
    lg = box_uv(0, 16, 4, 6, 4)
    for i, (n, (x, y, w, h)) in enumerate(lg.items()):
        sh = grey if n == "bottom" else (lambda u, v: 0.35 * (np.abs(v - 0.55) < 0.2))
        put(a, wool_face(seed + 20 + i, h * k, w * k, sh), x, y, k)
    return a


# ---------------------------------------------------------------------------- particles

FIRE = {"red": "#ff0000", "orange": "#ff6a00", "yellow": "#ffd800", "core": "#fff5c6"}


def flame(seed: int) -> np.ndarray:
    """Faithful's flame: a tall drop filling the height, red tip, orange rim, yellow body and a
    pale core low down (32 px reference scaled by 4 in shape, drawn at full resolution)."""
    h = w = 128
    ry, rx = refgrid(h, w, 4)
    L = np.interp(ry, [0, 5, 10, 15, 20, 24, 27, 29, 30.5, 32], [15, 13, 12, 11, 9, 8, 8, 8.6, 9.6, 11.5])
    R = np.interp(ry, [0, 3, 5, 10, 15, 20, 24, 27, 29, 30.5, 32], [16, 17, 17, 19, 20, 21.5, 23, 24, 23.5, 22, 20.5])
    wob = (noise(seed, 8, h, w, tile=False) - 0.5) * 0.6
    body = (rx >= L + wob) & (rx < R + wob) & (ry < 31.9)
    img = blank(h, w)
    set_(img, body, FIRE["yellow"])
    d = ndimage.distance_transform_edt(body)
    set_(img, body & (d <= 3.5), FIRE["orange"])
    set_(img, body & (ry < 4.5), FIRE["red"])
    set_(img, body & (ry < 10) & (d <= 2.5) & (rx < 16.5), FIRE["red"])
    core = (((rx - 15.8) / 5.6) ** 2 + ((ry - 25.5) / 4.8) ** 2 < 1) | (((rx - 13) / 2.6) ** 2 + ((ry - 21.5) / 3.2) ** 2 < 1)
    set_(img, core & body & (d > 5), FIRE["core"])
    # a few flicker pixels at the rim of the core
    fl = grow(core & body & (d > 5), 2) & ~core & body & (pix(seed + 1, 2, h, w) > 0.7) & (d > 5)
    set_(img, fl, FIRE["core"])
    return img


# (radius of the dense center, outer radius) per smoke stage, in our pixels (Faithful x4)
SMOKE = [(3, 5), (6, 14), (9, 22), (11, 26), (15, 36), (19, 44), (26, 58), (34, 70)]


def smoke(i: int):
    def painter(seed: int) -> np.ndarray:
        h = w = 128
        img = blank(h, w)
        yy, xx = grid(h, w)
        c0, c1 = SMOKE[i]
        cy, cx = 67.0, 66.0
        if i == 0:
            set_(img, (np.abs(yy - cy) < 4) & (np.abs(xx - cx) < 4), "#ffffff")
            return img
        r = rng(seed + 9)
        ang = np.arctan2(yy - cy, xx - cx)
        lobes = 1 + 0.22 * np.sin(3 * ang + r.uniform(0, 6.3)) + 0.14 * np.sin(5 * ang + r.uniform(0, 6.3))
        rad = np.hypot(yy - cy, xx - cx) / (c1 * lobes)
        n = (noise(seed, max(4, c1 / 2.2), h, w, tile=False) * 0.42
             + noise(seed + 7, max(3, c1 / 5), h, w, tile=False) * 0.25 + pix(seed + 1, 3, h, w) * 0.33)
        edge = np.clip((rad - c0 / c1 * 0.5) / (1.1 - c0 / c1 * 0.5), 0, 1)
        m = n > 0.28 + edge * 0.5
        m &= rad < 1.2
        # stray specks around the puff
        m |= (pix(seed + 2, 3, h, w) > 0.975) & (rad < 1.1) & (rad > 0.6)
        set_(img, m, "#ffffff")
        return img

    return painter


TEXTURES = {
    "entity/chest/normal": chest_single,
    "entity/chest/normal_left": lambda s: chest_double(s, True),
    "entity/chest/normal_right": lambda s: chest_double(s, False),
    "entity/player/wide/steve": steve,
    "entity/pig/pig_temperate": pig,
    "entity/sheep/sheep": sheep,
    "entity/sheep/sheep_wool": sheep_wool,
    "particle/flame": flame,
    **{f"particle/generic_{i}": smoke(i) for i in range(8)},
}


# ---------------------------------------------------------------------------- debug previews


def _previews():
    """Full-size atlases with their UV faces boxed next to the reference, and the chest faces
    put together like the game does (python tools/texgen/entities.py)."""
    import zlib
    from pathlib import Path

    from PIL import Image, ImageDraw

    here = Path(__file__).parent
    out = here / "out"
    out.mkdir(exist_ok=True)

    def show(img, scale):
        h, w = img.shape[:2]
        bg = np.where((np.indices((h, w)).sum(0) // 4 % 2)[..., None] == 0, 60, 90).repeat(3, 2)
        al = img[..., 3:4] / 255.0
        rgb = (img[..., :3] * al + bg * (1 - al)).astype(np.uint8)
        return Image.fromarray(rgb, "RGB").resize((w * scale, h * scale), Image.NEAREST)

    boxes = {
        "entity/chest/normal": [box_uv(0, 0, 2, 4, 1), box_uv(0, 0, 14, 5, 14), box_uv(0, 19, 14, 10, 14)],
        "entity/chest/normal_left": [box_uv(0, 0, 1, 4, 1), box_uv(0, 0, 15, 5, 14), box_uv(0, 19, 15, 10, 14)],
        "entity/chest/normal_right": [box_uv(0, 0, 1, 4, 1), box_uv(0, 0, 15, 5, 14), box_uv(0, 19, 15, 10, 14)],
        "entity/player/wide/steve": [box_uv(0, 0, 8, 8, 8), box_uv(16, 16, 8, 12, 4), box_uv(40, 16, 4, 12, 4),
                                     box_uv(0, 16, 4, 12, 4), box_uv(32, 48, 4, 12, 4), box_uv(16, 48, 4, 12, 4)],
        "entity/pig/pig_temperate": [box_uv(0, 0, 8, 8, 8), box_uv(16, 16, 4, 3, 1), box_uv(28, 8, 10, 16, 8),
                                     box_uv(0, 16, 4, 6, 4)],
        "entity/sheep/sheep": [box_uv(0, 0, 6, 6, 8), box_uv(28, 8, 8, 16, 6), box_uv(0, 16, 4, 12, 4)],
        "entity/sheep/sheep_wool": [box_uv(0, 0, 6, 6, 6), box_uv(28, 8, 8, 16, 6), box_uv(0, 16, 4, 6, 4)],
    }
    for path, bl in boxes.items():
        img = TEXTURES[path](zlib.crc32(path.encode()) & 0x7FFFFFFF)
        w = img.shape[1]
        scale = max(1, 512 // w)
        im = show(img, scale)
        d = ImageDraw.Draw(im)
        u = w * scale / 64
        for bx in bl:
            for name, (x, y, fw, fh) in bx.items():
                d.rectangle([x * u, y * u, (x + fw) * u - 1, (y + fh) * u - 1], outline=(255, 0, 255))
        ref = Image.open(here / "reference" / f"{path}.png").convert("RGBA")
        ref = ref.resize(im.size, Image.NEAREST)
        both = Image.new("RGB", (im.size[0] * 2 + 8, im.size[1]), (0, 0, 0))
        base = Image.new("RGBA", ref.size, (75, 75, 75, 255))
        base.alpha_composite(ref)
        both.paste(base.convert("RGB"), (0, 0))
        both.paste(im, (im.size[0] + 8, 0))
        both.save(out / f"uv_{path.split('/')[-1]}.png")

    def region(img, x, y, w, h):
        k = img.shape[1] // 64
        return img[y * k : (y + h) * k, x * k : (x + w) * k]

    def face(img, lid_x, wdt=14):
        k = img.shape[1] // 64
        f = blank(16 * k, 16 * k)
        lid = region(img, lid_x, 14, wdt, 5)[::-1]
        body = region(img, lid_x, 33, wdt, 10)[::-1]
        lid = np.array(Image.fromarray(lid.astype(np.uint8), "RGBA").resize((wdt * k, 4 * k), Image.NEAREST), np.float32)
        f[2 * k : 6 * k, k : k + wdt * k] = lid
        f[6 * k : 16 * k, k : k + wdt * k] = body
        return f

    c = TEXTURES["entity/chest/normal"](1)
    le = TEXTURES["entity/chest/normal_left"](2)
    ri = TEXTURES["entity/chest/normal_right"](3)
    row = np.concatenate([face(c, 42), face(c, 0),
                          np.pad(region(c, 28, 0, 14, 14), ((8, 8), (8, 8), (0, 0))),
                          np.pad(region(c, 14, 0, 14, 14), ((8, 8), (8, 8), (0, 0))),
                          np.pad(region(c, 1, 1, 2, 4)[::-1], ((0, 96), (0, 112), (0, 0)))], 1)
    dbl = np.concatenate([face(le, 43, 15)[:, :-8], face(ri, 43, 15)[:, 8:]], 1)
    dbl2 = np.concatenate([face(ri, 14, 15)[:, :-8], face(le, 14, 15)[:, 8:]], 1)
    show(row, 1).save(out / "game_chest.png")
    show(np.concatenate([dbl, dbl2], 1), 1).save(out / "game_double_chest.png")


if __name__ == "__main__":
    _previews()
