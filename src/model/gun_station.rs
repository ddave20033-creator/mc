//! The gun stations, made in Blockbench (`tools/blockbench/gun_station.bbmodel` and, three
//! blocks wide for the rifles, `rifle_station.bbmodel`, both from `gen_gun_station.py`): a
//! gunsmith's bench whose drawer slides out while it is used: the cleaning brush in it on the
//! left, three boxes of rounds on the right (their count written on them).
//! The data `bbmodel_to_rust.py` made of them and their texture pages; posed and drawn
//! through `viewmodel`.
//!
//! Model space: Blockbench pixels, the station from (-8, 0, -8) to (24, 16, 8) (the rifle
//! station to 40): its left block (seen from the front) is the one around the origin. Its
//! front (the drawer) faces +Z.

#[allow(unused_imports, dead_code)]
mod small {
    include!("gun_station_data.rs");
}
#[allow(unused_imports, dead_code)]
mod big {
    include!("rifle_station_data.rs");
}
pub use small::PAGES;
use small::{BONES, CUBES};

use super::viewmodel::{add_anim, bone_matrices, cube_matrix, emit_cube, find_anim, find_bone, Anim, Bone, BonePose, Cube};
use crate::world::mesh::Vertex;
use crate::world::textures::tex;
use glam::{IVec3, Mat4, Vec3};

/// The texture pages (`PAGES` of 128x128, one under the other), loaded into the texture
/// layers from `tex::GUN_STATION_MODEL`, and the rifle station's (`RIFLE_PAGES`, from
/// `tex::RIFLE_STATION_MODEL`).
pub static PNG: &[u8] = include_bytes!("gun_station.png");
pub static RIFLE_PNG: &[u8] = include_bytes!("rifle_station.png");
pub const RIFLE_PAGES: u32 = big::PAGES;

/// A station's model: its bones, cubes, animations and first texture layer.
struct Model {
    bones: &'static [Bone],
    cubes: &'static [Cube],
    anims: &'static [Anim],
    layer: u32,
}

static SMALL: Model = Model { bones: small::BONES, cubes: small::CUBES, anims: small::ANIMS, layer: tex::GUN_STATION_MODEL };
static RIFLE: Model = Model { bones: big::BONES, cubes: big::CUBES, anims: big::ANIMS, layer: tex::RIFLE_STATION_MODEL };

/// The small station's model, or the rifle station's.
fn model(rifle: bool) -> &'static Model {
    if rifle {
        &RIFLE
    } else {
        &SMALL
    }
}

/// Seconds the drawer takes to slide out (its "open" animation).
pub fn open_seconds() -> f32 {
    find_anim(small::ANIMS, "open").map_or(0.5, |a| a.length)
}

/// From model space to the world for the station whose left half is at `p`, its front
/// toward `toward` (a horizontal unit direction).
pub fn root(p: IVec3, toward: Vec3) -> Mat4 {
    let yaw = toward.x.atan2(toward.z);
    Mat4::from_translation(p.as_vec3() + Vec3::new(0.5, 0.0, 0.5))
        * Mat4::from_rotation_y(yaw)
        * Mat4::from_scale(Vec3::splat(1.0 / 16.0))
}

use crate::item::{box_ammo, box_count, BOX_MAGNUM, BOX_RIFLE};

/// The rounds stand in a box nose up in rows, as many as it has (up to a full grid): across
/// it, along it, and the station's pixels per unit of the guns' models (the same size as the
/// rounds lying on the table, `gui::gun_station::PX`).
const BOX_ACROSS: usize = 7;
const BOX_ALONG: usize = 18;
const ROUND_PX: f32 = 0.026 * 16.0;

