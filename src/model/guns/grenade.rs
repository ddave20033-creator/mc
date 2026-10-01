//! The grenades, made in Blockbench (`tools/blockbench/grenades.bbmodel`): a frag grenade (a
//! segmented olive body) and a smoke grenade (a grey can with a coloured band), each with its
//! fuse, spoon lever and pin ring. The data `bbmodel_to_rust.py` made of it and its texture
//! pages, drawn thrown, in the hand, dropped and into the item icons.
//!
//! Model space: Blockbench pixels, each grenade standing on the origin (its bottom's middle),
//! up +Y, its lever on the +X side. The pin (with its ring) and the spoon are bones of their
//! own: the pin comes out with the `pull_pin` animation, and a thrown grenade has neither.

use crate::model::blockbench::grenade as data;
use data::{ANIMS, BONES, CUBES};
use crate::model::rig::viewmodel::Cube;

use crate::model::rig::viewmodel::{add_anim, bone_matrices, cube_matrix, emit_cube, find_anim, find_bone, BonePose};
use crate::world::mesh::Vertex;
use crate::textures::tex;
use glam::{Mat4, Vec3};

/// Readying a grenade (seconds from the button going down): it comes up in `RAISE_TIME`,
/// then the other hand pulls the pin (`pull_pin`, `PULL_TIME` long); the throw gets harder
/// until `FULL_POWER`. The same in the first-person hand, on the body and for the game.
pub const RAISE_TIME: f32 = 0.2;
pub const PULL_TIME: f32 = 0.85;
pub const FULL_POWER: f32 = 3.0;

/// How hard a grenade readied for `t` seconds would be thrown (0..1).
pub fn power(t: f32) -> f32 {
    (t / FULL_POWER).clamp(0.0, 1.0)
}

/// How a grenade looks: its pin being pulled (`pull_pin` this far along, seconds), or gone,
/// and its spoon gone (thrown).
#[derive(Clone, Copy, Default, Debug)]
pub struct Look {
    pub pull: Option<f32>,
    pub pin_out: bool,
    pub spoon_off: bool,
}

impl Look {
    /// Thrown: no pin, no spoon.
    pub const THROWN: Look = Look { pull: None, pin_out: true, spoon_off: true };

    /// Readied for `t` seconds: the pin being pulled, then gone.
    pub fn readied(t: f32) -> Look {
        let pull = t - RAISE_TIME;
        Look { pull: (pull > 0.0).then_some(pull.min(PULL_TIME)), pin_out: pull >= PULL_TIME, spoon_off: false }
    }
}

fn root(smoke: bool) -> usize {
    find_bone(BONES, if smoke { "smoke" } else { "frag" }).unwrap_or(0)
}

/// Whether bone `b` is `root` or in it.
fn under(mut b: usize, root: usize) -> bool {
    loop {
        if b == root {
            return true;
        }
        let p = BONES[b].parent;
        if p < 0 {
            return false;
        }
        b = p as usize;
    }
}

fn cubes(smoke: bool) -> impl Iterator<Item = &'static Cube> {
    let r = root(smoke);
    CUBES.iter().filter(move |c| under(c.bone, r))
}

/// Where a grenade's cubes reach (model space, as it rests).
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

/// A grenade where `m` puts its model space, looking as `look` says. Returns where the
/// middle of its pin's ring is (where `m` puts it), for the hand pulling it.
pub fn emit(out: &mut Vec<Vertex>, smoke: bool, m: Mat4, look: Look, light: [u8; 4], fl: u8) -> Vec3 {
    let r = root(smoke);
    let name = if smoke { "smoke" } else { "frag" };
    let pin = find_bone(BONES, &format!("{name}_pin"));
    let spoon = find_bone(BONES, &format!("{name}_spoon"));
    let mut pose = vec![BonePose::default(); BONES.len()];
    if let (Some(t), Some(anim)) = (look.pull, find_anim(ANIMS, "pull_pin")) {
        add_anim(&mut pose, anim, t, 1.0, |b| !under(b, r));
    }
    for (bone, gone) in [(pin, look.pin_out), (spoon, look.spoon_off)] {
        if let (Some(b), true) = (bone, gone) {
            pose[b].scale = Vec3::ZERO;
        }
    }
    let (mats, shown) = bone_matrices(BONES, &pose, m);
    for c in CUBES.iter().filter(|c| under(c.bone, r) && shown[c.bone]) {
        emit_cube(out, c, mats[c.bone] * cube_matrix(c), tex::GRENADE_MODEL, light, fl);
    }
    pin.map_or(m.transform_point3(Vec3::ZERO), |b| mats[b].transform_point3(Vec3::from(BONES[b].origin)))
}

/// Where `m` must put a grenade `height` blocks tall so its middle (as it rests) is at the
/// origin of `m`.
pub fn sized(smoke: bool, m: Mat4, height: f32) -> Mat4 {
    let (lo, hi) = bounds(smoke);
    let k = height / (hi.y - lo.y).max(1e-3);
    m * Mat4::from_scale(Vec3::splat(k)) * Mat4::from_translation(-(lo + hi) * 0.5)
}

/// A grenade `height` blocks tall with its middle at the origin of `m` (lying about, in the
/// inventory), whole.
pub fn emit_sized(out: &mut Vec<Vertex>, smoke: bool, m: Mat4, height: f32, light: [u8; 4], fl: u8) {
    emit(out, smoke, sized(smoke, m, height), Look::default(), light, fl);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_pin_comes_out_and_goes() {
        for smoke in [false, true] {
            let mut out = Vec::new();
            let rest = emit(&mut out, smoke, Mat4::IDENTITY, Look::default(), [0; 4], 0);
            let whole = out.len();
            let mut out = Vec::new();
            let pulled = emit(&mut out, smoke, Mat4::IDENTITY, Look { pull: Some(0.35), ..Look::default() }, [0; 4], 0);
            // Out to the side, off the fuse (the pin is 2.9 long).
            assert!(pulled.x < rest.x - 3.0, "{rest} -> {pulled}");
            assert_eq!(out.len(), whole);
            let mut out = Vec::new();
            emit(&mut out, smoke, Mat4::IDENTITY, Look::THROWN, [0; 4], 0);
            assert!(out.len() < whole && !out.is_empty());
        }
    }
}
