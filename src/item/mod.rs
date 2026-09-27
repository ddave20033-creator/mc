//! Items: ids, names, icons, stacking and tools; mining rules, crafting and smelting are in
//! the submodules.
//!
//! Block items share the block's id (0..=255); other items start at 256.

pub mod armor;
pub mod crafting;
pub mod firearm;
pub mod inventory;
pub mod mining;

pub use armor::*;
pub use crafting::*;
pub use firearm::*;
pub use mining::*;

use crate::lang::is_hungarian;
use crate::world::textures::tex;
use crate::world::*;

pub type ItemId = u16;

pub const NONE: ItemId = 0;
pub const STICK: ItemId = 256;
pub const COAL: ItemId = 257;
pub const CHARCOAL: ItemId = 258;
pub const IRON_INGOT: ItemId = 259;
pub const GOLD_INGOT: ItemId = 260;
pub const DIAMOND: ItemId = 261;
pub const CLAY_BALL: ItemId = 262;
pub const BRICK: ItemId = 263;
pub const BUCKET: ItemId = 264;
pub const WATER_BUCKET: ItemId = 265;
pub const LAVA_BUCKET: ItemId = 266;
pub const PIG_SPAWN_EGG: ItemId = 267;
pub const PORKCHOP: ItemId = 268;
pub const COOKED_PORKCHOP: ItemId = 269;
pub const GLASS_BOTTLE: ItemId = 270;
/// Water straight from a lake: quenches thirst, but can make you sick.
pub const WATER_BOTTLE: ItemId = 271;
/// Water boiled in a furnace: safe to drink.
pub const PURIFIED_WATER: ItemId = 272;
pub const IRON_NUGGET: ItemId = 273;
pub const MUTTON: ItemId = 274;
pub const COOKED_MUTTON: ItemId = 275;
/// Shear sheep, and mine leaves, grass and dead bushes so they drop themselves.
pub const SHEARS: ItemId = 276;
pub const SHEEP_SPAWN_EGG: ItemId = 277;
/// Meat grilled on one side only (on top of a furnace): half as filling as cooked.
pub const HALF_COOKED_PORKCHOP: ItemId = 278;
pub const HALF_COOKED_MUTTON: ItemId = 279;
/// Meat left on the fire too long.
pub const BURNT_PORKCHOP: ItemId = 280;
pub const BURNT_MUTTON: ItemId = 281;
/// Meat burnt on one side only.
pub const HALF_BURNT_PORKCHOP: ItemId = 293;
pub const HALF_BURNT_MUTTON: ItemId = 294;
/// Meat burnt on one side and still raw on the other.
pub const RAW_BURNT_PORKCHOP: ItemId = 295;
pub const RAW_BURNT_MUTTON: ItemId = 296;
pub const COPPER_INGOT: ItemId = 297;
/// Steel, from iron in a blast furnace; ceramic plates, from bricks fired again in an
/// advanced furnace.
pub const STEEL_INGOT: ItemId = 340;
pub const CERAMIC_PLATE: ItemId = 341;
/// Grenades: thrown with the right mouse button.
pub const FRAG_GRENADE: ItemId = 342;
pub const SMOKE_GRENADE: ItemId = 343;
/// Pistol ammunition (9 mm): one is used up per shot.
pub const BULLET: ItemId = 282;
/// The five pistol parts, in the order they go together at the gun station: frame (with the
/// grip and trigger), barrel, recoil spring, slide and magazine.
pub const PISTOL_FRAME: ItemId = 283;
pub const PISTOL_BARREL: ItemId = 284;
pub const PISTOL_SPRING: ItemId = 285;
pub const PISTOL_SLIDE: ItemId = 286;
pub const PISTOL_MAGAZINE: ItemId = 287;
/// The guns (see `firearm`), put together at the gun station. A gun's `damage` is how dirty
/// it is (one per shot; cleaned at the gun station), its `data` holds the rounds in its
/// magazine and its attachments.
pub const PISTOL: ItemId = 288;
pub const DESERT_EAGLE: ItemId = 324;
pub const M16: ItemId = 325;
pub const SNIPER_RIFLE: ItemId = 326;
pub const SHOTGUN: ItemId = 327;
/// Ammunition of the other guns: 5.56 mm (M16), .50 AE (Desert Eagle), .50 BMG (sniper
/// rifle) and 12 gauge shells (shotgun).
pub const RIFLE_ROUND: ItemId = 320;
pub const MAGNUM_ROUND: ItemId = 321;
pub const BMG_ROUND: ItemId = 322;
pub const SHOTGUN_SHELL: ItemId = 323;
/// Pistol attachments, fitted at the gun station: a scope (zooms in far when aiming), a
/// silencer (no muzzle flash), an extended magazine and a laser sight (steadier from the hip).
pub const SCOPE: ItemId = 289;
pub const SILENCER: ItemId = 290;
pub const EXTENDED_MAGAZINE: ItemId = 291;
pub const LASER_SIGHT: ItemId = 292;

