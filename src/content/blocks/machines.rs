//! Workstations: the crafting table, the furnaces (+ facing: 0 north/-Z, 1 east/+X, 2 south/+Z, 3
//! west/-X; a burning furnace's ids come right after the furnace's, `furnace_id`) and the gun
//! stations.

use super::*;

blocks! {
    after super::fluids::END;

    CRAFTING_TABLE = BlockDef {
        key: "crafting_table", en: "Crafting Table", hu: "Barkácsasztal",
        faces: Faces::Custom(crafting_table_faces), mine: axe(2.5),
        creative: Creative::Functional(0), ..CUBE
    };
    FURNACE * 4 = BlockDef {
        key: "furnace", en: "Furnace", hu: "Kemence", ..FURNACE_DEF
    };
    FURNACE_LIT * 4 = BlockDef {
        key: "lit_furnace", light: 13, item: BlockItem::As(FURNACE), creative: Creative::None,
        ..FURNACE_DEF
    };
    /// Blast furnace (smelts iron too), and the chimney standing on it (+ facing).
    BLAST_FURNACE * 4 = BlockDef {
        key: "blast_furnace", en: "Blast Furnace", hu: "Kohó", place: Place::BigFurnace,
        ..FURNACE_DEF
    };
    BLAST_FURNACE_LIT * 4 = BlockDef {
        key: "lit_blast_furnace", light: 13, item: BlockItem::As(BLAST_FURNACE),
        creative: Creative::None, ..FURNACE_DEF
    };
    CHIMNEY * 4 = BlockDef {
        key: "chimney", model: Model::Chimney, opaque: false, item: BlockItem::As(BLAST_FURNACE),
        creative: Creative::None, ..FURNACE_DEF
    };
    /// Advanced furnace (smelts gold and diamond too), two wide and two tall: the furnace
    /// itself (lower left, seen from the front) + facing, lit + facing, and its other parts,
    /// `ADV_PART + (part - 1) * 4 + facing` (part 1 lower right, 2 upper left, 3 upper right),
    /// glowing ones from `ADV_PART_LIT`.
    ADV_FURNACE * 4 = BlockDef {
        key: "advanced_furnace", en: "Advanced Furnace", hu: "Fejlett kohó",
        place: Place::BigFurnace, ..FURNACE_DEF
    };
    ADV_FURNACE_LIT * 4 = BlockDef {
        key: "lit_advanced_furnace", light: 13, item: BlockItem::As(ADV_FURNACE),
        creative: Creative::None, ..FURNACE_DEF
    };
    ADV_PART * 12 = BlockDef {
        key: "advanced_furnace_part", model: Model::Cube, item: BlockItem::As(ADV_FURNACE),
        creative: Creative::None, ..FURNACE_DEF
    };
    ADV_PART_LIT * 12 = BlockDef {
        key: "lit_advanced_furnace_part", model: Model::Cube, item: BlockItem::As(ADV_FURNACE),
        creative: Creative::None, ..FURNACE_DEF
    };
    /// Metal workbench for assembling and cleaning guns: the item. Placed, it is a
    /// `GUN_BENCH` (this block itself is the old one-block station).
    GUN_STATION = BlockDef {
        key: "gun_station", en: "Gun Station", hu: "Fegyverasztal", faces: GUN_STATION_FACES,
        mine: pick(3.5, 0), place: Place::GunBench, creative: Creative::Functional(0), ..CUBE
    };
    /// The gun station, two blocks wide: + facing (its front, where its drawer slides out,
    /// toward the player who placed it) + right half (bit 2; the left half, seen from the
    /// front, holds what lies on it). The cells in front of it are kept free for the drawer.
    GUN_BENCH * 8 = BlockDef {
        key: "gun_bench", model: Model::GunBench, opaque: false, faces: GUN_STATION_FACES,
        mine: pick(3.5, 0), item: BlockItem::As(GUN_STATION), ..CUBE
    };
    /// The rifle station, three blocks wide: its left block (seen from the front) + facing,
    /// which holds what lies on it (and is the item), and its other two blocks (without a
    /// facing: `bench_main` finds the left block they belong to).
    RIFLE_BENCH * 4 = BlockDef {
        key: "rifle_station", en: "Rifle Station", hu: "Puskaasztal", model: Model::GunBench,
        opaque: false, faces: GUN_STATION_FACES, mine: pick(3.5, 0), place: Place::RifleBench,
        creative: Creative::Functional(0), ..CUBE
    };
    RIFLE_BENCH_PART = BlockDef {
        key: "rifle_station_part", model: Model::GunBench, opaque: false,
        faces: GUN_STATION_FACES, mine: pick(3.5, 0), item: BlockItem::As(RIFLE_BENCH), ..CUBE
    };
}

const FURNACE_DEF: BlockDef = BlockDef {
    model: Model::Furnace,
    faces: Faces::Custom(furnace_faces),
    mine: pick(3.5, 0),
    place: Place::Facing,
    creative: Creative::Functional(0),
    ..CUBE
};

const GUN_STATION_FACES: Faces = Faces::Sides {
    top: tex::GUN_STATION_TOP,
    side: tex::GUN_STATION_SIDE,
    bottom: tex::GUN_STATION_BOTTOM,
};

fn crafting_table_faces(_: Block, face: usize) -> u32 {
    match face {
        2 => tex::CRAFTING_TOP,
        3 => tex::PLANKS,
        0 | 1 => tex::CRAFTING_SIDE,
        _ => tex::CRAFTING_FRONT,
    }
}

fn furnace_faces(b: Block, face: usize) -> u32 {
    let ends = face == 2 || face == 3;
    let f = facing(b).unwrap_or(0);
    let front = face == front_face(f);
    match def(b).model {
        Model::Chimney => match face {
            2 => tex::CHIMNEY_TOP,
            3 => tex::BLAST_TOP,
            _ => tex::CHIMNEY_SIDE,
        },
        // The advanced furnace's other parts.
        Model::Cube => {
            let (part, lit) = adv_part(b).unwrap_or((1, false));
            if face == 2 && part >= 2 {
                tex::ADV_VENT_TOP
            } else if ends {
                tex::ADV_TOP
            } else if front {
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
        _ => {
            let (top, front_tex, side) = match furnace_base(b) {
                Some(BLAST_FURNACE) => (tex::BLAST_TOP, tex::BLAST_FRONT, tex::BLAST_SIDE),
                Some(ADV_FURNACE) => (tex::ADV_TOP, tex::ADV_FRONT, tex::ADV_SIDE),
                _ if is_lit_furnace(b) => (tex::FURNACE_TOP, tex::FURNACE_FRONT_LIT, tex::FURNACE_SIDE),
                _ => (tex::FURNACE_TOP, tex::FURNACE_FRONT, tex::FURNACE_SIDE),
            };
            if ends {
                top
            } else if front {
                front_tex
            } else {
                side
            }
        }
    }
}
