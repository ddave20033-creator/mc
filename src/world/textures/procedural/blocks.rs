//! Crafted blocks: the crafting table, furnace, chest, storage blocks, stone bricks, bed,
//! lantern and chain, the torch's parts and saplings; and the break cracks.

use super::*;

pub(super) fn new_block(l: u32, x: i32, y: i32) -> [u8; 4] {
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

pub(super) fn crafting(l: u32, x: i32, y: i32) -> [u8; 4] {
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

pub(super) fn furnace(l: u32, x: i32, y: i32) -> [u8; 4] {
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

pub(super) fn chest(l: u32, x: i32, y: i32) -> [u8; 4] {
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

/// Framed, bevelled storage block (iron/gold/diamond/coal).
pub(super) fn metal_block(l: u32, x: i32, y: i32, c: [f32; 3]) -> [u8; 4] {
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

/// A bucket's galvanized steel: pale mottled spangles, faint streaks down it, and two pressed
/// ridges around it (the rows are the wall from the rim down).
pub(super) fn bucket_metal(l: u32, x: i32, y: i32) -> [u8; 4] {
    let fy = y as f32 + 0.5;
    let spangle = voronoi(l, x, y, 40, 470);
    let flake = 0.94 + 0.08 * hash(l, spangle.id as i32, 0, 471);
    let v = flake + 0.04 * (vn2(l, x, y, 2, 24, 472) - 0.5) + 0.02 * (grain(l, x, y, 473) - 0.5);
    // A ridge: a light edge above, a dark one below.
    let ridge = |at: f32| {
        let d = fy - at;
        if (-4.0..0.0).contains(&d) {
            1.1
        } else if (0.0..4.0).contains(&d) {
            0.8
        } else {
            1.0
        }
    };
    col([178.0, 184.0, 190.0], v * ridge(40.0) * ridge(88.0), UNTINTED)
}

/// Red bed faces (the `tex::BED_*` layout, head toward the top/north) and its item, for when
/// no pack has them: a red blanket, a white pillow at the head, a wooden frame with a leg
/// under each corner.
pub(super) fn bed(l: u32, x: i32, y: i32) -> [u8; 4] {
    const P: i32 = TILE as i32 / 16;
    let (mx, my) = (x / P, y / P);
    let red = |v: f32| col([150.0, 28.0, 30.0], v * (0.9 + 0.15 * grain(l, x, y, 540)), 255);
    let white = |v: f32| col([228.0, 228.0, 232.0], v * (0.96 + 0.05 * grain(l, x, y, 541)), 255);
    let wood = |v: f32| {
        let p = planks(tex::PLANKS, x, y);
        col([p[0] as f32, p[1] as f32, p[2] as f32], v, 255)
    };
    const CLEAR: [u8; 4] = [0, 0, 0, 0];
    match l {
        tex::BED_HEAD_TOP if my < 7 => {
            let rim = !(1..15).contains(&mx) || !(1..6).contains(&my);
            white(if rim { 0.85 } else { 1.0 })
        }
        tex::BED_HEAD_TOP | tex::BED_FOOT_TOP => red(if mx == 0 || mx == 15 { 0.8 } else { 1.0 }),
        tex::BED_BOTTOM => wood(0.8),
        tex::BED_ITEM => {
            // Seen from the side: blanket, pillow on the right, frame and two legs.
            match my {
                5..=8 if (1..15).contains(&mx) => {
                    if mx >= 11 {
                        white(1.0)
                    } else {
                        red(1.0)
                    }
                }
                9..=10 if (1..15).contains(&mx) => wood(0.85),
                11..=12 if (1..3).contains(&mx) || (13..15).contains(&mx) => wood(0.6),
                _ => CLEAR,
            }
        }
        // Sides: the top 7 pixels are above the bed; blanket (and pillow), frame, legs.
        _ => {
            let (head, leg_left, leg_right) = match l {
                tex::BED_HEAD_EAST => (Some(false), false, true),
                tex::BED_HEAD_WEST => (Some(true), true, false),
                tex::BED_FOOT_EAST => (None, true, false),
                tex::BED_FOOT_WEST => (None, false, true),
                tex::BED_HEAD_END => (Some(true), true, true),
                _ => (None, true, true),
            };
            match my {
                0..=6 => CLEAR,
                7..=9 => match head {
                    // Head end: all pillow; head sides: the pillow half toward the head.
                    Some(_) if l == tex::BED_HEAD_END => white(0.95),
                    Some(left) if (mx < 8) == left => white(0.95),
                    _ => red(0.9),
                },
                10..=12 => wood(if my == 10 { 1.0 } else { 0.85 }),
                _ if (leg_left && mx < 3) || (leg_right && mx >= 13) => wood(0.6),
                _ => CLEAR,
            }
        }
    }
}

pub(super) const LANTERN_METAL: [f32; 3] = [64.0, 70.0, 86.0];

/// Built-in lantern texture in Minecraft's layout (16 units): body sides (0,2)-(6,9) with the
/// glowing window, cap sides (1,0)-(5,2), body top/bottom (0,9)-(6,15), cap top
/// (1,10)-(5,14), hanging ring (11,1)-(14,5) and standing handle (11,10)-(14,12).
pub(super) fn lantern(x: i32, y: i32) -> [u8; 4] {
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
pub(super) fn chain(x: i32, y: i32) -> [u8; 4] {
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

pub(super) fn torch_wood(l: u32, x: i32, y: i32) -> [u8; 4] {
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

pub(super) fn torch_cap(l: u32, x: i32, y: i32) -> [u8; 4] {
    let d = ((x as f32 - 63.5).powi(2) + (y as f32 - 63.5).powi(2)).sqrt();
    if d < 28.0 {
        col([255.0, 137.0, 27.0], 0.9 + 0.1 * grain(l, x, y, 512), 255)
    } else {
        col([66.0, 48.0, 35.0], 0.83 + 0.25 * grain(l, x, y, 513), 255)
    }
}

pub(super) fn torch_char(l: u32, x: i32, y: i32) -> [u8; 4] {
    let ember = grain(l, x, y, 514) > 0.88;
    if ember {
        col([133.0, 57.0, 20.0], 1.0, 255)
    } else {
        col([62.0, 47.0, 36.0], 0.8 + 0.3 * vn(l, x, y, 16, 515), 255)
    }
}

/// Break cracks, stage 0..9 (`layer` - `tex::CRACK`), from `crack_pattern`.
pub(super) fn crack_pixel(layer: u32, x: i32, y: i32, crack: &[u16]) -> [u8; 4] {
    let stage = (layer - tex::CRACK) as u16;
    let rank = crack[(y as usize) * TILE + x as usize];
    if rank != u16::MAX && rank <= (stage + 1) * 10 {
        // Multiply-blended: below mid-gray darkens the block underneath.
        [60, 60, 60, 255]
    } else {
        [0, 0, 0, 0]
    }
}

/// Rank (0..100) of each pixel in a random crack pattern; lower ranks appear first.
pub(in crate::world::textures) fn crack_pattern() -> Vec<u16> {
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
