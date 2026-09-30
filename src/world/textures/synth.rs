//! Layers made from other layers once the procedural and the packs' textures are in: the
//! Blockbench models' pages and their grime, doors, grilled meat, the stump marks, the
//! furnaces' cut fronts and insides, the slot glow, the surface materials, the fluids'
//! frames and the double chest.

use super::procedural::noise::{grime_hash, grime_noise, texel_noise, tiled_value_noise, value_noise};
use super::*;

/// The Blockbench models' texture pages, made by `tools/blockbench/bbmodel_to_rust.py`:
/// 128x128 pages one under the other, from layer `first`.
pub(super) fn synth_model_pages(base: &mut [u8], png: &[u8], pages: u32, first: u32) {
    let Some(img) = crate::pack::decode_png(png) else {
        return;
    };
    let layer_bytes = TILE * TILE * 4;
    for page in 0..pages as usize {
        let dst = (first as usize + page) * layer_bytes;
        for y in 0..TILE {
            let sy = page * TILE + y;
            if sy >= img.h as usize || img.w as usize != TILE {
                return;
            }
            let src = sy * TILE * 4;
            base[dst + y * TILE * 4..dst + (y + 1) * TILE * 4]
                .copy_from_slice(&img.rgba[src..src + TILE * 4]);
        }
    }
}

/// A dirty copy of the pistol's pages (`level` of `tex::PISTOL_DIRT_LEVELS - 1`): carbon and
/// old oil in blotches over everything, heavier the dirtier, the metal duller. The glass stays
/// clear.
pub(super) fn synth_grime(base: &mut [u8], first: u32, pages: u32, level: u32) {
    let layer_bytes = TILE * TILE * 4;
    let amount = level as f32 / (tex::PISTOL_DIRT_LEVELS - 1) as f32;
    // Smooth blotches: value noise on a coarse grid, and a finer one on top.
    let grime = [52.0, 44.0, 34.0];
    for page in 0..pages as usize {
        let src = (first as usize + page) * layer_bytes;
        let dst = (first as usize + (level * pages) as usize + page) * layer_bytes;
        for y in 0..TILE {
            for x in 0..TILE {
                let i = (y * TILE + x) * 4;
                let mut px = [base[src + i], base[src + i + 1], base[src + i + 2], base[src + i + 3]];
                if px[3] == 255 {
                    let (fx, fy) = (x as f32, y as f32 + page as f32 * TILE as f32);
                    let n = grime_noise(fx, fy, 11.0, 1) * 0.65 + grime_noise(fx, fy, 3.0, 2) * 0.35;
                    let speck = grime_hash(x as i32, y as i32 + page as i32 * 1000, 3);
                    let k = ((n - 0.62 + 0.55 * amount) * 1.8).clamp(0.0, 0.85)
                        + if speck < 0.05 * amount { 0.35 } else { 0.0 };
                    let dull = 1.0 - 0.22 * amount;
                    for c in 0..3 {
                        let v = px[c] as f32 * dull;
                        px[c] = (v + (grime[c] - v) * k.min(0.9)).clamp(0.0, 255.0) as u8;
                    }
                }
                base[dst + i..dst + i + 4].copy_from_slice(&px);
            }
        }
    }
}

