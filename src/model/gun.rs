//! The guns' 3D models: boxes grouped into the five parts that go together at the gun station,
//! and the attachments fitted there (scope, silencer, extended magazine, laser sight). The same
//! boxes are drawn in the hand, on the player model, as a dropped item, in the gun station's
//! 3D view, and into the item icons.
//!
//! Gun space is in "gun units" (about a centimetre): the muzzle points to +X, up is +Y, the
//! right side is +Z. The origin is above the trigger.

use super::emit_box;
use crate::item::{gun_mod, GunKind, GUN_KINDS};
use crate::world::mesh::Vertex;
use crate::world::textures::{tex, TILE};
use glam::{Mat3, Mat4, Vec2, Vec3};
use std::sync::OnceLock;

/// The pistol's parts (the Desert Eagle's are the same).
pub const FRAME: usize = 0;
pub const BARREL: usize = 1;
pub const SPRING: usize = 2;
pub const SLIDE: usize = 3;
pub const MAGAZINE: usize = 4;
pub const PARTS: usize = 5;

/// Where things are on a gun, and how it is held.
pub struct Spec {
    /// The top of the grip (the recoil turns the gun about it) and the middle of the right
    /// fist on the grip.
    pub grip: Vec3,
    pub hand: Vec3,
    /// Where the left hand holds a long gun (under the handguard, or on the pump).
    pub support: Option<Vec3>,
    /// Where the bullet leaves the bare barrel, and how much a silencer adds.
    pub muzzle: Vec3,
    pub silencer: f32,
    /// Where the spent case comes out, and where the laser sight's beam starts.
    pub eject: Vec3,
    pub laser: Vec3,
    /// The iron sights' line and the back of the rear sight; the scope's axis and the back of
    /// its eyepiece.
    pub sight_height: f32,
    pub rear_sight: f32,
    pub scope_height: f32,
    pub scope_eye: f32,
    /// The middle of the magazine's bottom and the way it comes out (reloading).
    pub mag_bottom: Vec3,
    pub mag_axis: Vec3,
    /// The part that flies back on every shot and stays back when empty (a pistol's slide).
    pub slide: Option<usize>,
    /// The part worked by hand after each shot (a bolt, a pump) and how far it travels.
    pub cycle: Option<(usize, f32)>,
    /// First-person view: blocks per gun unit, where the grip is (from the hip, camera space),
    /// and how far in front of the eye the rear sight is when aimed.
    pub view_scale: f32,
    pub hip: Vec3,
    pub eye_gap: f32,
    /// On the player model: model pixels per gun unit.
    pub arm_scale: f32,
    /// Size of the bare gun.
    pub bounds: (Vec3, Vec3),
}

#[derive(Clone, Copy)]
pub struct GunBox {
    pub part: usize,
    pub min: Vec3,
    pub max: Vec3,
    /// Turned this many degrees about Z around `pivot` (grips and magazines slant).
    pub tilt: f32,
    pub pivot: Vec3,
    pub layer: u32,
    pub tint: [u8; 3],
    /// The part this box sits inside once the gun is together (a barrel in a slide, a
    /// magazine in a grip): drawn first where there is no depth buffer.
    pub inside: Option<usize>,
    /// Only there with this attachment (`gun_mod` bit; 0 = always).
    pub with: u8,
    /// Left out with this attachment (a plain baseplate under an extended magazine).
    pub without: u8,
}

impl GunBox {
    /// From the box's own corners to gun space.
    pub fn transform(&self) -> Mat4 {
        if self.tilt != 0.0 {
            Mat4::from_translation(self.pivot)
                * Mat4::from_rotation_z(self.tilt.to_radians())
                * Mat4::from_translation(-self.pivot)
        } else {
            Mat4::IDENTITY
        }
    }

    /// Part of the gun with these attachments.
    pub fn shown(&self, mods: u8) -> bool {
        (self.with == 0 || mods & self.with != 0) && mods & self.without == 0
    }
}

const BRASS: [u8; 3] = [255, 196, 110];
const WHITE: [u8; 3] = [255; 3];
const POLY: u32 = tex::GUN_POLYMER;
const BLUED: u32 = tex::GUN_BLUED;
const STEEL: u32 = tex::GUN_STEEL;
const GLASS: u32 = tex::GUN_GLASS;
const PAINT: u32 = tex::WOOL;
const WOOD: u32 = tex::GUN_WOOD;
const SCOPE: u8 = gun_mod::SCOPE;
const SILENCER: u8 = gun_mod::SILENCER;
const EXTENDED: u8 = gun_mod::EXTENDED_MAGAZINE;
const LASER: u8 = gun_mod::LASER;

/// Collects the boxes; the part, slant, attachment and what they sit inside are set as it goes.
struct Builder {
    boxes: Vec<GunBox>,
    part: usize,
    tilt: f32,
    pivot: Vec3,
    inside: Option<usize>,
    with: u8,
    without: u8,
}

impl Builder {
    fn new() -> Self {
        Self {
            boxes: Vec::new(),
            part: 0,
            tilt: 0.0,
            pivot: Vec3::ZERO,
            inside: None,
            with: 0,
            without: 0,
        }
    }

    fn part(&mut self, part: usize) {
        self.part = part;
        self.tilt = 0.0;
        self.inside = None;
    }

    fn slant(&mut self, deg: f32, pivot: [f32; 3]) {
        self.tilt = deg;
        self.pivot = Vec3::from(pivot);
    }

    fn add(&mut self, min: [f32; 3], max: [f32; 3], layer: u32, tint: [u8; 3]) {
        self.boxes.push(GunBox {
            part: self.part,
            min: Vec3::from(min),
            max: Vec3::from(max),
            tilt: self.tilt,
            pivot: self.pivot,
            layer,
            tint,
            inside: self.inside,
            with: self.with,
            without: self.without,
        });
    }

    /// A pair of boxes mirrored across the gun (z and -z).
    fn pair(&mut self, min: [f32; 3], max: [f32; 3], layer: u32, tint: [u8; 3]) {
        self.add(min, max, layer, tint);
        self.add([min[0], min[1], -max[2]], [max[0], max[1], -min[2]], layer, tint);
    }

    /// Only with this attachment from here on (0: always).
    fn only_with(&mut self, bit: u8) {
        self.with = bit;
        self.tilt = 0.0;
        self.inside = None;
    }
}

const PISTOL_GRIP: Vec3 = Vec3::new(-4.7, 0.0, 0.0);
const PISTOL_TILT: f32 = -14.0;

