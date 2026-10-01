//! The guns and what goes with them: how they are held (`gun`), their moving parts posed and
//! drawn wherever they are seen (`pistol_view` for the pistol and the AK-47, `revolver_view`,
//! and `gun_view` choosing between them), what they leave in the world (`ballistics`), the
//! gun stations and the grenades. Their Blockbench models' data is in `model::blockbench`.

pub mod ballistics;
pub mod grenade;
pub mod gun;
pub mod gun_station;
pub mod gun_view;
pub mod pistol_view;
pub mod revolver_view;
