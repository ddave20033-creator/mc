//! Characters and creatures (the player's skin, the pig's, the sheep's and the wolf's
//! pages) and particles (flame, smoke, the big smoke cloud).

use super::*;

pub(super) const HAIR_C: [f32; 3] = [66.0, 42.0, 24.0];
pub(super) const SKIN_C: [f32; 3] = [206.0, 150.0, 112.0];
pub(super) const SHIRT_C: [f32; 3] = [40.0, 150.0, 162.0];
pub(super) const PANTS_C: [f32; 3] = [56.0, 64.0, 150.0];

/// Character skin textures, drawn on an 8x8 grid of 16-pixel cells like a classic blocky skin.
pub(super) fn character(l: u32, x: i32, y: i32) -> [u8; 4] {
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

pub(super) fn skin(l: u32, x: i32, y: i32) -> [u8; 4] {
    col(
        [204.0, 150.0, 114.0],
        0.94 + 0.06 * fbm(l, x, y, 270),
        UNTINTED,
    )
}

/// Built-in pig skin, laid out like Minecraft's pig texture (64x64 unit atlas, 2 px per unit
/// here; cut onto the `tex::PIG` pages): head at (0,0), snout at (16,16), legs at (0,16), body at (28,8).
pub(super) fn pig_skin(x: i32, y: i32) -> [u8; 4] {
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

/// A page of the pig's skin (`tex::PIG`..): the built-in atlas (`pig_skin`) cut up like a
/// pack's.
pub(super) fn pig_page(l: u32, x: i32, y: i32) -> [u8; 4] {
    match crate::entity::mob::pig_skin::SKIN.atlas_at(l - tex::PIG, x as u32, y as u32) {
        Some((u, v)) => pig_skin((u * 2.0) as i32, (v * 2.0) as i32),
        None => [0, 0, 0, 0],
    }
}

/// A page of the sheep's skin: plain skin colour.
pub(super) fn sheep_page(l: u32, x: i32, y: i32) -> [u8; 4] {
    match crate::entity::mob::sheep_skin::SKIN.atlas_at(l - tex::SHEEP, x as u32, y as u32) {
        Some(_) => col([214.0, 178.0, 150.0], 0.92 + 0.08 * grain(l, x, y, 562), 255),
        None => [0, 0, 0, 0],
    }
}

/// A page of the sheep's wool coat: plain white wool.
pub(super) fn sheep_wool_page(l: u32, x: i32, y: i32) -> [u8; 4] {
    match crate::entity::mob::sheep_skin::WOOL.atlas_at(l - tex::SHEEP_WOOL, x as u32, y as u32) {
        Some(_) => wool(l, x, y),
        None => [0, 0, 0, 0],
    }
}

/// Wool: soft white fibres (the block, and the sheep's coat).
pub(super) fn wool(l: u32, x: i32, y: i32) -> [u8; 4] {
    col(
        [236.0, 236.0, 236.0],
        0.9 + 0.06 * fbm(l, x, y, 560) + 0.06 * grain(l, x, y, 561),
        255,
    )
}

/// Teardrop flame particle.
pub(super) fn flame_particle(x: i32, y: i32) -> [u8; 4] {
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

/// Smoke particle frames (`tex::SMOKE`..): round puffs, bigger each frame.
pub(super) fn smoke_puff(l: u32, x: i32, y: i32) -> [u8; 4] {
    let r = 3.0 + (l - tex::SMOKE) as f32 * 1.3;
    let (fx, fy) = (d(x) - 16.0, d(y) - 16.0);
    if fx.hypot(fy) < r {
        col([200.0, 200.0, 200.0], 0.9 + 0.1 * grain(l, x, y, 530), 255)
    } else {
        [0, 0, 0, 0]
    }
}

/// A lumpy round puff of smoke (a few overlapping balls), lighter on top.
pub(super) fn cloud_puff(l: u32, x: i32, y: i32) -> [u8; 4] {
    let (fx, fy) = (d(x), d(y));
    const BALLS: [(f32, f32, f32); 5] = [
        (16.0, 18.0, 10.5),
        (9.5, 20.0, 7.0),
        (22.5, 20.0, 7.0),
        (12.5, 11.5, 6.5),
        (20.0, 11.0, 6.0),
    ];
    let inside = BALLS
        .iter()
        .any(|&(cx, cy, r)| (fx - cx).hypot(fy - cy) < r);
    if !inside {
        return [0, 0, 0, 0];
    }
    let shade = 0.82 + 0.18 * (1.0 - fy / 32.0) + 0.06 * (grain(l, x, y, 531) - 0.5);
    col([235.0, 235.0, 235.0], shade, 255)
}