/// The pistol (the Desert Eagle is the same shape, bigger and with a rail on its slide).
fn pistol(k: &mut Builder, deagle: bool) {
    let g = PISTOL_GRIP.to_array();
    // Frame: the dust cover with an accessory rail under it, the grip with finger grooves,
    // the beavertail, the trigger guard and trigger, the magazine release, the slide stop and
    // the takedown lever.
    k.part(FRAME);
    k.add([-6.6, 0.0, -1.2], [7.0, 2.0, 1.2], POLY, WHITE);
    k.add([3.0, -0.3, -0.9], [6.8, 0.0, 0.9], POLY, WHITE);
    for i in 0..3 {
        let x = 3.3 + i as f32 * 1.15;
        k.add([x, -0.65, -0.9], [x + 0.55, -0.3, 0.9], POLY, WHITE);
    }
    k.slant(PISTOL_TILT, g);
    k.add([-6.9, -9.0, -1.45], [-2.5, 0.4, 1.45], POLY, WHITE);
    for y in [-2.6, -4.7, -6.8] {
        k.add([-2.5, y, -1.25], [-2.15, y + 0.9, 1.25], POLY, WHITE);
    }
    k.slant(0.0, g);
    k.add([-7.6, 0.4, -1.0], [-6.6, 1.7, 1.0], POLY, WHITE);
    k.add([-2.8, -3.5, -0.5], [2.3, -2.8, 0.5], POLY, WHITE);
    k.add([1.6, -3.5, -0.5], [2.3, 0.0, 0.5], POLY, WHITE);
    k.add([-1.0, -2.4, -0.3], [-0.3, 0.0, 0.3], STEEL, WHITE);
    k.add([-2.9, -1.5, -1.6], [-2.2, -0.7, -1.3], STEEL, [150; 3]);
    k.add([-3.9, 1.2, -1.36], [-0.9, 1.8, -1.2], STEEL, [120; 3]);
    k.add([0.4, 1.1, -1.3], [1.2, 1.7, 1.3], STEEL, [150; 3]);

    // Barrel: the tube and the chamber block at its back (inside the slide).
    k.part(BARREL);
    k.inside = Some(SLIDE);
    k.add([-3.4, 2.3, -0.75], [8.6, 3.7, 0.75], STEEL, WHITE);
    k.add([-3.6, 2.1, -1.0], [-0.6, 4.0, 1.0], STEEL, WHITE);

    // Recoil spring: the guide rod and the turns of the coil around it.
    k.part(SPRING);
    k.inside = Some(SLIDE);
    k.add([0.4, 1.1, -0.25], [7.6, 1.7, 0.25], STEEL, WHITE);
    for i in 0..8 {
        let x = 0.8 + i as f32 * 0.85;
        k.add([x, 0.75, -0.55], [x + 0.4, 2.05, 0.55], STEEL, WHITE);
    }

    // Slide: the body with a narrower top, serrations at the back and the front, the
    // ejection port (the barrel's chamber seen through it), the muzzle opening and the
    // three-dot iron sights (a notch between two posts at the back, a post in front).
    k.part(SLIDE);
    let (body, tint) = if deagle { (STEEL, [215, 215, 222]) } else { (BLUED, WHITE) };
    k.add([-7.1, 2.0, -1.3], [8.0, 4.6, 1.3], body, tint);
    k.add([-7.1, 4.6, -1.05], [8.0, 5.2, 1.05], body, tint);
    for (x0, n) in [(-6.8, 5), (5.3, 3)] {
        for i in 0..n {
            let x = x0 + i as f32 * 0.6;
            for z in [1.3, -1.34] {
                k.add([x, 2.4, z], [x + 0.28, 4.4, z + 0.04], POLY, WHITE);
            }
        }
    }
    k.add([0.2, 3.7, 1.3], [3.2, 4.6, 1.34], STEEL, [175; 3]);
    k.add([0.2, 5.2, 0.1], [3.2, 5.24, 1.05], STEEL, [175; 3]);
    k.add([8.0, 2.4, -0.55], [8.06, 3.6, 0.55], POLY, WHITE);
    if deagle {
        // A rail along the top between the sights.
        k.add([-4.8, 5.2, -0.7], [5.6, 5.55, 0.7], BLUED, WHITE);
        for i in 0..7 {
            let x = -4.4 + i as f32 * 1.4;
            k.add([x, 5.55, -0.7], [x + 0.7, 5.8, 0.7], BLUED, WHITE);
        }
    }
    k.add([-6.7, 5.2, -1.05], [-5.6, 5.6, 1.05], BLUED, WHITE);
    k.pair([-6.7, 5.6, 0.35], [-5.6, 6.3, 1.05], BLUED, WHITE);
    k.pair([-6.74, 5.85, 0.6], [-6.7, 6.1, 0.85], PAINT, WHITE);
    k.add([6.9, 5.2, -0.28], [7.5, 6.3, 0.28], BLUED, WHITE);
    k.add([6.86, 5.85, -0.13], [6.9, 6.1, 0.13], PAINT, WHITE);

    // Magazine: inside the grip, with a cartridge on top and its baseplate under it.
    k.part(MAGAZINE);
    k.slant(PISTOL_TILT, g);
    k.inside = Some(FRAME);
    k.add([-6.3, -9.2, -1.0], [-3.2, -0.6, 1.0], BLUED, WHITE);
    k.add([-5.9, -0.6, -0.45], [-3.6, 0.1, 0.45], STEEL, BRASS);
    k.inside = None;
    k.without = EXTENDED;
    k.add([-7.1, -10.1, -1.55], [-2.4, -9.2, 1.55], POLY, WHITE);
    k.without = 0;

    // Extended magazine: it reaches further out of the grip, with a red baseplate.
    k.only_with(EXTENDED);
    k.part = MAGAZINE;
    k.slant(PISTOL_TILT, g);
    k.add([-6.5, -13.4, -1.05], [-3.0, -9.2, 1.05], BLUED, WHITE);
    k.add([-7.2, -14.3, -1.55], [-2.3, -13.4, 1.55], POLY, [220, 80, 70]);

    // Scope on two rings on the slide: the tube, its bells, a turret and the lenses.
    k.only_with(SCOPE);
    k.part = SLIDE;
    k.add([-4.8, 5.2, -0.9], [-3.8, 7.0, 0.9], BLUED, WHITE);
    k.add([1.8, 5.2, -0.9], [2.8, 7.0, 0.9], BLUED, WHITE);
    k.add([-6.0, 6.9, -1.1], [4.5, 9.1, 1.1], BLUED, WHITE);
    k.add([4.5, 6.5, -1.5], [7.5, 9.5, 1.5], BLUED, WHITE);
    k.add([-8.0, 6.6, -1.4], [-6.0, 9.4, 1.4], BLUED, WHITE);
    k.add([-1.0, 9.1, -0.5], [0.2, 9.7, 0.5], BLUED, WHITE);
    k.add([7.5, 6.8, -1.2], [7.55, 9.2, 1.2], GLASS, WHITE);
    k.add([-8.05, 6.9, -1.1], [-8.0, 9.1, 1.1], GLASS, WHITE);

    // Silencer on the barrel's end.
    k.only_with(SILENCER);
    k.part = BARREL;
    k.add([8.6, 1.9, -1.1], [15.0, 4.1, 1.1], POLY, WHITE);
    k.add([15.0, 2.2, -0.8], [15.4, 3.8, 0.8], STEEL, [120; 3]);
    k.add([10.2, 1.88, -1.12], [10.5, 4.12, 1.12], STEEL, [110; 3]);

    // Laser sight on the accessory rail, with its red lens.
    k.only_with(LASER);
    k.part = FRAME;
    k.add([2.9, -2.2, -0.85], [6.8, -0.65, 0.85], POLY, [160; 3]);
    k.add([6.8, -1.8, -0.45], [6.86, -0.9, 0.45], PAINT, [255, 60, 50]);
    k.only_with(0);
}

