//! How the guns are held and how big they are, in "gun space" (about a centimetre a unit: the
//! muzzle points to +X, up is +Y, the right side is +Z, the origin above the trigger), and the
//! parts they go together from at the gun station. The model itself is the Blockbench pistol
//! (`pistol_vm`), placed in gun space by `pistol_view::to_gun_space`.

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

/// Where things are on a gun, and how it is held.
pub struct Spec {
    /// The middle of the right fist on the grip.
    pub hand: Vec3,
    /// Where the left hand holds a long gun (under the handguard, or on the pump).
    pub support: Option<Vec3>,
    /// On the player model: model pixels per gun unit.
    pub arm_scale: f32,
    /// Size of the bare gun.
    pub bounds: (Vec3, Vec3),
}

static PISTOL: Spec = Spec {
    hand: Vec3::new(-5.8, -4.4, 0.0),
    support: None,
    arm_scale: 0.45,
    bounds: (Vec3::new(-9.6, -10.6, -1.7), Vec3::new(8.6, 6.4, 1.7)),
};

pub fn spec(kind: GunKind) -> &'static Spec {
    match kind {
        GunKind::Pistol => &PISTOL,
    }
}

/// From gun space to the unit-sized item space of `emit_held` (centered, one block long).
pub fn gun_to_unit(kind: GunKind) -> Mat4 {
    let (lo, hi) = spec(kind).bounds;
    let size = (hi - lo).max_element();
    Mat4::from_scale(Vec3::splat(1.0 / size)) * Mat4::from_translation(-(lo + hi) * 0.5)
}
