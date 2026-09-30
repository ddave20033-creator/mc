//! Armor: helmets, chestplates, leggings and boots of wool, copper, steel (from the blast
//! furnace) and diamond, and the bulletproof vest worn over them (steel and ceramic plates
//! from the advanced furnace). What each piece protects and how long it lasts; what a worn
//! set does is `item::armor`'s.

use super::*;
use crate::world::WOOL;

/// Slots worn in: helmet, chestplate, leggings, boots, and the vest over the chestplate.
pub const ARMOR_SLOTS: usize = 5;
pub const VEST_SLOT: usize = 4;
/// Materials, weakest first.
pub const MATERIALS: usize = 4;

/// What a material's pieces are made of.
pub const fn material_item(material: usize) -> ItemId {
    [WOOL, COPPER_INGOT, STEEL_INGOT, DIAMOND][material]
}

/// A piece of a material (`slot`: helmet, chestplate, leggings, boots): Minecraft's
/// durability for its slot times its material's; a row of the armor tab each material.
const fn piece(material: usize, slot: usize) -> ItemDef {
    ItemDef {
        icon: Icon::Flat(tex::ARMOR_ICONS + (material * 4 + slot) as u32),
        durability: [11, 16, 15, 13][slot] * [4, 10, 17, 33][material],
        armor: Some(Armor { slot, material }),
        on_use: OnUse::Wear,
        creative: Creative::Armor(material as u8),
        ..SINGLE
    }
}

items! {
    after misc::END;

    WOOL_HELMET = ItemDef { key: "wool_helmet", en: "Wool Helmet", hu: "Posztó sisak", ..piece(0, 0) };
    WOOL_CHESTPLATE = ItemDef { key: "wool_chestplate", en: "Wool Chestplate", hu: "Posztó mellvért", ..piece(0, 1) };
    WOOL_LEGGINGS = ItemDef { key: "wool_leggings", en: "Wool Leggings", hu: "Posztó lábvért", ..piece(0, 2) };
    WOOL_BOOTS = ItemDef { key: "wool_boots", en: "Wool Boots", hu: "Posztó csizma", ..piece(0, 3) };
    COPPER_HELMET = ItemDef { key: "copper_helmet", en: "Copper Helmet", hu: "Réz sisak", ..piece(1, 0) };
    COPPER_CHESTPLATE = ItemDef { key: "copper_chestplate", en: "Copper Chestplate", hu: "Réz mellvért", ..piece(1, 1) };
    COPPER_LEGGINGS = ItemDef { key: "copper_leggings", en: "Copper Leggings", hu: "Réz lábvért", ..piece(1, 2) };
    COPPER_BOOTS = ItemDef { key: "copper_boots", en: "Copper Boots", hu: "Réz csizma", ..piece(1, 3) };
    STEEL_HELMET = ItemDef { key: "steel_helmet", en: "Steel Helmet", hu: "Acél sisak", ..piece(2, 0) };
    STEEL_CHESTPLATE = ItemDef { key: "steel_chestplate", en: "Steel Chestplate", hu: "Acél mellvért", ..piece(2, 1) };
    STEEL_LEGGINGS = ItemDef { key: "steel_leggings", en: "Steel Leggings", hu: "Acél lábvért", ..piece(2, 2) };
    STEEL_BOOTS = ItemDef { key: "steel_boots", en: "Steel Boots", hu: "Acél csizma", ..piece(2, 3) };
    DIAMOND_HELMET = ItemDef { key: "diamond_helmet", en: "Diamond Helmet", hu: "Gyémánt sisak", ..piece(3, 0) };
    DIAMOND_CHESTPLATE = ItemDef {
        key: "diamond_chestplate", en: "Diamond Chestplate", hu: "Gyémánt mellvért", ..piece(3, 1)
    };
    DIAMOND_LEGGINGS = ItemDef { key: "diamond_leggings", en: "Diamond Leggings", hu: "Gyémánt lábvért", ..piece(3, 2) };
    DIAMOND_BOOTS = ItemDef { key: "diamond_boots", en: "Diamond Boots", hu: "Gyémánt csizma", ..piece(3, 3) };
    /// Worn over the chestplate (its own slot): it takes most of a bullet and some of a blast.
    BULLETPROOF_VEST = ItemDef {
        key: "bulletproof_vest", en: "Bulletproof Vest", hu: "Golyóálló mellény",
        icon: Icon::Flat(tex::ARMOR_ICONS + 16), durability: 90,
        armor: Some(Armor { slot: VEST_SLOT, material: 0 }), on_use: OnUse::Wear, creative: Creative::Armor(4),
        ..SINGLE
    };
}

/// The pieces' ids by `[material][slot]`.
static ARMOR_IDS: [[ItemId; 4]; MATERIALS] = {
    let mut t = [[NONE; 4]; MATERIALS];
    let mut i = 0;
    while i < ALL.len() {
        if let Some(a) = ALL[i].armor {
            if a.slot < 4 {
                t[a.material][a.slot] = ALL[i].id;
            }
        }
        i += 1;
    }
    t
};

/// The piece of a material worn in a slot (helmet, chestplate, leggings, boots).
pub fn armor_id(material: usize, piece: usize) -> ItemId {
    ARMOR_IDS[material][piece]
}

/// The slot a piece is worn in and its material (the vest: its own slot, material 0).
pub fn armor_of(id: ItemId) -> Option<(usize, usize)> {
    item_def(id)?.armor.map(|a| (a.slot, a.material))
}

/// Armor points (Minecraft's): each takes 4% off the damage, 20 at most.
pub fn armor_points(id: ItemId) -> u32 {
    match armor_of(id) {
        Some((VEST_SLOT, _)) | None => 0,
        Some((piece, m)) => [[1, 3, 2, 1], [2, 5, 4, 1], [2, 6, 5, 2], [3, 8, 6, 3]][m][piece],
    }
}

/// Hits a piece takes before it breaks.
pub fn armor_durability(id: ItemId) -> u16 {
    armor_of(id).map_or(0, |_| max_damage(id))
}
