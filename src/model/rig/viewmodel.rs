//! A view model made in Blockbench (`tools/blockbench/`): bones, cubes with their own piece of
//! texture on every face, and keyframe animations. `bbmodel_to_rust.py` turns the .bbmodel
//! into data (`model::blockbench`: `pistol_vm.rs`, `pistol_vm.png`); this poses and draws it the way the
//! Blockbench preview does, so what is animated there is what the game shows.
//!
//! Model space is Blockbench's: pixels, the first-person camera at the origin looking -Z. A
//! bone turns about its origin (Euler order ZYX, degrees); an animation adds its position and
//! rotation to the bone's and sets its scale, and several animations add up.

use crate::model::prim::{self, BoxUv, Paint};
use crate::world::mesh::{Vertex, FACE_N};
use glam::{Mat4, Vec3};

pub struct Bone {
    pub name: &'static str,
    /// Index of the parent bone (-1: the root). Parents come before their children.
    pub parent: i32,
    pub origin: [f32; 3],
    pub rot: [f32; 3],
}

/// One face's piece of the texture: its page (texture layer from the model's first) and its
/// corners (u left, v top, u right, v bottom; 0..1 on the page).
#[derive(Clone, Copy)]
pub struct Face {
    pub page: u8,
    pub uv: [f32; 4],
}

impl Face {
    /// A face without texture: not drawn (the generated data uses it for such faces).
    #[allow(dead_code)]
    pub const NONE: Face = Face { page: u8::MAX, uv: [0.0; 4] };
}

pub struct Cube {
    pub name: &'static str,
    pub bone: usize,
    pub from: [f32; 3],
    pub to: [f32; 3],
    pub origin: [f32; 3],
    pub rot: [f32; 3],
    /// In the order of `world::mesh::FACE_N`: +X, -X, +Y, -Y, +Z, -Z.
    pub faces: [Face; 6],
}

pub struct Key {
    pub t: f32,
    pub v: [f32; 3],
    /// 0 linear, 1 catmull-rom, 2 step (hold until the next key).
    pub interp: u8,
}

pub struct Channel {
    pub bone: usize,
    /// 0 position, 1 rotation, 2 scale.
    pub kind: u8,
    /// Sorted by time.
    pub keys: &'static [Key],
}

pub struct Anim {
    pub name: &'static str,
    pub length: f32,
    /// 0 once, 1 hold on the last frame, 2 loop.
    pub looping: u8,
    pub channels: &'static [Channel],
}

/// What the animations do to one bone, on top of its rest pose.
#[derive(Clone, Copy)]
pub struct BonePose {
    pub pos: Vec3,
    pub rot: Vec3,
    pub scale: Vec3,
}

impl Default for BonePose {
    fn default() -> Self {
        Self { pos: Vec3::ZERO, rot: Vec3::ZERO, scale: Vec3::ONE }
    }
}

pub fn find_bone(bones: &[Bone], name: &str) -> Option<usize> {
    bones.iter().position(|b| b.name == name)
}

pub fn find_anim<'a>(anims: &'a [Anim], name: &str) -> Option<&'a Anim> {
    anims.iter().find(|a| a.name == name)
}

/// A channel's value at `time`, interpolated like Blockbench does.
pub fn sample(ch: &Channel, time: f32, looping: bool) -> Vec3 {
    let keys = ch.keys;
    let v = |k: &Key| Vec3::from(k.v);
    let eps = 1.0 / 1200.0;
    let after = keys.iter().position(|k| k.t >= time - eps);
    let (b, a) = match after {
        None => return v(&keys[keys.len() - 1]),
        Some(0) => return v(&keys[0]),
        Some(i) => (i - 1, i),
    };
    let (kb, ka) = (&keys[b], &keys[a]);
    if (ka.t - time).abs() < eps {
        return v(ka);
    }
    if kb.interp == 2 {
        return v(kb);
    }
    let alpha = ((time - kb.t) / (ka.t - kb.t).max(1e-6)).clamp(0.0, 1.0);
    if kb.interp == 1 || ka.interp == 1 {
        let n = keys.len();
        let before = if b > 0 {
            v(&keys[b - 1])
        } else if looping && n >= 3 {
            v(&keys[n - 2])
        } else {
            v(kb)
        };
        let next = if a + 1 < n {
            v(&keys[a + 1])
        } else if looping && n >= 3 {
            v(&keys[1])
        } else {
            v(ka)
        };
        return catmull_rom(before, v(kb), v(ka), next, alpha);
    }
    v(kb).lerp(v(ka), alpha)
}

fn catmull_rom(p0: Vec3, p1: Vec3, p2: Vec3, p3: Vec3, t: f32) -> Vec3 {
    let (t2, t3) = (t * t, t * t * t);
    0.5 * (2.0 * p1
        + (p2 - p0) * t
        + (2.0 * p0 - 5.0 * p1 + 4.0 * p2 - p3) * t2
        + (3.0 * p1 - p0 - 3.0 * p2 + p3) * t3)
}

