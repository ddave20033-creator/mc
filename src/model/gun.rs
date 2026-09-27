//! The pistol's 3D model: boxes grouped into the five parts that go together at the gun
//! station (frame, barrel, recoil spring, slide, magazine), and the attachments fitted there
//! (scope, silencer, extended magazine, laser sight). The same boxes are drawn in the hand,
//! on the player model, as a dropped item and in the gun station's 3D view.
//!
//! Gun space is in "gun pixels": the muzzle points to +X, up is +Y, the right side is +Z. The
//! origin is on the top of the frame, above the trigger.

use super::emit_box;
use crate::item::gun_mod;
use crate::world::mesh::Vertex;
use crate::world::textures::tex;
use glam::{Mat4, Vec3};
use std::sync::OnceLock;

pub const FRAME: usize = 0;
pub const BARREL: usize = 1;
pub const SPRING: usize = 2;
pub const SLIDE: usize = 3;
pub const MAGAZINE: usize = 4;
pub const PARTS: usize = 5;

/// The top of the grip, where the hand holds the gun.
pub const GRIP: Vec3 = Vec3::new(-4.7, 0.0, 0.0);
/// The line of the iron sights: the tops of the front post and of the rear notch's posts.
pub const SIGHT_HEIGHT: f32 = 6.3;
/// The back of the rear sight.
pub const REAR_SIGHT: f32 = -6.7;
/// The scope's axis, and the back of its eyepiece.
pub const SCOPE_HEIGHT: f32 = 8.0;
pub const SCOPE_EYE: f32 = -8.05;
/// Where the spent case comes out: the ejection port on the right of the slide.
pub const EJECTION_PORT: Vec3 = Vec3::new(1.7, 4.8, 1.4);
/// Where the beam leaves the laser sight.
pub const LASER: Vec3 = Vec3::new(6.9, -1.35, 0.0);
/// The grip (and the magazine in it) slants back toward the bottom.
const GRIP_TILT: f32 = -14.0;
/// Size of the bare gun: from the back of the grip to the muzzle, and from the bottom of the
/// magazine to the top of the sights.
pub const BOUNDS: (Vec3, Vec3) = (Vec3::new(-9.6, -10.6, -1.7), Vec3::new(8.6, 6.4, 1.7));

/// Where the bullet leaves the gun (the end of the silencer when there is one).
pub fn muzzle(mods: u8) -> Vec3 {
    if mods & gun_mod::SILENCER != 0 {
        Vec3::new(15.4, 3.0, 0.0)
    } else {
        Vec3::new(8.6, 3.0, 0.0)
    }
}

#[derive(Clone, Copy)]
pub struct GunBox {
    pub part: usize,
    pub min: Vec3,
    pub max: Vec3,
    /// Turned about the top of the grip (the grip and the magazine).
    pub tilted: bool,
    pub layer: u32,
    pub tint: [u8; 3],
    /// The part this box sits inside once the gun is together (the barrel and the spring in
    /// the slide, the magazine in the grip): drawn first where there is no depth buffer.
    pub inside: Option<usize>,
    /// Only there with this attachment (`gun_mod` bit; 0 = always).
    pub with: u8,
    /// Left out with this attachment (the plain baseplate under an extended magazine).
    pub without: u8,
}

