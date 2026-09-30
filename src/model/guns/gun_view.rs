//! The held guns' Blockbench models by kind: the magazine-fed pistol and AK-47 (`pistol_view`,
//! each with its own `Rig`) or the revolver (`revolver_view`). What the hand, the player model,
//! the item and the game need of any of them.

use crate::model::pistol_view::{self as pistol, GunAnim};
use crate::model::revolver_view as revolver;
use crate::model::viewmodel::{Anim, Bone, BonePose, Cube};
use crate::model::{ak_vm, pistol_vm, revolver_vm};
use crate::item::GunKind;
use crate::world::mesh::Vertex;
use glam::{Mat4, Vec3};

/// How much bigger a gun is held than it is modelled (the models are to scale with each
/// other: a Glock 17 and a Ruger GP100 4.2"). On the player model, as big against its fist as a
/// real one is against a real hand (a Minecraft hand is 4 pixels, 22 cm, about 2.6 times a
/// real one; the pistol comes out about 9.5 pixels long); in the first-person view bigger, as
/// view models are.
pub const HELD_SCALE: f32 = 1.3;
pub const MODEL_SCALE: f32 = 1.15;

/// The rear sight's notch (or the scope's eyepiece, with one), model space: aiming brings it
/// to the middle of the view.
pub fn sight_point(kind: GunKind, mods: u8) -> Vec3 {
    match kind {
        GunKind::Pistol if mods & crate::item::gun_mod::SCOPE != 0 => Vec3::new(7.0, 15.5 - 24.0, 5.2 - 34.0),
        GunKind::Pistol => Vec3::new(7.0, 14.3 - 24.0, 6.5 - 34.0),
        GunKind::Revolver => revolver::sight_point(),
        // The rear sight's notch, far forward on the receiver.
        GunKind::Ak => Vec3::new(7.0, 15.1 - 24.0, -18.25 - 34.0),
    }
}

pub fn bones(kind: GunKind) -> &'static [Bone] {
    match kind {
        GunKind::Pistol => pistol_vm::BONES,
        GunKind::Revolver => revolver_vm::BONES,
        GunKind::Ak => ak_vm::BONES,
    }
}

pub fn cubes(kind: GunKind) -> &'static [Cube] {
    match kind {
        GunKind::Pistol => pistol_vm::CUBES,
        GunKind::Revolver => revolver_vm::CUBES,
        GunKind::Ak => ak_vm::CUBES,
    }
}

/// The first texture layer of the gun's pages as dirty as `dirt`.
pub fn layers(kind: GunKind, dirt: u8) -> u32 {
    match kind {
        GunKind::Pistol | GunKind::Ak => pistol::layers(pistol::rig(kind), dirt),
        GunKind::Revolver => revolver::layers(dirt),
    }
}

/// An item that is a piece of one of the Blockbench guns on its own (a part, an attachment, a
/// magazine or speedloader, a round): which gun's model, its bones posed as it lies, a
/// magazine's rounds (in it, of how many it holds: the pistol's shows them in its witness
/// holes), and how to turn the model for the item: the muzzle end to +X, its right side
/// toward +Z.
pub fn item_rig(st: &crate::item::Stack) -> Option<(GunKind, u64, Vec<BonePose>, Option<(u8, u8)>, Mat4)> {
    use crate::item::*;
    for r in [&pistol::PISTOL, &pistol::AK] {
        if let Some((bones, pose, mag, upright)) = pistol::bench::item_rig(r, st) {
            return Some((r.kind, bones, pose, mag, upright));
        }
    }
    let side = Mat4::from_rotation_y((-90f32).to_radians());
    if let Some(p) = REVOLVER_PARTS.iter().position(|&i| i == st.item) {
        return Some((GunKind::Revolver, revolver::bench::part(p), revolver::bench::pose([0.0; crate::model::gun::PARTS], 0), None, side));
    }
    if st.item == MAGNUM_ROUND {
        return Some((GunKind::Revolver, revolver::bench::round(), revolver::bench::round_pose(), None, side));
    }
    if st.item == SPEEDLOADER {
        // Its rounds toward the viewer.
        let up = Mat4::from_rotation_x((-70f32).to_radians());
        return Some((GunKind::Revolver, revolver::bench::speedloader(), revolver::bench::loader_pose(gun_rounds(st)), None, up));
    }
    None
}

pub fn anims(kind: GunKind) -> &'static [Anim] {
    match kind {
        GunKind::Pistol => pistol_vm::ANIMS,
        GunKind::Revolver => revolver_vm::ANIMS,
        GunKind::Ak => ak_vm::ANIMS,
    }
}

/// The gun's own bone (the whole gun, without the arms).
pub fn gun_bone(kind: GunKind) -> usize {
    match kind {
        GunKind::Pistol | GunKind::Ak => crate::model::viewmodel::find_bone(bones(kind), pistol::rig(kind).gun_bone).unwrap_or(0),
        GunKind::Revolver => revolver::gun_bone(),
    }
}

