//! Furniture and lights: chests (+ facing), beds, doors, torches and lanterns.

use super::*;

blocks! {
    after super::machines::END;

    CHEST * 4 = BlockDef {
        key: "chest", en: "Chest", hu: "Láda", place: Place::Chest, fuel: Some(15.0),
        creative: Creative::Functional(1), ..CHEST_DEF
    };
    /// Double chest halves: + facing. The other half is on the chest's local +X side (the
    /// viewer's right, seen from the front) for `CHEST_LEFT`, local -X for `CHEST_RIGHT`.
    CHEST_LEFT * 4 = BlockDef { key: "chest_left", item: BlockItem::As(CHEST), ..CHEST_DEF };
    CHEST_RIGHT * 4 = BlockDef { key: "chest_right", item: BlockItem::As(CHEST), ..CHEST_DEF };
    /// Red bed halves: + facing (bits 0-1, from the foot toward the head: the way the player
    /// looked when placing it) + head half (bit 2).
    BED * 8 = BlockDef {
        key: "red_bed", en: "Red Bed", hu: "Piros ágy", model: Model::Bed, opaque: false,
        faces: Faces::Custom(bed_faces), mine: mine(0.2, None, None),
        item: BlockItem::Own { stack: 1 }, icon: Some(tex::BED_ITEM), place: Place::Bed,
        creative: Creative::Functional(1), ..CUBE
    };
    /// Oak door halves: + facing (bits 0-1, the way the player looked when placing it)
    /// + open (bit 2) + upper half (bit 3) + hinge on the right (bit 4) + swings out (bit 5:
    /// toward the side it closes on, into the next block, instead of into its own block).
    OAK_DOOR * 64 = BlockDef {
        key: "oak_door", en: "Oak Door", hu: "Tölgyfa ajtó", model: Model::Door, solid: true,
        faces: Faces::Custom(door_faces), needs_support: true, mine: axe(3.0),
        icon: Some(tex::DOOR_ITEM), place: Place::Door, fuel: Some(10.0),
        creative: Creative::Functional(1), ..THIN
    };
    TORCH = BlockDef {
        key: "torch", en: "Torch", hu: "Fáklya", icon: Some(tex::TORCH), place: Place::Torch,
        creative: Creative::Functional(1), ..TORCH_DEF
    };
    /// Wall torches: + the side of the block they hang on (north, east, south, west).
    WALL_TORCH * 4 = BlockDef { key: "wall_torch", item: BlockItem::As(TORCH), ..TORCH_DEF };
    /// A lantern standing on a block, and one hanging from the block above.
    LANTERN = BlockDef {
        key: "lantern", en: "Lantern", hu: "Lámpás", icon: Some(tex::LANTERN_ITEM),
        place: Place::Lantern, creative: Creative::Functional(1), ..LANTERN_DEF
    };
    LANTERN_HANGING = BlockDef {
        key: "hanging_lantern", item: BlockItem::As(LANTERN), ..LANTERN_DEF
    };
}

const CHEST_DEF: BlockDef = BlockDef {
    model: Model::Chest,
    opaque: false,
    faces: Faces::Custom(chest_faces),
    mine: axe(2.5),
    ..CUBE
};

const TORCH_DEF: BlockDef = BlockDef {
    model: Model::Torch,
    faces: Faces::All(tex::TORCH),
    light: 14,
    ..PLANT
};

const LANTERN_DEF: BlockDef = BlockDef {
    model: Model::Lantern,
    faces: Faces::All(tex::LANTERN),
    light: 15,
    needs_support: true,
    mine: pick(3.5, 0),
    ..THIN
};

fn chest_faces(b: Block, face: usize) -> u32 {
    if face == 2 || face == 3 {
        tex::CHEST_TOP
    } else if Some(face) == facing(b).map(front_face) {
        tex::CHEST_FRONT
    } else {
        tex::CHEST_SIDE
    }
}

/// The bed is meshed on its own; this is for particles.
fn bed_faces(b: Block, face: usize) -> u32 {
    if face == 3 {
        tex::BED_BOTTOM
    } else if bed_head(b) {
        tex::BED_HEAD_TOP
    } else {
        tex::BED_FOOT_TOP
    }
}

fn door_faces(b: Block, _: usize) -> u32 {
    if door_upper(b) {
        tex::DOOR_TOP
    } else {
        tex::DOOR_BOTTOM
    }
}
