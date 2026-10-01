//! The Blockbench revolver (`revolver_vm`) wherever it is seen: its moving parts from what the
//! gun is doing (`pistol_view::GunAnim`): the hammer and trigger, the cylinder turned to the
//! chamber under the hammer and one on with each pull, what is in each chamber (a live round,
//! a fired case, nothing), the reload (swung out, the cases thrown out, loaded one round at a
//! time or from a speedloader, swung shut), and drawing it.

use crate::model::guns::pistol_view::GunAnim;
use crate::model::blockbench::revolver_vm as vm;
use crate::model::rig::viewmodel::{add_anim, cube_matrix, emit_cube, find_anim, find_bone, Anim, BonePose};
use crate::item::{chamber, revolver_next};
use crate::world::mesh::Vertex;
use crate::textures::tex;
use glam::{Mat4, Vec3};

/// Moments of the reload animation (seconds; see `gen_revolver.py`): the cylinder is out and
/// the ejector pushed (the cases come out), the cylinder is out and held (loading one round
/// at a time goes on from here), the speedloader lets go of its rounds, the speedloader is
/// away and the cylinder is swung shut from here, the end.
pub const RELOAD_EJECT: f32 = 0.65;
pub const RELOAD_OPEN: f32 = 0.95;
pub const RELOAD_RELEASE: f32 = 1.42;
pub const RELOAD_CLOSE: f32 = 1.62;
pub const RELOAD_END: f32 = 2.3;
/// Loading one round ("load_round"): it is seated in the chamber at `LOAD_SEAT`, the cylinder
/// has turned on to the next chamber at `LOAD_END`.
pub const LOAD_SEAT: f32 = 0.45;
pub const LOAD_END: f32 = 0.6;

/// The parts' windows of the strip animation (taken apart at the gun station), in the order of
/// `model::gun::FRAME` ..: the frame stays; the barrel, the mainspring, the cylinder, the
/// hammer.
pub const STRIP: [(f32, f32); 5] = [(0.0, 0.0), (1.8, 2.5), (1.3, 1.8), (0.0, 0.8), (0.8, 1.3)];

fn anim(name: &str) -> Option<&'static Anim> {
    find_anim(vm::ANIMS, name)
}

fn bone(name: &str) -> Option<usize> {
    find_bone(vm::BONES, name)
}

/// A pose with nothing moved (the speedloader and the loose round put away).
pub fn rest_pose() -> Vec<BonePose> {
    let mut pose = vec![BonePose::default(); vm::BONES.len()];
    for name in ["speedloader", "loose_round"] {
        if let Some(b) = bone(name) {
            pose[b].scale = Vec3::ZERO;
        }
    }
    pose
}

/// A chamber of a cylinder (`GunAnim::cyl`, a revolver's data).
fn chamber_of(cyl: u16, k: usize) -> u8 {
    ((cyl >> (2 * k)) & 3) as u8
}

/// The chamber under the hammer.
fn index_of(cyl: u16) -> usize {
    (((cyl >> 12) & 7) as usize).min(5)
}

/// How far the cylinder is turned (degrees) with chamber `k` under the hammer: chamber k is at
/// 90 + 60 k degrees at rest, a turn of +60 brings the next one (`revolver_next`) up.
fn turn_for(k: usize) -> f32 {
    ((6 - k % 6) % 6) as f32 * 60.0
}

/// A whole cylinder of live rounds, the first under the hammer (for the others' revolvers,
/// whose chambers are not sent).
pub const FULL: u16 = 0b0101_0101_0101;

