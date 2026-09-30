//! Blocks that are not whole cubes: torches, lanterns, chests, furnaces and their
//! chimneys, stairs, beds and plants.

use super::*;
use glam::{Mat4, Quat, Vec3};

/// Height of the top of a chest's base, drawn as its inside (what is in it lies there).
pub const CHEST_FLOOR: f32 = 10.0 / 16.0;

/// The hollows behind a furnace's front openings: (bottom, top, depth) of the mouth above
/// and the firebox below, and their half width. A little bigger than the openings in the
/// texture, so their edges stay hidden behind the front.
pub const FURNACE_HOLLOWS: &[(f32, f32, f32)] = &[(0.53, 0.84, 0.5), (0.02, 0.31, 0.45)];
pub const FURNACE_HOLLOW_HALF: f32 = 0.39;

/// Double chest half: a face with an edge on the other half (`dir`, the unit offset toward
/// it) uses the texture without the frame on that edge.
pub fn chest_open_layer(layer: u32, face: usize, dir: [i32; 3]) -> u32 {
    let i = match layer {
        tex::CHEST_FRONT => 0,
        tex::CHEST_SIDE => 1,
        tex::CHEST_TOP => 2,
        tex::CHEST_INSIDE => 3,
        _ => return layer,
    };
    let dot = |a: [i32; 3]| a[0] * dir[0] + a[1] * dir[1] + a[2] * dir[2];
    // Texture v runs against FACE_V, so its +V edge is the top row.
    let edge = match (dot(FACE_U[face]), dot(FACE_V[face])) {
        (1, _) => 0,
        (-1, _) => 1,
        (_, 1) => 2,
        (_, -1) => 3,
        _ => return layer,
    };
    tex::CHEST_OPEN + i * 4 + edge
}

/// A horizontal direction as on a bed facing north (turned back from facing `f`).
pub fn bed_local(d: glam::IVec3, f: u8) -> glam::IVec3 {
    let (x, z) = bed_local_f(d.x as f32, d.z as f32, f);
    glam::IVec3::new(x.round() as i32, d.y, z.round() as i32)
}

/// `bed_local` for a point (x, z) relative to the block center.
pub fn bed_local_f(mut x: f32, mut z: f32, f: u8) -> (f32, f32) {
    for _ in 0..(f & 3) {
        (x, z) = (z, -x);
    }
    (x, z)
}

/// Model transform of a placed torch (`block_center` = center of the block's bottom face).
/// In the torch model the stick runs from y -0.34 to 0.15 and the glowing tip ends at 0.19.
pub fn torch_transform(block_center: Vec3, kind: Block) -> Mat4 {
    if kind == TORCH {
        return Mat4::from_translation(block_center + Vec3::Y * 0.34);
    }
    let outward = -torch_support_offset(kind)
        .expect("wall torch direction")
        .as_vec3();
    // Minecraft's wall torch: the bottom of the stick is centered on the wall surface,
    // 3.5/16 up, and the torch leans 22.5 degrees away from the wall. The stick's lower
    // end sinks into the wall, so there is never a gap between them.
    let tilt = 22.5f32.to_radians();
    let axis = Vec3::Y * tilt.cos() + outward * tilt.sin();
    let bottom = block_center - outward * 0.5 + Vec3::Y * (3.5 / 16.0);
    Mat4::from_translation(bottom + axis * 0.34)
        * Mat4::from_quat(Quat::from_rotation_arc(Vec3::Y, axis))
}

impl Builder {
    /// Torch: the same wooden shaft and animated flame used by held torches.
    pub(super) fn torch(&mut self, r: &Region, x: i32, y: i32, z: i32, kind: Block) {
        let (s, b) = r.light(x, y, z);
        let block_center = Vec3::new(
            (x + self.ox) as f32 + 0.5,
            y as f32,
            (z + self.oz) as f32 + 0.5,
        );
        let transform = torch_transform(block_center, kind);
        let base = self.verts.len() as u32;
        crate::model::emit_torch(
            &mut self.verts,
            transform,
            [255, (s * 17) as u8, (b * 17) as u8, 0],
            0,
            x.wrapping_mul(73).wrapping_add(z.wrapping_mul(151)) as u8,
        );
        // emit_torch ends with two crossed flame planes (24 vertices). Draw
        // those in the blended pass; only the wooden shaft casts a shadow.
        let flame_start = self.verts.len() as u32 - 24;
        self.opaque.extend(base..flame_start);
        self.water.extend(flame_start..self.verts.len() as u32);
    }

