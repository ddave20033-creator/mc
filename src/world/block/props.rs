//! Block properties: what hides faces, blocks movement, stops sunlight or glows, and
//! the texture of each face.

use super::*;
use crate::world::textures::tex;

#[inline]
pub const fn is_water(b: u8) -> bool {
    b >= WATER && b <= WATER + FALLING
}
#[inline]
pub const fn is_lava(b: u8) -> bool {
    b >= LAVA && b <= LAVA + FALLING
}
#[inline]
pub const fn is_fluid(b: u8) -> bool {
    is_water(b) || is_lava(b)
}
#[inline]
pub fn fluid_level(b: u8) -> u8 {
    if is_water(b) {
        b - WATER
    } else {
        b - LAVA
    }
}
#[inline]
pub const fn is_leaves(b: u8) -> bool {
    matches!(b, OAK_LEAVES | SPRUCE_LEAVES | BIRCH_LEAVES)
}
#[inline]
pub const fn is_sapling(b: u8) -> bool {
    matches!(b, OAK_SAPLING | BIRCH_SAPLING | SPRUCE_SAPLING)
}
#[inline]
pub const fn is_torch(b: u8) -> bool {
    b == TORCH || (b >= WALL_TORCH && b < WALL_TORCH + 4)
}

#[inline]
pub const fn is_lantern(b: u8) -> bool {
    b == LANTERN || b == LANTERN_HANGING
}

/// Cross-shaped decorations (rendered as two crossed quads, walk-through).
#[inline]
pub const fn is_plant(b: u8) -> bool {
    matches!(b, TALL_GRASS | POPPY | DANDELION | DEAD_BUSH) || is_sapling(b)
}
/// A furnace of any kind (the block with the openings, not the other parts of a big one).
#[inline]
pub fn is_furnace(b: u8) -> bool {
    (FURNACE..FURNACE_LIT + 4).contains(&b)
        || (BLAST_FURNACE..BLAST_FURNACE_LIT + 4).contains(&b)
        || (ADV_FURNACE..ADV_FURNACE_LIT + 4).contains(&b)
}
#[inline]
pub const fn is_chimney(b: u8) -> bool {
    b >= CHIMNEY && b < CHIMNEY + 4
}
#[inline]
pub fn is_adv_part(b: u8) -> bool {
    (ADV_PART..ADV_PART_LIT + 12).contains(&b)
}
/// The kind of furnace (FURNACE, BLAST_FURNACE or ADV_FURNACE) a block belongs to, its
/// other parts included.
pub fn furnace_base(b: u8) -> Option<u8> {
    match b {
        _ if (FURNACE..FURNACE_LIT + 4).contains(&b) => Some(FURNACE),
        _ if (BLAST_FURNACE..BLAST_FURNACE_LIT + 4).contains(&b) || is_chimney(b) => {
            Some(BLAST_FURNACE)
        }
        _ if (ADV_FURNACE..ADV_FURNACE_LIT + 4).contains(&b) || is_adv_part(b) => {
            Some(ADV_FURNACE)
        }
        _ => None,
    }
}
/// What a furnace can smelt (`item::smelt_tier`): 1 the furnace, 2 the blast furnace, 3 the
/// advanced furnace.
pub fn furnace_tier(b: u8) -> u8 {
    match furnace_base(b) {
        Some(BLAST_FURNACE) => 2,
        Some(ADV_FURNACE) => 3,
        _ => 1,
    }
}
/// A burning furnace.
pub fn is_lit_furnace(b: u8) -> bool {
    is_furnace(b) && furnace_base(b).is_some_and(|base| b - base >= 4)
}

/// The front of a furnace with its openings cut out (the model has hollows behind them).
pub fn furnace_front_cut(b: u8) -> u32 {
    match furnace_base(b) {
        Some(BLAST_FURNACE) => tex::BLAST_FRONT_CUT,
        Some(ADV_FURNACE) => tex::ADV_FRONT_CUT,
        _ => tex::FURNACE_FRONT_CUT,
    }
}
#[inline]
pub const fn is_chest(b: u8) -> bool {
    (b >= CHEST && b < CHEST + 4) || (b >= CHEST_LEFT && b < CHEST_RIGHT + 4)
}

