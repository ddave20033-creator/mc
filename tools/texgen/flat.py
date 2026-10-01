"""The game's own look: flat, clean shapes (see BRIEF.md).

Every texture is a few flat colour areas with clean, anti-aliased edges: no noise, no
dithering, no per-pixel grain. Shapes are drawn on a `Canvas` in texture pixels (0..128,
fractions allowed) at 4x the resolution and averaged down, so edges are smooth. Light comes
from the top left: raised shapes get a light rim on that side and a shadow rim on the other
(`Canvas.raised`). Natural rock is made of flat-shaded facets (`Canvas.facets`), the game's
signature motif. Colours come in hue-shifted tone sets (`tones`): shadows cooler, light
warmer.

A shape is a function `(y, x) -> bool array` of the canvas's sample coordinates; the
`disk`, `ellipse`, `box`, `capsule`, `poly`... makers below build them. On a tiling canvas
the round shapes wrap around the edges (block textures tile seamlessly); use `tiled()` to
make any shape wrap.
"""

from __future__ import annotations

import colorsys

import numpy as np
from PIL import Image, ImageDraw

S = 128
SS = 4  # samples per pixel along each axis


# ---------------------------------------------------------------------------- colour


def hexc(h: str) -> np.ndarray:
    h = h.lstrip("#")
    return np.array([int(h[i : i + 2], 16) for i in (0, 2, 4)], np.float32)


def shift(rgb, dv: float = 0.0, ds: float = 0.0, dh: float = 0.0) -> np.ndarray:
    """`rgb` (0..255) with its value, saturation and hue (in turns) moved."""
    r, g, b = (np.asarray(rgb, np.float32) / 255.0).tolist()
    h, s, v = colorsys.rgb_to_hsv(r, g, b)
    h = (h + dh) % 1.0
    s = min(max(s + ds, 0.0), 1.0)
    v = min(max(v + dv, 0.0), 1.0)
    return np.array(colorsys.hsv_to_rgb(h, s, v), np.float32) * 255.0


def tones(base: str | np.ndarray, n: int = 4, spread: float = 0.16) -> list[np.ndarray]:
    """`n` flat tones of a material from dark to light around `base` (the middle-light one):
    darker tones turn a little cooler and richer, lighter ones warmer and paler."""
    c = hexc(base) if isinstance(base, str) else np.asarray(base, np.float32)
    mid = (n - 1) * 0.6
    out = []
    for i in range(n):
        k = (i - mid) / max(n - 1, 1)  # -0.6 .. 0.4
        out.append(shift(c, dv=k * spread * 2.2, ds=-k * 0.18, dh=k * 0.03 if k > 0 else k * 0.05))
    return out


def lerp(a, b, t: float) -> np.ndarray:
    a, b = np.asarray(a, np.float32), np.asarray(b, np.float32)
    return a + (b - a) * t


# ---------------------------------------------------------------------------- canvas


