//! The grenades, made in Blockbench (`tools/blockbench/grenades.bbmodel`): a frag grenade (a
//! segmented olive body) and a smoke grenade (a grey can with a coloured band), each with its
//! fuse, spoon lever and pin ring. The data `bbmodel_to_rust.py` made of it and its texture
//! pages, drawn thrown, in the hand, dropped and into the item icons.
//!
//! Model space: Blockbench pixels, each grenade standing on the origin (its bottom's middle),
//! up +Y, its lever on the +X side.

#[allow(unused_imports, dead_code)]
mod data {
    include!("grenade_data.rs");
}
use data::{BONES, CUBES};
pub use data::PAGES;
use super::viewmodel::Cube;

use super::viewmodel::{cube_matrix, emit_cube, find_bone};
use crate::world::mesh::Vertex;
use crate::world::textures::tex;
use glam::{Mat4, Vec3};

/// The texture pages (`PAGES` of 128x128, one under the other), loaded into the texture
/// layers from `tex::GRENADE_MODEL`.
pub static PNG: &[u8] = include_bytes!("grenade.png");

fn cubes(smoke: bool) -> impl Iterator<Item = &'static Cube> {
    let b = find_bone(BONES, if smoke { "smoke" } else { "frag" });
    CUBES.iter().filter(move |c| Some(c.bone) == b)
}

/// Where a grenade's cubes reach (model space).
pub fn bounds(smoke: bool) -> (Vec3, Vec3) {
    cubes(smoke).fold((Vec3::splat(f32::MAX), Vec3::splat(f32::MIN)), |(lo, hi), c| {
        let m = cube_matrix(c);
        let (a, b) = (Vec3::from(c.from), Vec3::from(c.to));
        let mut lo = lo;
        let mut hi = hi;
        for i in 0..8 {
            let p = m.transform_point3(Vec3::new(
                if i & 1 == 0 { a.x } else { b.x },
                if i & 2 == 0 { a.y } else { b.y },
                if i & 4 == 0 { a.z } else { b.z },
            ));
            lo = lo.min(p);
            hi = hi.max(p);
        }
        (lo, hi)
    })
}

/// A grenade where `m` puts its model space.
pub fn emit(out: &mut Vec<Vertex>, smoke: bool, m: Mat4, light: [u8; 4], fl: u8) {
    for c in cubes(smoke) {
        emit_cube(out, c, m * cube_matrix(c), tex::GRENADE_MODEL, light, fl);
    }
}

/// A grenade `height` blocks tall with its middle at the origin of `m` (thrown, lying about).
pub fn emit_sized(out: &mut Vec<Vertex>, smoke: bool, m: Mat4, height: f32, light: [u8; 4], fl: u8) {
    let (lo, hi) = bounds(smoke);
    let k = height / (hi.y - lo.y).max(1e-3);
    let m = m * Mat4::from_scale(Vec3::splat(k)) * Mat4::from_translation(-(lo + hi) * 0.5);
    emit(out, smoke, m, light, fl);
}
