//! The game's content, defined in one place: every block (`blocks`), every item (`items`) and
//! every mob (`mobs`). What the world, the mesher, the physics, mining, the items and the
//! creative inventory know about a block comes from its line there; what an item is, what it
//! does and where it is listed, from its line in `items`; what a mob is and how it behaves,
//! from its file.

pub mod blocks;
pub mod items;
pub mod mobs;

/// Where an item (a block's too) is in the creative inventory: the tab and the group (each
/// group starts on a new row; in a group, in the order of the tables: the blocks', then the
/// items' files).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Creative {
    /// Not listed.
    None,
    Blocks(u8),
    /// Blocks that do something: stations, storage, lights, beds and doors.
    Functional(u8),
    /// Tools and weapons (guns, their ammunition and parts, grenades) together.
    Tools(u8),
    /// Armor and the bulletproof vest.
    Armor(u8),
    Food(u8),
    /// Spawn eggs and the target dummy.
    Mobs(u8),
    Materials(u8),
}
