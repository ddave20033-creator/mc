//! Procedurally generated 128x128 textures (no external assets).
//!
//! Shapes are designed on a 32-unit grid (`d(x)` converts a pixel to design units) and
//! rendered at 128 px, so edges stay crisp while shading and detail get the full resolution.
//!
//! For opaque textures the alpha channel doubles as a *mask*: 255 = fully biome tinted,
//! 153 = untinted. Fire is generated continuously in the fragment shader.
//! Values below 128 are cut out (leaves, plants, glass).

use super::*;

/// Pixels per design unit.
const K: f32 = TILE as f32 / 32.0;

// ------------------------------------------------------------------ noise helpers

pub(super) fn hash(l: u32, x: i32, y: i32, s: u32) -> f32 {
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
fn vn2(l: u32, x: i32, y: i32, cx: i32, cy: i32, s: u32) -> f32 {
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

fn vn(l: u32, x: i32, y: i32, cell: i32, s: u32) -> f32 {
    vn2(l, x, y, cell, cell, s)
}

/// Smooth multi-octave noise, 0..1.
fn fbm(l: u32, x: i32, y: i32, s: u32) -> f32 {
    0.36 * vn(l, x, y, 64, s)
        + 0.26 * vn(l, x, y, 32, s + 1)
        + 0.18 * vn(l, x, y, 16, s + 2)
        + 0.12 * vn(l, x, y, 8, s + 3)
        + 0.08 * vn(l, x, y, 4, s + 4)
}

/// Fine surface grain, 0..1.
fn grain(l: u32, x: i32, y: i32, s: u32) -> f32 {
    0.55 * vn(l, x, y, 4, s) + 0.45 * vn(l, x, y, 2, s + 1)
}

struct Cell {
    d1: f32,
    d2: f32,
    id: u32,
    /// Offset from the nearest feature point to the pixel.
    dx: f32,
    dy: f32,
}

/// Tileable Voronoi in pixel units.
fn voronoi(l: u32, x: i32, y: i32, count: u32, s: u32) -> Cell {
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

fn col(rgb: [f32; 3], v: f32, a: u8) -> [u8; 4] {
    [
        (rgb[0] * v).clamp(0.0, 255.0) as u8,
        (rgb[1] * v).clamp(0.0, 255.0) as u8,
        (rgb[2] * v).clamp(0.0, 255.0) as u8,
        a,
    ]
}

fn lerp3(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    let t = t.clamp(0.0, 1.0);
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
    ]
}

/// Pixel center in design units (0..32).
fn d(v: i32) -> f32 {
    (v as f32 + 0.5) / K
}

/// Distance from point to segment.
fn seg_dist(px: f32, py: f32, a: (f32, f32), b: (f32, f32)) -> f32 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let t = (((px - a.0) * dx + (py - a.1) * dy) / (dx * dx + dy * dy)).clamp(0.0, 1.0);
    ((px - a.0 - t * dx).powi(2) + (py - a.1 - t * dy).powi(2)).sqrt()
}

/// Soft bevel for a rectangle (in pixels): > 1 near the top/left edge, < 1 near bottom/right.
fn bevel(x: f32, y: f32, x0: f32, y0: f32, x1: f32, y1: f32, w: f32) -> f32 {
    let tl = (x - x0).min(y - y0);
    let br = (x1 - x).min(y1 - y);
    if tl < w && tl <= br {
        1.0 + 0.16 * (1.0 - tl / w)
    } else if br < w {
        1.0 - 0.2 * (1.0 - br / w)
    } else {
        1.0
    }
}

const GRAY: [f32; 3] = [255.0, 255.0, 255.0];

// ------------------------------------------------------------------ terrain

fn grass_gray(l: u32, x: i32, y: i32) -> [u8; 4] {
    // Soft clumps plus short vertical blade strokes.
    let clumps = fbm(l, x, y, 1);
    let blades = vn2(l, x, y, 2, 7, 5) * 0.6 + vn2(l, x + 1, y, 3, 5, 6) * 0.4;
    let mut v = 0.66 + 0.2 * clumps + 0.16 * (blades - 0.5);
    if hash(l, x / 2, y / 2, 3) > 0.985 {
        v *= 1.12;
    }
    col(GRAY, v.min(1.0), 255)
}

fn dirt(l: u32, x: i32, y: i32) -> [u8; 4] {
    let base = [118.0, 84.0, 58.0];
    let mut v =
        0.82 + 0.22 * (fbm(l, x, y, 10) - 0.5) * 2.0 * 0.5 + 0.06 * (grain(l, x, y, 11) - 0.5);
    let c = voronoi(l, x, y, 26, 12);
    let r = 2.5 + hash(l, c.id as i32, 0, 13) * 3.0;
    if c.d1 < r && hash(l, c.id as i32, 1, 14) < 0.45 {
        // Small pebble with a highlight toward the top-left.
        let hl = 1.0 + 0.25 * (-(c.dx + c.dy) / (c.d1 + 0.5)).max(0.0) * (c.d1 / r);
        return col(
            [146.0, 128.0, 112.0],
            (0.92 - c.d1 / r * 0.15) * hl,
            UNTINTED,
        );
    }
    if vn(l, x, y, 6, 15) > 0.74 {
        v *= 0.84;
    }
    col(base, v, UNTINTED)
}

fn stone_base(l: u32, x: i32, y: i32) -> [u8; 4] {
    let _ = l;
    let n = fbm(3, x, y, 20);
    let mut v = 0.8 + 0.24 * n + 0.05 * (grain(3, x, y, 21) - 0.5);
    // A few short hairline cracks (Voronoi edges, only in some areas).
    let c = voronoi(3, x, y, 10, 22);
    if c.d2 - c.d1 < 1.0 && vn(3, x, y, 16, 23) > 0.62 {
        v *= 0.8;
    }
    v *= 1.0 + 0.04 * ((y as f32 / TILE as f32 * std::f32::consts::TAU * 3.0 + n * 4.0).sin());
    col([125.0, 125.0, 129.0], v, UNTINTED)
}

fn log_side(l: u32, x: i32, y: i32, base: [f32; 3]) -> [u8; 4] {
    let ridges = vn2(l, x, y, 6, 42, 30);
    let fine = vn2(l, x, y, 2, 12, 31);
    let mut v = 0.66 + 0.42 * ridges + 0.1 * (fine - 0.5);
    if ridges < 0.3 {
        v *= 0.8;
    }
    col(base, v, UNTINTED)
}

fn log_top(l: u32, x: i32, y: i32, bark: [f32; 3], light: [f32; 3], dark: [f32; 3]) -> [u8; 4] {
    if x < 8 || y < 8 || x > 119 || y > 119 {
        return log_side(l, x, y, bark);
    }
    let dist = ((x as f32 - 63.5).powi(2) + (y as f32 - 63.5).powi(2)).sqrt();
    let ring = ((dist * 0.62 + fbm(l, x, y, 40) * 6.0).sin() * 0.5 + 0.5).powf(1.6);
    let v = if x < 11 || y < 11 || x > 116 || y > 116 {
        0.8
    } else {
        0.95 + 0.06 * grain(l, x, y, 41)
    };
    col(lerp3(light, dark, ring), v, UNTINTED)
}

/// Leaves: shaded clusters over a dense canopy with a few consistent cut-out gaps.
fn leaves(l: u32, x: i32, y: i32, needles: bool) -> [u8; 4] {
    // Use the same tileable gap pattern for every tree, shifted by whole mip texels.
    // The old per-species noise made some leaves nearly solid and others see-through.
    let gap_x = x + (l as i32 * 16).rem_euclid(TILE as i32);
    let gap_y = y + (l as i32 * 32).rem_euclid(TILE as i32);
    if vn(0x1EAF, gap_x, gap_y, 8, 54) < 0.28 {
        return [0, 0, 0, 0];
    }
    let c = voronoi(l, x, y, 70, 50);
    let ang = hash(l, c.id as i32, 0, 51) * std::f32::consts::PI;
    let (sa, ca) = ang.sin_cos();
    let (lx, ly) = (c.dx * ca + c.dy * sa, -c.dx * sa + c.dy * ca);
    let (a, b) = if needles { (8.0, 1.8) } else { (7.5, 3.8) };
    let e = (lx / a).powi(2) + (ly / b).powi(2);
    let cluster = fbm(l, x, y, 52);
    if e < 1.0 {
        let rib = if !needles && ly.abs() < 0.6 {
            1.12
        } else {
            1.0
        };
        let v = (0.62 + 0.28 * (1.0 - e) + 0.12 * hash(l, c.id as i32, 1, 53))
            * rib
            * (0.85 + 0.3 * cluster);
        return col(GRAY, v.min(1.0), 255);
    }
    col(GRAY, 0.4 + 0.15 * cluster, 255)
}

/// Ore blobs with faceted, shiny crystals on top of stone.
fn ore(l: u32, x: i32, y: i32, color: [f32; 3]) -> [u8; 4] {
    let c = voronoi(l, x, y, 9, 60);
    let r = 6.0 + hash(l, c.id as i32, 0, 61) * 5.0 + (vn(l, x, y, 4, 62) - 0.5) * 4.0;
    if c.d1 < r && hash(l, c.id as i32, 1, 63) < 0.85 {
        let facet =
            ((c.dy.atan2(c.dx) / std::f32::consts::TAU * 6.0 + 3.0).floor() as i32).rem_euclid(6);
        let shade = [1.2, 1.05, 0.85, 0.72, 0.88, 1.1][facet as usize];
        let edge = if r - c.d1 < 1.2 { 0.62 } else { 1.0 };
        let sparkle = if c.d1 < 1.5 { 1.25 } else { 1.0 };
        return col(color, shade * edge * sparkle, UNTINTED);
    }
    stone_base(l, x, y)
}

fn flower(l: u32, x: i32, y: i32, petal: [f32; 3], center: [f32; 3], radius: f32) -> [u8; 4] {
    let (fx, fy) = (d(x), d(y));
    let dc = ((fx - 16.0).powi(2) + (fy - 11.0).powi(2)).sqrt();
    let ang = (fy - 11.0).atan2(fx - 16.0);
    let petals = radius * (0.82 + 0.18 * (ang * 5.0).cos().abs());
    if dc < radius * 0.32 {
        return col(center, 0.9 + 0.2 * grain(l, x, y, 70), UNTINTED);
    }
    if dc < petals {
        let shade = 1.08 - dc / radius * 0.35;
        return col(petal, shade * (0.94 + 0.08 * grain(l, x, y, 71)), UNTINTED);
    }
    let stem = seg_dist(fx, fy, (16.0, 13.0), (16.0, 32.0)) < 0.7;
    let leaf = seg_dist(fx, fy, (16.0, 24.0), (10.0, 19.0))
        < 1.3 * (1.0 - ((fx - 13.0).abs() / 4.0).min(0.6))
        || seg_dist(fx, fy, (16.0, 27.0), (22.0, 22.0))
            < 1.3 * (1.0 - ((fx - 19.0).abs() / 4.0).min(0.6));
    if stem || leaf {
        return col(
            [58.0, 118.0, 38.0],
            0.85 + 0.25 * grain(l, x, y, 72),
            UNTINTED,
        );
    }
    [0, 0, 0, 0]
}

// ------------------------------------------------------------------ items

const STICK_C: [f32; 3] = [118.0, 84.0, 44.0];

/// Material colors per tool tier: wood, stone, iron, gold, diamond.
const TIER_C: [[f32; 3]; 5] = [
    [176.0, 138.0, 82.0],
    [132.0, 132.0, 134.0],
    [226.0, 226.0, 230.0],
    [250.0, 214.0, 64.0],
    [96.0, 230.0, 222.0],
];

/// Filled shape given by `inside(x, y)` in design units, with a dark outline, a highlight
/// toward the top-left and a smooth shading gradient.
fn shape(x: i32, y: i32, color: [f32; 3], inside: impl Fn(f32, f32) -> bool) -> Option<[u8; 4]> {
    let (fx, fy) = (d(x), d(y));
    if !inside(fx, fy) {
        return None;
    }
    let e = 0.8;
    let edge =
        !inside(fx - e, fy) || !inside(fx + e, fy) || !inside(fx, fy - e) || !inside(fx, fy + e);
    let light = !inside(fx - 1.6, fy - 1.6);
    let dark = !inside(fx + 1.6, fy + 1.6);
    let v = if edge {
        0.45
    } else if light {
        1.22
    } else if dark {
        0.78
    } else {
        1.0 - (fx + fy) * 0.005
    };
    Some(col(color, v, 255))
}

fn in_polygon(x: f32, y: f32, points: &[(f32, f32)]) -> bool {
    let mut inside = false;
    let mut j = points.len() - 1;
    for i in 0..points.len() {
        let (xi, yi) = points[i];
        let (xj, yj) = points[j];
        if (yi > y) != (yj > y) && x < (xj - xi) * (y - yi) / (yj - yi) + xi {
            inside = !inside;
        }
        j = i;
    }
    inside
}

fn ingot_icon(l: u32, x: i32, y: i32) -> [u8; 4] {
    let (fx, fy) = (d(x), d(y));
    let outer = [
        (4.0, 17.0),
        (21.0, 7.0),
        (27.0, 9.0),
        (29.0, 12.0),
        (29.0, 16.0),
        (12.0, 26.0),
        (5.0, 23.0),
    ];
    if !in_polygon(fx, fy, &outer) {
        return [0, 0, 0, 0];
    }
    let gold = l == tex::GOLD_INGOT;
    let (rim, top, highlight, side, end) = if gold {
        (
            [111.0, 74.0, 12.0],
            [246.0, 193.0, 44.0],
            [255.0, 232.0, 112.0],
            [204.0, 139.0, 21.0],
            [165.0, 103.0, 16.0],
        )
    } else {
        (
            [68.0, 78.0, 91.0],
            [211.0, 222.0, 233.0],
            [250.0, 252.0, 255.0],
            [146.0, 163.0, 181.0],
            [108.0, 127.0, 148.0],
        )
    };
    let top_face = [
        (4.5, 17.0),
        (21.0, 7.5),
        (27.0, 9.5),
        (28.5, 12.0),
        (11.5, 21.5),
        (5.0, 19.0),
    ];
    let long_side = [(11.5, 21.5), (28.5, 12.0), (28.5, 15.5), (12.0, 25.5)];
    let short_end = [(5.0, 19.0), (11.5, 21.5), (12.0, 25.5), (5.0, 22.5)];
    let shine = [(7.5, 17.0), (21.0, 9.0), (25.0, 10.0), (11.5, 18.5)];
    let color = if in_polygon(fx, fy, &shine) {
        highlight
    } else if in_polygon(fx, fy, &top_face) {
        top
    } else if in_polygon(fx, fy, &long_side) {
        side
    } else if in_polygon(fx, fy, &short_end) {
        end
    } else {
        rim
    };
    col(color, 0.98 + 0.04 * grain(l, x, y, 401), 255)
}

fn diamond_icon(x: i32, y: i32) -> [u8; 4] {
    let (fx, fy) = (d(x), d(y));
    let outer = [
        (9.0, 6.0),
        (23.0, 6.0),
        (29.0, 13.0),
        (29.0, 16.0),
        (16.0, 28.0),
        (3.0, 16.0),
        (3.0, 13.0),
    ];
    if !in_polygon(fx, fy, &outer) {
        return [0, 0, 0, 0];
    }
    // (outline points, color) of each facet.
    type Facet<'a> = (&'a [(f32, f32)], [f32; 3]);
    let facets: &[Facet] = &[
        (
            &[
                (9.0, 6.5),
                (23.0, 6.5),
                (25.5, 11.5),
                (16.0, 15.0),
                (6.5, 11.5),
            ],
            [172.0, 252.0, 244.0],
        ),
        (
            &[
                (3.5, 13.0),
                (9.0, 6.5),
                (6.5, 11.5),
                (16.0, 15.0),
                (5.0, 15.5),
            ],
            [87.0, 206.0, 213.0],
        ),
        (
            &[
                (23.0, 6.5),
                (28.5, 13.0),
                (27.0, 15.5),
                (16.0, 15.0),
                (25.5, 11.5),
            ],
            [54.0, 172.0, 184.0],
        ),
        (
            &[(5.0, 15.5), (16.0, 15.0), (16.0, 27.0), (3.5, 15.5)],
            [110.0, 231.0, 225.0],
        ),
        (
            &[(16.0, 15.0), (27.0, 15.5), (16.0, 27.0)],
            [47.0, 157.0, 174.0],
        ),
    ];
    let color = facets
        .iter()
        .find(|(poly, _)| in_polygon(fx, fy, poly))
        .map(|(_, color)| *color)
        .unwrap_or([32.0, 111.0, 129.0]);
    col(color, 1.0, 255)
}

