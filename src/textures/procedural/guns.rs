//! Guns: the gun station, the pistol model's surfaces, the gun, part, attachment,
//! cartridge and grenade icons, the muzzle flash and the bullet hole.

use super::*;

pub(super) const BRASS: [f32; 3] = [214.0, 168.0, 76.0];
pub(super) const COPPER: [f32; 3] = [190.0, 112.0, 64.0];
pub(super) const GUN_BLUED_C: [f32; 3] = [54.0, 58.0, 68.0];
pub(super) const GUN_STEEL_C: [f32; 3] = [172.0, 176.0, 182.0];
pub(super) const GUN_POLYMER_C: [f32; 3] = [42.0, 42.0, 45.0];

/// Brushed metal brightness: fine streaks along x.
pub(super) fn brushed(l: u32, x: i32, y: i32, s: u32) -> f32 {
    0.9 + 0.1 * vn2(l, x, y, 32, 2, s) + 0.05 * (grain(l, x, y, s + 1) - 0.5)
}

/// A cartridge lying from its base `a` to its tip `b` (design units), `r` thick: a brass case
/// and a copper bullet narrowing to a round nose, shaded as a cylinder lit from the top left.
pub(super) fn cartridge(fx: f32, fy: f32, a: (f32, f32), b: (f32, f32), r: f32) -> Option<[f32; 3]> {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let len = dx.hypot(dy);
    let (ux, uy) = (dx / len, dy / len);
    let (px, py) = (fx - a.0, fy - a.1);
    let t = (px * ux + py * uy) / len;
    // Across the cartridge: -1 on the side toward the top left, 1 on the other.
    let mut s = (-px * uy + py * ux) / r;
    if uy - ux > 0.0 {
        s = -s;
    }
    if !(0.0..=1.0).contains(&t) {
        return None;
    }
    let case_end = 0.6;
    let half = if t < case_end {
        1.0
    } else {
        let k = (t - case_end) / (1.0 - case_end);
        0.84 * (1.0 - k * k).max(0.0).sqrt()
    };
    if s.abs() > half {
        return None;
    }
    let n = s / half.max(0.05);
    let rim = t < 0.08 || (t > case_end - 0.03 && t < case_end);
    let mut v = 1.02 - 0.3 * n * n + 0.18 * n.min(0.0) - if rim { 0.22 } else { 0.0 };
    if (n + 0.45).abs() < 0.16 && !rim {
        v += 0.3;
    }
    let base = if t < case_end { BRASS } else { COPPER };
    Some(base.map(|c| c * v))
}

