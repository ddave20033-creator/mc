//! The models made in Blockbench, as `tools/blockbench/bbmodel_to_rust.py` turned them into
//! game data: a module each with its bones, cubes and animations (`BONES`, `CUBES`, `ANIMS`)
//! and, next to it, its texture pages (`<name>.png`, `PNG`, `PAGES` of them). Generated: do
//! not edit these, change the `.bbmodel` in `tools/blockbench/` and run the script again.
//! The code that poses and draws them is elsewhere in `model` (`guns`, `items`, `players`).

// (A model's data holds more than its code reads: anchors, animations kept for later.)
#![allow(dead_code, unused_imports)]

/// The first-person pistol (`pistol.bbmodel`).
pub mod pistol_vm;
/// The first-person revolver (`revolver.bbmodel`).
pub mod revolver_vm;
/// The first-person AK-47 (`ak.bbmodel`).
pub mod ak_vm;
/// The frag and the smoke grenade (`grenades.bbmodel`).
pub mod grenade;
/// The gun station (`gun_station.bbmodel`).
pub mod gun_station;
/// The rifle station, three blocks wide (`rifle_station.bbmodel`).
pub mod rifle_station;
/// The target dummy (`dummy.bbmodel`).
pub mod dummy;
/// The fishing rod and its bobber (`fishing_rod.bbmodel`).
pub mod fishing_rod;
/// How a player holds the pistol, seen by the others (`tp_pistol.bbmodel`).
pub mod tp_pistol;
/// How a player holds the revolver (`tp_revolver.bbmodel`).
pub mod tp_revolver;
/// How a player holds the AK-47 (`tp_ak.bbmodel`).
pub mod tp_ak;
/// The player chopping with an axe (`chop.bbmodel`).
pub mod tp_chop;
