//! Everything in the world that is not a block: dropped items and falling blocks, block
//! entities (furnaces, chests, crafting tables), mobs, and the player's body and needs.

pub mod block_entity;
pub mod dropped;
pub mod mob;
pub mod skin_pages;
pub mod player;
pub mod survival;

pub use block_entity::{bench_event, BenchEvent, BenchItem, BlockEntities, Furnace, GunBench, Grilled};
pub use dropped::{FallingBlock, ItemEntity};