    /// Lantern standing on a block or hanging below one (emits its own light).
    pub(super) fn lantern(&mut self, r: &Region, x: i32, y: i32, z: i32, b: Block) {
        let (s, bl) = r.light(x, y, z);
        let m = Mat4::from_translation(Vec3::new(
            (x + self.ox) as f32 + 0.5,
            y as f32,
            (z + self.oz) as f32 + 0.5,
        )) * Mat4::from_scale(Vec3::splat(1.0 / 16.0));
        let kind = if b == LANTERN_HANGING {
            crate::model::lantern::LanternKind::Hanging
        } else {
            crate::model::lantern::LanternKind::Standing
        };
        let base = self.verts.len() as u32;
        crate::model::lantern::emit_lantern(
            &mut self.verts,
            m,
            [255, (s * 17) as u8, (bl * 17) as u8, 0],
            0,
            kind,
        );
        self.opaque.extend(base..self.verts.len() as u32);
    }

    /// One face (`face`: its outward direction) of the box `lo`..`hi` in the block at (x, y,
    /// z), moved to `plane` along its own axis: a wall of a hollow, facing into it. `shade`
    /// darkens it (like the corner shadows: 255 is none). The texture is squeezed toward its
    /// middle by `uv_scale` (1: the block's own spot of it).
    #[allow(clippy::too_many_arguments)]
    fn plane_face(
        &mut self,
        (x, y, z): (i32, i32, i32),
        face: usize,
        lo: [f32; 3],
        hi: [f32; 3],
        plane: f32,
        layer: u32,
        (s, bl): (u32, u32),
        shade: u8,
        uv_scale: f32,
    ) {
        let axis = FACE_N[face].iter().position(|&c| c != 0).unwrap();
        let base = self.verts.len() as u32;
        for &(su, sv) in &CORNERS {
            let c = corner_pos(face, su, sv);
            let mut p: [f32; 3] = std::array::from_fn(|k| lo[k] + (hi[k] - lo[k]) * c[k]);
            p[axis] = plane;
            self.push(Vertex {
                pos: [
                    (x + self.ox) as f32 + p[0],
                    y as f32 + p[1],
                    (z + self.oz) as f32 + p[2],
                ],
                uv: box_uv(face, p).map(|t| 0.5 + (t - 0.5) * uv_scale),
                layer: layer as f32,
                light: [shade, (s * 17) as u8, (bl * 17) as u8, face as u8],
                tint: [255, 255, 255, 0],
            });
        }
        self.opaque
            .extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }

    /// The hollow of the box `lo`..`hi`: its floor, ceiling (if `ceiling`) and walls facing
    /// inward, except the sides in `open` (outward directions: the way into it). `uv_scale`
    /// as in `plane_face`.
    #[allow(clippy::too_many_arguments)]
    fn hollow(
        &mut self,
        at: (i32, i32, i32),
        lo: [f32; 3],
        hi: [f32; 3],
        open: &[[i32; 3]],
        ceiling: bool,
        layer: u32,
        light: (u32, u32),
        uv_scale: f32,
    ) {
        for n in &FACE_N {
            // The wall on side `n` of the hollow faces the other way, into it.
            if open.contains(n) || (n[1] > 0 && !ceiling) {
                continue;
            }
            let inward = FACE_N.iter().position(|m| *m == n.map(|c| -c)).unwrap();
            let axis = n.iter().position(|&c| c != 0).unwrap();
            let plane = if n[axis] > 0 { hi[axis] } else { lo[axis] };
            // Floor lighter than the walls, the ceiling darkest: it looks deep.
            let shade = match n[1] {
                -1 => 235,
                1 => 150,
                _ => 195,
            };
            self.plane_face(at, inward, lo, hi, plane, layer, light, shade, uv_scale);
        }
    }

