//! Hash-based noise. The procedural textures' own (`hash`, `vn2`, `fbm`, `voronoi`...) come
//! first; after them the older helpers of the layers made from other layers (`texel_noise`
//! and friends, used by `synth`). They are separate on purpose: their hashes use different
//! constants, rounds and output bits, so merging them would change every texture made
//! with them.

use super::*;

pub(in crate::textures) fn hash(l: u32, x: i32, y: i32, s: u32) -> f32 {
    let mut h = l.wrapping_mul(0x9E37_79B1)
        ^ (x as u32).wrapping_mul(0x85EB_CA77)
        ^ (y as u32).wrapping_mul(0xC2B2_AE3D)
        ^ s.wrapping_mul(0x27D4_EB2F);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    h ^= h >> 12;
    h = h.wrapping_mul(0x297A_2D39);
    h ^= h >> 15;
    (h & 0xFF_FFFF) as f32 / 16_777_215.0
}

/// Tileable value noise with cells of `cx` x `cy` pixels (anisotropic for grain and bark).
pub(super) fn vn2(l: u32, x: i32, y: i32, cx: i32, cy: i32, s: u32) -> f32 {
    let (nx, ny) = ((TILE as i32 / cx).max(1), (TILE as i32 / cy).max(1));
    let (gx, gy) = (x.div_euclid(cx), y.div_euclid(cy));
    let (fx, fy) = (
        x.rem_euclid(cx) as f32 / cx as f32,
        y.rem_euclid(cy) as f32 / cy as f32,
    );
    let (ux, uy) = (fx * fx * (3.0 - 2.0 * fx), fy * fy * (3.0 - 2.0 * fy));
    let g = |a: i32, b: i32| hash(l, a.rem_euclid(nx), b.rem_euclid(ny), s);
    let (a, b, c, d) = (g(gx, gy), g(gx + 1, gy), g(gx, gy + 1), g(gx + 1, gy + 1));
    let top = a + (b - a) * ux;
    let bot = c + (d - c) * ux;
    top + (bot - top) * uy
}

pub(super) fn vn(l: u32, x: i32, y: i32, cell: i32, s: u32) -> f32 {
    vn2(l, x, y, cell, cell, s)
}

/// Smooth multi-octave noise, 0..1.
pub(super) fn fbm(l: u32, x: i32, y: i32, s: u32) -> f32 {
    0.36 * vn(l, x, y, 64, s)
        + 0.26 * vn(l, x, y, 32, s + 1)
        + 0.18 * vn(l, x, y, 16, s + 2)
        + 0.12 * vn(l, x, y, 8, s + 3)
        + 0.08 * vn(l, x, y, 4, s + 4)
}

/// Fine surface grain, 0..1.
pub(super) fn grain(l: u32, x: i32, y: i32, s: u32) -> f32 {
    0.55 * vn(l, x, y, 4, s) + 0.45 * vn(l, x, y, 2, s + 1)
}

pub(super) struct Cell {
    pub(super) d1: f32,
    pub(super) d2: f32,
    pub(super) id: u32,
    /// Offset from the nearest feature point to the pixel.
    pub(super) dx: f32,
    pub(super) dy: f32,
}

/// Tileable Voronoi in pixel units.
pub(super) fn voronoi(l: u32, x: i32, y: i32, count: u32, s: u32) -> Cell {
    let t = TILE as f32;
    let mut c = Cell {
        d1: f32::MAX,
        d2: f32::MAX,
        id: 0,
        dx: 0.0,
        dy: 0.0,
    };
    let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
    for i in 0..count {
        let (fx, fy) = (hash(l, i as i32, 1, s) * t, hash(l, i as i32, 2, s) * t);
        let mut dx = px - fx;
        let mut dy = py - fy;
        if dx > t * 0.5 {
            dx -= t;
        } else if dx < -t * 0.5 {
            dx += t;
        }
        if dy > t * 0.5 {
            dy -= t;
        } else if dy < -t * 0.5 {
            dy += t;
        }
        let d = (dx * dx + dy * dy).sqrt();
        if d < c.d1 {
            c.d2 = c.d1;
            c = Cell {
                d1: d,
                d2: c.d2,
                id: i,
                dx,
                dy,
            };
        } else if d < c.d2 {
            c.d2 = d;
        }
    }
    c
}

