//! Block properties, read from the blocks' table (`content::blocks`): the families of blocks,
//! what glows, and the texture and tint of each face.

use super::*;
use crate::textures::tex;

#[inline]
fn model(b: Block) -> Model {
    def(b).model
}

#[inline]
pub fn is_water(b: Block) -> bool {
    base(b) == WATER
}
#[inline]
pub fn is_lava(b: Block) -> bool {
    base(b) == LAVA
}
#[inline]
pub fn is_fluid(b: Block) -> bool {
    model(b) == Model::Fluid
}
#[inline]
pub fn fluid_level(b: Block) -> u8 {
    (b - base(b)) as u8
}
#[inline]
pub fn is_leaves(b: Block) -> bool {
    model(b) == Model::Leaves
}
#[inline]
pub fn is_sapling(b: Block) -> bool {
    matches!(b, OAK_SAPLING | BIRCH_SAPLING | SPRUCE_SAPLING)
}
#[inline]
pub fn is_torch(b: Block) -> bool {
    model(b) == Model::Torch
}
/// Cross-shaped decorations (rendered as two crossed quads, walk-through).
#[inline]
pub fn is_plant(b: Block) -> bool {
    model(b) == Model::Plant
}
/// A furnace of any kind (the block with the openings, not the other parts of a big one).
#[inline]
pub fn is_furnace(b: Block) -> bool {
    model(b) == Model::Furnace
}
#[inline]
pub fn is_chimney(b: Block) -> bool {
    model(b) == Model::Chimney
}
#[inline]
pub fn is_adv_part(b: Block) -> bool {
    matches!(base(b), ADV_PART | ADV_PART_LIT)
}
/// The kind of furnace (FURNACE, BLAST_FURNACE or ADV_FURNACE) a block belongs to, its
/// other parts included.
pub fn furnace_base(b: Block) -> Option<Block> {
    match base(b) {
        FURNACE | FURNACE_LIT => Some(FURNACE),
        BLAST_FURNACE | BLAST_FURNACE_LIT | CHIMNEY => Some(BLAST_FURNACE),
        ADV_FURNACE | ADV_FURNACE_LIT | ADV_PART | ADV_PART_LIT => Some(ADV_FURNACE),
        _ => None,
    }
}
/// The furnaces, in the order of their tiers (`furnace_tier` 1, 2, 3).
pub const FURNACES: [Block; 3] = [FURNACE, BLAST_FURNACE, ADV_FURNACE];
/// What a furnace can smelt (`item::smelt_tier`): 1 the furnace, 2 the blast furnace, 3 the
/// advanced furnace.
pub fn furnace_tier(b: Block) -> u8 {
    match furnace_base(b) {
        Some(BLAST_FURNACE) => 2,
        Some(ADV_FURNACE) => 3,
        _ => 1,
    }
}
/// A burning furnace.
pub fn is_lit_furnace(b: Block) -> bool {
    matches!(base(b), FURNACE_LIT | BLAST_FURNACE_LIT | ADV_FURNACE_LIT)
}

/// The front of a furnace with its openings cut out (the model has hollows behind them).
pub fn furnace_front_cut(b: Block) -> u32 {
    match furnace_base(b) {
        Some(BLAST_FURNACE) => tex::BLAST_FRONT_CUT,
        Some(ADV_FURNACE) => tex::ADV_FRONT_CUT,
        _ => tex::FURNACE_FRONT_CUT,
    }
}
#[inline]
pub fn is_chest(b: Block) -> bool {
    model(b) == Model::Chest
}

#[inline]
pub fn is_stump_mark(b: Block) -> bool {
    matches!(base(b), STUMP_MARK | STUMP_MARK_SNOWY)
}
/// The mark of a cut-down trunk at `stage` (0 the whole trunk's width) on `grass`.
pub fn stump_mark(grass: Block, stage: Block) -> Block {
    (if grass == SNOWY_GRASS { STUMP_MARK_SNOWY } else { STUMP_MARK }) + stage
}
pub fn stump_stage(b: Block) -> Block {
    b - base(b)
}
/// The block as the world's rules see it: a stump mark is the grass it is on.
pub fn soil(b: Block) -> Block {
    match base(b) {
        STUMP_MARK => GRASS,
        STUMP_MARK_SNOWY => SNOWY_GRASS,
        _ => b,
    }
}

/// Needs a solid block below it (breaks otherwise).
#[inline]
pub fn needs_support(b: Block) -> bool {
    def(b).needs_support
}
/// Falls like sand when unsupported.
#[inline]
pub fn has_gravity(b: Block) -> bool {
    def(b).gravity
}
/// Can be overwritten by placing a block or by flowing fluid.
#[inline]
pub fn is_replaceable(b: Block) -> bool {
    def(b).replaceable
}
/// Flowing fluid washes these away.
#[inline]
pub fn fluid_breaks(b: Block) -> bool {
    def(b).washes_away
}
#[inline]
pub fn emission(b: Block) -> u8 {
    def(b).light
}

/// Axis (0 x, 1 y, 2 z) of each face's normal.
const FACE_AXIS: [usize; 6] = [0, 0, 1, 1, 2, 2];

/// Texture array layer for a block face. Faces: 0 +X, 1 -X, 2 +Y, 3 -Y, 4 +Z, 5 -Z.
pub fn face_texture(b: Block, face: usize) -> u32 {
    if is_log(b) && (log_axis(b) != 1 || is_branch(b)) {
        // Lying or a branch: its tree's upright log, turned.
        let end = FACE_AXIS[face] == log_axis(b);
        return face_texture(log_base(b), if end { 2 } else { 0 });
    }
    match def(b).faces {
        Faces::All(t) => t,
        Faces::Column { side, end } => {
            if face == 2 || face == 3 {
                end
            } else {
                side
            }
        }
        Faces::Sides { top, side, bottom } => match face {
            2 => top,
            3 => bottom,
            _ => side,
        },
        Faces::Custom(f) => f(b, face),
    }
}

pub fn tint_kind(b: Block, face: usize) -> TintKind {
    match def(b).tint {
        TintKind::Grass if face == 3 && !is_plant(b) => TintKind::None,
        t => t,
    }
}

pub const SPRUCE_TINT: [u8; 3] = [97, 153, 97];
pub const BIRCH_TINT: [u8; 3] = [128, 167, 85];

/// Tint used for inventory icons (no biome context).
pub fn icon_tint(b: Block) -> [u8; 3] {
    if is_water(b) {
        return [70, 125, 230];
    }
    match def(b).tint {
        TintKind::Grass => [112, 170, 72],
        TintKind::Foliage => [90, 146, 56],
        TintKind::Spruce => SPRUCE_TINT,
        TintKind::Birch => BIRCH_TINT,
        TintKind::None => [255, 255, 255],
    }
}