/// Adds the shot, the trigger pulled on an empty chamber, the reload and loading a round to
/// the pose; the cylinder turned and its chambers shown as the gun has them (`GunAnim::cyl`).
/// With `parts_only` the arms and what holds the revolver are left still (the player model
/// holds it with its own arms).
pub fn add_gun_anims(pose: &mut [BonePose], g: &GunAnim, parts_only: bool) {
    let holding = crate::model::guns::gun_view::holding(vm::BONES, parts_only);
    let hold = |b: usize| holding.contains(&b);
    // What is in each chamber: nothing, a live round, or a fired case (its primer dented).
    for k in 0..6 {
        let c = chamber_of(g.cyl, k);
        let hide = |pose: &mut [BonePose], name: String, off: bool| {
            if let (Some(b), true) = (bone(&name), off) {
                pose[b].scale = Vec3::ZERO;
            }
        };
        hide(pose, format!("chamber{k}"), c == chamber::EMPTY);
        hide(pose, format!("bullet{k}"), c != chamber::LIVE);
        hide(pose, format!("dent{k}"), c != chamber::SPENT);
    }
    // The cylinder turned to the chamber under the hammer; while a pull or a load turns it on
    // (its animation adds the sixth), from the one before.
    let turning = g.shot.is_some_and(|t| anim("shoot").is_some_and(|a| t < a.length))
        || g.dry.is_some_and(|t| anim("shoot").is_some_and(|a| t < a.length));
    let index = index_of(g.cyl);
    let from = if turning { (index + 1) % 6 } else { index };
    debug_assert_eq!(revolver_next((index + 1) % 6), index);
    if let Some(c) = bone("cylinder") {
        pose[c].rot.z += turn_for(from);
    }
    if let (Some(p), Some(an)) = (g.reload, anim("reload")) {
        let t = p.clamp(0.0, 1.0) * an.length;
        // Eased in and out a little, so a reload does not jump from and back to the hip.
        let w = (t / 0.12).min((an.length - t) / 0.12).clamp(0.0, 1.0);
        let w = w * w * (3.0 - 2.0 * w);
        // The cases are pushed out only when the cylinder is emptied.
        let rounds = bone("cylinder_rounds");
        add_anim(pose, an, t, w, |b| hold(b) || (!g.ejects && Some(b) == rounds));
        // The speedloader's rounds: those going into the chambers, until they are let go.
        for k in 0..6 {
            let shown = g.loader & (1 << k) != 0 && t < RELOAD_RELEASE;
            if let (Some(b), false) = (bone(&format!("loader_round{k}")), shown) {
                pose[b].scale = Vec3::ZERO;
            }
        }
    }
    if let (Some(lt), Some(an)) = (g.load, anim("load_round")) {
        add_anim(pose, an, lt, 1.0, hold);
        // The round in the hand goes exactly when the game puts it in its chamber (not by the
        // animation's own key, which may be a frame off): never both, never neither.
        if let Some(b) = bone("loose_round") {
            pose[b].scale = if lt < LOAD_SEAT { Vec3::ONE } else { Vec3::ZERO };
        }
    }
    if let (Some(st), Some(an)) = (g.shot, anim("shoot")) {
        if st < an.length {
            add_anim(pose, an, st, 1.0, hold);
        }
    }
    // Pulled on an empty chamber, the double action still cocks the hammer and turns it.
    if let (Some(dt), Some(an)) = (g.dry, anim("shoot")) {
        let own = ["trigger", "hammer", "cylinder"].map(bone);
        if dt < an.length {
            add_anim(pose, an, dt, 1.0, |b| !own.contains(&Some(b)));
        }
    }
}

/// The first texture layer of the revolver's pages as dirty as `dirt`
/// (`pistol_view::dirt_level`).
pub fn layers(dirt: u8) -> u32 {
    crate::model::guns::gun_view::dirty_layer(tex::REVOLVER_VIEW, vm::PAGES, dirt)
}

/// The revolver's cubes.
pub fn emit(out: &mut Vec<Vertex>, mats: &[Mat4], shown: &[bool], dirt: u8, light: [u8; 4], fl: u8) {
    let first = layers(dirt);
    for c in vm::CUBES {
        if shown[c.bone] {
            emit_cube(out, c, mats[c.bone] * cube_matrix(c), first, light, fl);
        }
    }
}

/// Where the bullet leaves: bone and model point.
pub fn muzzle() -> (usize, Vec3) {
    (vm::MUZZLE.0, Vec3::from(vm::MUZZLE.1))
}

/// The middle of the cylinder's back, where the cases come out when reloading.
pub fn eject() -> (usize, Vec3) {
    (vm::CYLINDER.0, Vec3::from(vm::CYLINDER.1))
}

/// The head of what is in chamber `k` (its rim's middle): bone and model point.
pub fn chamber_head(k: usize) -> (usize, Vec3) {
    let name = format!("round{k}_rim_h");
    match vm::CUBES.iter().find(|c| c.name == name) {
        Some(c) => (c.bone, (Vec3::from(c.from) + Vec3::from(c.to)) * 0.5),
        None => eject(),
    }
}

/// The revolver's own bone (the whole gun, without the arms).
pub fn gun_bone() -> usize {
    bone("revolver").unwrap_or(0)
}

/// The rear sight's notch (model space), which aiming brings to the middle of the view: the
/// top of its ears, in the middle of the gun.
pub fn sight_point() -> Vec3 {
    let ear = vm::CUBES.iter().find(|c| c.name == "rear_sight_l");
    let barrel = muzzle().1;
    ear.map_or(Vec3::new(barrel.x, -10.1, -32.3), |c| {
        let (a, b) = (Vec3::from(c.from), Vec3::from(c.to));
        Vec3::new(barrel.x, a.y.max(b.y), (a.z + b.z) * 0.5)
    })
}

/// The revolver at the gun station: which bones each of its parts is.
pub mod bench {
    use crate::model::guns::gun::{BARREL, FRAME, PARTS};
    use crate::model::rig::viewmodel::{add_anim, find_anim, BonePose};
    use super::{bone, vm, STRIP};

    pub type Bones = u64;