pub fn rest_pose(kind: GunKind) -> Vec<BonePose> {
    match kind {
        GunKind::Pistol | GunKind::Ak => pistol::rest_pose(pistol::rig(kind)),
        GunKind::Revolver => revolver::rest_pose(),
    }
}

/// The gun's moving parts from what it is doing, and its attachments shown or not.
pub fn add_gun_anims(kind: GunKind, pose: &mut [BonePose], g: &GunAnim, mods: u8, parts_only: bool) {
    match kind {
        GunKind::Pistol | GunKind::Ak => {
            let r = pistol::rig(kind);
            pistol::add_gun_anims(r, pose, g, parts_only);
            pistol::apply_mods(r, pose, mods);
        }
        GunKind::Revolver => revolver::add_gun_anims(pose, g, parts_only),
    }
}

/// The posed gun's bones where `root` puts the model.
pub fn matrices(kind: GunKind, g: &GunAnim, mods: u8, parts_only: bool, root: Mat4) -> (Vec<Mat4>, Vec<bool>) {
    let mut pose = rest_pose(kind);
    add_gun_anims(kind, &mut pose, g, mods, parts_only);
    crate::model::viewmodel::bone_matrices(bones(kind), &pose, root)
}

/// The gun's cubes (see `pistol_view::emit_pistol`).
#[allow(clippy::too_many_arguments)]
pub fn emit(
    kind: GunKind,
    out: &mut Vec<Vertex>,
    glass: Option<&mut Vec<Vertex>>,
    mats: &[Mat4],
    shown: &[bool],
    eyepiece_view: bool,
    dirt: u8,
    lamp: bool,
    g: &GunAnim,
    light: [u8; 4],
    fl: u8,
) {
    match kind {
        GunKind::Pistol | GunKind::Ak => pistol::emit_pistol(pistol::rig(kind), out, glass, mats, shown, eyepiece_view, dirt, lamp, pistol::shown_mag(g), light, fl),
        GunKind::Revolver => revolver::emit(out, mats, shown, dirt, light, fl),
    }
}

/// Where the bullet leaves: bone and model point.
pub fn muzzle(kind: GunKind, mods: u8) -> (usize, Vec3) {
    match kind {
        GunKind::Pistol | GunKind::Ak => pistol::muzzle(pistol::rig(kind), mods),
        GunKind::Revolver => revolver::muzzle(),
    }
}

/// Where the spent cases come out (a revolver's: its cylinder, when reloading).
pub fn eject(kind: GunKind) -> (usize, Vec3) {
    match kind {
        GunKind::Pistol | GunKind::Ak => pistol::eject(pistol::rig(kind)),
        GunKind::Revolver => revolver::eject(),
    }
}

/// The laser sight's lens and the weapon light's (the muzzle on a gun without a rail).
pub fn laser(kind: GunKind) -> (usize, Vec3) {
    match kind {
        GunKind::Pistol | GunKind::Ak => pistol::laser(pistol::rig(kind)),
        GunKind::Revolver => revolver::muzzle(),
    }
}

pub fn light(kind: GunKind) -> (usize, Vec3) {
    match kind {
        GunKind::Pistol | GunKind::Ak => pistol::light(pistol::rig(kind)),
        GunKind::Revolver => revolver::muzzle(),
    }
}

pub fn eyepiece(kind: GunKind, mats: &[Mat4], shown: &[bool]) -> Option<(Vec3, Vec3, Vec3, f32)> {
    match kind {
        GunKind::Pistol | GunKind::Ak => pistol::eyepiece(pistol::rig(kind), mats, shown),
        GunKind::Revolver => None,
    }
}

pub fn to_gun_space(kind: GunKind) -> Mat4 {
    match kind {
        GunKind::Pistol | GunKind::Ak => pistol::to_gun_space(pistol::rig(kind)),
        GunKind::Revolver => revolver::to_gun_space(),
    }
}

pub fn rest_point_in_gun_space(kind: GunKind, point: (usize, Vec3)) -> Vec3 {
    match kind {
        GunKind::Pistol | GunKind::Ak => pistol::rest_point_in_gun_space(pistol::rig(kind), point),
        GunKind::Revolver => revolver::rest_point_in_gun_space(point),
    }
}

/// One cube of a round (see `round_parts`): the cube, where it is in the round's own frame,
/// and the first texture layer of its model's pages.
pub struct RoundPart {
    pub cube: &'static Cube,
    pub m: Mat4,
    pub layer: u32,
}