/// Where the attachments go on a long gun: a scope on rings over `rail` (x of the back and
/// front ring, height of the rail; None with a built-in scope), a silencer (length, radius) on
/// `muzzle`, a laser box ending at `laser`, and an extended magazine continuing the
/// magazine's bottom (`mag`: bottom, axis, half width, half depth).
struct Mounts {
    rail: Option<(f32, f32, f32)>,
    scope_part: usize,
    muzzle: Vec3,
    silencer: (f32, f32),
    laser: Vec3,
    mag: Option<(Vec3, Vec3, f32, f32)>,
}

/// Adds the attachments; returns the scope's axis height and the back of its eyepiece.
fn attachments(k: &mut Builder, m: &Mounts) -> (f32, f32) {
    let scope = m.rail.map(|rail| scope(k, m.scope_part, rail));
    k.only_with(0);
    other_attachments(k, m);
    scope.unwrap_or((0.0, 0.0))
}

/// A scope on rings over a rail: the tube, the bells, turrets and the lenses.
fn scope(k: &mut Builder, part: usize, (x0, x1, y): (f32, f32, f32)) -> (f32, f32) {
    k.only_with(SCOPE);
    k.part = part;
    let ring_top = y + 2.2;
    let (r, axis) = (1.4, ring_top + 1.2);
    k.add([x0 - 0.6, y, -1.1], [x0 + 0.6, ring_top, 1.1], BLUED, WHITE);
    k.add([x1 - 0.6, y, -1.1], [x1 + 0.6, ring_top, 1.1], BLUED, WHITE);
    let (back, front) = (x0 - 3.0, x1 + 3.0);
    k.add([back, axis - r, -r], [front, axis + r, r], BLUED, WHITE);
    k.add([front, axis - r - 0.6, -r - 0.6], [front + 4.0, axis + r + 0.6, r + 0.6], BLUED, WHITE);
    k.add([back - 3.0, axis - r - 0.4, -r - 0.4], [back, axis + r + 0.4, r + 0.4], BLUED, WHITE);
    let mid = (x0 + x1) * 0.5;
    k.add([mid - 0.8, axis + r, -0.6], [mid + 0.8, axis + r + 0.9, 0.6], BLUED, WHITE);
    k.add([mid - 0.8, axis - 0.6, r], [mid + 0.8, axis + 0.6, r + 0.9], BLUED, WHITE);
    let (rl, rb) = (r + 0.3, r + 0.2);
    k.add([front + 4.0, axis - rl, -rl], [front + 4.05, axis + rl, rl], GLASS, WHITE);
    let eye = back - 3.05;
    k.add([eye, axis - rb, -rb], [back - 3.0, axis + rb, rb], GLASS, WHITE);
    (axis, eye)
}

/// The silencer, the laser sight and the extended magazine of a long gun.
fn other_attachments(k: &mut Builder, m: &Mounts) {
    // Silencer.
    k.only_with(SILENCER);
    k.part = BARREL;
    let (len, rad) = m.silencer;
    let c = m.muzzle;
    k.add([c.x, c.y - rad, -rad], [c.x + len - 0.4, c.y + rad, rad], POLY, WHITE);
    k.add([c.x + len - 0.4, c.y - rad * 0.7, -rad * 0.7], [c.x + len, c.y + rad * 0.7, rad * 0.7], STEEL, [120; 3]);
    k.add([c.x + 1.6, c.y - rad - 0.02, -rad - 0.02], [c.x + 2.0, c.y + rad + 0.02, rad + 0.02], STEEL, [110; 3]);

    // Laser sight.
    k.only_with(LASER);
    k.part = BARREL;
    let l = m.laser;
    k.add([l.x - 4.0, l.y - 0.85, -0.85], [l.x - 0.1, l.y + 0.85, 0.85], POLY, [160; 3]);
    k.add([l.x - 0.1, l.y - 0.45, -0.45], [l.x, l.y + 0.45, 0.45], PAINT, [255, 60, 50]);

    // Extended magazine: a longer magazine with a red baseplate.
    if let Some((bottom, _, hw, hd)) = m.mag {
        k.only_with(EXTENDED);
        k.part = MAGAZINE_OF_LONG;
        k.add([bottom.x - hw, bottom.y - 5.0, -hd], [bottom.x + hw, bottom.y + 0.05, hd], BLUED, WHITE);
        k.add([bottom.x - hw - 0.3, bottom.y - 5.8, -hd - 0.3], [bottom.x + hw + 0.3, bottom.y - 5.0, hd + 0.3], POLY, [220, 80, 70]);
    }
    k.only_with(0);
}

/// The long guns keep their magazine as part 4 too.
const MAGAZINE_OF_LONG: usize = 4;