impl GunBox {
    /// From the box's own corners to gun space.
    pub fn transform(&self) -> Mat4 {
        if self.tilted {
            Mat4::from_translation(GRIP)
                * Mat4::from_rotation_z(GRIP_TILT.to_radians())
                * Mat4::from_translation(-GRIP)
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

/// Collects the boxes: the part and attachment they belong to are set as it goes.
struct Builder {
    boxes: Vec<GunBox>,
    part: usize,
    with: u8,
    without: u8,
}

impl Builder {
    fn add(&mut self, min: [f32; 3], max: [f32; 3], tilted: bool, layer: u32, tint: [u8; 3]) {
        self.boxes.push(GunBox {
            part: self.part,
            min: Vec3::from(min),
            max: Vec3::from(max),
            tilted,
            layer,
            tint,
            inside: None,
            with: self.with,
            without: self.without,
        });
    }
}

/// Every box of the pistol and its attachments.
pub fn boxes() -> &'static [GunBox] {
    static B: OnceLock<Vec<GunBox>> = OnceLock::new();
    B.get_or_init(|| {
        let (poly, blued, steel) = (tex::GUN_POLYMER, tex::GUN_BLUED, tex::GUN_STEEL);
        let (glass, white) = (tex::GUN_GLASS, tex::WOOL);
        let mut k = Builder {
            boxes: Vec::new(),
            part: FRAME,
            with: 0,
            without: 0,
        };

        // Frame: the dust cover with an accessory rail under it, the grip with finger
        // grooves, the beavertail, the trigger guard and trigger, the magazine release, the
        // slide stop and the takedown lever.
        k.add([-6.6, 0.0, -1.2], [7.0, 2.0, 1.2], false, poly, WHITE);
        k.add([3.0, -0.3, -0.9], [6.8, 0.0, 0.9], false, poly, WHITE);
        for i in 0..3 {
            let x = 3.3 + i as f32 * 1.15;
            k.add([x, -0.65, -0.9], [x + 0.55, -0.3, 0.9], false, poly, WHITE);
        }
        k.add([-6.9, -9.0, -1.45], [-2.5, 0.4, 1.45], true, poly, WHITE);
        for y in [-2.6, -4.7, -6.8] {
            k.add([-2.5, y, -1.25], [-2.15, y + 0.9, 1.25], true, poly, WHITE);
        }
        k.add([-7.6, 0.4, -1.0], [-6.6, 1.7, 1.0], false, poly, WHITE);
        k.add([-2.8, -3.5, -0.5], [2.3, -2.8, 0.5], false, poly, WHITE);
        k.add([1.6, -3.5, -0.5], [2.3, 0.0, 0.5], false, poly, WHITE);
        k.add([-1.0, -2.4, -0.3], [-0.3, 0.0, 0.3], false, steel, WHITE);
        k.add([-2.9, -1.5, -1.6], [-2.2, -0.7, -1.3], false, steel, [150; 3]);
        k.add([-3.9, 1.2, -1.36], [-0.9, 1.8, -1.2], false, steel, [120; 3]);
        k.add([0.4, 1.1, -1.3], [1.2, 1.7, 1.3], false, steel, [150; 3]);

        // Barrel: the tube and the chamber block at its back.
        k.part = BARREL;
        k.add([-3.4, 2.3, -0.75], [8.6, 3.7, 0.75], false, steel, WHITE);
        k.add([-3.6, 2.1, -1.0], [-0.6, 4.0, 1.0], false, steel, WHITE);

        // Recoil spring: the guide rod and the turns of the coil around it.
        k.part = SPRING;
        k.add([0.4, 1.1, -0.25], [7.6, 1.7, 0.25], false, steel, WHITE);
        for i in 0..8 {
            let x = 0.8 + i as f32 * 0.85;
            k.add([x, 0.75, -0.55], [x + 0.4, 2.05, 0.55], false, steel, WHITE);
        }

        // Slide: the body with a narrower top, serrations at the back and the front, the
        // ejection port (the barrel's chamber seen through it), the muzzle opening and the
        // three-dot iron sights (a notch between two posts at the back, a post in front).
        k.part = SLIDE;
        k.add([-7.1, 2.0, -1.3], [8.0, 4.6, 1.3], false, blued, WHITE);
        k.add([-7.1, 4.6, -1.05], [8.0, 5.2, 1.05], false, blued, WHITE);
        for (x0, n) in [(-6.8, 5), (5.3, 3)] {
            for i in 0..n {
                let x = x0 + i as f32 * 0.6;
                for z in [1.3, -1.34] {
                    k.add([x, 2.4, z], [x + 0.28, 4.4, z + 0.04], false, poly, WHITE);
                }
            }
        }
        k.add([0.2, 3.7, 1.3], [3.2, 4.6, 1.34], false, steel, [175; 3]);
        k.add([0.2, 5.2, 0.1], [3.2, 5.24, 1.05], false, steel, [175; 3]);
        k.add([8.0, 2.4, -0.55], [8.06, 3.6, 0.55], false, poly, WHITE);
        k.add([-6.7, 5.2, -1.05], [-5.6, 5.6, 1.05], false, blued, WHITE);
        k.add([-6.7, 5.6, -1.05], [-5.6, 6.3, -0.35], false, blued, WHITE);
        k.add([-6.7, 5.6, 0.35], [-5.6, 6.3, 1.05], false, blued, WHITE);
        k.add([-6.74, 5.85, -0.85], [-6.7, 6.1, -0.6], false, white, WHITE);
        k.add([-6.74, 5.85, 0.6], [-6.7, 6.1, 0.85], false, white, WHITE);
        k.add([6.9, 5.2, -0.28], [7.5, 6.3, 0.28], false, blued, WHITE);
        k.add([6.86, 5.85, -0.13], [6.9, 6.1, 0.13], false, white, WHITE);

        // Magazine: inside the grip, with a cartridge on top and its baseplate under it.
        k.part = MAGAZINE;
        k.add([-6.3, -9.2, -1.0], [-3.2, -0.6, 1.0], true, blued, WHITE);
        k.add([-5.9, -0.6, -0.45], [-3.6, 0.1, 0.45], true, steel, BRASS);
        k.without = gun_mod::EXTENDED_MAGAZINE;
        k.add([-7.1, -10.1, -1.55], [-2.4, -9.2, 1.55], true, poly, WHITE);
        k.without = 0;

        // Extended magazine: it reaches further out of the grip, with a red baseplate.
        k.with = gun_mod::EXTENDED_MAGAZINE;
        k.add([-6.5, -13.4, -1.05], [-3.0, -9.2, 1.05], true, blued, WHITE);
        k.add([-7.2, -14.3, -1.55], [-2.3, -13.4, 1.55], true, poly, [220, 80, 70]);

        // Scope on two rings on the slide: the tube, its bells, a turret and the lenses.
        k.part = SLIDE;
        k.with = gun_mod::SCOPE;
        k.add([-4.8, 5.2, -0.9], [-3.8, 7.0, 0.9], false, blued, WHITE);
        k.add([1.8, 5.2, -0.9], [2.8, 7.0, 0.9], false, blued, WHITE);
        k.add([-6.0, 6.9, -1.1], [4.5, 9.1, 1.1], false, blued, WHITE);
        k.add([4.5, 6.5, -1.5], [7.5, 9.5, 1.5], false, blued, WHITE);
        k.add([-8.0, 6.6, -1.4], [-6.0, 9.4, 1.4], false, blued, WHITE);
        k.add([-1.0, 9.1, -0.5], [0.2, 9.7, 0.5], false, blued, WHITE);
        k.add([7.5, 6.8, -1.2], [7.55, 9.2, 1.2], false, glass, WHITE);
        k.add([-8.05, 6.9, -1.1], [-8.0, 9.1, 1.1], false, glass, WHITE);

        // Silencer on the barrel's end.
        k.part = BARREL;
        k.with = gun_mod::SILENCER;
        k.add([8.6, 1.9, -1.1], [15.0, 4.1, 1.1], false, poly, WHITE);
        k.add([15.0, 2.2, -0.8], [15.4, 3.8, 0.8], false, steel, [120; 3]);
        k.add([10.2, 1.88, -1.12], [10.5, 4.12, 1.12], false, steel, [110; 3]);

        // Laser sight on the accessory rail, with its red lens.
        k.part = FRAME;
        k.with = gun_mod::LASER;
        k.add([2.9, -2.2, -0.85], [6.8, -0.65, 0.85], false, poly, [160; 3]);
        k.add([6.8, -1.8, -0.45], [6.86, -0.9, 0.45], false, white, [255, 60, 50]);

        let mut b = k.boxes;
        for x in &mut b {
            x.inside = match x.part {
                _ if x.with != 0 => None,
                BARREL | SPRING => Some(SLIDE),
                // All of the magazine but its baseplate.
                MAGAZINE if x.min.y > -10.0 => Some(FRAME),
                _ => None,
            };
        }
        b
    })
}

/// The middle of the magazine's bottom when it is in (where the hand holds it).
pub fn magazine_bottom() -> Vec3 {
    let tilt = Mat4::from_translation(GRIP)
        * Mat4::from_rotation_z(GRIP_TILT.to_radians())
        * Mat4::from_translation(-GRIP);
    tilt.transform_point3(Vec3::new(-4.75, -10.1, 0.0))
}

/// Down along the grip (the way the magazine comes out).
pub fn grip_axis() -> Vec3 {
    Mat4::from_rotation_z(GRIP_TILT.to_radians()).transform_vector3(Vec3::NEG_Y)
}

/// Where each part comes from when it is put in at the gun station (added to its place):
/// the barrel drops in from above, the spring and the slide slide on from the front and the
/// magazine goes up into the grip.
pub fn assembly_offset(part: usize) -> Vec3 {
    match part {
        FRAME => Vec3::new(0.0, -5.0, 0.0),
        BARREL => Vec3::new(0.0, 7.0, 0.0),
        SPRING => Vec3::new(9.0, 0.0, 0.0),
        SLIDE => Vec3::new(12.0, 0.0, 0.0),
        _ => grip_axis() * 11.0,
    }
}

/// Where each part lies when the gun is taken apart for cleaning.
pub fn exploded_offset(part: usize) -> Vec3 {
    match part {
        FRAME => Vec3::new(0.0, -2.5, 0.0),
        BARREL => Vec3::new(0.0, 5.0, 0.0),
        SPRING => Vec3::new(0.0, 2.0, 0.0),
        SLIDE => Vec3::new(0.0, 9.5, 0.0),
        _ => Vec3::new(10.0, -3.5, 0.0),
    }
}

/// The whole pistol with these attachments, in gun space, transformed by `m`.
pub fn emit_pistol(out: &mut Vec<Vertex>, m: Mat4, light: [u8; 4], fl: u8, mods: u8) {
    emit_pistol_with(out, m, light, fl, mods, |_| Vec3::ZERO);
}

/// The pistol with each part moved by `offset(part)` (the slide kicking back on a shot).
pub fn emit_pistol_with(
    out: &mut Vec<Vertex>,
    m: Mat4,
    light: [u8; 4],
    fl: u8,
    mods: u8,
    offset: impl Fn(usize) -> Vec3,
) {
    emit_pistol_parts(out, m, light, fl, mods, |part| {
        Some(Mat4::from_translation(offset(part)))
    });
}

/// The pistol with each part placed by `place(part)` in gun space (None leaves it out): the
/// magazines coming and going while reloading.
pub fn emit_pistol_parts(
    out: &mut Vec<Vertex>,
    m: Mat4,
    light: [u8; 4],
    fl: u8,
    mods: u8,
    place: impl Fn(usize) -> Option<Mat4>,
) {
    for b in boxes().iter().filter(|b| b.shown(mods)) {
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
pub fn gun_to_unit() -> Mat4 {
    let (lo, hi) = BOUNDS;
    let size = (hi - lo).max_element();
    Mat4::from_scale(Vec3::splat(1.0 / size)) * Mat4::from_translation(-(lo + hi) * 0.5)
}

/// The pistol in the right hand of the player model (arm space, model pixels), aimed along
/// the arm: the muzzle points away from the shoulder and the sights toward the back of the
/// hand.
pub fn in_arm() -> Mat4 {
    let basis = Mat4::from_cols(
        glam::Vec4::new(0.0, -1.0, 0.0, 0.0),
        glam::Vec4::new(0.0, 0.0, -1.0, 0.0),
        glam::Vec4::new(1.0, 0.0, 0.0, 0.0),
        glam::Vec4::W,
    );
    Mat4::from_translation(Vec3::new(1.0, -11.0, 0.0))
        * basis
        * Mat4::from_scale(Vec3::splat(0.45))
        * Mat4::from_translation(Vec3::new(5.8, 4.4, 0.0))
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
        let (lo, hi) = BOUNDS;
        for part in 0..PARTS {
            assert!(boxes().iter().any(|b| b.part == part), "part {part} has no boxes");
        }
        for b in boxes().iter().filter(|b| b.shown(0)) {
            for p in corners(b) {
                assert!(
                    p.cmpge(lo - 0.05).all() && p.cmple(hi + 0.05).all(),
                    "part {} corner {p} is outside the bounds",
                    b.part
                );
            }
        }
    }

    #[test]
    fn attachments_add_their_boxes() {
        let count = |mods| boxes().iter().filter(|b| b.shown(mods)).count();
        for (bit, _) in crate::item::ATTACHMENTS {
            assert!(count(bit) > count(0), "attachment {bit} adds nothing");
        }
        // The extended magazine replaces the plain baseplate and reaches further down.
        let lowest = |mods| {
            boxes()
                .iter()
                .filter(|b| b.shown(mods))
                .flat_map(corners)
                .map(|p| p.y)
                .fold(f32::MAX, f32::min)
        };
        assert!(lowest(gun_mod::EXTENDED_MAGAZINE) < lowest(0) - 3.0);
        assert!(muzzle(gun_mod::SILENCER).x > muzzle(0).x + 6.0);
    }

    #[test]
    fn the_sights_line_up() {
        // The front post and the rear notch's two posts end on the sight line.
        let tops: Vec<f32> = boxes()
            .iter()
            .filter(|b| b.part == SLIDE && b.with == 0 && b.layer == tex::GUN_BLUED && b.max.y > 6.0)
            .map(|b| b.max.y)
            .collect();
        assert_eq!(tops.len(), 3);
        assert!(tops.iter().all(|&y| (y - SIGHT_HEIGHT).abs() < 1e-4));
    }

    #[test]
    fn the_gun_points_along_the_raised_arm() {
        // Arm space: the arm hangs along -Y; the muzzle must be further along it than the grip.
        let m = in_arm();
        let grip = m.transform_point3(GRIP);
        let muzzle = m.transform_point3(muzzle(0));
        assert!(muzzle.y < grip.y - 4.0, "grip {grip}, muzzle {muzzle}");
        // The sights are toward the back of the hand (-Z), which faces up when aiming.
        let top = m.transform_point3(Vec3::new(0.0, 6.0, 0.0));
        let bottom = m.transform_point3(Vec3::new(0.0, -6.0, 0.0));
        assert!(top.z < bottom.z);
    }
}

#[cfg(test)]
mod dump {
    use super::*;

    /// GUN_DUMP=<file>: writes the model's faces (view, part, layer, shade, depth, tint, four
    /// corners) seen from a few angles, with and without attachments, to look at.
    #[test]
    fn dump_views() {
        let Ok(path) = std::env::var("GUN_DUMP") else {
            return;
        };
        let mut out = String::new();
        let views = [
            (0.0f32, 0.0f32, false, 0u8),
            (0.6, 0.35, false, 0),
            (0.4, 0.3, true, 0),
            (-0.9, 0.25, false, 0),
            (0.0, 0.0, false, 15),
            (0.7, 0.3, false, 15),
        ];
        for (v, &(yaw, pitch, exploded, mods)) in views.iter().enumerate() {
            let rot = glam::Mat3::from_rotation_x(pitch) * glam::Mat3::from_rotation_y(yaw);
            for b in boxes().iter().filter(|b| b.shown(mods)) {
                let off = if exploded { exploded_offset(b.part) } else { Vec3::ZERO };
                let m = Mat4::from_translation(off) * b.transform();
                let (lo, hi) = (b.min, b.max);
                let c = |x: f32, y: f32, z: f32| rot * m.transform_point3(Vec3::new(x, y, z));
                let faces = [
                    (Vec3::Z, [c(lo.x, hi.y, hi.z), c(hi.x, hi.y, hi.z), c(hi.x, lo.y, hi.z), c(lo.x, lo.y, hi.z)]),
                    (Vec3::NEG_Z, [c(hi.x, hi.y, lo.z), c(lo.x, hi.y, lo.z), c(lo.x, lo.y, lo.z), c(hi.x, lo.y, lo.z)]),
                    (Vec3::X, [c(hi.x, hi.y, hi.z), c(hi.x, hi.y, lo.z), c(hi.x, lo.y, lo.z), c(hi.x, lo.y, hi.z)]),
                    (Vec3::NEG_X, [c(lo.x, hi.y, lo.z), c(lo.x, hi.y, hi.z), c(lo.x, lo.y, hi.z), c(lo.x, lo.y, lo.z)]),
                    (Vec3::Y, [c(lo.x, hi.y, lo.z), c(hi.x, hi.y, lo.z), c(hi.x, hi.y, hi.z), c(lo.x, hi.y, hi.z)]),
                    (Vec3::NEG_Y, [c(lo.x, lo.y, hi.z), c(hi.x, lo.y, hi.z), c(hi.x, lo.y, lo.z), c(lo.x, lo.y, lo.z)]),
                ];
                for (n, q) in faces {
                    let n = rot * m.transform_vector3(n);
                    if n.z <= 0.01 {
                        continue;
                    }
                    let behind = !exploded && b.inside.is_some();
                    let depth = q.iter().map(|p| p.z).sum::<f32>() / 4.0 - if behind { 1000.0 } else { 0.0 };
                    let shade = 0.5 + 0.5 * n.dot(Vec3::new(-0.35, 0.6, 0.75).normalize()).max(0.0);
                    out += &format!(
                        "{v} {} {} {shade} {depth} {} {} {}",
                        b.part, b.layer, b.tint[0], b.tint[1], b.tint[2]
                    );
                    for p in q {
                        out += &format!(" {} {}", p.x, p.y);
                    }
                    out += "\n";
                }
            }
        }
        std::fs::write(path, out).unwrap();
    }
}
