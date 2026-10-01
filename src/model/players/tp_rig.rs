//! How a player holds each gun as the others see them (third person), made in Blockbench
//! (`tools/blockbench/tp_*.bbmodel`, from `gen_tp.py`): a rig per gun with a `gun` bone (the
//! gun's place, its origin at the right fist on the grip), a `left_hand` bone in it (where
//! the left hand holds it) and a `torso` bone (how far the body turns), and its animations: idle, walk, sprint, crouch, aim, shoot and
//! reload. They are added up here from what the player is doing; `player` points the arms at
//! the hands and turns it all with where the head looks.
//!
//! Model space: the player model's pixels, standing on the origin, facing -Z.

use crate::model::players::player::{PlayerPose, LIMB_SWING_SCALE};
use crate::model::rig::viewmodel::{add_anim, bone_matrices, find_anim, find_bone, Anim, Bone, BonePose};
use crate::item::GunKind;
use glam::{Mat4, Vec3};

/// A gun's rig (its row of `item::WEAPONS`).
fn rig(kind: GunKind) -> (&'static [Bone], &'static [Anim]) {
    (kind.def().tp_bones, kind.def().tp_anims)
}

/// Where the gun is held (a frame at the right fist on its grip, the muzzle toward -Z) and
/// where the left hand is, in the player model's space, before the head's look turns them.
#[derive(Clone, Copy)]
pub struct Held {
    pub gun: Mat4,
    pub left_hand: Vec3,
    /// How far the body is turned about the vertical (radians, + facing more to the left): a
    /// rifle's stance puts the left shoulder forward.
    pub turn: f32,
}

/// The rig's pose for what the player is doing now. Remembered for the last few poses: a
/// player's model asks for it a dozen times a frame (its root, head, arms, the gun's points).
pub fn held(kind: GunKind, p: &PlayerPose) -> Held {
    use std::cell::RefCell;
    type Key = (GunKind, [u32; 9]);
    thread_local! {
        static LAST: RefCell<Vec<(Key, Held)>> = const { RefCell::new(Vec::new()) };
    }
    let f = |v: f32| v.to_bits();
    let opt = |v: Option<f32>| v.map_or(u32::MAX, f32::to_bits);
    let key: Key = (
        kind,
        [
            f(p.gun.aim),
            f(p.sprint),
            opt(p.gun.reload),
            opt(p.gun.reload_time()),
            opt(p.gun.shot),
            f(p.time),
            f(p.limb_swing),
            f(p.limb_amount),
            f(p.crouch),
        ],
    );
    LAST.with_borrow_mut(|last| {
        if let Some((_, h)) = last.iter().find(|(k, _)| *k == key) {
            return *h;
        }
        let h = pose_held(kind, p);
        if last.len() >= 8 {
            last.remove(0);
        }
        last.push((key, h));
        h
    })
}

fn pose_held(kind: GunKind, p: &PlayerPose) -> Held {
    let (bones, anims) = rig(kind);
    let mut pose = vec![BonePose::default(); bones.len()];
    let smooth = |x: f32| {
        let x = x.clamp(0.0, 1.0);
        x * x * (3.0 - 2.0 * x)
    };
    let aim = smooth(p.gun.aim);
    let sprint = smooth(p.sprint) * (1.0 - aim);
    let reloading = p.gun.reload.is_some();
    let mut play = |name: &str, t: f32, weight: f32| {
        if let (Some(an), true) = (find_anim(anims, name), weight > 1e-3) {
            add_anim(&mut pose, an, t, weight, |_| false);
        }
    };
    if let Some(an) = find_anim(anims, "idle") {
        play("idle", p.time.rem_euclid(an.length), 1.0);
    }
    // One walk cycle a full swing of the legs.
    if let Some(an) = find_anim(anims, "walk") {
        let phase = (p.limb_swing * LIMB_SWING_SCALE / std::f32::consts::TAU).rem_euclid(1.0);
        play("walk", phase * an.length, p.limb_amount.min(1.0) * (1.0 - sprint) * (1.0 - 0.7 * aim));
    }
    if let Some(an) = find_anim(anims, "sprint") {
        let phase = (p.limb_swing * LIMB_SWING_SCALE / std::f32::consts::TAU).rem_euclid(1.0);
        play("sprint", phase * an.length, sprint * if reloading { 0.3 } else { 1.0 });
    }
    if let Some(an) = find_anim(anims, "crouch") {
        // The rig lowers the gun with shoulders sunk 3.2 pixels; the model's sink less now.
        play("crouch", an.length, p.crouch.clamp(0.0, 1.0) * crate::model::players::player::SNEAK_DROP / 3.2);
    }
    if let Some(an) = find_anim(anims, "aim") {
        play("aim", aim * an.length, 1.0);
    }
    if let (Some(an), Some(t)) = (find_anim(anims, "shoot"), p.gun.shot) {
        if t < an.length {
            play("shoot", t, 1.0);
        }
    }
    if let Some(an) = find_anim(anims, "reload") {
        // A magazine-fed gun's reload goes as the pistol's; the revolver's as its own (how
        // far, of its length).
        let t = if kind.uses_magazine() { p.gun.reload_time() } else { p.gun.reload.map(|f| f * an.length) };
        if let Some(t) = t {
            play("reload", t.min(an.length), 1.0);
        }
    }
    let (mats, _) = bone_matrices(bones, &pose, Mat4::IDENTITY);
    let at = |name: &str| find_bone(bones, name).map(|b| (mats[b], Vec3::from(bones[b].origin)));
    let gun = at("gun").map_or(Mat4::IDENTITY, |(m, o)| m * Mat4::from_translation(o));
    let left_hand = at("left_hand").map_or(gun.transform_point3(Vec3::ZERO), |(m, o)| m.transform_point3(o));
    let turn = at("torso").map_or(0.0, |(m, _)| (-m.x_axis.z).atan2(m.x_axis.x));
    Held { gun, left_hand, turn }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::item::GUN_KINDS;

    #[test]
    fn every_gun_has_its_rig() {
        for k in GUN_KINDS {
            let (bones, anims) = rig(k);
            for b in ["gun", "left_hand"] {
                assert!(find_bone(bones, b).is_some(), "{k:?} {b}");
            }
            for a in ["idle", "walk", "sprint", "crouch", "aim", "shoot", "reload"] {
                assert!(find_anim(anims, a).is_some(), "{k:?} {a}");
            }
        }
    }
}
