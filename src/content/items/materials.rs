//! Materials: sticks, bones, coal, clay and bricks, the metals (nuggets and ingots, steel from
//! the blast furnace), diamonds and ceramic plates (bricks fired again in the advanced furnace).

use super::*;

/// Something to make things of: a row of the materials' tab.
const fn material(group: u8) -> ItemDef {
    ItemDef { creative: Creative::Materials(group), ..ITEM }
}

items! {
    after FIRST_ITEM;

    STICK = ItemDef { key: "stick", en: "Stick", hu: "Bot", icon: Icon::Flat(tex::STICK), fuel: Some(5.0), ..material(0) };
    /// A bone (pigs and sheep drop one now and then): tames a wolf.
    BONE = ItemDef { key: "bone", en: "Bone", hu: "Csont", icon: Icon::Flat(tex::BONE), ..material(0) };
    COAL = ItemDef { key: "coal", en: "Coal", hu: "Szén", icon: Icon::Flat(tex::COAL), fuel: Some(80.0), ..material(0) };
    CHARCOAL = ItemDef {
        key: "charcoal", en: "Charcoal", hu: "Faszén", icon: Icon::Flat(tex::CHARCOAL), fuel: Some(80.0),
        ..material(0)
    };
    CLAY_BALL = ItemDef {
        key: "clay_ball", en: "Clay Ball", hu: "Agyaggolyó", icon: Icon::Flat(tex::CLAY_BALL),
        smelt: smelts(BRICK, 1), ..material(0)
    };
    /// Fired again in the advanced furnace, a ceramic plate.
    BRICK = ItemDef {
        key: "brick", en: "Brick", hu: "Tégla", icon: Icon::Flat(tex::BRICK), smelt: smelts(CERAMIC_PLATE, 3),
        ..material(0)
    };

    COPPER_INGOT = ItemDef {
        key: "copper_ingot", en: "Copper Ingot", hu: "Rézrúd", icon: Icon::Flat(tex::COPPER_INGOT), ..material(1)
    };
    IRON_NUGGET = ItemDef {
        key: "iron_nugget", en: "Iron Nugget", hu: "Vasrög", icon: Icon::Flat(tex::IRON_NUGGET), ..material(1)
    };
    /// In the blast furnace, steel.
    IRON_INGOT = ItemDef {
        key: "iron_ingot", en: "Iron Ingot", hu: "Vasrúd", icon: Icon::Flat(tex::IRON_INGOT),
        smelt: smelts(STEEL_INGOT, 2), ..material(1)
    };
    GOLD_INGOT = ItemDef {
        key: "gold_ingot", en: "Gold Ingot", hu: "Aranyrúd", icon: Icon::Flat(tex::GOLD_INGOT), ..material(1)
    };
    DIAMOND = ItemDef { key: "diamond", en: "Diamond", hu: "Gyémánt", icon: Icon::Flat(tex::DIAMOND), ..material(1) };

    STEEL_INGOT = ItemDef {
        key: "steel_ingot", en: "Steel Ingot", hu: "Acélrúd", icon: Icon::Flat(tex::STEEL_INGOT), ..material(2)
    };
    CERAMIC_PLATE = ItemDef {
        key: "ceramic_plate", en: "Ceramic Plate", hu: "Kerámialap", icon: Icon::Flat(tex::CERAMIC_PLATE),
        ..material(2)
    };
}