class Canvas:
    """A texture being drawn: `rgb` and `alpha` at SS x the texture's size. `tile`: shapes
    made with the canvas's makers wrap round the edges."""

    def __init__(self, w: int = S, h: int = S, tile: bool = True, bg=None):
        self.w, self.h, self.tile = w, h, tile
        self.rgb = np.zeros((h * SS, w * SS, 3), np.float32)
        self.alpha = np.zeros((h * SS, w * SS), np.float32)
        ys = (np.arange(h * SS, dtype=np.float32) + 0.5) / SS
        xs = (np.arange(w * SS, dtype=np.float32) + 0.5) / SS
        self.y, self.x = np.meshgrid(ys, xs, indexing="ij")
        if bg is not None:
            self.fill(np.ones_like(self.alpha, bool), bg)

    # -- painting

    def fill(self, mask, color) -> None:
        """Paints `mask` (a shape or a bool array) in `color` (an RGB or (H, W, 3) array)."""
        m = self.mask(mask)
        c = np.asarray(color, np.float32)
        self.rgb[m] = c[m] if c.ndim == 3 else c
        self.alpha[m] = 1.0

    def erase(self, mask) -> None:
        m = self.mask(mask)
        self.alpha[m] = 0.0

    def mask(self, shape) -> np.ndarray:
        return shape(self.y, self.x) if callable(shape) else np.asarray(shape, bool)

    def raised(self, shape, base, light=None, dark=None, rim: float = 2.0, tones_=None) -> None:
        """A raised flat shape lit from the top left: `base`, a `light` rim `rim` px wide on
        its top/left edge and a `dark` one on its bottom/right (from `tones_` = [dark, base,
        light] if given)."""
        if tones_ is not None:
            dark, base, light = tones_[0], tones_[1], tones_[2]
        m = self.mask(shape)
        self.fill(m, base)
        if light is not None:
            moved = moved_shape(shape, rim, rim)
            self.fill(m & ~self.mask(moved), light)
        if dark is not None:
            moved = moved_shape(shape, -rim, -rim)
            self.fill(m & ~self.mask(moved), dark)

    def facets(self, seed: int, count: int, tones_: list, crease=None, crease_w: float = 1.0,
               jitter: float = 0.9, mask=None, tilt: float = 1.0) -> np.ndarray:
        """Flat-shaded facets (tiling Voronoi cells), each one tone by how it faces the light
        (top left), optional `crease` lines between them. Returns the cell id per sample."""
        ids, border = voronoi(seed, count, self.h, self.w, jitter)
        r = np.random.default_rng(seed + 7)
        n = len(tones_)
        # a random facing per cell: lit (light from the top left) or turned away
        facing = r.uniform(-1, 1, (count, 2)) * tilt
        lit = (facing[:, 0] * -0.7 + facing[:, 1] * -0.7) * 0.5 + 0.5
        level = np.clip(np.round(lit * (n - 1)), 0, n - 1).astype(int)
        cols = np.stack([tones_[k] for k in level])
        m = np.ones_like(self.alpha, bool) if mask is None else self.mask(mask)
        self.fill(m, cols[ids])
        if crease is not None:
            self.fill(m & (border < crease_w * 0.5), crease)
        return ids

    # -- result

    def finish(self, cutout: bool = False, opaque: bool = False) -> np.ndarray:
        """The texture (float RGBA 0..255, H x W x 4): the samples averaged. `opaque` (blocks):
        full alpha; `cutout` (leaves, plants, items, sprites): alpha all or nothing."""
        a = self.alpha.reshape(self.h, SS, self.w, SS).mean((1, 3))
        rgb = (self.rgb * self.alpha[..., None]).reshape(self.h, SS, self.w, SS, 3).sum((1, 3))
        cov = self.alpha.reshape(self.h, SS, self.w, SS).sum((1, 3))
        rgb = rgb / np.maximum(cov, 1e-6)[..., None]
        out = np.zeros((self.h, self.w, 4), np.float32)
        out[..., :3] = rgb
        if opaque:
            out[..., 3] = 255.0
        elif cutout:
            out[..., 3] = np.where(a >= 0.5, 255.0, 0.0)
            out[a < 0.5, :3] = 0.0
        else:
            out[..., 3] = a * 255.0
        return out


# ---------------------------------------------------------------------------- shapes


def _d(a, c, size, tile):
    """Signed difference a - c, wrapped round `size` on a tiling canvas."""
    d = a - c
    if tile:
        d = (d + size * 0.5) % size - size * 0.5
    return d


def disk(cy, cx, r, tile: bool = True, size: int = S):
    return lambda y, x: _d(y, cy, size, tile) ** 2 + _d(x, cx, size, tile) ** 2 <= r * r


def ellipse(cy, cx, ry, rx, angle: float = 0.0, tile: bool = True, size: int = S):
    ca, sa = np.cos(angle), np.sin(angle)

    def f(y, x):
        dy, dx = _d(y, cy, size, tile), _d(x, cx, size, tile)
        u, v = dx * ca + dy * sa, -dx * sa + dy * ca
        return (u / rx) ** 2 + (v / ry) ** 2 <= 1.0

    return f


def box(y0, x0, y1, x1, r: float = 0.0, tile: bool = False, size: int = S):
    """The rectangle y0..y1, x0..x1 with corners rounded by `r`."""
    cy, cx = (y0 + y1) * 0.5, (x0 + x1) * 0.5
    hy, hx = (y1 - y0) * 0.5, (x1 - x0) * 0.5

    def f(y, x):
        qy = np.abs(_d(y, cy, size, tile)) - hy + r
        qx = np.abs(_d(x, cx, size, tile)) - hx + r
        out = np.hypot(np.maximum(qy, 0), np.maximum(qx, 0)) + np.minimum(np.maximum(qx, qy), 0) - r
        return out <= 0

    return f


