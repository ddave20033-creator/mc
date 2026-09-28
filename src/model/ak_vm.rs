//! The first-person AK-47, made in Blockbench (`tools/blockbench/ak.bbmodel`, from
//! `gen_ak.py`): the data `bbmodel_to_rust.py` made of it, and its texture pages. Posed and
//! drawn through `pistol_view` (its `Rig`), like the pistol.

include!("ak_vm_data.rs");

/// The texture pages (`PAGES` of 128x128, one under the other), loaded into the texture
/// layers from `tex::AK_VIEW`.
pub static PNG: &[u8] = include_bytes!("ak_vm.png");
