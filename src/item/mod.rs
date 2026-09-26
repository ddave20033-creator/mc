//! Items: ids, names, icons, stacking and tools; mining rules, crafting and smelting are in
//! the submodules.
//!
//! Block items share the block's id (0..=255); other items start at 256.

pub mod crafting;
pub mod inventory;
pub mod mining;

pub use crafting::*;
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
pub const FLINT: ItemId = 274;
pub const GUNPOWDER: ItemId = 275;
/// Lights TNT; wears out like a tool.
pub const FLINT_AND_STEEL: ItemId = 276;
pub const CREEPER_SPAWN_EGG: ItemId = 277;
const TOOL_BASE: ItemId = 300;

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
}

const TIERS: [Tier; 5] = [
    Tier::Wood,
    Tier::Stone,
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
            Tier::Iron => 6.0,
            Tier::Gold => 12.0,
            Tier::Diamond => 8.0,
        }
    }
    /// Harvest level: which ores this tier can mine.
    pub fn level(self) -> u8 {
        match self {
            Tier::Wood | Tier::Gold => 0,
            Tier::Stone => 1,
            Tier::Iron => 2,
            Tier::Diamond => 3,
        }
    }
    pub fn durability(self) -> u16 {
        match self {
            Tier::Wood => 59,
            Tier::Stone => 131,
            Tier::Iron => 250,
            Tier::Gold => 32,
            Tier::Diamond => 1561,
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
        }
    }
}

pub fn tool_id(kind: ToolKind, tier: Tier) -> ItemId {
    TOOL_BASE + tier as ItemId * 4 + kind as ItemId
}

/// Swords can block (right mouse button held), like in Minecraft 1.8.
pub fn is_sword(id: ItemId) -> bool {
    matches!(tool_of(id), Some((ToolKind::Sword, _)))
}

pub fn tool_of(id: ItemId) -> Option<(ToolKind, Tier)> {
    if (TOOL_BASE..TOOL_BASE + 20).contains(&id) {
        let i = id - TOOL_BASE;
        Some((KINDS[(i % 4) as usize], TIERS[(i / 4) as usize]))
    } else {
        None
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Stack {
    pub item: ItemId,
    pub count: u8,
    /// Durability used up (tools).
    pub damage: u16,
}

impl Stack {
    pub fn new(item: ItemId, count: u8) -> Self {
        Self {
            item,
            count,
            damage: 0,
        }
    }
    pub fn one(item: ItemId) -> Self {
        Self::new(item, 1)
    }
    pub fn stacks_with(&self, other: &Stack) -> bool {
        self.item == other.item && self.damage == other.damage && max_stack(self.item) > 1
    }
}

pub type Slot = Option<Stack>;

pub fn max_stack(id: ItemId) -> u8 {
    match id {
        _ if tool_of(id).is_some() => 1,
        FLINT_AND_STEEL => 1,
        WATER_BUCKET | LAVA_BUCKET => 1,
        BUCKET | WATER_BOTTLE | PURIFIED_WATER => 16,
        _ => 64,
    }
}

/// Food and drink: what eating or drinking this restores (Minecraft's food values).
pub fn consumable(id: ItemId) -> Option<crate::entity::survival::Consumable> {
    use crate::entity::survival::Consumable;
    let food = |food, saturation| Consumable {
        food,
        saturation,
        thirst: 0.0,
        dirty: false,
        drink: false,
    };
    let drink = |thirst, dirty| Consumable {
        food: 0.0,
        saturation: 0.0,
        thirst,
        dirty,
        drink: true,
    };
    Some(match id {
        PORKCHOP => food(3.0, 1.8),
        COOKED_PORKCHOP => food(8.0, 12.8),
        WATER_BOTTLE => drink(6.0, true),
        PURIFIED_WATER => drink(10.0, false),
        _ => return None,
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
        Tier::Iron => 2.0,
        Tier::Diamond => 3.0,
    };
    base + bonus
}

pub fn max_damage(id: ItemId) -> u16 {
    if id == FLINT_AND_STEEL {
        return 64;
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
    (IRON_ORE, "iron_ore", "Iron Ore", "Vasérc"),
    (GOLD_ORE, "gold_ore", "Gold Ore", "Aranyérc"),
    (DIAMOND_ORE, "diamond_ore", "Diamond Ore", "Gyémántérc"),
    (COAL_BLOCK, "coal_block", "Block of Coal", "Szénblokk"),
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
    (CHEST, "chest", "Chest", "Láda"),
    (TORCH, "torch", "Torch", "Fáklya"),
    (LANTERN, "lantern", "Lantern", "Lámpás"),
    (TNT, "tnt", "TNT", "TNT"),
];

/// The other items (ids from 256, tools aside), in creative inventory order: the id, key,
/// English and Hungarian names, and the icon's texture layer.
const ITEMS: &[(ItemId, &str, &str, &str, u32)] = &[
    (STICK, "stick", "Stick", "Bot", tex::STICK),
    (COAL, "coal", "Coal", "Szén", tex::COAL),
    (CHARCOAL, "charcoal", "Charcoal", "Faszén", tex::CHARCOAL),
    (FLINT, "flint", "Flint", "Kovakő", tex::FLINT),
    (
        GUNPOWDER,
        "gunpowder",
        "Gunpowder",
        "Puskapor",
        tex::GUNPOWDER,
    ),
    (
        FLINT_AND_STEEL,
        "flint_and_steel",
        "Flint and Steel",
        "Kovakő és acél",
        tex::FLINT_AND_STEEL,
    ),
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
        CREEPER_SPAWN_EGG,
        "creeper_spawn_egg",
        "Creeper Spawn Egg",
        "Creeper idéző tojás",
        tex::CREEPER_SPAWN_EGG,
    ),
];

/// Creative inventory order: blocks, other items, then the tools.
pub fn all_items() -> Vec<ItemId> {
    let mut v: Vec<ItemId> = BLOCK_ITEMS.iter().map(|e| e.0 as ItemId).collect();
    v.extend(ITEMS.iter().map(|e| e.0));
    for tier in TIERS {
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
        _ if is_furnace(b) => FURNACE,
        _ if is_chest(b) => CHEST,
        _ if is_torch(b) => TORCH,
        _ if is_lantern(b) => LANTERN,
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
        if is_plant(b) || b == TORCH {
            return Icon::Flat(face_texture(b, 0));
        }
        return Icon::Block(b);
    }
    Icon::Flat(match (item_entry(id), tool_of(id)) {
        (Some(e), _) => e.4,
        (None, Some((k, t))) => tex::TOOLS + t as u32 * 4 + k as u32,
        (None, None) => tex::STONE,
    })
}

/// Stable identifier used by /give and save files.
pub fn key(id: ItemId) -> String {
    if let Some((k, t)) = tool_of(id) {
        let tier = ["wooden", "stone", "iron", "golden", "diamond"][t as usize];
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
            let tier = ["Fa", "Kő", "Vas", "Arany", "Gyémánt"][t as usize];
            let kind = ["csákány", "balta", "ásó", "kard"][k as usize];
            return format!("{tier}{kind}");
        }
        let tier = ["Wooden", "Stone", "Iron", "Golden", "Diamond"][t as usize];
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
        FIRE => (if is_hungarian() { "Tűz" } else { "Fire" }).into(),
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
        assert_eq!(all.len(), BLOCK_ITEMS.len() + ITEMS.len() + 20);
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