/// A gun's attachments as bits of `gun_mods`, with their items.
pub mod gun_mod {
    pub const SCOPE: u8 = 1;
    pub const SILENCER: u8 = 2;
    pub const EXTENDED_MAGAZINE: u8 = 4;
    pub const LASER: u8 = 8;
}
pub const ATTACHMENTS: [(u8, ItemId); 4] = [
    (gun_mod::SCOPE, SCOPE),
    (gun_mod::SILENCER, SILENCER),
    (gun_mod::EXTENDED_MAGAZINE, EXTENDED_MAGAZINE),
    (gun_mod::LASER, LASER_SIGHT),
];

/// Rounds in a gun's magazine (the low 6 bits of its data).
pub fn gun_rounds(s: &Stack) -> u8 {
    (s.data & 0x3f) as u8
}
pub fn set_gun_rounds(s: &mut Stack, n: u8) {
    s.data = (s.data & !0x3f) | (n as u16 & 0x3f);
}
/// A gun's attachments (`gun_mod` bits, in the data's high byte).
pub fn gun_mods(s: &Stack) -> u8 {
    (s.data >> 8) as u8
}
pub fn set_gun_mods(s: &mut Stack, mods: u8) {
    s.data = (s.data & 0xff) | ((mods as u16) << 8);
}
/// Minecraft's shears durability.
const SHEARS_DURABILITY: u16 = 238;
const TOOL_BASE: ItemId = 300;
/// Tools of the tiers after the first five (the ids after the first 20 tools are taken).
const MORE_TOOLS_BASE: ItemId = 330;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ToolKind {
    Pickaxe,
    Axe,
    Shovel,
    Sword,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tier {
    Wood,
    Stone,
    Iron,
    Gold,
    Diamond,
    // Added later, so its tool ids and texture layers come after the others'.
    Copper,
}

/// In `Tier` order (tool ids and texture layers follow it).
const TIERS: [Tier; 6] = [
    Tier::Wood,
    Tier::Stone,
    Tier::Iron,
    Tier::Gold,
    Tier::Diamond,
    Tier::Copper,
];

/// From the first tool to the last.
pub const TIER_ORDER: [Tier; 6] = [
    Tier::Wood,
    Tier::Stone,
    Tier::Copper,
    Tier::Iron,
    Tier::Gold,
    Tier::Diamond,
];
const KINDS: [ToolKind; 4] = [
    ToolKind::Pickaxe,
    ToolKind::Axe,
    ToolKind::Shovel,
    ToolKind::Sword,
];

impl Tier {
    /// Mining speed multiplier with the right tool.
    pub fn speed(self) -> f32 {
        match self {
            Tier::Wood => 2.0,
            Tier::Stone => 4.0,
            Tier::Copper => 5.0,
            Tier::Iron => 6.5,
            Tier::Gold => 11.0,
            Tier::Diamond => 8.5,
        }
    }
    /// Harvest level: which ores this tier can mine (see `mining`): wood mines coal, stone
    /// copper, copper iron, iron (or gold) gold and diamond, diamond obsidian.
    pub fn level(self) -> u8 {
        match self {
            Tier::Wood => 0,
            Tier::Stone => 1,
            Tier::Copper => 2,
            Tier::Iron | Tier::Gold => 3,
            Tier::Diamond => 4,
        }
    }
    pub fn durability(self) -> u16 {
        match self {
            Tier::Wood => 59,
            Tier::Stone => 131,
            Tier::Copper => 190,
            Tier::Iron => 350,
            Tier::Gold => 40,
            Tier::Diamond => 1500,
        }
    }
    /// The material crafted into the tool head.
    fn material(self) -> ItemId {
        match self {
            Tier::Wood => PLANKS as ItemId,
            Tier::Stone => COBBLE as ItemId,
            Tier::Iron => IRON_INGOT,
            Tier::Gold => GOLD_INGOT,
            Tier::Diamond => DIAMOND,
            Tier::Copper => COPPER_INGOT,
        }
    }
}

pub fn tool_id(kind: ToolKind, tier: Tier) -> ItemId {
    let i = tier as ItemId * 4 + kind as ItemId;
    if i < 20 {
        TOOL_BASE + i
    } else {
        MORE_TOOLS_BASE + i - 20
    }
}

/// Swords can block (right mouse button held), like in Minecraft 1.8.
pub fn is_sword(id: ItemId) -> bool {
    matches!(tool_of(id), Some((ToolKind::Sword, _)))
}

pub fn tool_of(id: ItemId) -> Option<(ToolKind, Tier)> {
    let i = if (TOOL_BASE..TOOL_BASE + 20).contains(&id) {
        id - TOOL_BASE
    } else if (MORE_TOOLS_BASE..MORE_TOOLS_BASE + 4 * (TIERS.len() as ItemId - 5)).contains(&id) {
        20 + id - MORE_TOOLS_BASE
    } else {
        return None;
    };
    Some((KINDS[(i % 4) as usize], TIERS[(i / 4) as usize]))
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Stack {
    pub item: ItemId,
    pub count: u8,
    /// Durability used up (tools); how dirty a pistol is.
    pub damage: u16,
    /// Extra state of the item (a pistol: rounds in its magazine and its attachments).
    pub data: u16,
}

impl Stack {
    pub fn new(item: ItemId, count: u8) -> Self {
        Self {
            item,
            count,
            damage: 0,
            data: 0,
        }
    }
    pub fn one(item: ItemId) -> Self {
        Self::new(item, 1)
    }
    pub fn stacks_with(&self, other: &Stack) -> bool {
        self.item == other.item
            && self.damage == other.damage
            && self.data == other.data
            && max_stack(self.item) > 1
    }
}

pub type Slot = Option<Stack>;

pub fn max_stack(id: ItemId) -> u8 {
    match id {
        _ if tool_of(id).is_some() => 1,
        WATER_BUCKET | LAVA_BUCKET | SHEARS => 1,
        FRAG_GRENADE | SMOKE_GRENADE => 16,
        _ if armor_of(id).is_some() => 1,
        _ if GunKind::of(id).is_some() => 1,
        _ if id == BED as ItemId => 1,
        BUCKET | WATER_BOTTLE | PURIFIED_WATER => 16,
        _ => 64,
    }
}

/// Food and drink: what eating or drinking this restores (Minecraft's food values), and
/// what it can do to you.
pub fn consumable(id: ItemId) -> Option<crate::entity::survival::Consumable> {
    use crate::entity::survival::{Consumable, Sickness};
    if let Some(c) = meat_food(id) {
        return Some(c);
    }
    let drink = |thirst, sick| Consumable {
        food: 0.0,
        saturation: 0.0,
        thirst,
        sick,
        drink: true,
    };
    let bug = Sickness {
        chance: 0.7,
        poison: 5.0,
        nausea: 12.0,
    };
    Some(match id {
        WATER_BOTTLE => drink(6.0, Some(bug)),
        PURIFIED_WATER => drink(10.0, None),
        _ => return None,
    })
}

/// How done each side of a piece of meat is (0 raw, 1 cooked, 2 burnt; the more done side
/// first), for the meat variants in `meat`'s order.
pub const MEAT_SIDES: [[u8; 2]; 6] = [[0, 0], [1, 0], [1, 1], [2, 1], [2, 2], [2, 0]];

/// Meat in every way it can come off the grill, each side raw, cooked or burnt (see
/// `MEAT_SIDES`): raw, one side cooked, cooked, one side cooked and one burnt, burnt, one
/// side raw and one burnt.
pub fn meat(id: ItemId) -> Option<[ItemId; 6]> {
    const PORK: [ItemId; 6] = [
        PORKCHOP,
        HALF_COOKED_PORKCHOP,
        COOKED_PORKCHOP,
        HALF_BURNT_PORKCHOP,
        BURNT_PORKCHOP,
        RAW_BURNT_PORKCHOP,
    ];
    const LAMB: [ItemId; 6] = [
        MUTTON,
        HALF_COOKED_MUTTON,
        COOKED_MUTTON,
        HALF_BURNT_MUTTON,
        BURNT_MUTTON,
        RAW_BURNT_MUTTON,
    ];
    [PORK, LAMB].into_iter().find(|m| m.contains(&id))
}

/// The meat variant with these sides (in any order).
pub fn meat_with_sides(raw: ItemId, sides: [u8; 2]) -> ItemId {
    let key = [sides[0].max(sides[1]), sides[0].min(sides[1])];
    match (meat(raw), MEAT_SIDES.iter().position(|s| *s == key)) {
        (Some(m), Some(i)) => m[i],
        _ => raw,
    }
}

/// Eating meat: each side counts for itself (raw, cooked or burnt), and together they
/// make what it does. A burnt side spoils the taste of the rest (it fills you for less
/// long); raw and burnt parts can upset your stomach, the more so the worse they are.
fn meat_food(id: ItemId) -> Option<crate::entity::survival::Consumable> {
    use crate::entity::survival::{Consumable, Sickness};
    let m = meat(id)?;
    let sides = MEAT_SIDES[m.iter().position(|&i| i == id)?];
    // (food, saturation) of one side: raw, cooked, burnt (a whole piece is two sides,
    // matching Minecraft's raw and cooked values).
    let per = if m[0] == PORKCHOP {
        [(1.5, 0.9), (4.0, 6.4), (1.0, 0.5)]
    } else {
        [(1.0, 0.6), (3.0, 4.8), (0.5, 0.3)]
    };
    let food = per[sides[0] as usize].0 + per[sides[1] as usize].0;
    let mut saturation = per[sides[0] as usize].1 + per[sides[1] as usize].1;
    if sides[0] == 2 && sides[1] != 2 {
        saturation *= 0.5;
    }
    let sick = |chance, poison, nausea| {
        Some(Sickness {
            chance,
            poison,
            nausea,
        })
    };
    let sick = match sides {
        [0, 0] => sick(0.3, 0.0, 6.0),
        [1, 0] => sick(0.15, 0.0, 5.0),
        [1, 1] => None,
        [2, 1] => sick(0.3, 0.0, 6.0),
        [2, 0] => sick(0.5, 3.0, 8.0),
        _ => sick(0.7, 4.0, 10.0),
    };
    Some(Consumable {
        food,
        saturation,
        thirst: 0.0,
        sick,
        drink: false,
    })
}

/// Damage dealt when hitting a mob with this item (Minecraft 1.8 values; 1 = bare hand).
pub fn attack_damage(id: ItemId) -> f32 {
    let Some((kind, tier)) = tool_of(id) else {
        return 1.0;
    };
    let base = match kind {
        ToolKind::Sword => 5.0,
        ToolKind::Axe => 4.0,
        ToolKind::Pickaxe => 3.0,
        ToolKind::Shovel => 2.0,
    };
    let bonus = match tier {
        Tier::Wood | Tier::Gold => 0.0,
        Tier::Stone => 1.0,
        Tier::Copper => 1.5,
        Tier::Iron => 2.0,
        Tier::Diamond => 3.0,
    };
    base + bonus
}

pub fn max_damage(id: ItemId) -> u16 {
    if id == SHEARS {
        return SHEARS_DURABILITY;
    }
    if let Some(k) = GunKind::of(id) {
        return k.stats().dirt_max;
    }
    if armor_of(id).is_some() {
        return armor_durability(id);
    }
    tool_of(id).map(|(_, t)| t.durability()).unwrap_or(0)
}

/// Every block that exists as an item, in creative inventory order: the block, its key (for
/// /give and save files), and its English and Hungarian names.
const BLOCK_ITEMS: &[(u8, &str, &str, &str)] = &[
    (GRASS, "grass_block", "Grass Block", "Füves blokk"),
    (DIRT, "dirt", "Dirt", "Föld"),
    (STONE, "stone", "Stone", "Kő"),
    (COBBLE, "cobblestone", "Cobblestone", "Zúzottkő"),
    (STONE_BRICKS, "stone_bricks", "Stone Bricks", "Kőtégla"),
    (SAND, "sand", "Sand", "Homok"),
    (GRAVEL, "gravel", "Gravel", "Kavics"),
    (CLAY, "clay", "Clay", "Agyag"),
    (SANDSTONE, "sandstone", "Sandstone", "Homokkő"),
    (SNOW, "snow_block", "Snow Block", "Hóblokk"),
    (
        SNOWY_GRASS,
        "snowy_grass_block",
        "Snowy Grass Block",
        "Havas füves blokk",
    ),
    (ICE, "ice", "Ice", "Jég"),
    (OAK_LOG, "oak_log", "Oak Log", "Tölgyfarönk"),
    (BIRCH_LOG, "birch_log", "Birch Log", "Nyírfarönk"),
    (SPRUCE_LOG, "spruce_log", "Spruce Log", "Lucfenyőrönk"),
    (PLANKS, "oak_planks", "Oak Planks", "Tölgyfa deszka"),
    (OAK_STAIRS, "oak_stairs", "Oak Stairs", "Tölgyfa lépcső"),
    (OAK_DOOR, "oak_door", "Oak Door", "Tölgyfa ajtó"),
    (BRICKS, "bricks", "Bricks", "Téglák"),
    (GLASS, "glass", "Glass", "Üveg"),
    (GLOWSTONE, "glowstone", "Glowstone", "Izzókő"),
    (OAK_LEAVES, "oak_leaves", "Oak Leaves", "Tölgylevelek"),
    (
        BIRCH_LEAVES,
        "birch_leaves",
        "Birch Leaves",
        "Nyírfalevelek",
    ),
    (
        SPRUCE_LEAVES,
        "spruce_leaves",
        "Spruce Leaves",
        "Lucfenyőlevelek",
    ),
    (OAK_SAPLING, "oak_sapling", "Oak Sapling", "Tölgycsemete"),
    (
        BIRCH_SAPLING,
        "birch_sapling",
        "Birch Sapling",
        "Nyírfacsemete",
    ),
    (
        SPRUCE_SAPLING,
        "spruce_sapling",
        "Spruce Sapling",
        "Lucfenyőcsemete",
    ),
    (CACTUS, "cactus", "Cactus", "Kaktusz"),
    (TALL_GRASS, "grass", "Grass", "Fű"),
    (POPPY, "poppy", "Poppy", "Pipacs"),
    (DANDELION, "dandelion", "Dandelion", "Pitypang"),
    (DEAD_BUSH, "dead_bush", "Dead Bush", "Elszáradt bokor"),
    (COAL_ORE, "coal_ore", "Coal Ore", "Szénérc"),
    (COPPER_ORE, "copper_ore", "Copper Ore", "Rézérc"),
    (IRON_ORE, "iron_ore", "Iron Ore", "Vasérc"),
    (GOLD_ORE, "gold_ore", "Gold Ore", "Aranyérc"),
    (DIAMOND_ORE, "diamond_ore", "Diamond Ore", "Gyémántérc"),
    (COAL_BLOCK, "coal_block", "Block of Coal", "Szénblokk"),
    (COPPER_BLOCK, "copper_block", "Block of Copper", "Rézblokk"),
    (IRON_BLOCK, "iron_block", "Block of Iron", "Vasblokk"),
    (GOLD_BLOCK, "gold_block", "Block of Gold", "Aranyblokk"),
    (
        DIAMOND_BLOCK,
        "diamond_block",
        "Block of Diamond",
        "Gyémántblokk",
    ),
    (OBSIDIAN, "obsidian", "Obsidian", "Obszidián"),
    (BEDROCK, "bedrock", "Bedrock", "Alapkő"),
    (
        CRAFTING_TABLE,
        "crafting_table",
        "Crafting Table",
        "Barkácsasztal",
    ),
    (FURNACE, "furnace", "Furnace", "Kemence"),
    (BLAST_FURNACE, "blast_furnace", "Blast Furnace", "Kohó"),
    (ADV_FURNACE, "advanced_furnace", "Advanced Furnace", "Fejlett kohó"),
    (CHEST, "chest", "Chest", "Láda"),
    (TORCH, "torch", "Torch", "Fáklya"),
    (LANTERN, "lantern", "Lantern", "Lámpás"),
    (WOOL, "white_wool", "White Wool", "Fehér gyapjú"),
    (BED, "red_bed", "Red Bed", "Piros ágy"),
    (GUN_STATION, "gun_station", "Gun Station", "Fegyverasztal"),
];

/// The other items (ids from 256, tools aside), in creative inventory order: the id, key,
/// English and Hungarian names, and the icon's texture layer.
const ITEMS: &[(ItemId, &str, &str, &str, u32)] = &[
    (STICK, "stick", "Stick", "Bot", tex::STICK),
    (COAL, "coal", "Coal", "Szén", tex::COAL),
    (CHARCOAL, "charcoal", "Charcoal", "Faszén", tex::CHARCOAL),
    (
        IRON_INGOT,
        "iron_ingot",
        "Iron Ingot",
        "Vasrúd",
        tex::IRON_INGOT,
    ),
    (
        IRON_NUGGET,
        "iron_nugget",
        "Iron Nugget",
        "Vasrög",
        tex::IRON_NUGGET,
    ),
    (
        COPPER_INGOT,
        "copper_ingot",
        "Copper Ingot",
        "Rézrúd",
        tex::COPPER_INGOT,
    ),
    (
        STEEL_INGOT,
        "steel_ingot",
        "Steel Ingot",
        "Acélrúd",
        tex::STEEL_INGOT,
    ),
    (
        CERAMIC_PLATE,
        "ceramic_plate",
        "Ceramic Plate",
        "Kerámialap",
        tex::CERAMIC_PLATE,
    ),
    (
        FRAG_GRENADE,
        "frag_grenade",
        "Frag Grenade",
        "Repeszgránát",
        tex::FRAG_GRENADE,
    ),
    (
        SMOKE_GRENADE,
        "smoke_grenade",
        "Smoke Grenade",
        "Füstgránát",
        tex::SMOKE_GRENADE,
    ),
    (
        ARMOR_BASE + 0,
        "wool_helmet",
        "Wool Helmet",
        "Posztó sisak",
        tex::ARMOR_ICONS + 0,
    ),
    (
        ARMOR_BASE + 1,
        "wool_chestplate",
        "Wool Chestplate",
        "Posztó mellvért",
        tex::ARMOR_ICONS + 1,
    ),
    (
        ARMOR_BASE + 2,
        "wool_leggings",
        "Wool Leggings",
        "Posztó lábvért",
        tex::ARMOR_ICONS + 2,
    ),
    (
        ARMOR_BASE + 3,
        "wool_boots",
        "Wool Boots",
        "Posztó csizma",
        tex::ARMOR_ICONS + 3,
    ),
    (
        ARMOR_BASE + 4,
        "copper_helmet",
        "Copper Helmet",
        "Réz sisak",
        tex::ARMOR_ICONS + 4,
    ),
    (
        ARMOR_BASE + 5,
        "copper_chestplate",
        "Copper Chestplate",
        "Réz mellvért",
        tex::ARMOR_ICONS + 5,
    ),
    (
        ARMOR_BASE + 6,
        "copper_leggings",
        "Copper Leggings",
        "Réz lábvért",
        tex::ARMOR_ICONS + 6,
    ),
    (
        ARMOR_BASE + 7,
        "copper_boots",
        "Copper Boots",
        "Réz csizma",
        tex::ARMOR_ICONS + 7,
    ),
    (
        ARMOR_BASE + 8,
        "steel_helmet",
        "Steel Helmet",
        "Acél sisak",
        tex::ARMOR_ICONS + 8,
    ),
    (
        ARMOR_BASE + 9,
        "steel_chestplate",
        "Steel Chestplate",
        "Acél mellvért",
        tex::ARMOR_ICONS + 9,
    ),
    (
        ARMOR_BASE + 10,
        "steel_leggings",
        "Steel Leggings",
        "Acél lábvért",
        tex::ARMOR_ICONS + 10,
    ),
    (
        ARMOR_BASE + 11,
        "steel_boots",
        "Steel Boots",
        "Acél csizma",
        tex::ARMOR_ICONS + 11,
    ),
    (
        ARMOR_BASE + 12,
        "diamond_helmet",
        "Diamond Helmet",
        "Gyémánt sisak",
        tex::ARMOR_ICONS + 12,
    ),
    (
        ARMOR_BASE + 13,
        "diamond_chestplate",
        "Diamond Chestplate",
        "Gyémánt mellvért",
        tex::ARMOR_ICONS + 13,
    ),
    (
        ARMOR_BASE + 14,
        "diamond_leggings",
        "Diamond Leggings",
        "Gyémánt lábvért",
        tex::ARMOR_ICONS + 14,
    ),
    (
        ARMOR_BASE + 15,
        "diamond_boots",
        "Diamond Boots",
        "Gyémánt csizma",
        tex::ARMOR_ICONS + 15,
    ),
    (
        BULLETPROOF_VEST,
        "bulletproof_vest",
        "Bulletproof Vest",
        "Golyóálló mellény",
        tex::ARMOR_ICONS + 16,
    ),
    (
        GOLD_INGOT,
        "gold_ingot",
        "Gold Ingot",
        "Aranyrúd",
        tex::GOLD_INGOT,
    ),
    (DIAMOND, "diamond", "Diamond", "Gyémánt", tex::DIAMOND),
    (
        CLAY_BALL,
        "clay_ball",
        "Clay Ball",
        "Agyaggolyó",
        tex::CLAY_BALL,
    ),
    (BRICK, "brick", "Brick", "Tégla", tex::BRICK),
    (BUCKET, "bucket", "Bucket", "Vödör", tex::BUCKET),
    (
        WATER_BUCKET,
        "water_bucket",
        "Water Bucket",
        "Vizesvödör",
        tex::WATER_BUCKET,
    ),
    (
        LAVA_BUCKET,
        "lava_bucket",
        "Lava Bucket",
        "Lávás vödör",
        tex::LAVA_BUCKET,
    ),
    (
        PORKCHOP,
        "porkchop",
        "Raw Porkchop",
        "Nyers disznóhús",
        tex::PORKCHOP,
    ),
    (
        COOKED_PORKCHOP,
        "cooked_porkchop",
        "Cooked Porkchop",
        "Sült disznóhús",
        tex::COOKED_PORKCHOP,
    ),
    (MUTTON, "mutton", "Raw Mutton", "Nyers birkahús", tex::MUTTON),
    (
        COOKED_MUTTON,
        "cooked_mutton",
        "Cooked Mutton",
        "Sült birkahús",
        tex::COOKED_MUTTON,
    ),
    (
        HALF_COOKED_PORKCHOP,
        "half_cooked_porkchop",
        "Half-Cooked Porkchop",
        "Félig sült disznóhús",
        tex::HALF_COOKED_PORKCHOP,
    ),
    (
        HALF_COOKED_MUTTON,
        "half_cooked_mutton",
        "Half-Cooked Mutton",
        "Félig sült birkahús",
        tex::HALF_COOKED_MUTTON,
    ),
    (
        BURNT_PORKCHOP,
        "burnt_porkchop",
        "Burnt Porkchop",
        "Szenes disznóhús",
        tex::BURNT_PORKCHOP,
    ),
    (
        BURNT_MUTTON,
        "burnt_mutton",
        "Burnt Mutton",
        "Szenes birkahús",
        tex::BURNT_MUTTON,
    ),
    (
        HALF_BURNT_PORKCHOP,
        "half_burnt_porkchop",
        "Half-Burnt Porkchop",
        "Félig szenes disznóhús",
        tex::HALF_BURNT_PORKCHOP,
    ),
    (
        HALF_BURNT_MUTTON,
        "half_burnt_mutton",
        "Half-Burnt Mutton",
        "Félig szenes birkahús",
        tex::HALF_BURNT_MUTTON,
    ),
    (
        RAW_BURNT_PORKCHOP,
        "raw_burnt_porkchop",
        "Burnt-Raw Porkchop",
        "Szenes-nyers disznóhús",
        tex::RAW_BURNT_PORKCHOP,
    ),
    (
        RAW_BURNT_MUTTON,
        "raw_burnt_mutton",
        "Burnt-Raw Mutton",
        "Szenes-nyers birkahús",
        tex::RAW_BURNT_MUTTON,
    ),
    (SHEARS, "shears", "Shears", "Olló", tex::SHEARS),
    (
        GLASS_BOTTLE,
        "glass_bottle",
        "Glass Bottle",
        "Üvegpalack",
        tex::GLASS_BOTTLE,
    ),
    (
        WATER_BOTTLE,
        "water_bottle",
        "Water Bottle",
        "Vizes üveg",
        tex::WATER_BOTTLE,
    ),
    (
        PURIFIED_WATER,
        "purified_water",
        "Boiled Water",
        "Forralt víz",
        tex::PURIFIED_WATER,
    ),
    (
        PIG_SPAWN_EGG,
        "pig_spawn_egg",
        "Pig Spawn Egg",
        "Disznó idéző tojás",
        tex::PIG_SPAWN_EGG,
    ),
    (
        SHEEP_SPAWN_EGG,
        "sheep_spawn_egg",
        "Sheep Spawn Egg",
        "Birka idéző tojás",
        tex::SHEEP_SPAWN_EGG,
    ),
    (PISTOL, "pistol", "Pistol", "Pisztoly", tex::PISTOL),
    (BULLET, "bullet", "Bullet", "Töltény", tex::BULLET),
    (
        PISTOL_FRAME,
        "pistol_frame",
        "Pistol Frame",
        "Pisztolyváz",
        tex::PISTOL_PARTS,
    ),
    (
        PISTOL_BARREL,
        "pistol_barrel",
        "Pistol Barrel",
        "Pisztolycső",
        tex::PISTOL_PARTS + 1,
    ),
    (
        PISTOL_SPRING,
        "pistol_spring",
        "Recoil Spring",
        "Visszatérítő rugó",
        tex::PISTOL_PARTS + 2,
    ),
    (
        PISTOL_SLIDE,
        "pistol_slide",
        "Pistol Slide",
        "Pisztolyszán",
        tex::PISTOL_PARTS + 3,
    ),
    (
        PISTOL_MAGAZINE,
        "pistol_magazine",
        "Pistol Magazine",
        "Pisztolytár",
        tex::PISTOL_PARTS + 4,
    ),
    (
        DESERT_EAGLE,
        "desert_eagle",
        "Desert Eagle",
        "Desert Eagle",
        tex::GUN_ICONS,
    ),
    (M16, "m16", "M16 Rifle", "M16 gépkarabély", tex::GUN_ICONS + 1),
    (
        SNIPER_RIFLE,
        "sniper_rifle",
        "Sniper Rifle",
        "Mesterlövész puska",
        tex::GUN_ICONS + 2,
    ),
    (SHOTGUN, "shotgun", "Shotgun", "Sörétes puska", tex::GUN_ICONS + 3),
    (
        MAGNUM_ROUND,
        "magnum_round",
        ".50 AE Round",
        ".50 AE töltény",
        tex::AMMO_ICONS,
    ),
    (
        RIFLE_ROUND,
        "rifle_round",
        "5.56 mm Round",
        "5.56 mm-es töltény",
        tex::AMMO_ICONS + 1,
    ),
    (
        BMG_ROUND,
        "bmg_round",
        ".50 BMG Round",
        ".50 BMG töltény",
        tex::AMMO_ICONS + 2,
    ),
    (
        SHOTGUN_SHELL,
        "shotgun_shell",
        "Shotgun Shell",
        "Sörétes patron",
        tex::AMMO_ICONS + 3,
    ),
    (SCOPE, "scope", "Scope", "Távcső", tex::GUN_ATTACHMENTS),
    (
        SILENCER,
        "silencer",
        "Silencer",
        "Hangtompító",
        tex::GUN_ATTACHMENTS + 1,
    ),
    (
        EXTENDED_MAGAZINE,
        "extended_magazine",
        "Extended Magazine",
        "Bővített tár",
        tex::GUN_ATTACHMENTS + 2,
    ),
    (
        LASER_SIGHT,
        "laser_sight",
        "Laser Sight",
        "Lézeres célzó",
        tex::GUN_ATTACHMENTS + 3,
    ),
];

/// Creative inventory order: blocks, other items, then the tools.
pub fn all_items() -> Vec<ItemId> {
    let mut v: Vec<ItemId> = BLOCK_ITEMS.iter().map(|e| e.0 as ItemId).collect();
    v.extend(ITEMS.iter().map(|e| e.0));
    for tier in TIER_ORDER {
        for kind in KINDS {
            v.push(tool_id(kind, tier));
        }
    }
    v
}

fn block_entry(id: ItemId) -> Option<&'static (u8, &'static str, &'static str, &'static str)> {
    BLOCK_ITEMS.iter().find(|e| id < 256 && e.0 as ItemId == id)
}

fn item_entry(
    id: ItemId,
) -> Option<&'static (ItemId, &'static str, &'static str, &'static str, u32)> {
    ITEMS.iter().find(|e| e.0 == id)
}