/// M16: the lower receiver with the stock and pistol grip, the barrel with the handguard, the
/// bolt carrier (inside the upper receiver), the upper receiver with its carry handle and
/// sights, and a curved magazine.
fn m16(k: &mut Builder) -> (f32, f32) {
    k.part(0);
    k.add([-14.0, -4.0, -1.6], [4.0, 0.0, 1.6], POLY, WHITE);
    k.add([-2.0, -6.0, -1.8], [3.4, -3.5, 1.8], POLY, WHITE);
    k.add([-6.0, -6.6, -0.4], [-1.5, -6.0, 0.4], POLY, WHITE);
    k.add([-2.0, -6.6, -0.4], [-1.5, -4.0, 0.4], POLY, WHITE);
    k.add([-4.4, -5.8, -0.3], [-3.9, -4.0, 0.3], STEEL, WHITE);
    k.add([-26.0, -1.6, -1.1], [-14.0, 1.0, 1.1], POLY, [150; 3]);
    k.add([-34.0, -6.0, -1.6], [-22.0, 2.4, 1.6], POLY, WHITE);
    k.add([-35.2, -6.6, -1.75], [-34.0, 3.0, 1.75], POLY, [120; 3]);
    k.slant(-18.0, [-7.5, -4.0, 0.0]);
    k.add([-9.5, -12.0, -1.4], [-5.5, -4.0, 1.4], POLY, WHITE);

    k.part(1);
    k.add([4.0, -1.8, -1.8], [22.0, 4.2, 1.8], POLY, WHITE);
    for i in 0..5 {
        let x = 6.0 + i as f32 * 3.2;
        k.pair([x, -0.4, 1.8], [x + 1.4, 2.8, 1.85], POLY, [110; 3]);
    }
    k.add([3.4, -2.2, -2.1], [4.4, 4.6, 2.1], BLUED, WHITE);
    k.add([22.0, 0.9, -0.6], [34.0, 2.5, 0.6], BLUED, WHITE);
    k.add([24.0, 2.4, -0.8], [26.0, 4.4, 0.8], BLUED, WHITE);
    k.add([24.8, 4.4, -0.25], [25.3, 7.9, 0.25], BLUED, WHITE);
    k.pair([24.6, 5.0, 0.8], [25.5, 7.4, 1.2], BLUED, WHITE);
    k.add([34.0, 0.7, -0.8], [37.0, 2.7, 0.8], BLUED, WHITE);

    k.part(2);
    k.inside = Some(3);
    k.add([-10.0, 1.0, -0.9], [2.0, 3.0, 0.9], STEEL, WHITE);
    k.add([2.0, 1.3, -0.6], [4.0, 2.7, 0.6], STEEL, WHITE);

    k.part(3);
    k.add([-14.0, 0.0, -1.6], [4.0, 4.5, 1.6], POLY, WHITE);
    k.add([-10.0, 4.5, -0.9], [2.0, 5.2, 0.9], POLY, WHITE);
    k.add([-10.0, 4.5, -0.8], [-8.0, 7.0, 0.8], POLY, WHITE);
    k.add([0.5, 4.5, -0.8], [2.0, 7.0, 0.8], POLY, WHITE);
    k.add([-10.0, 6.3, -0.8], [2.0, 7.1, 0.8], POLY, WHITE);
    k.pair([-9.6, 7.1, 0.2], [-8.4, 7.9, 0.8], BLUED, WHITE);
    k.add([-4.0, 1.5, 1.6], [1.0, 3.2, 1.64], STEEL, [170; 3]);
    k.add([-6.0, 2.0, 1.6], [-4.5, 3.2, 2.2], POLY, WHITE);
    k.add([-15.2, 3.0, -0.6], [-14.0, 4.0, 0.6], STEEL, [150; 3]);

    k.part(4);
    k.inside = Some(0);
    k.add([-1.0, -3.5, -0.4], [2.4, -3.0, 0.4], STEEL, BRASS);
    k.inside = None;
    k.add([-1.6, -9.0, -1.2], [3.0, -3.5, 1.2], BLUED, WHITE);
    k.slant(12.0, [0.7, -9.0, 0.0]);
    k.add([-1.6, -14.0, -1.2], [3.0, -9.0, 1.2], BLUED, WHITE);
    k.without = EXTENDED;
    k.add([-1.8, -14.6, -1.35], [3.2, -14.0, 1.35], POLY, WHITE);
    k.without = 0;
    k.only_with(EXTENDED);
    k.part = 4;
    k.slant(12.0, [0.7, -9.0, 0.0]);
    k.add([-1.6, -19.0, -1.2], [3.0, -14.0, 1.2], BLUED, WHITE);
    k.add([-1.8, -19.6, -1.35], [3.2, -19.0, 1.35], POLY, [220, 80, 70]);
    k.only_with(0);

    attachments(
        k,
        &Mounts {
            rail: Some((-7.0, 0.5, 7.1)),
            scope_part: 3,
            muzzle: Vec3::new(37.0, 1.7, 0.0),
            silencer: (9.0, 1.3),
            laser: Vec3::new(21.0, -2.7, 0.0),
            mag: None,
        },
    )
}

/// Sniper rifle (a .50 like the Barrett): the stock with the lower receiver and grip, the
/// long barrel with its muzzle brake, handguard and folded bipod, the bolt, the upper receiver
/// with its built-in scope, and a box magazine.
fn sniper(k: &mut Builder) -> (f32, f32) {
    k.part(0);
    k.add([-20.0, -4.0, -1.8], [8.0, 0.0, 1.8], BLUED, WHITE);
    k.add([-44.0, -5.0, -1.6], [-20.0, 1.0, 1.6], BLUED, WHITE);
    k.add([-38.0, 1.0, -1.4], [-26.0, 3.2, 1.4], POLY, WHITE);
    k.add([-45.5, -7.0, -1.8], [-44.0, 3.0, 1.8], POLY, [120; 3]);
    k.add([-36.0, -7.0, -1.0], [-33.0, -5.0, 1.0], POLY, WHITE);
    k.add([-12.0, -6.4, -0.4], [-6.0, -5.9, 0.4], BLUED, WHITE);
    k.add([-6.5, -6.4, -0.4], [-6.0, -4.0, 0.4], BLUED, WHITE);
    k.add([-9.6, -5.6, -0.3], [-9.1, -4.0, 0.3], STEEL, WHITE);
    k.add([-6.0, -5.0, -2.0], [1.0, -3.5, 2.0], BLUED, WHITE);
    k.slant(-15.0, [-13.0, -4.0, 0.0]);
    k.add([-15.0, -12.0, -1.5], [-11.0, -4.0, 1.5], POLY, WHITE);

    k.part(1);
    k.add([8.0, 0.8, -0.9], [40.0, 3.2, 0.9], BLUED, WHITE);
    k.pair([26.0, 1.7, 0.9], [38.0, 2.3, 0.93], STEEL, [150; 3]);
    k.add([40.0, 0.2, -1.5], [46.0, 3.8, 1.5], BLUED, WHITE);
    for x in [41.4, 43.6] {
        k.pair([x, 0.8, 1.5], [x + 1.0, 3.2, 1.55], POLY, WHITE);
    }
    k.add([8.0, -1.0, -2.0], [24.0, 4.5, 2.0], BLUED, WHITE);
    for i in 0..5 {
        let x = 10.0 + i as f32 * 2.8;
        k.pair([x, 1.0, 2.0], [x + 1.4, 3.5, 2.05], POLY, WHITE);
    }
    k.add([24.0, -2.2, -0.5], [36.0, -1.2, 0.5], STEEL, [170; 3]);

    k.part(2);
    k.inside = Some(3);
    k.add([-16.0, 1.2, -1.0], [0.0, 3.2, 1.0], STEEL, WHITE);
    k.inside = None;
    k.add([-6.0, 1.6, 1.8], [-4.5, 2.4, 3.6], STEEL, WHITE);
    k.add([-6.2, 1.2, 3.6], [-4.3, 2.8, 4.4], STEEL, WHITE);

    k.part(3);
    k.add([-20.0, 0.0, -1.8], [8.0, 5.0, 1.8], BLUED, WHITE);
    k.add([-18.0, 5.0, -1.0], [6.0, 5.6, 1.0], BLUED, WHITE);
    k.add([-8.0, 1.5, 1.8], [-2.0, 3.5, 1.84], STEEL, [170; 3]);
    // The built-in scope.
    k.add([-12.0, 5.6, -1.0], [-10.5, 8.0, 1.0], BLUED, WHITE);
    k.add([0.0, 5.6, -1.0], [1.5, 8.0, 1.0], BLUED, WHITE);
    k.add([-16.0, 7.6, -1.4], [5.0, 10.4, 1.4], BLUED, WHITE);
    k.add([5.0, 7.0, -2.0], [10.0, 11.0, 2.0], BLUED, WHITE);
    k.add([-19.0, 7.2, -1.8], [-16.0, 10.8, 1.8], BLUED, WHITE);
    k.add([-6.0, 10.4, -0.6], [-4.4, 11.4, 0.6], BLUED, WHITE);
    k.add([-6.0, 8.4, 1.4], [-4.4, 9.6, 2.4], BLUED, WHITE);
    k.add([10.0, 7.3, -1.7], [10.05, 10.7, 1.7], GLASS, WHITE);
    k.add([-19.05, 7.5, -1.5], [-19.0, 10.5, 1.5], GLASS, WHITE);

    k.part(4);
    k.inside = Some(0);
    k.add([-5.0, -3.5, -0.6], [0.0, -2.8, 0.6], STEEL, BRASS);
    k.inside = None;
    k.add([-5.6, -10.0, -1.5], [0.6, -3.5, 1.5], BLUED, WHITE);
    k.without = EXTENDED;
    k.add([-6.0, -10.6, -1.6], [1.0, -10.0, 1.6], POLY, WHITE);
    k.without = 0;

    attachments(
        k,
        &Mounts {
            rail: None,
            scope_part: 3,
            muzzle: Vec3::new(46.0, 2.0, 0.0),
            silencer: (10.0, 2.0),
            laser: Vec3::new(14.0, -2.1, 0.0),
            mag: Some((Vec3::new(-2.5, -10.0, 0.0), Vec3::NEG_Y, 3.1, 1.5)),
        },
    );
    (9.0, -19.05)
}