/// Tool part at a point (in 16ths of the icon, 0..16): 0 = empty, 1 = handle, 2 = head.
/// Shapes are defined along the diagonal: `a` runs up the handle toward the top right,
/// `p` across it toward the top left (0 = handle center line).
fn tool_part(kind: u32, u: f32, v: f32) -> u8 {
    if !(0.0..16.0).contains(&u) || !(0.0..16.0).contains(&v) {
        return 0;
    }
    let (px, py) = (u - 8.0, v - 8.0);
    let r = std::f32::consts::FRAC_1_SQRT_2;
    let a = (px - py) * r;
    let p = -(px + py) * r + 0.35;
    let within = |v: f32, lo: f32, hi: f32| v >= lo && v <= hi;
    let stick = |bottom: f32, top: f32| p.abs() <= 0.62 && within(a, bottom, top);
    let head = match kind {
        // Pickaxe: a curved head across the top of the handle with points bent back.
        0 => {
            let center = 3.9 - 0.075 * p * p;
            let half = 1.15 - 0.55 * (p.abs() / 7.3).powi(2);
            p.abs() <= 7.3 && (a - center).abs() <= half
        }
        // Axe: blade on the top-left side whose cutting edge flares out, and a short back.
        1 => {
            let flare = (p - 3.5).max(0.0);
            (within(p, 0.5, 5.6) && within(a, 1.8 - flare * 0.9, 5.5 + flare * 0.3))
                || (within(p, -1.3, 0.5) && within(a, 3.5, 5.5))
        }
        // Shovel: spade with straight shoulders and a rounded tip.
        2 => {
            let w = if a > 6.3 {
                2.25 * (1.0 - ((a - 6.3) / 2.4).powi(2)).max(0.0).sqrt()
            } else {
                2.25 - (3.4 - a).max(0.0) * 1.6
            };
            within(a, 2.6, 8.7) && p.abs() <= w
        }
        // Sword: pointed blade, cross guard and pommel.
        _ => {
            let blade = within(a, -1.4, 9.9) && p.abs() <= 1.05f32.min((9.9 - a) * 0.8);
            let guard = within(a, -2.3, -1.3) && p.abs() <= 2.7;
            let pommel = within(a, -9.4, -7.8) && p.abs() <= 1.0;
            blade || guard || pommel
        }
    };
    if head {
        2
    } else if stick(
        if kind == 3 { -7.8 } else { -9.4 },
        if kind == 1 { 5.5 } else { 3.7 },
    ) {
        1
    } else {
        0
    }
}

