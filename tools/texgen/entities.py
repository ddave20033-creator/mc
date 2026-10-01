"""Entity atlases (chest, player, pig, sheep, wolf) and particles (flame, smoke), in the look of
Faithful 64x redrawn at our sizes.

Atlases use Minecraft's box UV layout (`ModelPart.Cube`, see `src/entity/mob/model.rs` and the mobs in `src/content/mobs/`) in a 64 unit
wide atlas, all at 8 px per unit (chest, player and pig 512 x 512; sheep, its wool and the wolf
512 x 256; the game cuts the mobs' faces onto several texture layers, `src/textures/skin_pages.rs`).
Faithful's atlases are 4 px per unit, so shapes measured on them ("ref px") are doubled here.
Faces are painted in atlas orientation (the chest is stored upside down).
"""

from __future__ import annotations

import numpy as np
from scipy import ndimage

from common import Ramp, blank, ellipse, grid, grow, hexc, noise, pix, polygon, rng, shrink, thick_line

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
    if isinstance(color, str):
        color = hexc(color)
    color = np.asarray(color, np.float32)
    img[mask, :3] = color[mask][..., :3] if color.ndim == 3 else color
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

# Faithful's pig: soft pink contour bands (a rounded, pillowy shading quantized to 8 tones),
# painted here at 8 px per unit (512 x 512). Faces are described in "ref px" (Faithful's 4 per
# unit); every field is evaluated per our pixel, so the band edges are twice as fine.
PIG = Ramp("#be504d", "#c6615a", "#e4686a", "#e67973", "#e68583", "#e6918b", "#f19e98", "#eea5a4")
PIG_DARK = "#894746"
PIG_HI = "#fbbebe"
PK = 8


def P(i):
    return PIG.at(i)


def sup(rx, ry, cx, cy, ax, ay, p=2.0):
    """Superellipse radius (1 on its outline)."""
    return ((np.abs(rx - cx) / ax) ** p + (np.abs(ry - cy) / ay) ** p) ** (1.0 / p)


def tone_face(ramp, seed, wu, hu, field, wob=0.35):
    """A face `wu` x `hu` units: `field(rx, ry)` (ref px coordinates in the face) gives the tone
    0..n-1 of `ramp`, its band edges wobbling a little so they look hand drawn."""
    h, w = hu * PK, wu * PK
    yy, xx = grid(h, w)
    ry, rx = yy / 2.0, xx / 2.0
    t = field(rx, ry) + (aniso(seed, h, w, 7, 7) - 0.5) * wob
    idx = np.clip(np.round(t), 0, len(ramp) - 1).astype(int)
    return canvas(h, w, ramp.colors[idx]), rx, ry


def pig_face(seed, wu, hu, field, wob=0.35):
    return tone_face(PIG, seed, wu, hu, field, wob)


def pline(img, pts, color, width=0.5):
    """A thin crease through `pts` (ref px, a polyline), `width` ref px wide."""
    m = np.zeros(img.shape[:2], bool)
    for (x0, y0), (x1, y1) in zip(pts[:-1], pts[1:]):
        m |= thick_line((x0 * 2, y0 * 2), (x1 * 2, y1 * 2), width * 2, *img.shape[:2])
    set_(img, m, color)


