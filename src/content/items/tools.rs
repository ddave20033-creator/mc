//! Tools: pickaxes, axes, shovels and swords of each tier (wood to diamond), the shears and the
//! fishing rod. What a tier mines, how fast and how long it lasts are its `Tier`'s; what
//! each tool mines faster, the blocks' (`content::blocks::Mine`).

use super::*;
use crate::item::Stack;
use crate::world::{COBBLE, PLANKS};

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
    // Added later, so its texture layers come after the others'.
    Copper,
}

/// In `Tier` order (the texture layers and the recipes follow it).
pub const TIERS: [Tier; 6] = [Tier::Wood, Tier::Stone, Tier::Iron, Tier::Gold, Tier::Diamond, Tier::Copper];

/// From the first tool to the last.
pub const TIER_ORDER: [Tier; 6] = [Tier::Wood, Tier::Stone, Tier::Copper, Tier::Iron, Tier::Gold, Tier::Diamond];

/// The shapes each tier is made in.
pub const TOOL_KINDS: [ToolKind; 4] = [ToolKind::Pickaxe, ToolKind::Axe, ToolKind::Shovel, ToolKind::Sword];

impl Tier {
    /// Mining speed multiplier with the right tool.
    pub const fn speed(self) -> f32 {
        match self {
            Tier::Wood => 2.0,
            Tier::Stone => 4.0,
            Tier::Copper => 5.0,
            Tier::Iron => 6.5,
            Tier::Gold => 11.0,
            Tier::Diamond => 8.5,
        }
    }

    /// Harvest level: which ores this tier can mine (see `item::mining`): wood mines coal,
    /// stone copper, copper iron, iron (or gold) gold and diamond, diamond obsidian.
    pub const fn level(self) -> u8 {
        match self {
            Tier::Wood => 0,
            Tier::Stone => 1,
            Tier::Copper => 2,
            Tier::Iron | Tier::Gold => 3,
            Tier::Diamond => 4,
        }
    }

