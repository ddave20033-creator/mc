//! Models of block entities and where things lie on them: chest lids and doors, the items in
//! chests, on crafting tables and in and on furnaces, and the slot highlights.

use super::furnace::{grill_box, Furnace, FLIP_TIME};
use crate::item::{icon, Icon, ItemId, Slot, Stack, BUCKET, COAL, LAVA_BUCKET};
use crate::model::prim::{self, quad_at, BoxUv, Paint, Sides};
use crate::util::vertex_light;
use crate::world::mesh::{box_uv, corner_pos, flags, Vertex, CORNERS, FACE_N, FURNACE_HOLLOWS};
use crate::textures::tex;
use crate::world::*;
use glam::{IVec3, Mat4, Vec3};
use std::f32::consts::{FRAC_PI_2, PI};

/// Box in a block-local frame (0..1) with model UVs, transformed by `m` into the world.
fn emit_part(out: &mut Vec<Vertex>, m: Mat4, lo: Vec3, hi: Vec3, layers: [u32; 6], light: [u8; 4]) {
    prim::cuboid(out, m, lo, hi, BoxUv::Model, |face| {
        Some(Paint { layer: layers[face], light, face: face as u8, tint: [255; 3], fl: flags::ENTITY })
    });
}

/// The whole texture on a quad, its top left at the first corner.
const FLAT_UVS: [[f32; 2]; 4] = [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];

/// A chest half's 27 slots lie on its floor in 3 rows of 9, like Minecraft's chest screen.
pub const CHEST_COLS: usize = 9;
pub const CHEST_ROWS: usize = 3;
/// The frame drawn around the inside of a chest (2 of 16 pixels from the edge): the slots
/// are within it.
const CHEST_RIM: f32 = 2.0 / 16.0;
/// What is in a chest lies on the floor inside it.
pub use crate::world::mesh::CHEST_FLOOR;

/// Where a chest half's inside is across (0 at its left, as seen from the front, .. 1):
/// within the frame, which a double chest half has on its outer sides only. `side` is where
/// the other half is (as in `build_chest_lid`: -1, 0 or 1).
fn chest_inner(side: i32) -> (f32, f32) {
    match side.signum() {
        1 => (CHEST_RIM, 1.0),
        -1 => (0.0, 1.0 - CHEST_RIM),
        _ => (CHEST_RIM, 1.0 - CHEST_RIM),
    }
}

/// Size of a chest slot's cell on the floor: across (the chest's right) and deep (its front).
pub fn chest_cell_size(side: i32) -> (f32, f32) {
    let (a, b) = chest_inner(side);
    (
        (b - a) / CHEST_COLS as f32,
        (1.0 - 2.0 * CHEST_RIM) / CHEST_ROWS as f32,
    )
}

/// The chest half at `q` facing `facing`: (floor center, its right, its front) in the world.
fn chest_floor(q: IVec3, facing: u8) -> (Vec3, Vec3, Vec3) {
    (
        q.as_vec3() + Vec3::new(0.5, CHEST_FLOOR, 0.5),
        chest_right(facing).as_vec3(),
        facing_dir(facing).as_vec3(),
    )
}

/// Where slot `i` (0..27) lies on the floor of the chest half at `q`: the first row at the
/// back, left to right as seen from the front.
pub fn chest_cell(q: IVec3, facing: u8, side: i32, i: usize) -> Vec3 {
    let (o, right, front) = chest_floor(q, facing);
    let (cw, cd) = chest_cell_size(side);
    let x = chest_inner(side).0 + ((i % CHEST_COLS) as f32 + 0.5) * cw - 0.5;
    let z = CHEST_RIM + ((i / CHEST_COLS) as f32 + 0.5) * cd - 0.5;
    o + right * x + front * z
}