/// Tool sprite at full texture resolution: the shapes of `tool_part` with a thin dark
/// outline, bevelled edges lit from the top left, wood grain on the handle and a soft
/// sheen on the head.
fn tool_icon(l: u32, x: i32, y: i32) -> [u8; 4] {
    let i = l - tex::TOOLS;
    let (tier, kind) = ((i / 4) as usize, i % 4);
    // [outline, dark, mid, light] per tier; handles use dark oak colors.
    const HANDLE: [[f32; 3]; 4] = [
        [40.0, 26.0, 12.0],
        [73.0, 54.0, 21.0],
        [104.0, 78.0, 30.0],
        [137.0, 103.0, 39.0],
    ];
    const TIERS: [[[f32; 3]; 4]; 5] = [
        [
            [45.0, 30.0, 12.0],
            [104.0, 78.0, 40.0],
            [150.0, 116.0, 66.0],
            [188.0, 152.0, 94.0],
        ],
        [
            [36.0, 36.0, 38.0],
            [96.0, 96.0, 98.0],
            [132.0, 132.0, 134.0],
            [168.0, 168.0, 170.0],
        ],
        [
            [48.0, 48.0, 54.0],
            [150.0, 150.0, 156.0],
            [198.0, 198.0, 204.0],
            [236.0, 236.0, 240.0],
        ],
        [
            [74.0, 50.0, 6.0],
            [212.0, 158.0, 22.0],
            [250.0, 208.0, 52.0],
            [255.0, 248.0, 150.0],
        ],
        [
            [14.0, 60.0, 58.0],
            [36.0, 168.0, 160.0],
            [76.0, 226.0, 212.0],
            [184.0, 255.0, 246.0],
        ],
    ];
    let k = 16.0 / TILE as f32;
    let (u, v) = ((x as f32 + 0.5) * k, (y as f32 + 0.5) * k);
    let part = |du: f32, dv: f32| tool_part(kind, u + du, v + dv);
    let palette = |part: u8| if part == 2 { &TIERS[tier] } else { &HANDLE };
    // Outline width and bevel depth, in 16ths.
    let (o, bevel) = (0.3, 0.42);
    let ring = [
        (o, 0.0),
        (-o, 0.0),
        (0.0, o),
        (0.0, -o),
        (o * 0.7, o * 0.7),
        (-o * 0.7, o * 0.7),
        (o * 0.7, -o * 0.7),
        (-o * 0.7, -o * 0.7),
    ];
    let here = part(0.0, 0.0);
    let near_max = ring.iter().map(|&(du, dv)| part(du, dv)).max().unwrap_or(0);
    if here == 0 {
        // Outline around the shape (the head's outline wins where both touch).
        return match near_max {
            0 => [0, 0, 0, 0],
            n => col(palette(n)[0], 1.0, 255),
        };
    }
    // The head is outlined against the handle too, so it reads as a separate piece.
    if here == 1 && near_max == 2 {
        return col(TIERS[tier][0], 1.0, 255);
    }
    let pal = palette(here);
    let lit = part(-bevel, -bevel) != here;
    let shaded = part(bevel, bevel) != here;
    let base = match (lit, shaded) {
        (true, false) => pal[3],
        (false, true) => pal[1],
        _ => {
            // Soft gradient across the face: brighter toward the lit side.
            let t = (0.5 - (u + v - 16.0) * 0.02).clamp(0.0, 1.0);
            [
                pal[2][0] + (pal[3][0] - pal[2][0]) * t * 0.35,
                pal[2][1] + (pal[3][1] - pal[2][1]) * t * 0.35,
                pal[2][2] + (pal[3][2] - pal[2][2]) * t * 0.35,
            ]
        }
    };
    let texture = if here == 1 {
        // Wood grain running along the handle.
        let along = vn2(l, x + y, x - y, 3, 24, 420);
        0.9 + 0.16 * along
    } else if tier == 0 {
        0.93 + 0.12 * grain(l, x, y, 421)
    } else {
        0.97 + 0.05 * grain(l, x, y, 422)
    };
    col(base, texture, 255)
}

fn item_icon(l: u32, x: i32, y: i32) -> [u8; 4] {
    let n = grain(l, x, y, 400);
    let (fx, fy) = (d(x), d(y));
    let out = match l {
        tex::STICK => shape(x, y, STICK_C, |fx, fy| {
            seg_dist(fx, fy, (8.0, 25.0), (24.0, 7.0)) < 1.8
        }),
        tex::COAL | tex::CHARCOAL => {
            let c = if l == tex::COAL {
                [48.0, 48.0, 54.0]
            } else {
                [74.0, 58.0, 46.0]
            };
            let lump = |fx: f32, fy: f32| {
                let a = (fy - 17.0).atan2(fx - 16.0);
                let r = 9.5 + (a * 3.0).sin() * 1.2 + (a * 5.0 + 1.0).cos() * 0.8;
                ((fx - 16.0).powi(2) + (fy - 17.0).powi(2)).sqrt() < r
            };
            shape(x, y, c, lump).map(|p| {
                if n > 0.8 {
                    col([120.0, 120.0, 132.0], 1.0, 255)
                } else {
                    p
                }
            })
        }
        tex::IRON_INGOT | tex::GOLD_INGOT => Some(ingot_icon(l, x, y)),
        tex::DIAMOND => Some(diamond_icon(x, y)),
        tex::CLAY_BALL => shape(x, y, [160.0, 166.0, 182.0], |fx, fy| {
            ((fx - 16.0).powi(2) + (fy - 16.0).powi(2)).sqrt() < 8.5
        }),
        tex::BRICK => shape(x, y, [168.0, 84.0, 60.0], |fx, fy| {
            fx > 5.0 && fx < 27.0 && fy > 11.0 && fy < 21.0
        }),
        tex::BUCKET | tex::WATER_BUCKET | tex::LAVA_BUCKET => {
            let body = |fx: f32, fy: f32| {
                let half = 9.0 - (fy - 10.0) * 0.12;
                fy > 10.0 && fy < 27.0 && (fx - 16.0).abs() < half
            };
            let handle = |fx: f32, fy: f32| {
                let dd = ((fx - 16.0).powi(2) + ((fy - 11.0) * 1.3).powi(2)).sqrt();
                fy < 11.0 && dd > 8.0 && dd < 9.6
            };
            let inside = |fx: f32, fy: f32| fy > 10.5 && fy < 13.0 && (fx - 16.0).abs() < 7.5;
            if inside(fx, fy) && l != tex::BUCKET {
                Some(if l == tex::WATER_BUCKET {
                    col([50.0, 100.0, 220.0], 0.9 + 0.2 * n, 255)
                } else {
                    col([255.0, 130.0, 20.0], 0.9 + 0.2 * n, 255)
                })
            } else if inside(fx, fy) {
                Some(col([60.0, 60.0, 64.0], 1.0, 255))
            } else {
                shape(x, y, TIER_C[2], body).or_else(|| shape(x, y, [110.0, 110.0, 116.0], handle))
            }
        }
        tex::LANTERN_ITEM => {
            let body = |fx: f32, fy: f32| (fx - 16.0).abs() < 7.0 && fy > 12.0 && fy < 28.0;
            let cap = |fx: f32, fy: f32| (fx - 16.0).abs() < 4.5 && fy > 8.0 && fy <= 12.0;
            let ring = (fx - 16.0).hypot(fy - 6.0);
            let window = (fx - 16.0).abs() < 4.5 && fy > 14.5 && fy < 25.5;
            if window {
                Some(col([250.0, 214.0, 110.0], 0.95 + 0.1 * n, 255))
            } else if (1.6..3.2).contains(&ring) && fy < 8.5 {
                Some(col(LANTERN_METAL, 0.8, 255))
            } else {
                shape(x, y, LANTERN_METAL, |fx, fy| body(fx, fy) || cap(fx, fy))
            }
        }
        tex::IRON_NUGGET => shape(x, y, [200.0, 200.0, 206.0], |fx, fy| {
            let a = (fy - 17.0).atan2(fx - 16.0);
            let r = 5.5 + (a * 3.0).sin() * 0.8;
            (fx - 16.0).hypot(fy - 17.0) < r
        }),
        tex::PIG_SPAWN_EGG | tex::CREEPER_SPAWN_EGG => {
            let creeper = l == tex::CREEPER_SPAWN_EGG;
            let egg = |fx: f32, fy: f32| {
                // Narrower at the top, like an egg.
                let ry = if fy < 17.0 { 11.0 } else { 9.0 };
                let rx = 7.0 + (fy - 7.0).clamp(0.0, 10.0) * 0.12;
                ((fx - 16.0) / rx).powi(2) + ((fy - 17.0) / ry).powi(2) < 1.0
            };
            let spots = [
                (13.0, 11.0, 2.0),
                (19.5, 15.5, 2.4),
                (13.0, 20.5, 2.0),
                (19.0, 23.0, 1.6),
            ];
            let spot = spots
                .iter()
                .any(|&(sx, sy, r)| (fx - sx).hypot(fy - sy) < r);
            let c = match (creeper, spot) {
                (false, true) => [196.0, 88.0, 96.0],
                (false, false) => [240.0, 164.0, 162.0],
                (true, true) => [20.0, 20.0, 20.0],
                (true, false) => [13.0, 168.0, 18.0],
            };
            shape(x, y, c, egg)
        }
        tex::GLASS_BOTTLE | tex::WATER_BOTTLE | tex::PURIFIED_WATER => {
            // Round flask with a neck and a cork.
            let flask = |fx: f32, fy: f32| {
                (fx - 16.0).hypot((fy - 20.0) * 1.05) < 8.0
                    || ((fx - 16.0).abs() < 2.8 && fy > 6.0 && fy < 14.0)
            };
            let cork = (fx - 16.0).abs() < 2.6 && fy > 3.0 && fy <= 6.5;
            let liquid = (fx - 16.0).hypot((fy - 20.0) * 1.05) < 6.6 && fy > 16.0;
            let highlight = (fx - 12.5).hypot(fy - 17.5) < 1.6;
            if cork && l != tex::GLASS_BOTTLE {
                Some(col([150.0, 110.0, 70.0], 0.9 + 0.2 * n, 255))
            } else if !flask(fx, fy) {
                None
            } else if highlight {
                Some(col([250.0, 252.0, 255.0], 1.0, 255))
            } else if liquid && l != tex::GLASS_BOTTLE {
                let c = if l == tex::WATER_BOTTLE {
                    [60.0, 96.0, 120.0]
                } else {
                    [150.0, 214.0, 250.0]
                };
                shape(x, y, c, flask).map(|p| {
                    let v = 0.94 + 0.12 * n;
                    std::array::from_fn(|i| {
                        if i < 3 {
                            (p[i] as f32 * v).min(255.0) as u8
                        } else {
                            255
                        }
                    })
                })
            } else {
                // Glass: only the outline and a faint fill are drawn.
                let edge = !flask(fx - 1.2, fy)
                    || !flask(fx + 1.2, fy)
                    || !flask(fx, fy - 1.2)
                    || !flask(fx, fy + 1.2);
                if edge {
                    Some(col([200.0, 220.0, 235.0], 1.0, 255))
                } else if l == tex::GLASS_BOTTLE {
                    ((x + y) % 9 == 0).then(|| col([214.0, 232.0, 244.0], 0.9, 255))
                } else {
                    Some(col([214.0, 232.0, 244.0], 0.95, 255))
                }
            }
        }
        tex::PORKCHOP | tex::COOKED_PORKCHOP => {
            let meat = |fx: f32, fy: f32| {
                let outline = [
                    (6.0, 13.0),
                    (12.0, 7.0),
                    (21.0, 6.0),
                    (27.0, 10.0),
                    (27.0, 18.0),
                    (21.0, 25.0),
                    (12.0, 26.0),
                    (6.0, 21.0),
                ];
                in_polygon(fx, fy, &outline)
            };
            // A rim of fat along the top right edge, and the bone end at the lower left.
            let rim = meat(fx, fy) && !meat(fx + 2.2, fy - 2.2);
            let bone = (fx - 11.5).hypot(fy - 20.0) < 2.6;
            let (flesh, fat) = if l == tex::COOKED_PORKCHOP {
                ([152.0, 86.0, 46.0], [214.0, 172.0, 112.0])
            } else {
                ([226.0, 108.0, 116.0], [246.0, 216.0, 208.0])
            };
            let c = if bone {
                [236.0, 232.0, 214.0]
            } else if rim {
                fat
            } else {
                flesh
            };
            shape(x, y, c, meat).map(|p| {
                let v = 0.93 + 0.12 * n;
                std::array::from_fn(|i| {
                    if i < 3 {
                        (p[i] as f32 * v).min(255.0) as u8
                    } else {
                        255
                    }
                })
            })
        }
        tex::FLINT => {
            let outline = [
                (15.0, 5.0),
                (22.0, 9.0),
                (26.0, 17.0),
                (22.0, 26.0),
                (12.0, 27.0),
                (7.0, 20.0),
                (9.0, 11.0),
            ];
            // Chipped facets: a lighter one at the top left, a darker one at the bottom right.
            let facet = if fx + fy < 24.0 {
                1.18
            } else if fx - fy > 2.0 {
                0.82
            } else {
                1.0
            };
            shape(x, y, [58.0, 56.0, 60.0], |fx, fy| {
                in_polygon(fx, fy, &outline)
            })
            .map(|p| {
                col(
                    [p[0] as f32, p[1] as f32, p[2] as f32],
                    facet * (0.92 + 0.16 * n),
                    255,
                )
            })
        }
        tex::GUNPOWDER => {
            // A little heap of dark grains.
            let heap = |fx: f32, fy: f32| {
                let top = 12.0 + ((fx - 16.0) / 11.0).powi(2) * 12.0;
                fy > top && fy < 25.0 && (fx - 16.0).abs() < 11.5
            };
            shape(x, y, [84.0, 84.0, 84.0], heap).map(|p| {
                // Salt-and-pepper grains, a few texels each.
                let g = hash(l, x / 3, y / 3, 441);
                let v = if g > 0.86 {
                    1.5
                } else if g < 0.18 {
                    0.55
                } else {
                    0.85 + 0.3 * n
                };
                col([p[0] as f32, p[1] as f32, p[2] as f32], v, 255)
            })
        }
        tex::FLINT_AND_STEEL => {
            // A C-shaped steel striker with a piece of flint below it on the right.
            let striker = |fx: f32, fy: f32| {
                let r = (fx - 13.0).hypot(fy - 13.0);
                let open = fx > 13.0 && fy > 13.0;
                (6.0..9.5).contains(&r) && !open
            };
            let flint = [
                (18.0, 17.0),
                (26.0, 19.0),
                (27.0, 26.0),
                (20.0, 28.0),
                (16.0, 23.0),
            ];
            shape(x, y, [60.0, 58.0, 62.0], |fx, fy| {
                in_polygon(fx, fy, &flint)
            })
            .or_else(|| shape(x, y, [170.0, 172.0, 180.0], striker))
        }
        _ => Some(tool_icon(l, x, y)).filter(|p| p[3] > 0),
    };
    out.unwrap_or([0, 0, 0, 0])
}