/// The rounds standing on a box's floor (the cube `floor`, drawn by `m`), `v` in the box.
fn emit_box_rounds(out: &mut Vec<Vertex>, m: Mat4, floor: &Cube, v: u16, light: [u8; 4], fl: u8) {
    let Some(ammo) = box_ammo(v) else { return };
    let (a, b) = (Vec3::from(floor.from), Vec3::from(floor.to));
    let (lo, hi) = (a.min(b), a.max(b));
    // Inside the walls (0.25 thick).
    let (x0, x1, z0, z1) = (lo.x + 0.25, hi.x - 0.25, lo.z + 0.25, hi.z - 0.25);
    let (dx, dz) = ((x1 - x0) / BOX_ACROSS as f32, (z1 - z0) / BOX_ALONG as f32);
    let n = (box_count(v) as usize).min(BOX_ACROSS * BOX_ALONG);
    // (a full box shows a full grid; the rows fill from the back)
    for k in 0..n {
        let (row, col) = (k / BOX_ACROSS, k % BOX_ACROSS);
        let at = Vec3::new(x0 + dx * (col as f32 + 0.5), hi.y, z0 + dz * (row as f32 + 0.5));
        let r = m * Mat4::from_translation(at) * Mat4::from_scale(Vec3::splat(ROUND_PX));
        super::gun_view::emit_round(out, ammo, false, r, light, fl);
    }
}
/// The count stencilled on a box: pale yellow, on a box of magnum rounds red.
const STENCIL: [u8; 3] = [214, 196, 112];
const STENCIL_MAGNUM: [u8; 3] = [226, 92, 72];
const STENCIL_RIFLE: [u8; 3] = [120, 196, 110];

/// The colour of a box's stencil: the kind of rounds in it.
fn stencil(v: u16) -> [u8; 3] {
    if v & BOX_MAGNUM != 0 {
        STENCIL_MAGNUM
    } else if v & BOX_RIFLE != 0 {
        STENCIL_RIFLE
    } else {
        STENCIL
    }
}

/// The rifle station's magazine loader in its drawer: there or not, and seconds into its
/// feeding while it loads (None: still).
#[derive(Clone, Copy, Default)]
pub struct Loader {
    pub there: bool,
    pub feed: Option<f32>,
}

/// Each bone's matrix with the drawer `open` (0 shut .. 1 out), the brush in it or not, the
/// boxes of rounds in it (how many in each; None: not there) and the loader.
fn posed(md: &Model, root: Mat4, open: f32, brush: bool, ammo: [Option<u16>; 3], loader: Loader) -> (Vec<Mat4>, Vec<bool>) {
    let mut pose = vec![BonePose::default(); md.bones.len()];
    if let Some(an) = find_anim(md.anims, "open") {
        add_anim(&mut pose, an, open.clamp(0.0, 1.0) * an.length, 1.0, |_| false);
    }
    match (loader.feed, find_anim(md.anims, "feed")) {
        (Some(t), Some(an)) if loader.there => add_anim(&mut pose, an, t.rem_euclid(an.length), 1.0, |_| false),
        _ => {
            if let Some(b) = find_bone(md.bones, "loader_round") {
                pose[b].scale = Vec3::ZERO;
            }
        }
    }
    if let (Some(b), false) = (find_bone(md.bones, "loader"), loader.there) {
        pose[b].scale = Vec3::ZERO;
    }
    let mut hide = |name: &str| {
        if let Some(b) = find_bone(md.bones, name) {
            pose[b].scale = Vec3::ZERO;
        }
    };
    if !brush {
        hide("brush");
    }
    for (i, &n) in ammo.iter().enumerate() {
        // The rounds in it are drawn on their own (`emit_box_rounds`).
        for level in 1..=3 {
            hide(&format!("ammo_{i}_{level}"));
        }
        if n.is_none() {
            hide(&format!("ammo_box_{i}"));
        }
    }
    bone_matrices(md.bones, &pose, root)
}

/// Lit up: the drawer's front and handle (the mouse is on them).
fn is_handle(c: &Cube) -> bool {
    c.name == "drawer_front" || c.name.starts_with("handle_")
}

#[allow(clippy::too_many_arguments)]
fn emit(md: &Model, out: &mut Vec<Vertex>, root: Mat4, open: f32, brush: bool, ammo: [Option<u16>; 3], loader: Loader, handle_lit: bool, light: [u8; 4], fl: u8) {
    let (mats, shown) = posed(md, root, open, brush, ammo, loader);
    for c in md.cubes {
        if shown[c.bone] {
            let from = out.len();
            emit_cube(out, c, mats[c.bone] * cube_matrix(c), md.layer, light, fl);
            if handle_lit && is_handle(c) {
                for v in &mut out[from..] {
                    v.light[0] = v.light[0].saturating_add(90);
                    v.light[1] = v.light[1].saturating_add(90);
                }
            }
        }
    }
    if open > 0.0 {
        // The rounds in the boxes (only seen with the drawer out).
        for (i, n) in ammo.iter().enumerate() {
            let floor = format!("ammo_box_{i}_floor");
            if let (Some(v), Some(c)) = (n, md.cubes.iter().find(|c| c.name == floor)) {
                if shown[c.bone] {
                    emit_box_rounds(out, mats[c.bone], c, *v, light, fl);
                }
            }
        }
        for (i, n) in ammo.iter().enumerate() {
            if let (Some(n), Some((m, face))) = (n, label(md, &mats, i)) {
                let color = stencil(*n);
                emit_number(out, m, face, box_count(*n), color, light, fl);
            }
        }
    }
}

