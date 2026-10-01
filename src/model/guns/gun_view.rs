//! The held guns' Blockbench models by kind: a magazine-fed gun's (`pistol_view`, each with
//! its own `Rig`, named in its row of `item::WEAPONS`) or the revolver's (`revolver_view`).
//! What the hand, the player model, the item and the game need of any of them, and the helpers
//! both kinds of model share.

use crate::model::guns::pistol_view::{self as pistol, GunAnim, Rig};
use crate::model::guns::revolver_view as revolver;
use crate::model::blockbench::revolver_vm;
use crate::model::rig::viewmodel::{find_bone, Anim, Bone, BonePose, Cube};
use crate::item::{GunKind, GUN_KINDS};
use crate::world::mesh::Vertex;
use crate::textures::tex;
use glam::{Mat4, Vec3};

/// How much bigger a gun is held than it is modelled (the models are to scale with each
/// other: a Glock 17 and a Ruger GP100 4.2"). On the player model, as big against its fist as a
/// real one is against a real hand (a Minecraft hand is 4 pixels, 22 cm, about 2.6 times a
/// real one; the pistol comes out about 9.5 pixels long); in the first-person view bigger, as
/// view models are.
pub const HELD_SCALE: f32 = 1.3;
pub const MODEL_SCALE: f32 = 1.15;

/// A magazine-fed gun's model (`pistol_view`), or None for the revolver (`revolver_view`):
/// what each function here goes by.
fn rig(kind: GunKind) -> Option<&'static Rig> {
    kind.magazine().map(|m| m.rig)
}

/// The rear sight's notch (or the scope's eyepiece, with one), model space: aiming brings it
/// to the middle of the view.
pub fn sight_point(kind: GunKind, mods: u8) -> Vec3 {
    match rig(kind) {
        Some(r) => pistol::sight_point(r, mods),
        None => revolver::sight_point(),
    }
}

pub fn bones(kind: GunKind) -> &'static [Bone] {
    rig(kind).map_or(revolver_vm::BONES, |r| r.bones)
}

pub fn cubes(kind: GunKind) -> &'static [Cube] {
    rig(kind).map_or(revolver_vm::CUBES, |r| r.cubes)
}

pub fn anims(kind: GunKind) -> &'static [Anim] {
    rig(kind).map_or(revolver_vm::ANIMS, |r| r.anims)
}

/// The first texture layer of the gun's pages as dirty as `dirt`.
pub fn layers(kind: GunKind, dirt: u8) -> u32 {
    match rig(kind) {
        Some(r) => pistol::layers(r, dirt),
        None => revolver::layers(dirt),
    }
}

