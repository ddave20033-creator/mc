//! Fluids: + level. Level 0 is a source, 1..7 flowing, 8 falling (`FALLING`).

use super::*;

blocks! {
    after super::ores::END;

    WATER * 9 = BlockDef {
        key: "water", en: "Water", hu: "Víz", model: Model::Fluid, faces: Faces::All(tex::WATER),
        stops_sky: true, replaceable: true, mine: None, drops: Drops::Nothing,
        item: BlockItem::Other(WATER_BUCKET), ..THIN
    };
    LAVA * 9 = BlockDef {
        key: "lava", en: "Lava", hu: "Láva", model: Model::Fluid, faces: Faces::All(tex::LAVA),
        stops_sky: true, light: 15, replaceable: true, mine: None, drops: Drops::Nothing,
        item: BlockItem::Other(LAVA_BUCKET), ..THIN
    };
}
