//! Every item that is not a block: a line each in the files of this folder, by kind
//! (materials, tools, food, misc, armor, guns, ammo). A line gives an item its constant, its
//! key (commands, save files), its names, its icon, how many stack, how long it lasts, what it
//! does to a mob, what it is as a tool, armor, food, fuel, something to smelt, a gun's part or
//! a grenade, what the right mouse button does with it (`OnUse`), and where it is in the
//! creative inventory. Code of its own an item has where its `OnUse` says (the game's side of
//! using items, `client::player::items`).
//!
//! The blocks' items are the blocks' own (`content::blocks`, `BlockItem::Own`): their lines
//! here are made from the blocks' table, with the block's id as theirs.
//!
//! Ids are not written anywhere: they follow from the order of the lines (and of the files,
//! see `PARTS`), from `FIRST_ITEM` on. Save files keep the items' keys (`save`), so the order
//! can change. Adding an item is adding a line to its file (and its icon: `tex`, a painter in
//! `textures::procedural` and a resource pack name in `textures::pack`); a recipe for it goes
//! in `item::crafting`.

pub use crate::content::Creative;
use crate::content::blocks::{BlockDef, BlockItem, BLOCKS, BLOCK_IDS};
use crate::entity::survival::{Consumable, Sickness};
use crate::sim::grenade::GrenadeKind;
use crate::world::textures::tex;
use crate::world::Block;
use std::collections::HashMap;
use std::sync::OnceLock;

pub type ItemId = u16;

pub const NONE: ItemId = 0;
/// The first id of an item that is not a block (the blocks' ids are below it).
pub const FIRST_ITEM: ItemId = 1024;

/// How an item is drawn in a slot.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Icon {
    /// Drawn as an isometric cube.
    Block(Block),
    /// Drawn as a flat sprite from this texture layer.
    Flat(u32),
}

/// What an item smelts into in a furnace, and the furnace it needs: 1 the furnace (food,
/// charcoal, bricks, copper, stone), 2 the blast furnace (iron, glass, steel), 3 the advanced
/// furnace (gold, diamond, ceramic plates).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Smelt {
    pub into: ItemId,
    pub furnace: u8,
}

pub const fn smelts(into: ItemId, furnace: u8) -> Option<Smelt> {
    Some(Smelt { into, furnace })
}

/// A piece of armor: the slot it is worn in (`ARMOR_SLOTS`; the vest its own) and its
/// material (`MATERIALS`, weakest first; the vest 0).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Armor {
    pub slot: usize,
    pub material: usize,
}

/// What an item is to a gun. A gun, its parts and its magazines get as dirty as the gun
/// (its `Stats::dirt_max`; taken apart, each is cleaned on its own).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum GunRole {
    None,
    /// The gun itself.
    Gun(GunKind),
    /// One of the parts it goes together from (not its magazine).
    Part(GunKind),
    /// A magazine that goes into it (its standard or its extended one).
    Magazine(GunKind),
    /// Loads it all at once (the revolver's speedloader); its data is the rounds in it.
    Loader(GunKind),
    /// Fitted on a gun at the gun station: its `gun_mod` bit.
    Attachment(u8),
}

/// What holding the item and pressing the right mouse button does (`client::player::items`).
/// Eating and drinking go by `food` (the button held).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum OnUse {
    /// Nothing of its own: it opens what it points at, or is used on the mob.
    None,
    /// Places its block.
    Place,
    /// Aims (a gun): nothing is opened or placed with it.
    Aim,
    /// Readied and thrown by holding the button (a grenade, `client::tools::grenades`).
    Throw,
    /// Cast by holding the button (the fishing rod, `client::tools::fishing`), unless there is
    /// something to open.
    Cast,
    /// Nothing at all (a magazine: it is loaded at the gun station).
    Nothing,
    /// Put on, swapped with what is worn in its slot.
    Wear,
    /// Held up to block, Minecraft 1.8's swords (the button held).
    Guard,
    /// Scoops up a still fluid (the empty bucket).
    Scoop,
    /// Pours out this fluid (a full bucket).
    Pour(Block),
    /// Fills with water (the glass bottle).
    FillBottle,
    /// Puts its mob into the world (a spawn egg, the target dummy: `MobDef::egg`).
    Spawn,
}