/// Oak door textures (both halves and the item) made from the final oak planks when no pack
/// has them: vertical boards in a darker frame, a recessed panel with a handle below and a
/// four-pane window above.
pub(super) fn synth_doors(base: &mut [u8]) {
    let layer_bytes = TILE * TILE * 4;
    let layer = |l: u32| l as usize * layer_bytes;
    let empty = |base: &[u8], l: u32| base[layer(l)..layer(l) + layer_bytes].iter().all(|&v| v == 0);
    let planks = base[layer(tex::PLANKS)..layer(tex::PLANKS) + layer_bytes].to_vec();
    // Boards run up and down: the planks texture turned a quarter.
    let wood = |x: usize, y: usize, k: f32| -> [u8; 4] {
        let i = (x * TILE + (TILE - 1 - y)) * 4;
        let c = |v: u8| (v as f32 * k).clamp(0.0, 255.0) as u8;
        [c(planks[i]), c(planks[i + 1]), c(planks[i + 2]), 255]
    };
    // One Minecraft pixel is 8 texels.
    const P: usize = TILE / 16;
    let door_pixel = |x: usize, y: usize, upper: bool| -> [u8; 4] {
        let (mx, my) = (x / P, y / P);
        let frame = mx < 2 || mx >= 14 || (upper && my < 2) || (!upper && my >= 14);
        if frame {
            // Bevel: light on the outer top/left edge, dark at the bottom/right.
            let k = if x < 2 || (upper && y < 2) {
                0.95
            } else if x >= TILE - 2 || (!upper && y >= TILE - 2) {
                0.55
            } else {
                0.78
            };
            return wood(x, y, k);
        }
        if upper {
            // Window: 2x2 panes between 3..13 across and 3..11 down.
            if (3..13).contains(&mx) && (3..11).contains(&my) {
                let mullion = mx == 7 || mx == 8 || my == 6 || my == 7;
                if mullion {
                    return wood(x, y, 0.72);
                }
                let shine = if (x % (5 * P)) + (y % (4 * P)) < 3 * P { 1.25 } else { 1.0 };
                let v = |c: f32| (c * shine).min(255.0) as u8;
                return [v(58.0), v(66.0), v(78.0), 255];
            }
            let rim = (2..14).contains(&mx) && (2..12).contains(&my);
            return wood(x, y, if rim { 0.82 } else { 1.0 });
        }
        // Lower half: a recessed panel, and the handle near the top on the free side (the
        // hinges are on the left, as in Minecraft's texture).
        if (12..14).contains(&mx) && (1..4).contains(&my) {
            let (hx, hy) = (x - 12 * P, y - P);
            let rim = hx < 2 || hy < 2 || hx >= 2 * P - 2 || hy >= 3 * P - 2;
            return if rim {
                [38, 38, 42, 255]
            } else if hx < 5 && hy < 5 {
                [150, 150, 158, 255]
            } else {
                [92, 92, 100, 255]
            };
        }
        let panel = (5..11).contains(&mx) && (4..12).contains(&my);
        let k = if panel {
            if x == 5 * P || y == 4 * P {
                0.6
            } else if x == 11 * P - 1 || y == 12 * P - 1 {
                1.1
            } else {
                0.88
            }
        } else {
            1.0
        };
        wood(x, y, k)
    };
    for (l, upper) in [(tex::DOOR_TOP, true), (tex::DOOR_BOTTOM, false)] {
        if !empty(base, l) {
            continue;
        }
        let o = layer(l);
        for y in 0..TILE {
            for x in 0..TILE {
                let i = o + (y * TILE + x) * 4;
                base[i..i + 4].copy_from_slice(&door_pixel(x, y, upper));
            }
        }
    }
    if empty(base, tex::DOOR_ITEM) {
        // The whole door squeezed into the middle of the sprite, with a dark outline.
        let (top, bottom) = (layer(tex::DOOR_TOP), layer(tex::DOOR_BOTTOM));
        let (x0, x1) = (4 * P, 12 * P);
        let o = layer(tex::DOOR_ITEM);
        for y in 0..TILE {
            for x in x0..x1 {
                let sx = (x - x0) * TILE / (x1 - x0);
                let (src, sy) = if y < TILE / 2 { (top, y * 2) } else { (bottom, y * 2 - TILE) };
                let s = src + (sy * TILE + sx) * 4;
                let mut px = [base[s], base[s + 1], base[s + 2], base[s + 3]];
                if x < x0 + 4 || x >= x1 - 4 || y < 4 || y >= TILE - 4 {
                    px = [px[0] / 3, px[1] / 3, px[2] / 3, 255];
                }
                let i = o + (y * TILE + x) * 4;
                base[i..i + 4].copy_from_slice(&px);
            }
        }
    }
}

