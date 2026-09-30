//! The first-person pistol, made in Blockbench (`tools/blockbench/pistol.bbmodel`): the data
//! `bbmodel_to_rust.py` made of it, and its texture pages. Posed and drawn by
//! `hand::HandAnim::build_gun` through `viewmodel`.

include!("pistol_vm_data.rs");

/// The texture pages (`PAGES` of 128x128, one under the other), loaded into the texture
/// layers from `tex::PISTOL_VIEW`.
pub static PNG: &[u8] = include_bytes!("pistol_vm.png");