/// A frame of fire (without a resource pack): flames rising from the bottom edge, their
/// noise scrolling up a full period over the animation so it loops.
fn fire_frame(l: u32, x: i32, y: i32) -> [u8; 4] {
    let (base, variant) = if l >= tex::FIRE_1 {
        (tex::FIRE_1, 1)
    } else {
        (tex::FIRE_0, 0)
    };
    let k = (l - base) as i32;
    // Scroll by 4 texels per frame: 32 frames = one 128 texel period of the noise.
    let n = fbm(tex::FIRE_0 + variant, x, y + k * 4, 460);
    let h = 1.0 - (y as f32 + 0.5) / TILE as f32;
    let heat = n * 1.25 - h * 1.05 + 0.1;
    if heat < 0.0 {
        return [0, 0, 0, 0];
    }
    let c = lerp3([200.0, 60.0, 10.0], [255.0, 240.0, 150.0], heat * 2.2);
    col(c, 1.0, 255)
}

/// A frame of an explosion puff (without a resource pack): a ball of smoke that grows,
/// brightens and then breaks up.
fn explosion_frame(k: u32, x: i32, y: i32) -> [u8; 4] {
    let t = k as f32 / (tex::EXPLOSION_FRAMES - 1) as f32;
    let (fx, fy) = (d(x) - 16.0, d(y) - 16.0);
    let n = fbm(tex::EXPLOSION, x, y, 470);
    let r = fx.hypot(fy) / 16.0;
    if r > 0.45 + 0.5 * t.sqrt() + (n - 0.5) * 0.3 || n < t * 0.75 {
        return [0, 0, 0, 0];
    }
    let v = 0.7 + 0.5 * n - 0.3 * t;
    col([230.0, 226.0, 220.0], v, 255)
}

/// TNT: red paper wrapped around the sticks, a white label with "TNT" on the sides, and the
/// stick ends (with the fuse on top) on the top and bottom.
fn tnt(l: u32, x: i32, y: i32) -> [u8; 4] {
    let (fx, fy) = (d(x), d(y));
    let n = grain(l, x, y, 450);
    const RED: [f32; 3] = [196.0, 44.0, 30.0];
    if l == tex::TNT_SIDE {
        if (11.0..21.0).contains(&fy) {
            const GLYPHS: [[&str; 5]; 2] = [
                ["#####", "..#..", "..#..", "..#..", "..#.."],
                ["#...#", "##..#", "#.#.#", "#..##", "#...#"],
            ];
            let (gx, gy) = (fx.floor() as i32 - 7, fy.floor() as i32 - 13);
            let ink = (0..17).contains(&gx) && (0..5).contains(&gy) && gx % 6 < 5 && {
                let glyph = GLYPHS[(gx / 6 == 1) as usize];
                glyph[gy as usize].as_bytes()[(gx % 6) as usize] == b'#'
            };
            let edge = !(11.6..20.4).contains(&fy);
            return if ink {
                col([30.0, 28.0, 28.0], 1.0, UNTINTED)
            } else {
                col(
                    [236.0, 232.0, 222.0],
                    if edge { 0.8 } else { 0.94 + 0.08 * n },
                    UNTINTED,
                )
            };
        }
        // Vertical folds in the paper, one per stick.
        let fold = (fx / 4.0).fract();
        let v = 0.78 + 0.3 * (fold * std::f32::consts::PI).sin() + 0.08 * n;
        return col(RED, v, UNTINTED);
    }
    // Top and bottom: four stick ends in the wrapping, the fuse sticking out of the middle.
    let (cx, cy) = (
        (fx / 16.0).floor() * 16.0 + 8.0,
        (fy / 16.0).floor() * 16.0 + 8.0,
    );
    let r = (fx - cx).hypot(fy - cy);
    let bottom = l == tex::TNT_BOTTOM;
    if !bottom && (fx - 16.0).hypot(fy - 16.0) < 2.2 {
        return col([60.0, 58.0, 56.0], 0.9 + 0.2 * n, UNTINTED);
    }
    let v = if r < 5.5 {
        1.0 + 0.1 * n - r * 0.02
    } else if r < 6.8 {
        0.62
    } else {
        0.78 + 0.1 * n
    };
    col(RED, v * if bottom { 0.85 } else { 1.0 }, UNTINTED)
}

const LANTERN_METAL: [f32; 3] = [64.0, 70.0, 86.0];

/// Built-in lantern texture in Minecraft's layout (16 units): body sides (0,2)-(6,9) with the
/// glowing window, cap sides (1,0)-(5,2), body top/bottom (0,9)-(6,15), cap top
/// (1,10)-(5,14), hanging ring (11,1)-(14,5) and standing handle (11,10)-(14,12).
fn lantern(x: i32, y: i32) -> [u8; 4] {
    let l = tex::LANTERN;
    let (u, v) = (x as f32 / 8.0, y as f32 / 8.0);
    let n = 0.92 + 0.12 * grain(l, x, y, 620);
    let metal = |k: f32| col(LANTERN_METAL, k * n, 255);
    let inside = |x0: f32, y0: f32, x1: f32, y1: f32| u >= x0 && u < x1 && v >= y0 && v < y1;
    if inside(0.0, 2.0, 6.0, 9.0) {
        // Window: bright in the middle, orange toward the frame.
        if inside(1.0, 3.0, 5.0, 8.0) {
            let d = ((u - 3.0).abs() / 2.0).max((v - 5.5).abs() / 2.5);
            let c = lerp3([255.0, 246.0, 150.0], [236.0, 130.0, 40.0], d * d);
            return col(c, 1.0, 255);
        }
        return metal(1.0);
    }
    if inside(1.0, 0.0, 5.0, 2.0) || inside(0.0, 9.0, 6.0, 15.0) || inside(1.0, 10.0, 5.0, 14.0) {
        return metal(0.9);
    }
    // Ring and handle: a loop outline.
    let ring = inside(11.0, 1.0, 14.0, 5.0) && !inside(12.0, 2.0, 13.0, 4.0);
    let handle = inside(11.0, 10.0, 14.0, 12.0) && !inside(12.0, 11.0, 13.0, 12.0);
    if ring || handle {
        return metal(0.75);
    }
    [0, 0, 0, 0]
}

