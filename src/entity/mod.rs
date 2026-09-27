//! Everything in the world that is not a block: dropped items and falling blocks, block
//! entities (furnaces, chests, crafting tables), mobs, and the player's body and needs.

pub mod block_entity;
pub mod dropped;
pub mod mob;
pub mod player;
pub mod survival;

pub use block_entity::{BlockEntities, Furnace, SMELT_TIME};
pub use dropped::{FallingBlock, ItemEntity};
