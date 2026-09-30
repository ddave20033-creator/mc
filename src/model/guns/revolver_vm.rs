//! The first-person revolver, made in Blockbench (`tools/blockbench/revolver.bbmodel`, from
//! `gen_revolver.py`): the data `bbmodel_to_rust.py` made of it, and its texture pages. Posed
//! and drawn through `revolver_view`.

include!("revolver_vm_data.rs");

/// The texture pages (`PAGES` of 128x128, one under the other), loaded into the texture
/// layers from `tex::REVOLVER_VIEW`.
pub static PNG: &[u8] = include_bytes!("revolver_vm.png");