/// Built-in chain: two strips of links, (0,0)-(3,16) and (3,0)-(6,16), like Minecraft's.
fn chain(x: i32, y: i32) -> [u8; 4] {
    let (u, v) = (x as f32 / 8.0, y as f32 / 8.0);
    let (strip, off) = if u < 3.0 {
        (u, 0.0)
    } else if u < 6.0 {
        (u - 3.0, 2.0)
    } else {
        return [0, 0, 0, 0];
    };
    // Oval links every 4 units; each is an outline around a 1x2 hole.
    let lv = (v + off).rem_euclid(4.0);
    let link = lv < 3.5 && !((1.0..2.0).contains(&strip) && (0.75..2.75).contains(&lv));
    if link {
        col(LANTERN_METAL, 0.8 + 0.2 * (lv / 3.5), 255)
    } else {
        [0, 0, 0, 0]
    }
}

/// Built-in pig skin, laid out like Minecraft's pig texture (64x64 texel atlas, 2 px per
/// texel here): head at (0,0), snout at (16,16), legs at (0,16), body at (28,8).
/// Creeper atlas (64x32 texels in the top half of the layer): mottled green, with the face
/// on the head's front (texels 8..16 x 8..16).
fn creeper_skin(x: i32, y: i32) -> [u8; 4] {
    let (tx, ty) = (x / 2, y / 2);
    if ty >= 32 {
        return [0, 0, 0, 0];
    }
    const FACE: [&str; 8] = [
        "........", "........", ".##..##.", ".##..##.", "...##...", "..####..", "..####..",
        "..#..#..",
    ];
    if (8..16).contains(&tx) && (8..16).contains(&ty) {
        let row = FACE[(ty - 8) as usize].as_bytes();
        if row[(tx - 8) as usize] == b'#' {
            return [18, 22, 18, 255];
        }
    }
    let n = hash(tex::CREEPER, tx, ty, 480);
    let g = if n > 0.8 {
        [180.0, 220.0, 170.0]
    } else if n < 0.3 {
        [60.0, 140.0, 50.0]
    } else {
        [94.0, 178.0, 80.0]
    };
    col(g, 1.0, UNTINTED)
}

fn pig_skin(x: i32, y: i32) -> [u8; 4] {
    let (u, v) = (x / 2, y / 2);
    let l = tex::PIG;
    let n = 0.95 + 0.06 * grain(l, x, y, 610) + 0.05 * (vn(l, x, y, 8, 611) - 0.5);
    let pink = [240.0, 160.0, 158.0];
    let c = |rgb: [f32; 3], k: f32| col(rgb, k * n, UNTINTED);
    let within =
        |x0: i32, y0: i32, w: i32, h: i32| (x0..x0 + w).contains(&u) && (y0..y0 + h).contains(&v);
    // Head front (8,8) 8x8: eyes on its 5th row.
    if within(8, 8, 8, 8) {
        return match (u - 8, v - 8) {
            (1, 4) | (6, 4) => col([236.0, 236.0, 236.0], 1.0, UNTINTED),
            (2, 4) | (5, 4) => col([30.0, 24.0, 30.0], 1.0, UNTINTED),
            _ => c(pink, 1.0),
        };
    }
    // Snout front (17,17) 4x3 with two nostrils, the rest of the snout a deeper pink.
    if within(17, 17, 4, 3) {
        return match (u - 17, v - 17) {
            (0, 1) | (3, 1) => c([150.0, 70.0, 80.0], 1.0),
            _ => c([236.0, 138.0, 146.0], 1.0),
        };
    }
    if within(16, 16, 8, 4) {
        return c([226.0, 134.0, 140.0], 1.0);
    }
    // Hooves: the leg bottoms and the lowest row of the leg sides.
    if within(8, 16, 4, 4) || within(0, 25, 16, 1) {
        return c([150.0, 96.0, 96.0], 1.0);
    }
    // Belly (36,16) a bit darker, back (54,16) and chest (36,8) lighter.
    if within(36, 16, 10, 16) {
        return c(pink, 0.9);
    }
    if within(54, 16, 10, 16) || within(36, 8, 10, 8) {
        return c(pink, 1.04);
    }
    // Rear (46,8) 10x8: a curly tail.
    if within(46, 8, 10, 8) {
        let (tx, ty) = (x as f32 / 2.0 - 51.0, y as f32 / 2.0 - 12.0);
        let r = tx.hypot(ty);
        return if (0.9..2.1).contains(&r) {
            c([206.0, 118.0, 124.0], 1.0)
        } else {
            c(pink, 0.98)
        };
    }
    c(pink, 1.0)
}

fn torch_icon(x: i32, y: i32) -> [u8; 4] {
    let (fx, fy) = (d(x), d(y));
    if (2.0..16.0).contains(&fy) {
        let center = 16.0 + (fy * 0.7).sin() * 0.7;
        let half = if fy < 9.0 {
            (fy - 2.0) * 0.65 + 0.4
        } else {
            4.9 - (fy - 9.0) * 0.42
        };
        let dx = (fx - center).abs();
        if dx < half {
            let c = if dx < half * 0.32 && fy > 6.0 {
                [255.0, 250.0, 174.0]
            } else if dx < half * 0.7 {
                [255.0, 188.0, 52.0]
            } else {
                [225.0, 83.0, 22.0]
            };
            return col(c, 1.0, 255);
        }
    }
    let center = 16.0 + (fy - 20.0) * 0.12;
    if (13.0..30.0).contains(&fy) && (fx - center).abs() < 2.2 {
        if fy < 17.0 {
            return col([71.0, 53.0, 36.0], if fy < 15.0 { 0.8 } else { 1.0 }, 255);
        }
        let side = if fx < center - 0.7 {
            1.2
        } else if fx > center + 0.8 {
            0.67
        } else {
            1.0
        };
        return col(
            [151.0, 98.0, 48.0],
            side * (0.94 + 0.1 * vn(tex::TORCH, x, y, 8, 510)),
            255,
        );
    }
    [0, 0, 0, 0]
}

// ------------------------------------------------------------------ crafted blocks

fn planks(l: u32, x: i32, y: i32) -> [u8; 4] {
    let _ = l;
    let l = tex::PLANKS;
    let board = y / 32;
    let yb = y % 32;
    let base = [168.0, 132.0, 82.0];
    if yb >= 30 {
        return col(base, 0.5, UNTINTED);
    }
    let joint = (board * 47 + 21).rem_euclid(128);
    let dx = (x - joint).rem_euclid(128);
    if dx < 2 {
        return col(base, if dx == 0 { 0.58 } else { 0.72 }, UNTINTED);
    }
    // Nail heads next to each joint.
    for nx in [joint - 5, joint + 6] {
        for ny in [6, 23] {
            if (x - nx).rem_euclid(128).min((nx - x).rem_euclid(128)) <= 1 && (yb - ny).abs() <= 1 {
                return col([90.0, 70.0, 50.0], 0.9, UNTINTED);
            }
        }
    }
    let g =
        vn2(l, x + board * 37, y, 40, 3, 120 + board as u32) * 0.7 + vn2(l, x, y, 12, 2, 125) * 0.3;
    let tone = 0.86 + 0.12 * hash(l, board, 0, 122);
    let edge = if yb < 2 {
        1.1
    } else if yb > 27 {
        0.9
    } else {
        1.0
    };
    col(base, tone * (0.88 + 0.22 * g) * edge, UNTINTED)
}

/// Framed, bevelled storage block (iron/gold/diamond/coal).
fn metal_block(l: u32, x: i32, y: i32, c: [f32; 3]) -> [u8; 4] {
    let (fx, fy) = (x as f32 + 0.5, y as f32 + 0.5);
    let outer = bevel(fx, fy, 0.0, 0.0, 128.0, 128.0, 8.0);
    let inner = if (12.0..116.0).contains(&fx) && (12.0..116.0).contains(&fy) {
        bevel(fx, fy, 12.0, 12.0, 116.0, 116.0, 5.0).recip()
    } else {
        1.0
    };
    let v = 0.9 + 0.1 * fbm(l, x, y, 410) + 0.04 * ((fx + fy) * 0.05).sin();
    let rivet = [(20, 20), (107, 20), (20, 107), (107, 107)]
        .iter()
        .any(|&(rx, ry)| (x - rx).pow(2) + (y - ry).pow(2) <= 9);
    col(
        c,
        v * outer * inner * if rivet { 0.78 } else { 1.0 },
        UNTINTED,
    )
}

fn furnace(l: u32, x: i32, y: i32) -> [u8; 4] {
    let pattern = if l == tex::FURNACE_FRONT_LIT {
        tex::FURNACE_FRONT
    } else {
        l
    };
    let (fx, fy) = (x as f32 + 0.5, y as f32 + 0.5);
    let stone = |v: f32| col([120.0, 120.0, 123.0], v, UNTINTED);
    let base =
        0.84 + 0.18 * fbm(tex::FURNACE_SIDE, x, y, 501) + 0.04 * (grain(pattern, x, y, 502) - 0.5);
    let frame_v = bevel(fx, fy, 0.0, 0.0, 128.0, 128.0, 8.0);
    if l == tex::FURNACE_TOP {
        return stone(base * frame_v);
    }
    // Stone-brick courses on the sides and around the opening.
    let row = y / 32;
    let off = if row % 2 == 0 { 0 } else { 32 };
    let mortar = y % 32 >= 30 || (x + off) % 64 >= 62;
    let front = l != tex::FURNACE_SIDE;
    if front && (28..100).contains(&x) && (60..112).contains(&y) {
        // Opening with a lintel on top.
        if y < 66 {
            return stone(0.62 + 0.05 * grain(pattern, x, y, 503));
        }
        return col(
            [26.0, 24.0, 24.0],
            0.8 + 0.3 * grain(pattern, x, y, 504),
            UNTINTED,
        );
    }
    if mortar {
        return stone(0.66 * frame_v);
    }
    let brick_bevel = if y % 32 < 2 {
        1.08
    } else if y % 32 > 26 {
        0.92
    } else {
        1.0
    };
    stone(base * frame_v * brick_bevel)
}