    pub const fn durability(self) -> u16 {
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
    pub const fn material(self) -> ItemId {
        match self {
            Tier::Wood => PLANKS,
            Tier::Stone => COBBLE,
            Tier::Iron => IRON_INGOT,
            Tier::Gold => GOLD_INGOT,
            Tier::Diamond => DIAMOND,
            Tier::Copper => COPPER_INGOT,
        }
    }
}

/// A tool's damage: its kind's, and its tier's bonus.
const fn tool_attack(kind: ToolKind, tier: Tier) -> f32 {
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

/// A tool of a kind and tier: its icon, how long it lasts and hits come from them; a row of
/// the tools' tab for each kind; wooden ones burn; swords block.
const fn tool(kind: ToolKind, tier: Tier) -> ItemDef {
    ItemDef {
        icon: Icon::Flat(tex::tool_layer(tier as usize, kind as usize)),
        durability: tier.durability(),
        attack: tool_attack(kind, tier),
        tool: Some((kind, tier)),
        fuel: if matches!(tier, Tier::Wood) { Some(10.0) } else { None },
        on_use: if matches!(kind, ToolKind::Sword) { OnUse::Guard } else { OnUse::None },
        creative: Creative::Tools(kind as u8),
        ..SINGLE
    }
}

use ToolKind::*;

items! {
    after materials::END;

    WOODEN_PICKAXE = ItemDef { key: "wooden_pickaxe", en: "Wooden Pickaxe", hu: "Facsákány", ..tool(Pickaxe, Tier::Wood) };
    WOODEN_AXE = ItemDef { key: "wooden_axe", en: "Wooden Axe", hu: "Fabalta", ..tool(Axe, Tier::Wood) };
    WOODEN_SHOVEL = ItemDef { key: "wooden_shovel", en: "Wooden Shovel", hu: "Faásó", ..tool(Shovel, Tier::Wood) };
    WOODEN_SWORD = ItemDef { key: "wooden_sword", en: "Wooden Sword", hu: "Fakard", ..tool(Sword, Tier::Wood) };
    STONE_PICKAXE = ItemDef { key: "stone_pickaxe", en: "Stone Pickaxe", hu: "Kőcsákány", ..tool(Pickaxe, Tier::Stone) };
    STONE_AXE = ItemDef { key: "stone_axe", en: "Stone Axe", hu: "Kőbalta", ..tool(Axe, Tier::Stone) };
    STONE_SHOVEL = ItemDef { key: "stone_shovel", en: "Stone Shovel", hu: "Kőásó", ..tool(Shovel, Tier::Stone) };
    STONE_SWORD = ItemDef { key: "stone_sword", en: "Stone Sword", hu: "Kőkard", ..tool(Sword, Tier::Stone) };
    COPPER_PICKAXE = ItemDef { key: "copper_pickaxe", en: "Copper Pickaxe", hu: "Rézcsákány", ..tool(Pickaxe, Tier::Copper) };
    COPPER_AXE = ItemDef { key: "copper_axe", en: "Copper Axe", hu: "Rézbalta", ..tool(Axe, Tier::Copper) };
    COPPER_SHOVEL = ItemDef { key: "copper_shovel", en: "Copper Shovel", hu: "Rézásó", ..tool(Shovel, Tier::Copper) };
    COPPER_SWORD = ItemDef { key: "copper_sword", en: "Copper Sword", hu: "Rézkard", ..tool(Sword, Tier::Copper) };
    IRON_PICKAXE = ItemDef { key: "iron_pickaxe", en: "Iron Pickaxe", hu: "Vascsákány", ..tool(Pickaxe, Tier::Iron) };
    IRON_AXE = ItemDef { key: "iron_axe", en: "Iron Axe", hu: "Vasbalta", ..tool(Axe, Tier::Iron) };
    IRON_SHOVEL = ItemDef { key: "iron_shovel", en: "Iron Shovel", hu: "Vasásó", ..tool(Shovel, Tier::Iron) };
    IRON_SWORD = ItemDef { key: "iron_sword", en: "Iron Sword", hu: "Vaskard", ..tool(Sword, Tier::Iron) };
    GOLDEN_PICKAXE = ItemDef { key: "golden_pickaxe", en: "Golden Pickaxe", hu: "Aranycsákány", ..tool(Pickaxe, Tier::Gold) };
    GOLDEN_AXE = ItemDef { key: "golden_axe", en: "Golden Axe", hu: "Aranybalta", ..tool(Axe, Tier::Gold) };
    GOLDEN_SHOVEL = ItemDef { key: "golden_shovel", en: "Golden Shovel", hu: "Aranyásó", ..tool(Shovel, Tier::Gold) };
    GOLDEN_SWORD = ItemDef { key: "golden_sword", en: "Golden Sword", hu: "Aranykard", ..tool(Sword, Tier::Gold) };
    DIAMOND_PICKAXE = ItemDef { key: "diamond_pickaxe", en: "Diamond Pickaxe", hu: "Gyémántcsákány", ..tool(Pickaxe, Tier::Diamond) };
    DIAMOND_AXE = ItemDef { key: "diamond_axe", en: "Diamond Axe", hu: "Gyémántbalta", ..tool(Axe, Tier::Diamond) };
    DIAMOND_SHOVEL = ItemDef { key: "diamond_shovel", en: "Diamond Shovel", hu: "Gyémántásó", ..tool(Shovel, Tier::Diamond) };
    DIAMOND_SWORD = ItemDef { key: "diamond_sword", en: "Diamond Sword", hu: "Gyémántkard", ..tool(Sword, Tier::Diamond) };

    /// Shear sheep, and mine leaves, grass and dead bushes so they drop themselves (Minecraft's
    /// durability).
    SHEARS = ItemDef {
        key: "shears", en: "Shears", hu: "Olló", icon: Icon::Flat(tex::SHEARS), durability: 238,
        creative: Creative::Tools(4), ..SINGLE
    };
    /// A fishing rod: cast with the right button held (the longer, the farther), reeled in
    /// with the mouse wheel (see `client::tools::fishing`). Its `data` is the reel's gear (`rod_gear`).
    FISHING_ROD = ItemDef {
        key: "fishing_rod", en: "Fishing Rod", hu: "Horgászbot", icon: Icon::Flat(tex::FISHING_ROD),
        durability: FISHING_ROD_DURABILITY, on_use: OnUse::Cast, creative: Creative::Tools(4), ..SINGLE
    };
}

/// The tools' ids by `[tier as usize][kind as usize]`.
static TOOL_IDS: [[ItemId; 4]; 6] = {
    let mut t = [[NONE; 4]; 6];
    let mut i = 0;
    while i < ALL.len() {
        if let Some((kind, tier)) = ALL[i].tool {
            t[tier as usize][kind as usize] = ALL[i].id;
        }
        i += 1;
    }
    t
};

/// The tool of this kind and tier.
pub fn tool_id(kind: ToolKind, tier: Tier) -> ItemId {
    TOOL_IDS[tier as usize][kind as usize]
}

/// How long a fishing rod lasts (a fish caught wears it by one, a snapped line by more).
pub const FISHING_ROD_DURABILITY: u16 = 64;
/// The reel's gears (1 slow and strong .. `ROD_GEARS` fast and weak).
pub const ROD_GEARS: u8 = 5;

/// The gear a fishing rod's reel is in (1..=`ROD_GEARS`; a new rod is in the middle one).
pub fn rod_gear(s: &Stack) -> u8 {
    match (s.data & 0x7) as u8 {
        0 => 3,
        g => g.min(ROD_GEARS),
    }
}

pub fn set_rod_gear(s: &mut Stack, gear: u8) {
    s.data = (s.data & !0x7) | gear.clamp(1, ROD_GEARS) as u16;
}
