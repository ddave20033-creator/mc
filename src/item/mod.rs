//! Items: ids, names, icons, stacking and tools; mining rules, crafting and smelting are in
//! the submodules.
//!
//! Block items share the block's id (0..=255); other items start at 256.

pub mod armor;
pub mod crafting;
pub mod firearm;
pub mod inventory;
pub mod mining;
pub mod weapons;
#[cfg(test)]
mod snapshot_tests;

pub use armor::*;
pub use crafting::*;
pub use firearm::*;
pub use mining::*;
pub use weapons::*;

use crate::lang::is_hungarian;
use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::OnceLock;
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
/// The guide book: opened with a right click, it explains crafting, the furnaces and the
/// guns (see `game::book`).
pub const GUIDE_BOOK: ItemId = 298;
/// Pistol ammunition (9 mm): one is used up per shot.
pub const BULLET: ItemId = 282;
/// The five pistol parts, in the order they go together at the gun station: frame (with the
/// grip and trigger), barrel, recoil spring, slide and magazine.
pub const PISTOL_FRAME: ItemId = 283;
pub const PISTOL_BARREL: ItemId = 284;
pub const PISTOL_SPRING: ItemId = 285;
pub const PISTOL_SLIDE: ItemId = 286;
pub const PISTOL_MAGAZINE: ItemId = 287;
/// The gun (see `firearm`), put together at the gun station. Its `damage` is how dirty it is
/// (one per shot; cleaned at the gun station), its `data` holds the rounds in its magazine
/// and its attachments.
pub const PISTOL: ItemId = 288;
/// Pistol attachments, fitted at the gun station: a scope (zooms in far when aiming), a
/// silencer (no muzzle flash) and a laser sight (steadier from the hip). The extended magazine
/// is a magazine (20 rounds), like `PISTOL_MAGAZINE` (12).
pub const SCOPE: ItemId = 289;
pub const SILENCER: ItemId = 290;
pub const EXTENDED_MAGAZINE: ItemId = 291;
pub const LASER_SIGHT: ItemId = 292;
/// A box of pistol rounds (an ammo can): its `data` is the rounds in it, up to
/// `AMMO_BOX_ROUNDS`. It belongs to a gun station (three in its drawer, where magazines are
/// loaded): taken out onto its table and put back, never into an inventory, and not made.
pub const AMMO_BOX: ItemId = 361;
/// A weapon light for the pistol's accessory rail (instead of a laser sight): switched on and
/// off in the hand, it lights up what the gun points at. Only in creative (it is not made).
pub const FLASHLIGHT: ItemId = 362;
/// A six-shot revolver, put together at the gun station from its five parts. It is loaded
/// straight from the bullets carried (one at a time, or six at once from a speedloader). Its
/// `data` is its cylinder (see `revolver_chamber`), its `damage` how dirty it is.
pub const REVOLVER: ItemId = 363;
/// A speedloader: six rounds held in a ring, to load a revolver's cylinder at once. Its `data`
/// is the rounds in it (loaded at the gun station, like a magazine).
pub const SPEEDLOADER: ItemId = 364;
/// The revolver's five parts, in the order of `model::gun::FRAME` ..: the frame (with the
/// grip, trigger and sights), the barrel, the mainspring, the cylinder (on its crane, with the
/// ejector) and the hammer.
pub const REVOLVER_FRAME: ItemId = 365;
pub const REVOLVER_BARREL: ItemId = 366;
pub const REVOLVER_SPRING: ItemId = 367;
pub const REVOLVER_CYLINDER: ItemId = 368;
pub const REVOLVER_HAMMER: ItemId = 369;
/// Revolver ammunition (.357 Magnum): longer and heavier than the pistol's 9 mm, which does
/// not fit the revolver (nor this the pistol).
pub const MAGNUM_ROUND: ItemId = 370;
/// A wooden target dummy: set up with a right click, it shows the damage it takes above its
/// head (see `entity::mob`, `MobKind::Dummy`).
pub const TARGET_DUMMY: ItemId = 371;
/// The AK-47, put together at the gun station from its five parts: the receiver (with the
/// barrel, sights, handguard, grip and stock), the gas tube, the bolt carrier, the dust cover
/// with the recoil spring, and its curved 30-round magazine. Its data is laid out like the
/// pistol's (rounds, the magazine in it, the chamber), its `damage` is how dirty it is.
pub const AK47: ItemId = 372;
/// Rifle ammunition (7.62x39 mm): only for the AK.
pub const RIFLE_ROUND: ItemId = 373;
pub const AK_MAGAZINE: ItemId = 374;
pub const AK_RECEIVER: ItemId = 375;
pub const AK_GAS_TUBE: ItemId = 376;
pub const AK_BOLT: ItemId = 377;
pub const AK_COVER: ItemId = 378;
/// An automatic magazine loader for the rifle station's drawer: a magazine put on it is filled
/// from the boxes of rounds beside it, one round after another.
pub const MAG_LOADER: ItemId = 379;
/// A bone (pigs and sheep drop one now and then): tames a wolf.
pub const BONE: ItemId = 380;
pub const WOLF_SPAWN_EGG: ItemId = 381;
/// A fishing rod: cast with the right button held (the longer, the farther), reeled in with
/// the mouse wheel (see `game::fishing`). Its `data` is the reel's gear (`rod_gear`).
pub const FISHING_ROD: ItemId = 382;
/// A fish caught with the rod (any kind), and grilled in a furnace.
pub const RAW_FISH: ItemId = 383;
pub const COOKED_FISH: ItemId = 384;
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
/// The AK's parts, in the order of `model::gun::FRAME` .. (`GunKind::parts`).
pub const AK_PARTS: [ItemId; 5] = [AK_RECEIVER, AK_GAS_TUBE, AK_BOLT, AK_COVER, AK_MAGAZINE];
pub const REVOLVER_PARTS: [ItemId; 5] = [REVOLVER_FRAME, REVOLVER_BARREL, REVOLVER_SPRING, REVOLVER_CYLINDER, REVOLVER_HAMMER];
pub const AMMO_BOX_ROUNDS: u16 = 128;