fn chest(l: u32, x: i32, y: i32) -> [u8; 4] {
    // Position-mapped: the chest spans x 8..120; lid rows 16..48 (y 10..14 px), base rows 48..128.
    let (fx, fy) = (x as f32 + 0.5, y as f32 + 0.5);
    let wood = [164.0, 114.0, 54.0];
    let rim = [92.0, 60.0, 28.0];
    let g = vn2(l, x, y, 36, 3, 503) * 0.7 + grain(l, x, y, 504) * 0.3;
    let plank = |v: f32| col(wood, v * (0.84 + 0.2 * g), UNTINTED);
    match l {
        tex::CHEST_TOP => {
            let inset = |a: f32| !(13.0..=115.0).contains(&a);
            if inset(fx) || inset(fy) {
                return col(rim, 0.9 + 0.12 * grain(l, x, y, 505), UNTINTED);
            }
            plank(
                bevel(fx, fy, 13.0, 13.0, 115.0, 115.0, 4.0)
                    * if (y - 13) % 34 >= 32 { 0.72 } else { 1.0 },
            )
        }
        tex::CHEST_INSIDE => col([70.0, 46.0, 22.0], 0.8 + 0.2 * g, UNTINTED),
        tex::CHEST_LATCH => {
            let v = bevel(fx, fy, 0.0, 0.0, 128.0, 128.0, 22.0);
            col(
                [196.0, 198.0, 206.0],
                v * (0.92 + 0.08 * grain(l, x, y, 506)),
                UNTINTED,
            )
        }
        _ => {
            let side = |a: f32| !(13.0..=115.0).contains(&a);
            if fy < 16.0 {
                // Above the chest model (only seen on the item icon).
                return col(rim, 0.9, UNTINTED);
            }
            let lid = (16.0..48.0).contains(&fy);
            let (y0, y1) = if lid { (16.0, 48.0) } else { (48.0, 128.0) };
            if side(fx) || fy - y0 < 5.0 || y1 - fy < 5.0 {
                return col(
                    rim,
                    bevel(fx, fy, 8.0, y0, 120.0, y1, 3.0) * (0.9 + 0.12 * grain(l, x, y, 507)),
                    UNTINTED,
                );
            }
            plank(1.0)
        }
    }
}

fn crafting(l: u32, x: i32, y: i32) -> [u8; 4] {
    let (fx, fy) = (x as f32 + 0.5, y as f32 + 0.5);
    let frame = [98.0, 66.0, 36.0];
    match l {
        tex::CRAFTING_TOP => {
            // 3x3 work surface matching where items are shown on top.
            let border = fx < 10.0 || fy < 10.0 || fx > 118.0 || fy > 118.0;
            let cell = |a: f32| {
                let t = (a - 10.0) / 36.0;
                (t - t.round()).abs() * 36.0 < 1.5 && a > 12.0 && a < 116.0
            };
            if border {
                return col(
                    frame,
                    bevel(fx, fy, 0.0, 0.0, 128.0, 128.0, 6.0) * (0.9 + 0.12 * grain(l, x, y, 510)),
                    UNTINTED,
                );
            }
            if cell(fx) || cell(fy) {
                return col(frame, 0.72, UNTINTED);
            }
            let p = planks(l, x, y);
            [
                ((p[0] as f32) * 1.04) as u8,
                ((p[1] as f32) * 1.04) as u8,
                ((p[2] as f32) * 1.04) as u8,
                UNTINTED,
            ]
        }
        _ => {
            if y < 22 {
                return col(
                    frame,
                    bevel(fx, fy, 0.0, 0.0, 128.0, 22.0, 5.0) * (0.9 + 0.12 * grain(l, x, y, 511)),
                    UNTINTED,
                );
            }
            let (px, py) = (d(x), d(y));
            let metal = [150.0, 152.0, 158.0];
            let handle = [120.0, 84.0, 46.0];
            if l == tex::CRAFTING_SIDE {
                // Saw: blade with teeth and a wooden grip.
                let blade = px > 6.0 && px < 13.0 && py > 9.0 && py < 26.0 - (px - 6.0) * 0.4;
                let teeth =
                    (13.0..14.0).contains(&px) && (py as i32) % 2 == 0 && py > 9.0 && py < 24.0;
                if blade || teeth {
                    return col(metal, 0.95 + 0.1 * (py - 9.0) / 17.0, UNTINTED);
                }
                if seg_dist(px, py, (9.5, 6.5), (9.5, 9.0)) < 2.0 {
                    return col(handle, 1.0, UNTINTED);
                }
            } else {
                // Hammer and a pair of tongs.
                if seg_dist(px, py, (22.0, 12.0), (22.0, 27.0)) < 1.1 {
                    return col(handle, 1.0 + (22.0 - px) * 0.08, UNTINTED);
                }
                if seg_dist(px, py, (18.0, 11.0), (26.0, 11.0)) < 2.2 {
                    return col(metal, 1.0 + (11.0 - py) * 0.06, UNTINTED);
                }
                if seg_dist(px, py, (7.0, 10.0), (10.0, 26.0)) < 0.8
                    || seg_dist(px, py, (12.0, 10.0), (9.0, 26.0)) < 0.8
                {
                    return col([80.0, 80.0, 86.0], 1.0, UNTINTED);
                }
            }
            planks(l, x, y)
        }
    }
}

fn new_block(l: u32, x: i32, y: i32) -> [u8; 4] {
    match l {
        tex::CRAFTING_TOP | tex::CRAFTING_SIDE | tex::CRAFTING_FRONT => crafting(l, x, y),
        tex::FURNACE_FRONT | tex::FURNACE_FRONT_LIT | tex::FURNACE_SIDE | tex::FURNACE_TOP => {
            furnace(l, x, y)
        }
        tex::CHEST_FRONT | tex::CHEST_SIDE | tex::CHEST_TOP => chest(l, x, y),
        tex::TORCH => torch_icon(x, y),
        tex::OAK_SAPLING | tex::BIRCH_SAPLING | tex::SPRUCE_SAPLING => {
            let (fx, fy) = (d(x), d(y));
            let stem = seg_dist(fx, fy, (16.0, 31.0), (16.0, 14.0)) < 0.9;
            let (leaf, c) = match l {
                tex::SPRUCE_SAPLING => (
                    (fx - 16.0).abs() < (fy - 4.0) * 0.45
                        && fy < 24.0
                        && ((fy * 1.2) as i32 % 4 != 0),
                    [52.0, 100.0, 60.0],
                ),
                tex::BIRCH_SAPLING => (
                    (((fx - 16.0) / 9.0).powi(2) + ((fy - 12.0) / 8.0).powi(2)) < 1.0,
                    [118.0, 168.0, 72.0],
                ),
                _ => (
                    (((fx - 16.0) / 10.0).powi(2) + ((fy - 12.0) / 8.0).powi(2)) < 1.0,
                    [72.0, 140.0, 42.0],
                ),
            };
            let lv = leaves(l, x, y, l == tex::SPRUCE_SAPLING);
            if leaf && lv[3] > 0 {
                col(c, lv[0] as f32 / 255.0 * 1.25, 255)
            } else if stem {
                let sc = if l == tex::BIRCH_SAPLING {
                    [214.0, 212.0, 204.0]
                } else {
                    STICK_C
                };
                col(sc, 0.9 + 0.1 * grain(l, x, y, 505), 255)
            } else {
                [0, 0, 0, 0]
            }
        }
        tex::IRON_BLOCK => metal_block(l, x, y, [214.0, 216.0, 222.0]),
        tex::GOLD_BLOCK => metal_block(l, x, y, [248.0, 206.0, 64.0]),
        tex::DIAMOND_BLOCK => metal_block(l, x, y, [98.0, 222.0, 214.0]),
        tex::COAL_BLOCK => metal_block(l, x, y, [44.0, 44.0, 50.0]),
        _ => {
            // Stone bricks: two courses per half, bevelled.
            let row = y / 32;
            let off = if row % 2 == 0 { 0 } else { 32 };
            let (bx, by) = ((x + off) % 64, y % 32);
            if by >= 30 || bx >= 62 {
                return col(
                    [74.0, 74.0, 77.0],
                    0.9 + 0.1 * grain(l, x, y, 507),
                    UNTINTED,
                );
            }
            let bv = bevel(bx as f32 + 0.5, by as f32 + 0.5, 0.0, 0.0, 62.0, 30.0, 4.0);
            let chip = hash(l, (x + off) / 64 + row * 7, 0, 508);
            col(
                [124.0, 124.0, 128.0],
                (0.84 + 0.18 * fbm(l, x, y, 509) + 0.06 * chip) * bv,
                UNTINTED,
            )
        }
    }
}

// ------------------------------------------------------------------ character

const HAIR_C: [f32; 3] = [66.0, 42.0, 24.0];
const SKIN_C: [f32; 3] = [206.0, 150.0, 112.0];
const SHIRT_C: [f32; 3] = [40.0, 150.0, 162.0];
const PANTS_C: [f32; 3] = [56.0, 64.0, 150.0];

/// Character skin textures, drawn on an 8x8 grid of 16-pixel cells like a classic blocky skin.
fn character(l: u32, x: i32, y: i32) -> [u8; 4] {
    let (cx, cy) = (x / 16, y / 16);
    let n = 0.95 + 0.07 * hash(l, x / 4, y / 4, 300) + 0.03 * (grain(l, x, y, 302) - 0.5);
    let c = |rgb: [f32; 3], v: f32| col(rgb, v * n, UNTINTED);
    match l {
        tex::FACE => match (cx, cy) {
            (_, 0) | (_, 1) => c(HAIR_C, 1.0),
            (0 | 7, 2) => c(HAIR_C, 0.9),
            (1, 4) | (6, 4) => col([238.0, 238.0, 240.0], 1.0, UNTINTED),
            (2, 4) | (5, 4) => col([62.0, 84.0, 170.0], 1.0, UNTINTED),
            (3 | 4, 5) => c(SKIN_C, 0.86),
            (3 | 4, 6) => c([120.0, 66.0, 52.0], 1.0),
            _ => c(SKIN_C, 1.0),
        },
        tex::HEAD_SIDE => match (cx, cy) {
            (_, 0) | (_, 1) => c(HAIR_C, 1.0),
            (4..=7, 2) | (6..=7, 3) => c(HAIR_C, 0.92),
            (3, 4) => c(SKIN_C, 0.84),
            _ => c(SKIN_C, 0.97),
        },
        tex::HAIR => c(HAIR_C, 0.9 + 0.15 * vn(l, x, y, 16, 301)),
        tex::SHIRT_FRONT => {
            if cy == 0 && (3..=4).contains(&cx) {
                c(SKIN_C, 0.95)
            } else {
                c(SHIRT_C, 1.0)
            }
        }
        tex::SHIRT => c(SHIRT_C, 0.95),
        tex::ARM => {
            if y < 44 {
                c(SHIRT_C, 0.95)
            } else {
                c(SKIN_C, 1.0 - (y - 44) as f32 * 0.001)
            }
        }
        _ => {
            if y < 104 {
                c(PANTS_C, 1.0)
            } else {
                c([86.0, 86.0, 92.0], 1.0)
            }
        }
    }
}