/// Pump shotgun: the receiver with the trigger group, the barrel, the magazine tube under it,
/// the wooden pump and the wooden stock.
fn shotgun(k: &mut Builder) -> (f32, f32) {
    k.part(0);
    k.add([-14.0, -3.0, -1.6], [0.0, 3.0, 1.6], BLUED, WHITE);
    k.add([-10.0, -5.0, -0.4], [-4.0, -4.5, 0.4], BLUED, WHITE);
    k.add([-4.5, -5.0, -0.4], [-4.0, -3.0, 0.4], BLUED, WHITE);
    k.add([-7.8, -4.5, -0.3], [-7.3, -3.0, 0.3], STEEL, WHITE);
    k.add([-9.0, -3.05, -1.0], [-2.0, -3.0, 1.0], POLY, WHITE);
    k.add([-9.0, 0.0, 1.6], [-3.0, 2.0, 1.64], STEEL, [160; 3]);
    k.pair([-5.0, 3.0, 0.2], [-4.0, 3.6, 0.6], BLUED, WHITE);

    k.part(1);
    k.add([0.0, 1.2, -0.9], [32.0, 3.0, 0.9], BLUED, WHITE);
    k.add([31.0, 3.0, -0.3], [31.6, 3.6, 0.3], STEEL, BRASS);
    k.add([25.5, -1.3, -0.9], [27.0, 3.1, 0.9], BLUED, WHITE);

    k.part(2);
    k.add([0.0, -1.2, -0.8], [24.0, 0.8, 0.8], BLUED, WHITE);
    k.without = EXTENDED;
    k.add([24.0, -1.3, -0.9], [25.0, 0.9, 0.9], STEEL, WHITE);
    k.without = 0;
    k.only_with(EXTENDED);
    k.part = 2;
    k.add([24.0, -1.2, -0.8], [30.0, 0.8, 0.8], BLUED, WHITE);
    k.add([30.0, -1.3, -0.9], [31.0, 0.9, 0.9], STEEL, [220, 80, 70]);
    k.only_with(0);

    k.part(3);
    k.add([5.0, -2.2, -1.5], [17.0, 1.2, 1.5], WOOD, WHITE);
    for i in 0..5 {
        let x = 6.5 + i as f32 * 2.0;
        k.pair([x, -2.0, 1.5], [x + 0.7, 1.0, 1.55], WOOD, [150; 3]);
    }

    k.part(4);
    k.add([-36.0, -4.0, -1.5], [-14.0, 2.5, 1.5], WOOD, WHITE);
    k.add([-36.0, -8.0, -1.5], [-24.0, -4.0, 1.5], WOOD, WHITE);
    k.slant(-20.0, [-16.0, -3.0, 0.0]);
    k.add([-24.0, -6.0, -1.4], [-16.0, -3.0, 1.4], WOOD, WHITE);
    k.part(4);
    k.add([-37.5, -8.5, -1.7], [-36.0, 3.0, 1.7], POLY, [120; 3]);

    attachments(
        k,
        &Mounts {
            rail: Some((-11.0, -3.0, 3.0)),
            scope_part: 0,
            muzzle: Vec3::new(32.0, 2.1, 0.0),
            silencer: (8.0, 1.4),
            laser: Vec3::new(29.1, -2.2, 0.0),
            mag: None,
        },
    )
}

fn scaled(mut b: Vec<GunBox>, s: f32) -> Vec<GunBox> {
    for x in &mut b {
        x.min *= s;
        x.max *= s;
        x.pivot *= s;
    }
    b
}

/// The Desert Eagle is the pistol this much bigger.
const DEAGLE: f32 = 1.3;

struct Model {
    boxes: Vec<GunBox>,
    spec: Spec,
}

