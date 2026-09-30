//! Stone and what is built of it.

use super::*;

blocks! {
    after super::earth::END;

    STONE = BlockDef {
        key: "stone", en: "Stone", hu: "Kő", faces: Faces::All(tex::STONE), mine: pick(1.5, 0),
        drops: Drops::Item(COBBLE, 1), creative: Creative::Blocks(1), ..CUBE
    };
    COBBLE = BlockDef {
        key: "cobblestone", en: "Cobblestone", hu: "Zúzottkő", faces: Faces::All(tex::COBBLE),
        mine: pick(2.0, 0), smelt: smelts(STONE, 1), creative: Creative::Blocks(1), ..CUBE
    };
    STONE_BRICKS = BlockDef {
        key: "stone_bricks", en: "Stone Bricks", hu: "Kőtégla",
        faces: Faces::All(tex::STONE_BRICKS), mine: pick(1.5, 0), creative: Creative::Blocks(1),
        ..CUBE
    };
    SANDSTONE = BlockDef {
        key: "sandstone", en: "Sandstone", hu: "Homokkő",
        faces: Faces::Column { side: tex::SANDSTONE, end: tex::SANDSTONE_TOP },
        mine: pick(0.8, 0), creative: Creative::Blocks(1), ..CUBE
    };
    BRICKS = BlockDef {
        key: "bricks", en: "Bricks", hu: "Téglák", faces: Faces::All(tex::BRICKS),
        mine: pick(2.0, 0), creative: Creative::Blocks(1), ..CUBE
    };
    OBSIDIAN = BlockDef {
        key: "obsidian", en: "Obsidian", hu: "Obszidián", faces: Faces::All(tex::OBSIDIAN),
        mine: pick(50.0, 4), creative: Creative::Blocks(1), ..CUBE
    };
    BEDROCK = BlockDef {
        key: "bedrock", en: "Bedrock", hu: "Alapkő", faces: Faces::All(tex::BEDROCK), mine: None,
        creative: Creative::Blocks(1), ..CUBE
    };
}
