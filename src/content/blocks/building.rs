//! Wood and building blocks: logs (upright, and lying along X or Z) and branches, planks, stairs,
//! glass, glowstone and wool.

use super::*;

blocks! {
    after super::stone::END;

    OAK_LOG = BlockDef {
        key: "oak_log", en: "Oak Log", hu: "Tölgyfarönk",
        faces: Faces::Column { side: tex::OAK_LOG, end: tex::OAK_LOG_TOP },
        creative: Creative::Blocks(2), ..LOG
    };
    BIRCH_LOG = BlockDef {
        key: "birch_log", en: "Birch Log", hu: "Nyírfarönk",
        faces: Faces::Column { side: tex::BIRCH_LOG, end: tex::BIRCH_LOG_TOP },
        creative: Creative::Blocks(2), ..LOG
    };
    SPRUCE_LOG = BlockDef {
        key: "spruce_log", en: "Spruce Log", hu: "Lucfenyőrönk",
        faces: Faces::Column { side: tex::SPRUCE_LOG, end: tex::SPRUCE_LOG_TOP },
        creative: Creative::Blocks(2), ..LOG
    };
    OAK_LOG_X = BlockDef { key: "oak_log_x", item: BlockItem::As(OAK_LOG), ..LOG };
    OAK_LOG_Z = BlockDef { key: "oak_log_z", item: BlockItem::As(OAK_LOG), ..LOG };
    SPRUCE_LOG_X = BlockDef { key: "spruce_log_x", item: BlockItem::As(SPRUCE_LOG), ..LOG };
    SPRUCE_LOG_Z = BlockDef { key: "spruce_log_z", item: BlockItem::As(SPRUCE_LOG), ..LOG };
    BIRCH_LOG_X = BlockDef { key: "birch_log_x", item: BlockItem::As(BIRCH_LOG), ..LOG };
    BIRCH_LOG_Z = BlockDef { key: "birch_log_z", item: BlockItem::As(BIRCH_LOG), ..LOG };
    /// Branches: thin round logs growing out of the trees' trunks, upright or lying along X
    /// or Z (spruce branches only lie). Mined, they give back their tree's log.
    OAK_BRANCH = BlockDef { key: "oak_branch", item: BlockItem::As(OAK_LOG), ..LOG };
    OAK_BRANCH_X = BlockDef { key: "oak_branch_x", item: BlockItem::As(OAK_LOG), ..LOG };
    OAK_BRANCH_Z = BlockDef { key: "oak_branch_z", item: BlockItem::As(OAK_LOG), ..LOG };
    BIRCH_BRANCH = BlockDef { key: "birch_branch", item: BlockItem::As(BIRCH_LOG), ..LOG };
    BIRCH_BRANCH_X = BlockDef { key: "birch_branch_x", item: BlockItem::As(BIRCH_LOG), ..LOG };
    BIRCH_BRANCH_Z = BlockDef { key: "birch_branch_z", item: BlockItem::As(BIRCH_LOG), ..LOG };
    SPRUCE_BRANCH_X = BlockDef { key: "spruce_branch_x", item: BlockItem::As(SPRUCE_LOG), ..LOG };
    SPRUCE_BRANCH_Z = BlockDef { key: "spruce_branch_z", item: BlockItem::As(SPRUCE_LOG), ..LOG };
    PLANKS = BlockDef {
        key: "oak_planks", en: "Oak Planks", hu: "Tölgyfa deszka", faces: Faces::All(tex::PLANKS),
        mine: axe(2.0), fuel: Some(15.0), creative: Creative::Blocks(2), ..CUBE
    };
    /// Oak stairs: + facing (bits 0-1, toward the tall back) + upside down (bit 2).
    OAK_STAIRS * 8 = BlockDef {
        key: "oak_stairs", en: "Oak Stairs", hu: "Tölgyfa lépcső", model: Model::Stairs,
        opaque: false, faces: Faces::All(tex::PLANKS), mine: axe(2.0), place: Place::Stairs,
        fuel: Some(15.0), creative: Creative::Blocks(2), ..CUBE
    };
    GLASS = BlockDef {
        key: "glass", en: "Glass", hu: "Üveg", faces: Faces::All(tex::GLASS),
        mine: mine(0.3, None, None), drops: Drops::Nothing, cull_same: true, solid: true,
        creative: Creative::Blocks(2), ..THIN
    };
    GLOWSTONE = BlockDef {
        key: "glowstone", en: "Glowstone", hu: "Izzókő", faces: Faces::All(tex::GLOWSTONE),
        light: 15, mine: mine(0.3, None, None), creative: Creative::Blocks(2), ..CUBE
    };
    WOOL = BlockDef {
        key: "white_wool", en: "White Wool", hu: "Fehér gyapjú", faces: Faces::All(tex::WOOL),
        mine: mine(0.8, None, None), fuel: Some(5.0), creative: Creative::Blocks(2), ..CUBE
    };
}

/// A round log, mined with an axe; it burns, and smelts into charcoal.
const LOG: BlockDef = BlockDef {
    model: Model::Log,
    opaque: false,
    mine: mine(2.0, Some(ToolKind::Axe), None),
    place: Place::Log,
    fuel: Some(15.0),
    smelt: smelts(CHARCOAL, 1),
    ..CUBE
};