/// The slot whose cell on the floor of the chest half at `q` holds `point`.
pub fn chest_cell_at(q: IVec3, facing: u8, side: i32, point: Vec3) -> Option<usize> {
    let (o, right, front) = chest_floor(q, facing);
    let (cw, cd) = chest_cell_size(side);
    let (a, b) = chest_inner(side);
    let x = (point - o).dot(right) + 0.5 - a;
    let z = (point - o).dot(front) + 0.5 - CHEST_RIM;
    if !(0.0..b - a).contains(&x) || !(0.0..1.0 - 2.0 * CHEST_RIM).contains(&z) {
        return None;
    }
    let i = (z / cd) as usize * CHEST_COLS + (x / cw) as usize;
    (i < 27).then_some(i)
}

/// Where the other half of the double chest at `q` is, across (-1, 0 or 1).
pub fn chest_side(b: Block, facing: u8) -> i32 {
    chest_partner_offset(b).map_or(0, |d| d.dot(chest_right(facing)))
}

/// An item lying on a surface at `c`: blocks as little cubes, the rest flat. `size` is the
/// flat item's size; `lift` raises it (the one under the mouse).
fn lying_item(
    out: &mut Vec<Vertex>,
    c: Vec3,
    turn: f32,
    size: f32,
    lift: f32,
    st: &Stack,
    light: [u8; 4],
) {
    let m = match icon(st.item) {
        // A block lies as a low slab, like a small pile of it, rather than a tall cube.
        Icon::Block(_) => {
            let k = size * 0.66;
            Mat4::from_translation(c + Vec3::Y * (k * 0.25 + lift))
                * Mat4::from_rotation_y(turn)
                * Mat4::from_scale(Vec3::new(k, k * 0.5, k))
        }
        Icon::Flat(_) => {
            Mat4::from_translation(c + Vec3::Y * (size / 32.0 + 0.005 + lift))
                * Mat4::from_rotation_y(turn)
                * Mat4::from_rotation_x(-FRAC_PI_2)
                * Mat4::from_scale(Vec3::splat(size))
        }
    };
    crate::model::emit_lying(out, m, st, light, flags::ENTITY);
    // A second copy on top for a stack, like a small pile.
    if st.count > 1 && matches!(icon(st.item), Icon::Flat(_)) {
        let pile = Mat4::from_translation(Vec3::new(size * 0.06, size * 0.1, -size * 0.06));
        crate::model::emit_lying(out, pile * m, st, light, flags::ENTITY);
    }
}

/// The items in an open chest half, lying on its floor; `lift` is the slot under the mouse.
#[allow(clippy::too_many_arguments)]
pub fn build_chest_items(
    out: &mut Vec<Vertex>,
    q: IVec3,
    facing: u8,
    side: i32,
    slots: &[Slot],
    lift: Option<usize>,
    sky: u8,
    blk: u8,
) {
    let light = vertex_light(sky, blk);
    let (cw, cd) = chest_cell_size(side);
    // Within its cell, so the ones along the edges stay inside the frame.
    let size = cw.min(cd) * 0.9;
    let face_yaw = {
        let d = facing_dir(facing).as_vec3();
        d.x.atan2(d.z)
    };
    for (i, st) in slots.iter().enumerate().take(27) {
        let Some(st) = st else { continue };
        let c = chest_cell(q, facing, side, i);
        let turn = ((q.x * 31 + q.z * 17 + i as i32 * 7).rem_euclid(9)) as f32 * 0.05 - 0.2;
        let up = if lift == Some(i) { 0.04 } else { 0.0 };
        lying_item(out, c, face_yaw + turn, size, up, st, light);
    }
}

/// A soft highlight on a surface, brightening it a little: the slot under the mouse in a
/// chest or on a table, or the corner of a furnace where the held meat goes. `c` are its
/// corners in order around it; drawn with the overlays (multiplied onto what is there).
pub fn build_glow(out: &mut Vec<Vertex>, c: [Vec3; 4]) {
    let paint = Paint { layer: tex::SLOT_GLOW, light: [255, 255, 0, 0], face: 0, tint: [255; 3], fl: flags::OVERLAY };
    // Both sides.
    quad_at(out, c, FLAT_UVS, &paint, Sides::Both);
}