// ------------------------------------------------------------------ dispatcher

pub(super) fn pixel(layer: u32, x: i32, y: i32, crack: &[u16]) -> [u8; 4] {
    let l = layer;
    match layer {
        tex::GRASS_TOP => grass_gray(l, x, y),
        tex::GRASS_SIDE | tex::SNOWY_GRASS_SIDE => {
            // Grass (or snow) hanging over the dirt with an uneven, dripping edge.
            let fx = x;
            let depth = 18
                + (vn(l, fx, 0, 16, 80) * 12.0) as i32
                + if vn(l, fx, 0, 4, 81) > 0.72 { 8 } else { 0 };
            if y < depth {
                if layer == tex::GRASS_SIDE {
                    let g = grass_gray(tex::GRASS_TOP, x, y);
                    let shade = 1.0 - (y as f32 / depth as f32) * 0.12;
                    col(GRAY, g[0] as f32 / 255.0 * shade, 255)
                } else {
                    col(
                        [242.0, 247.0, 252.0],
                        0.92 + 0.08 * fbm(l, x, y, 82),
                        UNTINTED,
                    )
                }
            } else if y < depth + 3 {
                let dd = dirt(tex::DIRT, x, y);
                col([dd[0] as f32, dd[1] as f32, dd[2] as f32], 0.72, UNTINTED)
            } else {
                dirt(tex::DIRT, x, y)
            }
        }
        tex::DIRT => dirt(l, x, y),
        tex::STONE => stone_base(l, x, y),
        tex::SAND => {
            let v = 0.92
                + 0.06 * fbm(l, x, y, 90)
                + 0.06 * (grain(l, x, y, 91) - 0.5)
                + 0.025 * (y as f32 * 0.2 + fbm(l, x, y, 92) * 8.0).sin();
            let speck = hash(l, x / 2, y / 2, 93);
            let c = if speck > 0.97 {
                [180.0, 160.0, 120.0]
            } else {
                [222.0, 206.0, 158.0]
            };
            col(c, v, UNTINTED)
        }
        tex::OAK_LOG => log_side(l, x, y, [104.0, 80.0, 50.0]),
        tex::OAK_LOG_TOP => log_top(
            l,
            x,
            y,
            [104.0, 80.0, 50.0],
            [186.0, 150.0, 96.0],
            [148.0, 114.0, 68.0],
        ),
        tex::OAK_LEAVES => leaves(l, x, y, false),
        tex::SPRUCE_LEAVES => leaves(l, x, y, true),
        tex::BIRCH_LEAVES => leaves(l, x, y, false),
        tex::WATER => {
            // Directional waves repeat across every block and form visible bands at a distance.
            // Keep the albedo subtle; moving ripples and highlights come from the water shader.
            let v = 0.87
                + 0.08 * fbm(l, x, y, 100)
                + 0.02 * (vn(l, x, y, 16, 106) - 0.5)
                + 0.01 * (grain(l, x, y, 107) - 0.5);
            col(GRAY, v, 255)
        }
        tex::SNOW => {
            let sparkle = if hash(l, x, y, 111) > 0.995 {
                1.06
            } else {
                1.0
            };
            col(
                [242.0, 247.0, 253.0],
                (0.94 + 0.06 * fbm(l, x, y, 110)) * sparkle,
                UNTINTED,
            )
        }
        tex::PLANKS => planks(l, x, y),
        tex::COBBLE => {
            let c = voronoi(l, x, y, 16, 130);
            if c.d2 - c.d1 < 3.0 {
                return col(
                    [58.0, 58.0, 60.0],
                    0.85 + 0.25 * grain(l, x, y, 131),
                    UNTINTED,
                );
            }
            let r = (c.d1 / ((c.d1 + c.d2) * 0.5)).min(1.0);
            let hl = (-(c.dx + c.dy) / (c.d1 + 1.0)).clamp(-1.0, 1.0) * r * 0.18;
            let s = 0.8 + 0.3 * hash(l, c.id as i32, 0, 132);
            col(
                [126.0, 126.0, 129.0],
                s * (1.08 - 0.28 * r * r + hl) * (0.95 + 0.08 * grain(l, x, y, 133)),
                UNTINTED,
            )
        }
        tex::BEDROCK => {
            let v = 0.2 + 0.85 * vn(l, x, y, 16, 140) * (0.55 + 0.5 * vn(l, x, y, 4, 141));
            col([90.0, 90.0, 94.0], v, UNTINTED)
        }
        tex::GLASS => {
            let edge = x < 4 || y < 4 || x > 123 || y > 123;
            let inner = x < 7 || y < 7 || x > 120 || y > 120;
            let s = x + y;
            let streak = ((46..54).contains(&s) && (14..40).contains(&x))
                || ((150..156).contains(&s) && (80..108).contains(&x));
            if edge {
                [190, 216, 230, 255]
            } else if inner {
                [222, 238, 246, 255]
            } else if streak {
                [240, 248, 252, 255]
            } else {
                [0, 0, 0, 0]
            }
        }
        tex::BRICKS => {
            let row = y / 32;
            let off = if row % 2 == 0 { 0 } else { 32 };
            let (bx, by) = ((x + off) % 64, y % 32);
            if by >= 27 || bx >= 59 {
                return col(
                    [184.0, 176.0, 166.0],
                    0.86 + 0.14 * grain(l, x, y, 150),
                    UNTINTED,
                );
            }
            let id = (x + off) / 64 + row * 4;
            let bv = bevel(bx as f32 + 0.5, by as f32 + 0.5, 0.0, 0.0, 59.0, 27.0, 4.0);
            let v = (0.8 + 0.2 * hash(l, id, 0, 151) + 0.12 * (fbm(l, x, y, 152) - 0.5)) * bv;
            col([150.0, 72.0, 56.0], v, UNTINTED)
        }
        tex::GRAVEL => {
            let c = voronoi(l, x, y, 60, 160);
            if c.d2 - c.d1 < 1.6 {
                return col([60.0, 56.0, 54.0], 0.9, UNTINTED);
            }
            let palette = [
                [138.0, 128.0, 122.0],
                [110.0, 106.0, 104.0],
                [152.0, 142.0, 130.0],
                [94.0, 90.0, 88.0],
            ];
            let pc = palette[(hash(l, c.id as i32, 0, 161) * 4.0) as usize % 4];
            let r = (c.d1 / ((c.d1 + c.d2) * 0.5)).min(1.0);
            let hl = (-(c.dx + c.dy) / (c.d1 + 1.0)).clamp(-1.0, 1.0) * r * 0.2;
            col(
                pc,
                (1.08 - 0.3 * r * r + hl) * (0.94 + 0.1 * grain(l, x, y, 162)),
                UNTINTED,
            )
        }
        tex::SANDSTONE => {
            let band = if y % 40 < 2 {
                0.84
            } else if y < 14 {
                1.06
            } else if y > 114 {
                0.94
            } else {
                1.0
            };
            col(
                [216.0, 200.0, 146.0],
                band * (0.92 + 0.07 * fbm(l, x, y, 170) + 0.04 * grain(l, x, y, 171)),
                UNTINTED,
            )
        }
        tex::SANDSTONE_TOP => {
            let fx = x as f32 + 0.5;
            let fy = y as f32 + 0.5;
            col(
                [220.0, 204.0, 150.0],
                (0.93 + 0.07 * fbm(l, x, y, 175)) * bevel(fx, fy, 0.0, 0.0, 128.0, 128.0, 6.0),
                UNTINTED,
            )
        }
        tex::SPRUCE_LOG => log_side(l, x, y, [64.0, 45.0, 28.0]),
        tex::SPRUCE_LOG_TOP => log_top(
            l,
            x,
            y,
            [64.0, 45.0, 28.0],
            [134.0, 100.0, 62.0],
            [104.0, 76.0, 46.0],
        ),
        tex::BIRCH_LOG => {
            let mark = vn2(l, x, y, 24, 5, 180) > 0.68 && (y % 20) < 5;
            let v = 0.9 + 0.1 * fbm(l, x, y, 182);
            if mark {
                col(
                    [44.0, 42.0, 40.0],
                    0.9 + 0.2 * grain(l, x, y, 181),
                    UNTINTED,
                )
            } else {
                col(
                    [218.0, 216.0, 208.0],
                    v - 0.05 * vn2(l, x, y, 3, 20, 183),
                    UNTINTED,
                )
            }
        }
        tex::BIRCH_LOG_TOP => log_top(
            l,
            x,
            y,
            [216.0, 214.0, 206.0],
            [206.0, 190.0, 150.0],
            [180.0, 160.0, 120.0],
        ),
        tex::CACTUS => {
            let rib = x % 32;
            let mut v = 0.84 + 0.12 * fbm(l, x, y, 190);
            v *= 1.0 + 0.14 * -(rib as f32 / 32.0 * std::f32::consts::TAU).cos();
            if rib < 2 {
                v *= 0.7;
            }
            let spine = hash(l, x / 4, y / 4, 191) > 0.965 && rib > 6 && rib < 26;
            if spine && (x % 4 == 1) && (y % 4 == 1) {
                return col([232.0, 228.0, 196.0], 1.0, UNTINTED);
            }
            col([78.0, 122.0, 42.0], v, UNTINTED)
        }
        tex::CACTUS_TOP => {
            let dd = ((x as f32 - 63.5).powi(2) + (y as f32 - 63.5).powi(2)).sqrt();
            let v = 0.85 + 0.15 * ((dd * 0.3).sin() * 0.5 + 0.5);
            col([90.0, 136.0, 50.0], v, UNTINTED)
        }
        tex::TALL_GRASS => {
            // Tapered blades rising from the bottom edge.
            for i in 0..18 {
                let bx = 10.0 + hash(l, i, 0, 200) * 108.0;
                let h = 50.0 + hash(l, i, 1, 201) * 74.0;
                let lean = (hash(l, i, 2, 202) - 0.5) * 60.0;
                let t = (127.5 - y as f32) / h;
                if !(0.0..1.0).contains(&t) {
                    continue;
                }
                let cx = bx + lean * t * t;
                let w = 4.2 * (1.0 - t) + 0.6;
                let dx = x as f32 + 0.5 - cx;
                if dx.abs() < w {
                    let v = 0.5
                        + 0.42 * t
                        + 0.1 * hash(l, i, 3, 203)
                        + if dx.abs() < w * 0.25 { 0.05 } else { 0.0 };
                    return col(GRAY, v.min(1.0), 255);
                }
            }
            [0, 0, 0, 0]
        }
        tex::POPPY => flower(l, x, y, [206.0, 32.0, 30.0], [40.0, 24.0, 16.0], 5.5),
        tex::DANDELION => flower(l, x, y, [246.0, 214.0, 40.0], [214.0, 150.0, 20.0], 4.5),
        tex::DEAD_BUSH => {
            let (fx, fy) = (d(x), d(y));
            let segs = [
                ((16.0, 31.0), (16.0, 18.0)),
                ((16.0, 22.0), (8.0, 12.0)),
                ((16.0, 20.0), (25.0, 9.0)),
                ((16.0, 26.0), (5.0, 21.0)),
                ((16.0, 25.0), (27.0, 19.0)),
                ((10.0, 15.0), (6.0, 6.0)),
                ((22.0, 13.0), (27.0, 4.0)),
            ];
            if segs.iter().any(|&(a, b)| seg_dist(fx, fy, a, b) < 0.55) {
                col(
                    [116.0, 82.0, 44.0],
                    0.8 + 0.3 * grain(l, x, y, 210),
                    UNTINTED,
                )
            } else {
                [0, 0, 0, 0]
            }
        }
        tex::COAL_ORE => ore(l, x, y, [40.0, 40.0, 44.0]),
        tex::IRON_ORE => ore(l, x, y, [216.0, 176.0, 146.0]),
        tex::GOLD_ORE => ore(l, x, y, [250.0, 214.0, 64.0]),
        tex::DIAMOND_ORE => ore(l, x, y, [98.0, 234.0, 226.0]),
        tex::OBSIDIAN => {
            let n = fbm(l, x, y, 220);
            let streak = vn2(l, x, y, 24, 10, 221);
            if streak > 0.72 {
                col([96.0, 68.0, 146.0], 0.7 + 0.5 * n, UNTINTED)
            } else {
                col([22.0, 16.0, 34.0], 0.75 + 0.6 * n, UNTINTED)
            }
        }
        tex::ICE => {
            let c = voronoi(l, x, y, 7, 230);
            let crack = c.d2 - c.d1 < 1.0 && vn(l, x, y, 16, 232) > 0.45;
            let gloss = 1.0 + 0.1 * (1.0 - (x + y) as f32 / 256.0);
            if crack {
                col([236.0, 244.0, 255.0], 1.0, UNTINTED)
            } else {
                col(
                    [146.0, 186.0, 244.0],
                    (0.88 + 0.1 * fbm(l, x, y, 231)) * gloss,
                    UNTINTED,
                )
            }
        }
        tex::CLAY => col(
            [160.0, 166.0, 180.0],
            0.93 + 0.08 * fbm(l, x, y, 240) + 0.03 * (grain(l, x, y, 241) - 0.5),
            UNTINTED,
        ),
        tex::LAVA => {
            let t = fbm(l, x, y, 250);
            let c = lerp3(
                [176.0, 36.0, 4.0],
                [255.0, 136.0, 18.0],
                ((t - 0.3) * 2.2).clamp(0.0, 1.0),
            );
            let c = if t > 0.66 {
                lerp3(c, [255.0, 232.0, 120.0], (t - 0.66) * 4.0)
            } else {
                c
            };
            col(c, 1.0, 255)
        }
        tex::GLOWSTONE => {
            let c = voronoi(l, x, y, 22, 260);
            if c.d2 - c.d1 < 2.4 {
                col([140.0, 96.0, 40.0], 0.9, UNTINTED)
            } else {
                let v = 0.82 + 0.25 * hash(l, c.id as i32, 0, 261) + (0.3 - c.d1 * 0.02).max(0.0);
                col([255.0, 214.0, 124.0], v, UNTINTED)
            }
        }
        tex::SKIN => col(
            [204.0, 150.0, 114.0],
            0.94 + 0.06 * fbm(l, x, y, 270),
            UNTINTED,
        ),
        tex::SLEEVE => col(SHIRT_C, 0.9 + 0.1 * fbm(l, x, y, 280), UNTINTED),
        tex::FACE..=tex::LEG => character(l, x, y),
        tex::CRAFTING_TOP..=tex::STONE_BRICKS => new_block(l, x, y),
        tex::CHEST_INSIDE | tex::CHEST_LATCH => chest(l, x, y),
        tex::TORCH_WOOD => {
            let stripe = if x < 20 {
                1.12
            } else if x > 104 {
                0.66
            } else {
                1.0
            };
            col(
                [145.0, 93.0, 45.0],
                stripe * (0.84 + 0.24 * vn2(l, x, y, 8, 64, 511)),
                255,
            )
        }
        tex::TORCH_CAP => {
            let d = ((x as f32 - 63.5).powi(2) + (y as f32 - 63.5).powi(2)).sqrt();
            if d < 28.0 {
                col([255.0, 137.0, 27.0], 0.9 + 0.1 * grain(l, x, y, 512), 255)
            } else {
                col([66.0, 48.0, 35.0], 0.83 + 0.25 * grain(l, x, y, 513), 255)
            }
        }
        // The flame's shape and colors are computed continuously by the shader.
        tex::TORCH_FLAME => [255, 255, 255, 255],
        tex::TORCH_CHAR => {
            let ember = grain(l, x, y, 514) > 0.88;
            if ember {
                col([133.0, 57.0, 20.0], 1.0, 255)
            } else {
                col([62.0, 47.0, 36.0], 0.8 + 0.3 * vn(l, x, y, 16, 515), 255)
            }
        }
        tex::HEAD_BACK => character(tex::HAIR, x, y),
        _ if (tex::FURNACE_ANIM..tex::FLAME_PARTICLE).contains(&l) => {
            pixel(tex::FURNACE_FRONT_LIT, x, y, crack)
        }
        tex::FLAME_PARTICLE => {
            // Teardrop flame: round bottom, narrowing to a point at the top.
            let (fx, fy) = (d(x) - 16.0, d(y) - 18.0);
            let r = if fy > 0.0 {
                7.0
            } else {
                7.0 * (1.0 + fy / 14.0).max(0.0)
            };
            if fx.abs() > r || fx.hypot(fy.max(0.0)) > 7.0 {
                [0, 0, 0, 0]
            } else {
                let heat = 1.0 - fx.abs() / 7.0;
                col(
                    lerp3([255.0, 120.0, 20.0], [255.0, 240.0, 150.0], heat),
                    1.0,
                    255,
                )
            }
        }
        _ if (tex::SMOKE..tex::SMOKE + SMOKE_FRAMES).contains(&l) => {
            let r = 3.0 + (l - tex::SMOKE) as f32 * 1.3;
            let (fx, fy) = (d(x) - 16.0, d(y) - 16.0);
            if fx.hypot(fy) < r {
                col([200.0, 200.0, 200.0], 0.9 + 0.1 * grain(l, x, y, 530), 255)
            } else {
                [0, 0, 0, 0]
            }
        }
        tex::SHIRT_BACK => character(tex::SHIRT, x, y),
        tex::PIG => pig_skin(x, y),
        tex::CREEPER => creeper_skin(x, y),
        tex::LANTERN => lantern(x, y),
        tex::CHAIN => chain(x, y),
        tex::TNT_SIDE | tex::TNT_TOP | tex::TNT_BOTTOM => tnt(l, x, y),
        _ if is_fire(l) => fire_frame(l, x, y),
        _ if (tex::EXPLOSION..tex::EXPLOSION + tex::EXPLOSION_FRAMES).contains(&l) => {
            explosion_frame(l - tex::EXPLOSION, x, y)
        }
        _ if is_item_icon(l) => item_icon(l, x, y),
        _ => {
            // Break cracks, stage 0..9
            let stage = (layer - tex::CRACK) as u16;
            let rank = crack[(y as usize) * TILE + x as usize];
            if rank != u16::MAX && rank <= (stage + 1) * 10 {
                // Multiply-blended: below mid-gray darkens the block underneath.
                [60, 60, 60, 255]
            } else {
                [0, 0, 0, 0]
            }
        }
    }
}

