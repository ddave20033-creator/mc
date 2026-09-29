//! Chopping a tree with an axe, as made in Blockbench (`tools/blockbench/chop.bbmodel`, from
//! `gen_chop_anim.py`): the player's rig with the axe in both hands, and its `chop`
//! animation (drawn back, swung round level, the edge biting into the trunk, stuck a
//! moment, pulled out). The first-person view draws the axe and the forearms from it, the
//! player model reaches for the axe where it has it.
//!
//! Model space: the player model's pixels, standing on the origin, facing -Z.

#[allow(unused_imports, dead_code)]
mod data {
    include!("tp_chop_data.rs");
}

use super::viewmodel::{add_anim, bone_matrices, find_anim, find_bone, BonePose};
use glam::{Mat4, Vec3};

/// Seconds into the chop when the axe bites in, and the whole chop (the animation's length).
pub const HIT: f32 = 0.38;
pub const LENGTH: f32 = 0.8;

/// Where the axe is held at rest (the right fist's middle, on the handle's end): the axe's
/// handle goes up (+Y) from it, its edge toward -Z.
pub const GRIP: Vec3 = Vec3::new(6.0, 13.2, 0.0);
/// The left hand's place on the handle, and the middle of the edge (at rest).
pub const LEFT_GRIP: Vec3 = Vec3::new(6.0, 18.2, 0.0);
pub const EDGE: Vec3 = Vec3::new(6.0, 25.95, -5.6);

/// The forearms (the lower halves of the arms) at rest, and the model's eye.
pub const RIGHT_FOREARM: (Vec3, Vec3) = (Vec3::new(4.04, 12.0, -1.96), Vec3::new(7.96, 18.0, 1.96));
pub const LEFT_FOREARM: (Vec3, Vec3) = (Vec3::new(-7.96, 12.0, -1.96), Vec3::new(-4.04, 18.0, 1.96));
pub const EYE: Vec3 = Vec3::new(0.0, 28.6, -1.0);

/// Blocks per model pixel.
pub const PX: f32 = 1.8 / 32.0;

/// Where the rig's parts are `t` seconds into the chop: each part's rest place (model
/// space) to where it is.
pub struct ChopPose {
    pub axe: Mat4,
    pub right_hand: Mat4,
    pub left_hand: Mat4,
}

pub fn pose(t: f32) -> ChopPose {
    let bones = data::BONES;
    let mut p = vec![BonePose::default(); bones.len()];
    if let Some(anim) = find_anim(data::ANIMS, "chop") {
        add_anim(&mut p, anim, t.clamp(0.0, LENGTH - 1e-4), 1.0, |_| false);
    }
    let (mats, _) = bone_matrices(bones, &p, Mat4::IDENTITY);
    let at = |name: &str| find_bone(bones, name).map_or(Mat4::IDENTITY, |i| mats[i]);
    ChopPose {
        axe: at("axe"),
        right_hand: at("right_hand"),
        left_hand: at("left_hand"),
    }
}

/// The held axe item (`emit_held`'s flat sprite, one block across, its handle corner to
/// corner) placed on the rig's axe at rest, in model pixels: stood up so the handle runs up
/// from the grip, its head at the top, the blade's edge toward -Z.
pub fn axe_item() -> Mat4 {
    Mat4::from_translation(GRIP)
        * Mat4::from_scale(Vec3::splat(1.0 / PX))
        * Mat4::from_rotation_y(-std::f32::consts::FRAC_PI_2)
        * Mat4::from_translation(Vec3::new(0.0, 0.36, 0.0))
        * Mat4::from_scale(Vec3::splat(0.72))
        * Mat4::from_rotation_z(std::f32::consts::FRAC_PI_4)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_animation_is_as_long_as_the_game_thinks() {
        let anim = find_anim(data::ANIMS, "chop").expect("chop animation");
        assert!((anim.length - LENGTH).abs() < 1e-3);
    }

    #[test]
    fn the_axe_bites_in_ahead_of_the_player() {
        let edge = pose(HIT).axe.transform_point3(EDGE);
        assert!(edge.z < -10.0, "{edge}");
    }
}