/// Chest lid (with its latch) hinged at the back; `open` is 0 (closed) .. 1 (open). `side`:
/// 0 for a single chest, else the double chest's other half is at local `side` * X; the lid
/// then reaches it and the latch sits half on each side of the seam.
pub fn build_chest_lid(
    out: &mut Vec<Vertex>,
    p: IVec3,
    facing: u8,
    side: i32,
    open: f32,
    sky: u8,
    blk: u8,
) {
    let light = vertex_light(sky, blk);
    // Local frame: front toward +Z; rotate it to the chest facing around the block center.
    let yaw = match front_face(facing) {
        4 => 0.0,
        5 => PI,
        0 => FRAC_PI_2,
        _ => -FRAC_PI_2,
    };
    let center = Vec3::new(0.5, 0.0, 0.5);
    let to_world = Mat4::from_translation(p.as_vec3() + center)
        * Mat4::from_rotation_y(yaw)
        * Mat4::from_translation(-center);
    // Ease out like Minecraft: fast at first, settling at the top.
    let t = 1.0 - (1.0 - open.clamp(0.0, 1.0)).powi(3);
    let hinge = Vec3::new(0.0, 10.0 / 16.0, 1.0 / 16.0);
    let m = to_world
        * Mat4::from_translation(hinge)
        * Mat4::from_rotation_x(-t * FRAC_PI_2 * 0.95)
        * Mat4::from_translation(-hinge);
    let px = 1.0 / 16.0;
    // Faces: +X, -X, top, bottom, front (+Z), back.
    let mut lid = [
        tex::CHEST_SIDE,
        tex::CHEST_SIDE,
        tex::CHEST_TOP,
        tex::CHEST_INSIDE,
        tex::CHEST_FRONT,
        tex::CHEST_SIDE,
    ];
    let (mut x0, mut x1, mut latch) = (px, 15.0 * px, (7.0 * px, 9.0 * px));
    match side.signum() {
        1 => (x1, latch) = (1.0, (15.0 * px, 1.0)),
        -1 => (x0, latch) = (0.0, (0.0, px)),
        _ => {}
    }
    if side != 0 {
        let dir = [side.signum(), 0, 0];
        for (face, l) in lid.iter_mut().enumerate() {
            *l = crate::world::mesh::chest_open_layer(*l, face, dir);
        }
    }
    emit_part(
        out,
        m,
        Vec3::new(x0, 10.0 * px, px),
        Vec3::new(x1, 14.0 * px, 15.0 * px),
        lid,
        light,
    );
    emit_part(
        out,
        m,
        Vec3::new(latch.0, 7.0 * px, 15.0 * px),
        Vec3::new(latch.1, 11.0 * px, 16.0 * px),
        [tex::CHEST_LATCH; 6],
        light,
    );
}