/// The block this item places (base variant for directional blocks).
pub fn block_of(id: ItemId) -> Option<u8> {
    block_entry(id).map(|e| e.0)
}

/// The item a placed block counts as (pick block / creative).
pub fn item_of_block(b: u8) -> Option<ItemId> {
    let base = match b {
        _ if furnace_base(b).is_some() => furnace_base(b).unwrap(),
        _ if is_chest(b) => CHEST,
        _ if is_torch(b) => TORCH,
        _ if is_lantern(b) => LANTERN,
        _ if is_door(b) => OAK_DOOR,
        _ if is_stairs(b) => OAK_STAIRS,
        _ if is_bed(b) => BED,
        _ if is_log(b) => log_base(b),
        _ if is_water(b) => return Some(WATER_BUCKET),
        _ if is_lava(b) => return Some(LAVA_BUCKET),
        _ => b,
    };
    block_of(base as ItemId).map(|_| base as ItemId)
}

pub enum Icon {
    /// Drawn as an isometric cube.
    Block(u8),
    /// Drawn as a flat sprite from this texture layer.
    Flat(u32),
}

pub fn icon(id: ItemId) -> Icon {
    if let Some(b) = block_of(id) {
        if b == LANTERN {
            return Icon::Flat(tex::LANTERN_ITEM);
        }
        if b == OAK_DOOR {
            return Icon::Flat(tex::DOOR_ITEM);
        }
        if b == BED {
            return Icon::Flat(tex::BED_ITEM);
        }
        if is_plant(b) || b == TORCH {
            return Icon::Flat(face_texture(b, 0));
        }
        return Icon::Block(b);
    }
    Icon::Flat(match (item_entry(id), tool_of(id)) {
        (Some(e), _) => e.4,
        (None, Some((k, t))) => crate::world::textures::tool_layer(t as usize, k as usize),
        (None, None) => tex::STONE,
    })
}

