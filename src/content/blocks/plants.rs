//! Plants: leaves, saplings, grass, flowers, dead bushes and cactus.

use super::*;

blocks! {
    after super::building::END;

    OAK_LEAVES = BlockDef {
        key: "oak_leaves", en: "Oak Leaves", hu: "Tölgylevelek", faces: Faces::All(tex::OAK_LEAVES),
        tint: TintKind::Foliage, ..LEAVES
    };
    BIRCH_LEAVES = BlockDef {
        key: "birch_leaves", en: "Birch Leaves", hu: "Nyírfalevelek",
        faces: Faces::All(tex::BIRCH_LEAVES), tint: TintKind::Birch, ..LEAVES
    };
    SPRUCE_LEAVES = BlockDef {
        key: "spruce_leaves", en: "Spruce Leaves", hu: "Lucfenyőlevelek",
        faces: Faces::All(tex::SPRUCE_LEAVES), tint: TintKind::Spruce, ..LEAVES
    };
    OAK_SAPLING = BlockDef {
        key: "oak_sapling", en: "Oak Sapling", hu: "Tölgycsemete",
        faces: Faces::All(tex::OAK_SAPLING), icon: Some(tex::OAK_SAPLING),
        fuel: Some(5.0), creative: Creative::Blocks(3), ..PLANT
    };
    BIRCH_SAPLING = BlockDef {
        key: "birch_sapling", en: "Birch Sapling", hu: "Nyírfacsemete",
        faces: Faces::All(tex::BIRCH_SAPLING), icon: Some(tex::BIRCH_SAPLING),
        fuel: Some(5.0), creative: Creative::Blocks(3), ..PLANT
    };
    SPRUCE_SAPLING = BlockDef {
        key: "spruce_sapling", en: "Spruce Sapling", hu: "Lucfenyőcsemete",
        faces: Faces::All(tex::SPRUCE_SAPLING), icon: Some(tex::SPRUCE_SAPLING),
        fuel: Some(5.0), creative: Creative::Blocks(3), ..PLANT
    };
    TALL_GRASS = BlockDef {
        key: "grass", en: "Grass", hu: "Fű", faces: Faces::All(tex::TALL_GRASS),
        tint: TintKind::Grass, replaceable: true, drops: Drops::Nothing, shears: true,
        icon: Some(tex::TALL_GRASS), creative: Creative::Blocks(3), ..PLANT
    };
    POPPY = BlockDef {
        key: "poppy", en: "Poppy", hu: "Pipacs", faces: Faces::All(tex::POPPY),
        icon: Some(tex::POPPY), creative: Creative::Blocks(3), ..PLANT
    };
    DANDELION = BlockDef {
        key: "dandelion", en: "Dandelion", hu: "Pitypang", faces: Faces::All(tex::DANDELION),
        icon: Some(tex::DANDELION), creative: Creative::Blocks(3), ..PLANT
    };
    DEAD_BUSH = BlockDef {
        key: "dead_bush", en: "Dead Bush", hu: "Elszáradt bokor", faces: Faces::All(tex::DEAD_BUSH),
        drops: Drops::Custom(dead_bush_drops), shears: true, icon: Some(tex::DEAD_BUSH),
        creative: Creative::Blocks(3), ..PLANT
    };
    CACTUS = BlockDef {
        key: "cactus", en: "Cactus", hu: "Kaktusz",
        faces: Faces::Column { side: tex::CACTUS, end: tex::CACTUS_TOP },
        needs_support: true, mine: mine(0.4, None, None), creative: Creative::Blocks(3), ..CUBE
    };
}

const LEAVES: BlockDef = BlockDef {
    model: Model::Leaves,
    opaque: false,
    mine: mine(0.2, Some(ToolKind::Sword), None),
    drops: Drops::Custom(leaf_drops),
    shears: true,
    creative: Creative::Blocks(3),
    ..CUBE
};

/// Now and then a sapling of the tree, or (oak) a stick.
fn leaf_drops(b: Block, _: ItemId, r: f32) -> Vec<Stack> {
    if r < 0.05 {
        let sapling = match b {
            BIRCH_LEAVES => BIRCH_SAPLING,
            SPRUCE_LEAVES => SPRUCE_SAPLING,
            _ => OAK_SAPLING,
        };
        vec![Stack::one(sapling as ItemId)]
    } else if b == OAK_LEAVES && r > 0.98 {
        vec![Stack::one(STICK)]
    } else {
        Vec::new()
    }
}

/// Up to two sticks.
fn dead_bush_drops(_: Block, _: ItemId, r: f32) -> Vec<Stack> {
    let n = (r * 3.0) as u8;
    if n > 0 {
        vec![Stack::new(STICK, n)]
    } else {
        Vec::new()
    }
}