fn models() -> &'static [Model; 5] {
    static M: OnceLock<[Model; 5]> = OnceLock::new();
    M.get_or_init(|| {
        let pistol_tilt = |p: Vec3| {
            (Mat4::from_translation(PISTOL_GRIP)
                * Mat4::from_rotation_z(PISTOL_TILT.to_radians())
                * Mat4::from_translation(-PISTOL_GRIP))
            .transform_point3(p)
        };
        let mag_axis = Mat4::from_rotation_z(PISTOL_TILT.to_radians()).transform_vector3(Vec3::NEG_Y);
        let pistol_spec = Spec {
            grip: PISTOL_GRIP,
            hand: Vec3::new(-5.8, -4.4, 0.0),
            support: None,
            muzzle: Vec3::new(8.6, 3.0, 0.0),
            silencer: 6.8,
            eject: Vec3::new(1.7, 4.8, 1.4),
            laser: Vec3::new(6.9, -1.35, 0.0),
            sight_height: 6.3,
            rear_sight: -6.7,
            scope_height: 8.0,
            scope_eye: -8.05,
            mag_bottom: pistol_tilt(Vec3::new(-4.75, -10.1, 0.0)),
            mag_axis,
            slide: Some(SLIDE),
            cycle: None,
            view_scale: 0.022,
            hip: Vec3::new(0.3, -0.29, -0.56),
            eye_gap: 0.2,
            arm_scale: 0.45,
            bounds: (Vec3::new(-9.6, -10.6, -1.7), Vec3::new(8.6, 6.4, 1.7)),
        };

        let mut k = Builder::new();
        pistol(&mut k, false);
        let pistol_model = Model {
            boxes: k.boxes,
            spec: pistol_spec,
        };

        let mut k = Builder::new();
        pistol(&mut k, true);
        let p = &pistol_model.spec;
        let s = DEAGLE;
        let deagle = Model {
            boxes: scaled(k.boxes, s),
            spec: Spec {
                grip: p.grip * s,
                hand: p.hand * s,
                muzzle: p.muzzle * s,
                silencer: p.silencer * s,
                eject: p.eject * s,
                laser: p.laser * s,
                sight_height: p.sight_height * s,
                rear_sight: p.rear_sight * s,
                scope_height: p.scope_height * s,
                scope_eye: p.scope_eye * s,
                mag_bottom: p.mag_bottom * s,
                view_scale: 0.019,
                hip: Vec3::new(0.3, -0.31, -0.6),
                arm_scale: 0.4,
                bounds: (p.bounds.0 * s, p.bounds.1 * s),
                ..*p
            },
        };

        let mut k = Builder::new();
        let (scope_height, scope_eye) = m16(&mut k);
        let m16_model = Model {
            boxes: k.boxes,
            spec: Spec {
                grip: Vec3::new(-7.5, -4.0, 0.0),
                hand: Vec3::new(-8.6, -7.8, 0.0),
                support: Some(Vec3::new(14.0, -1.8, 0.0)),
                muzzle: Vec3::new(37.0, 1.7, 0.0),
                silencer: 9.0,
                eject: Vec3::new(-1.5, 2.6, 1.8),
                laser: Vec3::new(21.0, -2.7, 0.0),
                sight_height: 7.85,
                rear_sight: -9.6,
                scope_height,
                scope_eye,
                mag_bottom: Vec3::new(1.9, -14.4, 0.0),
                mag_axis: Vec3::new(0.1, -1.0, 0.0).normalize(),
                slide: None,
                cycle: None,
                view_scale: 0.0115,
                hip: Vec3::new(0.24, -0.22, -0.42),
                eye_gap: 0.12,
                arm_scale: 0.3,
                bounds: (Vec3::new(-35.2, -15.0, -2.2), Vec3::new(37.0, 7.9, 2.2)),
            },
        };

        let mut k = Builder::new();
        let (scope_height, scope_eye) = sniper(&mut k);
        let sniper_model = Model {
            boxes: k.boxes,
            spec: Spec {
                grip: Vec3::new(-13.0, -4.0, 0.0),
                hand: Vec3::new(-14.0, -7.8, 0.0),
                support: Some(Vec3::new(16.0, -1.0, 0.0)),
                muzzle: Vec3::new(46.0, 2.0, 0.0),
                silencer: 10.0,
                eject: Vec3::new(-5.0, 2.5, 2.0),
                laser: Vec3::new(14.0, -2.1, 0.0),
                sight_height: scope_height,
                rear_sight: scope_eye,
                scope_height,
                scope_eye,
                mag_bottom: Vec3::new(-2.5, -10.6, 0.0),
                mag_axis: Vec3::NEG_Y,
                slide: None,
                cycle: Some((2, 5.0)),
                view_scale: 0.0102,
                hip: Vec3::new(0.24, -0.21, -0.36),
                eye_gap: 0.12,
                arm_scale: 0.26,
                bounds: (Vec3::new(-45.5, -12.2, -2.2), Vec3::new(46.0, 11.4, 4.4)),
            },
        };

        let mut k = Builder::new();
        let (scope_height, scope_eye) = shotgun(&mut k);
        let shotgun_model = Model {
            boxes: k.boxes,
            spec: Spec {
                grip: Vec3::new(-16.0, -2.0, 0.0),
                hand: Vec3::new(-19.0, -3.5, 0.0),
                support: Some(Vec3::new(11.0, -2.2, 0.0)),
                muzzle: Vec3::new(32.0, 2.1, 0.0),
                silencer: 8.0,
                eject: Vec3::new(-6.0, 1.0, 1.8),
                laser: Vec3::new(29.1, -2.2, 0.0),
                sight_height: 3.55,
                rear_sight: -5.0,
                scope_height,
                scope_eye,
                // Shells go in through the loading port under the receiver.
                mag_bottom: Vec3::new(-5.5, -3.0, 0.0),
                mag_axis: Vec3::NEG_Y,
                slide: None,
                cycle: Some((3, 5.0)),
                view_scale: 0.0115,
                hip: Vec3::new(0.24, -0.21, -0.4),
                eye_gap: 0.12,
                arm_scale: 0.28,
                bounds: (Vec3::new(-37.5, -9.2, -1.8), Vec3::new(32.0, 3.6, 1.8)),
            },
        };
        [pistol_model, deagle, m16_model, sniper_model, shotgun_model]
    })
}

fn model(kind: GunKind) -> &'static Model {
    &models()[kind as usize]
}

/// Every box of a gun and its attachments.
pub fn boxes(kind: GunKind) -> &'static [GunBox] {
    &model(kind).boxes
}

pub fn spec(kind: GunKind) -> &'static Spec {
    &model(kind).spec
}

/// Where the bullet leaves the gun (the end of the silencer when there is one).
pub fn muzzle(kind: GunKind, mods: u8) -> Vec3 {
    let s = spec(kind);
    if mods & SILENCER != 0 {
        s.muzzle + Vec3::X * s.silencer
    } else {
        s.muzzle
    }
}

/// Where each part's boxes are together (gun space), with these attachments.
fn part_bounds(kind: GunKind, part: usize, mods: u8) -> (Vec3, Vec3) {
    let mut lo = Vec3::splat(f32::MAX);
    let mut hi = Vec3::splat(f32::MIN);
    for b in boxes(kind).iter().filter(|b| b.part == part && b.shown(mods)) {
        let m = b.transform();
        for i in 0..8 {
            let c = Vec3::new(
                if i & 1 == 0 { b.min.x } else { b.max.x },
                if i & 2 == 0 { b.min.y } else { b.max.y },
                if i & 4 == 0 { b.min.z } else { b.max.z },
            );
            let p = m.transform_point3(c);
            lo = lo.min(p);
            hi = hi.max(p);
        }
    }
    (lo, hi)
}

/// Where each part comes from when it is put in at the gun station (added to its place).
/// The pistols' parts come the way they really go on (the barrel drops in, the spring and the
/// slide slide on from the front, the magazine goes up into the grip); the others come from
/// the side of the gun they are on, a magazine along its well.
pub fn assembly_offset(kind: GunKind, part: usize) -> Vec3 {
    let s = spec(kind);
    match kind {
        GunKind::Pistol | GunKind::DesertEagle => {
            let k = if kind == GunKind::DesertEagle { DEAGLE } else { 1.0 };
            k * match part {
                FRAME => Vec3::new(0.0, -5.0, 0.0),
                BARREL => Vec3::new(0.0, 7.0, 0.0),
                SPRING => Vec3::new(9.0, 0.0, 0.0),
                SLIDE => Vec3::new(12.0, 0.0, 0.0),
                _ => s.mag_axis * 11.0,
            }
        }
        _ if part == 4 && kind != GunKind::Shotgun => s.mag_axis * 14.0,
        _ => {
            let (lo, hi) = part_bounds(kind, part, 0);
            let (glo, ghi) = s.bounds;
            let d = (lo + hi) * 0.5 - (glo + ghi) * 0.5;
            let d = Vec3::new(d.x, d.y, 0.0);
            let dir = if d.length() < 2.0 { Vec3::Y } else { d.normalize() };
            dir * (8.0 + (hi - lo).y)
        }
    }
}