/// Stable identifier used by /give and save files.
pub fn key(id: ItemId) -> String {
    if let Some((k, t)) = tool_of(id) {
        let tier = ["wooden", "stone", "iron", "golden", "diamond", "copper"][t as usize];
        let kind = ["pickaxe", "axe", "shovel", "sword"][k as usize];
        return format!("{tier}_{kind}");
    }
    let key = match (block_entry(id), item_entry(id)) {
        (Some(e), _) => e.1,
        (None, Some(e)) => e.1,
        (None, None) => "unknown",
    };
    key.to_string()
}

pub fn from_key(k: &str) -> Option<ItemId> {
    let k = k.strip_prefix("minecraft:").unwrap_or(k);
    all_items().into_iter().find(|&id| key(id) == k)
}

/// Display name in the current language.
pub fn name(id: ItemId) -> String {
    let hu = is_hungarian();
    if let Some((k, t)) = tool_of(id) {
        if hu {
            let tier = ["Fa", "Kő", "Vas", "Arany", "Gyémánt", "Réz"][t as usize];
            let kind = ["csákány", "balta", "ásó", "kard"][k as usize];
            return format!("{tier}{kind}");
        }
        let tier = ["Wooden", "Stone", "Iron", "Golden", "Diamond", "Copper"][t as usize];
        let kind = ["Pickaxe", "Axe", "Shovel", "Sword"][k as usize];
        return format!("{tier} {kind}");
    }
    let (en, hun) = match (block_entry(id), item_entry(id)) {
        (Some(e), _) => (e.2, e.3),
        (None, Some(e)) => (e.2, e.3),
        (None, None) => ("Unknown", "Ismeretlen"),
    };
    (if hu { hun } else { en }).to_string()
}

