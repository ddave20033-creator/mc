use super::textures::tex;
use glam::IVec3;

pub const AIR: u8 = 0;
pub const GRASS: u8 = 1;
pub const DIRT: u8 = 2;
pub const STONE: u8 = 3;
pub const SAND: u8 = 4;
pub const OAK_LOG: u8 = 5;
pub const OAK_LEAVES: u8 = 6;
pub const SNOW: u8 = 7;
pub const PLANKS: u8 = 8;
pub const COBBLE: u8 = 9;
pub const BEDROCK: u8 = 10;
pub const GLASS: u8 = 11;
pub const BRICKS: u8 = 12;
pub const GRAVEL: u8 = 13;
pub const SNOWY_GRASS: u8 = 14;
pub const SANDSTONE: u8 = 15;
pub const SPRUCE_LOG: u8 = 16;
pub const SPRUCE_LEAVES: u8 = 17;
pub const BIRCH_LOG: u8 = 18;
pub const BIRCH_LEAVES: u8 = 19;
pub const CACTUS: u8 = 20;
pub const TALL_GRASS: u8 = 21;
pub const POPPY: u8 = 22;
pub const DANDELION: u8 = 23;
pub const DEAD_BUSH: u8 = 24;
pub const COAL_ORE: u8 = 25;
pub const IRON_ORE: u8 = 26;
pub const GOLD_ORE: u8 = 27;
pub const DIAMOND_ORE: u8 = 28;
pub const OBSIDIAN: u8 = 29;
pub const ICE: u8 = 30;
pub const CLAY: u8 = 31;
pub const GLOWSTONE: u8 = 32;
pub const CRAFTING_TABLE: u8 = 33;
/// Furnace, lit furnace and chest: base id + facing (0 north/-Z, 1 east/+X, 2 south/+Z, 3 west/-X).
pub const FURNACE: u8 = 34;
pub const FURNACE_LIT: u8 = 38;
pub const CHEST: u8 = 42;
pub const TORCH: u8 = 46;
pub const OAK_SAPLING: u8 = 47;
pub const BIRCH_SAPLING: u8 = 48;
pub const SPRUCE_SAPLING: u8 = 49;
pub const IRON_BLOCK: u8 = 50;
pub const GOLD_BLOCK: u8 = 51;
pub const DIAMOND_BLOCK: u8 = 52;
pub const COAL_BLOCK: u8 = 53;
pub const STONE_BRICKS: u8 = 54;
/// Wall torches encode which adjacent block they are attached to:
/// north, east, south, west. Floor torches keep the original TORCH id.
pub const WALL_TORCH: u8 = 55;
/// Lantern standing on a block, and hanging from the block above.
pub const LANTERN: u8 = 59;
pub const LANTERN_HANGING: u8 = 60;

/// Fluids: base id + level. Level 0 = source, 1..7 = flowing, 8 = falling.
pub const WATER: u8 = 64;
pub const LAVA: u8 = 80;
pub const FALLING: u8 = 8;

/// Double chest halves: base id + facing. The other half is on the chest's local +X side
/// (the viewer's right, seen from the front) for `CHEST_LEFT`, local -X for `CHEST_RIGHT`.
pub const CHEST_LEFT: u8 = 96;
pub const CHEST_RIGHT: u8 = 100;