/// Where each part lies when the gun is taken apart for cleaning: the parts one above the
/// other (in the order they are on the gun from the top), the magazine beside them.
pub fn exploded_offset(kind: GunKind, part: usize) -> Vec3 {
    static OFFSETS: OnceLock<[[Vec3; PARTS]; 5]> = OnceLock::new();
    let all = OFFSETS.get_or_init(|| {
        std::array::from_fn(|ki| {
            let kind = GUN_KINDS[ki];
            let mods = kind.shown_mods(0);
            let bounds: [(Vec3, Vec3); PARTS] = std::array::from_fn(|p| part_bounds(kind, p, mods));
            let mut order: Vec<usize> = (0..PARTS).filter(|&p| p != MAGAZINE || kind == GunKind::Shotgun).collect();
            order.sort_by(|&a, &b| {
                let ca = bounds[a].0.y + bounds[a].1.y;
                let cb = bounds[b].0.y + bounds[b].1.y;
                cb.total_cmp(&ca)
            });
            let mut out = [Vec3::ZERO; PARTS];
            let gap = 2.0 * spec(kind).view_scale.recip() * 0.022;
            let mut top = spec(kind).bounds.1.y + gap * 2.0;
            for p in order {
                let (lo, hi) = bounds[p];
                out[p] = Vec3::new(0.0, top - hi.y, 0.0);
                top -= hi.y - lo.y + gap;
            }
            if kind != GunKind::Shotgun {
                // The magazine under the rest, toward the front.
                let (lo, hi) = bounds[MAGAZINE];
                let (glo, ghi) = spec(kind).bounds;
                let x = ghi.x - (hi.x - lo.x) * 0.5 - (ghi.x - glo.x) * 0.15;
                out[MAGAZINE] = Vec3::new(x - (lo.x + hi.x) * 0.5, top - hi.y, 0.0);
            }
            out
        })
    });
    all[kind as usize][part]
}

/// The whole gun with these attachments, in gun space, transformed by `m`.
pub fn emit_gun(out: &mut Vec<Vertex>, kind: GunKind, m: Mat4, light: [u8; 4], fl: u8, mods: u8) {
    emit_gun_parts(out, kind, m, light, fl, mods, |_| Some(Mat4::IDENTITY));
}

/// The gun with each part placed by `place(part)` in gun space (None leaves it out): a slide
/// kicking back, the magazines coming and going while reloading.
pub fn emit_gun_parts(
    out: &mut Vec<Vertex>,
    kind: GunKind,
    m: Mat4,
    light: [u8; 4],
    fl: u8,
    mods: u8,
    place: impl Fn(usize) -> Option<Mat4>,
) {
    let mods = kind.shown_mods(mods);
    for b in boxes(kind).iter().filter(|b| b.shown(mods)) {
        let Some(at) = place(b.part) else {
            continue;
        };
        emit_box(
            out,
            m * at * b.transform(),
            b.min,
            b.max,
            [b.layer; 6],
            [b.tint; 6],
            light,
            fl,
        );
    }
}

/// From gun space to the unit-sized item space of `emit_held` (centered, one block long).
pub fn gun_to_unit(kind: GunKind) -> Mat4 {
    let (lo, hi) = spec(kind).bounds;
    let size = (hi - lo).max_element();
    Mat4::from_scale(Vec3::splat(1.0 / size)) * Mat4::from_translation(-(lo + hi) * 0.5)
}

/// The gun in the right hand of the player model (arm space, model pixels), aimed along the
/// arm: the muzzle points away from the shoulder and the sights toward the back of the hand.
pub fn in_arm(kind: GunKind) -> Mat4 {
    let s = spec(kind);
    let basis = Mat4::from_cols(
        glam::Vec4::new(0.0, -1.0, 0.0, 0.0),
        glam::Vec4::new(0.0, 0.0, -1.0, 0.0),
        glam::Vec4::new(1.0, 0.0, 0.0, 0.0),
        glam::Vec4::W,
    );
    Mat4::from_translation(Vec3::new(1.0, -11.0, 0.0))
        * basis
        * Mat4::from_scale(Vec3::splat(s.arm_scale))
        * Mat4::from_translation(-s.hand)
}

/// Average colours of the model's surfaces, for the icons.
fn surface_color(layer: u32) -> [f32; 3] {
    match layer {
        tex::GUN_BLUED => [64.0, 70.0, 84.0],
        tex::GUN_STEEL => [176.0, 180.0, 188.0],
        tex::GUN_POLYMER => [52.0, 52.0, 56.0],
        tex::GUN_GLASS => [80.0, 140.0, 200.0],
        tex::GUN_WOOD => [150.0, 98.0, 54.0],
        _ => [236.0, 236.0, 236.0],
    }
}

/// The gun's item icon (TILE x TILE, RGBA): its model seen from the right, a little from
/// above, filling the icon (a long gun across it, muzzle up to the right), with a dark
/// outline.
pub fn icon(kind: GunKind) -> &'static [[u8; 4]] {
    static ICONS: OnceLock<[Vec<[u8; 4]>; 5]> = OnceLock::new();
    &ICONS.get_or_init(|| std::array::from_fn(|i| render_icon(GUN_KINDS[i])))[kind as usize]
}

