//! The target dummy, made in Blockbench (`tools/blockbench/dummy.bbmodel`): crossed wooden
//! feet, a post, a burlap sack of a torso with a target on its front, a crossbar for arms and a
//! sack of a head. The data `bbmodel_to_rust.py` made of it and its texture pages, drawn
//! standing in the world (`entity::mob`), in the hand, dropped and into its item icon.
//!
//! Model space: Blockbench pixels, standing on the origin, up +Y, its front toward +Z. The
//! `body` bone rocks on its origin (the top of the feet) when the dummy is hit.

#[allow(unused_imports, dead_code)]
mod data {
    include!("dummy_data.rs");
}
use data::{BONES, CUBES};
pub use data::PAGES;

use super::viewmodel::{cube_matrix, emit_cube, find_bone};
use crate::world::mesh::Vertex;
use crate::world::textures::tex;
use glam::{Mat4, Quat, Vec2, Vec3};

/// The texture pages (`PAGES` of 128x128, one under the other), loaded into the texture
/// layers from `tex::DUMMY_MODEL`.
pub static PNG: &[u8] = include_bytes!("dummy.png");

/// How tall the model is (Blockbench pixels).
pub const HEIGHT: f32 = 31.0;

/// The dummy where `m` puts its model space, its body tipped by `tilt` (radians toward model
/// +X and +Z).
pub fn emit(out: &mut Vec<Vertex>, m: Mat4, tilt: Vec2, light: [u8; 4], fl: u8) {
    let body = find_bone(BONES, "body");
    let t = Vec3::new(tilt.x, 0.0, tilt.y);
    let angle = t.length();
    let rock = if angle > 1e-4 {
        // Tipped over toward `t`: turned about the horizontal axis across it.
        let pivot = body.map_or(Vec3::ZERO, |b| Vec3::from(BONES[b].origin));
        Mat4::from_translation(pivot)
            * Mat4::from_quat(Quat::from_axis_angle(Vec3::Y.cross(t / angle), angle))
            * Mat4::from_translation(-pivot)
    } else {
        Mat4::IDENTITY
    };
    for c in CUBES {
        let bone = if Some(c.bone) == body { rock } else { Mat4::IDENTITY };
        emit_cube(out, c, m * bone * cube_matrix(c), tex::DUMMY_MODEL, light, fl);
    }
}

/// The dummy `height` blocks tall with its middle at the origin of `m` (an item).
pub fn emit_sized(out: &mut Vec<Vertex>, m: Mat4, height: f32, light: [u8; 4], fl: u8) {
    let k = height / HEIGHT;
    let m = m * Mat4::from_scale(Vec3::splat(k)) * Mat4::from_translation(Vec3::new(0.0, -HEIGHT * 0.5, 0.0));
    emit(out, m, Vec2::ZERO, light, fl);
}
