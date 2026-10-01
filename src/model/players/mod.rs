//! The player: the blocky player model (`player`), the first-person hand (`hand`), how a
//! player holds each gun as the others see them (`tp_rig`) and chopping with an axe
//! (`chop_rig`). The rigs' Blockbench data is in `model::blockbench` (`tp_*`).

pub mod chop_rig;
pub mod hand;
pub mod player;
pub mod tp_rig;
