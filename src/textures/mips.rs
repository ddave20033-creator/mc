//! The mipmaps: which layers are cut out (alpha tested) and how the levels are averaged.

use super::*;

pub(super) fn is_item_icon(l: u32) -> bool {
    (tex::STICK..tex::CHEST_INSIDE).contains(&l)
        || (tex::PIG_SPAWN_EGG..=tex::IRON_NUGGET).contains(&l)
        || l == tex::DOOR_ITEM
        || l == tex::BED_ITEM
        || (tex::MUTTON..=tex::SHEEP_SPAWN_EGG).contains(&l)
        || (tex::HALF_COOKED_PORKCHOP..=tex::BURNT_MUTTON).contains(&l)
        || (tex::HALF_BURNT_PORKCHOP..=tex::RAW_BURNT_MUTTON).contains(&l)
        || (tex::PISTOL..tex::GUN_GLASS).contains(&l)
        || l == tex::COPPER_INGOT
        || (tex::STEEL_INGOT..=tex::TARGET_DUMMY).contains(&l)
        || (tex::ARMOR_ICONS..tex::ARMOR_ICONS + 17).contains(&l)
        || l == tex::BOOK
        || (tex::MORE_TOOLS..tex::MORE_TOOLS + 4).contains(&l)
        || l == tex::BONE
        || l == tex::WOLF_SPAWN_EGG
        || (tex::FISHING_ROD..=tex::COOKED_FISH).contains(&l)
}

fn is_crack(l: u32) -> bool {
    (tex::CRACK..tex::CRACK + 10).contains(&l)
}

const CUTOUT: &[u32] = &[
    tex::OAK_LEAVES,
    tex::SPRUCE_LEAVES,
    tex::BIRCH_LEAVES,
    tex::GLASS,
    tex::TALL_GRASS,
    tex::POPPY,
    tex::DANDELION,
    tex::DEAD_BUSH,
    tex::OAK_SAPLING,
    tex::BIRCH_SAPLING,
    tex::SPRUCE_SAPLING,
    tex::TORCH,
];

/// Keep the fraction of leaf gaps steady as a texture is minified. Otherwise alpha
/// testing can turn a canopy from see-through up close into a solid wall at distance.
fn preserve_leaf_coverage(base: &[u8], next: &mut [u8], size: usize, layer: usize) {
    let stride = TILE / size;
    let base_start = layer * TILE * TILE * 4;
    let next_start = layer * size * size * 4;
    let base_holes = (0..TILE * TILE)
        .filter(|&i| base[base_start + i * 4 + 3] < 128)
        .count();
    let target = (base_holes * size * size + TILE * TILE / 2) / (TILE * TILE);
    let mut coverage = Vec::with_capacity(size * size);
    for y in 0..size {
        for x in 0..size {
            let mut opaque = 0u32;
            for sy in y * stride..(y + 1) * stride {
                for sx in x * stride..(x + 1) * stride {
                    opaque += (base[base_start + (sy * TILE + sx) * 4 + 3] >= 128) as u32;
                }
            }
            // Stable spatial tie-breaker for equal coverage at the smallest mips.
            let tie = ((x * 73) ^ (y * 151) ^ (layer * 31)) as u32;
            coverage.push((opaque, tie, y * size + x));
        }
    }
    coverage.sort_unstable();
    for (rank, &(_, _, i)) in coverage.iter().enumerate() {
        next[next_start + i * 4 + 3] = if rank < target { 0 } else { 255 };
    }
}

pub(super) fn is_cutout(l: u32) -> bool {
    CUTOUT.contains(&l)
        || (tex::STUMP_MARK..tex::STUMP_MARK + crate::world::STUMP_STAGES as u32).contains(&l)
        || (tex::LOGO..tex::LOGO + tex::LOGO_TILES).contains(&l)
        || is_crack(l)
        || is_item_icon(l)
        || l == tex::LANTERN
        || l == tex::CHAIN
        || l == tex::DOOR_TOP
        || l == tex::DOOR_BOTTOM
        || (tex::BED_HEAD_EAST..=tex::BED_FOOT_END).contains(&l)
        || l == tex::FLAME_PARTICLE
        || l == tex::MUZZLE_FLASH
        || l == tex::MUZZLE_FLASH_SIDE
        || l == tex::BULLET_HOLE
        || l == tex::SLOT_GLOW
        || l == tex::FURNACE_FRONT_CUT
        || l == tex::BLAST_FRONT_CUT
        || l == tex::ADV_FRONT_CUT
        || (tex::SMOKE..tex::SMOKE + SMOKE_FRAMES).contains(&l)
        || l == tex::CLOUD
        // (the wolf's collar: only its band shows)
        || (tex::WOLF_COLLAR..tex::BONE).contains(&l)
}

/// The mip levels of all the layers (full size and back to back in `base`), down to 1x1.
pub(super) fn mip_chain(base: Vec<u8>) -> Vec<Vec<u8>> {
    // Mipmaps average in linear light; the sRGB decode of every byte value is looked up.
    let linear: [f32; 256] = std::array::from_fn(|v| (v as f32 / 255.0).powf(2.2));
    let layers = base.len() / (TILE * TILE * 4);
    let mut levels = vec![base];
    let mut size = TILE;
    while size > 1 {
        let prev = levels.last().unwrap();
        let ns = size / 2;
        let mut next = vec![0u8; ns * ns * 4 * layers];
        for l in 0..layers {
            let id = l as u32;
            let cutout = is_cutout(id);
            for y in 0..ns {
                for x in 0..ns {
                    let mut rgb = [0.0f32; 3];
                    let mut wsum = 0.0;
                    let mut asum = 0.0;
                    for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                        let i = ((l * size + y * 2 + dy) * size + x * 2 + dx) * 4;
                        let a = prev[i + 3] as f32 / 255.0;
                        let w = if cutout { a } else { 1.0 };
                        for c in 0..3 {
                            rgb[c] += linear[prev[i + c] as usize] * w;
                        }
                        wsum += w;
                        asum += a;
                    }
                    let mut a = asum / 4.0;
                    let logo = (tex::LOGO..tex::LOGO + tex::LOGO_TILES).contains(&id);
                    if cutout && id != tex::GLASS && !is_crack(id) && !logo {
                        a = if a > 0.3 { 1.0 } else { 0.0 };
                    } else if id == tex::GLASS {
                        a = if a > 0.45 { 1.0 } else { 0.0 };
                    }
                    let o = ((l * ns + y) * ns + x) * 4;
                    for c in 0..3 {
                        let v = if wsum > 0.0 { rgb[c] / wsum } else { 0.0 };
                        next[o + c] = (v.powf(1.0 / 2.2) * 255.0).round() as u8;
                    }
                    // Keep the tint masks exact instead of averaging them.
                    let first_alpha = prev[((l * size + y * 2) * size + x * 2) * 4 + 3];
                    next[o + 3] = if !cutout && id != tex::GRASS_SIDE && !is_crack(id) {
                        first_alpha
                    } else {
                        (a * 255.0).round() as u8
                    };
                }
            }
        }
        for layer in [tex::OAK_LEAVES, tex::SPRUCE_LEAVES, tex::BIRCH_LEAVES] {
            if (layer as usize) < layers {
                preserve_leaf_coverage(&levels[0], &mut next, ns, layer as usize);
            }
        }
        levels.push(next);
        size = ns;
    }
    levels
}
