//! How the guns are held and how big they are, in "gun space" (about a centimetre a unit: the
//! muzzle points to +X, up is +Y, the right side is +Z, the origin above the trigger), and the
//! parts they go together from at the gun station. The models themselves are the Blockbench
//! pistol, revolver and AK-47, placed in gun space by `gun_view::to_gun_space`.

use crate::item::GunKind;
use glam::{Mat4, Vec3};

/// The pistol's parts, in the order they go together (`GunKind::parts`); which bones of the
/// Blockbench model each is: `pistol_view::bench::part`.
pub const FRAME: usize = 0;
pub const BARREL: usize = 1;
pub const SPRING: usize = 2;
pub const SLIDE: usize = 3;
pub const MAGAZINE: usize = 4;
pub const PARTS: usize = 5;

/// Where things are on a gun, and how it is held (each gun's is in `item::WEAPONS`).
pub struct Spec {
    /// The middle of the right fist on the grip.
    pub hand: Vec3,
    /// On the player model: model pixels per gun unit, and how much thicker (across) it is
    /// drawn there than it is (seen from the side, a gun as thin as it really is against a
    /// Minecraft player looks like a stick). As its third-person rig has it (`gen_tp.py`).
    pub arm_scale: f32,
    pub thick: f32,
    /// Size of the bare gun.
    pub bounds: (Vec3, Vec3),
}

/// A gun's (its row of `item::WEAPONS`).
pub fn spec(kind: GunKind) -> &'static Spec {
    &kind.def().spec
}

/// From gun space to the unit-sized item space of `emit_held` (centered; the longest gun one
/// unit long, the others as much shorter as they are: the handguns against each other, a
/// long gun on its own).
pub fn gun_to_unit(kind: GunKind) -> Mat4 {
    let (lo, hi) = spec(kind).bounds;
    let size = crate::item::GUN_KINDS
        .iter()
        .filter(|k| k.long() == kind.long())
        .map(|&k| {
            let (a, b) = spec(k).bounds;
            (b - a).max_element()
        })
        .fold(0.0f32, f32::max);
    Mat4::from_scale(Vec3::splat(1.0 / size)) * Mat4::from_translation(-(lo + hi) * 0.5)
}
