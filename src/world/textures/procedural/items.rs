//! Item icons: tools, ingots, gems, food, buckets, bottles, the torch, the guide book and
//! armor (and the book's and armor's surfaces as held or worn).

use super::*;

pub(super) const STICK_C: [f32; 3] = [118.0, 84.0, 44.0];

/// Material colors per tool tier: wood, stone, iron, gold, diamond.
pub(super) const TIER_C: [[f32; 3]; 5] = [
    [176.0, 138.0, 82.0],
    [132.0, 132.0, 134.0],
    [226.0, 226.0, 230.0],
    [250.0, 214.0, 64.0],
    [96.0, 230.0, 222.0],
];

pub(super) fn item_icon(l: u32, x: i32, y: i32) -> [u8; 4] {
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
        tex::IRON_INGOT | tex::GOLD_INGOT | tex::COPPER_INGOT | tex::STEEL_INGOT => {
            Some(ingot_icon(l, x, y))
        }
        tex::CERAMIC_PLATE => {
            // A cream plate with rounded corners, slightly curved (lighter at the top).
            let inside = |fx: f32, fy: f32| {
                let (dx, dy) = ((fx - 16.0).abs() - 7.0, (fy - 16.0).abs() - 10.0);
                dx.max(0.0).hypot(dy.max(0.0)) < 3.0
            };
            let c = [226.0, 214.0, 190.0].map(|c| c * (0.93 + 0.1 * n));
            shape(x, y, c, inside)
        }
        tex::FRAG_GRENADE | tex::SMOKE_GRENADE => grenade_icon(l == tex::SMOKE_GRENADE, x, y),
        // Drawn from its model later (`render_item_icons`).
        tex::FLASHLIGHT => None,
        _ if (tex::REVOLVER..=tex::TARGET_DUMMY).contains(&l) => None,
        tex::AMMO_BOX => {
            // An olive ammo can seen from the front and a little above: its open top full of
            // brass, its front with a yellow stencilled band.
            let can = |fx: f32, fy: f32| (5.0..27.0).contains(&fx) && (10.0..27.0).contains(&fy);
            let top = (6.0..26.0).contains(&fx) && (10.0..15.0).contains(&fy);
            let c = if top {
                let (cx, cy) = ((fx - 6.0) % 3.0, (fy - 10.0) % 2.5);
                if (cx - 1.5).hypot(cy - 1.25) < 1.1 {
                    [206.0, 164.0, 70.0]
                } else {
                    [60.0, 48.0, 30.0]
                }
            } else if (18.0..20.0).contains(&fy) && (9.0..23.0).contains(&fx) {
                [214.0, 196.0, 112.0]
            } else if fy < 16.0 {
                [70.0, 76.0, 46.0]
            } else {
                [92.0, 100.0, 60.0].map(|c| c * (0.95 + 0.08 * n))
            };
            shape(x, y, c, can)
        }
        _ if (tex::ARMOR_ICONS..tex::ARMOR_ICONS + 17).contains(&l) => armor_icon(l, x, y),
        tex::BOOK => book_icon(x, y),
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
        tex::SHEARS => {
            // Two blades crossing at the pivot, with round handles.
            let blade = |a: (f32, f32), b: (f32, f32)| seg_dist(fx, fy, a, b) < 1.8;
            let handle = |cx: f32, cy: f32| (2.0..3.6).contains(&(fx - cx).hypot(fy - cy));
            if blade((15.0, 17.0), (26.0, 6.0)) || blade((17.0, 15.0), (24.0, 5.0)) {
                Some(col([214.0, 216.0, 222.0], 0.92 + 0.12 * n, 255))
            } else if handle(9.0, 23.0) || handle(13.0, 27.0) {
                Some(col([120.0, 60.0, 40.0], 0.9 + 0.15 * n, 255))
            } else {
                None
            }
        }
        tex::BONE => {
            // A pale bone lying diagonally, two knobs at each end.
            let bone = |fx: f32, fy: f32| {
                let off = (fx - fy).abs() / std::f32::consts::SQRT_2;
                let along = (fx + fy) * 0.5;
                (off < 1.8 && (7.0..25.0).contains(&along))
                    || [(7.0, 9.5), (9.5, 7.0), (22.5, 25.0), (25.0, 22.5)].iter().any(|&(cx, cy)| (fx - cx).hypot(fy - cy) < 3.0)
            };
            shape(x, y, [236.0, 228.0, 206.0], bone)
        }
        tex::PIG_SPAWN_EGG | tex::SHEEP_SPAWN_EGG | tex::WOLF_SPAWN_EGG => {
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
            let c = match (l, spot) {
                (tex::WOLF_SPAWN_EGG, true) => [196.0, 164.0, 128.0],
                (tex::WOLF_SPAWN_EGG, false) => [216.0, 212.0, 208.0],
                (tex::SHEEP_SPAWN_EGG, true) => [250.0, 182.0, 182.0],
                (tex::SHEEP_SPAWN_EGG, false) => [232.0, 232.0, 232.0],
                (_, true) => [196.0, 88.0, 96.0],
                _ => [240.0, 164.0, 162.0],
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
        tex::PORKCHOP | tex::COOKED_PORKCHOP | tex::MUTTON | tex::COOKED_MUTTON => {
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
            let (flesh, fat) = match l {
                tex::COOKED_PORKCHOP => ([152.0, 86.0, 46.0], [214.0, 172.0, 112.0]),
                tex::COOKED_MUTTON => ([124.0, 70.0, 40.0], [196.0, 150.0, 100.0]),
                tex::MUTTON => ([196.0, 60.0, 62.0], [240.0, 206.0, 196.0]),
                _ => ([226.0, 108.0, 116.0], [246.0, 216.0, 208.0]),
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
        tex::RAW_FISH | tex::COOKED_FISH => fish_icon(l == tex::COOKED_FISH, x, y, n),
        _ if (tex::PISTOL..tex::GUN_GLASS).contains(&l) => gun_icon(l, x, y),
        _ => Some(tool_icon(l, x, y)).filter(|p| p[3] > 0),
    };
    out.unwrap_or([0, 0, 0, 0])
}

pub(super) fn ingot_icon(l: u32, x: i32, y: i32) -> [u8; 4] {
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
    let (rim, top, highlight, side, end) = if l == tex::STEEL_INGOT {
        (
            [34.0, 38.0, 46.0],
            [138.0, 146.0, 160.0],
            [200.0, 208.0, 222.0],
            [96.0, 104.0, 118.0],
            [70.0, 76.0, 88.0],
        )
    } else if l == tex::COPPER_INGOT {
        (
            [92.0, 38.0, 20.0],
            [228.0, 132.0, 84.0],
            [255.0, 196.0, 150.0],
            [184.0, 94.0, 56.0],
            [140.0, 66.0, 38.0],
        )
    } else if gold {
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

pub(super) fn diamond_icon(x: i32, y: i32) -> [u8; 4] {
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
pub(super) fn tool_part(kind: u32, u: f32, v: f32) -> u8 {
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
pub(super) fn tool_icon(l: u32, x: i32, y: i32) -> [u8; 4] {
    let i = if l >= tex::MORE_TOOLS {
        20 + l - tex::MORE_TOOLS
    } else {
        l - tex::TOOLS
    };
    let (tier, kind) = ((i / 4) as usize, i % 4);
    // [outline, dark, mid, light] per tier; handles use dark oak colors.
    const HANDLE: [[f32; 3]; 4] = [
        [40.0, 26.0, 12.0],
        [73.0, 54.0, 21.0],
        [104.0, 78.0, 30.0],
        [137.0, 103.0, 39.0],
    ];
    const TIERS: [[[f32; 3]; 4]; 6] = [
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
        // Copper
        [
            [70.0, 30.0, 14.0],
            [170.0, 84.0, 48.0],
            [222.0, 128.0, 80.0],
            [255.0, 190.0, 146.0],
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

/// A cod lying corner to corner (head at the bottom left, tail at the top right): raw, sandy
/// with a darker back and a pale belly; cooked, white flaky flesh between a toasted head and
/// tail.
pub(super) fn fish_icon(cooked: bool, x: i32, y: i32, n: f32) -> Option<[u8; 4]> {
    let (fx, fy) = (d(x), d(y));
    // Along the fish (0 head .. 1 tail root) and across it (+ toward the back, up-left).
    let (head, tail): ((f32, f32), (f32, f32)) = ((6.0, 26.0), (23.5, 9.5));
    let (dx, dy) = (tail.0 - head.0, tail.1 - head.1);
    let len = (dx * dx + dy * dy).sqrt();
    let along = |fx: f32, fy: f32| ((fx - head.0) * dx + (fy - head.1) * dy) / (len * len);
    let across = |fx: f32, fy: f32| ((fx - head.0) * dy - (fy - head.1) * dx) / len;
    let body = |fx: f32, fy: f32| {
        let t = along(fx, fy);
        let half = 6.5 * (1.0 - ((t - 0.42) / 0.58).powi(2)).max(0.0).sqrt() + 0.6;
        (-0.05..=1.0).contains(&t) && across(fx, fy).abs() < half
    };
    let fin = |fx: f32, fy: f32| {
        // The tail: a fan past the root, notched in the middle.
        let (t, a) = (along(fx, fy), across(fx, fy));
        t > 0.9 && t < 1.3 && a.abs() < 1.0 + (t - 0.95).max(0.0) * 16.0 && !(t > 1.2 && a.abs() < (t - 1.2) * 16.0)
    };
    let inside = |fx: f32, fy: f32| body(fx, fy) || fin(fx, fy);
    let t = along(fx, fy);
    let a = across(fx, fy);
    let eye = (fx - 8.5).hypot(fy - 22.0) < 1.3;
    let c = if eye {
        if cooked { [232.0, 226.0, 210.0] } else { [26.0, 20.0, 16.0] }
    } else if !body(fx, fy) || t < 0.22 {
        // tail, head
        if cooked { [196.0, 152.0, 102.0] } else { [206.0, 176.0, 132.0] }
    } else if cooked {
        if (t * 10.0 + a.abs() * 0.15).fract() < 0.2 { [206.0, 190.0, 150.0] } else { [236.0, 226.0, 196.0] }
    } else if a > 1.5 {
        [178.0, 138.0, 90.0]
    } else if a < -2.0 {
        [220.0, 200.0, 164.0]
    } else {
        [200.0, 164.0, 116.0]
    };
    shape(x, y, c, inside).map(|p| if eye { col(c, 1.0, 255) } else { col([p[0] as f32, p[1] as f32, p[2] as f32], 0.95 + 0.08 * n, 255) })
}

pub(super) fn torch_icon(x: i32, y: i32) -> [u8; 4] {
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

pub(super) const BOOK_LEATHER: [f32; 3] = [128.0, 46.0, 34.0];
pub(super) const BOOK_GOLD: [f32; 3] = [226.0, 180.0, 74.0];
pub(super) const BOOK_PAPER: [f32; 3] = [238.0, 228.0, 198.0];

/// The guide book's icon: a red leather cover with gold bands and a gold diamond on it,
/// its spine darker, and the pages showing along the right and bottom.
pub(super) fn book_icon(x: i32, y: i32) -> Option<[u8; 4]> {
    let l = tex::BOOK;
    let (fx, fy) = (d(x), d(y));
    let cover = |fx: f32, fy: f32| fx > 5.5 && fx < 24.5 && fy > 3.5 && fy < 27.0;
    let pages = |fx: f32, fy: f32| fx > 8.0 && fx < 26.5 && fy > 5.5 && fy < 28.8;
    if cover(fx, fy) {
        let band = ((8.0..9.6).contains(&fy) || (21.0..22.6).contains(&fy)) && fx > 11.0 && fx < 22.5;
        let emblem = (fx - 16.8).abs() + (fy - 15.3).abs() < 3.4;
        let spine = fx < 9.5;
        let seam = (9.5..10.3).contains(&fx);
        let n = 0.9 + 0.16 * fbm(l, x, y, 790);
        return shape(x, y, BOOK_LEATHER, cover).map(|p| {
            if p[0] < 80 && !band && !emblem {
                // The outline stays.
                p
            } else if band || emblem {
                col(BOOK_GOLD, 0.95 + 0.1 * grain(l, x, y, 791), 255)
            } else if seam {
                col(BOOK_LEATHER, 0.55, 255)
            } else if spine {
                col(BOOK_LEATHER, 0.75 * n, 255)
            } else {
                col(BOOK_LEATHER, n, 255)
            }
        });
    }
    shape(x, y, BOOK_PAPER, pages).map(|p| {
        // The edges of the sheets.
        let sheet = if fx > 24.5 { (y % 4 == 0) as u8 } else { (x % 4 == 0) as u8 };
        if p[0] < 150 {
            p
        } else {
            col(BOOK_PAPER, 1.0 - 0.14 * sheet as f32, 255)
        }
    })
}

/// The open book in a player's hands: its leather (with a gold line near the edge), the edges
/// of its sheets, and a written page (lines of words, a heading and a small picture).
pub(super) fn book_surface(l: u32, x: i32, y: i32) -> [u8; 4] {
    let t = TILE as i32;
    match l {
        tex::BOOK_COVER => {
            let edge = x.min(y).min(t - 1 - x).min(t - 1 - y);
            if (10..14).contains(&edge) {
                col(BOOK_GOLD, 0.9 + 0.1 * grain(l, x, y, 792), 255)
            } else {
                col(BOOK_LEATHER, 0.86 + 0.2 * fbm(l, x, y, 793), 255)
            }
        }
        tex::BOOK_EDGE => {
            let sheet = (y % 6 == 0) || (x % 6 == 0);
            col(BOOK_PAPER, if sheet { 0.8 } else { 0.97 + 0.05 * grain(l, x, y, 794) }, 255)
        }
        _ => {
            let paper = col(BOOK_PAPER, 0.96 + 0.06 * fbm(l, x, y, 795), 255);
            let ink = col([70.0, 58.0, 48.0], 1.0, 255);
            let (m, top) = (14, 14);
            if x < m || x >= t - m || y < top || y >= t - 14 {
                return paper;
            }
            // A heading, a picture in the upper right, then lines of words.
            if y < top + 8 {
                return if x < 70 && (y - top) % 8 < 5 { col([140.0, 40.0, 30.0], 1.0, 255) } else { paper };
            }
            if (74..t - m).contains(&x) && (top + 14..top + 46).contains(&y) {
                let frame = x == 74 || x == t - m - 1 || y == top + 14 || y == top + 45;
                return if frame { ink } else { col([180.0, 160.0, 120.0], 0.9 + 0.2 * vn(l, x, y, 8, 796), 255) };
            }
            let row = (y - top - 14) / 8;
            let in_row = (y - top - 14) % 8 < 4;
            let right = if (top + 14..top + 46).contains(&y) { 68 } else { t - m };
            // Each line ends somewhere, the last of a paragraph early.
            let end = if row % 5 == 4 { 30 + (hash(l, row, 0, 797) * 50.0) as i32 } else { right };
            let word = (x + row * 37) % 17 < 3;
            if in_row && x < end.min(right) && !word {
                ink
            } else {
                paper
            }
        }
    }
}

/// Armor icons: a helmet, chestplate, leggings or boots in its material's color (wool
/// quilted, the metals with a shine), or the olive bulletproof vest with its pouches.
pub(super) fn armor_icon(l: u32, x: i32, y: i32) -> Option<[u8; 4]> {
    let i = l - tex::ARMOR_ICONS;
    let (fx, fy) = (d(x), d(y));
    if i == 16 {
        let vest = |fx: f32, fy: f32| {
            in_polygon(fx, fy, &[(8.0, 5.0), (13.0, 5.0), (16.0, 9.0), (19.0, 5.0), (24.0, 5.0), (26.5, 10.0), (26.5, 27.0), (5.5, 27.0), (5.5, 10.0)])
        };
        let pouch = (fy > 17.0 && fy < 23.0) && ((7.5..11.5).contains(&fx) || (12.5..19.5).contains(&fx) || (20.5..24.5).contains(&fx));
        let strap = (fy - 13.0).abs() < 0.6;
        let c = if pouch {
            [84.0, 90.0, 60.0]
        } else if strap {
            [60.0, 64.0, 44.0]
        } else {
            [110.0, 118.0, 80.0]
        };
        return shape(x, y, c, vest);
    }
    let (piece, material) = ((i % 4) as usize, (i / 4) as usize);
    let color = [[200.0, 188.0, 160.0], [226.0, 140.0, 88.0], [178.0, 186.0, 200.0], [110.0, 226.0, 230.0]][material];
    let inside = |fx: f32, fy: f32| match piece {
        0 => {
            // A dome over the head, cheek guards down the sides, the face open between.
            let dome = ((fx - 16.0) / 11.5).powi(2) + ((fy - 17.0) / 11.0).powi(2) < 1.0 && fy < 17.5;
            let cheeks = (fy >= 17.5 && fy < 25.0) && ((4.5..9.5).contains(&fx) || (22.5..27.5).contains(&fx));
            dome || cheeks
        }
        1 => in_polygon(
            fx,
            fy,
            &[(5.0, 6.0), (12.0, 6.0), (16.0, 9.5), (20.0, 6.0), (27.0, 6.0), (28.5, 13.0), (24.0, 14.0), (24.0, 27.0), (8.0, 27.0), (8.0, 14.0), (3.5, 13.0)],
        ),
        2 => in_polygon(fx, fy, &[(8.0, 5.0), (24.0, 5.0), (25.5, 27.5), (19.0, 27.5), (16.0, 13.0), (13.0, 27.5), (6.5, 27.5)]),
        _ => {
            let leg = |x0: f32| (x0..x0 + 6.0).contains(&fx) && (12.0..28.0).contains(&fy);
            let toe = |x0: f32| (x0..x0 + 9.0).contains(&fx) && (23.5..28.0).contains(&fy);
            leg(4.0) || toe(4.0) || leg(18.0) || toe(18.0)
        }
    };
    let quilt = material == 0 && ((fx + fy) % 5.0 < 0.8 || (fx - fy).rem_euclid(5.0) < 0.8);
    let shine = material > 0 && (fx - fy + 4.0).abs() < 1.2;
    let c = color.map(|c| {
        c * if quilt {
            0.8
        } else if shine {
            1.25
        } else {
            1.0
        }
    });
    shape(x, y, c.map(|c: f32| c.min(255.0)), inside)
}

/// What armor looks like worn (tinted by its material): quilted cloth, riveted metal
/// plates, or the vest's woven fabric with straps of webbing across.
pub(super) fn armor_surface(l: u32, x: i32, y: i32) -> [u8; 4] {
    let (fx, fy) = (x as f32, y as f32);
    match l {
        tex::ARMOR_WOOL => {
            let q = 32.0;
            let seam = (fx + fy).rem_euclid(q) < 3.0 || (fx - fy).rem_euclid(q) < 3.0;
            let v = if seam { 0.72 } else { 0.95 + 0.08 * fbm(l, x, y, 740) };
            col([232.0; 3], v * (0.94 + 0.06 * grain(l, x, y, 741)), 255)
        }
        tex::ARMOR_METAL => {
            // Two plates with a seam between them, a rivet in each corner, a bevel.
            let (u, v) = (fx % 64.0, fy);
            let edge = u < 3.0 || u > 61.0 || v < 3.0 || v > 125.0;
            let rivet = [(8.0, 8.0), (56.0, 8.0), (8.0, 120.0), (56.0, 120.0)]
                .iter()
                .any(|&(cx, cy)| (u - cx).hypot(v - cy) < 3.2);
            let k = if rivet {
                1.25
            } else if edge {
                0.7
            } else {
                brushed(l, x, y, 742) * (1.0 - (v - 64.0).abs() / 64.0 * 0.12)
            };
            col([225.0; 3], k, 255)
        }
        _ => {
            // Webbing straps every 24 texels, stitched down.
            let band = (fy % 24.0) < 9.0;
            let stitch = band && (fx % 16.0) < 2.0;
            let weave = 0.92 + 0.08 * (((x + y) % 4 < 2) as i32 as f32) + 0.05 * grain(l, x, y, 743);
            let v = if stitch { 0.6 } else if band { 0.82 * weave } else { weave };
            col([215.0; 3], v, 255)
        }
    }
}