/// Display name of a block in the world (for debug info).
pub fn block_name(b: u8) -> String {
    match b {
        AIR => "-".into(),
        _ if is_water(b) => (if is_hungarian() { "Víz" } else { "Water" }).into(),
        _ if is_lava(b) => (if is_hungarian() { "Láva" } else { "Lava" }).into(),
        _ => item_of_block(b).map(name).unwrap_or_else(|| "?".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_item_has_a_unique_key_and_a_name() {
        let all = all_items();
        let mut keys = std::collections::HashSet::new();
        for &id in &all {
            let k = key(id);
            assert_ne!(k, "unknown", "item {id} has no key");
            assert!(keys.insert(k.clone()), "duplicate key {k}");
            assert_eq!(from_key(&k), Some(id));
            assert_ne!(name(id), "Unknown", "item {id} has no name");
        }
        assert_eq!(all.len(), BLOCK_ITEMS.len() + ITEMS.len() + 4 * TIERS.len());
        // Keys stored in save files must not change.
        assert_eq!(key(GRASS as ItemId), "grass_block");
        assert_eq!(key(PURIFIED_WATER), "purified_water");
        assert_eq!(
            key(tool_id(ToolKind::Pickaxe, Tier::Diamond)),
            "diamond_pickaxe"
        );
        assert!(matches!(icon(IRON_NUGGET), Icon::Flat(l) if l == tex::IRON_NUGGET));
    }
}
