//! The gun station, made in Blockbench (`tools/blockbench/gun_station.bbmodel`): a gunsmith's
//! bench two blocks wide whose drawer slides out while it is used: the cleaning brush in it on
//! the left, three boxes of rounds on the right (their count written on them).
//! The data `bbmodel_to_rust.py` made of it and its texture pages; posed and drawn through
//! `viewmodel`.
//!
//! Model space: Blockbench pixels, the station from (-8, 0, -8) to (24, 16, 8): its left half
//! (seen from the front) is the block around the origin. Its front (the drawer) faces +Z.

include!("gun_station_data.rs");

use super::viewmodel::{add_anim, bone_matrices, cube_matrix, emit_cube, find_anim, find_bone, BonePose};
use crate::world::mesh::Vertex;
use crate::world::textures::tex;
use glam::{IVec3, Mat4, Vec3};

/// The texture pages (`PAGES` of 128x128, one under the other), loaded into the texture
/// layers from `tex::GUN_STATION_MODEL`.
pub static PNG: &[u8] = include_bytes!("gun_station.png");

/// Seconds the drawer takes to slide out (its "open" animation).
pub fn open_seconds() -> f32 {
    find_anim(ANIMS, "open").map_or(0.5, |a| a.length)
}

/// From model space to the world for the station whose left half is at `p`, its front
/// toward `toward` (a horizontal unit direction).
pub fn root(p: IVec3, toward: Vec3) -> Mat4 {
    let yaw = toward.x.atan2(toward.z);
    Mat4::from_translation(p.as_vec3() + Vec3::new(0.5, 0.0, 0.5))
        * Mat4::from_rotation_y(yaw)
        * Mat4::from_scale(Vec3::splat(1.0 / 16.0))
}

use crate::item::AMMO_BOX_ROUNDS;

/// How full a box of rounds looks (0 empty .. 3 full).
fn fill_level(n: u16) -> usize {
    if n == 0 {
        0
    } else {
        (1 + (n as usize - 1) * 3 / AMMO_BOX_ROUNDS as usize).min(3)
    }
}

/// Each bone's matrix with the drawer `open` (0 shut .. 1 out), the brush in it or not, and
/// the boxes of rounds in it (how many in each; None: not there).
fn posed(root: Mat4, open: f32, brush: bool, ammo: [Option<u16>; 3]) -> (Vec<Mat4>, Vec<bool>) {
    let mut pose = vec![BonePose::default(); BONES.len()];
    if let Some(an) = find_anim(ANIMS, "open") {
        add_anim(&mut pose, an, open.clamp(0.0, 1.0) * an.length, 1.0, |_| false);
    }
    let mut hide = |name: &str| {
        if let Some(b) = find_bone(BONES, name) {
            pose[b].scale = Vec3::ZERO;
        }
    };
    if !brush {
        hide("brush");
    }
    for (i, &n) in ammo.iter().enumerate() {
        let Some(n) = n else {
            hide(&format!("ammo_box_{i}"));
            continue;
        };
        for level in 1..=3 {
            if level != fill_level(n) {
                hide(&format!("ammo_{i}_{level}"));
            }
        }
    }
    bone_matrices(BONES, &pose, root)
}

/// Lit up: the drawer's front and handle (the mouse is on them).
fn is_handle(c: &Cube) -> bool {
    c.name == "drawer_front" || c.name.starts_with("handle_")
}

#[allow(clippy::too_many_arguments)]
fn emit(out: &mut Vec<Vertex>, root: Mat4, open: f32, brush: bool, ammo: [Option<u16>; 3], handle_lit: bool, light: [u8; 4], fl: u8) {
    let (mats, shown) = posed(root, open, brush, ammo);
    for c in CUBES {
        if shown[c.bone] {
            let from = out.len();
            emit_cube(out, c, mats[c.bone] * cube_matrix(c), tex::GUN_STATION_MODEL, light, fl);
            if handle_lit && is_handle(c) {
                for v in &mut out[from..] {
                    v.light[0] = v.light[0].saturating_add(90);
                    v.light[1] = v.light[1].saturating_add(90);
                }
            }
        }
    }
    if open > 0.0 {
        for (i, n) in ammo.iter().enumerate() {
            if let (Some(n), Some((m, face))) = (n, label(&mats, i)) {
                emit_number(out, m, face, *n, light, fl);
            }
        }
    }
}