/// A box of rounds holds one kind: 9 mm bullets, magnum rounds or rifle rounds (these bits of
/// its `data`, and of the value kept for a box in a gun station's drawer); the rest is how
/// many. Once rounds are in it only that kind goes in, until it is empty again.
pub const BOX_MAGNUM: u16 = 0x8000;
pub const BOX_RIFLE: u16 = 0x4000;
/// The bits that say what kind of rounds a box holds.
pub const BOX_KIND: u16 = BOX_MAGNUM | BOX_RIFLE;

/// The rounds that go into a box.
pub const BOX_AMMO: [ItemId; 3] = [BULLET, MAGNUM_ROUND, RIFLE_ROUND];

/// Rounds in a box of them.
pub fn box_rounds(st: &Stack) -> u16 {
    box_count(st.data)
}

/// Rounds in a box (its `data`, or a drawer's box).
pub fn box_count(v: u16) -> u16 {
    (v & !BOX_KIND).min(AMMO_BOX_ROUNDS)
}

/// What kind of round a box holds (None: it is empty).
pub fn box_ammo(v: u16) -> Option<ItemId> {
    match (box_count(v), v & BOX_KIND) {
        (0, _) => None,
        (_, BOX_MAGNUM) => Some(MAGNUM_ROUND),
        (_, BOX_RIFLE) => Some(RIFLE_ROUND),
        _ => Some(BULLET),
    }
}

/// How many rounds of `item` go into a box still (none of another kind than it holds).
pub fn box_room(v: u16, item: ItemId) -> u16 {
    let fits = BOX_AMMO.contains(&item) && box_ammo(v).is_none_or(|a| a == item);
    if fits { AMMO_BOX_ROUNDS - box_count(v) } else { 0 }
}

/// A box with `n` more rounds of `item` in it.
pub fn box_with(v: u16, item: ItemId, n: u16) -> u16 {
    let kind = match item {
        MAGNUM_ROUND => BOX_MAGNUM,
        RIFLE_ROUND => BOX_RIFLE,
        _ => 0,
    };
    (box_count(v) + n).min(AMMO_BOX_ROUNDS) | kind
}

/// A box with `n` fewer rounds in it (empty, it takes either kind again).
pub fn box_without(v: u16, n: u16) -> u16 {
    let left = box_count(v).saturating_sub(n);
    if left == 0 { 0 } else { left | (v & BOX_KIND) }
}

#[cfg(test)]
mod box_tests {
    use super::*;

    #[test]
    fn a_box_holds_one_kind_of_round_until_it_is_empty() {
        let v = box_with(0, MAGNUM_ROUND, 5);
        assert_eq!((box_count(v), box_ammo(v)), (5, Some(MAGNUM_ROUND)));
        // The other kind does not go in; more of its own does, up to the top.
        assert_eq!(box_room(v, BULLET), 0);
        assert_eq!(box_room(v, MAGNUM_ROUND), AMMO_BOX_ROUNDS - 5);
        assert_eq!(box_room(v, PISTOL), 0);
        // Emptied, it takes either again.
        let empty = box_without(v, 5);
        assert_eq!(box_ammo(empty), None);
        assert!(box_room(empty, BULLET) == AMMO_BOX_ROUNDS && box_room(empty, MAGNUM_ROUND) == AMMO_BOX_ROUNDS);
        // Old boxes (a count only) are 9 mm.
        assert_eq!(box_ammo(40), Some(BULLET));
    }
}

/// A gun's attachments as bits of `gun_mods`, with their items. `EXTENDED_MAGAZINE` is not
/// fitted: it says the magazine in the gun is an extended one.
pub mod gun_mod {
    pub const SCOPE: u8 = 1;
    pub const SILENCER: u8 = 2;
    pub const EXTENDED_MAGAZINE: u8 = 4;
    pub const LASER: u8 = 8;
    /// A weapon light on the accessory rail (where the laser sight goes: one or the other),
    /// and whether it is switched on (not an attachment: its state).
    pub const LIGHT: u8 = 16;
    pub const LIGHT_ON: u8 = 32;
    /// What the accessory rail holds.
    pub const RAIL: u8 = LASER | LIGHT;
}
pub const ATTACHMENTS: [(u8, ItemId); 4] = [
    (gun_mod::SCOPE, SCOPE),
    (gun_mod::SILENCER, SILENCER),
    (gun_mod::LASER, LASER_SIGHT),
    (gun_mod::LIGHT, FLASHLIGHT),
];

/// Whether an attachment (`gun_mod` bit) can go on a gun with `mods`: not one it has, and the
/// accessory rail holds one thing (a laser sight or a weapon light).
pub fn attachment_fits(mods: u8, bit: u8) -> bool {
    mods & bit == 0 && !(bit & gun_mod::RAIL != 0 && mods & gun_mod::RAIL != 0)
}

