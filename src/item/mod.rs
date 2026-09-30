//! Items in use: stacks and slots, the inventory, mining, crafting, what a worn set of armor
//! does and what a gun holds. What each item is (its id, key, names, icon, stacking, tool,
//! armor, food, fuel, smelting, gun facts, what using it does) is its line in
//! `content::items`, re-exported here, so `crate::item::*` brings all of it.
//!
//! Block items share the block's id (below `FIRST_ITEM`); other items start at `FIRST_ITEM`.

pub mod armor;
pub mod crafting;
pub mod firearm;
pub mod inventory;
pub mod mining;
#[cfg(test)]
mod snapshot_tests;

pub use crate::content::items::*;
pub use armor::*;
pub use crafting::*;
pub use firearm::*;
pub use mining::*;

use crate::world::*;

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

/// Display name of a block in the world (for debug info).
pub fn block_name(b: Block) -> String {
    crate::content::blocks::block_name(b)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::textures::tex;

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
        assert_eq!(all.len(), block_items().count() + ITEMS.len() - 1);
        assert_eq!(from_key(&key(AMMO_BOX)), Some(AMMO_BOX));
        // Keys stored in save files must not change.
        assert_eq!(key(GRASS as ItemId), "grass_block");
        assert_eq!(key(PURIFIED_WATER), "purified_water");
        assert_eq!(key(tool_id(ToolKind::Pickaxe, Tier::Diamond)), "diamond_pickaxe");
        assert!(matches!(icon(IRON_NUGGET), Icon::Flat(l) if l == tex::IRON_NUGGET));
    }
}