/// One door half at `p`, swung `open` (0 closed .. 1 open) around its hinge edge.
pub fn build_door(out: &mut Vec<Vertex>, p: IVec3, b: Block, open: f32, sky: u8, blk: u8) {
    let light = vertex_light(sky, blk);
    let f = door_facing(b);
    let hinge_right = door_hinge_right(b);
    // Closed, the panel lies against side `c`; open, against side `o` (the hinge side).
    let c = facing_dir(door_side_facing(f, false, hinge_right));
    let o_facing = door_side_facing(f, true, hinge_right);
    let o = facing_dir(o_facing);
    let (cv, ov) = (c.as_vec3(), o.as_vec3());
    // Panel x runs along `bx`; flipped where that would make a left-handed frame.
    let flip = ov.cross(Vec3::Y).dot(cv) < 0.0;
    let bx = if flip { -ov } else { ov };
    let t = 3.0 / 16.0;
    // Turning about the vertical edge shared by both positions: +90 degrees takes a facing
    // to the previous one (east to north).
    let c_facing = door_side_facing(f, false, hinge_right);
    let mut sign = if c_facing == (o_facing + 3) & 3 { 1.0 } else { -1.0 };
    // Swinging out turns the other way, into the block on the closed side.
    if door_out(b) {
        sign = -sign;
    }
    let eased = {
        let k = open.clamp(0.0, 1.0);
        k * k * (3.0 - 2.0 * k)
    };
    let center = p.as_vec3() + Vec3::new(0.5, 0.0, 0.5);
    let pivot = center + (cv + ov) * (0.5 - t * 0.5);
    let m = Mat4::from_translation(pivot)
        * Mat4::from_rotation_y(sign * eased * FRAC_PI_2)
        * Mat4::from_translation(-pivot);
    // Panel space: x along the width (0 at the free edge, 1 at the hinge, unless flipped),
    // y up, z across the thickness (1 at the block side).
    let to_world = |l: Vec3| {
        let w = center + bx * (l.x - 0.5) + Vec3::Y * l.y + cv * (0.5 - t + (l.z - (1.0 - t)));
        m.transform_point3(w)
    };
    let layer = face_texture(b, 4);
    let lo = Vec3::new(0.0, 0.0, 1.0 - t);
    let hi = Vec3::ONE;
    for face in 0..6 {
        let n_world = {
            let n = Vec3::from(FACE_N[face].map(|v| v as f32));
            let w = bx * n.x + Vec3::Y * n.y + cv * n.z;
            m.transform_vector3(w)
        };
        // Face index nearest the turned normal (for the shading by direction).
        let shade_face = (0..6)
            .max_by(|&a, &b| {
                let d = |i: usize| Vec3::from(FACE_N[i].map(|v| v as f32)).dot(n_world);
                d(a).total_cmp(&d(b))
            })
            .unwrap();
        let mut pos = [Vec3::ZERO; 4];
        let mut uvs = [[0.0; 2]; 4];
        for (i, &(su, sv)) in CORNERS.iter().enumerate() {
            let k = Vec3::from(corner_pos(face, su, sv));
            let local = lo + (hi - lo) * k;
            let mut uv = box_uv(face, local.to_array());
            if face >= 4 {
                // Both sides show the hinges (the texture's left edge) at the hinge.
                uv[0] = if flip { local.x } else { 1.0 - local.x };
            }
            (pos[i], uvs[i]) = (to_world(local), uv);
        }
        let paint = Paint { layer, light, face: shade_face as u8, tint: [255; 3], fl: flags::ENTITY };
        quad_at(out, pos, uvs, &paint, Sides::Front);
    }
}

/// Spacing of the 3x3 grid drawn on the crafting table's top (about 3.3 of 16 pixels).
pub const TABLE_CELL: f32 = 0.207;

/// A table's grid as seen by someone standing on its `side` (a facing: the direction from
/// the table to them): (top center, their right, toward them).
fn table_top(p: IVec3, side: u8) -> (Vec3, Vec3, Vec3) {
    let toward = facing_dir(side).as_vec3();
    (
        p.as_vec3() + Vec3::new(0.5, 1.0, 0.5),
        (-toward).cross(Vec3::Y),
        toward,
    )
}

/// Where cell `i` of a crafting table's grid is, read like a page from `side` (cell 4 is the
/// middle, where what is crafted appears).
pub fn table_cell(p: IVec3, side: u8, i: usize) -> Vec3 {
    let (o, right, toward) = table_top(p, side);
    o + right * ((i % 3) as f32 - 1.0) * TABLE_CELL + toward * ((i / 3) as f32 - 1.0) * TABLE_CELL
}

/// The grid cell of a crafting table under `point` on its top.
pub fn table_cell_at(p: IVec3, side: u8, point: Vec3) -> Option<usize> {
    let (o, right, toward) = table_top(p, side);
    let x = (point - o).dot(right) / TABLE_CELL + 1.5;
    let z = (point - o).dot(toward) / TABLE_CELL + 1.5;
    if !(0.0..3.0).contains(&x) || !(0.0..3.0).contains(&z) {
        return None;
    }
    Some(z as usize * 3 + x as usize)
}