/// An item: one line of the table.
#[derive(Clone, Copy)]
pub struct ItemDef {
    pub id: ItemId,
    pub key: &'static str,
    pub en: &'static str,
    pub hu: &'static str,
    pub icon: Icon,
    /// How many go in a slot.
    pub stack: u8,
    /// How long it lasts (tools, shears, the fishing rod, armor; 0: it does not wear). A gun's
    /// things get dirty instead (`GunRole`).
    pub durability: u16,
    /// Damage dealt hitting a mob with it (Minecraft 1.8 values; 1 = bare hand).
    pub attack: f32,
    pub tool: Option<(ToolKind, Tier)>,
    pub armor: Option<Armor>,
    /// Eaten or drunk: what it restores and what it can do to you.
    pub food: Option<Consumable>,
    /// Seconds it burns in a furnace (one item smelts in 10 s).
    pub fuel: Option<f32>,
    pub smelt: Option<Smelt>,
    pub gun: GunRole,
    pub grenade: Option<GrenadeKind>,
    pub on_use: OnUse,
    /// The block it places (a block's item: its block).
    pub block: Option<Block>,
    pub creative: Creative,
}

/// A plain item: stacks up to 64, does not wear, hits like a hand, does nothing of its own.
/// Every line starts from it (or from a file's own preset built on it).
pub(super) const ITEM: ItemDef = ItemDef {
    id: 0,
    key: "",
    en: "",
    hu: "",
    icon: Icon::Flat(tex::STONE),
    stack: 64,
    durability: 0,
    attack: 1.0,
    tool: None,
    armor: None,
    food: None,
    fuel: None,
    smelt: None,
    gun: GunRole::None,
    grenade: None,
    on_use: OnUse::None,
    block: None,
    creative: Creative::None,
};

/// One to a slot (tools, guns, buckets of something...).
pub(super) const SINGLE: ItemDef = ItemDef { stack: 1, ..ITEM };

/// An id that is no item.
const NO_ITEM: ItemDef = ItemDef { key: "unknown", en: "Unknown", hu: "Ismeretlen", ..ITEM };