/// Rank (0..100) of each pixel in a random crack pattern; lower ranks appear first.
pub(super) fn crack_pattern() -> Vec<u16> {
    let mut rank = vec![u16::MAX; TILE * TILE];
    let branches = 8;
    for b in 0..branches {
        let (mut x, mut y) = (63.5f32, 63.5f32);
        let mut ang = b as f32 / branches as f32 * std::f32::consts::TAU + hash(900, b, 0, 1) * 0.6;
        let len = 64 + (hash(900, b, 1, 2) * 32.0) as i32;
        for step in 0..len {
            let r = (step * 100 / 96).min(100) as u16;
            for (ox, oy) in [(0, 0), (1, 0), (0, 1)] {
                let (ix, iy) = (x as i32 + ox, y as i32 + oy);
                if (0..TILE as i32).contains(&ix) && (0..TILE as i32).contains(&iy) {
                    let i = iy as usize * TILE + ix as usize;
                    rank[i] = rank[i].min(r);
                }
            }
            ang += (hash(900, b, step / 3, 3) - 0.5) * 0.5;
            x += ang.cos();
            y += ang.sin();
            // Side twigs.
            if step % 24 == 12 && hash(900, b, step, 4) > 0.4 {
                let (mut tx, mut ty) = (x, y);
                let ta = ang
                    + if hash(900, b, step, 5) > 0.5 {
                        0.9
                    } else {
                        -0.9
                    };
                for k in 0..16 {
                    let (ix, iy) = (tx as i32, ty as i32);
                    if (0..TILE as i32).contains(&ix) && (0..TILE as i32).contains(&iy) {
                        let i = iy as usize * TILE + ix as usize;
                        rank[i] = rank[i].min((r + 10 + k * 2).min(100));
                    }
                    tx += ta.cos();
                    ty += ta.sin();
                }
            }
        }
    }
    rank
}