/// The gun station: a small steel workbench with a plain steel top.
pub(super) fn gun_station(l: u32, x: i32, y: i32) -> [u8; 4] {
    let (fx, fy) = (x as f32 + 0.5, y as f32 + 0.5);
    let steel = |v: f32| col([150.0, 154.0, 162.0], v, UNTINTED);
    let dark = |v: f32| col([74.0, 78.0, 86.0], v, UNTINTED);
    let rivet = |cx: i32, cy: i32| (x - cx).pow(2) + (y - cy).pow(2) <= 6;
    match l {
        tex::GUN_STATION_TOP => {
            // Bevelled rim with rivets in the corners.
            if !(8.0..120.0).contains(&fx) || !(8.0..120.0).contains(&fy) {
                let v = brushed(l, x, y, 600) * bevel(fx, fy, 0.0, 0.0, 128.0, 128.0, 6.0);
                let r = [(4, 4), (123, 4), (4, 123), (123, 123)]
                    .iter()
                    .any(|&(cx, cy)| rivet(cx, cy));
                return dark(if r { v * 1.35 } else { v });
            }
            // A plain brushed steel plate, a few faint scratches on it.
            let scratch = vn2(l, x, y, 64, 1, 603) > 0.9;
            let v = brushed(l, x, y, 604) * if scratch { 1.05 } else { 1.0 };
            steel(v * 1.08)
        }
        tex::GUN_STATION_BOTTOM => {
            let foot = [(10, 10), (117, 10), (10, 117), (117, 117)]
                .iter()
                .any(|&(cx, cy)| (x - cx).abs() < 9 && (y - cy).abs() < 9);
            if foot {
                return col([30.0, 30.0, 32.0], 0.9 + 0.2 * grain(l, x, y, 605), UNTINTED);
            }
            dark(brushed(l, x, y, 606) * bevel(fx, fy, 0.0, 0.0, 128.0, 128.0, 6.0))
        }
        _ => {
            // Side: thick top, a drawer, legs and a shelf with an ammo box below.
            let leg = x < 14 || x >= 114;
            if y < 16 {
                return steel(brushed(l, x, y, 607) * bevel(fx, fy, 0.0, 0.0, 128.0, 16.0, 3.0));
            }
            if leg {
                let (x0, x1) = if x < 14 { (0.0, 14.0) } else { (114.0, 128.0) };
                let stripe = if (fx - x0 - 4.0).abs() < 1.5 { 1.15 } else { 1.0 };
                return dark(
                    (0.95 + 0.08 * vn2(l, x, y, 2, 32, 608))
                        * bevel(fx, fy, x0, 16.0, x1, 128.0, 2.5)
                        * stripe,
                );
            }
            if y < 20 {
                return col([24.0, 25.0, 28.0], 1.0, UNTINTED);
            }
            if y < 54 {
                // Drawer front with a handle.
                if (48..80).contains(&x) && (33..40).contains(&y) {
                    return col(GUN_STEEL_C, bevel(fx, fy, 48.0, 33.0, 80.0, 40.0, 2.0), UNTINTED);
                }
                if (56..72).contains(&x) && (43..49).contains(&y) {
                    return col([210.0, 206.0, 186.0], 0.95, UNTINTED);
                }
                return dark(1.12 * brushed(l, x, y, 609) * bevel(fx, fy, 16.0, 21.0, 112.0, 53.0, 3.0));
            }
            if (98..106).contains(&y) {
                return dark(1.05 * bevel(fx, fy, 14.0, 98.0, 114.0, 106.0, 2.0));
            }
            // Ammo box on the shelf.
            if (26..66).contains(&x) && (72..98).contains(&y) {
                if (40..52).contains(&x) && (76..82).contains(&y) {
                    return col(GUN_STEEL_C, 0.8, UNTINTED);
                }
                let v = bevel(fx, fy, 26.0, 72.0, 66.0, 98.0, 3.0) * (0.9 + 0.12 * grain(l, x, y, 610));
                return col([84.0, 94.0, 56.0], v, UNTINTED);
            }
            // Dark space under the top, a little lighter toward the shelf.
            let v = 0.8 + 0.25 * ((fy - 54.0) / 60.0).clamp(0.0, 1.0);
            col([30.0, 32.0, 36.0], v * (0.95 + 0.1 * grain(l, x, y, 611)), UNTINTED)
        }
    }
}

/// The pistol model's surfaces: blued steel, bare steel and stippled polymer, with a bevel
/// so the edges of every box read.
pub(super) fn gun_surface(l: u32, x: i32, y: i32) -> [u8; 4] {
    let (fx, fy) = (x as f32 + 0.5, y as f32 + 0.5);
    let edge = bevel(fx, fy, 0.0, 0.0, 128.0, 128.0, 10.0);
    let (c, v) = match l {
        // Lens glass: deep blue with a bright reflection across it.
        tex::GUN_GLASS => {
            let shine = ((fx - fy).abs() < 14.0) as i32 as f32 * 0.5;
            ([60.0, 110.0, 170.0], 0.8 + shine + 0.2 * (1.0 - fy / 128.0))
        }
        // Walnut: dark grain lines running along the stock.
        tex::GUN_WOOD => {
            let grain = vn2(l, x, y, 64, 3, 623);
            let line = ((grain * 9.0).fract() - 0.5).abs() < 0.08;
            ([150.0, 98.0, 54.0], 0.85 + 0.25 * grain - if line { 0.18 } else { 0.0 })
        }
        tex::GUN_BLUED => (GUN_BLUED_C, brushed(l, x, y, 620)),
        tex::GUN_STEEL => (GUN_STEEL_C, brushed(l, x, y, 621)),
        _ => (GUN_POLYMER_C, 0.82 + 0.3 * grain(l, x, y, 622)),
    };
    col(c, v * edge, 255)
}