/// A round (9 mm `BULLET`, `MAGNUM_ROUND` or `RIFLE_ROUND`; `spent`: only its fired case) as the guns' own
/// models have it, the same everywhere it is seen (in the boxes of rounds, on the ground):
/// in its own frame, its head's middle at the origin and its nose up (+Y), in the guns'
/// model units.
pub fn round_parts(ammo: crate::item::ItemId, spent: bool) -> &'static [RoundPart] {
    use std::sync::OnceLock;
    static CACHE: OnceLock<[Vec<RoundPart>; 6]> = OnceLock::new();
    let all = CACHE.get_or_init(|| {
        let build = |kind: GunKind, spent: bool| {
            let (set, pose) = match kind {
                GunKind::Pistol | GunKind::Ak => (pistol::bench::round(pistol::rig(kind)), pistol::rest_pose(pistol::rig(kind))),
                GunKind::Revolver => (revolver::bench::round(), revolver::bench::round_pose()),
            };
            let (mats, _) = crate::model::viewmodel::bone_matrices(bones(kind), &pose, Mat4::IDENTITY);
            let is_bullet = |c: &Cube| c.name.contains("bullet") || c.name.contains("nose");
            let own: Vec<&'static Cube> = cubes(kind)
                .iter()
                .filter(|c| set & (1 << c.bone) != 0 && !(spent && is_bullet(c)))
                .collect();
            // The head: the middle of the case's back.
            let (mut lo, mut hi) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
            for c in own.iter().filter(|c| !is_bullet(c)) {
                let m = mats[c.bone] * crate::model::viewmodel::cube_matrix(c);
                for p in [Vec3::from(c.from), Vec3::from(c.to)] {
                    let q = m.transform_point3(p);
                    lo = lo.min(q);
                    hi = hi.max(q);
                }
            }
            let head = Vec3::new((lo.x + hi.x) * 0.5, (lo.y + hi.y) * 0.5, hi.z);
            // The models' rounds point -Z: turned nose up.
            let frame = Mat4::from_rotation_x(std::f32::consts::FRAC_PI_2) * Mat4::from_translation(-head);
            own.into_iter()
                .map(|c| RoundPart { cube: c, m: frame * mats[c.bone] * crate::model::viewmodel::cube_matrix(c), layer: layers(kind, 0) })
                .collect::<Vec<_>>()
        };
        [
            build(GunKind::Pistol, false),
            build(GunKind::Pistol, true),
            build(GunKind::Revolver, false),
            build(GunKind::Revolver, true),
            build(GunKind::Ak, false),
            build(GunKind::Ak, true),
        ]
    });
    let kind = match ammo {
        crate::item::MAGNUM_ROUND => 1,
        crate::item::RIFLE_ROUND => 2,
        _ => 0,
    };
    let i = kind * 2 + spent as usize;
    &all[i]
}

/// How long a round is (or its case, `spent`) and how wide at its widest (the rim), in the
/// guns' model units.
pub fn round_size(ammo: crate::item::ItemId, spent: bool) -> (f32, f32) {
    let (mut lo, mut hi) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
    for p in round_parts(ammo, spent) {
        for q in [Vec3::from(p.cube.from), Vec3::from(p.cube.to)] {
            let q = p.m.transform_point3(q);
            lo = lo.min(q);
            hi = hi.max(q);
        }
    }
    (hi.y - lo.y, (hi.x - lo.x).max(hi.z - lo.z))
}

/// Draws a round (see `round_parts`), `m` from its own frame to the world.
pub fn emit_round(out: &mut Vec<Vertex>, ammo: crate::item::ItemId, spent: bool, m: Mat4, light: [u8; 4], fl: u8) {
    for p in round_parts(ammo, spent) {
        crate::model::viewmodel::emit_cube(out, p.cube, m * p.m, p.layer, light, fl);
    }
}

#[cfg(test)]
mod round_tests {
    use super::*;
    use crate::item::{BULLET, MAGNUM_ROUND, RIFLE_ROUND};

    #[test]
    fn the_rounds_are_as_long_as_the_real_ones() {
        let (nine, nine_w) = round_size(BULLET, false);
        let (magnum, _) = round_size(MAGNUM_ROUND, false);
        let (nine_case, _) = round_size(BULLET, true);
        let (magnum_case, _) = round_size(MAGNUM_ROUND, true);
        // 9x19 mm: 30 mm long, 19 mm case, 9.9 mm across; .357 Magnum: 40 mm, 33 mm case.
        assert!((nine / nine_w - 30.0 / 9.9).abs() < 0.35, "{nine} {nine_w}");
        assert!((magnum / nine - 40.0 / 30.0).abs() < 0.12, "{magnum} {nine}");
        assert!((magnum_case / nine_case - 33.0 / 19.0).abs() < 0.15, "{magnum_case} {nine_case}");
        // 7.62x39 mm: 56 mm long, a 39 mm case.
        let (rifle, _) = round_size(RIFLE_ROUND, false);
        let (rifle_case, _) = round_size(RIFLE_ROUND, true);
        assert!((rifle / nine - 56.0 / 30.0).abs() < 0.2, "{rifle} {nine}");
        assert!((rifle_case / nine_case - 39.0 / 19.0).abs() < 0.3, "{rifle_case} {nine_case}");
        // Nose up, the head at the bottom.
        for (ammo, spent) in [(BULLET, false), (MAGNUM_ROUND, false), (BULLET, true)] {
            let lowest = round_parts(ammo, spent)
                .iter()
                .flat_map(|p| [p.m.transform_point3(Vec3::from(p.cube.from)), p.m.transform_point3(Vec3::from(p.cube.to))])
                .fold(f32::MAX, |a, q| a.min(q.y));
            assert!(lowest > -0.2, "{lowest}");
        }
    }
}
