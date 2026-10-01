//! Ores, and the blocks of what they give.

use super::*;

blocks! {
    after super::plants::END;

    COAL_ORE = BlockDef {
        key: "coal_ore", en: "Coal Ore", hu: "Szénérc", faces: Faces::All(tex::COAL_ORE),
        mine: pick(3.0, 0), drops: Drops::Item(COAL, 1), creative: Creative::Blocks(4), ..CUBE
    };
    COPPER_ORE = BlockDef {
        key: "copper_ore", en: "Copper Ore", hu: "Rézérc", faces: Faces::All(tex::COPPER_ORE),
        mine: pick(3.0, 1), smelt: smelts(COPPER_INGOT, 1), creative: Creative::Blocks(4), ..CUBE
    };
    IRON_ORE = BlockDef {
        key: "iron_ore", en: "Iron Ore", hu: "Vasérc", faces: Faces::All(tex::IRON_ORE),
        mine: pick(3.0, 2), smelt: smelts(IRON_INGOT, 2), creative: Creative::Blocks(4), ..CUBE
    };
    GOLD_ORE = BlockDef {
        key: "gold_ore", en: "Gold Ore", hu: "Aranyérc", faces: Faces::All(tex::GOLD_ORE),
        mine: pick(3.0, 3), smelt: smelts(GOLD_INGOT, 3), creative: Creative::Blocks(4), ..CUBE
    };
    DIAMOND_ORE = BlockDef {
        key: "diamond_ore", en: "Diamond Ore", hu: "Gyémántérc",
        faces: Faces::All(tex::DIAMOND_ORE), mine: pick(3.0, 3), smelt: smelts(DIAMOND, 3),
        creative: Creative::Blocks(4), ..CUBE
    };
    COAL_BLOCK = BlockDef {
        key: "coal_block", en: "Block of Coal", hu: "Szénblokk", faces: Faces::All(tex::COAL_BLOCK),
        mine: pick(5.0, 0), fuel: Some(800.0), creative: Creative::Blocks(5), ..CUBE
    };
    COPPER_BLOCK = BlockDef {
        key: "copper_block", en: "Block of Copper", hu: "Rézblokk",
        faces: Faces::All(tex::COPPER_BLOCK), mine: pick(5.0, 1), creative: Creative::Blocks(5),
        ..CUBE
    };
    IRON_BLOCK = BlockDef {
        key: "iron_block", en: "Block of Iron", hu: "Vasblokk", faces: Faces::All(tex::IRON_BLOCK),
        mine: pick(5.0, 2), creative: Creative::Blocks(5), ..CUBE
    };
    GOLD_BLOCK = BlockDef {
        key: "gold_block", en: "Block of Gold", hu: "Aranyblokk", faces: Faces::All(tex::GOLD_BLOCK),
        mine: pick(3.0, 3), creative: Creative::Blocks(5), ..CUBE
    };
    DIAMOND_BLOCK = BlockDef {
        key: "diamond_block", en: "Block of Diamond", hu: "Gyémántblokk",
        faces: Faces::All(tex::DIAMOND_BLOCK), mine: pick(5.0, 3), creative: Creative::Blocks(5),
        ..CUBE
    };
}

/// The ores found in the ground, from the most common (and softest) to the rarest.
pub const ORES: [Block; 5] = [COAL_ORE, COPPER_ORE, IRON_ORE, GOLD_ORE, DIAMOND_ORE];