/// The station whose left half is at `p`: its drawer `open`, the brush in it or not, this many
/// rounds in each box, its handle lit up or not.
#[allow(clippy::too_many_arguments)]
pub fn emit_block(out: &mut Vec<Vertex>, p: IVec3, toward: Vec3, open: f32, brush: bool, ammo: [Option<u16>; 3], handle_lit: bool, light: [u8; 4], fl: u8) {
    emit(out, root(p, toward), open, brush, ammo, handle_lit, light, fl);
}

/// Where the count goes on a box: the bone's matrix, and the panel on its front (model
/// space, on the front face).
fn label(mats: &[Mat4], i: usize) -> Option<(Mat4, (Vec3, Vec3))> {
    let name = format!("ammo_box_{i}_front");
    let c = CUBES.iter().find(|c| c.name == name)?;
    let (a, b) = (Vec3::from(c.from), Vec3::from(c.to));
    let (lo, hi) = (a.min(b), a.max(b));
    Some((mats[c.bone], (Vec3::new(lo.x + 0.4, lo.y + 0.55, hi.z), Vec3::new(hi.x - 0.4, hi.y - 0.5, hi.z))))
}

/// A number stencilled on a panel (model space `lo` .. `hi` on a face looking +Z), as army
/// ammunition is marked: pale yellow seven-segment digits, a little gap where their strokes
/// meet, in the panel's middle.
fn emit_number(out: &mut Vec<Vertex>, m: Mat4, (lo, hi): (Vec3, Vec3), n: u16, light: [u8; 4], fl: u8) {
    // Segments a..g: top, top right, bottom right, bottom, bottom left, top left, middle.
    const DIGITS: [u8; 10] = [0x3f, 0x06, 0x5b, 0x4f, 0x66, 0x6d, 0x7d, 0x07, 0x7f, 0x6f];
    let text = n.to_string();
    let h = hi.y - lo.y;
    let (dw, gap, t) = (h * 0.5, h * 0.2, h * 0.11);
    let count = text.len() as f32;
    let width = count * dw + (count - 1.0) * gap;
    let mut x = (lo.x + hi.x) * 0.5 - width * 0.5;
    let (z0, z1) = (hi.z + 0.004, hi.z + 0.012);
    let paint = [[214, 196, 112]; 6];
    // The stencil's bridges: each stroke stops short of the next.
    let cut = t * 0.45;
    for ch in text.bytes() {
        let bits = DIGITS[(ch - 48) as usize];
        let (x0, x1, y0, ym, y1) = (x, x + dw, lo.y, lo.y + h * 0.5, hi.y);
        let segs: [(f32, f32, f32, f32); 7] = [
            (x0 + cut, y1 - t, x1 - cut, y1),
            (x1 - t, ym + cut, x1, y1 - cut),
            (x1 - t, y0 + cut, x1, ym - cut),
            (x0 + cut, y0, x1 - cut, y0 + t),
            (x0, y0 + cut, x0 + t, ym - cut),
            (x0, ym + cut, x0 + t, y1 - cut),
            (x0 + cut, ym - t * 0.5, x1 - cut, ym + t * 0.5),
        ];
        for (k, &(a, b, c, d)) in segs.iter().enumerate() {
            if bits & (1 << k) != 0 {
                super::emit_box(out, m, Vec3::new(a, b, z0), Vec3::new(c, d, z1), [tex::WOOL; 6], paint, light, fl);
            }
        }
        x += dw + gap;
    }
}

/// The boxes of rounds' cubes where the station's pose puts them (for the mouse to find
/// them in the drawer): the box, each cube and its matrix.
pub fn ammo_boxes(p: IVec3, toward: Vec3, open: f32) -> Vec<(usize, &'static Cube, Mat4)> {
    let (mats, _) = posed(root(p, toward), open, true, [Some(0); 3]);
    let mut out = Vec::new();
    for i in 0..3 {
        let prefix = format!("ammo_box_{i}_");
        for c in CUBES.iter().filter(|c| c.name.starts_with(&prefix)) {
            out.push((i, c, mats[c.bone] * cube_matrix(c)));
        }
    }
    out
}

/// As an item: the unit cube centered on the origin, like a block item (`emit_held`); the
/// station fits in it lengthwise.
pub fn emit_item(out: &mut Vec<Vertex>, m: Mat4, light: [u8; 4], fl: u8) {
    let root = m * Mat4::from_scale(Vec3::splat(1.0 / 32.0)) * Mat4::from_translation(Vec3::new(-8.0, -8.0, 0.0));
    emit(out, root, 0.0, true, [Some(0); 3], false, light, fl);
}