/// The stump marks: the dirt (the pack's, if it has one) in a circle as wide as the trunk
/// was, then smaller, its edge a little ragged and darker; the rest see-through.
pub(super) fn synth_stump_marks(base: &mut [u8]) {
    let layer_bytes = TILE * TILE * 4;
    let dirt = base[tex::DIRT as usize * layer_bytes..][..layer_bytes].to_vec();
    for stage in 0..crate::world::STUMP_STAGES as u32 {
        let radius = [0.42, 0.3, 0.18][stage as usize];
        let out = &mut base[(tex::STUMP_MARK + stage) as usize * layer_bytes..][..layer_bytes];
        for y in 0..TILE {
            for x in 0..TILE {
                let dx = (x as f32 + 0.5) / TILE as f32 - 0.5;
                let dy = (y as f32 + 0.5) / TILE as f32 - 0.5;
                let d = (dx * dx + dy * dy).sqrt();
                // Grass creeping in: the edge in and out by a texel or so (of 16).
                let px = TILE as i32 / 16;
                let ragged = (procedural::hash(77 + stage, x as i32 / px, y as i32 / px, 5) - 0.5) * 0.07;
                let i = (y * TILE + x) * 4;
                if d + ragged > radius {
                    out[i..i + 4].copy_from_slice(&[0; 4]);
                    continue;
                }
                let rim = if d + ragged > radius - 0.05 { 0.8 } else { 1.0 };
                for k in 0..3 {
                    out[i + k] = (dirt[i + k] as f32 * rim) as u8;
                }
                out[i + 3] = UNTINTED;
            }
        }
    }
}

fn luma(c: &[u8]) -> f32 {
    (c[0] as f32 * 0.3 + c[1] as f32 * 0.59 + c[2] as f32 * 0.11) / 255.0
}