/// The station (the rifle station with `rifle`) whose left block is at `p`: its drawer `open`,
/// the brush in it or not, this many rounds in each box, its loader, its handle lit up or not.
#[allow(clippy::too_many_arguments)]
pub fn emit_block(out: &mut Vec<Vertex>, rifle: bool, p: IVec3, toward: Vec3, open: f32, brush: bool, ammo: [Option<u16>; 3], loader: Loader, handle_lit: bool, light: [u8; 4], fl: u8) {
    emit(model(rifle), out, root(p, toward), open, brush, ammo, loader, handle_lit, light, fl);
}

/// Where the magazine lies in the rifle station's loader (the drawer `open`): a frame at its
/// middle, in the station's pixels, its x along the loader toward the feed block.
pub fn loader_mount(p: IVec3, toward: Vec3, open: f32) -> Option<Mat4> {
    let there = Loader { there: true, feed: None };
    let (mats, _) = posed(&RIFLE, root(p, toward), open, true, [Some(0); 3], there);
    let b = find_bone(RIFLE.bones, "loader_mag")?;
    Some(mats[b] * Mat4::from_translation(Vec3::from(RIFLE.bones[b].origin)))
}

/// Whether a cube with this matrix is there to be found: a part put away is shrunk to nothing
/// (its matrix cannot be undone, and every ray would seem to hit it).
fn findable(m: Mat4) -> bool {
    m.determinant().abs() > 1e-12
}

/// The loader's cubes where the rifle station's pose puts them (for the mouse to find it in
/// the drawer): each cube and its matrix.
pub fn loader_cubes(p: IVec3, toward: Vec3, open: f32) -> Vec<(&'static Cube, Mat4)> {
    let there = Loader { there: true, feed: None };
    let (mats, _) = posed(&RIFLE, root(p, toward), open, true, [Some(0); 3], there);
    RIFLE
        .cubes
        .iter()
        .filter(|c| c.name.starts_with("loader_") && !c.name.starts_with("loader_round"))
        .map(|c| (c, mats[c.bone] * cube_matrix(c)))
        .filter(|&(_, m)| findable(m))
        .collect()
}

/// The loader's bay in the rifle station's drawer (the drawer's own pixels): across from the
/// tools to the split before the boxes, front to back, up to above the loader.
const LOADER_BAY: (Vec3, Vec3) = (Vec3::new(9.5, 10.6, -5.65), Vec3::new(25.1, 16.0, 7.55));

/// Whether the world point `at` is in the loader's bay of the rifle station's drawer (open
/// `open`): where the loader goes, and everything about it is found.
pub fn in_loader_bay(p: IVec3, toward: Vec3, open: f32, at: Vec3) -> bool {
    let there = Loader { there: true, feed: None };
    let (mats, _) = posed(&RIFLE, root(p, toward), open, true, [Some(0); 3], there);
    let Some(b) = find_bone(RIFLE.bones, "drawer") else { return false };
    let q = mats[b].inverse().transform_point3(at);
    let (lo, hi) = LOADER_BAY;
    q.cmpge(lo).all() && q.cmple(hi).all()
}

/// The loader on its own (an item): its cubes, the unit cube centered on the origin.
pub fn emit_loader_item(out: &mut Vec<Vertex>, m: Mat4, light: [u8; 4], fl: u8) {
    let own: Vec<&Cube> = RIFLE
        .cubes
        .iter()
        .filter(|c| c.name.starts_with("loader_") && !c.name.starts_with("loader_round"))
        .collect();
    let (mut lo, mut hi) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
    for c in &own {
        for q in [Vec3::from(c.from), Vec3::from(c.to)] {
            let q = cube_matrix(c).transform_point3(q);
            lo = lo.min(q);
            hi = hi.max(q);
        }
    }
    let k = 0.9 / (hi - lo).max_element().max(1e-3);
    let root = m * Mat4::from_scale(Vec3::splat(k)) * Mat4::from_translation(-(lo + hi) * 0.5);
    for c in own {
        emit_cube(out, c, root * cube_matrix(c), RIFLE.layer, light, fl);
    }
}