def pig(seed: int) -> np.ndarray:
    a = blank(64 * PK, 64 * PK)
    hd = box_uv(0, 0, 8, 8, 8)

    def at(img, face):
        put(a, img, face[0], face[1], PK)

    # ---- head top: light, a paler rim darkening into the corners, faint lengthwise creases
    img, rx, ry = pig_face(seed + 1, 8, 8, lambda x, y: 7.4 - np.clip((sup(x, y, 16, 16, 15.5, 16.5, 4) - 0.86) * 9, 0, 2.4))
    for pts in (((8, 1), (8.5, 12), (8, 24)), ((17, 0), (17, 7)), ((24, 19), (24.5, 30))):
        pline(img, pts, P(6))
    at(img, hd["top"])
    # ---- under the chin: flat and dark
    img, rx, ry = pig_face(seed + 2, 8, 8, lambda x, y: 1.0 + 0 * x, 0.0)
    at(img, hd["bottom"])

    # ---- head sides: lit toward the front top, a heavy jowl shadow low at the back, the ear
    def side(x, y):
        t = 5.0 + 1.9 * np.clip((x - 6) / 26 - y / 13, 0, 1)
        t -= np.clip((sup(x, y, 21, 6, 20.5, 21.5, 2.6) - 0.97) * 16, 0, None)
        t -= (x < 1) * (y < 20) * 1.0
        t -= np.clip(y - 29.5, 0, None) * 0.5
        return t

    sd, rx, ry = pig_face(seed + 3, 8, 8, side)
    e = sup(rx, ry, 19.5, 16.5, 4.2, 7.2)
    sh = np.clip((rx - 19.5) / 4, -1, 1)
    set_(sd, e < 1.0, P(2))
    set_(sd, (e < 0.82) & (sh > -0.3), P(3))
    set_(sd, e < 0.66, P(2))
    set_(sd, e < 0.5, P(1))
    set_(sd, (e < 0.5) & (e > 0.36) & (sh > 0.4), P(2))
    at(sd, hd["right"])
    at(sd[:, ::-1].copy(), hd["left"])

    # ---- face: light brow, eyes, the snout's frame, round cheeks
    def front(x, y):
        t = 5.0 + 1.25 * np.clip((10.5 - y) / 4.0, 0, 1)
        t -= np.clip((sup(x, y, 16, 13, 17, 14.5, 3) - 0.92) * 10, 0, 1)
        t -= np.clip((sup(x, y, 16, 11, 18.5, 20.5, 3) - 0.97) * 10, 0, 1)
        return t

    fr, rx, ry = pig_face(seed + 4, 8, 8, front)
    for x0, wd in ((0.5, 1.8), (8, 4.6), (24, 4.6), (31.5, 1.8)):  # pale petals running into the brow
        cx_ = x0 + (16 - x0) * ry / 13
        set_(fr, (np.abs(rx - cx_) < wd * np.clip(1 - ry / 10.5, 0, 1) ** 0.6) & (ry < 10.5), P(7))
    set_(fr, (ry >= 16) & (ry < 17) & (np.abs(rx - 16) < 8), P(6))
    for xs in (8, 23.5):
        pline(fr, ((xs, 16.5), (xs + (xs - 16) * 0.05, 22), (xs + (xs - 16) * 0.12, 27)), P(4))
    bridge = sup(rx, ry, 16, 14, 5.5, 2.1, 4) < 1
    set_(fr, bridge, P(4))
    set_(fr, bridge & (ry < 13), P(3))
    for x0, c in ((0, "#020001"), (4, "#ffffff"), (24, "#ffffff"), (28, "#020001")):
        set_(fr, (rx >= x0) & (rx < x0 + 4) & (ry >= 12) & (ry < 16), c)
    set_(fr, (rx >= 4) & (rx < 8) & (ry >= 15.5) & (ry < 16), "#e9dcdc")
    set_(fr, (rx >= 24) & (rx < 28) & (ry >= 15.5) & (ry < 16), "#e9dcdc")
    set_(fr, (rx >= 0.5) & (rx < 1.5) & (ry >= 12.5) & (ry < 13.5), "#2a2426")
    set_(fr, (rx >= 28.5) & (rx < 29.5) & (ry >= 12.5) & (ry < 13.5), "#2a2426")
    at(fr, hd["front"])

    # ---- back of the head
    img, rx, ry = pig_face(seed + 5, 8, 8, lambda x, y: 5.0 - np.clip((sup(x, y, 16, 9, 16.5, 19, 3) - 0.97) * 12, 0, 4)
                           - ((x < 1) | (x > 31)) * 0.8)
    at(img, hd["back"])

    # ---- snout (16, 16): 4 x 3 x 1
    sn = box_uv(16, 16, 4, 3, 1)
    img, rx, ry = pig_face(seed + 6, 4, 1, lambda x, y: 6.3 + 0.5 * (x > 3) * (x < 12) + 0 * y, 0.2)
    set_(img, (rx < 4) & (ry < 2 + rx * 0.3), PIG_HI)
    set_(img, (rx < 2) & (ry >= 2), P(7))
    at(img, sn["top"])
    img, rx, ry = pig_face(seed + 7, 4, 1, lambda x, y: 2.0 + ((x < 1) | (x > 15)) * 1 + 0 * y, 0.0)
    at(img, sn["bottom"])

    def snf(x, y):
        s_ = sup(x, y, 8, 6.2, 8.2, 6.3, 3)
        t = 5.0 + (s_ < 0.93) * 1.0 - (s_ < 0.7) * 1.0 - ((x < 1) | (x > 15)) * 1.0
        t -= (y > 11) * 0.6 * ((x < 3) | (x > 13))
        return t

    img, rx, ry = pig_face(seed + 8, 4, 3, snf, 0.2)
    for cx in (4.5, 11.5):  # nostrils: dark rounded crosses on a slightly darker patch
        set_(img, sup(rx, ry, cx, 5.8, 2.6, 2.6, 1.6) < 1, P(3))
        m = (sup(rx, ry, cx, 5.8, 2.0, 0.9, 3) < 1) | (sup(rx, ry, cx, 5.8, 0.9, 2.1, 3) < 1)
        set_(img, m, PIG_DARK)
        set_(img, m & (rx < cx - 0.4) & (ry < 5.3), "#a0564f")
    at(img, sn["front"])
    for n in ("right", "left"):
        img, rx, ry = pig_face(seed + 9, 1, 3, lambda x, y: 5.4 - (y > 9) * 0.8 + 0 * x, 0.25)
        at(img, sn[n])
    img, rx, ry = pig_face(seed + 10, 4, 3, lambda x, y: 4.0 + 0 * x * y, 0.0)
    at(img, sn["back"])

    # ---- body (28, 8): 10 x 16 x 8, lying along the model's Y
    bd = box_uv(28, 8, 10, 16, 8)

    def flank(x, y):
        """The right flank: light along its upper edge (left), darker toward the belly."""
        t = np.interp(x, [0, 3, 17, 22, 29, 32], [7.4, 6.4, 6.2, 4.8, 3.9, 3.0])
        t -= np.clip((sup(x, y, 4, 32, 30, 32.5, 2.5) - 0.9) * 10, 0, 4)
        return t

    for n, flip in (("right", False), ("left", True)):
        img, rx, ry = pig_face(seed + 11 + flip, 8, 16, flank)
        pline(img, ((9.5, 6), (9, 30), (9.5, 56)), P(5))
        pline(img, ((18.5, 12), (18, 30), (18.5, 48)), P(4))
        r = rng(seed + 13 + flip)
        for _ in range(2):
            x0 = r.uniform(3, 16)
            y0 = r.uniform(10, 30)
            pline(img, ((x0, y0), (x0 + r.uniform(-0.5, 0.5), y0 + r.uniform(8, 16))), P(6))
        at(img if not flip else img[:, ::-1].copy(), bd[n])

    def belly(x, y):
        t = 2.0 + np.clip(1 - sup(x, y, 20, -2, 12, 5, 2), 0, 1) * 1.5
        t -= np.clip((sup(x, y, 20, 32, 21, 33, 3) - 0.93) * 8, 0, 1.2)
        t += np.clip(1 - sup(x, y, 20, 64, 9, 6, 2), 0, 1) * 2
        return t

    img, rx, ry = pig_face(seed + 15, 10, 16, belly)
    # the dark stripe down the belly: forked at the chest, dark diamonds along its sides
    band = (np.abs(rx - 19.5) < 7.0) & (ry > 13.5 - np.abs(rx - 19.5) * 0.25) & (ry < 58.5)
    band &= ~((ry < 16) & (np.abs(rx - 19.5) < 2.5 - (ry - 13.5)))
    band |= polygon([(2 * 12.6, 2 * 16), (2 * 9.2, 2 * 7.5), (2 * 11.2, 2 * 7.5), (2 * 16.5, 2 * 14.5)], *img.shape[:2])
    band |= polygon([(2 * 26.4, 2 * 16), (2 * 29.8, 2 * 7.5), (2 * 27.8, 2 * 7.5), (2 * 22.5, 2 * 14.5)], *img.shape[:2])
    set_(img, band, P(1))
    for cy in (21, 33, 45.5):
        for sx in (-1, 1):
            c_ = 19.5 + sx * 7.5
            set_(img, (np.abs(rx - c_) / 3.3 + np.abs(ry - cy) / 3.3) < 1, P(1))
            set_(img, (np.abs(rx - c_) / 2.4 + np.abs(ry - cy) / 2.4) < 1, P(0))
    at(img, bd["front"])

    def back(x, y):
        t = 7.3 - np.clip((sup(x, y, 20, 26, 18, 30, 2.6) - 0.85) * 9, 0, 1.4)
        t -= np.clip((sup(x, y, 20, 24, 21, 40, 2.6) - 0.93) * 9, 0, 4)
        return t

    img, rx, ry = pig_face(seed + 16, 10, 16, back)
    for x0, y0, y1 in ((13, 3, 42), (24, 5, 40), (34, 3, 30)):
        pline(img, ((x0, y0), (x0 - 0.3, (y0 + y1) / 2), (x0, y1)), P(5))
    at(img, bd["back"])

    def neck(x, y):  # the body's front end (its top in the atlas): a bowl of bands round the neck
        d = np.hypot(x - 20, (y - 14) / 1.05)
        t = np.where((d < 17.6) & (y > 11), 4.0, 3.0)
        t = np.where((d > 13.5) & (d < 16.3) & (y > 20), 5.0, t)
        t = np.where(sup(x, y, 20, 6, 16, 18.5) < 1, 3.0, t)
        return t - np.clip((sup(x, y, 20, 5, 22.5, 27, 2.3) - 0.93) * 7, 0, 2.2)

    img, rx, ry = pig_face(seed + 17, 10, 8, neck)
    at(img, bd["top"])

    def rear(x, y):
        t = 4.0 + np.clip(1 - sup(x, y, 20, -4, 12, 11, 2), 0, 1) * 3
        t -= np.clip((sup(x, y, 20, 8, 21, 25, 2.5) - 0.97) * 8, 0, 3)
        t -= (x > 39) * 1.0
        return t

    img, rx, ry = pig_face(seed + 18, 10, 8, rear)
    # the curly tail: a dark ring (its root) round a spiral
    cx, cy = 19.5, 17.5
    d = np.hypot(rx - cx, ry - cy)
    ang = np.arctan2(ry - cy, rx - cx)
    set_(img, (np.abs(d - 8.2) < 0.55) & ~((ang > -0.9) & (ang < 0.1)), P(0))
    set_(img, (np.abs(d - 8.9) < 0.4) & ((ang > 0.6) & (ang < 2.5)), P(1))
    turn = (ang + np.pi) / (2 * np.pi)
    arm = 1.0 + (turn + np.floor((d - 1.0 - turn * 2.8) / 2.8)) * 2.8
    spiral = (np.abs(d - arm) < 0.5) & (d < 5.4) & (d > 0.7)
    spiral |= (np.abs(d - (5.5 + (ang - 0.3) * 0.8)) < 0.5) & (ang > 0.3) & (ang < 1.4)
    set_(img, spiral, PIG_DARK)
    at(img, bd["bottom"])

    # ---- legs (0, 16): 4 x 6 x 4, blotchy
    lg = box_uv(0, 16, 4, 6, 4)
    for i, n in enumerate(("right", "front", "left", "back")):
        def blot(x, y, i=i):
            m2 = aniso(seed + 50 + i, 6 * PK, 4 * PK, 7, 6)
            m = aniso(seed + 40 + i, 6 * PK, 4 * PK, 22, 18) + (m2 - 0.5) * 0.25
            return 5.0 + (m > 0.6) * 1.0 + (m > 0.78) * 0.6 - (m < 0.4) * 1.0 - (m < 0.26) * 1.0 + 0 * x
        img, rx, ry = pig_face(seed + 30 + i, 4, 6, blot, 0.25)
        if n in ("front", "back"):  # the hoof's toe lines along the bottom
            for k_ in range(4):
                xk = k_ * 4 + (0.5 if n == "front" else 3.5)
                set_(img, (np.abs(rx - xk) < 0.5) & (ry >= 22), PIG_DARK)
        at(img, lg[n])
    img, rx, ry = pig_face(seed + 35, 4, 4, lambda x, y: 6.0 - np.clip((sup(x, y, 0, 0, 16, 16, 2) - 0.75) * 6, 0, 1))
    at(img, lg["top"])
    img, rx, ry = pig_face(seed + 36, 4, 4, lambda x, y: 3.0 + 0 * x * y, 0.0)
    set_(img, (rx % 4) < 1, PIG_DARK)
    set_(img, ((rx % 4) >= 3) & ((rx % 4) < 3.5), P(2))
    at(img, lg["bottom"])
    return a