/// Adds `anim` at `time` (seconds; wrapped for a looping one, held at its end otherwise) with
/// weight `w` to the pose. Channels of bones for which `skip` says so are left out. A scale
/// replaces the one there, eased in by the weight.
pub fn add_anim(pose: &mut [BonePose], anim: &Anim, time: f32, w: f32, skip: impl Fn(usize) -> bool) {
    let looping = anim.looping == 2;
    let time = if looping && anim.length > 0.0 {
        time.rem_euclid(anim.length)
    } else {
        time.clamp(0.0, anim.length)
    };
    for ch in anim.channels {
        if skip(ch.bone) || ch.keys.is_empty() {
            continue;
        }
        let v = sample(ch, time, looping);
        let p = &mut pose[ch.bone];
        match ch.kind {
            0 => p.pos += v * w,
            1 => p.rot += v * w,
            _ => p.scale = p.scale.lerp(v, w),
        }
    }
}

/// Euler angles in degrees, Blockbench's order (ZYX: X turned first, then Y, then Z).
pub fn rot_zyx(r: Vec3) -> Mat4 {
    Mat4::from_rotation_z(r.z.to_radians())
        * Mat4::from_rotation_y(r.y.to_radians())
        * Mat4::from_rotation_x(r.x.to_radians())
}

/// Each bone's transform from model space to where `root` puts the model, and whether it is
/// shown (a bone scaled to nothing hides everything in it).
pub fn bone_matrices(bones: &[Bone], pose: &[BonePose], root: Mat4) -> (Vec<Mat4>, Vec<bool>) {
    let mut mats = Vec::with_capacity(bones.len());
    let mut shown = Vec::with_capacity(bones.len());
    for (i, b) in bones.iter().enumerate() {
        let (parent, parent_shown) = if b.parent < 0 {
            (root, true)
        } else {
            (mats[b.parent as usize], shown[b.parent as usize])
        };
        let p = pose[i];
        let o = Vec3::from(b.origin);
        mats.push(
            parent
                * Mat4::from_translation(o + p.pos)
                * rot_zyx(Vec3::from(b.rot) + p.rot)
                * Mat4::from_scale(p.scale)
                * Mat4::from_translation(-o),
        );
        shown.push(parent_shown && p.scale.min_element() > 1e-3);
    }
    (mats, shown)
}

/// A cube's own turn about its origin, in its bone's model space.
pub fn cube_matrix(c: &Cube) -> Mat4 {
    let o = Vec3::from(c.origin);
    Mat4::from_translation(o) * rot_zyx(Vec3::from(c.rot)) * Mat4::from_translation(-o)
}

/// The face index (`FACE_N`) nearest to a direction, for the shading of a turned face.
fn face_of(n: Vec3) -> u8 {
    let a = n.abs();
    if a.x >= a.y && a.x >= a.z {
        if n.x > 0.0 { 0 } else { 1 }
    } else if a.y >= a.z {
        if n.y > 0.0 { 2 } else { 3 }
    } else if n.z > 0.0 {
        4
    } else {
        5
    }
}

/// One cube with transform `m` (model space to the world), its faces on the texture layers
/// from `first_layer`.
pub fn emit_cube(out: &mut Vec<Vertex>, c: &Cube, m: Mat4, first_layer: u32, light: [u8; 4], fl: u8) {
    let uv = BoxUv::Rects(c.faces.map(|f| f.uv));
    prim::cuboid(out, m, Vec3::from(c.from), Vec3::from(c.to), uv, |face| {
        let f = c.faces[face];
        (f.page != u8::MAX).then(|| Paint {
            layer: first_layer + f.page as u32,
            light,
            face: face_of(m.transform_vector3(Vec3::from(FACE_N[face].map(|v| v as f32)))),
            tint: [255; 3],
            fl,
        })
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    static KEYS: [Key; 3] = [
        Key { t: 0.0, v: [0.0, 0.0, 0.0], interp: 0 },
        Key { t: 1.0, v: [2.0, 0.0, 0.0], interp: 2 },
        Key { t: 2.0, v: [4.0, 0.0, 0.0], interp: 0 },
    ];

    #[test]
    fn keys_interpolate_like_blockbench() {
        let ch = Channel { bone: 0, kind: 0, keys: &KEYS };
        assert_eq!(sample(&ch, 0.5, false).x, 1.0);
        // A step key holds until the next one.
        assert_eq!(sample(&ch, 1.5, false).x, 2.0);
        assert_eq!(sample(&ch, 2.0, false).x, 4.0);
        assert_eq!(sample(&ch, 9.0, false).x, 4.0);
    }

    #[test]
    fn a_bone_turns_about_its_origin() {
        let bones = [Bone { name: "b", parent: -1, origin: [1.0, 0.0, 0.0], rot: [0.0, 90.0, 0.0] }];
        let (m, shown) = bone_matrices(&bones, &[BonePose::default()], Mat4::IDENTITY);
        assert!(shown[0]);
        // The origin stays; a point one pixel to its +X side turns to -Z.
        assert!(m[0].transform_point3(Vec3::new(1.0, 0.0, 0.0)).distance(Vec3::new(1.0, 0.0, 0.0)) < 1e-5);
        assert!(m[0].transform_point3(Vec3::new(2.0, 0.0, 0.0)).distance(Vec3::new(1.0, 0.0, -1.0)) < 1e-5);
    }
}
