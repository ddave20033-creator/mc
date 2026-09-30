//! The player: the blocky player model (`player`), the first-person hand (`hand`), how a
//! player holds each gun as the others see them (`tp_rig`) and chopping with an axe
//! (`chop_rig`).
//!
//! (Named `players` and not `player`, because `player` is re-exported from `crate::model`
//! under its old path.)

pub mod chop_rig;
pub mod hand;
pub mod player;
pub mod tp_rig;