/// An item that is a piece of one of the Blockbench guns on its own (a part, an attachment, a
/// magazine or speedloader, a round): which gun's model, its bones posed as it lies, a
/// magazine's rounds (in it, of how many it holds: the pistol's shows them in its witness
/// holes), and how to turn the model for the item: the muzzle end to +X, its right side
/// toward +Z.
pub fn item_rig(st: &crate::item::Stack) -> Option<(GunKind, u64, Vec<BonePose>, Option<(u8, u8)>, Mat4)> {
    use crate::item::*;
    for r in GUN_KINDS.into_iter().filter_map(rig) {
        if let Some((bones, pose, mag, upright)) = pistol::bench::item_rig(r, st) {
            return Some((r.kind, bones, pose, mag, upright));
        }
    }
    let side = Mat4::from_rotation_y((-90f32).to_radians());
    if let Some(p) = REVOLVER_PARTS.iter().position(|&i| i == st.item) {
        return Some((GunKind::Revolver, revolver::bench::part(p), revolver::bench::pose([0.0; crate::model::guns::gun::PARTS], 0), None, side));
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

/// The gun's own bone (the whole gun, without the arms).
pub fn gun_bone(kind: GunKind) -> usize {
    match rig(kind) {
        Some(r) => find_bone(r.bones, r.gun_bone).unwrap_or(0),
        None => revolver::gun_bone(),
    }
}

pub fn rest_pose(kind: GunKind) -> Vec<BonePose> {
    match rig(kind) {
        Some(r) => pistol::rest_pose(r),
        None => revolver::rest_pose(),
    }
}

/// The gun's moving parts from what it is doing, and its attachments shown or not.
pub fn add_gun_anims(kind: GunKind, pose: &mut [BonePose], g: &GunAnim, mods: u8, parts_only: bool) {
    match rig(kind) {
        Some(r) => {
            pistol::add_gun_anims(r, pose, g, parts_only);
            pistol::apply_mods(r, pose, mods);
        }
        None => revolver::add_gun_anims(pose, g, parts_only),
    }
}

/// The posed gun's bones where `root` puts the model.
pub fn matrices(kind: GunKind, g: &GunAnim, mods: u8, parts_only: bool, root: Mat4) -> (Vec<Mat4>, Vec<bool>) {
    let mut pose = rest_pose(kind);
    add_gun_anims(kind, &mut pose, g, mods, parts_only);
    crate::model::rig::viewmodel::bone_matrices(bones(kind), &pose, root)
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
    match rig(kind) {
        Some(r) => pistol::emit_pistol(r, out, glass, mats, shown, eyepiece_view, dirt, lamp, pistol::shown_mag(g), light, fl),
        None => revolver::emit(out, mats, shown, dirt, light, fl),
    }
}

/// Where the bullet leaves: bone and model point.
pub fn muzzle(kind: GunKind, mods: u8) -> (usize, Vec3) {
    rig(kind).map_or_else(revolver::muzzle, |r| pistol::muzzle(r, mods))
}

/// Where the spent cases come out (a revolver's: its cylinder, when reloading).
pub fn eject(kind: GunKind) -> (usize, Vec3) {
    rig(kind).map_or_else(revolver::eject, pistol::eject)
}

/// The laser sight's lens and the weapon light's (the muzzle on a gun without a rail).
pub fn laser(kind: GunKind) -> (usize, Vec3) {
    rig(kind).map_or_else(revolver::muzzle, pistol::laser)
}

pub fn light(kind: GunKind) -> (usize, Vec3) {
    rig(kind).map_or_else(revolver::muzzle, pistol::light)
}

pub fn eyepiece(kind: GunKind, mats: &[Mat4], shown: &[bool]) -> Option<(Vec3, Vec3, Vec3, f32)> {
    rig(kind).and_then(|r| pistol::eyepiece(r, mats, shown))
}

pub fn to_gun_space(kind: GunKind) -> Mat4 {
    model_to_gun_space(kind, bones(kind))
}

pub fn rest_point_in_gun_space(kind: GunKind, point: (usize, Vec3)) -> Vec3 {
    rest_point(bones(kind), || rest_pose(kind), || to_gun_space(kind), point)
}

/// The bones that are the arms and what holds a gun, not the gun's own parts: with
/// `parts_only` (the player model holds it with its own arms) those of `bones` that are left
/// still, otherwise none.
pub(crate) fn holding(bones: &[Bone], parts_only: bool) -> Vec<usize> {
    const HOLDING: [&str; 5] = ["viewmodel", "right_arm", "right_arm_mesh", "left_arm", "left_arm_mesh"];
    if parts_only {
        HOLDING.iter().filter_map(|n| find_bone(bones, n)).collect()
    } else {
        Vec::new()
    }
}

/// A bone and everything under it (bit i: bone i).
pub(crate) fn subtree(bones: &[Bone], name: &str) -> u64 {
    let Some(root) = find_bone(bones, name) else { return 0 };
    let mut set: u64 = 1 << root;
    // Parents come before their children.
    for (i, b) in bones.iter().enumerate() {
        if b.parent >= 0 && set & (1 << b.parent) != 0 {
            set |= 1 << i;
        }
    }
    set
}

/// The first texture layer of a model's pages (the clean ones at `view`, `pages` of them a
/// set) as dirty as `dirt` (`pistol_view::dirt_level`).
pub(crate) fn dirty_layer(view: u32, pages: u32, dirt: u8) -> u32 {
    view + (dirt as u32).min(tex::PISTOL_DIRT_LEVELS - 1) * pages
}

/// From a Blockbench gun's model space to the old gun space (`gun::Spec`: the muzzle +X, the
/// right side +Z, about a centimetre a unit), so it sits where the old pistol did: the right
/// fist's middle on the grip at the spec's `hand`, all the guns at the same scale.
fn model_to_gun_space(kind: GunKind, bones: &[Bone]) -> Mat4 {
    let spec = crate::model::guns::gun::spec(kind);
    let fist = find_bone(bones, "right_arm_mesh").map_or(Vec3::ZERO, |b| Vec3::from(bones[b].origin));
    // The old pistol is 18.2 gun units long, the Blockbench one 21.8 pixels.
    let scale = 18.2 / 21.8;
    Mat4::from_translation(spec.hand)
        * Mat4::from_rotation_y((-90.0f32).to_radians())
        * Mat4::from_scale(Vec3::splat(scale))
        * Mat4::from_translation(-fist)
}

/// A model point of a gun at rest, in the old gun space (for the third-person muzzle,
/// ejection port and laser, see `player::gun_point`). The rest pose's bone matrices are made
/// once for each model (asked for every frame for the muzzle, the ejection port and the
/// light).
fn rest_point(bones: &'static [Bone], rest: impl FnOnce() -> Vec<BonePose>, root: impl FnOnce() -> Mat4, (b, p): (usize, Vec3)) -> Vec3 {
    use std::cell::RefCell;
    use std::rc::Rc;
    thread_local! {
        static REST: RefCell<Vec<(usize, Rc<Vec<Mat4>>)>> = const { RefCell::new(Vec::new()) };
    }
    let id = bones.as_ptr() as usize;
    let mats = REST.with_borrow_mut(|cache| match cache.iter().find(|(k, _)| *k == id) {
        Some((_, m)) => m.clone(),
        None => {
            let (mats, _) = crate::model::rig::viewmodel::bone_matrices(bones, &rest(), root());
            let m = Rc::new(mats);
            cache.push((id, m.clone()));
            m
        }
    });
    mats[b].transform_point3(p)
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
    // Each gun's round and its case, in the order of `GUN_KINDS`.
    static CACHE: OnceLock<Vec<[Vec<RoundPart>; 2]>> = OnceLock::new();
    let all = CACHE.get_or_init(|| {
        let build = |kind: GunKind, spent: bool| {
            let (set, pose) = match rig(kind) {
                Some(r) => (pistol::bench::round(r), pistol::rest_pose(r)),
                None => (revolver::bench::round(), revolver::bench::round_pose()),
            };
            let (mats, _) = crate::model::rig::viewmodel::bone_matrices(bones(kind), &pose, Mat4::IDENTITY);
            let is_bullet = |c: &Cube| c.name.contains("bullet") || c.name.contains("nose");
            let own: Vec<&'static Cube> = cubes(kind)
                .iter()
                .filter(|c| set & (1 << c.bone) != 0 && !(spent && is_bullet(c)))
                .collect();
            // The head: the middle of the case's back.
            let (mut lo, mut hi) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
            for c in own.iter().filter(|c| !is_bullet(c)) {
                let m = mats[c.bone] * crate::model::rig::viewmodel::cube_matrix(c);
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
                .map(|c| RoundPart { cube: c, m: frame * mats[c.bone] * crate::model::rig::viewmodel::cube_matrix(c), layer: layers(kind, 0) })
                .collect::<Vec<_>>()
        };
        GUN_KINDS.map(|k| [build(k, false), build(k, true)]).into()
    });
    // (the pistol's for anything else)
    let kind = GUN_KINDS.iter().position(|k| k.ammo() == ammo).unwrap_or(0);
    &all[kind][spent as usize]
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
        crate::model::rig::viewmodel::emit_cube(out, p.cube, m * p.m, p.layer, light, fl);
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