    /// Chest base; the lid is drawn separately every frame so it can open. A double chest
    /// half reaches the other half, with no wall between them.
    pub(super) fn chest(&mut self, r: &Region, x: i32, y: i32, z: i32, b: Block) {
        let (s, bl) = r.light(x, y, z);
        let (mut lo, mut hi) = (
            [1.0 / 16.0, 0.0, 1.0 / 16.0],
            [15.0 / 16.0, 10.0 / 16.0, 15.0 / 16.0],
        );
        let dir = chest_partner_offset(b).map(|d| d.to_array());
        if let Some(d) = dir {
            for k in [0, 2] {
                match d[k] {
                    1 => hi[k] = 1.0,
                    -1 => lo[k] = 0.0,
                    _ => {}
                }
            }
        }
        for (face, &n) in FACE_N.iter().enumerate() {
            if face == 3 && is_opaque(r.get(x, y - 1, z)) {
                continue;
            }
            if dir == Some(n) {
                continue;
            }
            let layer = if face == 2 {
                tex::CHEST_INSIDE
            } else {
                face_texture(b, face)
            };
            let layer = dir.map_or(layer, |d| chest_open_layer(layer, face, d));
            let base = self.verts.len() as u32;
            for &(su, sv) in &CORNERS {
                let c = corner_pos(face, su, sv);
                let p: [f32; 3] = std::array::from_fn(|k| lo[k] + (hi[k] - lo[k]) * c[k]);
                self.push(Vertex {
                    pos: [
                        (x + self.ox) as f32 + p[0],
                        y as f32 + p[1],
                        (z + self.oz) as f32 + p[2],
                    ],
                    uv: box_uv(face, p),
                    layer: layer as f32,
                    light: [255, (s * 17) as u8, (bl * 17) as u8, face as u8],
                    tint: [255, 255, 255, 0],
                });
            }
            self.opaque
                .extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
        }
    }

    /// Furnace: a cube whose front has two openings (the mouth above, for things to smelt,
    /// and the firebox below), each with a hollow behind it.
    pub(super) fn furnace(&mut self, r: &Region, x: i32, y: i32, z: i32, b: Block) {
        let f = facing(b).unwrap_or(0);
        let front = front_face(f);
        for (face, n) in FACE_N.iter().enumerate() {
            if is_opaque(r.get(x + n[0], y + n[1], z + n[2])) {
                continue;
            }
            let layer = if face == front {
                furnace_front_cut(b)
            } else {
                face_texture(b, face)
            };
            let from = self.opaque.len();
            self.cube_face(r, x, y, z, face, layer, [255; 3], 0, face_rotated(b, face));
            self.to_dir(face, from, layer);
        }
        let d = FACE_N[front];
        if is_opaque(r.get(x + d[0], y + d[1], z + d[2])) {
            return;
        }
        // Lit from the front, and warmly by the fire while it burns.
        let (ls, lb) = r.light(x + d[0], y + d[1], z + d[2]);
        let lit = is_lit_furnace(b);
        let light = (ls, if lit { lb.max(13) } else { lb });
        let across = if d[0] != 0 { 2 } else { 0 };
        let along = 2 - across;
        for &(y0, y1, depth) in FURNACE_HOLLOWS {
            let mut lo = [0.0; 3];
            let mut hi = [0.0; 3];
            lo[across] = 0.5 - FURNACE_HOLLOW_HALF;
            hi[across] = 0.5 + FURNACE_HOLLOW_HALF;
            (lo[1], hi[1]) = (y0, y1);
            if d[along] > 0 {
                (lo[along], hi[along]) = (1.0 - depth, 1.0);
            } else {
                (lo[along], hi[along]) = (0.0, depth);
            }
            let inside = tex::FURNACE_INSIDE;
            self.hollow((x, y, z), lo, hi, &[d], true, inside, light, 1.0);
        }
    }