// ------------------------------------------------------------------ layers made from layers
//
// The noise of `synth` (grime, the furnace's inside, grilled meat). Each is its own formula:
// `texel_noise` adds where `hash` xors, and mixes once, 8 bits out; `grime_hash` xors like
// `hash` but without the layer and the last round, 16 bits out. Unifying them with `hash`
// would change those textures, so they stay as they are.

/// A hash of a texel for noise, 0..1.
pub(in crate::textures) fn texel_noise(x: usize, y: usize, salt: u32) -> f32 {
    let h = (x as u32)
        .wrapping_mul(0x9E37_79B1)
        .wrapping_add((y as u32).wrapping_mul(0x85EB_CA6B))
        .wrapping_add(salt.wrapping_mul(0xC2B2_AE35));
    let h = (h ^ (h >> 15)).wrapping_mul(0x2C1B_3C6D);
    ((h >> 8) & 0xFF) as f32 / 255.0
}

/// Smooth value noise in 0..1 with `cell` texel wide lattice cells.
pub(in crate::textures) fn value_noise(x: usize, y: usize, cell: usize, salt: u32) -> f32 {
    let (fx, fy) = (x as f32 / cell as f32, y as f32 / cell as f32);
    let (x0, y0) = (fx.floor() as usize, fy.floor() as usize);
    let (tx, ty) = (fx - x0 as f32, fy - y0 as f32);
    let (sx, sy) = (tx * tx * (3.0 - 2.0 * tx), ty * ty * (3.0 - 2.0 * ty));
    let n = |a, b| texel_noise(a, b, salt);
    let top = n(x0, y0) + (n(x0 + 1, y0) - n(x0, y0)) * sx;
    let bot = n(x0, y0 + 1) + (n(x0 + 1, y0 + 1) - n(x0, y0 + 1)) * sx;
    top + (bot - top) * sy
}

/// Smooth value noise over `texel_noise` with cells of `size` texels, tiling across the
/// texture's edges (the last cell blends into the first; `value_noise` does not wrap).
pub(in crate::textures) fn tiled_value_noise(x: usize, y: usize, size: usize, salt: u32) -> f32 {
    let n = TILE / size;
    let (fx, fy) = (x as f32 / size as f32, y as f32 / size as f32);
    let (x0, y0) = (fx.floor() as usize, fy.floor() as usize);
    let (tx, ty) = (fx - x0 as f32, fy - y0 as f32);
    let (tx, ty) = (tx * tx * (3.0 - 2.0 * tx), ty * ty * (3.0 - 2.0 * ty));
    let at = |cx: usize, cy: usize| texel_noise(cx % n, cy % n, salt);
    let top = at(x0, y0) + (at(x0 + 1, y0) - at(x0, y0)) * tx;
    let bottom = at(x0, y0 + 1) + (at(x0 + 1, y0 + 1) - at(x0, y0 + 1)) * tx;
    top + (bottom - top) * ty
}

/// The grime's hash of a lattice point, 0..1 (`synth_grime`). Like `hash` without the layer
/// and the last mixing round, 16 bits out.
pub(in crate::textures) fn grime_hash(x: i32, y: i32, s: u32) -> f32 {
    let mut h = (x as u32).wrapping_mul(0x9E37_79B1) ^ (y as u32).wrapping_mul(0x85EB_CA77) ^ s.wrapping_mul(0xC2B2_AE3D);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    h ^= h >> 12;
    (h & 0xffff) as f32 / 65535.0
}

/// The grime's smooth value noise over `grime_hash` with `cell` wide cells, at any point
/// (not tiling: the model pages are not tiles).
pub(in crate::textures) fn grime_noise(x: f32, y: f32, cell: f32, s: u32) -> f32 {
    let (gx, gy) = (x / cell, y / cell);
    let (ix, iy) = (gx.floor() as i32, gy.floor() as i32);
    let (fx, fy) = (gx - ix as f32, gy - iy as f32);
    let (sx, sy) = (fx * fx * (3.0 - 2.0 * fx), fy * fy * (3.0 - 2.0 * fy));
    let a = grime_hash(ix, iy, s) + (grime_hash(ix + 1, iy, s) - grime_hash(ix, iy, s)) * sx;
    let b = grime_hash(ix, iy + 1, s) + (grime_hash(ix + 1, iy + 1, s) - grime_hash(ix, iy + 1, s)) * sx;
    a + (b - a) * sy
}