#[inline]
pub fn is_water(b: u8) -> bool {
    (WATER..WATER + 16).contains(&b)
}
#[inline]
pub fn is_lava(b: u8) -> bool {
    (LAVA..LAVA + 16).contains(&b)
}
#[inline]
pub fn is_fluid(b: u8) -> bool {
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
pub fn is_leaves(b: u8) -> bool {
    matches!(b, OAK_LEAVES | SPRUCE_LEAVES | BIRCH_LEAVES)
}
#[inline]
pub fn is_sapling(b: u8) -> bool {
    matches!(b, OAK_SAPLING | BIRCH_SAPLING | SPRUCE_SAPLING)
}
#[inline]
pub fn is_torch(b: u8) -> bool {
    b == TORCH || (WALL_TORCH..WALL_TORCH + 4).contains(&b)
}

#[inline]
pub fn is_lantern(b: u8) -> bool {
    b == LANTERN || b == LANTERN_HANGING
}

/// Offset from a torch block to the block holding it up.
pub fn torch_support_offset(b: u8) -> Option<IVec3> {
    match b {
        TORCH | LANTERN => Some(IVec3::NEG_Y),
        LANTERN_HANGING => Some(IVec3::Y),
        WALL_TORCH => Some(IVec3::NEG_Z),
        x if x == WALL_TORCH + 1 => Some(IVec3::X),
        x if x == WALL_TORCH + 2 => Some(IVec3::Z),
        x if x == WALL_TORCH + 3 => Some(IVec3::NEG_X),
        _ => None,
    }
}

pub fn wall_torch_for_support(offset: IVec3) -> Option<u8> {
    match offset {
        IVec3::NEG_Z => Some(WALL_TORCH),
        IVec3::X => Some(WALL_TORCH + 1),
        IVec3::Z => Some(WALL_TORCH + 2),
        IVec3::NEG_X => Some(WALL_TORCH + 3),
        _ => None,
    }
}
/// Cross-shaped decorations (rendered as two crossed quads, walk-through).
#[inline]
pub fn is_plant(b: u8) -> bool {
    matches!(b, TALL_GRASS | POPPY | DANDELION | DEAD_BUSH) || is_sapling(b)
}
#[inline]
pub fn is_furnace(b: u8) -> bool {
    (FURNACE..FURNACE_LIT + 4).contains(&b)
}
#[inline]
pub fn is_chest(b: u8) -> bool {
    (CHEST..CHEST + 4).contains(&b) || (CHEST_LEFT..CHEST_RIGHT + 4).contains(&b)
}
/// Facing of a directional block.
pub fn facing(b: u8) -> Option<u8> {
    match b {
        _ if (FURNACE..FURNACE + 4).contains(&b) => Some(b - FURNACE),
        _ if (FURNACE_LIT..FURNACE_LIT + 4).contains(&b) => Some(b - FURNACE_LIT),
        _ if (CHEST..CHEST + 4).contains(&b) => Some(b - CHEST),
        _ if (CHEST_LEFT..CHEST_RIGHT + 4).contains(&b) => Some((b - CHEST_LEFT) & 3),
        _ => None,
    }
}
/// A chest's local +X axis in the world (its front is local +Z): the viewer's right.
pub fn chest_right(facing: u8) -> IVec3 {
    [IVec3::NEG_X, IVec3::NEG_Z, IVec3::X, IVec3::Z][facing as usize & 3]
}
/// Offset from a double chest half to its other half.
pub fn chest_partner_offset(b: u8) -> Option<IVec3> {
    let f = facing(b)?;
    match b {
        _ if (CHEST_LEFT..CHEST_LEFT + 4).contains(&b) => Some(chest_right(f)),
        _ if (CHEST_RIGHT..CHEST_RIGHT + 4).contains(&b) => Some(-chest_right(f)),
        _ => None,
    }
}
/// The id of a double chest half's other half.
pub fn chest_other_half(b: u8) -> Option<u8> {
    let f = facing(b)?;
    match b {
        _ if (CHEST_LEFT..CHEST_LEFT + 4).contains(&b) => Some(CHEST_RIGHT + f),
        _ if (CHEST_RIGHT..CHEST_RIGHT + 4).contains(&b) => Some(CHEST_LEFT + f),
        _ => None,
    }
}
/// Chest id for a facing: single (`side` 0), or the half whose partner is at
/// `side * chest_right(facing)`.
pub fn chest_id(facing: u8, side: i32) -> u8 {
    let f = facing & 3;
    match side.signum() {
        1 => CHEST_LEFT + f,
        -1 => CHEST_RIGHT + f,
        _ => CHEST + f,
    }
}
/// Face index (0 +X, 1 -X, 2 +Y, 3 -Y, 4 +Z, 5 -Z) that a facing points to.
pub fn front_face(facing: u8) -> usize {
    [5, 0, 4, 1][facing as usize & 3]
}
/// Needs a solid block below it (breaks otherwise).
pub fn needs_support(b: u8) -> bool {
    is_plant(b) || is_torch(b) || is_lantern(b) || b == CACTUS
}
/// Falls like sand when unsupported.
pub fn has_gravity(b: u8) -> bool {
    matches!(b, SAND | GRAVEL)
}
/// Fully hides neighbouring faces and blocks light.
#[inline]
pub fn is_opaque(b: u8) -> bool {
    !(b == AIR
        || b == GLASS
        || is_torch(b)
        || is_lantern(b)
        || is_leaves(b)
        || is_plant(b)
        || is_fluid(b)
        || is_chest(b))
}
/// Blocks player movement.
#[inline]
pub fn is_solid(b: u8) -> bool {
    !(b == AIR || is_torch(b) || is_lantern(b) || is_plant(b) || is_fluid(b))
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
    !(b == AIR || b == GLASS || is_torch(b) || is_lantern(b) || is_plant(b))
}
#[inline]
pub fn emission(b: u8) -> u8 {
    match b {
        _ if is_lava(b) => 15,
        GLOWSTONE => 15,
        _ if is_torch(b) => 14,
        _ if is_lantern(b) => 15,
        _ if (FURNACE_LIT..FURNACE_LIT + 4).contains(&b) => 13,
        _ => 0,
    }
}

/// Texture array layer for a block face. Faces: 0 +X, 1 -X, 2 +Y, 3 -Y, 4 +Z, 5 -Z.
pub fn face_texture(b: u8, face: usize) -> u32 {
    let top = face == 2;
    let bottom = face == 3;
    let ends = top || bottom;
    match b {
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
            if ends {
                tex::FURNACE_TOP
            } else if face == front_face(f) {
                if b >= FURNACE_LIT {
                    tex::FURNACE_FRONT_LIT
                } else {
                    tex::FURNACE_FRONT
                }
            } else {
                tex::FURNACE_SIDE
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
        GOLD_BLOCK => tex::GOLD_BLOCK,
        DIAMOND_BLOCK => tex::DIAMOND_BLOCK,
        COAL_BLOCK => tex::COAL_BLOCK,
        STONE_BRICKS => tex::STONE_BRICKS,
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
    match b {
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
    fn wall_torch_orientation_and_block_rules() {
        for support in [IVec3::NEG_Z, IVec3::X, IVec3::Z, IVec3::NEG_X] {
            let b = wall_torch_for_support(support).unwrap();
            assert_eq!(torch_support_offset(b), Some(support));
            assert!(is_torch(b) && needs_support(b) && fluid_breaks(b));
            assert!(!is_solid(b) && !is_opaque(b));
            assert_eq!(emission(b), emission(TORCH));
            assert_eq!(
                crate::item::item_of_block(b),
                Some(TORCH as crate::item::ItemId)
            );
        }
        assert_eq!(torch_support_offset(TORCH), Some(IVec3::NEG_Y));
        assert_eq!(wall_torch_for_support(IVec3::Y), None);
    }

    #[test]
    fn double_chest_halves_point_at_each_other() {
        for f in 0..4 {
            assert_eq!(chest_partner_offset(chest_id(f, 0)), None);
            for side in [-1, 1] {
                let b = chest_id(f, side);
                assert!(is_chest(b) && !is_opaque(b));
                assert_eq!(facing(b), Some(f));
                let d = chest_partner_offset(b).unwrap();
                assert_eq!(d, chest_right(f) * side);
                let other = chest_other_half(b).unwrap();
                assert_eq!(chest_partner_offset(other), Some(-d));
                assert_eq!(facing(other), Some(f));
                assert_eq!(
                    crate::item::item_of_block(b),
                    Some(CHEST as crate::item::ItemId)
                );
            }
        }
    }
}