/// Where the count goes on a box: the bone's matrix, and the panel on its front (model
/// space, on the front face).
fn label(md: &Model, mats: &[Mat4], i: usize) -> Option<(Mat4, (Vec3, Vec3))> {
    let name = format!("ammo_box_{i}_front");
    let c = md.cubes.iter().find(|c| c.name == name)?;
    let (a, b) = (Vec3::from(c.from), Vec3::from(c.to));
    let (lo, hi) = (a.min(b), a.max(b));
    Some((mats[c.bone], (Vec3::new(lo.x + 0.4, lo.y + 0.55, hi.z), Vec3::new(hi.x - 0.4, hi.y - 0.5, hi.z))))
}

/// A number stencilled on a panel (model space `lo` .. `hi` on a face looking +Z), as army
/// ammunition is marked: pale yellow seven-segment digits, a little gap where their strokes
/// meet, in the panel's middle.
fn emit_number(out: &mut Vec<Vertex>, m: Mat4, (lo, hi): (Vec3, Vec3), n: u16, color: [u8; 3], light: [u8; 4], fl: u8) {
    // Segments a..g: top, top right, bottom right, bottom, bottom left, top left, middle.
    const DIGITS: [u8; 10] = [0x3f, 0x06, 0x5b, 0x4f, 0x66, 0x6d, 0x7d, 0x07, 0x7f, 0x6f];
    let text = n.to_string();
    let h = hi.y - lo.y;
    let (dw, gap, t) = (h * 0.5, h * 0.2, h * 0.11);
    let count = text.len() as f32;
    let width = count * dw + (count - 1.0) * gap;
    let mut x = (lo.x + hi.x) * 0.5 - width * 0.5;
    // (standing off the face enough not to flicker through it, seen from further away too)
    let (z0, z1) = (hi.z + 0.05, hi.z + 0.12);
    let paint = [color; 6];
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

/// Grenades the rifle station's crate holds of each kind (frag, smoke).
pub const CRATE_MAX: u8 = 12;
/// The crate's halves (frag grenades, smoke grenades) inside its walls (model pixels) and how
/// the grenades stand in them: so many across and deep, this tall.
const CRATE_HALVES: [(f32, f32); 2] = [(8.6, 16.3), (16.7, 24.4)];
const CRATE_Z: (f32, f32) = (-4.8, 5.4);
const CRATE_FLOOR: f32 = 2.2;
const CRATE_GRID: (usize, usize) = (3, 4);
const CRATE_GRENADE: f32 = 4.2;

/// The rifle station's shelf bone matrix (it does not move).
fn shelf(p: IVec3, toward: Vec3) -> Option<Mat4> {
    let (mats, _) = posed(&RIFLE, root(p, toward), 0.0, true, [Some(0); 3], Loader::default());
    find_bone(RIFLE.bones, "shelf").map(|b| mats[b])
}

/// The grenades standing in the rifle station's crate (`n`: frag and smoke grenades), filled
/// from the back, and the count stencilled on each half's front.
pub fn emit_crate(out: &mut Vec<Vertex>, p: IVec3, toward: Vec3, n: [u8; 2], light: [u8; 4], fl: u8) {
    let Some(m) = shelf(p, toward) else { return };
    let (across, deep) = CRATE_GRID;
    for (half, &count) in n.iter().enumerate() {
        let (x0, x1) = CRATE_HALVES[half];
        let (dx, dz) = ((x1 - x0) / across as f32, (CRATE_Z.1 - CRATE_Z.0) / deep as f32);
        for k in 0..(count.min(CRATE_MAX) as usize).min(across * deep) {
            let (row, col) = (k / across, k % across);
            let at = Vec3::new(x0 + dx * (col as f32 + 0.5), CRATE_FLOOR + CRATE_GRENADE * 0.5, CRATE_Z.0 + dz * (row as f32 + 0.5));
            // (each turned a little its own way)
            let turn = Mat4::from_rotation_y(((k * 47 + half * 13) % 360) as f32 * 0.35_f32.to_radians() * 7.0);
            super::grenade::emit_sized(out, half == 1, m * Mat4::from_translation(at) * turn, CRATE_GRENADE, light, fl);
        }
        let name = format!("crate_front_{half}");
        if let Some(c) = RIFLE.cubes.iter().find(|c| c.name == name) {
            let (a, b) = (Vec3::from(c.from), Vec3::from(c.to));
            let (lo, hi) = (a.min(b), a.max(b));
            let face = (Vec3::new(lo.x + 1.5, lo.y + 0.45, hi.z), Vec3::new(hi.x - 1.5, hi.y - 0.45, hi.z));
            let color = if half == 0 { STENCIL } else { STENCIL_MAGNUM };
            emit_number(out, m, face, count as u16, color, light, fl);
        }
    }
}

/// The crate's halves where the station puts them (for the crosshair to find them): each
/// half's box (model pixels, up to the grenades' tops) and the shelf's matrix.
pub fn crate_halves(p: IVec3, toward: Vec3) -> Vec<(usize, Vec3, Vec3, Mat4)> {
    let Some(m) = shelf(p, toward) else { return Vec::new() };
    let (lo_x, hi_x) = (CRATE_HALVES[0].0 - 0.4, CRATE_HALVES[1].1 + 0.4);
    let mid = (CRATE_HALVES[0].1 + CRATE_HALVES[1].0) * 0.5;
    let (y0, y1) = (1.6, CRATE_FLOOR + CRATE_GRENADE);
    let (z0, z1) = (CRATE_Z.0 - 0.4, CRATE_Z.1 + 0.4);
    vec![
        (0, Vec3::new(lo_x, y0, z0), Vec3::new(mid, y1, z1), m),
        (1, Vec3::new(mid, y0, z0), Vec3::new(hi_x, y1, z1), m),
    ]
}

/// The boxes of rounds' cubes where the station's pose puts them (for the mouse to find
/// them in the drawer): the box, each cube and its matrix.
pub fn ammo_boxes(rifle: bool, p: IVec3, toward: Vec3, open: f32) -> Vec<(usize, &'static Cube, Mat4)> {
    let md = model(rifle);
    let (mats, _) = posed(md, root(p, toward), open, true, [Some(0); 3], Loader::default());
    let mut out = Vec::new();
    for i in 0..3 {
        let prefix = format!("ammo_box_{i}_");
        for c in md.cubes.iter().filter(|c| c.name.starts_with(&prefix)) {
            let m = mats[c.bone] * cube_matrix(c);
            if findable(m) {
                out.push((i, c, m));
            }
        }
    }
    out
}

/// As an item: the unit cube centered on the origin, like a block item (`emit_held`); the
/// station fits in it lengthwise.
pub fn emit_item(out: &mut Vec<Vertex>, rifle: bool, m: Mat4, light: [u8; 4], fl: u8) {
    let long = if rifle { 48.0 } else { 32.0 };
    let root = m * Mat4::from_scale(Vec3::splat(1.0 / long)) * Mat4::from_translation(Vec3::new(8.0 - long * 0.5, -8.0, 0.0));
    emit(model(rifle), out, root, 0.0, true, [Some(0); 3], Loader::default(), false, light, fl);
}

/// The drawer's front and handle where the station's pose puts them (for the mouse to open
/// or shut it with): each cube and its matrix.
pub fn drawer_handle(rifle: bool, p: IVec3, toward: Vec3, open: f32) -> Vec<(&'static Cube, Mat4)> {
    let md = model(rifle);
    let (mats, _) = posed(md, root(p, toward), open, true, [Some(0); 3], Loader::default());
    md.cubes
        .iter()
        .filter(|c| is_handle(c))
        .map(|c| (c, mats[c.bone] * cube_matrix(c)))
        .filter(|&(_, m)| findable(m))
        .collect()
}

fn brush_cubes(md: &'static Model) -> impl Iterator<Item = &'static Cube> {
    let b = find_bone(md.bones, "brush");
    md.cubes.iter().filter(move |c| Some(c.bone) == b)
}

/// The brush's cubes where the station's pose puts them (for the mouse to find it in the
/// drawer): each cube's matrix.
pub fn brush_in_drawer(rifle: bool, p: IVec3, toward: Vec3, open: f32) -> Vec<(&'static Cube, Mat4)> {
    let md = model(rifle);
    let (mats, _) = posed(md, root(p, toward), open, true, [Some(0); 3], Loader::default());
    brush_cubes(md).map(|c| (c, mats[c.bone] * cube_matrix(c)))
        .filter(|&(_, m)| findable(m)).collect()
}

/// The brush held with its bristles' middle at `at`, turned `turn` about the up axis, tipped
/// `tilt` (radians) as it scrubs.
pub fn emit_brush(out: &mut Vec<Vertex>, at: Vec3, turn: f32, tilt: f32, light: [u8; 4], fl: u8) {
    // The bristles' bottom middle in model space: the brush's origin, down to its bristles.
    let (lo, hi) = brush_cubes(&SMALL).fold((Vec3::splat(f32::MAX), Vec3::splat(f32::MIN)), |(lo, hi), c| {
        (lo.min(Vec3::from(c.from)), hi.max(Vec3::from(c.to)))
    });
    let bottom = Vec3::new((lo.x + hi.x) * 0.5, lo.y, (lo.z + hi.z) * 0.5);
    let m = Mat4::from_translation(at)
        * Mat4::from_rotation_y(turn)
        * Mat4::from_rotation_z(tilt)
        * Mat4::from_scale(Vec3::splat(1.0 / 16.0))
        * Mat4::from_translation(-bottom);
    for c in brush_cubes(&SMALL) {
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

/// A box of rounds on its own with `v` in it (its value: the count and the kind), `m`
/// putting it in place: from its bottom's middle, in pixels, its front (with the count)
/// toward +Z.
pub fn emit_ammo_box(out: &mut Vec<Vertex>, m: Mat4, v: u16, light: [u8; 4], fl: u8) {
    let (cubes, lo, hi) = box_cubes();
    let m = m * Mat4::from_translation(-Vec3::new((lo.x + hi.x) * 0.5, lo.y, (lo.z + hi.z) * 0.5));
    for c in cubes {
        // Its rounds are drawn in 3D (`emit_box_rounds`), not the old flat layer of them.
        if c.name.starts_with("ammo_rounds_0_") {
            continue;
        }
        emit_cube(out, c, m * cube_matrix(c), tex::GUN_STATION_MODEL, light, fl);
        if c.name == "ammo_box_0_floor" {
            emit_box_rounds(out, m, c, v, light, fl);
        }
    }
    let bones: Vec<Mat4> = vec![m; BONES.len()];
    if let Some((mm, face)) = label(&SMALL, &bones, 0) {
        emit_number(out, mm, face, box_count(v), stencil(v), light, fl);
    }
}

#[cfg(test)]
mod ammo_box_tests {
    use super::*;

    /// The rounds drawn in a box: vertices on the revolver's texture pages (magnum rounds),
    /// and on the pistol's (9 mm).
    fn rounds(out: &[Vertex]) -> (usize, usize) {
        let revolver = tex::REVOLVER_VIEW as f32..(tex::REVOLVER_VIEW + crate::model::revolver_vm::PAGES) as f32;
        let pistol = tex::PISTOL_VIEW as f32..(tex::PISTOL_VIEW + crate::model::pistol_vm::PAGES) as f32;
        (out.iter().filter(|v| revolver.contains(&v.layer)).count(), out.iter().filter(|v| pistol.contains(&v.layer)).count())
    }

    #[test]
    fn a_box_shows_its_own_rounds_in_3d() {
        let draw = |v: u16| {
            let mut out = Vec::new();
            emit_ammo_box(&mut out, Mat4::IDENTITY, v, [255; 4], 0);
            rounds(&out)
        };
        let (m, p) = draw(40 | BOX_MAGNUM);
        assert!(m > 0 && p == 0, "{m} {p}");
        let (m, p) = draw(40);
        assert!(m == 0 && p > 0, "{m} {p}");
        assert_eq!(draw(0), (0, 0));
        // More rounds, more drawn.
        assert!(draw(80).1 > draw(40).1);
        // In the drawer too, when it is out.
        let drawer = |open: f32| {
            let mut out = Vec::new();
            emit(&SMALL, &mut out, Mat4::IDENTITY, open, true, [Some(12 | BOX_MAGNUM), None, Some(5)], Loader::default(), false, [255; 4], 0);
            rounds(&out)
        };
        let (m, p) = drawer(1.0);
        assert!(m > 0 && p > 0);
    }
}