/// Declares a file's items: `after PREVIOUS_END; NAME = ItemDef { .. }; ...`. Each gets the
/// constant `NAME` (the id after the previous item's, the first after the last of the file
/// before), and its line in the file's `DEFS`; `END` is the id after its last.
macro_rules! items {
    (after $prev:expr; $( $(#[$m:meta])* $name:ident = $def:expr; )*) => {
        items!(@ids $prev; $( $(#[$m])* $name; )*);
        pub(super) const DEFS: &[ItemDef] = &[$( ItemDef { id: $name, ..$def } ),*];
    };
    (@ids $at:expr; ) => {
        pub(super) const END: ItemId = $at;
    };
    (@ids $at:expr; $(#[$m:meta])* $name:ident; $($rest:tt)*) => {
        $(#[$m])*
        pub const $name: ItemId = $at;
        items!(@ids $name + 1; $($rest)*);
    };
}

pub mod ammo;
pub mod armor;
pub mod food;
pub mod guns;
pub mod materials;
pub mod misc;
pub mod tools;

pub use ammo::*;
pub use armor::*;
pub use food::*;
pub use guns::*;
pub use materials::*;
pub use misc::*;
pub use tools::*;


/// The files' items, in the order of their ids.
const PARTS: [&[ItemDef]; 7] =
    [materials::DEFS, tools::DEFS, food::DEFS, misc::DEFS, armor::DEFS, guns::DEFS, ammo::DEFS];
const ITEM_TYPES: usize = {
    let (mut n, mut i) = (0, 0);
    while i < PARTS.len() {
        n += PARTS[i].len();
        i += 1;
    }
    n
};
const ALL: [ItemDef; ITEM_TYPES] = {
    let mut out = [ITEM; ITEM_TYPES];
    let (mut n, mut i) = (0, 0);
    while i < PARTS.len() {
        let mut j = 0;
        while j < PARTS[i].len() {
            out[n] = PARTS[i][j];
            n += 1;
            j += 1;
        }
        i += 1;
    }
    out
};
/// Every item that is not a block, in the order of their ids (from `FIRST_ITEM`).
pub static ITEMS: [ItemDef; ITEM_TYPES] = ALL;

const _: () = {
    assert!(BLOCK_IDS <= FIRST_ITEM as usize);
    assert!(ammo::END as usize == FIRST_ITEM as usize + ITEM_TYPES);
};

/// A block's item line, made from the block's.
const fn block_item(d: &BlockDef) -> ItemDef {
    let stack = match d.item {
        BlockItem::Own { stack } => stack,
        _ => 0,
    };
    ItemDef {
        id: d.id,
        key: d.key,
        en: d.en,
        hu: d.hu,
        icon: match d.icon {
            Some(l) => Icon::Flat(l),
            None => Icon::Block(d.id),
        },
        stack,
        fuel: d.fuel,
        smelt: d.smelt,
        on_use: OnUse::Place,
        block: Some(d.id),
        creative: d.creative,
        ..ITEM
    }
}

/// The blocks' item lines (one for each line of the blocks' table; used for the blocks that
/// are items of their own).
static BLOCK_ITEMS: [ItemDef; BLOCKS.len()] = {
    let mut out = [ITEM; BLOCKS.len()];
    let mut i = 0;
    while i < BLOCKS.len() {
        out[i] = block_item(&BLOCKS[i]);
        i += 1;
    }
    out
};
/// For each block id, 1 + its line in `BLOCK_ITEMS` when it is an item of its own (the
/// block's first id only), 0 otherwise.
static BLOCK_ITEM_OF: [u8; BLOCK_IDS] = {
    let mut t = [0u8; BLOCK_IDS];
    let mut i = 0;
    while i < BLOCKS.len() {
        assert!(i < 255);
        if let BlockItem::Own { .. } = BLOCKS[i].item {
            t[BLOCKS[i].id as usize] = i as u8 + 1;
        }
        i += 1;
    }
    t
};

/// An item's line (a block's item too); None for an id that is no item.
pub fn item_def(id: ItemId) -> Option<&'static ItemDef> {
    if id >= FIRST_ITEM {
        ITEMS.get((id - FIRST_ITEM) as usize)
    } else {
        match BLOCK_ITEM_OF.get(id as usize).copied() {
            Some(n) if n > 0 => Some(&BLOCK_ITEMS[n as usize - 1]),
            _ => None,
        }
    }
}

/// An item's line; the unknown item's for an id that is no item.
fn def(id: ItemId) -> &'static ItemDef {
    item_def(id).unwrap_or(&NO_ITEM)
}

/// How many go in a slot.
pub fn max_stack(id: ItemId) -> u8 {
    def(id).stack
}

/// How long it lasts (tools, shears, the fishing rod, armor), or how dirty a gun, its parts
/// and magazines can get; 0: it does not wear.
pub fn max_damage(id: ItemId) -> u16 {
    let d = def(id);
    match d.gun {
        GunRole::Gun(k) | GunRole::Part(k) | GunRole::Magazine(k) => k.stats().dirt_max,
        _ => d.durability,
    }
}

/// Damage dealt when hitting a mob with this item (Minecraft 1.8 values; 1 = bare hand).
pub fn attack_damage(id: ItemId) -> f32 {
    def(id).attack
}

/// Food and drink: what eating or drinking this restores (Minecraft's food values), and
/// what it can do to you.
pub fn consumable(id: ItemId) -> Option<Consumable> {
    def(id).food
}

/// Seconds a fuel item burns (one item smelts in 10 s).
pub fn fuel_time(id: ItemId) -> Option<f32> {
    def(id).fuel
}

/// What `id` smelts into, whatever the furnace (see `smelt_tier`). Meat is not smelted but
/// grilled on the furnace's top (`meat`).
pub fn smelt(id: ItemId) -> Option<ItemId> {
    def(id).smelt.map(|s| s.into)
}

/// The furnace `smelt(id)` needs: 1 the furnace, 2 the blast furnace, 3 the advanced furnace
/// (1 for what does not smelt).
pub fn smelt_tier(id: ItemId) -> u8 {
    def(id).smelt.map_or(1, |s| s.furnace)
}

/// A tool's kind and tier.
pub fn tool_of(id: ItemId) -> Option<(ToolKind, Tier)> {
    def(id).tool
}

/// Swords can block (right mouse button held), like in Minecraft 1.8.
pub fn is_sword(id: ItemId) -> bool {
    def(id).on_use == OnUse::Guard
}

/// What the right mouse button does with it.
pub fn on_use(id: ItemId) -> OnUse {
    def(id).on_use
}

/// What it is to a gun.
pub fn gun_role(id: ItemId) -> GunRole {
    def(id).gun
}

/// Where it is in the creative inventory.
pub fn creative(id: ItemId) -> Creative {
    def(id).creative
}

pub fn icon(id: ItemId) -> Icon {
    def(id).icon
}

/// The block this item places (base variant for directional blocks).
pub fn block_of(id: ItemId) -> Option<Block> {
    def(id).block
}

/// The item a placed block counts as (pick block / creative).
pub fn item_of_block(b: Block) -> Option<ItemId> {
    let d = crate::content::blocks::def(b);
    match d.item {
        BlockItem::Own { .. } => Some(d.id as ItemId),
        BlockItem::As(other) => Some(other as ItemId),
        BlockItem::Other(item) => Some(item),
        BlockItem::None => None,
    }
}

/// The blocks that are items (their own), in the order of the blocks' table.
pub fn block_items() -> impl Iterator<Item = &'static BlockDef> {
    BLOCKS.iter().filter(|d| matches!(d.item, BlockItem::Own { .. }))
}

/// Every item listed (commands, the creative inventory, the checks): the blocks' in the order
/// of their table, then the others in the order of theirs (not the ones that are not listed:
/// the box of rounds, which belongs to the gun station).
pub fn all_items() -> Vec<ItemId> {
    block_items()
        .map(|d| d.id as ItemId)
        .chain(ITEMS.iter().filter(|d| d.creative != Creative::None).map(|d| d.id))
        .collect()
}

/// Stable identifier used by /give and save files.
pub fn key(id: ItemId) -> String {
    def(id).key.to_string()
}

/// The item with this key (`minecraft:` in front is fine).
pub fn from_key(k: &str) -> Option<ItemId> {
    static BY_KEY: OnceLock<HashMap<&'static str, ItemId>> = OnceLock::new();
    let by_key = BY_KEY.get_or_init(|| {
        let blocks = block_items().map(|d| (d.key, d.id as ItemId));
        blocks.chain(ITEMS.iter().map(|d| (d.key, d.id))).collect()
    });
    let k = k.strip_prefix("minecraft:").unwrap_or(k);
    by_key.get(k).copied()
}

/// Display name in the current language.
pub fn name(id: ItemId) -> String {
    let d = def(id);
    (if crate::app::lang::is_hungarian() { d.hu } else { d.en }).to_string()
}

/// Food and drink that is eaten (`food` of a line).
pub(super) const fn eaten(food: f32, saturation: f32, sick: Option<Sickness>) -> Option<Consumable> {
    Some(Consumable { food, saturation, thirst: 0.0, sick, drink: false })
}

/// Drunk (the bottle comes back empty).
pub(super) const fn drunk(thirst: f32, sick: Option<Sickness>) -> Option<Consumable> {
    Some(Consumable { food: 0.0, saturation: 0.0, thirst, sick, drink: true })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn the_table_is_consistent() {
        // Keys are unique (the blocks' items' too), ids follow each other file by file.
        let mut keys = HashSet::new();
        let mut next = FIRST_ITEM;
        let ends = [materials::END, tools::END, food::END, misc::END, armor::END, guns::END, ammo::END];
        for (part, end) in PARTS.iter().zip(ends) {
            for d in *part {
                assert_eq!(d.id, next, "{}", d.key);
                next += 1;
                assert!(!d.key.is_empty() && keys.insert(d.key), "key {:?}", d.key);
                assert_eq!(item_def(d.id).map(|x| x.key), Some(d.key));
            }
            assert_eq!(end, next);
        }
        for d in block_items() {
            assert!(keys.insert(d.key), "key {:?}", d.key);
        }
        // Every item has its names and an icon; what is listed is found by its key.
        for id in all_items().into_iter().chain([AMMO_BOX]) {
            let d = item_def(id).unwrap();
            assert!(!d.en.is_empty() && !d.hu.is_empty(), "{}", d.key);
            assert!(d.stack > 0, "{}", d.key);
            assert_eq!(from_key(d.key), Some(id));
            assert_eq!(from_key(&format!("minecraft:{}", d.key)), Some(id));
            match d.icon {
                Icon::Block(b) => assert_eq!(b, id),
                Icon::Flat(l) => assert!(l > 0 || id < FIRST_ITEM, "{}", d.key),
            }
            // What smelts goes into something that is an item; what wears does not stack.
            if let Some(s) = d.smelt {
                assert!(item_def(s.into).is_some() && (1..=3).contains(&s.furnace), "{}", d.key);
            }
            if max_damage(id) > 0 && d.tool.is_some() {
                assert_eq!(d.stack, 1, "{}", d.key);
            }
        }
        // Ids that are no item.
        assert!(item_def(FIRST_ITEM + ITEMS.len() as ItemId).is_none());
        assert!(item_def(crate::world::FURNACE + 1).is_none());
        assert_eq!(key(9999), "unknown");
        assert_eq!(from_key("nothing"), None);
    }

    #[test]
    fn tools_armor_and_eggs_resolve_both_ways() {
        for tier in TIER_ORDER {
            for kind in TOOL_KINDS {
                let id = tool_id(kind, tier);
                assert_eq!(tool_of(id), Some((kind, tier)));
                assert_eq!(max_damage(id), tier.durability());
                assert_eq!(is_sword(id), kind == ToolKind::Sword);
            }
        }
        assert_eq!(ITEMS.iter().filter(|d| d.tool.is_some()).count(), TIER_ORDER.len() * TOOL_KINDS.len());
        for m in 0..MATERIALS {
            for piece in 0..4 {
                assert_eq!(armor_of(armor_id(m, piece)), Some((piece, m)));
            }
        }
        assert_eq!(armor_of(BULLETPROOF_VEST), Some((VEST_SLOT, 0)));
        // Spawning is what a mob's egg does, and only that.
        for d in &ITEMS {
            let egg = crate::content::mobs::MobKind::by_egg(d.id).is_some();
            assert_eq!(d.on_use == OnUse::Spawn, egg, "{}", d.key);
        }
    }

    #[test]
    fn furnaces_smelt_and_burn_as_before() {
        use crate::world::*;
        let smelted = [
            (CLAY_BALL, BRICK, 1),
            (IRON_INGOT, STEEL_INGOT, 2),
            (BRICK, CERAMIC_PLATE, 3),
            (RAW_FISH, COOKED_FISH, 1),
            (WATER_BOTTLE, PURIFIED_WATER, 1),
            (COPPER_ORE, COPPER_INGOT, 1),
            (IRON_ORE, IRON_INGOT, 2),
            (GOLD_ORE, GOLD_INGOT, 3),
            (DIAMOND_ORE, DIAMOND, 3),
            (SAND, GLASS, 2),
            (COBBLE, STONE, 1),
            (OAK_LOG, CHARCOAL, 1),
            (BIRCH_LOG, CHARCOAL, 1),
            (SPRUCE_LOG, CHARCOAL, 1),
        ];
        for (from, into, tier) in smelted {
            assert_eq!((smelt(from), smelt_tier(from)), (Some(into), tier), "{}", key(from));
        }
        let smeltable = all_items().into_iter().filter(|&i| smelt(i).is_some()).count();
        assert_eq!(smeltable, smelted.len());
        let mut fuels = vec![
            (COAL, 80.0),
            (CHARCOAL, 80.0),
            (LAVA_BUCKET, 1000.0),
            (STICK, 5.0),
            (COAL_BLOCK, 800.0),
            (OAK_LOG, 15.0),
            (BIRCH_LOG, 15.0),
            (SPRUCE_LOG, 15.0),
            (PLANKS, 15.0),
            (CRAFTING_TABLE, 15.0),
            (CHEST, 15.0),
            (OAK_STAIRS, 15.0),
            (OAK_DOOR, 10.0),
            (OAK_SAPLING, 5.0),
            (BIRCH_SAPLING, 5.0),
            (SPRUCE_SAPLING, 5.0),
            (WOOL, 5.0),
        ];
        fuels.extend(TOOL_KINDS.map(|k| (tool_id(k, Tier::Wood), 10.0)));
        for &(id, t) in &fuels {
            assert_eq!(fuel_time(id), Some(t), "{}", key(id));
        }
        let burning = all_items().into_iter().filter(|&i| fuel_time(i).is_some()).count();
        assert_eq!(burning, fuels.len());
        // Nothing else: the parts of a block, and what is no item.
        assert_eq!(fuel_time(OAK_LOG_X), None);
        assert_eq!(smelt(9999), None);
        assert_eq!(smelt_tier(9999), 1);
    }
}
