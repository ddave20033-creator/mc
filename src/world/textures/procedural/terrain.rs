//! Natural blocks: grass, dirt, stone, sand, logs, leaves, plants, ores and the other
//! terrain; and oak planks.

use super::*;

pub(super) fn grass_gray(l: u32, x: i32, y: i32) -> [u8; 4] {
    // Soft clumps plus short vertical blade strokes.
    let clumps = fbm(l, x, y, 1);
    let blades = vn2(l, x, y, 2, 7, 5) * 0.6 + vn2(l, x + 1, y, 3, 5, 6) * 0.4;
    let mut v = 0.66 + 0.2 * clumps + 0.16 * (blades - 0.5);
    if hash(l, x / 2, y / 2, 3) > 0.985 {
        v *= 1.12;
    }
    col(GRAY, v.min(1.0), 255)
}

pub(super) fn grass_side(l: u32, x: i32, y: i32) -> [u8; 4] {
    // Grass (or snow) hanging over the dirt with an uneven, dripping edge.
    let fx = x;
    let depth = 18
        + (vn(l, fx, 0, 16, 80) * 12.0) as i32
        + if vn(l, fx, 0, 4, 81) > 0.72 { 8 } else { 0 };
    if y < depth {
        if l == tex::GRASS_SIDE {
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

pub(super) fn dirt(l: u32, x: i32, y: i32) -> [u8; 4] {
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

pub(super) fn stone_base(l: u32, x: i32, y: i32) -> [u8; 4] {
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

pub(super) fn sand(l: u32, x: i32, y: i32) -> [u8; 4] {
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

pub(super) fn water(l: u32, x: i32, y: i32) -> [u8; 4] {
    // Directional waves repeat across every block and form visible bands at a distance.
    // Keep the albedo subtle; moving ripples and highlights come from the water shader.
    let v = 0.87
        + 0.08 * fbm(l, x, y, 100)
        + 0.02 * (vn(l, x, y, 16, 106) - 0.5)
        + 0.01 * (grain(l, x, y, 107) - 0.5);
    col(GRAY, v, 255)
}

pub(super) fn snow(l: u32, x: i32, y: i32) -> [u8; 4] {
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

pub(super) fn cobble(l: u32, x: i32, y: i32) -> [u8; 4] {
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

pub(super) fn bedrock(l: u32, x: i32, y: i32) -> [u8; 4] {
    let v = 0.2 + 0.85 * vn(l, x, y, 16, 140) * (0.55 + 0.5 * vn(l, x, y, 4, 141));
    col([90.0, 90.0, 94.0], v, UNTINTED)
}

pub(super) fn glass(x: i32, y: i32) -> [u8; 4] {
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

pub(super) fn bricks(l: u32, x: i32, y: i32) -> [u8; 4] {
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

pub(super) fn gravel(l: u32, x: i32, y: i32) -> [u8; 4] {
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

pub(super) fn sandstone(l: u32, x: i32, y: i32) -> [u8; 4] {
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

pub(super) fn sandstone_top(l: u32, x: i32, y: i32) -> [u8; 4] {
    let fx = x as f32 + 0.5;
    let fy = y as f32 + 0.5;
    col(
        [220.0, 204.0, 150.0],
        (0.93 + 0.07 * fbm(l, x, y, 175)) * bevel(fx, fy, 0.0, 0.0, 128.0, 128.0, 6.0),
        UNTINTED,
    )
}

pub(super) fn log_side(l: u32, x: i32, y: i32, base: [f32; 3]) -> [u8; 4] {
    let ridges = vn2(l, x, y, 6, 42, 30);
    let fine = vn2(l, x, y, 2, 12, 31);
    let mut v = 0.66 + 0.42 * ridges + 0.1 * (fine - 0.5);
    if ridges < 0.3 {
        v *= 0.8;
    }
    col(base, v, UNTINTED)
}

pub(super) fn log_top(l: u32, x: i32, y: i32, bark: [f32; 3], light: [f32; 3], dark: [f32; 3]) -> [u8; 4] {
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

pub(super) fn birch_log(l: u32, x: i32, y: i32) -> [u8; 4] {
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

/// Leaves: shaded clusters over a dense canopy with a few consistent cut-out gaps.
pub(super) fn leaves(l: u32, x: i32, y: i32, needles: bool) -> [u8; 4] {
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

pub(super) fn cactus(l: u32, x: i32, y: i32) -> [u8; 4] {
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

pub(super) fn cactus_top(x: i32, y: i32) -> [u8; 4] {
    let dd = ((x as f32 - 63.5).powi(2) + (y as f32 - 63.5).powi(2)).sqrt();
    let v = 0.85 + 0.15 * ((dd * 0.3).sin() * 0.5 + 0.5);
    col([90.0, 136.0, 50.0], v, UNTINTED)
}

pub(super) fn tall_grass(l: u32, x: i32, y: i32) -> [u8; 4] {
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

pub(super) fn flower(l: u32, x: i32, y: i32, petal: [f32; 3], center: [f32; 3], radius: f32) -> [u8; 4] {
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

pub(super) fn dead_bush(l: u32, x: i32, y: i32) -> [u8; 4] {
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

/// Ore blobs with faceted, shiny crystals on top of stone.
pub(super) fn ore(l: u32, x: i32, y: i32, color: [f32; 3]) -> [u8; 4] {
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

pub(super) fn obsidian(l: u32, x: i32, y: i32) -> [u8; 4] {
    let n = fbm(l, x, y, 220);
    let streak = vn2(l, x, y, 24, 10, 221);
    if streak > 0.72 {
        col([96.0, 68.0, 146.0], 0.7 + 0.5 * n, UNTINTED)
    } else {
        col([22.0, 16.0, 34.0], 0.75 + 0.6 * n, UNTINTED)
    }
}

pub(super) fn ice(l: u32, x: i32, y: i32) -> [u8; 4] {
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

pub(super) fn clay(l: u32, x: i32, y: i32) -> [u8; 4] {
    col(
        [160.0, 166.0, 180.0],
        0.93 + 0.08 * fbm(l, x, y, 240) + 0.03 * (grain(l, x, y, 241) - 0.5),
        UNTINTED,
    )
}

pub(super) fn lava(l: u32, x: i32, y: i32) -> [u8; 4] {
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

pub(super) fn glowstone(l: u32, x: i32, y: i32) -> [u8; 4] {
    let c = voronoi(l, x, y, 22, 260);
    if c.d2 - c.d1 < 2.4 {
        col([140.0, 96.0, 40.0], 0.9, UNTINTED)
    } else {
        let v = 0.82 + 0.25 * hash(l, c.id as i32, 0, 261) + (0.3 - c.d1 * 0.02).max(0.0);
        col([255.0, 214.0, 124.0], v, UNTINTED)
    }
}

pub(super) fn planks(l: u32, x: i32, y: i32) -> [u8; 4] {
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