    /// Blast furnace chimney (`CHIMNEY_BOXES`): the slab, the stack on it and the rim round
    /// its top, without the faces hidden under each other or against solid neighbours.
    pub(super) fn chimney(&mut self, r: &Region, x: i32, y: i32, z: i32, b: Block) {
        let (s, bl) = r.light(x, y, z);
        for (i, &(lo, hi)) in CHIMNEY_BOXES.iter().enumerate() {
            for (face, &n) in FACE_N.iter().enumerate() {
                let hidden = match (i, face) {
                    (0, 2) => false,
                    (0, _) => is_opaque(r.get(x + n[0], y + n[1], z + n[2])),
                    // The stack's ends lie against the slab and the rim.
                    (1, 2 | 3) => true,
                    _ => false,
                };
                if hidden {
                    continue;
                }
                let layer = face_texture(b, face);
                let base = self.verts.len() as u32;
                for &(su, sv) in &CORNERS {
                    let c = corner_pos(face, su, sv);
                    let p: [f32; 3] = std::array::from_fn(|k| lo[k] + (hi[k] - lo[k]) * c[k]);
                    self.push(Vertex {
                        pos: [
                            (x + self.ox) as f32 + p[0],
                            y as f32 + p[1],
                            (z + self.oz) as f32 + p[2],
                        ],
                        uv: box_uv(face, p),
                        layer: layer as f32,
                        light: [255, (s * 17) as u8, (bl * 17) as u8, face as u8],
                        tint: [255, 255, 255, 0],
                    });
                }
                self.opaque
                    .extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
            }
        }
    }

    /// Stairs: the filled eighths of the block, without the faces between them or against
    /// solid neighbours.
    pub(super) fn stairs(&mut self, r: &Region, x: i32, y: i32, z: i32, b: Block) {
        let octants = |x: i32, y: i32, z: i32, b: Block| {
            stairs_octants(b, |d| r.get(x + d.x, y + d.y, z + d.z))
        };
        let bits = octants(x, y, z, b);
        let filled = |bits: u8, o: [i32; 3]| bits & (1 << (o[0] + 2 * o[2] + 4 * o[1])) != 0;
        for i in 0..8 {
            let o = [i & 1, i >> 2, (i >> 1) & 1];
            if !filled(bits, o) {
                continue;
            }
            for (face, n) in FACE_N.iter().enumerate() {
                let q: [i32; 3] = std::array::from_fn(|k| o[k] + n[k]);
                let inside = q.iter().all(|&c| (0..2).contains(&c));
                let (lx, ly, lz) = if inside {
                    if filled(bits, q) {
                        continue;
                    }
                    (x, y, z)
                } else {
                    let (nx, ny, nz) = (x + n[0], y + n[1], z + n[2]);
                    let nb = r.get(nx, ny, nz);
                    if is_opaque(nb) {
                        continue;
                    }
                    if is_stairs(nb) {
                        let wrapped = q.map(|c| c.rem_euclid(2));
                        if filled(octants(nx, ny, nz, nb), wrapped) {
                            continue;
                        }
                    }
                    (nx, ny, nz)
                };
                let (s, bl) = r.light(lx, ly, lz);
                let lo = o.map(|c| c as f32 * 0.5);
                let base = self.verts.len() as u32;
                for &(su, sv) in &CORNERS {
                    let c = corner_pos(face, su, sv);
                    let p: [f32; 3] = std::array::from_fn(|k| lo[k] + 0.5 * c[k]);
                    self.push(Vertex {
                        pos: [
                            (x + self.ox) as f32 + p[0],
                            y as f32 + p[1],
                            (z + self.oz) as f32 + p[2],
                        ],
                        uv: box_uv(face, p),
                        layer: face_texture(b, face) as f32,
                        light: [255, (s * 17) as u8, (bl * 17) as u8, face as u8],
                        tint: [255, 255, 255, 0],
                    });
                }
                self.opaque
                    .extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
            }
        }
    }

