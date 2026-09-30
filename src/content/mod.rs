//! The game's content, defined in one place: every block (`blocks`) and every mob (`mobs`).
//! What the world, the mesher, the physics, mining, the items and the creative inventory know
//! about a block comes from its line there; what a mob is and how it behaves, from its file.

pub mod blocks;
pub mod mobs;