/// A gun's state besides its rounds (bits of its data; a gun without them has a magazine in,
/// a round in the chamber and its slide forward): no magazine in it, nothing in the chamber,
/// the slide held back (by an empty magazine, after its last round).
pub mod gun_state {
    pub const NO_MAG: u16 = 0x40;
    pub const CHAMBER_EMPTY: u16 = 0x80;
    pub const LOCKED: u16 = 0x1000;
}
pub fn gun_has_mag(s: &Stack) -> bool {
    s.item == REVOLVER || s.data & gun_state::NO_MAG == 0
}
/// A round ready to fire (a revolver: any live round in its cylinder).
pub fn gun_chambered(s: &Stack) -> bool {
    if s.item == REVOLVER {
        return gun_rounds(s) > 0;
    }
    s.data & gun_state::CHAMBER_EMPTY == 0
}
pub fn gun_locked(s: &Stack) -> bool {
    s.item != REVOLVER && s.data & gun_state::LOCKED != 0
}
pub fn set_gun_state(s: &mut Stack, bit: u16, on: bool) {
    if s.item == REVOLVER {
        return;
    }
    s.data = if on { s.data | bit } else { s.data & !bit };
}

/// What is in each of a revolver's six chambers: two bits each in its `data` (chamber k in
/// bits 2k, 2k+1), and the chamber under the hammer in bits 12-14. Chamber k is the one the
/// model's `chamber{k}` is; the cylinder turns the next one (`revolver_next`) under the hammer
/// as the trigger is pulled.
pub mod chamber {
    pub const EMPTY: u8 = 0;
    pub const LIVE: u8 = 1;
    /// A fired case, left in the chamber until the cylinder is emptied.
    pub const SPENT: u8 = 2;
}
pub fn revolver_chamber(s: &Stack, k: usize) -> u8 {
    ((s.data >> (2 * k)) & 3) as u8
}
pub fn set_revolver_chamber(s: &mut Stack, k: usize, v: u8) {
    s.data = (s.data & !(3 << (2 * k))) | ((v as u16 & 3) << (2 * k));
}
pub fn revolver_index(s: &Stack) -> usize {
    (((s.data >> 12) & 7) as usize).min(5)
}
pub fn set_revolver_index(s: &mut Stack, k: usize) {
    s.data = (s.data & !(7 << 12)) | (((k % 6) as u16) << 12);
}
/// The chamber that comes under the hammer after `k` (the cylinder turning a sixth, anticlockwise
/// seen from behind).
pub fn revolver_next(k: usize) -> usize {
    (k + 5) % 6
}
/// Rounds ready to fire: in the magazine and in the chamber (a revolver's: live in its
/// cylinder).
pub fn gun_ready_rounds(s: &Stack) -> u8 {
    if s.item == REVOLVER {
        return gun_rounds(s);
    }
    (if gun_has_mag(s) { gun_rounds(s) } else { 0 }) + gun_chambered(s) as u8
}

/// A magazine (or a speedloader): how many rounds it holds. Its data is the rounds in it.
pub fn magazine_capacity(item: ItemId) -> Option<u8> {
    if item == SPEEDLOADER {
        return Some(6);
    }
    let (kind, extended) = magazine_of(item)?;
    Some(if extended { kind.magazine_size(gun_mod::EXTENDED_MAGAZINE) } else { kind.magazine_size(0) })
}

/// A magazine that goes into a gun (not a speedloader): which gun, and whether it is its
/// extended one.
fn magazine_of(item: ItemId) -> Option<(GunKind, bool)> {
    GUN_KINDS.into_iter().find_map(|k| {
        let m = k.magazine()?;
        if m.item == item {
            Some((k, false))
        } else {
            m.extended.filter(|&(e, _)| e == item).map(|_| (k, true))
        }
    })
}

/// A magazine that goes into a gun (not a speedloader): which gun.
pub fn magazine_gun(item: ItemId) -> Option<GunKind> {
    magazine_of(item).map(|(k, _)| k)
}

/// A magazine that goes into a gun (not a speedloader).
pub fn is_gun_magazine(item: ItemId) -> bool {
    magazine_gun(item).is_some()
}

