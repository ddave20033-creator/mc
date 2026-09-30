//! Earth: air, grass, dirt, sand, gravel, clay, snow and ice, and the marks of cut-down trunks on
//! grass.

use super::*;

blocks! {
    after 0;

    AIR = BlockDef {
        key: "air", en: "-", hu: "-", model: Model::Air, replaceable: true, mine: None,
        drops: Drops::Nothing, item: BlockItem::None, ..THIN
    };

    // Earth.
    GRASS = BlockDef {
        key: "grass_block", en: "Grass Block", hu: "Füves blokk",
        faces: Faces::Sides { top: tex::GRASS_TOP, side: tex::GRASS_SIDE, bottom: tex::DIRT },
        tint: TintKind::Grass, mine: shovel(0.6), drops: Drops::Item(DIRT, 1),
        creative: Creative::Blocks(0), ..CUBE
    };
    SNOWY_GRASS = BlockDef {
        key: "snowy_grass_block", en: "Snowy Grass Block", hu: "Havas füves blokk",
        faces: Faces::Sides { top: tex::SNOW, side: tex::SNOWY_GRASS_SIDE, bottom: tex::DIRT },
        mine: shovel(0.6), drops: Drops::Item(DIRT, 1), creative: Creative::Blocks(0), ..CUBE
    };
    DIRT = BlockDef {
        key: "dirt", en: "Dirt", hu: "Föld", faces: Faces::All(tex::DIRT), mine: shovel(0.5),
        creative: Creative::Blocks(0), ..CUBE
    };
    SAND = BlockDef {
        key: "sand", en: "Sand", hu: "Homok", faces: Faces::All(tex::SAND), mine: shovel(0.5),
        gravity: true, creative: Creative::Blocks(0), ..CUBE
    };
    GRAVEL = BlockDef {
        key: "gravel", en: "Gravel", hu: "Kavics", faces: Faces::All(tex::GRAVEL),
        mine: shovel(0.6), gravity: true, creative: Creative::Blocks(0), ..CUBE
    };
    CLAY = BlockDef {
        key: "clay", en: "Clay", hu: "Agyag", faces: Faces::All(tex::CLAY), mine: shovel(0.6),
        drops: Drops::Item(CLAY_BALL, 4), creative: Creative::Blocks(0), ..CUBE
    };
    SNOW = BlockDef {
        key: "snow_block", en: "Snow Block", hu: "Hóblokk", faces: Faces::All(tex::SNOW),
        mine: mine(0.2, Some(ToolKind::Shovel), Some(0)), creative: Creative::Blocks(0), ..CUBE
    };
    ICE = BlockDef {
        key: "ice", en: "Ice", hu: "Jég", faces: Faces::All(tex::ICE),
        mine: mine(0.5, Some(ToolKind::Pickaxe), None), drops: Drops::Nothing, cull_same: true,
        creative: Creative::Blocks(0), ..CUBE
    };
    /// Grass with the mark of a cut-down trunk in the middle of its top (bare soil in a
    /// circle, smaller at each stage as the grass grows back over it): `+ stage`.
    STUMP_MARK * 3 = BlockDef {
        key: "stump_mark", en: "Grass Block", hu: "Füves blokk", tint: TintKind::Grass,
        faces: Faces::Sides { top: tex::GRASS_TOP, side: tex::GRASS_SIDE, bottom: tex::DIRT },
        mine: shovel(0.6), drops: Drops::Item(DIRT, 1), item: BlockItem::As(GRASS), ..CUBE
    };
    /// The same on snowy grass.
    STUMP_MARK_SNOWY * 3 = BlockDef {
        key: "snowy_stump_mark", en: "Snowy Grass Block", hu: "Havas füves blokk",
        faces: Faces::Sides { top: tex::SNOW, side: tex::SNOWY_GRASS_SIDE, bottom: tex::DIRT },
        mine: shovel(0.6), drops: Drops::Item(DIRT, 1), item: BlockItem::As(SNOWY_GRASS), ..CUBE
    };
}