/// Items left in a crafting table grid, lying on its top in a 3x3 layout facing whoever
/// last used it from `side`; `lift` is the cell under the mouse.
#[allow(clippy::too_many_arguments)]
pub fn build_table_items(
    out: &mut Vec<Vertex>,
    p: IVec3,
    side: u8,
    grid: &[Slot; 9],
    lift: Option<usize>,
    sky: u8,
    blk: u8,
) {
    let light = vertex_light(sky, blk);
    let toward = facing_dir(side).as_vec3();
    let face_yaw = toward.x.atan2(toward.z);
    for (i, st) in grid.iter().enumerate() {
        let Some(st) = st else { continue };
        let c = table_cell(p, side, i);
        // A little turn per slot so it looks placed by hand.
        let turn = ((p.x * 31 + p.z * 17 + i as i32 * 7).rem_euclid(9)) as f32 * 0.07 - 0.28;
        let up = if lift == Some(i) { 0.04 } else { 0.0 };
        lying_item(out, c, face_yaw + turn, 0.2, up, st, light);
    }
}

/// Seconds the ingredients take to slide together into the middle of the table.
pub const CRAFT_SLIDE: f32 = 0.3;

/// What was crafted at a table, lying in the middle of the grid (over whatever is left there).
/// `t` is the seconds since it was made: the ingredients (`used`, one of each cell) slide into
/// the middle, shrinking, and it swells up there. `hovered`: lifted a little.
#[allow(clippy::too_many_arguments)]
pub fn build_table_made(
    out: &mut Vec<Vertex>,
    p: IVec3,
    side: u8,
    made: &Stack,
    used: &[Slot; 9],
    t: f32,
    hovered: bool,
    sky: u8,
    blk: u8,
) {
    let light = vertex_light(sky, blk);
    let toward = facing_dir(side).as_vec3();
    let face_yaw = toward.x.atan2(toward.z);
    let middle = table_cell(p, side, 4);
    let k = (t / CRAFT_SLIDE).clamp(0.0, 1.0);
    if k < 1.0 {
        let ease = k * k * (3.0 - 2.0 * k);
        for (i, st) in used.iter().enumerate() {
            let Some(st) = st else { continue };
            let at = table_cell(p, side, i).lerp(middle, ease) + Vec3::Y * 0.02;
            let turn = face_yaw + ease * 2.0;
            lying_item(out, at, turn, 0.2 * (1.0 - 0.6 * ease), 0.0, st, light);
        }
    }
    // It appears as they meet, a little bigger at first, then settles: within the middle
    // cell, just above whatever is left there.
    let grow = ((k - 0.7) / 0.3).clamp(0.0, 1.0);
    if grow > 0.0 {
        let pop = 1.0 + 0.15 * (grow * PI).sin() * (1.0 - (t - CRAFT_SLIDE).clamp(0.0, 1.0));
        let up = 0.02 + if hovered { 0.03 } else { 0.0 };
        let size = TABLE_CELL * 0.78 * grow * pop;
        lying_item(out, middle, face_yaw, size, up, made, light);
    }
}