/// Rounds in a gun's magazine (the low 6 bits of its data), or in a magazine; a revolver's
/// live rounds.
pub fn gun_rounds(s: &Stack) -> u8 {
    if s.item == REVOLVER {
        return (0..6).filter(|&k| revolver_chamber(s, k) == chamber::LIVE).count() as u8;
    }
    (s.data & 0x3f) as u8
}
/// A revolver: `n` live rounds from the chamber after the one under the hammer on, the others
/// empty.
pub fn set_gun_rounds(s: &mut Stack, n: u8) {
    if s.item == REVOLVER {
        let mut k = revolver_index(s);
        for i in 0..6 {
            k = revolver_next(k);
            set_revolver_chamber(s, k, if i < n as usize { chamber::LIVE } else { chamber::EMPTY });
        }
        return;
    }
    s.data = (s.data & !0x3f) | (n as u16 & 0x3f);
}
/// A gun's attachments (`gun_mod` bits, in the low half of the data's high byte).
/// (the first four in bits 8-11 of the data, the weapon light and its switch in bits 13-14;
/// a revolver takes none)
pub fn gun_mods(s: &Stack) -> u8 {
    if s.item == REVOLVER {
        return 0;
    }
    ((s.data >> 8) & 0x0f) as u8 | ((s.data >> 9) & 0x30) as u8
}
pub fn set_gun_mods(s: &mut Stack, mods: u8) {
    if s.item == REVOLVER {
        return;
    }
    s.data = (s.data & !0x6f00) | ((mods as u16 & 0x0f) << 8) | ((mods as u16 & 0x30) << 9);
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

/// How many go in a slot (see `ItemDef::max_stack`).
pub fn max_stack(id: ItemId) -> u8 {
    def(id).max_stack
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
    let food = |food, saturation, sick| Consumable {
        food,
        saturation,
        thirst: 0.0,
        sick,
        drink: false,
    };
    Some(match id {
        // Raw fish can upset the stomach a little; grilled it is as good as cooked meat.
        RAW_FISH => food(2.0, 0.4, Some(Sickness { chance: 0.2, poison: 0.0, nausea: 5.0 })),
        COOKED_FISH => food(5.0, 6.0, None),
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
    def(id).attack_damage
}

/// How long it lasts (tools, shears, the fishing rod, armor), or how dirty a gun, its parts
/// and magazines can get; 0: it does not wear.
pub fn max_damage(id: ItemId) -> u16 {
    def(id).max_damage
}

/// A tool's damage: its kind's, and its tier's bonus.
fn tool_attack(kind: ToolKind, tier: Tier) -> f32 {
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

/// A hand-written line of `BLOCK_ITEMS` or `ITEMS`: the item, its key (for /give and save
/// files), its English and Hungarian names, its icon's texture layer (a block's comes from the
/// block), and how many stack and how long it lasts where that is its own (the families, the
/// tools, armor and guns, are filled in by `build_table`).
#[derive(Clone, Copy)]
struct Row {
    id: ItemId,
    key: &'static str,
    en: &'static str,
    hu: &'static str,
    icon: Option<u32>,
    stack: u8,
    lasts: u16,
}

const fn blk(b: u8, key: &'static str, en: &'static str, hu: &'static str) -> Row {
    Row { id: b as ItemId, key, en, hu, icon: None, stack: 64, lasts: 0 }
}

const fn it(id: ItemId, key: &'static str, en: &'static str, hu: &'static str, icon: u32) -> Row {
    Row { id, key, en, hu, icon: Some(icon), stack: 64, lasts: 0 }
}

impl Row {
    const fn stack(mut self, n: u8) -> Row {
        self.stack = n;
        self
    }
    const fn lasts(mut self, n: u16) -> Row {
        self.lasts = n;
        self
    }
}

/// Everything about one item id that `max_stack`, `max_damage`, `attack_damage`, `key`,
/// `name`, `icon` and `block_of` read (see `def`).
struct ItemDef {
    key: Cow<'static, str>,
    en: Cow<'static, str>,
    hu: Cow<'static, str>,
    icon: Icon,
    max_stack: u8,
    max_damage: u16,
    attack_damage: f32,
    /// The block it places.
    block: Option<u8>,
}

/// An id that is no item.
const NO_ITEM: ItemDef = ItemDef {
    key: Cow::Borrowed("unknown"),
    en: Cow::Borrowed("Unknown"),
    hu: Cow::Borrowed("Ismeretlen"),
    icon: Icon::Flat(tex::STONE),
    max_stack: 64,
    max_damage: 0,
    attack_damage: 1.0,
    block: None,
};

struct ItemTable {
    /// Indexed by item id (the ids after the last item's are `NO_ITEM`).
    defs: Vec<ItemDef>,
    by_key: HashMap<Cow<'static, str>, ItemId>,
}

fn table() -> &'static ItemTable {
    static T: OnceLock<ItemTable> = OnceLock::new();
    T.get_or_init(build_table)
}

fn def(id: ItemId) -> &'static ItemDef {
    table().defs.get(id as usize).unwrap_or(&NO_ITEM)
}

/// The icon of a block's item: a cube, or a flat sprite for the ones that are not cubes.
fn block_icon(b: u8) -> Icon {
    match b {
        LANTERN => Icon::Flat(tex::LANTERN_ITEM),
        OAK_DOOR => Icon::Flat(tex::DOOR_ITEM),
        BED => Icon::Flat(tex::BED_ITEM),
        _ if is_plant(b) || b == TORCH => Icon::Flat(face_texture(b, 0)),
        _ => Icon::Block(b),
    }
}

fn build_table() -> ItemTable {
    let mut defs: Vec<ItemDef> = Vec::new();
    let mut put = |id: ItemId, d: ItemDef| {
        if defs.len() <= id as usize {
            defs.resize_with(id as usize + 1, || NO_ITEM);
        }
        defs[id as usize] = d;
    };
    for r in BLOCK_ITEMS.iter().chain(ITEMS) {
        put(
            r.id,
            ItemDef {
                key: r.key.into(),
                en: r.en.into(),
                hu: r.hu.into(),
                icon: r.icon.map_or_else(|| block_icon(r.id as u8), Icon::Flat),
                max_stack: r.stack,
                max_damage: r.lasts,
                attack_damage: 1.0,
                block: r.icon.is_none().then_some(r.id as u8),
            },
        );
    }
    for tier in TIERS {
        for kind in KINDS {
            let (t, k) = (tier as usize, kind as usize);
            let key = ["wooden", "stone", "iron", "golden", "diamond", "copper"][t];
            let kind_key = ["pickaxe", "axe", "shovel", "sword"][k];
            let en = ["Wooden", "Stone", "Iron", "Golden", "Diamond", "Copper"][t];
            let kind_en = ["Pickaxe", "Axe", "Shovel", "Sword"][k];
            let hu = ["Fa", "Kő", "Vas", "Arany", "Gyémánt", "Réz"][t];
            let kind_hu = ["csákány", "balta", "ásó", "kard"][k];
            put(
                tool_id(kind, tier),
                ItemDef {
                    key: format!("{key}_{kind_key}").into(),
                    en: format!("{en} {kind_en}").into(),
                    hu: format!("{hu}{kind_hu}").into(),
                    icon: Icon::Flat(crate::world::textures::tool_layer(t, k)),
                    max_stack: 1,
                    max_damage: tier.durability(),
                    attack_damage: tool_attack(kind, tier),
                    block: None,
                },
            );
        }
    }
    // The families: armor; guns, magazines and speedloaders do not stack; a gun's parts and
    // magazines get as dirty as the gun (taken apart, each is cleaned on its own).
    for (id, d) in defs.iter_mut().enumerate() {
        let id = id as ItemId;
        if armor_of(id).is_some() {
            d.max_stack = 1;
            d.max_damage = armor_durability(id);
        }
        if GunKind::of(id).is_some() || magazine_capacity(id).is_some() {
            d.max_stack = 1;
        }
        let gun = GunKind::of(id)
            .or_else(|| GUN_KINDS.into_iter().find(|k| k.parts().contains(&id)))
            .or_else(|| magazine_gun(id));
        if let Some(k) = gun {
            d.max_damage = k.stats().dirt_max;
        }
    }
    // By key, as `all_items` lists them (the first with a key), and the box of rounds: it is
    // not listed, but lies on gun stations in save files.
    let mut by_key = HashMap::new();
    for id in all_items().into_iter().chain([AMMO_BOX]) {
        if let Some(d) = defs.get(id as usize) {
            by_key.entry(d.key.clone()).or_insert(id);
        }
    }
    ItemTable { defs, by_key }
}

/// Every block that exists as an item, in creative inventory order: the block, its key (for
/// /give and save files), and its English and Hungarian names.
const BLOCK_ITEMS: &[Row] = &[
    blk(GRASS, "grass_block", "Grass Block", "Füves blokk"),
    blk(DIRT, "dirt", "Dirt", "Föld"),
    blk(STONE, "stone", "Stone", "Kő"),
    blk(COBBLE, "cobblestone", "Cobblestone", "Zúzottkő"),
    blk(STONE_BRICKS, "stone_bricks", "Stone Bricks", "Kőtégla"),
    blk(SAND, "sand", "Sand", "Homok"),
    blk(GRAVEL, "gravel", "Gravel", "Kavics"),
    blk(CLAY, "clay", "Clay", "Agyag"),
    blk(SANDSTONE, "sandstone", "Sandstone", "Homokkő"),
    blk(SNOW, "snow_block", "Snow Block", "Hóblokk"),
    blk(SNOWY_GRASS, "snowy_grass_block", "Snowy Grass Block", "Havas füves blokk"),
    blk(ICE, "ice", "Ice", "Jég"),
    blk(OAK_LOG, "oak_log", "Oak Log", "Tölgyfarönk"),
    blk(BIRCH_LOG, "birch_log", "Birch Log", "Nyírfarönk"),
    blk(SPRUCE_LOG, "spruce_log", "Spruce Log", "Lucfenyőrönk"),
    blk(PLANKS, "oak_planks", "Oak Planks", "Tölgyfa deszka"),
    blk(OAK_STAIRS, "oak_stairs", "Oak Stairs", "Tölgyfa lépcső"),
    blk(OAK_DOOR, "oak_door", "Oak Door", "Tölgyfa ajtó"),
    blk(BRICKS, "bricks", "Bricks", "Téglák"),
    blk(GLASS, "glass", "Glass", "Üveg"),
    blk(GLOWSTONE, "glowstone", "Glowstone", "Izzókő"),
    blk(OAK_LEAVES, "oak_leaves", "Oak Leaves", "Tölgylevelek"),
    blk(BIRCH_LEAVES, "birch_leaves", "Birch Leaves", "Nyírfalevelek"),
    blk(SPRUCE_LEAVES, "spruce_leaves", "Spruce Leaves", "Lucfenyőlevelek"),
    blk(OAK_SAPLING, "oak_sapling", "Oak Sapling", "Tölgycsemete"),
    blk(BIRCH_SAPLING, "birch_sapling", "Birch Sapling", "Nyírfacsemete"),
    blk(SPRUCE_SAPLING, "spruce_sapling", "Spruce Sapling", "Lucfenyőcsemete"),
    blk(CACTUS, "cactus", "Cactus", "Kaktusz"),
    blk(TALL_GRASS, "grass", "Grass", "Fű"),
    blk(POPPY, "poppy", "Poppy", "Pipacs"),
    blk(DANDELION, "dandelion", "Dandelion", "Pitypang"),
    blk(DEAD_BUSH, "dead_bush", "Dead Bush", "Elszáradt bokor"),
    blk(COAL_ORE, "coal_ore", "Coal Ore", "Szénérc"),
    blk(COPPER_ORE, "copper_ore", "Copper Ore", "Rézérc"),
    blk(IRON_ORE, "iron_ore", "Iron Ore", "Vasérc"),
    blk(GOLD_ORE, "gold_ore", "Gold Ore", "Aranyérc"),
    blk(DIAMOND_ORE, "diamond_ore", "Diamond Ore", "Gyémántérc"),
    blk(COAL_BLOCK, "coal_block", "Block of Coal", "Szénblokk"),
    blk(COPPER_BLOCK, "copper_block", "Block of Copper", "Rézblokk"),
    blk(IRON_BLOCK, "iron_block", "Block of Iron", "Vasblokk"),
    blk(GOLD_BLOCK, "gold_block", "Block of Gold", "Aranyblokk"),
    blk(DIAMOND_BLOCK, "diamond_block", "Block of Diamond", "Gyémántblokk"),
    blk(OBSIDIAN, "obsidian", "Obsidian", "Obszidián"),
    blk(BEDROCK, "bedrock", "Bedrock", "Alapkő"),
    blk(CRAFTING_TABLE, "crafting_table", "Crafting Table", "Barkácsasztal"),
    blk(FURNACE, "furnace", "Furnace", "Kemence"),
    blk(BLAST_FURNACE, "blast_furnace", "Blast Furnace", "Kohó"),
    blk(ADV_FURNACE, "advanced_furnace", "Advanced Furnace", "Fejlett kohó"),
    blk(CHEST, "chest", "Chest", "Láda"),
    blk(TORCH, "torch", "Torch", "Fáklya"),
    blk(LANTERN, "lantern", "Lantern", "Lámpás"),
    blk(WOOL, "white_wool", "White Wool", "Fehér gyapjú"),
    blk(BED, "red_bed", "Red Bed", "Piros ágy").stack(1),
    blk(GUN_STATION, "gun_station", "Gun Station", "Fegyverasztal"),
    blk(RIFLE_BENCH, "rifle_station", "Rifle Station", "Puskaasztal"),
];

/// The other items (ids from 256, tools aside), in creative inventory order: the id, key,
/// English and Hungarian names, and the icon's texture layer.
const ITEMS: &[Row] = &[
    it(STICK, "stick", "Stick", "Bot", tex::STICK),
    it(GUIDE_BOOK, "guide_book", "Guide Book", "Kézikönyv", tex::BOOK).stack(1),
    it(COAL, "coal", "Coal", "Szén", tex::COAL),
    it(CHARCOAL, "charcoal", "Charcoal", "Faszén", tex::CHARCOAL),
    it(IRON_INGOT, "iron_ingot", "Iron Ingot", "Vasrúd", tex::IRON_INGOT),
    it(IRON_NUGGET, "iron_nugget", "Iron Nugget", "Vasrög", tex::IRON_NUGGET),
    it(COPPER_INGOT, "copper_ingot", "Copper Ingot", "Rézrúd", tex::COPPER_INGOT),
    it(STEEL_INGOT, "steel_ingot", "Steel Ingot", "Acélrúd", tex::STEEL_INGOT),
    it(CERAMIC_PLATE, "ceramic_plate", "Ceramic Plate", "Kerámialap", tex::CERAMIC_PLATE),
    it(FRAG_GRENADE, "frag_grenade", "Frag Grenade", "Repeszgránát", tex::FRAG_GRENADE).stack(16),
    it(SMOKE_GRENADE, "smoke_grenade", "Smoke Grenade", "Füstgránát", tex::SMOKE_GRENADE).stack(16),
    it(ARMOR_BASE + 0, "wool_helmet", "Wool Helmet", "Posztó sisak", tex::ARMOR_ICONS + 0),
    it(ARMOR_BASE + 1, "wool_chestplate", "Wool Chestplate", "Posztó mellvért", tex::ARMOR_ICONS + 1),
    it(ARMOR_BASE + 2, "wool_leggings", "Wool Leggings", "Posztó lábvért", tex::ARMOR_ICONS + 2),
    it(ARMOR_BASE + 3, "wool_boots", "Wool Boots", "Posztó csizma", tex::ARMOR_ICONS + 3),
    it(ARMOR_BASE + 4, "copper_helmet", "Copper Helmet", "Réz sisak", tex::ARMOR_ICONS + 4),
    it(ARMOR_BASE + 5, "copper_chestplate", "Copper Chestplate", "Réz mellvért", tex::ARMOR_ICONS + 5),
    it(ARMOR_BASE + 6, "copper_leggings", "Copper Leggings", "Réz lábvért", tex::ARMOR_ICONS + 6),
    it(ARMOR_BASE + 7, "copper_boots", "Copper Boots", "Réz csizma", tex::ARMOR_ICONS + 7),
    it(ARMOR_BASE + 8, "steel_helmet", "Steel Helmet", "Acél sisak", tex::ARMOR_ICONS + 8),
    it(ARMOR_BASE + 9, "steel_chestplate", "Steel Chestplate", "Acél mellvért", tex::ARMOR_ICONS + 9),
    it(ARMOR_BASE + 10, "steel_leggings", "Steel Leggings", "Acél lábvért", tex::ARMOR_ICONS + 10),
    it(ARMOR_BASE + 11, "steel_boots", "Steel Boots", "Acél csizma", tex::ARMOR_ICONS + 11),
    it(ARMOR_BASE + 12, "diamond_helmet", "Diamond Helmet", "Gyémánt sisak", tex::ARMOR_ICONS + 12),
    it(ARMOR_BASE + 13, "diamond_chestplate", "Diamond Chestplate", "Gyémánt mellvért", tex::ARMOR_ICONS + 13),
    it(ARMOR_BASE + 14, "diamond_leggings", "Diamond Leggings", "Gyémánt lábvért", tex::ARMOR_ICONS + 14),
    it(ARMOR_BASE + 15, "diamond_boots", "Diamond Boots", "Gyémánt csizma", tex::ARMOR_ICONS + 15),
    it(BULLETPROOF_VEST, "bulletproof_vest", "Bulletproof Vest", "Golyóálló mellény", tex::ARMOR_ICONS + 16),
    it(GOLD_INGOT, "gold_ingot", "Gold Ingot", "Aranyrúd", tex::GOLD_INGOT),
    it(DIAMOND, "diamond", "Diamond", "Gyémánt", tex::DIAMOND),
    it(CLAY_BALL, "clay_ball", "Clay Ball", "Agyaggolyó", tex::CLAY_BALL),
    it(BRICK, "brick", "Brick", "Tégla", tex::BRICK),
    it(BUCKET, "bucket", "Bucket", "Vödör", tex::BUCKET).stack(16),
    it(WATER_BUCKET, "water_bucket", "Water Bucket", "Vizesvödör", tex::WATER_BUCKET).stack(1),
    it(LAVA_BUCKET, "lava_bucket", "Lava Bucket", "Lávás vödör", tex::LAVA_BUCKET).stack(1),
    it(PORKCHOP, "porkchop", "Raw Porkchop", "Nyers disznóhús", tex::PORKCHOP),
    it(COOKED_PORKCHOP, "cooked_porkchop", "Cooked Porkchop", "Sült disznóhús", tex::COOKED_PORKCHOP),
    it(MUTTON, "mutton", "Raw Mutton", "Nyers birkahús", tex::MUTTON),
    it(COOKED_MUTTON, "cooked_mutton", "Cooked Mutton", "Sült birkahús", tex::COOKED_MUTTON),
    it(HALF_COOKED_PORKCHOP, "half_cooked_porkchop", "Half-Cooked Porkchop", "Félig sült disznóhús", tex::HALF_COOKED_PORKCHOP),
    it(HALF_COOKED_MUTTON, "half_cooked_mutton", "Half-Cooked Mutton", "Félig sült birkahús", tex::HALF_COOKED_MUTTON),
    it(BURNT_PORKCHOP, "burnt_porkchop", "Burnt Porkchop", "Szenes disznóhús", tex::BURNT_PORKCHOP),
    it(BURNT_MUTTON, "burnt_mutton", "Burnt Mutton", "Szenes birkahús", tex::BURNT_MUTTON),
    it(HALF_BURNT_PORKCHOP, "half_burnt_porkchop", "Half-Burnt Porkchop", "Félig szenes disznóhús", tex::HALF_BURNT_PORKCHOP),
    it(HALF_BURNT_MUTTON, "half_burnt_mutton", "Half-Burnt Mutton", "Félig szenes birkahús", tex::HALF_BURNT_MUTTON),
    it(RAW_BURNT_PORKCHOP, "raw_burnt_porkchop", "Burnt-Raw Porkchop", "Szenes-nyers disznóhús", tex::RAW_BURNT_PORKCHOP),
    it(RAW_BURNT_MUTTON, "raw_burnt_mutton", "Burnt-Raw Mutton", "Szenes-nyers birkahús", tex::RAW_BURNT_MUTTON),
    it(SHEARS, "shears", "Shears", "Olló", tex::SHEARS).stack(1).lasts(SHEARS_DURABILITY),
    it(GLASS_BOTTLE, "glass_bottle", "Glass Bottle", "Üvegpalack", tex::GLASS_BOTTLE),
    it(WATER_BOTTLE, "water_bottle", "Water Bottle", "Vizes üveg", tex::WATER_BOTTLE).stack(16),
    it(PURIFIED_WATER, "purified_water", "Boiled Water", "Forralt víz", tex::PURIFIED_WATER).stack(16),
    it(PIG_SPAWN_EGG, "pig_spawn_egg", "Pig Spawn Egg", "Disznó idéző tojás", tex::PIG_SPAWN_EGG),
    it(SHEEP_SPAWN_EGG, "sheep_spawn_egg", "Sheep Spawn Egg", "Birka idéző tojás", tex::SHEEP_SPAWN_EGG),
    it(WOLF_SPAWN_EGG, "wolf_spawn_egg", "Wolf Spawn Egg", "Farkas idéző tojás", tex::WOLF_SPAWN_EGG),
    it(BONE, "bone", "Bone", "Csont", tex::BONE),
    it(FISHING_ROD, "fishing_rod", "Fishing Rod", "Horgászbot", tex::FISHING_ROD).stack(1).lasts(FISHING_ROD_DURABILITY),
    it(RAW_FISH, "raw_fish", "Raw Fish", "Nyers hal", tex::RAW_FISH),
    it(COOKED_FISH, "cooked_fish", "Cooked Fish", "Sült hal", tex::COOKED_FISH),
    it(PISTOL, "pistol", "Pistol", "Pisztoly", tex::PISTOL),
    it(REVOLVER, "revolver", "Revolver", "Revolver", tex::REVOLVER),
    it(SPEEDLOADER, "speedloader", "Speedloader", "Gyorstöltő", tex::SPEEDLOADER),
    it(MAGNUM_ROUND, "magnum_round", "Magnum Round", "Magnum töltény", tex::MAGNUM_ROUND),
    it(TARGET_DUMMY, "target_dummy", "Target Dummy", "Gyakorlóbábu", tex::TARGET_DUMMY).stack(16),
    it(AK47, "ak47", "AK-47", "AK-47", tex::AK47),
    it(RIFLE_ROUND, "rifle_round", "7.62 Round", "7,62-es töltény", tex::RIFLE_ROUND),
    it(MAG_LOADER, "magazine_loader", "Magazine Loader", "Tárazógép", tex::MAG_LOADER).stack(1),
    it(AK_MAGAZINE, "ak_magazine", "AK Magazine", "AK-tár", tex::AK_PARTS + 4),
    it(AK_RECEIVER, "ak_receiver", "AK Receiver", "AK-tok", tex::AK_PARTS),
    it(AK_GAS_TUBE, "ak_gas_tube", "AK Gas Tube", "AK-gázcső", tex::AK_PARTS + 1),
    it(AK_BOLT, "ak_bolt_carrier", "AK Bolt Carrier", "AK-zárkeret", tex::AK_PARTS + 2),
    it(AK_COVER, "ak_dust_cover", "AK Dust Cover", "AK-tokfedél", tex::AK_PARTS + 3),
    it(REVOLVER_FRAME, "revolver_frame", "Revolver Frame", "Revolverváz", tex::REVOLVER_PARTS),
    it(REVOLVER_BARREL, "revolver_barrel", "Revolver Barrel", "Revolvercső", tex::REVOLVER_PARTS + 1),
    it(REVOLVER_SPRING, "revolver_mainspring", "Mainspring", "Kakasrugó", tex::REVOLVER_PARTS + 2),
    it(REVOLVER_CYLINDER, "revolver_cylinder", "Revolver Cylinder", "Forgótár", tex::REVOLVER_PARTS + 3),
    it(REVOLVER_HAMMER, "revolver_hammer", "Hammer", "Kakas", tex::REVOLVER_PARTS + 4),
    it(BULLET, "bullet", "Bullet", "Töltény", tex::BULLET),
    it(PISTOL_FRAME, "pistol_frame", "Pistol Frame", "Pisztolyváz", tex::PISTOL_PARTS),
    it(PISTOL_BARREL, "pistol_barrel", "Pistol Barrel", "Pisztolycső", tex::PISTOL_PARTS + 1),
    it(PISTOL_SPRING, "pistol_spring", "Recoil Spring", "Visszatérítő rugó", tex::PISTOL_PARTS + 2),
    it(PISTOL_SLIDE, "pistol_slide", "Pistol Slide", "Pisztolyszán", tex::PISTOL_PARTS + 3),
    it(PISTOL_MAGAZINE, "pistol_magazine", "Pistol Magazine", "Pisztolytár", tex::PISTOL_PARTS + 4),
    it(SCOPE, "scope", "Scope", "Távcső", tex::GUN_ATTACHMENTS),
    it(SILENCER, "silencer", "Silencer", "Hangtompító", tex::GUN_ATTACHMENTS + 1),
    it(EXTENDED_MAGAZINE, "extended_magazine", "Extended Magazine", "Bővített tár", tex::GUN_ATTACHMENTS + 2),
    it(LASER_SIGHT, "laser_sight", "Laser Sight", "Lézeres célzó", tex::GUN_ATTACHMENTS + 3),
    it(AMMO_BOX, "ammo_box", "Ammo Box", "Töltényes doboz", tex::AMMO_BOX).stack(1),
    it(FLASHLIGHT, "weapon_light", "Weapon Light", "Fegyverlámpa", tex::FLASHLIGHT),
];

/// Creative inventory order: blocks, other items, then the tools.
pub fn all_items() -> Vec<ItemId> {
    let mut v: Vec<ItemId> = BLOCK_ITEMS.iter().map(|r| r.id).collect();
    v.extend(ITEMS.iter().map(|r| r.id).filter(|&id| id != AMMO_BOX));
    for tier in TIER_ORDER {
        for kind in KINDS {
            v.push(tool_id(kind, tier));
        }
    }
    v
}

/// The block this item places (base variant for directional blocks).
pub fn block_of(id: ItemId) -> Option<u8> {
    def(id).block
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
        _ if is_rifle_bench(b) => RIFLE_BENCH,
        _ if is_gun_bench(b) => GUN_STATION,
        _ if is_log(b) => log_base(b),
        _ if is_stump_mark(b) => soil(b),
        _ if is_water(b) => return Some(WATER_BUCKET),
        _ if is_lava(b) => return Some(LAVA_BUCKET),
        _ => b,
    };
    block_of(base as ItemId).map(|_| base as ItemId)
}

#[derive(Clone, Copy)]
pub enum Icon {
    /// Drawn as an isometric cube.
    Block(u8),
    /// Drawn as a flat sprite from this texture layer.
    Flat(u32),
}

pub fn icon(id: ItemId) -> Icon {
    def(id).icon
}

/// Stable identifier used by /give and save files.
pub fn key(id: ItemId) -> String {
    def(id).key.to_string()
}

pub fn from_key(k: &str) -> Option<ItemId> {
    let k = k.strip_prefix("minecraft:").unwrap_or(k);
    table().by_key.get(k).copied()
}

/// Display name in the current language.
pub fn name(id: ItemId) -> String {
    let d = def(id);
    (if is_hungarian() { &d.hu } else { &d.en }).to_string()
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
        // (all but the box of rounds, which belongs to the gun station and is not listed)
        assert_eq!(all.len(), BLOCK_ITEMS.len() + ITEMS.len() - 1 + 4 * TIERS.len());
        assert_eq!(from_key(&key(AMMO_BOX)), Some(AMMO_BOX));
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
