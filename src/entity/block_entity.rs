//! Block entities: furnaces (smelting), chests and crafting tables with their contents, and
//! their animated parts: chest lids and the items lying on crafting tables.

use crate::item::inventory::take;
use crate::item::{fuel_time, icon, smelt, Icon, Slot, Stack, BUCKET, LAVA_BUCKET};
use crate::model::emit_held;
use crate::util::vertex_light;
use crate::world::mesh::{box_uv, corner_pos, flags, Vertex, CORNERS, FACE_N};
use crate::world::textures::tex;
use crate::world::*;
use glam::{IVec3, Mat4, Vec3};
use std::f32::consts::{FRAC_PI_2, PI};
pub const SMELT_TIME: f32 = 10.0;

#[derive(Default, Clone)]
pub struct Furnace {
    pub input: Slot,
    pub fuel: Slot,
    pub output: Slot,
    /// Seconds of fuel left.
    pub burn: f32,
    pub burn_total: f32,
    /// Seconds spent smelting the current item.
    pub cook: f32,
}

impl Furnace {
    fn can_smelt(&self) -> bool {
        let Some(out) = self.input.and_then(|s| smelt(s.item)) else {
            return false;
        };
        match self.output {
            None => true,
            Some(o) => o.item == out && o.count < crate::item::max_stack(out),
        }
    }

    /// Advances the furnace. Returns true while it is burning.
    pub fn update(&mut self, dt: f32) -> bool {
        let smeltable = self.can_smelt();
        if self.burn <= 0.0 && smeltable {
            if let Some(f) = self.fuel.and_then(|s| fuel_time(s.item).map(|t| (s, t))) {
                self.burn = f.1;
                self.burn_total = f.1;
                if f.0.item == LAVA_BUCKET {
                    self.fuel = Some(Stack::one(BUCKET));
                } else {
                    take(&mut self.fuel, 1);
                }
            }
        }
        if self.burn > 0.0 {
            self.burn -= dt;
            if smeltable {
                self.cook += dt;
                if self.cook >= SMELT_TIME {
                    self.cook = 0.0;
                    let out = smelt(self.input.unwrap().item).unwrap();
                    take(&mut self.input, 1);
                    match &mut self.output {
                        Some(o) => o.count += 1,
                        None => self.output = Some(Stack::one(out)),
                    }
                }
            } else {
                self.cook = 0.0;
            }
            true
        } else {
            self.cook = (self.cook - dt * 2.0).max(0.0);
            false
        }
    }
}

/// Block entities that hold items.
#[derive(Default)]
pub struct BlockEntities {
    pub furnaces: FastMap<IVec3, Furnace>,
    pub chests: FastMap<IVec3, Box<[Slot; 27]>>,
    /// Crafting tables keep whatever is left in their 3x3 grid.
    pub tables: FastMap<IVec3, [Slot; 9]>,
}

impl BlockEntities {
    /// Removes the block entity at `p`, returning its contents.
    pub fn remove(&mut self, p: IVec3) -> Vec<Stack> {
        let mut out = Vec::new();
        if let Some(f) = self.furnaces.remove(&p) {
            out.extend([f.input, f.fuel, f.output].into_iter().flatten());
        }
        if let Some(c) = self.chests.remove(&p) {
            out.extend(c.iter().flatten().copied());
        }
        if let Some(t) = self.tables.remove(&p) {
            out.extend(t.iter().flatten().copied());
        }
        out
    }
}

/// Box in a block-local frame (0..1) with model UVs, transformed by `m` into the world.
fn emit_part(out: &mut Vec<Vertex>, m: Mat4, lo: Vec3, hi: Vec3, layers: [u32; 6], light: [u8; 4]) {
    for (face, &layer) in layers.iter().enumerate() {
        let mut quad = [Vertex::default(); 4];
        for (i, &(su, sv)) in CORNERS.iter().enumerate() {
            let c = corner_pos(face, su, sv);
            let local = lo + (hi - lo) * Vec3::from(c);
            quad[i] = Vertex {
                pos: m.transform_point3(local).to_array(),
                uv: box_uv(face, local.to_array()),
                layer: layer as f32,
                light: [light[0], light[1], light[2], face as u8],
                tint: [255, 255, 255, flags::ENTITY],
            };
        }
        out.extend_from_slice(&[quad[0], quad[1], quad[2], quad[0], quad[2], quad[3]]);
    }
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
pub fn build_door(out: &mut Vec<Vertex>, p: IVec3, b: u8, open: f32, sky: u8, blk: u8) {
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
        let mut quad = [Vertex::default(); 4];
        for (i, &(su, sv)) in CORNERS.iter().enumerate() {
            let k = Vec3::from(corner_pos(face, su, sv));
            let local = lo + (hi - lo) * k;
            let mut uv = box_uv(face, local.to_array());
            if face >= 4 {
                // Both sides show the hinges (the texture's left edge) at the hinge.
                uv[0] = if flip { local.x } else { 1.0 - local.x };
            }
            quad[i] = Vertex {
                pos: to_world(local).to_array(),
                uv,
                layer: layer as f32,
                light: [light[0], light[1], light[2], shade_face as u8],
                tint: [255, 255, 255, flags::ENTITY],
            };
        }
        out.extend_from_slice(&[quad[0], quad[1], quad[2], quad[0], quad[2], quad[3]]);
    }
}

/// Items left in a crafting table grid, lying on its top in a 3x3 layout.
pub fn build_table_items(out: &mut Vec<Vertex>, p: IVec3, grid: &[Slot; 9], sky: u8, blk: u8) {
    // Spacing of the 3x3 grid drawn on the table's top (about 3.3 of 16 pixels).
    const CELL: f32 = 0.207;
    let light = vertex_light(sky, blk);
    for (i, st) in grid.iter().enumerate() {
        let Some(st) = st else { continue };
        let cell = Vec3::new(
            ((i % 3) as f32 - 1.0) * CELL,
            0.0,
            ((i / 3) as f32 - 1.0) * CELL,
        );
        let c = p.as_vec3() + Vec3::new(0.5, 1.0, 0.5) + cell;
        // A little turn per slot so it looks placed by hand.
        let turn = ((p.x * 31 + p.z * 17 + i as i32 * 7).rem_euclid(9)) as f32 * 0.07 - 0.28;
        let m = match icon(st.item) {
            Icon::Block(_) => {
                Mat4::from_translation(c + Vec3::Y * 0.075)
                    * Mat4::from_rotation_y(turn)
                    * Mat4::from_scale(Vec3::splat(0.15))
            }
            Icon::Flat(_) => {
                Mat4::from_translation(c + Vec3::Y * 0.01)
                    * Mat4::from_rotation_y(turn)
                    * Mat4::from_rotation_x(-FRAC_PI_2)
                    * Mat4::from_scale(Vec3::splat(0.2))
            }
        };
        emit_held(out, m, st.item, light, flags::ENTITY);
    }
}