# ---------------------------------------------------------------------------- sheep

# Faithful's sheep at 8 px per unit (512 x 256): a marbled white fleece on the head and the
# legs' tops, a round tan face, and sheared tan skin covered in little white fleece tufts; the
# wool coat is white fleece with fine V marks, greyer underneath.
SK8 = 8
FLEECE = Ramp("#d2d2d2", "#dedede", "#ececec", "#f8f6f5")
WOOL = Ramp("#d4d4d4", "#dfdfdf", "#efefef", "#f8f8f8", "#ffffff")
TAN = {"d": "#af886b", "m": "#b7947b", "l": "#c09e86", "hoof": "#57463a"}


def fleece(seed, h, w, bias=0.0):
    """Marbled fleece: soft light clumps parted by greyer veins, with a fine grain."""
    t = aniso(seed, h, w, 12, 12) * 0.45 + aniso(seed + 1, h, w, 4, 4) * 0.35 + pix(seed + 2, 1, h, w) * 0.2
    t = (t - 0.2) * 1.6 + 0.08
    t -= np.clip(0.07 - np.abs(aniso(seed + 3, h, w, 9, 9) - 0.5), 0, None) * 4
    return canvas(h, w, FLEECE.shade(np.clip(t + bias, 0, 1), 0.3))


def tan(seed, h, w):
    """Sheared skin: mottled tan with darker veins."""
    n = aniso(seed, h, w, 14, 14)
    t = 0.5 + (aniso(seed + 1, h, w, 5, 5) - 0.5) * 0.5 + (pix(seed + 2, 1, h, w) - 0.5) * 0.15
    t -= np.clip(0.16 - np.abs(n - 0.5), 0, None) * 2.2
    rgb = np.where((t > 0.66)[..., None], hexc(TAN["l"]), np.where((t < 0.3)[..., None], hexc(TAN["d"]), hexc(TAN["m"])))
    return canvas(h, w, rgb)


def stroke(img, p0, p1, width, color, color2=None):
    """A short tapering stroke (x, y) -> (x, y) drawn in its bounding box; its right side in
    `color2` (shaded)."""
    h, w = img.shape[:2]
    x0 = int(max(0, min(p0[0], p1[0]) - width - 1))
    x1 = int(min(w, max(p0[0], p1[0]) + width + 2))
    y0 = int(max(0, min(p0[1], p1[1]) - width - 1))
    y1 = int(min(h, max(p0[1], p1[1]) + width + 2))
    if x1 <= x0 or y1 <= y0:
        return
    yy, xx = np.mgrid[y0:y1, x0:x1].astype(np.float32) + 0.5
    dx, dy = p1[0] - p0[0], p1[1] - p0[1]
    L2 = max(dx * dx + dy * dy, 1e-6)
    t = np.clip(((xx - p0[0]) * dx + (yy - p0[1]) * dy) / L2, 0, 1)
    px, py = p0[0] + t * dx, p0[1] + t * dy
    rad = width * 0.5 * (1.0 - 0.55 * t)
    d2 = (xx - px) ** 2 + (yy - py) ** 2
    m = d2 <= rad * rad
    sub = img[y0:y1, x0:x1]
    sub[m, :3] = hexc(color)
    sub[m, 3] = 255
    if color2 is not None:
        side = ((xx - px) * dy - (yy - py) * dx) > 0.35 * np.sqrt(L2)
        sub[m & side, :3] = hexc(color2)


def tufts(img, seed, count, region=None):
    """Little white fleece tufts left on sheared skin: strokes and V / Y shapes, mostly
    upright, lit on their left."""
    h, w = img.shape[:2]
    x0, y0, x1, y1 = region or (0, 0, w, h)
    r = rng(seed)
    cols_ = (("#f8f6f5", "#dedede"), ("#ececec", "#d2d2d2"), ("#f8f6f5", "#ececec"))
    for _ in range(count):
        x, y = r.uniform(x0 - 2, x1), r.uniform(y0 - 2, y1 + 6)
        c, c2 = cols_[r.integers(0, len(cols_))]
        L = r.uniform(5, 11)
        ang = -np.pi / 2 + r.normal(0, 0.35)
        kind = r.random()
        if kind < 0.45:  # a V: two strokes from a bottom point
            for sgn in (-1, 1):
                a_ = ang + sgn * r.uniform(0.22, 0.45)
                stroke(img, (x, y), (x + np.cos(a_) * L * 0.75, y + np.sin(a_) * L * 0.75), r.uniform(3.0, 4.0), c, c2)
        elif kind < 0.6:  # a Y
            mx, my = x + np.cos(ang) * L * 0.5, y + np.sin(ang) * L * 0.5
            stroke(img, (x, y), (mx, my), 3.6, c, c2)
            for sgn in (-1, 1):
                a_ = ang + sgn * 0.55
                stroke(img, (mx, my), (mx + np.cos(a_) * L * 0.5, my + np.sin(a_) * L * 0.5), 3.0, c, c2)
        else:
            stroke(img, (x, y), (x + np.cos(ang) * L, y + np.sin(ang) * L), r.uniform(3.2, 4.4), c, c2)