/// What lies on and in a furnace: meat on the corners of its top (with the side on the fire
/// underneath, turning over while flipped), and what is being smelted in its mouth.
#[allow(clippy::too_many_arguments)]
pub fn build_furnace_items(
    out: &mut Vec<Vertex>,
    p: IVec3,
    facing: u8,
    f: &Furnace,
    time: f32,
    sky: u8,
    blk: u8,
    inside_light: [u8; 4],
    flame_planes: bool,
) {
    let light = vertex_light(sky, blk);
    let base = p.as_vec3();
    for (i, g) in f.grill.iter().enumerate() {
        let Some(g) = g else { continue };
        let (lo, hi) = grill_box(i);
        let c = base + (lo + hi) * 0.5;
        // 0 when the flip starts, 1 when the meat is back down.
        let t = 1.0 - g.flip / FLIP_TIME;
        let ease = t * t * (3.0 - 2.0 * t);
        let lift = if g.flip > 0.0 {
            (t * PI).sin() * 0.3
        } else {
            0.0
        };
        // Side 0 (the sprite's front) faces up when side 1 is down.
        let lying = if g.down == 1 { -FRAC_PI_2 } else { FRAC_PI_2 };
        let turn = ((p.x * 13 + p.z * 29 + i as i32 * 11).rem_euclid(7)) as f32 * 0.09 - 0.27;
        // A thick piece: its rim shows how the side on the fire is doing.
        let (size, thick) = (0.4, 1.6);
        let m = Mat4::from_translation(Vec3::new(
            c.x,
            base.y + 1.0 + size * thick / 32.0 + 0.004 + lift,
            c.z,
        )) * Mat4::from_rotation_y(turn)
            * Mat4::from_rotation_x(lying + PI * (1.0 - ease))
            * Mat4::from_scale(Vec3::new(size, size, size * thick));
        crate::model::emit_sprite_sides(
            out,
            m,
            [g.side_layer(0), g.side_layer(1)],
            g.side_layer(g.down as usize),
            light,
            flags::ENTITY,
        );
    }
    build_furnace_inside(out, p, facing, f, time, inside_light, flame_planes);
}

/// Where flames rise from in a burning furnace's firebox (near the front, where they show
/// through the slot): the world spot for `k` (0..1 across it).
pub fn furnace_flame_spot(p: IVec3, facing: u8, k: f32) -> Vec3 {
    let d = facing_dir(facing).as_vec3();
    let right = Vec3::Y.cross(d);
    let fire_y = FURNACE_HOLLOWS[1].0;
    p.as_vec3() + Vec3::new(0.5, fire_y + 0.05, 0.5) + right * (k - 0.5) * 0.6 + d * 0.41
}

/// How many pieces show for a stack in a furnace opening: one, a few, or a heap.
fn pile_stage(count: u8) -> usize {
    match count {
        0 => 0,
        1 => 1,
        2..=16 => 3,
        _ => 6,
    }
}

/// Where the pieces of a heap lie in a furnace hollow: (across, toward the front, up), the
/// first one in front (it is the one being smelted or burning brightest).
/// The fuel's heap in the low firebox lies nearer the front, where it can be seen.
const FIRE_HEAP: [(f32, f32, f32); 6] = [
    (0.0, 0.42, 0.0),
    (-0.17, 0.39, 0.0),
    (0.17, 0.38, 0.0),
    (-0.08, 0.3, 0.0),
    (0.1, 0.29, 0.0),
    (0.0, 0.35, 0.06),
];
const HEAP: [(f32, f32, f32); 6] = [
    (0.0, 0.38, 0.0),
    (-0.15, 0.32, 0.0),
    (0.15, 0.31, 0.0),
    (-0.07, 0.2, 0.0),
    (0.09, 0.19, 0.0),
    (0.0, 0.27, 0.08),
];

/// One piece of `item` lying at `c` (its bottom), turned `turn`: blocks as small cubes, the
/// rest as a flat item on its back. `heat` (0..1) makes it glow red hot.
#[allow(clippy::too_many_arguments)]
fn emit_piece(
    out: &mut Vec<Vertex>,
    c: Vec3,
    turn: f32,
    item: ItemId,
    scale: f32,
    heat: f32,
    tilt: f32,
    light: [u8; 4],
) {
    let m = match icon(item) {
        Icon::Block(_) => {
            let k = 0.11 * scale;
            Mat4::from_translation(c + Vec3::Y * k * 0.5)
                * Mat4::from_rotation_y(turn)
                * Mat4::from_rotation_z(tilt)
                * Mat4::from_scale(Vec3::splat(k))
        }
        Icon::Flat(_) => {
            let k = 0.17 * scale;
            Mat4::from_translation(c + Vec3::Y * (k / 32.0 + 0.004))
                * Mat4::from_rotation_y(turn)
                * Mat4::from_rotation_x(-FRAC_PI_2 + tilt)
                * Mat4::from_scale(Vec3::splat(k))
        }
    };
    let start = out.len();
    crate::model::emit_held(out, m, item, light, flags::ENTITY);
    if heat > 0.0 {
        let hot = [255.0, 110.0, 40.0];
        let tint: [u8; 3] =
            std::array::from_fn(|k| (255.0 + (hot[k] - 255.0) * heat).clamp(0.0, 255.0) as u8);
        for v in &mut out[start..] {
            for (c, t) in v.tint.iter_mut().zip(tint) {
                *c = ((*c as u16 * t as u16) / 255) as u8;
            }
            if heat > 0.3 {
                v.tint[3] |= flags::EMISSIVE;
            }
        }
    }
}