/// Pistol, part and bullet icons (the pistol faces right).
pub(super) fn gun_icon(l: u32, x: i32, y: i32) -> Option<[u8; 4]> {
    let (fx, fy) = (d(x), d(y));
    let n = grain(l, x, y, 630);
    let slide = |fx: f32, fy: f32| (7.0..27.0).contains(&fx) && (8.5..14.0).contains(&fy);
    let sights = |fx: f32, fy: f32| {
        ((25.0..26.2).contains(&fx) || (8.0..9.6).contains(&fx)) && (7.4..8.6).contains(&fy)
    };
    let muzzle = |fx: f32, fy: f32| (26.5..28.5).contains(&fx) && (9.8..12.4).contains(&fy);
    let rail = |fx: f32, fy: f32| (8.0..25.5).contains(&fx) && (13.5..16.0).contains(&fy);
    // The grip slants back toward the bottom.
    let grip = |fx: f32, fy: f32| {
        in_polygon(fx, fy, &[(8.0, 15.0), (15.0, 15.0), (12.5, 27.5), (5.0, 27.5)])
    };
    let guard = |fx: f32, fy: f32| {
        let d = seg_dist(fx, fy, (15.0, 19.0), (20.5, 19.0))
            .min(seg_dist(fx, fy, (20.5, 15.5), (20.5, 19.0)));
        d < 0.75
    };
    let trigger = |fx: f32, fy: f32| seg_dist(fx, fy, (17.0, 15.5), (16.3, 18.0)) < 0.6;
    let frame = |fx: f32, fy: f32| rail(fx, fy) || grip(fx, fy) || guard(fx, fy);
    let stipple = (x / 4 + y / 4) % 2 == 0;
    let polymer = |fx: f32, fy: f32| {
        if grip(fx, fy) && !rail(fx, fy) {
            GUN_POLYMER_C.map(|c| c * if stipple { 0.8 } else { 1.15 })
        } else {
            GUN_POLYMER_C
        }
    };
    let out = match l {
        // Scope: a tube with wider ends, the mounting rings under it and the lens in front.
        tex::GUN_ATTACHMENTS => {
            let tube = |fx: f32, fy: f32| (6.0..26.0).contains(&fx) && (11.5..17.5).contains(&fy);
            let bells = |fx: f32, fy: f32| {
                ((3.5..8.5).contains(&fx) || (22.5..28.5).contains(&fx)) && (9.5..19.5).contains(&fy)
            };
            let rings = |fx: f32, fy: f32| {
                ((10.0..12.5).contains(&fx) || (19.0..21.5).contains(&fx)) && (17.0..22.5).contains(&fy)
            };
            let inside = |fx: f32, fy: f32| tube(fx, fy) || bells(fx, fy) || rings(fx, fy);
            let c = if (27.0..28.5).contains(&fx) && (10.5..18.5).contains(&fy) {
                [110.0, 170.0, 220.0]
            } else if (13.5..16.5).contains(&fx) && fy < 11.5 {
                GUN_BLUED_C.map(|c| c * 1.5)
            } else {
                // Rounded: lighter along the top of the tube.
                let k = 1.35 - ((fy - 13.0).abs() / 6.0).min(0.5);
                GUN_BLUED_C.map(|c| c * k)
            };
            let turret = (13.5..16.5).contains(&fx) && (9.0..11.5).contains(&fy);
            shape(x, y, c, |fx, fy| inside(fx, fy) || turret)
        }
        // Silencer: a long tube with a threaded end and a few bands.
        l if l == tex::GUN_ATTACHMENTS + 1 => {
            let body = |fx: f32, fy: f32| (7.0..28.0).contains(&fx) && (11.5..20.5).contains(&fy);
            let thread = |fx: f32, fy: f32| (3.0..7.0).contains(&fx) && (13.5..18.5).contains(&fy);
            let c = if thread(fx, fy) {
                GUN_STEEL_C.map(|c| c * if x % 4 < 2 { 0.75 } else { 1.0 })
            } else if (fx - 11.0).abs() < 0.6 || (fx - 24.0).abs() < 0.6 {
                GUN_POLYMER_C.map(|c| c * 0.7)
            } else {
                let k = 1.45 - ((fy - 14.0).abs() / 7.0).min(0.6);
                GUN_POLYMER_C.map(|c| c * k)
            };
            shape(x, y, c, |fx, fy| body(fx, fy) || thread(fx, fy))
        }
        // Extended magazine: a longer magazine with a heavier baseplate.
        l if l == tex::GUN_ATTACHMENTS + 2 => {
            let body = |fx: f32, fy: f32| {
                in_polygon(fx, fy, &[(14.5, 2.5), (20.5, 2.5), (18.0, 25.5), (11.0, 25.5)])
            };
            let plate = |fx: f32, fy: f32| (9.0..20.5).contains(&fx) && (25.5..29.5).contains(&fy);
            let window = (15.5..17.5).contains(&fx) && (6.0..23.0).contains(&fy) && y % 8 < 5;
            let c = if window {
                BRASS.map(|c| c * 0.8)
            } else if plate(fx, fy) {
                [150.0, 40.0, 36.0]
            } else {
                GUN_BLUED_C
            };
            shape(x, y, c, |fx, fy| body(fx, fy) || plate(fx, fy))
        }
        // Laser sight: a small box on a rail clamp, its red lens and beam.
        l if l == tex::GUN_ATTACHMENTS + 3 => {
            if (25.0..31.5).contains(&fx) && (fy - 16.0).abs() < 0.5 {
                return Some([255, 40, 40, 255]);
            }
            let body = |fx: f32, fy: f32| (7.0..23.0).contains(&fx) && (12.0..21.0).contains(&fy);
            let clamp = |fx: f32, fy: f32| (9.0..21.0).contains(&fx) && (9.5..12.0).contains(&fy);
            let lens = |fx: f32, fy: f32| (23.0..25.0).contains(&fx) && (13.5..18.5).contains(&fy);
            let c = if lens(fx, fy) {
                [240.0, 50.0, 40.0]
            } else if (10.0..13.0).contains(&fx) && (15.0..18.0).contains(&fy) {
                GUN_STEEL_C
            } else {
                GUN_POLYMER_C.map(|c| c * 1.3)
            };
            shape(x, y, c, |fx, fy| body(fx, fy) || clamp(fx, fy) || lens(fx, fy))
        }
        tex::PISTOL => {
            let inside = |fx: f32, fy: f32| {
                slide(fx, fy) || sights(fx, fy) || muzzle(fx, fy) || frame(fx, fy) || trigger(fx, fy)
            };
            let c = if slide(fx, fy) || sights(fx, fy) {
                // Grip serrations at the back of the slide, and the ejection port.
                if (9.0..12.5).contains(&fx) && (x % 5 < 2) && fy > 9.5 {
                    GUN_BLUED_C.map(|c| c * 0.6)
                } else if (16.0..21.0).contains(&fx) && fy < 10.5 {
                    [24.0, 24.0, 28.0]
                } else {
                    GUN_BLUED_C.map(|c| c * 1.25)
                }
            } else if muzzle(fx, fy) || trigger(fx, fy) {
                GUN_STEEL_C
            } else {
                polymer(fx, fy)
            };
            shape(x, y, c, inside)
        }
        tex::BULLET => {
            let c = cartridge(fx, fy, (8.0, 24.0), (24.5, 7.5), 3.2)?;
            let inside = |fx: f32, fy: f32| cartridge(fx, fy, (8.0, 24.0), (24.5, 7.5), 3.2).is_some();
            // Keep the cylinder shading, with the outline from `shape`.
            shape(x, y, c, inside)
        }
        _ => match l - tex::PISTOL_PARTS {
            // Frame: rail, grip, trigger guard and trigger.
            0 => {
                let inside = |fx: f32, fy: f32| frame(fx, fy - 3.0) || trigger(fx, fy - 3.0);
                let c = if trigger(fx, fy - 3.0) {
                    GUN_STEEL_C
                } else {
                    polymer(fx, fy - 3.0)
                };
                shape(x, y, c, inside)
            }
            // Barrel: a tube with the chamber block at the back.
            1 => {
                let tube = |fx: f32, fy: f32| (6.0..27.0).contains(&fx) && (14.0..18.0).contains(&fy);
                let hood = |fx: f32, fy: f32| (6.0..12.0).contains(&fx) && (12.5..19.5).contains(&fy);
                let bore = (fx - 26.3).abs() < 0.6 && (15.3..16.7).contains(&fy);
                let c = if bore {
                    [20.0, 20.0, 22.0]
                } else {
                    let band = 1.0 + 0.25 * (1.0 - ((fy - 15.2).abs() / 2.0).min(1.0));
                    GUN_STEEL_C.map(|c| c * band * 0.9)
                };
                shape(x, y, c, |fx, fy| tube(fx, fy) || hood(fx, fy))
            }
            // Recoil spring coiled around its guide rod.
            2 => {
                let rod = |fx: f32, fy: f32| (3.5..28.5).contains(&fx) && (15.0..17.0).contains(&fy);
                // Slanted turns of wire, one every 2.6 units.
                let coil = |fx: f32, fy: f32| {
                    (0..8).any(|i| {
                        let x0 = 6.0 + i as f32 * 2.6;
                        seg_dist(fx, fy, (x0, 20.0), (x0 + 1.8, 12.0)) < 1.0
                    })
                };
                let c = if coil(fx, fy) {
                    GUN_STEEL_C.map(|c| c * 1.05)
                } else {
                    GUN_BLUED_C.map(|c| c * 1.3)
                };
                shape(x, y, c, |fx, fy| rod(fx, fy) || coil(fx, fy))
            }
            // Slide: serrations at the back, the ejection port and the sights.
            3 => {
                let body = |fx: f32, fy: f32| (4.0..28.0).contains(&fx) && (12.0..19.0).contains(&fy);
                let sight = |fx: f32, fy: f32| {
                    ((25.5..27.0).contains(&fx) || (5.0..7.0).contains(&fx)) && (10.8..12.2).contains(&fy)
                };
                let c = if (6.0..10.5).contains(&fx) && x % 5 < 2 && fy > 13.0 {
                    GUN_BLUED_C.map(|c| c * 0.6)
                } else if (14.0..20.0).contains(&fx) && (12.0..14.5).contains(&fy) {
                    [24.0, 24.0, 28.0]
                } else {
                    GUN_BLUED_C.map(|c| c * 1.25)
                };
                shape(x, y, c, |fx, fy| body(fx, fy) || sight(fx, fy))
            }
            // Magazine: a slanted box with a baseplate and a cartridge on top.
            _ => {
                let body = |fx: f32, fy: f32| {
                    in_polygon(fx, fy, &[(14.0, 6.0), (20.0, 6.0), (18.0, 25.0), (11.5, 25.0)])
                };
                let plate = |fx: f32, fy: f32| (10.0..20.0).contains(&fx) && (25.0..28.0).contains(&fy);
                let round = cartridge(fx, fy, (14.5, 5.3), (21.0, 3.8), 1.2);
                if let Some(c) = round {
                    Some(col(c, 1.0, 255))
                } else {
                    let window = (15.0..17.0).contains(&fx) && (10.0..20.0).contains(&fy) && y % 8 < 5;
                    let c = if window {
                        BRASS.map(|c| c * 0.8)
                    } else if plate(fx, fy) {
                        GUN_POLYMER_C.map(|c| c * 1.4)
                    } else {
                        GUN_BLUED_C
                    };
                    shape(x, y, c, |fx, fy| body(fx, fy) || plate(fx, fy))
                }
            }
        },
    };
    out.map(|p| {
        // A little grain so the icons are not flat.
        let v = 0.96 + 0.08 * n;
        [
            (p[0] as f32 * v).min(255.0) as u8,
            (p[1] as f32 * v).min(255.0) as u8,
            (p[2] as f32 * v).min(255.0) as u8,
            p[3],
        ]
    })
}