fn render_icon(kind: GunKind) -> Vec<[u8; 4]> {
    let n = TILE;
    let slant = if spec(kind).support.is_some() { 0.62 } else { 0.0 };
    let rot = Mat3::from_rotation_z(slant) * Mat3::from_rotation_x(0.3);
    let mods = kind.shown_mods(0);
    struct Face {
        quad: [Vec2; 4],
        depth: f32,
        color: [f32; 3],
    }
    let mut faces = Vec::new();
    let (mut lo, mut hi) = (Vec2::splat(f32::MAX), Vec2::splat(f32::MIN));
    for b in boxes(kind).iter().filter(|b| b.shown(mods)) {
        let m = b.transform();
        let (a, c) = (b.min, b.max);
        let v = |x: f32, y: f32, z: f32| rot * m.transform_point3(Vec3::new(x, y, z));
        let quads = [
            (Vec3::Z, [v(a.x, c.y, c.z), v(c.x, c.y, c.z), v(c.x, a.y, c.z), v(a.x, a.y, c.z)], 1.0),
            (Vec3::Y, [v(a.x, c.y, a.z), v(c.x, c.y, a.z), v(c.x, c.y, c.z), v(a.x, c.y, c.z)], 1.3),
            (Vec3::X, [v(c.x, c.y, c.z), v(c.x, c.y, a.z), v(c.x, a.y, a.z), v(c.x, a.y, c.z)], 0.8),
            (Vec3::NEG_X, [v(a.x, c.y, a.z), v(a.x, c.y, c.z), v(a.x, a.y, c.z), v(a.x, a.y, a.z)], 0.8),
        ];
        for (normal, q, shade) in quads {
            let nrm = rot * m.transform_vector3(normal);
            if nrm.z <= 0.01 {
                continue;
            }
            let base = surface_color(b.layer);
            let color = std::array::from_fn(|i| base[i] * b.tint[i] as f32 / 255.0 * shade);
            for p in &q {
                lo = lo.min(p.truncate());
                hi = hi.max(p.truncate());
            }
            // What sits inside another part goes first, so it stays hidden.
            let behind = if b.inside.is_some() { 1000.0 } else { 0.0 };
            faces.push(Face {
                quad: q.map(|p| p.truncate()),
                depth: q.iter().map(|p| p.z).sum::<f32>() / 4.0 - behind,
                color,
            });
        }
    }
    faces.sort_by(|a, b| a.depth.total_cmp(&b.depth));
    let size = hi - lo;
    let margin = n as f32 * 0.06;
    let k = ((n as f32 - 2.0 * margin) / size.x).min((n as f32 - 2.0 * margin) / size.y);
    let center = (lo + hi) * 0.5;
    let to_px = |p: Vec2| Vec2::new(n as f32 * 0.5 + (p.x - center.x) * k, n as f32 * 0.5 - (p.y - center.y) * k);
    let mut img = vec![[0u8; 4]; n * n];
    for f in &faces {
        let q = f.quad.map(to_px);
        let (qlo, qhi) = q.iter().fold((Vec2::splat(f32::MAX), Vec2::splat(f32::MIN)), |(a, b), p| (a.min(*p), b.max(*p)));
        let (x0, x1) = (qlo.x.floor().max(0.0) as usize, (qhi.x.ceil() as usize).min(n));
        let (y0, y1) = (qlo.y.floor().max(0.0) as usize, (qhi.y.ceil() as usize).min(n));
        for y in y0..y1 {
            for x in x0..x1 {
                let p = Vec2::new(x as f32 + 0.5, y as f32 + 0.5);
                let mut sign = 0.0f32;
                let inside = (0..4).all(|i| {
                    let c = (q[(i + 1) % 4] - q[i]).perp_dot(p - q[i]);
                    if c.abs() < 1e-6 {
                        return true;
                    }
                    if sign == 0.0 {
                        sign = c.signum();
                    }
                    c.signum() == sign
                });
                if inside {
                    img[y * n + x] = [
                        f.color[0].min(255.0) as u8,
                        f.color[1].min(255.0) as u8,
                        f.color[2].min(255.0) as u8,
                        255,
                    ];
                }
            }
        }
    }
    // A dark outline around the shape, like the drawn icons.
    let solid = |x: i32, y: i32| (0..n as i32).contains(&x) && (0..n as i32).contains(&y) && img[y as usize * n + x as usize][3] > 0;
    let mut out = img.clone();
    for y in 0..n as i32 {
        for x in 0..n as i32 {
            let i = y as usize * n + x as usize;
            if img[i][3] == 0 {
                continue;
            }
            if !(solid(x - 1, y) && solid(x + 1, y) && solid(x, y - 1) && solid(x, y + 1)) {
                out[i] = [img[i][0] / 3, img[i][1] / 3, img[i][2] / 3, 255];
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn corners(b: &GunBox) -> impl Iterator<Item = Vec3> + '_ {
        let m = b.transform();
        (0..8).map(move |i| {
            m.transform_point3(Vec3::new(
                if i & 1 == 0 { b.min.x } else { b.max.x },
                if i & 2 == 0 { b.min.y } else { b.max.y },
                if i & 4 == 0 { b.min.z } else { b.max.z },
            ))
        })
    }

    #[test]
    fn every_part_has_boxes_inside_the_bounds() {
        for kind in GUN_KINDS {
            let (lo, hi) = spec(kind).bounds;
            for part in 0..PARTS {
                assert!(boxes(kind).iter().any(|b| b.part == part), "{kind:?} part {part} has no boxes");
            }
            for b in boxes(kind).iter().filter(|b| b.shown(kind.shown_mods(0))) {
                for p in corners(b) {
                    assert!(
                        p.cmpge(lo - 0.1).all() && p.cmple(hi + 0.1).all(),
                        "{kind:?} part {} corner {p} is outside the bounds",
                        b.part
                    );
                }
            }
        }
    }

    #[test]
    fn attachments_add_their_boxes() {
        for kind in GUN_KINDS {
            let count = |mods| boxes(kind).iter().filter(|b| b.shown(mods)).count();
            for (bit, _) in crate::item::ATTACHMENTS {
                if kind.fits(bit) {
                    assert!(count(bit) > count(0), "{kind:?}: attachment {bit} adds nothing");
                }
            }
            assert!(muzzle(kind, SILENCER).x > muzzle(kind, 0).x + 5.0);
        }
    }

    #[test]
    fn the_iron_sights_line_up() {
        // The front post and the rear notch's two posts end on the sight line.
        for (kind, part) in [(GunKind::Pistol, SLIDE), (GunKind::M16, 1), (GunKind::M16, 3)] {
            let s = spec(kind);
            let tops: Vec<f32> = boxes(kind)
                .iter()
                .filter(|b| b.part == part && b.with == 0 && b.layer == BLUED && b.max.y > s.sight_height - 0.2)
                .map(|b| b.max.y)
                .collect();
            assert!(!tops.is_empty(), "{kind:?}");
            assert!(tops.iter().all(|&y| (y - s.sight_height).abs() < 0.11), "{kind:?}: {tops:?}");
        }
    }

    #[test]
    fn guns_point_along_the_raised_arm() {
        for kind in GUN_KINDS {
            // Arm space: the arm hangs along -Y; the muzzle must be further along it.
            let m = in_arm(kind);
            let grip = m.transform_point3(spec(kind).grip);
            let muzzle = m.transform_point3(muzzle(kind, 0));
            assert!(muzzle.y < grip.y - 4.0, "{kind:?}: grip {grip}, muzzle {muzzle}");
        }
    }

    #[test]
    fn exploded_parts_do_not_overlap() {
        for kind in GUN_KINDS {
            let mods = kind.shown_mods(0);
            let b: Vec<(Vec3, Vec3)> = (0..PARTS)
                .map(|p| {
                    let (lo, hi) = part_bounds(kind, p, mods);
                    let o = exploded_offset(kind, p);
                    (lo + o, hi + o)
                })
                .collect();
            for i in 0..PARTS {
                for j in i + 1..PARTS {
                    let overlap = b[i].0.x < b[j].1.x - 0.01
                        && b[j].0.x < b[i].1.x - 0.01
                        && b[i].0.y < b[j].1.y - 0.01
                        && b[j].0.y < b[i].1.y - 0.01;
                    assert!(!overlap, "{kind:?}: parts {i} and {j} overlap when taken apart");
                }
            }
        }
    }

    #[test]
    fn icons_have_a_shape() {
        for kind in GUN_KINDS {
            let n = icon(kind).iter().filter(|p| p[3] > 0).count();
            assert!(n > 800 && n < TILE * TILE / 2, "{kind:?}: {n} pixels");
        }
    }
}