/// Inside a furnace's openings (see `mesh::FURNACE_HOLLOWS`). In the mouth above, what is
/// being smelted lies in a heap of one, three or six pieces by how much there is; while the
/// furnace burns the front piece glows red hot, shrinks and turns into what it smelts into,
/// then cools (and pops out of the front). In the firebox below, the fuel lies the same way
/// and burns (lava as a pool): in flame planes here with `flame_planes`, otherwise the
/// resource pack's flame particles (`furnace_flame_spots`) do it, like its torches.
fn build_furnace_inside(
    out: &mut Vec<Vertex>,
    p: IVec3,
    facing: u8,
    f: &Furnace,
    time: f32,
    light: [u8; 4],
    flame_planes: bool,
) {
    let d = facing_dir(facing).as_vec3();
    let right = Vec3::Y.cross(d);
    let yaw = d.x.atan2(d.z);
    let center = p.as_vec3() + Vec3::new(0.5, 0.0, 0.5);
    let [(mouth_y, _, _), (fire_y, _, _)] = [FURNACE_HOLLOWS[0], FURNACE_HOLLOWS[1]];
    let lit = f.burn > 0.0;
    let seed = (p.x * 31 + p.z * 17 + p.y * 7).rem_euclid(97);
    let spot = |(a, t, up): (f32, f32, f32), floor: f32| {
        center + right * a + d * t + Vec3::Y * (floor + up)
    };

    // The mouth: what is still to smelt and what is done, in one heap. How big it is shows
    // how much is in there, and the share of finished pieces how much of it is done.
    let raw = f.input.map_or(0, |s| s.count);
    let done = f.output.map_or(0, |s| s.count);
    let total = raw as u32 + done as u32;
    if total > 0 {
        let n = pile_stage(total.min(255) as u8);
        let mut finished = ((n as u32 * done as u32 + total / 2) / total) as usize;
        if done > 0 {
            finished = finished.max(1);
        }
        if raw > 0 {
            finished = finished.min(n - 1);
        }
        let raw_item = f.input.map(|s| s.item);
        let smelts_into = raw_item.and_then(|i| f.smelts(i));
        let done_item = f.output.map(|s| s.item).or(smelts_into);
        let progress = (f.cook / f.smelt_time()).clamp(0.0, 1.0);
        let smelting = smelts_into.is_some() && (lit || f.cook > 0.0);
        // The front pieces are the ones still to smelt (the first of them is being smelted),
        // the finished ones lie behind.
        let first_done = n - finished;
        for i in (0..n).rev() {
            let turn = yaw + ((seed + i as i32 * 23) % 11) as f32 * 0.12 - 0.6;
            let tilt = [0.0, 0.18, -0.15, 0.1, -0.2, 0.25][i];
            let warm = if lit { 0.12 } else { 0.0 };
            let (item, heat, scale) = if i >= first_done {
                (done_item.unwrap_or(0), warm, 1.0)
            } else if i == 0 && smelting {
                // Heats up (0..0.55), changes (0.55..0.65), cools (0.65..1).
                let change = ((progress - 0.55) / 0.1).clamp(0.0, 1.0);
                let heat = if progress < 0.55 {
                    ((progress - 0.05) / 0.5).clamp(0.0, 1.0)
                } else {
                    1.0 - ((progress - 0.65) / 0.35).clamp(0.0, 1.0) * 0.8
                };
                let item = if change >= 0.5 {
                    done_item.unwrap_or(0)
                } else {
                    raw_item.unwrap_or(0)
                };
                (item, heat, 1.0 - (change * PI).sin() * 0.5)
            } else {
                (raw_item.unwrap_or(0), warm, 1.0)
            };
            if item == 0 {
                continue;
            }
            let shake = if i == 0 && heat > 0.6 {
                (time * 40.0).sin() * 0.05 * heat
            } else {
                0.0
            };
            let at = spot(HEAP[i], mouth_y);
            emit_piece(out, at, turn, item, scale, heat, tilt + shake, light);
        }
    }

    // The firebox.
    let fuel = f.fuel.filter(|s| s.item != BUCKET);
    if fuel.is_some_and(|s| s.item == LAVA_BUCKET) {
        // Lava: a pool on the floor (drawn like flowing lava, glowing).
        let (a, t0, t1) = (0.36, 0.1, 0.49);
        let y = fire_y + 0.035;
        let corners = [
            spot((-a, t0, 0.0), y),
            spot((a, t0, 0.0), y),
            spot((a, t1, 0.0), y),
            spot((-a, t1, 0.0), y),
        ];
        let paint = Paint {
            layer: tex::LAVA,
            light: [255; 4],
            face: 2,
            tint: [128, 128, 0],
            fl: flags::FLUID | flags::EMISSIVE | flags::ENTITY,
        };
        quad_at(out, corners, FLAT_UVS, &paint, Sides::Both);
    } else {
        // The fuel, or while the last of it burns away, embers.
        let (item, count) = match (f.fuel, lit) {
            (Some(s), _) => (s.item, s.count),
            (None, true) => (COAL, 1),
            (None, false) => (COAL, 0),
        };
        let burning = lit && item != BUCKET;
        for i in (0..pile_stage(count)).rev() {
            let turn = yaw + ((seed + i as i32 * 37) % 13) as f32 * 0.1 - 0.6;
            let tilt = [0.0, -0.12, 0.14, 0.2, -0.18, 0.1][i];
            // Glowing embers, flickering.
            let heat = if burning {
                0.6 + 0.25 * ((time * 7.0 + i as f32 * 1.7 + seed as f32).sin() * 0.5 + 0.5)
            } else {
                0.0
            };
            let at = spot(FIRE_HEAP[i], fire_y);
            emit_piece(out, at, turn, item, 0.9, heat, tilt, light);
        }
    }
    if lit && flame_planes {
        // Flames over the fuel.
        let n = match f.fuel.map_or(1, |s| pile_stage(s.count)) {
            0 | 1 => 2,
            3 => 3,
            _ => 4,
        };
        for i in 0..n {
            // Low and near the front: seen through the slot from above.
            let a = (i as f32 + 0.5) / n as f32 * 0.6 - 0.3;
            let h = 0.15 + 0.04 * ((i * 7 + seed as usize) % 3) as f32;
            let at = spot((a, 0.4 + 0.04 * (i % 2) as f32, 0.0), fire_y + 0.02);
            let m = Mat4::from_translation(at)
                * Mat4::from_rotation_y(yaw + 0.4 * i as f32)
                * Mat4::from_scale(Vec3::new(0.24, h, 0.24));
            let phase = (seed as u8).wrapping_add(i as u8 * 53);
            crate::model::emit_flame(out, m, flags::ENTITY, phase);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cells_are_found_where_they_are_drawn() {
        let q = IVec3::new(4, 60, -7);
        for facing in 0..4 {
            for side in -1..=1 {
                for i in 0..27 {
                    let c = chest_cell(q, facing, side, i);
                    assert_eq!(chest_cell_at(q, facing, side, c), Some(i));
                }
            }
            for i in 0..9 {
                assert_eq!(table_cell_at(q, facing, table_cell(q, facing, i)), Some(i));
            }
        }
    }
}