/// A grenade: an olive, segmented egg with the fuse, lever and pin ring on top; or a smoke
/// grenade, a gray can with a colored band.
pub(super) fn grenade_icon(smoke: bool, x: i32, y: i32) -> Option<[u8; 4]> {
    let (fx, fy) = (d(x), d(y));
    // The fuse head and lever (steel), and the pin's ring beside it.
    let head = (13.0..19.0).contains(&fx) && (5.5..10.0).contains(&fy);
    let lever = (18.5..21.0).contains(&fx) && (7.0..19.0).contains(&fy);
    let ring = ((fx - 10.5).hypot(fy - 7.0) - 2.6).abs() < 0.8;
    if ring {
        return shape(x, y, [190.0, 194.0, 200.0], |fx, fy| ((fx - 10.5).hypot(fy - 7.0) - 2.6).abs() < 0.8);
    }
    if head || lever {
        return shape(x, y, [150.0, 156.0, 166.0], |fx, fy| {
            ((13.0..19.0).contains(&fx) && (5.5..10.0).contains(&fy))
                || ((18.5..21.0).contains(&fx) && (7.0..19.0).contains(&fy))
        });
    }
    if smoke {
        let can = |fx: f32, fy: f32| (10.0..22.0).contains(&fx) && (9.5..28.0).contains(&fy);
        let band = (17.0..20.5).contains(&fy);
        let c = if band { [210.0, 60.0, 50.0] } else { [118.0, 124.0, 128.0] };
        return shape(x, y, c, can);
    }
    let egg = |fx: f32, fy: f32| ((fx - 16.0) / 7.2).powi(2) + ((fy - 18.5) / 9.0).powi(2) < 1.0;
    // Grooves into segments.
    let groove = egg(fx, fy) && ((fx - 16.0).abs() % 4.0 < 0.7 || (fy - 18.5).abs() % 4.5 < 0.7);
    let c = if groove { [58.0, 66.0, 40.0] } else { [98.0, 110.0, 64.0] };
    shape(x, y, c, egg)
}