/// `b` with its upper right half recoloured to look like `a`: `b`'s shape and drawing are
/// kept (so the two sides always fit), each pixel's brightness is mapped onto `a`'s colours,
/// and the two sides meet in a ragged, dithered band across the meat.
fn half_over(a: &[u8], b: &[u8]) -> Vec<u8> {
    // `a`'s colours by brightness: opaque pixels sorted by luma, averaged into buckets.
    const BUCKETS: usize = 16;
    let ramp_of = |img: &[u8]| {
        let mut px: Vec<[f32; 4]> = img
            .chunks_exact(4)
            .filter(|c| c[3] > 127)
            .map(|c| [luma(c), c[0] as f32, c[1] as f32, c[2] as f32])
            .collect();
        px.sort_by(|p, q| p[0].total_cmp(&q[0]));
        (0..BUCKETS)
            .map(|k| {
                let s = &px[k * px.len() / BUCKETS..((k + 1) * px.len() / BUCKETS).max(k * px.len() / BUCKETS + 1)];
                let n = s.len() as f32;
                [0, 1, 2, 3].map(|c| s.iter().map(|p| p[c]).sum::<f32>() / n)
            })
            .collect::<Vec<_>>()
    };
    if a.chunks_exact(4).all(|c| c[3] <= 127) || b.chunks_exact(4).all(|c| c[3] <= 127) {
        return b.to_vec();
    }
    let (ra, rb) = (ramp_of(a), ramp_of(b));
    // Where a brightness of `b` falls in its own range, as a position on `a`'s ramp.
    let recolor = |c: &[u8]| -> [u8; 3] {
        let l = luma(c);
        let k = rb.iter().rposition(|r| r[0] <= l).unwrap_or(0);
        let f = if k + 1 < BUCKETS && rb[k + 1][0] > rb[k][0] {
            ((l - rb[k][0]) / (rb[k + 1][0] - rb[k][0])).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let (p, q) = (ra[k], ra[(k + 1).min(BUCKETS - 1)]);
        [1, 2, 3].map(|ch| (p[ch] + (q[ch] - p[ch]) * f).round().clamp(0.0, 255.0) as u8)
    };
    let mut out = b.to_vec();
    for y in 0..TILE {
        for x in 0..TILE {
            let i = (y * TILE + x) * 4;
            if b[i + 3] <= 127 {
                continue;
            }
            // Across the meat (which lies from the lower left to the upper right), wavy.
            let d = (x as f32 - y as f32) / TILE as f32
                + (value_noise(x, y, 20, 11) - 0.5) * 0.45
                + (value_noise(x, y, 6, 12) - 0.5) * 0.1;
            let t = ((d + 0.025) / 0.05).clamp(0.0, 1.0);
            // Dithered: pixels flip one by one through the band, no straight line.
            if t > texel_noise(x, y, 13) {
                let c = recolor(&b[i..i + 4]);
                // Seared a little darker right at the edge of the done side.
                let edge = 1.0 - 0.3 * (1.0 - d / 0.07).clamp(0.0, 1.0);
                for ch in 0..3 {
                    out[i + ch] = (c[ch] as f32 * edge) as u8;
                }
            }
        }
    }
    out
}

/// Charred: cooked meat blackened almost all over, with a few glowing embers.
fn char_meat(roasted: &[u8]) -> Vec<u8> {
    let mut out = roasted.to_vec();
    for y in 0..TILE {
        for x in 0..TILE {
            let i = (y * TILE + x) * 4;
            let noise = texel_noise(x, y, 3);
            let c = &roasted[i..i + 4];
            let k = 0.16 + 0.22 * luma(c) + 0.06 * noise;
            out[i] = (c[0] as f32 * k + 10.0) as u8;
            out[i + 1] = (c[1] as f32 * k * 0.8 + 6.0) as u8;
            out[i + 2] = (c[2] as f32 * k * 0.7 + 4.0) as u8;
            if noise > 0.985 && c[3] > 127 {
                out[i..i + 3].copy_from_slice(&[168, 58, 18]);
            }
        }
    }
    out
}

/// Grilled meat, made from the final raw and cooked textures (unless a pack has them): burnt
/// (the cooked meat charred), and the pieces with two different sides, shown as the one over
/// the upper right half of the other: one side cooked (cooked over raw), one side burnt
/// (burnt over cooked), and burnt on one side, raw on the other (burnt over raw). On the
/// grill each side shows the raw, cooked or burnt meat's own texture (same shape).
pub(super) fn synth_grilled(base: &mut [u8]) {
    let layer_bytes = TILE * TILE * 4;
    let get = |base: &[u8], l: u32| base[l as usize * layer_bytes..][..layer_bytes].to_vec();
    let empty = |base: &[u8], l: u32| get(base, l).iter().all(|&v| v == 0);
    let put = |base: &mut [u8], l: u32, px: &[u8]| {
        if empty(base, l) {
            base[l as usize * layer_bytes..][..layer_bytes].copy_from_slice(px);
        }
    };
    for (raw, cooked, half, half_burnt, raw_burnt, burnt) in [
        (
            tex::PORKCHOP,
            tex::COOKED_PORKCHOP,
            tex::HALF_COOKED_PORKCHOP,
            tex::HALF_BURNT_PORKCHOP,
            tex::RAW_BURNT_PORKCHOP,
            tex::BURNT_PORKCHOP,
        ),
        (
            tex::MUTTON,
            tex::COOKED_MUTTON,
            tex::HALF_COOKED_MUTTON,
            tex::HALF_BURNT_MUTTON,
            tex::RAW_BURNT_MUTTON,
            tex::BURNT_MUTTON,
        ),
    ] {
        let (r, c) = (get(base, raw), get(base, cooked));
        put(base, burnt, &char_meat(&c));
        let b = get(base, burnt);
        put(base, half, &half_over(&c, &r));
        put(base, half_burnt, &half_over(&b, &c));
        put(base, raw_burnt, &half_over(&b, &r));
    }
}

/// The highlight of a slot or spot: a soft rounded patch that slightly brightens what is under
/// it (drawn multiplied: mid-gray changes nothing), with a faintly brighter rim. Clear outside.
pub(super) fn synth_glow(base: &mut [u8]) {
    let o = tex::SLOT_GLOW as usize * TILE * TILE * 4;
    let t = TILE as f32;
    let radius = t * 0.2;
    let feather = t * 0.08;
    for y in 0..TILE {
        for x in 0..TILE {
            let p = [x as f32 + 0.5, y as f32 + 0.5];
            let q = p.map(|v| (v - t * 0.5).abs() - (t * 0.5 - radius));
            let outside =
                (q[0].max(0.0).powi(2) + q[1].max(0.0).powi(2)).sqrt() + q[0].max(q[1]).min(0.0);
            // Distance inward from the rounded edge.
            let depth = radius - outside;
            let i = o + (y * TILE + x) * 4;
            if depth <= 0.0 {
                continue;
            }
            let fade = (depth / feather).min(1.0);
            let rim = (1.0 - ((depth - feather * 0.9) / (t * 0.05)).abs()).clamp(0.0, 1.0);
            let v = (150.0 + 30.0 * rim) as u8;
            let a = (26.0 + 229.0 * fade) as u8;
            base[i..i + 4].copy_from_slice(&[v, v, v, a]);
        }
    }
}

/// A furnace front (`front`) with its two openings cut out (the mouth above, the firebox
/// below) into `cut`, for the furnace model with real hollows behind them: in each row,
/// everything between the dark outline of an opening is made clear.
pub(super) fn synth_furnace_cut(base: &mut [u8], front: u32, cut: u32) {
    let layer_bytes = TILE * TILE * 4;
    let src = base[front as usize * layer_bytes..][..layer_bytes].to_vec();
    let o = cut as usize * layer_bytes;
    let dark = |x: usize, y: usize| {
        let i = (y * TILE + x) * 4;
        (src[i] as u32 + src[i + 1] as u32 + src[i + 2] as u32) < 60
    };
    for y in 0..TILE {
        let row = &mut base[o + y * TILE * 4..][..TILE * 4];
        row.copy_from_slice(&src[y * TILE * 4..][..TILE * 4]);
        for px in row.chunks_exact_mut(4) {
            px[3] = 255;
        }
        if !(TILE / 10..TILE - 2).contains(&y) {
            continue;
        }
        let first = (TILE / 16..TILE - TILE / 16).find(|&x| dark(x, y));
        let last = (TILE / 16..TILE - TILE / 16).rev().find(|&x| dark(x, y));
        if let (Some(a), Some(b)) = (first, last) {
            if b - a >= TILE / 8 {
                for x in a..=b {
                    row[x * 4 + 3] = 0;
                }
            }
        }
    }
}

/// Inside a furnace: charred, sooty black all over, with soft smudges of soot and ash and
/// a fine grain (smooth like the other textures, not blocky).
pub(super) fn synth_furnace_inside(base: &mut [u8]) {
    let o = tex::FURNACE_INSIDE as usize * TILE * TILE * 4;
    for y in 0..TILE {
        for x in 0..TILE {
            let smudge = tiled_value_noise(x, y, TILE / 4, 11) * 0.65 + tiled_value_noise(x, y, TILE / 16, 13) * 0.35;
            let grain = texel_noise(x, y, 12);
            let v = 22.0 + 28.0 * smudge * smudge + 6.0 * grain;
            let i = o + (y * TILE + x) * 4;
            base[i..i + 4].copy_from_slice(&[(v * 1.06) as u8, v as u8, (v * 0.94) as u8, 255]);
        }
    }
}

/// Surface material, stored in the alpha of fully opaque block textures (which is 255
/// otherwise): alpha = 255 - code, code = pit | metal << 1 | shine << 2 (0..63). The world
/// shader reads it back (base mip level) for sunk ore nuggets and shine.
#[derive(Clone, Copy, Default, PartialEq)]
struct Material {
    /// Sunk into the block (ore nuggets, drawn deeper with parallax).
    pit: bool,
    /// Highlights in the surface's own colour (else white, like a gem or polished stone).
    metal: bool,
    /// 0 matte .. 15 mirror-like.
    shine: u8,
}

impl Material {
    const fn new(metal: bool, shine: u8) -> Self {
        Self { pit: false, metal, shine }
    }

    fn alpha(self) -> u8 {
        255 - (self.pit as u8 | (self.metal as u8) << 1 | self.shine.min(15) << 2)
    }
}

/// Ore nuggets set into the stone, shiny: every texel of an ore whose colour is not one of the
/// stone texture's own colours (or the stone's at that spot) is a nugget. Lone specks are left
/// out and pinholes filled. An ore gets no nuggets if that does not give sensible ones (a pack
/// whose ore is not drawn over its stone). Metal and gem blocks shine all over.
pub(super) fn mark_materials(base: &mut [u8]) {
    let layer_bytes = TILE * TILE * 4;
    for (layer, m) in [
        (tex::IRON_BLOCK, Material::new(true, 10)),
        (tex::COPPER_BLOCK, Material::new(true, 10)),
        (tex::GOLD_BLOCK, Material::new(true, 13)),
        (tex::DIAMOND_BLOCK, Material::new(false, 14)),
        (tex::COAL_BLOCK, Material::new(false, 5)),
        (tex::OBSIDIAN, Material::new(false, 9)),
    ] {
        let px = &mut base[layer as usize * layer_bytes..][..layer_bytes];
        if px.chunks_exact(4).all(|c| c[3] == 255) {
            for c in px.chunks_exact_mut(4) {
                c[3] = m.alpha();
            }
        }
    }

    let stone = base[tex::STONE as usize * layer_bytes..][..layer_bytes].to_vec();
    let mut palette: Vec<[u8; 3]> = Vec::new();
    for c in stone.chunks_exact(4) {
        let c = [c[0], c[1], c[2]];
        if !palette.contains(&c) {
            palette.push(c);
            if palette.len() > 64 {
                break;
            }
        }
    }
    let near = |a: &[u8], b: &[u8]| (0..3).map(|i| (a[i] as i32 - b[i] as i32).abs()).sum::<i32>() <= 12;
    for (layer, nugget) in [
        (tex::COAL_ORE, Material::new(false, 4)),
        (tex::IRON_ORE, Material::new(true, 9)),
        (tex::COPPER_ORE, Material::new(true, 10)),
        (tex::GOLD_ORE, Material::new(true, 13)),
        (tex::DIAMOND_ORE, Material::new(false, 15)),
    ] {
        let ore = &mut base[layer as usize * layer_bytes..][..layer_bytes];
        if !ore.chunks_exact(4).all(|c| c[3] == 255) {
            continue;
        }
        let mut pit: Vec<bool> = (0..TILE * TILE)
            .map(|i| {
                let c = &ore[i * 4..i * 4 + 4];
                !(near(c, &stone[i * 4..]) || (palette.len() <= 64 && palette.iter().any(|p| near(c, p))))
            })
            .collect();
        for _ in 0..2 {
            let prev = pit.clone();
            let count = |x: usize, y: usize| {
                [(1, 0), (TILE - 1, 0), (0, 1), (0, TILE - 1)]
                    .iter()
                    .filter(|d| prev[(y + d.1) % TILE * TILE + (x + d.0) % TILE])
                    .count()
            };
            for i in 0..TILE * TILE {
                let n = count(i % TILE, i / TILE);
                if prev[i] && n <= 1 {
                    pit[i] = false;
                } else if !prev[i] && n >= 3 {
                    pit[i] = true;
                }
            }
        }
        let share = pit.iter().filter(|&&p| p).count() as f32 / (TILE * TILE) as f32;
        if !(0.01..=0.6).contains(&share) {
            continue;
        }
        for (i, &p) in pit.iter().enumerate() {
            let m = if p { Material { pit: true, ..nugget } } else { Material::default() };
            ore[i * 4 + 3] = m.alpha();
        }
    }
}

/// Fluids without animation frames: every frame is the still texture (it still scrolls).
pub(super) fn synth_fluid_frames(base: &mut [u8]) {
    let layer_bytes = TILE * TILE * 4;
    for (still, anim) in [(tex::WATER, tex::WATER_ANIM), (tex::LAVA, tex::LAVA_ANIM)] {
        let src = still as usize * layer_bytes;
        for f in 0..tex::FLUID_FRAMES {
            let dst = (anim + f) as usize * layer_bytes;
            if base[dst..dst + layer_bytes].iter().all(|&v| v == 0) {
                base.copy_within(src..src + layer_bytes, dst);
            }
        }
    }
}

/// Double chest faces. Without the pack's double chest textures, the half of the single
/// chest texture toward the open edge is stretched over the frame there. The top and
/// bottom edges are the left and right ones transposed.
pub(super) fn synth_double_chest(base: &mut [u8]) {
    let layer_bytes = TILE * TILE * 4;
    let half = TILE / 2;
    let stretch = |a: usize| if a < half { a } else { half + (a - half) / 2 };
    let mirror = |a: usize| TILE - 1 - a;
    for (i, src) in [
        tex::CHEST_FRONT,
        tex::CHEST_SIDE,
        tex::CHEST_TOP,
        tex::CHEST_INSIDE,
    ]
    .into_iter()
    .enumerate()
    {
        let src = src as usize * layer_bytes;
        let open = |edge: usize| (tex::CHEST_OPEN as usize + i * 4 + edge) * layer_bytes;
        for edge in 0..2 {
            let dst = open(edge);
            if base[dst..dst + layer_bytes].iter().any(|&v| v != 0) {
                continue; // from the pack
            }
            for y in 0..TILE {
                for x in 0..TILE {
                    let sx = if edge == 0 {
                        stretch(x)
                    } else {
                        mirror(stretch(mirror(x)))
                    };
                    let (s, d) = (src + (y * TILE + sx) * 4, dst + (y * TILE + x) * 4);
                    base.copy_within(s..s + 4, d);
                }
            }
        }
        for (edge, from) in [(2, 1), (3, 0)] {
            let (src, dst) = (open(from), open(edge));
            for y in 0..TILE {
                for x in 0..TILE {
                    let (s, d) = (src + (x * TILE + y) * 4, dst + (y * TILE + x) * 4);
                    base.copy_within(s..s + 4, d);
                }
            }
        }
    }
}
