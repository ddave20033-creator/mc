//! Food and drink: pork and mutton in every way they come off the grill (a furnace's top),
//! fish, and water from a lake and boiled.

use super::*;

/// How done each side of a piece of meat is (0 raw, 1 cooked, 2 burnt; the more done side
/// first), for the meat variants in `meat`'s order.
pub const MEAT_SIDES: [[u8; 2]; 6] = [[0, 0], [1, 0], [1, 1], [2, 1], [2, 2], [2, 0]];

/// (food, saturation) of one side of a piece: raw, cooked, burnt (a whole piece is two sides,
/// matching Minecraft's raw and cooked values).
const PORK: [(f32, f32); 3] = [(1.5, 0.9), (4.0, 6.4), (1.0, 0.5)];
const LAMB: [(f32, f32); 3] = [(1.0, 0.6), (3.0, 4.8), (0.5, 0.3)];

/// Eating meat: each side counts for itself (raw, cooked or burnt), and together they make
/// what it does. A burnt side spoils the taste of the rest (it fills you for less long); raw
/// and burnt parts can upset your stomach, the more so the worse they are.
const fn grilled(per: [(f32, f32); 3], sides: [u8; 2]) -> Option<Consumable> {
    let (a, b) = (per[sides[0] as usize], per[sides[1] as usize]);
    let food = a.0 + b.0;
    let mut saturation = a.1 + b.1;
    if sides[0] == 2 && sides[1] != 2 {
        saturation *= 0.5;
    }
    const fn sick(chance: f32, poison: f32, nausea: f32) -> Option<Sickness> {
        Some(Sickness { chance, poison, nausea })
    }
    let sick = match sides {
        [0, 0] => sick(0.3, 0.0, 6.0),
        [1, 0] => sick(0.15, 0.0, 5.0),
        [1, 1] => None,
        [2, 1] => sick(0.3, 0.0, 6.0),
        [2, 0] => sick(0.5, 3.0, 8.0),
        _ => sick(0.7, 4.0, 10.0),
    };
    eaten(food, saturation, sick)
}

/// A piece of meat done so (`MEAT_SIDES[done]`): a row of the food tab each kind.
const fn meat_of(per: [(f32, f32); 3], done: usize, group: u8, icon: u32) -> ItemDef {
    ItemDef { icon: Icon::Flat(icon), food: grilled(per, MEAT_SIDES[done]), creative: Creative::Food(group), ..ITEM }
}
const fn pork(done: usize, icon: u32) -> ItemDef {
    meat_of(PORK, done, 0, icon)
}
const fn lamb(done: usize, icon: u32) -> ItemDef {
    meat_of(LAMB, done, 1, icon)
}

/// What lake water can do to you.
const BUG: Sickness = Sickness { chance: 0.7, poison: 5.0, nausea: 12.0 };