/// The drawer's front and handle where the station's pose puts them (for the mouse to open
/// or shut it with): each cube and its matrix.
pub fn drawer_handle(p: IVec3, toward: Vec3, open: f32) -> Vec<(&'static Cube, Mat4)> {
    let (mats, _) = posed(root(p, toward), open, true, [Some(0); 3]);
    CUBES
        .iter()
        .filter(|c| is_handle(c))
        .map(|c| (c, mats[c.bone] * cube_matrix(c)))
        .collect()
}

fn brush_cubes() -> impl Iterator<Item = &'static Cube> {
    let b = find_bone(BONES, "brush");
    CUBES.iter().filter(move |c| Some(c.bone) == b)
}

/// The brush's cubes where the station's pose puts them (for the mouse to find it in the
/// drawer): each cube's matrix.
pub fn brush_in_drawer(p: IVec3, toward: Vec3, open: f32) -> Vec<(&'static Cube, Mat4)> {
    let (mats, _) = posed(root(p, toward), open, true, [Some(0); 3]);
    brush_cubes().map(|c| (c, mats[c.bone] * cube_matrix(c))).collect()
}

/// The brush held with its bristles' middle at `at`, turned `turn` about the up axis, tipped
/// `tilt` (radians) as it scrubs.
pub fn emit_brush(out: &mut Vec<Vertex>, at: Vec3, turn: f32, tilt: f32, light: [u8; 4], fl: u8) {
    // The bristles' bottom middle in model space: the brush's origin, down to its bristles.
    let (lo, hi) = brush_cubes().fold((Vec3::splat(f32::MAX), Vec3::splat(f32::MIN)), |(lo, hi), c| {
        (lo.min(Vec3::from(c.from)), hi.max(Vec3::from(c.to)))
    });
    let bottom = Vec3::new((lo.x + hi.x) * 0.5, lo.y, (lo.z + hi.z) * 0.5);
    let m = Mat4::from_translation(at)
        * Mat4::from_rotation_y(turn)
        * Mat4::from_rotation_z(tilt)
        * Mat4::from_scale(Vec3::splat(1.0 / 16.0))
        * Mat4::from_translation(-bottom);
    for c in brush_cubes() {
        emit_cube(out, c, m * cube_matrix(c), tex::GUN_STATION_MODEL, light, fl);
    }
}

/// A box of rounds on its own (out of the drawer): its cubes (those of the drawer's first
/// box), and where they reach (model space).
fn box_cubes() -> (Vec<&'static Cube>, Vec3, Vec3) {
    let cubes: Vec<&Cube> = CUBES
        .iter()
        .filter(|c| c.name.starts_with("ammo_box_0_") || c.name.starts_with("ammo_rounds_0_"))
        .collect();
    let (mut lo, mut hi) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
    for c in cubes.iter().filter(|c| c.name.starts_with("ammo_box_0_")) {
        lo = lo.min(Vec3::from(c.from)).min(Vec3::from(c.to));
        hi = hi.max(Vec3::from(c.from)).max(Vec3::from(c.to));
    }
    (cubes, lo, hi)
}

/// The size of a box of rounds (Blockbench pixels: across, high, long).
pub fn ammo_box_size() -> Vec3 {
    let (_, lo, hi) = box_cubes();
    hi - lo
}

/// A box of rounds on its own with `rounds` in it, `m` putting it in place: from its bottom's
/// middle, in pixels, its front (with the count) toward +Z.
pub fn emit_ammo_box(out: &mut Vec<Vertex>, m: Mat4, rounds: u16, light: [u8; 4], fl: u8) {
    let (cubes, lo, hi) = box_cubes();
    let m = m * Mat4::from_translation(-Vec3::new((lo.x + hi.x) * 0.5, lo.y, (lo.z + hi.z) * 0.5));
    let level = fill_level(rounds);
    for c in cubes {
        if let Some(l) = c.name.strip_prefix("ammo_rounds_0_") {
            if l.parse::<usize>().ok() != Some(level) {
                continue;
            }
        }
        emit_cube(out, c, m * cube_matrix(c), tex::GUN_STATION_MODEL, light, fl);
    }
    let bones: Vec<Mat4> = vec![m; BONES.len()];
    if let Some((mm, face)) = label(&bones, 0) {
        emit_number(out, mm, face, rounds, light, fl);
    }
}