def sheep(seed: int) -> np.ndarray:
    k = SK8
    a = blank(32 * k, 64 * k)
    hd = box_uv(0, 0, 6, 6, 8)
    for i, n in enumerate(("top", "bottom", "right", "left", "back")):
        x, y, w, h = hd[n]
        put(a, fleece(seed + i, h * k, w * k, -0.12 if n in ("bottom", "back") else 0.0), x, y, k)
    # face: a round tan head with eyes and a pink nose
    fr = fleece(seed + 9, 48, 48)
    yy, xx = grid(48, 48)
    ry, rx = yy / 2, xx / 2  # ref px (24 x 24)
    face = sup(rx, ry, 12, 12.2, 12.3, 12.0, 2.4) < 1
    face &= ry > 0.6
    skin = canvas(48, 48, np.where((aniso(seed + 10, 48, 48, 6, 6) > 0.62)[..., None], hexc(TAN["l"]), hexc(TAN["m"])))
    set_(fr, face, skin)
    rim = face & ~shrink(face, 2)
    set_(fr, rim & ((ry > 12) | (rx > 18)), TAN["d"])
    set_(fr, rim & (ry <= 12) & (rx <= 18), TAN["l"])
    set_(fr, face & (sup(rx, ry, 8.5, 5.5, 4.5, 2.5, 2) < 1) & (pix(seed + 11, 2, 48, 48) > 0.45), TAN["l"])
    for x0, c in ((0, "#000000"), (4, "#ffffff"), (16, "#ffffff"), (20, "#000000")):
        set_(fr, (rx >= x0) & (rx < x0 + 4) & (ry >= 8) & (ry < 12), c)
    set_(fr, (((rx >= 4) & (rx < 8)) | ((rx >= 16) & (rx < 20))) & (ry >= 11.5) & (ry < 12), "#e2dcd8")
    nose = sup(rx, ry, 12, 18.6, 4.2, 2.6, 2.2) < 1
    set_(fr, nose, "#ffb8b8")
    set_(fr, nose & ((ry > 19.6) | (sup(rx, ry, 12, 18.6, 4.2, 2.6, 2.2) > 0.78) & (ry > 18.6)), "#e69494")
    set_(fr, nose & (sup(rx, ry, 10.6, 17.6, 1.5, 0.8) < 1), "#ffd2d2")
    put(a, fr, *hd["front"][:2], k)
    # ears on the head sides beside the face: tan leaves, pink inside, pointing up and out
    for n, flip in (("right", False), ("left", True)):
        x, y = hd[n][0] * k, hd[n][1] * k
        img = a[y : y + 48, x : x + 64]
        ey, ex = grid(48, 64)
        ery, erx = ey / 2, ex / 2
        if flip:
            erx = 32 - erx
        u = (erx - 21.5) * np.cos(0.75) + (ery - 9) * np.sin(0.75)
        v = -(erx - 21.5) * np.sin(0.75) + (ery - 9) * np.cos(0.75)
        ear = (u / 6.4) ** 2 + (v / (4.4 - np.clip(u, 0, None) * 0.35)) ** 2 < 1
        set_(img, ear, TAN["m"])
        set_(img, ear & (v > 1.6), TAN["d"])
        set_(img, ear & ~shrink(ear, 1) & (v <= 1.6), TAN["d"])
        inner = ((u + 2.0) / 3.6) ** 2 + ((v + 1.0) / 2.2) ** 2 < 1
        set_(img, inner & ear, "#ffb8b8")
        set_(img, inner & ear & (v > -0.2), "#e69494")
    # body: sheared skin with fleece tufts
    bd = box_uv(28, 8, 8, 16, 6)
    for i, (n, (x, y, w, h)) in enumerate(bd.items()):
        img = tan(seed + 20 + i, h * k, w * k)
        tufts(img, seed + 30 + i, int(w * h * 0.55))
        put(a, img, x, y, k)
    # legs: fleece on the upper third, tan below, the hoof's dark edge at the bottom
    lg = box_uv(0, 16, 4, 12, 4)
    for i, n in enumerate(("right", "front", "left", "back")):
        x, y, w, h = lg[n]
        img = tan(seed + 40 + i, 96, 32)
        edge = 32 + np.round((pix(seed + 45 + i, 2, 1, 32)[0] - 0.5) * 3).astype(int)
        top = fleece(seed + 50 + i, 40, 32, -0.05)
        for c in range(32):
            img[: edge[c], c] = top[: edge[c], c]
            img[edge[c] : edge[c] + 2, c, :3] = hexc(TAN["d"])
        img[94:, :, :3] = hexc(TAN["hoof"])
        img[92:94, :, :3] = hexc("#6b5646")
        put(a, img, x, y, k)
    put(a, fleece(seed + 60, 32, 32), *lg["top"][:2], k)
    sole = tan(seed + 61, 32, 32)
    sole[:, :2, :3] = sole[:, 30:, :3] = hexc(TAN["hoof"])
    sole[30:, :, :3] = sole[:2, :, :3] = hexc(TAN["hoof"])
    put(a, sole, *lg["bottom"][:2], k)
    return a


def wool_marks(img, seed, n, colors, size=(3, 6), mask=None):
    """Faithful's fine V marks on the wool (fur hanging down): two 1 px strokes meeting at a
    lower point."""
    h, w = img.shape[:2]
    chevron_marks(img, seed, n, colors, (0, 0, w, h), size, mask)


def wool_face(seed, h, w, shade):
    """White wool with fine V marks; `shade(u, v)` greys it (0 white .. 1 grey), its edges
    ragged like fur hanging over them."""
    yy, xx = grid(h, w)
    t = aniso(seed, h, w, 9, 9) * 0.3 + pix(seed + 1, 1, h, w) * 0.12 + 0.66
    jv = (aniso(seed + 3, h, w, 4, 3) - 0.5) * 0.3 + (pix(seed + 4, 2, h, w) - 0.5) * 0.06
    ju = (aniso(seed + 5, h, w, 3, 4) - 0.5) * 0.22 + (pix(seed + 6, 2, h, w) - 0.5) * 0.05
    t -= shade(np.clip(xx / w + ju, 0, 1), np.clip(yy / h + jv, 0, 1))
    img = canvas(h, w, WOOL.shade(np.clip(t, 0, 1), 0.15))
    marks = img.copy()
    wool_marks(marks, seed + 2, int(h * w / 55), ("#000000",), (3, 6))
    m = marks[..., 0] < 1
    darker = WOOL.shade(np.clip(t - 0.28, 0, 1), 0.0)
    img[m, :3] = darker[m]
    return img


def sheep_wool(seed: int) -> np.ndarray:
    k = SK8
    a = blank(32 * k, 64 * k)
    grey = lambda u, v: 0.8 + 0.0 * u  # noqa: E731
    hd = box_uv(0, 0, 6, 6, 6)
    for i, (n, (x, y, w, h)) in enumerate(hd.items()):
        sh = {"bottom": grey, "back": lambda u, v: 0.45 + 0.2 * np.clip(np.abs(u - 0.5) * 2 - 0.5, 0, 1)}.get(
            n, lambda u, v: 0.5 * np.clip(v - 0.72, 0, 1) * 4)
        put(a, wool_face(seed + i, h * k, w * k, sh), x, y, k)
    bd = box_uv(28, 8, 8, 16, 6)
    for i, (n, (x, y, w, h)) in enumerate(bd.items()):
        if n == "front":  # the belly: a grey band along the middle
            sh = lambda u, v: 0.75 * np.clip(1.45 - np.abs(u - 0.55) * 3.6, 0, 1) + 0.04  # noqa: E731
        elif n in ("top", "bottom"):
            sh = lambda u, v: 0.55 * (v > 0.64) + 0.15 * (v > 0.85)  # noqa: E731
        elif n == "back":
            sh = lambda u, v: 0.1 * np.clip(np.abs(u - 0.5) * 2 - 0.7, 0, 1) * 3  # noqa: E731
        else:
            sh = lambda u, v: 0.06 * np.clip(np.abs(u - 0.5) * 2 - 0.5, 0, 1) * 2  # noqa: E731
        put(a, wool_face(seed + 10 + i, h * k, w * k, sh), x, y, k)
    lg = box_uv(0, 16, 4, 6, 4)
    for i, (n, (x, y, w, h)) in enumerate(lg.items()):
        sh = grey if n == "bottom" else (lambda u, v: 0.3 * (np.abs(v - 0.5) < 0.2))
        put(a, wool_face(seed + 20 + i, h * k, w * k, sh), x, y, k)
    return a


# ---------------------------------------------------------------------------- wolf

# Faithful's wolf redrawn at 8 px per unit (512 x 256, twice Faithful 64x's detail): pale grey
# fur made of short strands, a tan muzzle with a grey arch, a black nose on a striped tan snout
# and a pale jaw. Shapes measured on Faithful ("ref px", 4 per unit) are doubled here.
WK = 8
WF = {"f0": "#b9b5b4", "f1": "#c1bebe", "f2": "#cac7c8", "f3": "#d3cfcf", "f4": "#dddadb",
      "f5": "#e6e3e4", "g1": "#b0aaa7", "g2": "#9f9a96", "g3": "#8d8782",
      "dark": "#393835", "dark2": "#2c2b29", "dark3": "#48463f", "black": "#121416",
      "nose_hi": "#393c3f", "glint": "#6d7278",
      "tan": "#a78f7e", "tan_l": "#b39c8b", "tan_d": "#9a8272", "brow": "#8c6f52",
      "arch": "#9a8c88", "arch_l": "#a69994", "arch_d": "#958679",
      "sn_l": "#ceaf96", "sn_m": "#c4ab9c", "sn_o": "#d4b5a4", "sn_hi": "#dcc1ae",
      "sn_d": "#947b68", "sn_dd": "#836b59", "jaw": "#e4d8d9", "jaw_d": "#d6c8c8",
      "white": "#ffffff", "white_d": "#e9e6e6", "red_d": "#b60f0f", "red": "#e42e2e",
      "red_dd": "#8c0b0b", "red_hi": "#ff7a6e",
      "paw": "#a1785b", "paw_d": "#8b654b", "paw_l": "#b08a6c", "leg_top": "#2d2d28",
      "tail_top": "#494239", "tail_top_d": "#3c362f", "tail_bot": "#81766d",
      "tail_bot_d": "#736961", "tail_rim": "#c9c1c2"}