/// Muzzle flash sprites (drawn glowing, tinted): a star of uneven spikes around a white-hot
/// middle seen from the front; from the side a flame tongue leaving the muzzle at the left.
/// The colour runs from white in the middle to yellow and orange at the edges.
pub(super) fn muzzle_flash(l: u32, x: i32, y: i32) -> [u8; 4] {
    let (u, v) = ((x as f32 + 0.5) / TILE as f32, (y as f32 + 0.5) / TILE as f32);
    // How far out this texel is, 0 in the middle .. 1 at the flame's edge (None: outside).
    let edge = if l == tex::MUZZLE_FLASH {
        let (dx, dy) = (u - 0.5, v - 0.5);
        let r = dx.hypot(dy) * 2.0;
        let a = dy.atan2(dx);
        // Eight spikes of different lengths, and a round core.
        let spikes = (0..8)
            .map(|i| {
                let ang = i as f32 * std::f32::consts::TAU / 8.0 + hash(l, i, 0, 700) * 0.3;
                let len = 0.55 + 0.45 * hash(l, i, 1, 701);
                let d = ((a - ang + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI).abs();
                len * (1.0 - d / 0.28).max(0.0).powf(1.6)
            })
            .fold(0.0f32, f32::max);
        let reach = (0.38 + 0.08 * vn(l, x, y, 16, 702)).max(spikes);
        (r < reach).then(|| r / reach)
    } else {
        // Along the tongue: widest a third of the way out, ragged edges, a pointed tip.
        let along = u;
        let half = 0.42 * (along / 0.3).min(1.0).powf(0.6) * (1.0 - along).max(0.0).powf(0.8);
        let ragged = half * (0.8 + 0.35 * vn(l, x, y, 8, 703));
        let off = (v - 0.5).abs();
        (off < ragged && along > 0.02).then(|| (off / ragged.max(1e-3)).max(along * 0.8))
    };
    let Some(k) = edge else {
        return [0, 0, 0, 0];
    };
    let white = [255.0, 250.0, 230.0];
    let yellow = [255.0, 214.0, 110.0];
    let orange = [255.0, 130.0, 40.0];
    let c = if k < 0.35 {
        lerp3(white, yellow, k / 0.35)
    } else {
        lerp3(yellow, orange, (k - 0.35) / 0.65)
    };
    col(c, 0.92 + 0.08 * grain(l, x, y, 704), 255)
}

/// A bullet hole, multiplied onto a block (mid-gray leaves it as it is): a black hole with
/// a ragged edge, a dark ring of crushed material around it, cracks running out and a faint
/// scorch fading away; clear beyond.
pub(super) fn bullet_hole(l: u32, x: i32, y: i32) -> [u8; 4] {
    let (u, v) = ((x as f32 + 0.5) / TILE as f32 - 0.5, (y as f32 + 0.5) / TILE as f32 - 0.5);
    let r = u.hypot(v) * 2.0;
    let a = v.atan2(u);
    let rough = 0.85 + 0.3 * vn(l, x, y, 8, 710);
    let gray = |g: f32, alpha: f32| col([g, g, g], 1.0, (alpha.clamp(0.0, 1.0) * 255.0) as u8);
    if r < 0.3 * rough {
        return gray(12.0 + 20.0 * grain(l, x, y, 711), 1.0);
    }
    if r < 0.46 * rough {
        return gray(60.0 + 30.0 * grain(l, x, y, 712), 1.0);
    }
    // Six cracks of different lengths.
    let crack = (0..6).any(|i| {
        let ang = i as f32 * std::f32::consts::TAU / 6.0 + hash(l, i, 0, 713) * 0.8;
        let len = 0.7 + 0.28 * hash(l, i, 1, 714);
        let d = ((a - ang + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI).abs();
        let wiggle = 0.05 * (r * 20.0 + i as f32).sin();
        r < len && (d + wiggle).abs() * r < 0.025 * (1.0 - r / len) + 0.006
    });
    if crack {
        return gray(55.0, 1.0);
    }
    // The scorch: a little darker than the block, fading out.
    let scorch = 1.0 - ((r - 0.46) / 0.45).clamp(0.0, 1.0);
    if scorch > 0.0 {
        return gray(95.0, scorch * scorch * 0.9);
    }
    [0, 0, 0, 0]
}