items! {
    after tools::END;

    PORKCHOP = ItemDef { key: "porkchop", en: "Raw Porkchop", hu: "Nyers disznóhús", ..pork(0, tex::PORKCHOP) };
    /// Meat grilled on one side only (on top of a furnace): half as filling as cooked.
    HALF_COOKED_PORKCHOP = ItemDef {
        key: "half_cooked_porkchop", en: "Half-Cooked Porkchop", hu: "Félig sült disznóhús",
        ..pork(1, tex::HALF_COOKED_PORKCHOP)
    };
    COOKED_PORKCHOP = ItemDef {
        key: "cooked_porkchop", en: "Cooked Porkchop", hu: "Sült disznóhús", ..pork(2, tex::COOKED_PORKCHOP)
    };
    /// Meat burnt on one side only.
    HALF_BURNT_PORKCHOP = ItemDef {
        key: "half_burnt_porkchop", en: "Half-Burnt Porkchop", hu: "Félig szenes disznóhús",
        ..pork(3, tex::HALF_BURNT_PORKCHOP)
    };
    /// Meat left on the fire too long.
    BURNT_PORKCHOP = ItemDef {
        key: "burnt_porkchop", en: "Burnt Porkchop", hu: "Szenes disznóhús", ..pork(4, tex::BURNT_PORKCHOP)
    };
    /// Meat burnt on one side and still raw on the other.
    RAW_BURNT_PORKCHOP = ItemDef {
        key: "raw_burnt_porkchop", en: "Burnt-Raw Porkchop", hu: "Szenes-nyers disznóhús",
        ..pork(5, tex::RAW_BURNT_PORKCHOP)
    };

    MUTTON = ItemDef { key: "mutton", en: "Raw Mutton", hu: "Nyers birkahús", ..lamb(0, tex::MUTTON) };
    HALF_COOKED_MUTTON = ItemDef {
        key: "half_cooked_mutton", en: "Half-Cooked Mutton", hu: "Félig sült birkahús",
        ..lamb(1, tex::HALF_COOKED_MUTTON)
    };
    COOKED_MUTTON = ItemDef {
        key: "cooked_mutton", en: "Cooked Mutton", hu: "Sült birkahús", ..lamb(2, tex::COOKED_MUTTON)
    };
    HALF_BURNT_MUTTON = ItemDef {
        key: "half_burnt_mutton", en: "Half-Burnt Mutton", hu: "Félig szenes birkahús",
        ..lamb(3, tex::HALF_BURNT_MUTTON)
    };
    BURNT_MUTTON = ItemDef {
        key: "burnt_mutton", en: "Burnt Mutton", hu: "Szenes birkahús", ..lamb(4, tex::BURNT_MUTTON)
    };
    RAW_BURNT_MUTTON = ItemDef {
        key: "raw_burnt_mutton", en: "Burnt-Raw Mutton", hu: "Szenes-nyers birkahús",
        ..lamb(5, tex::RAW_BURNT_MUTTON)
    };

    /// Water straight from a lake: quenches thirst, but can make you sick.
    WATER_BOTTLE = ItemDef {
        key: "water_bottle", en: "Water Bottle", hu: "Vizes üveg", icon: Icon::Flat(tex::WATER_BOTTLE),
        stack: 16, food: drunk(6.0, Some(BUG)), smelt: smelts(PURIFIED_WATER, 1), creative: Creative::Food(2),
        ..ITEM
    };
    /// Water boiled in a furnace: safe to drink.
    PURIFIED_WATER = ItemDef {
        key: "purified_water", en: "Boiled Water", hu: "Forralt víz", icon: Icon::Flat(tex::PURIFIED_WATER),
        stack: 16, food: drunk(10.0, None), creative: Creative::Food(2), ..ITEM
    };

    /// A fish caught with the rod (any kind): it can upset the stomach a little; grilled in a
    /// furnace it is as good as cooked meat.
    RAW_FISH = ItemDef {
        key: "raw_fish", en: "Raw Fish", hu: "Nyers hal", icon: Icon::Flat(tex::RAW_FISH),
        food: eaten(2.0, 0.4, Some(Sickness { chance: 0.2, poison: 0.0, nausea: 5.0 })),
        smelt: smelts(COOKED_FISH, 1), creative: Creative::Food(3), ..ITEM
    };
    COOKED_FISH = ItemDef {
        key: "cooked_fish", en: "Cooked Fish", hu: "Sült hal", icon: Icon::Flat(tex::COOKED_FISH),
        food: eaten(5.0, 6.0, None), creative: Creative::Food(3), ..ITEM
    };
}

/// Meat in every way it can come off the grill, each side raw, cooked or burnt (see
/// `MEAT_SIDES`): raw, one side cooked, cooked, one side cooked and one burnt, burnt, one side
/// raw and one burnt.
pub const MEATS: [[ItemId; 6]; 2] = [
    [PORKCHOP, HALF_COOKED_PORKCHOP, COOKED_PORKCHOP, HALF_BURNT_PORKCHOP, BURNT_PORKCHOP, RAW_BURNT_PORKCHOP],
    [MUTTON, HALF_COOKED_MUTTON, COOKED_MUTTON, HALF_BURNT_MUTTON, BURNT_MUTTON, RAW_BURNT_MUTTON],
];

/// The kind of meat `id` is, every way it comes.
pub fn meat(id: ItemId) -> Option<[ItemId; 6]> {
    MEATS.into_iter().find(|m| m.contains(&id))
}

/// The meat variant with these sides (in any order).
pub fn meat_with_sides(raw: ItemId, sides: [u8; 2]) -> ItemId {
    let key = [sides[0].max(sides[1]), sides[0].min(sides[1])];
    match (meat(raw), MEAT_SIDES.iter().position(|s| *s == key)) {
        (Some(m), Some(i)) => m[i],
        _ => raw,
    }
}