WOLF_BOXES = {
    "head": box_uv(0, 0, 6, 6, 4), "ear": box_uv(16, 14, 2, 2, 1), "snout": box_uv(0, 10, 3, 3, 4),
    "body": box_uv(18, 14, 6, 9, 6), "mane": box_uv(21, 0, 8, 6, 7), "leg": box_uv(0, 18, 2, 8, 2),
    "tail": box_uv(9, 18, 2, 8, 2),
}


def wpx(part, name):
    """(x, y, w, h) in pixels of a wolf face."""
    return tuple(v * WK for v in WOLF_BOXES[part][name])


def wset(img, mask, color):
    img[mask, :3] = hexc(color)
    img[mask, 3] = 255


def strands(img, seed, n, colors, region=None, length=(5, 12), angle=np.pi / 2, spread=0.35,
            curve=0.06, width=1, weights=None, mask=None):
    """`n` short 1-2 px fur strands (vectorized random walks) in `region` (x0, y0, x1, y1),
    heading along `angle` (radians, screen y down) give or take `spread`, bending by `curve`.
    Only pixels of `mask` (if given) and inside the region are painted."""
    h, w = img.shape[:2]
    x0, y0, x1, y1 = region or (0, 0, w, h)
    r = rng(seed)
    px, py = r.uniform(x0, x1, n), r.uniform(y0, y1, n)
    th = angle + r.normal(0, spread, n)
    L = r.integers(length[0], length[1] + 1, n)
    cv = r.normal(0, curve, n)
    c = cols(*colors)
    p = None if weights is None else np.asarray(weights, np.float64) / np.sum(weights)
    ci = r.choice(len(colors), n, p=p)
    for t in range(int(L.max())):
        xi, yi = np.floor(px).astype(int), np.floor(py).astype(int)
        for dx in range(width):
            xx = xi + dx
            ok = (t < L) & (xx >= x0) & (xx < x1) & (yi >= y0) & (yi < y1)
            if mask is not None:
                ok &= mask[np.clip(yi, 0, h - 1), np.clip(xx, 0, w - 1)]
            img[yi[ok], xx[ok], :3] = c[ci[ok]]
        px, py, th = px + np.cos(th), py + np.sin(th), th + cv


def chevron_marks(img, seed, n, colors, region, size=(3, 6), mask=None):
    """Faithful's V-shaped fur marks: two strands meeting at a lower point."""
    r = rng(seed)
    x0, y0, x1, y1 = region
    c = cols(*colors)
    h, w = img.shape[:2]
    for _ in range(n):
        ax, ay = r.uniform(x0, x1), r.uniform(y0, y1)
        s = int(r.integers(size[0], size[1] + 1))
        col = c[r.integers(0, len(c))]
        tilt = r.normal(0, 0.25)
        for sgn in (-1, 1):
            a = -np.pi / 2 + sgn * r.uniform(0.45, 0.8) + tilt
            for i in range(s):
                x, y = int(ax + np.cos(a) * i), int(ay + np.sin(a) * i)
                if x0 <= x < x1 and y0 <= y < y1 and 0 <= x < w and 0 <= y < h and (mask is None or mask[y, x]):
                    img[y, x, :3] = col