def capsule(p0, p1, width: float, tile: bool = False, size: int = S):
    """A stroke (x, y) -> (x, y) `width` wide with round ends."""
    (x0, y0), (x1, y1) = p0, p1

    def f(y, x):
        dx, dy = x1 - x0, y1 - y0
        px, py = _d(x, x0, size, tile), _d(y, y0, size, tile)
        t = np.clip((px * dx + py * dy) / max(dx * dx + dy * dy, 1e-6), 0, 1)
        return (px - t * dx) ** 2 + (py - t * dy) ** 2 <= (width * 0.5) ** 2

    return f


def poly(points):
    """A filled polygon of (x, y) points (not wrapping; use `tiled` for that)."""

    def f(y, x):
        h, w = y.shape
        im = Image.new("L", (w, h), 0)
        ImageDraw.Draw(im).polygon([(px * SS, py * SS) for px, py in points], fill=255)
        return np.array(im) > 127

    return f


def tiled(shape, size: int = S):
    """`shape` repeated round the edges (any shape, so it tiles)."""

    def f(y, x):
        out = np.zeros(y.shape, bool)
        for oy in (-size, 0, size):
            for ox in (-size, 0, size):
                out |= shape(y + oy, x + ox)
        return out

    return f


def moved_shape(shape, dy: float, dx: float):
    return lambda y, x: shape(y - dy, x - dx)


def union(*shapes):
    return lambda y, x: np.logical_or.reduce([s(y, x) for s in shapes])


def minus(a, b):
    return lambda y, x: a(y, x) & ~b(y, x)


def ring(cy, cx, r0, r1, tile: bool = True, size: int = S):
    return minus(disk(cy, cx, r1, tile, size), disk(cy, cx, r0, tile, size))


# ---------------------------------------------------------------------------- tiling cells


def voronoi(seed: int, count: int, h: int = S, w: int = S, jitter: float = 0.9):
    """Tiling Voronoi cells at sample resolution: (cell id, distance to the border)."""
    r = np.random.default_rng(seed)
    side = int(np.ceil(np.sqrt(count)))
    pts = []
    for i in range(side):
        for j in range(side):
            if len(pts) < count:
                pts.append(((i + 0.5 + (r.random() - 0.5) * jitter) * h / side,
                            (j + 0.5 + (r.random() - 0.5) * jitter) * w / side))
    ys = (np.arange(h * SS, dtype=np.float32) + 0.5) / SS
    xs = (np.arange(w * SS, dtype=np.float32) + 0.5) / SS
    yy, xx = np.meshgrid(ys, xs, indexing="ij")
    best = np.full(yy.shape, 1e9, np.float32)
    second = np.full(yy.shape, 1e9, np.float32)
    ids = np.zeros(yy.shape, np.int32)
    for k, (py, px) in enumerate(pts):
        dy = (yy - py + h * 0.5) % h - h * 0.5
        dx = (xx - px + w * 0.5) % w - w * 0.5
        d = np.hypot(dy, dx)
        closer = d < best
        second = np.where(closer, best, np.minimum(second, d))
        ids = np.where(closer, k, ids)
        best = np.where(closer, d, best)
    return ids, (second - best) * 0.5


def scatter(seed: int, n: int, min_dist: float, size: int = S, margin: float = 0.0):
    """Up to `n` points (x, y) at least `min_dist` apart (measured round the tile's edges),
    `margin` from the edges when given (sprites)."""
    r = np.random.default_rng(seed)
    pts: list[tuple[float, float]] = []
    tries = 0
    while len(pts) < n and tries < n * 200:
        tries += 1
        p = (r.uniform(margin, size - margin), r.uniform(margin, size - margin))
        ok = True
        for q in pts:
            dx = abs(p[0] - q[0]); dy = abs(p[1] - q[1])
            if margin == 0.0:
                dx, dy = min(dx, size - dx), min(dy, size - dy)
            if dx * dx + dy * dy < min_dist * min_dist:
                ok = False
                break
        if ok:
            pts.append(p)
    return pts


# ---------------------------------------------------------------------------- sprites


def outlined(c: Canvas, color, width: float = 3.0) -> None:
    """Draws a `width` px outline of `color` round everything drawn on `c` (items, sprites):
    a ring outside the drawing, of the drawing's own darkest hue as chosen by the caller."""
    from scipy import ndimage

    m = c.alpha > 0.5
    r = int(round(width * SS))
    yy, xx = np.mgrid[-r : r + 1, -r : r + 1]
    k = yy * yy + xx * xx <= r * r
    ring_ = ndimage.binary_dilation(m, k) & ~m
    c.rgb[ring_] = np.asarray(color, np.float32)
    c.alpha[ring_] = 1.0