#[inline]
pub fn is_stump_mark(b: u8) -> bool {
    (STUMP_MARK..STUMP_MARK + 2 * STUMP_STAGES).contains(&b)
}
/// The mark of a cut-down trunk at `stage` (0 the whole trunk's width) on `grass`.
pub fn stump_mark(grass: u8, stage: u8) -> u8 {
    STUMP_MARK + stage + if grass == SNOWY_GRASS { STUMP_STAGES } else { 0 }
}
pub fn stump_stage(b: u8) -> u8 {
    (b - STUMP_MARK) % STUMP_STAGES
}
/// The block as the world's rules see it: a stump mark is the grass it is on.
pub fn soil(b: u8) -> u8 {
    if !is_stump_mark(b) {
        b
    } else if b - STUMP_MARK < STUMP_STAGES {
        GRASS
    } else {
        SNOWY_GRASS
    }
}

/// Needs a solid block below it (breaks otherwise).
pub fn needs_support(b: u8) -> bool {
    is_plant(b) || is_torch(b) || is_lantern(b) || b == CACTUS || is_door(b)
}
/// Falls like sand when unsupported.
pub fn has_gravity(b: u8) -> bool {
    matches!(b, SAND | GRAVEL)
}
/// Fully hides neighbouring faces and blocks light.
#[inline]
pub fn is_opaque(b: u8) -> bool {
    PROPS[b as usize] & OPAQUE != 0
}
/// Blocks player movement.
#[inline]
pub fn is_solid(b: u8) -> bool {
    PROPS[b as usize] & SOLID != 0
}
/// Can be overwritten by placing a block or by flowing fluid.
#[inline]
pub fn is_replaceable(b: u8) -> bool {
    b == AIR || b == TALL_GRASS || is_fluid(b)
}
/// Flowing fluid washes these away.
#[inline]
pub fn fluid_breaks(b: u8) -> bool {
    is_plant(b) || is_torch(b)
}
/// Stops full-strength sunlight (used for the heightmap).
#[inline]
pub fn attenuates_sky(b: u8) -> bool {
    PROPS[b as usize] & ATTENUATES_SKY != 0
}

// `is_opaque`, `is_solid` and `attenuates_sky` are asked for every block and its neighbours
// in the mesher's and the lighting's inner loops: they are looked up in a table, made from
// the rules below when compiling.
const OPAQUE: u8 = 1;
const SOLID: u8 = 2;
const ATTENUATES_SKY: u8 = 4;
static PROPS: [u8; 256] = {
    let mut t = [0u8; 256];
    let mut i = 0;
    while i < 256 {
        let b = i as u8;
        t[i] = if opaque_rule(b) { OPAQUE } else { 0 }
            | if solid_rule(b) { SOLID } else { 0 }
            | if attenuates_sky_rule(b) { ATTENUATES_SKY } else { 0 };
        i += 1;
    }
    t
};
const fn opaque_rule(b: u8) -> bool {
    !(b == AIR
        || b == GLASS
        || is_torch(b)
        || is_lantern(b)
        || is_leaves(b)
        || is_plant(b)
        || is_fluid(b)
        || is_chest(b)
        || is_door(b)
        || is_stairs(b)
        || is_bed(b)
        || is_chimney(b)
        || is_gun_bench(b)
        || is_log(b))
}
const fn solid_rule(b: u8) -> bool {
    !(b == AIR || is_torch(b) || is_lantern(b) || is_plant(b) || is_fluid(b))
}
const fn attenuates_sky_rule(b: u8) -> bool {
    !(b == AIR || b == GLASS || is_torch(b) || is_lantern(b) || is_plant(b) || is_door(b))
}
#[inline]
pub fn emission(b: u8) -> u8 {
    match b {
        _ if is_lava(b) => 15,
        GLOWSTONE => 15,
        _ if is_torch(b) => 14,
        _ if is_lantern(b) => 15,
        _ if is_lit_furnace(b) => 13,
        _ => 0,
    }
}

/// Axis (0 x, 1 y, 2 z) of each face's normal.
const FACE_AXIS: [usize; 6] = [0, 0, 1, 1, 2, 2];