def wolf_fur(seed, h, w, bias=0.0):
    """Soft grey fur at 8 px per unit: mottled patches of the fur tones, then many short strands
    (mostly hanging down), light V marks and a few darker hairs."""
    t = aniso(seed, h, w, 22, 14) * 0.6 + aniso(seed + 1, h, w, 8, 5) * 0.4 + bias
    t = (t - 0.18) / 0.56
    img = canvas(h, w, Ramp(WF["f1"], WF["f2"], WF["f3"], WF["f3"], WF["f4"]).shade(np.clip(t, 0, 1), 0.25))
    area = h * w
    strands(img, seed + 2, area // 110, (WF["f2"], WF["f1"]), length=(4, 9), spread=0.35, curve=0.1, weights=(3, 1))
    strands(img, seed + 3, area // 120, (WF["f4"], WF["f3"]), length=(4, 9), spread=0.35, curve=0.1, weights=(3, 1))
    chevron_marks(img, seed + 4, area // 420, (WF["f4"],), (0, 0, w, h), (3, 5))
    chevron_marks(img, seed + 5, area // 900, (WF["f1"],), (0, 0, w, h), (3, 4))
    strands(img, seed + 6, area // 2500, (WF["g1"],), length=(3, 6), spread=0.3)
    return img


def tuft8(img, seed, x, y_base, height, width, lean, colors=("g1", "g2")):
    """A grey fur flame rising from `y_base` (Faithful's tufts hanging over the body), drawn
    only over painted pixels: tapering body, darker core line, light rim on its left."""
    h, w = img.shape[:2]
    yy, xx = grid(h, w)
    i = (y_base + 1 - yy) / height  # 0 at the base .. 1 at the tip
    cx = x + lean * height * i ** 2
    half = width / 2 * np.clip(1 - i, 0, 1) ** 0.75
    m = (i >= 0) & (i <= 1) & (np.abs(xx - cx) <= half + 0.3) & (img[..., 3] > 0)
    wset(img, m, WF[colors[0]])
    core = m & (np.abs(xx - cx) <= np.maximum(half * 0.35, 0.5)) & (i < 0.7)
    wset(img, core, WF[colors[1]])
    rim = m & (xx - cx < -half + 1.2) & (i < 0.8) & (i > 0.1)
    wset(img, rim, WF["f1"])


def lock(img, seed, x, y, length, sign, lean, w0, colors, curl=2.5):
    """A wavy tapering hair lock from (x, y), `sign` -1 rising, +1 hanging; its color steps
    through `colors` (WF keys) from the root to the tip. Drawn only over painted pixels."""
    r = rng(seed)
    ph = r.uniform(0, 6.3)
    n = max(2, length // 3)
    pts = [(x + lean * j + np.sin(ph + j / 7.0) * curl * (j / length), y + sign * j)
           for j in np.linspace(0, length, n + 1)]
    painted = img[..., 3] > 0
    for k in range(n):
        f = k / n
        col = colors[min(int(f * len(colors)), len(colors) - 1)]
        before = img.copy()
        stroke(img, pts[k], pts[k + 1], max(1.2, w0 * (1 - f * 0.8)), WF[col])
        img[~painted] = before[~painted]


def wave_line(img, seed, x, y0, y1, colors, amp=2.0, period=18.0, width=2, horizontal=False):
    """A wavy dark fur line (vertical from y0 to y1 around column x, or horizontal)."""
    r = rng(seed)
    ph = r.uniform(0, 6.3)
    c = [hexc(WF[k]) for k in colors]
    h, w = img.shape[:2]
    for s in range(y0, y1):
        o = amp * np.sin(ph + s / period * 2 * np.pi) + amp * 0.4 * np.sin(ph * 2 + s / period * 5)
        for d in range(width):
            px, py = (int(round(x + o)) + d, s) if not horizontal else (s, int(round(x + o)) + d)
            if 0 <= px < w and 0 <= py < h and img[py, px, 3] > 0:
                img[py, px, :3] = c[min(d, len(c) - 1)]


def wolf(mood="wild"):
    def painter(seed: int) -> np.ndarray:
        H, W = 32 * WK, 64 * WK
        fur = wolf_fur(seed, H, W)
        shade = wolf_fur(seed + 7, H, W, bias=-0.22)
        a = blank(H, W)
        B = WOLF_BOXES
        for part in B:
            for n in B[part]:
                x, y, w, h = wpx(part, n)
                a[y : y + h, x : x + w] = fur[y : y + h, x : x + w]
        # shaded faces: under the head, the belly side, the tail's right side, the legs' backs
        for part, n in (("head", "bottom"), ("body", "front"), ("tail", "right"), ("leg", "back"),
                        ("mane", "front")):
            x, y, w, h = wpx(part, n)
            a[y : y + h, x : x + w] = shade[y : y + h, x : x + w]
        r = rng(seed + 11)
        yy, xx = grid(H, W)

        # ---- head front: the tan muzzle (spilling onto the head sides), arch and brow
        cx = 56.0
        jag = (pix(seed + 12, 2, H, 1)[:, :1] - 0.5) * 3.0
        hw = np.where(yy < 64, 24.0, 24.0 + (yy - 64) * 10.0 / 16.0) + jag
        muzzle = (yy >= 56) & (yy < 80) & (np.abs(xx - cx) <= hw)
        muzzle |= (yy >= 48) & (yy < 56) & (np.abs(xx - cx) <= 8)
        # a darker fur shadow just outside the muzzle's slanted sides
        side_sh = (yy >= 62) & (yy < 80) & (np.abs(xx - cx) <= hw + 3) & ~muzzle
        wset(a, side_sh & (pix(seed + 13, 1, H, W) > 0.35), WF["f1"])
        wset(a, muzzle, WF["tan"])
        strands(a, seed + 14, 160, (WF["tan_l"], WF["tan_d"]), (0, 48, 112, 80), (3, 7), spread=0.5,
                mask=muzzle)
        wset(a, muzzle & ~np.roll(muzzle, 1, 0) & (yy >= 56), WF["tan_l"])
        outer = ((xx - cx) / 26.5) ** 2 + ((yy - 80.5) / 21.5) ** 2 <= 1
        inner = ((xx - cx) / 16.5) ** 2 + ((yy - 80.5) / 13.5) ** 2 <= 1
        arch = outer & (yy < 80)
        wset(a, arch, WF["arch"])
        strands(a, seed + 15, 60, (WF["arch_l"], WF["arch_d"]), (24, 56, 88, 80), (3, 6), spread=0.6,
                mask=arch & ~inner)
        wset(a, arch & ~grow(inner, 0) & ~shrink(outer, 2), WF["arch_d"])
        in_fur = inner & (yy < 80)
        a[in_fur] = fur[in_fur]
        wset(a, grow(in_fur, 2) & ~in_fur & arch & (yy < 80), WF["arch_d"])
        # the brow mark between the eyes: a shallow curve
        brow = (np.abs(xx - cx) <= 7) & (np.abs(yy - (58.5 + ((xx - cx) / 7) ** 2 * 1.5)) <= 1.1)
        wset(a, brow, WF["brow"])

        # ---- eyes (rows 48..56), pupils toward the nose
        for sgn in (1, -1):
            def X(x0, x1):  # a column range on the left eye mirrored for the right eye
                return (xx >= x0) & (xx < x1) if sgn > 0 else (xx >= 112 - x1) & (xx < 112 - x0)
            rows = (yy >= 48) & (yy < 56)
            white, pupil = rows & X(32, 40), rows & X(40, 48)
            if mood == "angry":
                wset(a, white | pupil, WF["red_d"])
                wset(a, pupil | (rows & X(38, 40)), WF["red"])
                wset(a, (white | pupil) & (yy >= 54), WF["red_dd"])
                wset(a, (yy >= 49) & (yy < 51) & X(42, 44), WF["red_hi"])
                # black slanted brow: a wedge from the outer top down to the nose side
                pts = [(32, 37), (35, 39.5), (40.5, 43), (48, 45.5), (48, 48), (35, 48), (32, 45)]
                if sgn < 0:
                    pts = [(112 - x, y) for x, y in pts]
                wedge = polygon(pts, H, W)
                wset(a, wedge, WF["black"])
                wset(a, wedge & ~np.roll(wedge, 1, 0), WF["nose_hi"])
                wset(a, np.roll(wedge, 1, 0) & ~wedge & (yy < 48), WF["g2"])
            else:
                wset(a, white, WF["white"])
                wset(a, white & ((yy >= 54) | X(38, 40)), WF["white_d"])
                wset(a, pupil, WF["black"])
                wset(a, (yy >= 49) & (yy < 51) & X(41, 43), WF["glint"])
                if mood == "tame":  # a round white lid over the pupil: soft, friendly eyes
                    lid = (yy >= 44) & (yy < 48) & X(34, 47)
                    lid &= ~((yy < 46) & (X(34, 36) | X(45, 47)))
                    wset(a, lid, WF["white"])
                    wset(a, (yy >= 44) & (yy < 45) & X(36, 45), WF["white_d"])
                    wset(a, (yy >= 43) & (yy < 44) & X(37, 44), WF["f1"])
                else:  # a grey lid line arching over the eye
                    lid = (yy >= 45) & (yy < 48) & X(34, 48) & ~((yy < 46) & (X(34, 36) | X(46, 48)))
                    wset(a, lid, WF["f1"])
                    wset(a, (yy >= 47) & (yy < 48) & X(33, 47), WF["g1"])

        # ---- snout: box (0, 10) 3 x 3 x 4
        for n, base in (("top", "sn_o"), ("bottom", "sn_m"), ("right", "sn_l"), ("left", "sn_l"),
                        ("front", "sn_l"), ("back", "sn_m")):
            x, y, w, h = wpx("snout", n)
            wset(a, (xx >= x) & (xx < x + w) & (yy >= y) & (yy < y + h), WF[base])
        # top face (x 32..56, y 80..112): light tan left, warmer right, a stripe down the middle
        top = (xx >= 32) & (xx < 56) & (yy >= 80) & (yy < 112)
        wset(a, top & (xx >= 47), WF["sn_l"])
        wset(a, top & (yy < 86) & (pix(seed + 16, 2, H, W) > 0.4), WF["sn_m"])
        bot = (xx >= 56) & (xx < 80) & (yy >= 80) & (yy < 112)
        wset(a, bot & (xx >= 72) & (yy >= 96), WF["sn_l"])
        wset(a, bot & (xx < 62) & (yy < 90), WF["sn_l"])
        for face_m, sx, s in ((top, 40, seed + 17), (bot, 64, seed + 18)):
            off = np.round(np.sin(yy / 14.0 + s % 5) * 0.6 + (pix(s, 4, H, 1)[:, :1] - 0.5) * 0.9)
            stripe = face_m & (xx >= sx + off) & (xx < sx + 8 + off)
            wset(a, stripe, WF["tan"])
            wset(a, stripe & (xx < sx + 2 + off), WF["sn_d"])
            wset(a, stripe & (xx >= sx + 6 + off), WF["tan_d"])
            strands(a, s + 1, 45, (WF["sn_hi"], WF["sn_m"]), (32, 80, 80, 112), (3, 6),
                    spread=0.15, weights=(2, 1), mask=face_m & ~stripe)
        # the pale jaw: a half disc over the snout's right side, front and left side
        jaw = (((xx - 44) / 37.0) ** 2 + ((yy - 111.5) / 22.5) ** 2 <= 1) & (yy >= 112) & (yy < 136) & (xx < 88)
        wset(a, jaw, WF["jaw"])
        wset(a, jaw & (((xx - 44) / 34.0) ** 2 + ((yy - 111.5) / 20.5) ** 2 > 1) & (yy > 120), WF["jaw_d"])
        # nose: a rounded black blob across the snout top's front edge
        nose = ((xx - 44) / 8.6) ** 2 + ((yy - 114.3) / 6.4) ** 2 <= 1
        wset(a, grow(nose, 1) & ~nose & (yy > 113), WF["sn_d"])
        wset(a, nose, WF["black"])
        hi = nose & (((xx - 42) / 6.0) ** 2 + ((yy - 111) / 3.2) ** 2 <= 1) & (yy < 113)
        wset(a, hi, WF["nose_hi"])
        wset(a, (xx >= 40) & (xx < 42) & (yy >= 110) & (yy < 111), WF["glint"])
        for nx in (40, 47):  # nostrils
            wset(a, (xx >= nx) & (xx < nx + 2) & (yy >= 116) & (yy < 118), "#050607")
        # mouth line along the bottom edge (row 134..136)
        mouth = (yy >= 134) & (yy < 136) & (xx < 88)
        wset(a, mouth, WF["nose_hi"])
        wset(a, mouth & ((xx < 8) | (xx >= 80)), WF["sn_d"])
        wset(a, mouth & (((xx >= 8) & (xx < 16)) | ((xx >= 72) & (xx < 80))), WF["black"])
        wset(a, (yy >= 133) & (yy < 134) & (xx >= 16) & (xx < 72), WF["jaw_d"])
        if mood == "angry":  # a snarl: dark open mouth with pointed teeth rising from the jaw
            wset(a, (yy >= 129) & (yy < 136) & (xx >= 18) & (xx < 70), WF["nose_hi"])
            wset(a, (yy >= 129) & (yy < 130) & (xx >= 20) & (xx < 68), WF["black"])
            tx = (xx - 26) % 6
            teeth = (yy >= 131) & (yy < 135) & (xx >= 26) & (xx < 62) & (np.abs(tx - 2.5) <= (yy - 130.5) * 0.7)
            wset(a, teeth, WF["white"])
            wset(a, teeth & (tx >= 3.5), WF["white_d"])
            for fx in (20, 64):  # fangs, taller
                fang = (yy >= 129) & (xx >= fx) & (xx < fx + 4) & (np.abs(xx - fx - 1.5) <= (yy - 128.5) * 0.45)
                wset(a, fang & (yy < 136), WF["white"])
            wset(a, (yy >= 135) & (yy < 136) & (xx >= 18) & (xx < 70), WF["jaw_d"])
        # back of the snout (inside the head): a brown rounded patch
        back = (xx >= 92) & (xx < 112) & (yy >= 116) & (yy < 136)
        back &= ~(((xx < 94) | (xx >= 110)) & (yy < 118))
        wset(a, back, WF["sn_d"])
        wset(a, back & ~shrink(back, 1), WF["sn_dd"])

        # ---- ears: dark all round with a little texture, fur on the front and back
        for n in ("top", "bottom", "right", "left"):
            x, y, w, h = wpx("ear", n)
            m = (xx >= x) & (xx < x + w) & (yy >= y) & (yy < y + h)
            wset(a, m, WF["dark"])
            wset(a, m & (pix(seed + 20 + x, 2, H, W) > 0.8), WF["dark2"])
        for n in ("front", "back"):
            x, y, w, h = wpx("ear", n)
            m = (xx >= x) & (xx < x + w) & (yy >= y) & (yy < y + h)
            wset(a, m & ((xx < x + 1) | (xx >= x + w - 1)), WF["f1"])
            wset(a, m & (yy < y + 2), WF["g1"])

        # ---- mane: dark streaks fanning from the top edge of its top/bottom faces
        for n in ("top", "bottom"):
            x, y, w, h = wpx("mane", n)
            for i, fx in enumerate(r.choice(np.arange(4, w - 4, 5), 6, replace=False)):
                ln = int(r.integers(14, 30))
                lock(a, seed + 40 + i * 7 + x, x + int(fx), y, ln, 1, r.uniform(-0.2, 0.2), 3.2,
                     ("g2", "g2", "g1", "f1"), curl=r.uniform(2, 4))
        # grey tufts at the bottom of the mane's sides and back
        for n, xs_ in (("right", (0.28, 0.55, 0.8)), ("left", (0.2, 0.45, 0.72)), ("back", (0.3, 0.8))):
            x, y, w, h = wpx("mane", n)
            for i, f in enumerate(xs_):  # a tuft: a tall dark lock and lighter ones curling beside it
                tx = x + w * f + r.uniform(-2, 2)
                tall = int(r.integers(20, 30))
                lean = r.uniform(-0.25, 0.25)
                for j, (dx_, k_, w_, cols_) in enumerate(((-4.0, 0.6, 5.0, ("g1", "g1", "f1")),
                                                          (4.0, 0.7, 5.0, ("g1", "g1", "f1")),
                                                          (0.0, 1.0, 7.0, ("g2", "g2", "g1", "g1")))):
                    lock(a, seed + 50 + i * 5 + j + x, tx + dx_, y + h, int(tall * k_), -1,
                         lean + dx_ * 0.05, w_, cols_, curl=r.uniform(1.5, 3.5))
            strands(a, seed + 60 + x, w // 3, (WF["f4"], WF["f3"]), (x, y, x + w, y + h - 16), (4, 9))

        # ---- body: tail root on the rear, a dark wavy line on the back
        x, y, w, h = wpx("body", "bottom")
        root = ellipse(y + 28, x + 22, 9.5, 8.5, H, W)
        ring_ = grow(root, 3) & ~root
        wset(a, ring_ & (pix(seed + 70, 1, H, W) > 0.4), WF["f1"])
        wset(a, root, "#a39d99")
        wset(a, root & ~np.roll(root, 2, 0), "#b3adaa")
        wset(a, root & ~np.roll(root, -2, 1) & ~np.roll(root, -2, 0), "#918a86")
        for i, (tx, lean) in enumerate(((x + 12, 0.3), (x + 31, -0.3), (x + 22, 0.0))):
            tuft8(a, seed + 71 + i, tx, y + 44 if i < 2 else y + 20, 14 if i < 2 else 8, 4, lean, ("f1", "g1"))
        x, y, w, h = wpx("body", "back")
        wave_line(a, seed + 72, x + 30, y + 3, y + h - 2, ("g2", "g1", "f1"), 2.5, 60, 3)
        for i, n in enumerate(("right", "left", "front")):
            x, y, w, h = wpx("body", n)
            wave_line(a, seed + 73 + i, x + int(r.integers(10, w - 10)), y + int(r.integers(4, 20)),
                      y + h - int(r.integers(4, 20)), ("f1", "g1"), 1.5, 50, 1)

        # ---- legs: dark top, brown paw pad, a grey fur fold above the paw
        x, y, w, h = wpx("leg", "top")
        m = (xx >= x) & (xx < x + w) & (yy >= y) & (yy < y + h)
        wset(a, m, WF["leg_top"])
        x, y, w, h = wpx("leg", "bottom")
        m = (xx >= x) & (xx < x + w) & (yy >= y) & (yy < y + h)
        wset(a, m, WF["paw"])
        wset(a, m & ~shrink(m, 1), WF["paw_d"])
        wset(a, m & (((xx - x - 8) / 4.0) ** 2 + ((yy - y - 9) / 3.5) ** 2 <= 1), WF["paw_l"])
        for i in range(3):
            wset(a, m & (np.abs(xx - (x + 3 + i * 5)) < 1.5) & (np.abs(yy - (y + 3.5)) < 1.5), WF["paw_l"])
        x0, _, _, _ = wpx("leg", "right")
        _, y, _, h = wpx("leg", "right")
        # a soft fold of darker fur: a wavy band, darkest along its lower edge, lit above
        wav = (np.sin(xx / 7.0 + seed % 5) * 1.6 + np.sin(xx / 3.1) * 0.6
               + (pix(seed + 84, 2, 1, W)[0][None, :] - 0.5) * 1.5)
        legs = (xx < 64) & (yy >= y) & (yy < y + h)
        fold = legs & (yy >= y + 39 + wav) & (yy < y + 46 + wav)
        wset(a, fold & (pix(seed + 85, 1, H, W) > 0.25), WF["g1"])
        wset(a, legs & (yy >= y + 44 + wav) & (yy < y + 46 + wav), WF["g2"])
        wset(a, legs & (yy >= y + 37 + wav) & (yy < y + 39 + wav) & (pix(seed + 86, 1, H, W) > 0.4), WF["f4"])
        strands(a, seed + 87, 40, (WF["g2"], WF["f1"]), (0, y + 34, 64, y + 50), (3, 6), angle=0.0,
                spread=0.3)
        wset(a, (xx < 64) & (yy >= y + 47) & (yy < y + h) & (pix(seed + 83, 1, H, W) > 0.8), WF["f1"])

        # ---- tail: dark top (root), grey tip, wavy lines along it
        x, y, w, h = wpx("tail", "top")
        m = (xx >= x) & (xx < x + w) & (yy >= y) & (yy < y + h)
        wset(a, m, WF["tail_top"])
        wset(a, m & (pix(seed + 90, 1, H, W) > 0.7), WF["tail_top_d"])
        x, y, w, h = wpx("tail", "bottom")
        m = (xx >= x) & (xx < x + w) & (yy >= y) & (yy < y + h)
        wset(a, m, WF["tail_bot"])
        wset(a, m & (pix(seed + 91, 1, H, W) > 0.75), WF["tail_bot_d"])
        rim = m & (((xx - x - 8) / 8.5) ** 2 + ((yy - y - 8) / 8.5) ** 2 > 1)
        wset(a, rim, WF["tail_rim"])
        for i, (n, fx) in enumerate((("right", 4), ("front", 9), ("left", 7), ("back", 11))):
            x, y, w, h = wpx("tail", n)
            if i in (0, 1, 3):
                wave_line(a, seed + 92 + i, x + fx, y + 4 + i * 3, y + h - 4, ("g2", "g1"), 1.2, 44, 2)
        return a

    return painter


def wolf_collar(seed: int) -> np.ndarray:
    """Only the collar, in neutral greys (the game tints it): a band round the top of the
    mane's four sides and a frame round the mane's front (its `top` face), like Faithful's."""
    H, W = 32 * WK, 64 * WK
    a = blank(H, W)
    tone = np.array([119, 142, 170, 186, 202, 215, 226], np.float32)
    x, y, w, h = wpx("mane", "top")
    yy, xx = grid(h, w)
    frame = (xx < 4) | (xx >= w - 4) | (yy < 4) | (yy >= h - 4)
    t = np.full((h, w), 5.0)
    t -= ((xx >= w - 4) | (yy >= h - 4)) * 1.5  # shaded bottom/right
    t += ((xx < 1) | (yy < 1)) * 1.0  # a lit outer edge
    t -= ((xx >= 3) & (xx < 4) & (yy >= 3) & (yy < h - 3)) | ((yy >= 3) & (yy < 4) & (xx >= 3) & (xx < w - 3))
    t += (pix(seed, 1, h, w) - 0.5) * 1.2
    idx = np.clip(np.round(t), 0, 6).astype(int)
    a[y : y + h, x : x + w, :3] = tone[idx][..., None]
    a[y : y + h, x : x + w, 3] = np.where(frame, 255, 0)
    # the band: 8 px along the top of the right, front, left and back faces
    x0 = wpx("mane", "right")[0]
    fx, _, fw, _ = wpx("mane", "front")
    bx, _, bw, _ = wpx("mane", "back")
    y0 = wpx("mane", "right")[1]
    xs = np.arange(x0, bx + bw) + 0.5
    near = np.clip(1 - np.abs(xs - (fx + fw / 2)) / (fw / 2 + 30), 0, 1)
    rows = np.arange(8)[:, None]
    t = 5.4 - near[None, :] * 2.8 - np.clip(rows - 4, 0, None) * 0.55 + (rows == 0) * 0.8
    t = t - (rows == 7) * 1.0
    t += (pix(seed + 2, 1, 8, len(xs)) - 0.5) * 1.1
    # stitching dots along the middle
    stitch = (rows == 3) & ((np.arange(len(xs)) % 6) < 2)[None, :]
    t -= stitch * 1.2
    idx = np.clip(np.round(t), 0, 6).astype(int)
    a[y0 : y0 + 8, x0 : bx + bw, :3] = tone[idx][..., None]
    a[y0 : y0 + 8, x0 : bx + bw, 3] = 255
    return a


# ---------------------------------------------------------------------------- particles

FIRE ={"red": "#ff0000", "orange": "#ff6a00", "yellow": "#ffd800", "core": "#fff5c6"}


def flame(seed: int) -> np.ndarray:
    """Faithful's flame at twice its detail: a tall drop filling the height, a red tip that
    runs down its left rim, an orange rim, a yellow body and a pale core low down on the left
    (shapes in 32 px "ref" coordinates)."""
    h = w = 128
    ry, rx = refgrid(h, w, 4)
    wob = (noise(seed, 8, h, w, tile=False) - 0.5) * 0.5
    L = np.interp(ry, [0, 21, 26, 29, 32], [15.2, 8, 8, 9, 11.4])
    R = np.interp(ry, [0, 2, 22, 27, 29, 32], [16, 16.4, 24, 24, 23.2, 20.4])
    body = (rx >= L + wob) & (rx < R + wob) & (ry < 31.9)
    img = blank(h, w)
    set_(img, body, FIRE["yellow"])
    d = ndimage.distance_transform_edt(body)
    right = rx > (L + R) / 2
    rim = body & (d <= np.where(right, 4.5, 3.5))
    rim &= ~((ry > 19.5) & (ry < 25.5) & ~right)  # the lit left flank has no rim
    set_(img, rim, FIRE["orange"])
    set_(img, body & (ry < 3.5), FIRE["red"])
    set_(img, rim & (ry < 9.5) & ~right, FIRE["red"])
    set_(img, rim & (ry > 6.5) & (ry < 9.2) & right & (d <= 2), FIRE["red"])
    core = polygon([(4 * x, 4 * y) for x, y in ((11, 20), (12.8, 19.8), (14.2, 21.5), (16, 23), (19, 25),
                                                (22, 26.8), (21.3, 28.4), (19.5, 29.6), (12, 29.6),
                                                (10.2, 28.2), (10, 22.4))], h, w)
    core &= body & (d > 3)
    set_(img, core, FIRE["core"])
    # a few flicker pixels at the core's rim
    fl = grow(core, 2) & ~core & body & (d > 4) & (pix(seed + 1, 2, h, w) > 0.72)
    set_(img, fl, FIRE["core"])
    return img


# Per smoke stage: how far the puff reaches (in 2 px cells) and how many cells it covers
# (about Faithful's sizes, at twice its detail).
SMOKE = [(1.5, 16), (7.5, 76), (12.5, 108), (14, 228), (17, 480), (20.5, 640), (27, 1232), (31, 1800)]


def smoke(i: int):
    """Faithful's smoke puffs: chunky ragged white blobs, dense in the middle with a few
    holes, drawn in 2 px cells with 4 px lumps along the edge."""
    def painter(seed: int) -> np.ndarray:
        h = w = 128
        img = blank(h, w)
        if i == 0:
            yy, xx = grid(h, w)
            set_(img, (np.abs(yy - 67) < 4) & (np.abs(xx - 66) < 4), "#ffffff")
            return img
        reach, cells = SMOKE[i]
        g = 64
        yy, xx = grid(g, g)
        cy, cx = 33.5, 33.0
        r = rng(seed + 9)
        ang = np.arctan2(yy - cy, xx - cx)
        lobes = 1 + 0.12 * np.sin(2 * ang + r.uniform(0, 6.3)) + 0.1 * np.sin(3 * ang + r.uniform(0, 6.3))
        rad = np.hypot(yy - cy, xx - cx) / (reach * lobes)
        t = (1 - rad + (noise(seed, max(2.0, reach / 2.5), g, g, tile=False) - 0.5) * 0.35
             + (pix(seed + 1, 2, g, g) - 0.5) * 0.6 + (pix(seed + 3, 1, g, g) - 0.5) * 0.08)
        t[(rad > 1.05) | (yy < 1) | (xx < 1) | (yy > g - 1) | (xx > g - 1)] = -9
        thr = np.sort(t.ravel())[::-1][cells]
        m = t > thr
        # tidy: no lone cells; pinholes closed where the puff is dense
        nb = sum(np.roll(np.roll(m, dy, 0), dx, 1) for dy, dx in ((1, 0), (-1, 0), (0, 1), (0, -1)))
        m = (m & (nb >= 1)) | (~m & (nb >= 4) & (rad < 0.5))
        set_(img, np.repeat(np.repeat(m, 2, 0), 2, 1), "#ffffff")
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
    "entity/wolf/wolf": wolf("wild"),
    "entity/wolf/wolf_tame": wolf("tame"),
    "entity/wolf/wolf_angry": wolf("angry"),
    "entity/wolf/wolf_collar": wolf_collar,
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
        **{f"entity/wolf/{n}": list(WOLF_BOXES.values())
           for n in ("wolf", "wolf_tame", "wolf_angry", "wolf_collar")},
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