    /// Bed half: a box 9/16 high with the pack's face textures (the legs are cut out of the
    /// sides) and the bottom at 3/16, turned toward the bed's facing. The faces between the
    /// halves are left out.
    pub(super) fn bed(&mut self, r: &Region, x: i32, y: i32, z: i32, b: Block) {
        let (s, bl) = r.light(x, y, z);
        let f = bed_facing(b);
        let head = bed_head(b);
        for (face, &n) in FACE_N.iter().enumerate() {
            // The texture as on a bed whose head points north: turn the face's direction back.
            let d = glam::IVec3::from(n);
            let local = bed_local(d, f);
            let layer = match (face, local.x, local.z) {
                (2, ..) if head => tex::BED_HEAD_TOP,
                (2, ..) => tex::BED_FOOT_TOP,
                (3, ..) => tex::BED_BOTTOM,
                (_, 1, _) if head => tex::BED_HEAD_EAST,
                (_, 1, _) => tex::BED_FOOT_EAST,
                (_, -1, _) if head => tex::BED_HEAD_WEST,
                (_, -1, _) => tex::BED_FOOT_WEST,
                (_, _, -1) if head => tex::BED_HEAD_END,
                (_, _, 1) if !head => tex::BED_FOOT_END,
                _ => continue, // toward the other half
            };
            // The top and bottom are inside the block: always drawn.
            if face != 2 && face != 3 && is_opaque(r.get(x + n[0], y + n[1], z + n[2])) {
                continue;
            }
            let (lo, hi) = ([0.0, 0.0, 0.0], [1.0, BED_HEIGHT, 1.0]);
            let bottom_y = 3.0 / 16.0;
            let base = self.verts.len() as u32;
            for &(su, sv) in &CORNERS {
                let c = corner_pos(face, su, sv);
                let mut p: [f32; 3] = std::array::from_fn(|k| lo[k] + (hi[k] - lo[k]) * c[k]);
                if face == 3 {
                    p[1] = bottom_y;
                }
                let uv = if face == 2 || face == 3 {
                    // Top and bottom: the texture's top edge toward the head.
                    let l = bed_local_f(p[0] - 0.5, p[2] - 0.5, f);
                    [l.0 + 0.5, l.1 + 0.5]
                } else {
                    box_uv(face, p)
                };
                // A texel in from the edges: the faces do not tile, so filtering must not
                // wrap around to the opposite edge (the pillow's white would line the seam).
                let inset = 1.0 / 128.0;
                let uv = uv.map(|c| c.clamp(inset, 1.0 - inset));
                self.push(Vertex {
                    pos: [
                        (x + self.ox) as f32 + p[0],
                        y as f32 + p[1],
                        (z + self.oz) as f32 + p[2],
                    ],
                    uv,
                    layer: layer as f32,
                    light: [255, (s * 17) as u8, (bl * 17) as u8, face as u8],
                    tint: [255, 255, 255, 0],
                });
            }
            self.opaque
                .extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
        }
    }

    pub(super) fn plant(&mut self, r: &Region, x: i32, y: i32, z: i32, layer: u32, tint: [u8; 3]) {
        let (s, b) = r.light(x, y, z);
        let light = [255, (s * 17) as u8, (b * 17) as u8, 6];
        let (wx, wz) = ((x + self.ox) as f32, (z + self.oz) as f32);
        let i = 0.15;
        let quads = [[(i, i), (1.0 - i, 1.0 - i)], [(1.0 - i, i), (i, 1.0 - i)]];
        for q in quads {
            let base = self.verts.len() as u32;
            let (a, bq) = (q[0], q[1]);
            let pts = [
                ([wx + a.0, y as f32, wz + a.1], [0.0, 1.0]),
                ([wx + bq.0, y as f32, wz + bq.1], [1.0, 1.0]),
                ([wx + bq.0, y as f32 + 1.0, wz + bq.1], [1.0, 0.0]),
                ([wx + a.0, y as f32 + 1.0, wz + a.1], [0.0, 0.0]),
            ];
            for (pos, uv) in pts {
                self.push(Vertex {
                    pos,
                    uv,
                    layer: layer as f32,
                    light,
                    tint: [tint[0], tint[1], tint[2], flags::PLANT],
                });
            }
            self.opaque
                .extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
            self.opaque
                .extend_from_slice(&[base, base + 2, base + 1, base, base + 3, base + 2]);
        }
    }
}