    fn subtree(name: &str) -> Bones {
        crate::model::guns::gun_view::subtree(vm::BONES, name)
    }

    /// The bones a part is (`gun::FRAME` ..: frame, barrel, mainspring, cylinder, hammer).
    pub fn part(part: usize) -> Bones {
        match part {
            FRAME => (subtree("frame") | subtree("grip")) & !subtree("hammer"),
            BARREL => subtree("barrel"),
            2 => subtree("mainspring"),
            3 => subtree("crane") & !subtree("speedloader") & !subtree("loose_round"),
            4 => subtree("hammer"),
            _ => 0,
        }
    }

    /// The whole gun.
    pub fn gun() -> Bones {
        (0..PARTS).fold(0, |b, p| b | part(p))
    }

    /// The speedloader, with its rounds.
    pub fn speedloader() -> Bones {
        subtree("speedloader")
    }

    /// A magnum round on its own (the one loaded by hand), and its pose (shown).
    pub fn round() -> Bones {
        subtree("loose_round")
    }

    pub fn round_pose() -> Vec<BonePose> {
        vec![BonePose::default(); vm::BONES.len()]
    }

    pub fn strip_length() -> f32 {
        find_anim(vm::ANIMS, "strip").map_or(2.5, |a| a.length)
    }

    /// The pose with each part as far into its window of the strip animation as `at` says
    /// (seconds from the start of the whole animation; a part is only moved within its own
    /// window), the cylinder's chambers as `cyl` has them.
    pub fn pose(at: [f32; PARTS], cyl: u16) -> Vec<BonePose> {
        let mut pose = super::rest_pose();
        let g = super::GunAnim { cyl, ..Default::default() };
        super::add_gun_anims(&mut pose, &g, true);
        if let Some(an) = find_anim(vm::ANIMS, "strip") {
            for (p, &t) in at.iter().enumerate() {
                let m = part(p);
                let (a, b) = STRIP[p];
                if m != 0 && b > a && t > a {
                    add_anim(&mut pose, an, t.min(b), 1.0, |i| m & (1 << i) == 0);
                }
            }
        }
        pose
    }

    /// The speedloader on its own, with `rounds` in it, lying as the item.
    pub fn loader_pose(rounds: u8) -> Vec<BonePose> {
        let mut pose = vec![BonePose::default(); vm::BONES.len()];
        for k in 0..6 {
            if let (Some(b), true) = (bone(&format!("loader_round{k}")), k >= rounds as usize) {
                pose[b].scale = glam::Vec3::ZERO;
            }
        }
        pose
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn the_parts_are_the_whole_gun_once_each() {
            let mut seen: Bones = 0;
            for p in 0..PARTS {
                let b = part(p);
                assert!(b != 0, "part {p}");
                assert_eq!(seen & b, 0, "part {p} shares bones");
                seen |= b;
            }
            // Every drawn cube but the speedloader's and the loose round's is in a part.
            let loose = speedloader() | (1 << bone("loose_round").unwrap()) | (1 << bone("loose_bullet").unwrap());
            for c in vm::CUBES {
                assert!((seen | loose) & (1 << c.bone) != 0, "{} is in no part", c.name);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_reload_throws_the_cases_out_before_loading() {
        assert!(0.0 < RELOAD_EJECT && RELOAD_EJECT < RELOAD_OPEN && RELOAD_OPEN < RELOAD_RELEASE);
        assert!(RELOAD_RELEASE < RELOAD_CLOSE && RELOAD_CLOSE < RELOAD_END);
        assert!((anim("reload").unwrap().length - RELOAD_END).abs() < 1e-3);
        assert!((anim("load_round").unwrap().length - LOAD_END).abs() < 1e-3);
        for name in ["cylinder", "cylinder_rounds", "speedloader", "hammer", "trigger", "crane", "revolver", "loose_round"] {
            assert!(bone(name).is_some(), "{name}");
        }
    }

    #[test]
    fn each_pull_turns_the_next_chamber_under_the_hammer() {
        // Chamber k sits at 90 + 60 k degrees; turned for chamber c, c is at the top (90).
        for c in 0..6 {
            let at = (90.0 + 60.0 * c as f32 + turn_for(c)).rem_euclid(360.0);
            assert!((at - 90.0).abs() < 1e-3, "{c}: {at}");
            // A pull turns it a further +60: the next one comes up.
            assert!((turn_for(revolver_next(c)) - (turn_for(c) + 60.0)).rem_euclid(360.0) < 1e-3);
        }
    }

    #[test]
    fn the_revolver_sits_like_the_pistol() {
        let m = crate::model::guns::gun_view::rest_point_in_gun_space(crate::item::GunKind::Revolver, muzzle());
        assert!((m - Vec3::new(8.6, 3.0, 0.0)).length() < 4.0, "{m}");
    }
}

