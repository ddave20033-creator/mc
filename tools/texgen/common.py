"""Shared helpers for RustCraft's own texture set.

Style (keep every texture consistent with it) — see BRIEF.md, the Faithful 64x look at
double its resolution:
- 128x128 pixels, drawn natively with real 1-2 px detail (never on a coarse 4 px grid, never
  upscaled): crisp pixel-art with flat colors from a palette per material (`Ramp`, usually
  6-10 shades), hard edges, no blur. Continuous values (noise, lighting) go through
  `Ramp.shade`, which snaps them to the palette with ordered dithering (use little dither).
- Light comes from the top left. Raised things: lighter top/left edge, darker bottom/right.
- Items: a transparent 128x128 sprite, the object filling roughly 8..120, with a thin dark
  outline (2 px, `outline`) and inner bevel shading. Tools point from bottom left to top
  right.
- Everything is deterministic: use the given `seed` / `rng(seed)`, never global randomness.

Arrays are float32 RGBA in 0..255 of shape (H, W, 4), row 0 at the top.
"""

from __future__ import annotations

import numpy as np
from PIL import Image
from scipy import ndimage

S = 128  # texture size

# ---------------------------------------------------------------------------- color


def hexc(h: str) -> np.ndarray:
    """'#rrggbb' (or 'rrggbb') to an RGB float array."""
    h = h.lstrip("#")
    return np.array([int(h[i : i + 2], 16) for i in (0, 2, 4)], dtype=np.float32)


def mix(a, b, t):
    a, b = np.asarray(a, np.float32), np.asarray(b, np.float32)
    return a + (b - a) * t


# 4x4 Bayer matrix (0..1) for ordered dithering.
_BAYER4 = (
    np.array([[0, 8, 2, 10], [12, 4, 14, 6], [3, 11, 1, 9], [15, 7, 13, 5]], np.float32) + 0.5
) / 16.0