/// Texture array layer for a block face. Faces: 0 +X, 1 -X, 2 +Y, 3 -Y, 4 +Z, 5 -Z.
pub fn face_texture(b: u8, face: usize) -> u32 {
    if is_stump_mark(b) {
        return face_texture(soil(b), face);
    }
    let top = face == 2;
    let bottom = face == 3;
    let ends = top || bottom;
    if log_axis(b) != 1 || is_branch(b) {
        let end = FACE_AXIS[face] == log_axis(b);
        return face_texture(log_base(b), if end { 2 } else { 0 });
    }
    match b {
        _ if is_door(b) => {
            if door_upper(b) {
                tex::DOOR_TOP
            } else {
                tex::DOOR_BOTTOM
            }
        }
        _ if is_stairs(b) => tex::PLANKS,
        // The bed is meshed on its own; this is for particles.
        _ if is_bed(b) => {
            if bottom {
                tex::BED_BOTTOM
            } else if bed_head(b) {
                tex::BED_HEAD_TOP
            } else {
                tex::BED_FOOT_TOP
            }
        }
        GRASS => {
            if top {
                tex::GRASS_TOP
            } else if bottom {
                tex::DIRT
            } else {
                tex::GRASS_SIDE
            }
        }
        SNOWY_GRASS => {
            if top {
                tex::SNOW
            } else if bottom {
                tex::DIRT
            } else {
                tex::SNOWY_GRASS_SIDE
            }
        }
        DIRT => tex::DIRT,
        STONE => tex::STONE,
        SAND => tex::SAND,
        OAK_LOG => {
            if ends {
                tex::OAK_LOG_TOP
            } else {
                tex::OAK_LOG
            }
        }
        OAK_LEAVES => tex::OAK_LEAVES,
        SNOW => tex::SNOW,
        PLANKS => tex::PLANKS,
        COBBLE => tex::COBBLE,
        BEDROCK => tex::BEDROCK,
        GLASS => tex::GLASS,
        BRICKS => tex::BRICKS,
        GRAVEL => tex::GRAVEL,
        SANDSTONE => {
            if ends {
                tex::SANDSTONE_TOP
            } else {
                tex::SANDSTONE
            }
        }
        SPRUCE_LOG => {
            if ends {
                tex::SPRUCE_LOG_TOP
            } else {
                tex::SPRUCE_LOG
            }
        }
        SPRUCE_LEAVES => tex::SPRUCE_LEAVES,
        BIRCH_LOG => {
            if ends {
                tex::BIRCH_LOG_TOP
            } else {
                tex::BIRCH_LOG
            }
        }
        BIRCH_LEAVES => tex::BIRCH_LEAVES,
        CACTUS => {
            if ends {
                tex::CACTUS_TOP
            } else {
                tex::CACTUS
            }
        }
        TALL_GRASS => tex::TALL_GRASS,
        POPPY => tex::POPPY,
        DANDELION => tex::DANDELION,
        DEAD_BUSH => tex::DEAD_BUSH,
        COAL_ORE => tex::COAL_ORE,
        IRON_ORE => tex::IRON_ORE,
        COPPER_ORE => tex::COPPER_ORE,
        GOLD_ORE => tex::GOLD_ORE,
        DIAMOND_ORE => tex::DIAMOND_ORE,
        OBSIDIAN => tex::OBSIDIAN,
        ICE => tex::ICE,
        CLAY => tex::CLAY,
        GLOWSTONE => tex::GLOWSTONE,
        CRAFTING_TABLE => match face {
            2 => tex::CRAFTING_TOP,
            3 => tex::PLANKS,
            0 | 1 => tex::CRAFTING_SIDE,
            _ => tex::CRAFTING_FRONT,
        },
        _ if is_furnace(b) => {
            let f = facing(b).unwrap();
            let (top, front, side) = match furnace_base(b) {
                Some(BLAST_FURNACE) => (tex::BLAST_TOP, tex::BLAST_FRONT, tex::BLAST_SIDE),
                Some(ADV_FURNACE) => (tex::ADV_TOP, tex::ADV_FRONT, tex::ADV_SIDE),
                _ if is_lit_furnace(b) => (
                    tex::FURNACE_TOP,
                    tex::FURNACE_FRONT_LIT,
                    tex::FURNACE_SIDE,
                ),
                _ => (tex::FURNACE_TOP, tex::FURNACE_FRONT, tex::FURNACE_SIDE),
            };
            if ends {
                top
            } else if face == front_face(f) {
                front
            } else {
                side
            }
        }
        _ if is_chimney(b) => match face {
            2 => tex::CHIMNEY_TOP,
            3 => tex::BLAST_TOP,
            _ => tex::CHIMNEY_SIDE,
        },
        _ if is_adv_part(b) => {
            let (part, lit) = adv_part(b).unwrap();
            let f = facing(b).unwrap();
            if top && part >= 2 {
                tex::ADV_VENT_TOP
            } else if ends {
                tex::ADV_TOP
            } else if face == front_face(f) {
                match (part, lit) {
                    (1, _) => tex::ADV_PANEL,
                    (2, false) => tex::ADV_HOOD_L,
                    (2, true) => tex::ADV_HOOD_L_LIT,
                    (_, false) => tex::ADV_HOOD_R,
                    _ => tex::ADV_HOOD_R_LIT,
                }
            } else {
                tex::ADV_SIDE
            }
        }
        _ if is_chest(b) => {
            if ends {
                tex::CHEST_TOP
            } else if Some(face) == facing(b).map(front_face) {
                tex::CHEST_FRONT
            } else {
                tex::CHEST_SIDE
            }
        }
        _ if is_torch(b) => tex::TORCH,
        _ if is_lantern(b) => tex::LANTERN,
        OAK_SAPLING => tex::OAK_SAPLING,
        BIRCH_SAPLING => tex::BIRCH_SAPLING,
        SPRUCE_SAPLING => tex::SPRUCE_SAPLING,
        IRON_BLOCK => tex::IRON_BLOCK,
        COPPER_BLOCK => tex::COPPER_BLOCK,
        GOLD_BLOCK => tex::GOLD_BLOCK,
        DIAMOND_BLOCK => tex::DIAMOND_BLOCK,
        COAL_BLOCK => tex::COAL_BLOCK,
        STONE_BRICKS => tex::STONE_BRICKS,
        WOOL => tex::WOOL,
        _ if is_gun_bench(b) => match face {
            2 => tex::GUN_STATION_TOP,
            3 => tex::GUN_STATION_BOTTOM,
            _ => tex::GUN_STATION_SIDE,
        },
        GUN_STATION => match face {
            2 => tex::GUN_STATION_TOP,
            3 => tex::GUN_STATION_BOTTOM,
            _ => tex::GUN_STATION_SIDE,
        },
        _ if is_water(b) => tex::WATER,
        _ if is_lava(b) => tex::LAVA,
        _ => tex::STONE,
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum TintKind {
    None,
    Grass,
    Foliage,
    Spruce,
    Birch,
}

pub fn tint_kind(b: u8, face: usize) -> TintKind {
    match soil(b) {
        GRASS if face != 3 => TintKind::Grass,
        TALL_GRASS => TintKind::Grass,
        OAK_LEAVES => TintKind::Foliage,
        SPRUCE_LEAVES => TintKind::Spruce,
        BIRCH_LEAVES => TintKind::Birch,
        _ => TintKind::None,
    }
}

pub const SPRUCE_TINT: [u8; 3] = [97, 153, 97];
pub const BIRCH_TINT: [u8; 3] = [128, 167, 85];

/// Tint used for inventory icons (no biome context).
pub fn icon_tint(b: u8) -> [u8; 3] {
    match b {
        GRASS | TALL_GRASS => [112, 170, 72],
        OAK_LEAVES => [90, 146, 56],
        SPRUCE_LEAVES => SPRUCE_TINT,
        BIRCH_LEAVES => BIRCH_TINT,
        _ if is_water(b) => [70, 125, 230],
        _ => [255, 255, 255],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn property_tables_follow_the_rules() {
        for b in 0..=255u8 {
            assert_eq!(is_opaque(b), opaque_rule(b), "opaque {b}");
            assert_eq!(is_solid(b), solid_rule(b), "solid {b}");
            assert_eq!(attenuates_sky(b), attenuates_sky_rule(b), "sky {b}");
        }
    }
}