def bayer(h: int = S, w: int = S, scale: int = 1) -> np.ndarray:
    """Tiled Bayer threshold map; `scale` makes each dither cell scale x scale pixels."""
    ys = (np.arange(h) // scale) % 4
    xs = (np.arange(w) // scale) % 4
    return _BAYER4[ys[:, None], xs[None, :]]


class Ramp:
    """A material's palette, dark to light."""

    def __init__(self, *colors: str):
        self.colors = np.stack([hexc(c) for c in colors])

    def __len__(self):
        return len(self.colors)

    def shade(self, t: np.ndarray, dither: float = 0.6, scale: int = 1) -> np.ndarray:
        """Maps values 0..1 to palette colors (RGB), dithered between neighbouring steps.
        `dither` 0 gives hard bands, 1 a full ordered dither; `scale` is the dither cell."""
        t = np.clip(np.asarray(t, np.float32), 0.0, 1.0)
        n = len(self.colors) - 1
        x = t * n
        lo = np.floor(x)
        frac = x - lo
        th = 0.5 + (bayer(t.shape[0], t.shape[1], scale) - 0.5) * dither
        idx = np.clip(lo + (frac > th), 0, n).astype(np.int32)
        return self.colors[idx]

    def at(self, i: int) -> np.ndarray:
        return self.colors[max(0, min(len(self.colors) - 1, i))]


# ---------------------------------------------------------------------------- noise


def rng(seed: int) -> np.random.Generator:
    return np.random.default_rng(seed)


def noise(seed: int, cell: float, h: int = S, w: int = S, tile: bool = True) -> np.ndarray:
    """Smooth value noise in 0..1 with features about `cell` pixels big. With `tile` it wraps
    around the edges (textures tile seamlessly), which needs `cell` to divide the size."""
    gh = max(1, int(round(h / cell)))
    gw = max(1, int(round(w / cell)))
    g = rng(seed).random((gh + 1, gw + 1)).astype(np.float32)
    if tile:
        g[gh, :] = g[0, :]
        g[:, gw] = g[:, 0]
    ys = np.linspace(0, gh, h, endpoint=False, dtype=np.float32)
    xs = np.linspace(0, gw, w, endpoint=False, dtype=np.float32)
    y0 = np.floor(ys).astype(int)
    x0 = np.floor(xs).astype(int)
    fy = ys - y0
    fx = xs - x0
    fy = fy * fy * (3 - 2 * fy)
    fx = fx * fx * (3 - 2 * fx)
    a = g[y0][:, x0]
    b = g[y0][:, x0 + 1]
    c = g[y0 + 1][:, x0]
    d = g[y0 + 1][:, x0 + 1]
    top = a + (b - a) * fx[None, :]
    bot = c + (d - c) * fx[None, :]
    return top + (bot - top) * fy[:, None]


def fbm(seed: int, cell: float = 32, octaves: int = 4, h: int = S, w: int = S) -> np.ndarray:
    """Fractal noise in 0..1 (tiling), starting at features of `cell` pixels."""
    total = np.zeros((h, w), np.float32)
    amp, norm = 1.0, 0.0
    for o in range(octaves):
        c = max(1.0, cell / (2**o))
        total += noise(seed * 31 + o, c, h, w) * amp
        norm += amp
        amp *= 0.5
    return total / norm


def pix(seed: int, size: int = 2, h: int = S, w: int = S) -> np.ndarray:
    """Blocky white noise 0..1: random value per `size` x `size` cluster (pixel-art grain)."""
    gh, gw = (h + size - 1) // size, (w + size - 1) // size
    g = rng(seed).random((gh, gw)).astype(np.float32)
    return np.repeat(np.repeat(g, size, 0), size, 1)[:h, :w]


def voronoi(seed: int, count: int, h: int = S, w: int = S, jitter: float = 1.0):
    """Tiling Voronoi cells: (cell id, distance to nearest point, distance to the border
    between cells). Points are spread on a grid for even cell sizes."""
    r = rng(seed)
    side = int(np.ceil(np.sqrt(count)))
    pts = []
    for i in range(side):
        for j in range(side):
            if len(pts) >= count:
                break
            pts.append(
                (
                    (i + 0.5 + (r.random() - 0.5) * jitter) * h / side,
                    (j + 0.5 + (r.random() - 0.5) * jitter) * w / side,
                )
            )
    pts = np.array(pts, np.float32)
    yy, xx = np.mgrid[0:h, 0:w].astype(np.float32)
    best = np.full((h, w), 1e9, np.float32)
    second = np.full((h, w), 1e9, np.float32)
    ids = np.zeros((h, w), np.int32)
    for k, (py, px) in enumerate(pts):
        for oy in (-h, 0, h):
            for ox in (-w, 0, w):
                d = np.hypot(yy - (py + oy), xx - (px + ox))
                closer = d < best
                second = np.where(closer, best, np.minimum(second, d))
                ids = np.where(closer, k, ids)
                best = np.where(closer, d, best)
    return ids, best, (second - best) * 0.5


# ---------------------------------------------------------------------------- canvas


def blank(h: int = S, w: int = S) -> np.ndarray:
    return np.zeros((h, w, 4), np.float32)


def solid(rgb, h: int = S, w: int = S) -> np.ndarray:
    out = blank(h, w)
    out[..., :3] = np.asarray(rgb, np.float32)
    out[..., 3] = 255
    return out


def rgba(rgb: np.ndarray, alpha: float | np.ndarray = 255) -> np.ndarray:
    """RGB (H, W, 3) to RGBA with `alpha` (scalar or (H, W))."""
    out = np.empty(rgb.shape[:2] + (4,), np.float32)
    out[..., :3] = rgb
    out[..., 3] = alpha
    return out


def paint(img: np.ndarray, mask: np.ndarray, rgb) -> np.ndarray:
    """Sets `mask` pixels to `rgb` (a color or an (H, W, 3) image), fully opaque."""
    m = mask.astype(bool)
    rgb = np.asarray(rgb, np.float32)
    img[m, :3] = rgb[m] if rgb.ndim == 3 else rgb
    img[m, 3] = 255
    return img


def over(dst: np.ndarray, src: np.ndarray) -> np.ndarray:
    """`src` drawn over `dst` (alpha is treated as fully on or off: pixel art)."""
    m = src[..., 3] > 127
    dst[m] = src[m]
    return dst


def grid(h: int = S, w: int = S):
    """Pixel center coordinates (y, x) arrays."""
    yy, xx = np.mgrid[0:h, 0:w].astype(np.float32)
    return yy + 0.5, xx + 0.5


def disk(cy, cx, r, h: int = S, w: int = S) -> np.ndarray:
    yy, xx = grid(h, w)
    return (yy - cy) ** 2 + (xx - cx) ** 2 <= r * r


def ellipse(cy, cx, ry, rx, h: int = S, w: int = S) -> np.ndarray:
    yy, xx = grid(h, w)
    return ((yy - cy) / ry) ** 2 + ((xx - cx) / rx) ** 2 <= 1.0


def rect(y0, x0, y1, x1, h: int = S, w: int = S) -> np.ndarray:
    """Pixels with y0 <= y < y1 and x0 <= x < x1."""
    m = np.zeros((h, w), bool)
    m[max(0, int(y0)) : max(0, int(y1)), max(0, int(x0)) : max(0, int(x1))] = True
    return m


def polygon(points, h: int = S, w: int = S) -> np.ndarray:
    """Filled polygon; points are (x, y) in pixels."""
    from PIL import ImageDraw

    im = Image.new("L", (w, h), 0)
    ImageDraw.Draw(im).polygon([(float(x), float(y)) for x, y in points], fill=255)
    return np.array(im) > 127


def thick_line(p0, p1, width: float, h: int = S, w: int = S) -> np.ndarray:
    """A line (x, y) -> (x, y) of `width` pixels with round ends."""
    yy, xx = grid(h, w)
    (x0, y0), (x1, y1) = p0, p1
    dx, dy = x1 - x0, y1 - y0
    L2 = dx * dx + dy * dy
    t = np.clip(((xx - x0) * dx + (yy - y0) * dy) / max(L2, 1e-6), 0, 1)
    px, py = x0 + t * dx, y0 + t * dy
    return (xx - px) ** 2 + (yy - py) ** 2 <= (width * 0.5) ** 2


def grow(mask: np.ndarray, r: int) -> np.ndarray:
    """Mask dilated by `r` pixels (square structuring element, like pixel art outlines)."""
    if r <= 0:
        return mask.astype(bool)
    return ndimage.binary_dilation(mask, np.ones((2 * r + 1, 2 * r + 1), bool))


def shrink(mask: np.ndarray, r: int) -> np.ndarray:
    if r <= 0:
        return mask.astype(bool)
    return ndimage.binary_erosion(mask, np.ones((2 * r + 1, 2 * r + 1), bool))


def edge_light(mask: np.ndarray, width: int = 4) -> np.ndarray:
    """Per pixel of `mask`: +1 near its top/left edge, -1 near its bottom/right edge, 0 inside
    (for bevels: lit from the top left)."""
    m = mask.astype(bool)
    out = np.zeros(m.shape, np.float32)
    for k in range(1, width + 1):
        up = np.zeros_like(m)
        up[k:] = m[:-k]
        left = np.zeros_like(m)
        left[:, k:] = m[:, :-k]
        down = np.zeros_like(m)
        down[:-k] = m[k:]
        right = np.zeros_like(m)
        right[:, :-k] = m[:, k:]
        lit = m & (~up | ~left)
        dark = m & (~down | ~right)
        w = (width + 1 - k) / width
        out = np.where(lit & (out == 0), w, out)
        out = np.where(dark & (out == 0), -w, out)
    return out


def dist_inside(mask: np.ndarray) -> np.ndarray:
    """Distance (pixels) from each pixel of `mask` to its edge."""
    return ndimage.distance_transform_edt(mask)


def outline(img: np.ndarray, rgb, width: int = 2) -> np.ndarray:
    """Adds a `width` pixel outline of `rgb` around the opaque part of a sprite."""
    m = img[..., 3] > 127
    ring = grow(m, width) & ~m
    return paint(img, ring, rgb)


def sprite(mask: np.ndarray, ramp: Ramp, base: float = 0.55, bevel: int = 6,
           grain_seed: int = 0, grain: float = 0.08, dither: float = 0.5) -> np.ndarray:
    """A shaded solid shape for items: palette color by a lit bevel plus a little grain."""
    t = np.full(mask.shape, base, np.float32)
    t += edge_light(mask, bevel) * 0.35
    if grain:
        t += (pix(grain_seed, 4, *mask.shape) - 0.5) * grain
    out = blank(*mask.shape)
    return paint(out, mask, ramp.shade(t, dither))


def tile_shift(img: np.ndarray, dy: int, dx: int) -> np.ndarray:
    return np.roll(np.roll(img, dy, 0), dx, 1)


def to_u8(img: np.ndarray) -> np.ndarray:
    return np.clip(np.round(img), 0, 255).astype(np.uint8)


def save(img: np.ndarray, path) -> None:
    Image.fromarray(to_u8(img), "RGBA").save(path, optimize=True)


def opaque(img: np.ndarray) -> np.ndarray:
    """Forces full alpha (blocks)."""
    img = img.copy()
    img[..., 3] = 255
    return img
