//! Chunk meshing. Each job copies the 3x3 chunk neighbourhood into a flat region,
//! flood-fills sky and block light through it, then emits faces for the center chunk
//! with smooth lighting + ambient occlusion.

use super::gen::Generator;
use super::textures::tex;
use super::*;
use glam::{Mat4, Quat, Vec3};
use std::collections::VecDeque;
use std::sync::Arc;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct Vertex {
    pub pos: [f32; 3],
    pub uv: [f32; 2],
    pub layer: f32,
    /// ao (0..255), sky light, block light, normal index
    pub light: [u8; 4],
    /// rgb tint, flags
    pub tint: [u8; 4],
}

pub mod flags {
    pub const LEAVES: u8 = 1;
    pub const PLANT: u8 = 2;
    pub const EMISSIVE: u8 = 4;
    pub const WATER: u8 = 8;
    pub const OVERLAY: u8 = 16;
    pub const VIEWMODEL: u8 = 32;
    pub const ENTITY: u8 = 64;
    pub const FLUID: u8 = 128;
}

pub struct MeshData {
    pub pos: ChunkPos,
    pub vertices: Vec<Vertex>,
    /// Opaque indices first, then translucent (water) indices.
    pub indices: Vec<u32>,
    pub opaque_count: u32,
    pub min_y: f32,
    pub max_y: f32,
}

// Faces: 0 +X, 1 -X, 2 +Y, 3 -Y, 4 +Z, 5 -Z. u x v = n, so corners are CCW seen from outside.
pub const FACE_N: [[i32; 3]; 6] = [
    [1, 0, 0],
    [-1, 0, 0],
    [0, 1, 0],
    [0, -1, 0],
    [0, 0, 1],
    [0, 0, -1],
];
pub const FACE_U: [[i32; 3]; 6] = [
    [0, 0, -1],
    [0, 0, 1],
    [1, 0, 0],
    [1, 0, 0],
    [1, 0, 0],
    [-1, 0, 0],
];
pub const FACE_V: [[i32; 3]; 6] = [
    [0, 1, 0],
    [0, 1, 0],
    [0, 0, -1],
    [0, 0, 1],
    [0, 1, 0],
    [0, 1, 0],
];
pub const CORNERS: [(i32, i32); 4] = [(-1, -1), (1, -1), (1, 1), (-1, 1)];

/// Unit-cube corner (0..1) for a face corner.
pub fn corner_pos(face: usize, su: i32, sv: i32) -> [f32; 3] {
    let (n, u, v) = (FACE_N[face], FACE_U[face], FACE_V[face]);
    std::array::from_fn(|k| 0.5 + 0.5 * (n[k] + su * u[k] + sv * v[k]) as f32)
}

pub fn corner_uv(su: i32, sv: i32) -> [f32; 2] {
    [(su + 1) as f32 * 0.5, 1.0 - (sv + 1) as f32 * 0.5]
}

/// Texture coordinates for a point `p` (0..1 in the block) on a face, so parts smaller than a
/// block show the matching piece of the texture (Minecraft-style model UVs).
pub fn box_uv(face: usize, p: [f32; 3]) -> [f32; 2] {
    let along = |a: [i32; 3]| {
        let k = a.iter().position(|&c| c != 0).unwrap();
        if a[k] > 0 {
            p[k]
        } else {
            1.0 - p[k]
        }
    };
    [along(FACE_U[face]), 1.0 - along(FACE_V[face])]
}

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

const RW: usize = 48;

struct Region {
    blocks: Vec<u8>,
    sky: Vec<u8>,
    blk: Vec<u8>,
    h: usize,
    hm: Vec<i32>,
    /// Region coords -> (previous block, change time) for recently changed fluids.
    old: FastMap<(i32, i32, i32), (u8, f32)>,
}

fn propagate(levels: &mut [u8], blocks: &[u8], h: usize, q: &mut VecDeque<u32>) {
    let layer = RW * RW;
    while let Some(i) = q.pop_front() {
        let i = i as usize;
        let l = levels[i];
        if l <= 1 {
            continue;
        }
        let nl = l - 1;
        let x = i % RW;
        let z = (i / RW) % RW;
        let y = i / layer;
        let mut visit = |j: usize, q: &mut VecDeque<u32>| {
            if levels[j] < nl && !is_opaque(blocks[j]) {
                levels[j] = nl;
                q.push_back(j as u32);
            }
        };
        if x > 0 {
            visit(i - 1, q);
        }
        if x + 1 < RW {
            visit(i + 1, q);
        }
        if z > 0 {
            visit(i - RW, q);
        }
        if z + 1 < RW {
            visit(i + RW, q);
        }
        if y > 0 {
            visit(i - layer, q);
        }
        if y + 1 < h {
            visit(i + layer, q);
        }
    }
}

impl Region {
    fn new(nb: &[Arc<ChunkData>; 9]) -> Self {
        let h = nb
            .iter()
            .map(|c| c.max_y as usize + 2)
            .max()
            .unwrap()
            .min(HEIGHT);
        let mut blocks = vec![AIR; RW * RW * h];
        let mut hm = vec![0i32; RW * RW];
        for (i, c) in nb.iter().enumerate() {
            let (ox, oz) = ((i % 3) * 16, (i / 3) * 16);
            let top = (c.max_y as usize + 1).min(h);
            for y in 0..top {
                for z in 0..16 {
                    let dst = (y * RW + oz + z) * RW + ox;
                    blocks[dst..dst + 16].copy_from_slice(c.row(y, z));
                }
            }
            for z in 0..16 {
                for x in 0..16 {
                    hm[(oz + z) * RW + ox + x] = c.heightmap[z * 16 + x] as i32;
                }
            }
        }
        Self {
            blocks,
            sky: Vec::new(),
            blk: Vec::new(),
            h,
            hm,
            old: FastMap::default(),
        }
    }

    #[inline]
    fn idx(&self, x: usize, y: usize, z: usize) -> usize {
        (y * RW + z) * RW + x
    }

    #[inline]
    fn get(&self, x: i32, y: i32, z: i32) -> u8 {
        if y < 0 {
            return BEDROCK;
        }
        if y >= self.h as i32 || x < 0 || z < 0 || x >= RW as i32 || z >= RW as i32 {
            return AIR;
        }
        self.blocks[self.idx(x as usize, y as usize, z as usize)]
    }

    #[inline]
    fn light(&self, x: i32, y: i32, z: i32) -> (u32, u32) {
        if y >= self.h as i32 {
            return (15, 0);
        }
        if y < 0 || x < 0 || z < 0 || x >= RW as i32 || z >= RW as i32 {
            return (0, 0);
        }
        let i = self.idx(x as usize, y as usize, z as usize);
        (self.sky[i] as u32, self.blk[i] as u32)
    }

    fn compute_light(&mut self) {
        let n = self.blocks.len();
        let mut sky = vec![0u8; n];
        let mut q = VecDeque::new();
        for z in 0..RW {
            for x in 0..RW {
                let top = self.hm[z * RW + x];
                let start = (top + 1).max(0) as usize;
                for y in start..self.h {
                    sky[self.idx(x, y, z)] = 15;
                }
                let mut maxn = top;
                if x > 0 {
                    maxn = maxn.max(self.hm[z * RW + x - 1]);
                }
                if x + 1 < RW {
                    maxn = maxn.max(self.hm[z * RW + x + 1]);
                }
                if z > 0 {
                    maxn = maxn.max(self.hm[(z - 1) * RW + x]);
                }
                if z + 1 < RW {
                    maxn = maxn.max(self.hm[(z + 1) * RW + x]);
                }
                let end = ((maxn + 1) as usize).min(self.h.saturating_sub(1));
                for y in start..=end {
                    q.push_back(self.idx(x, y, z) as u32);
                }
            }
        }
        propagate(&mut sky, &self.blocks, self.h, &mut q);

        let mut blk = vec![0u8; n];
        for (i, &b) in self.blocks.iter().enumerate() {
            let e = emission(b);
            if e > 0 {
                blk[i] = e;
                q.push_back(i as u32);
            }
        }
        propagate(&mut blk, &self.blocks, self.h, &mut q);
        self.sky = sky;
        self.blk = blk;
    }
}

/// Model transform of a placed torch (`block_center` = center of the block's bottom face).
/// In the torch model the stick runs from y -0.34 to 0.15 and the glowing tip ends at 0.19.
pub fn torch_transform(block_center: Vec3, kind: u8) -> Mat4 {
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

/// Which glass neighbours a glass face joins with (connected textures): bits 0 -u, 1 +u,
/// 2 +v, 3 -v, then the corners (-u,+v), (+u,+v), (-u,-v), (+u,-v). A neighbour only joins
/// if its own face in this direction is visible too.
fn glass_mask(r: &Region, x: i32, y: i32, z: i32, face: usize) -> u8 {
    let (n, u, v) = (FACE_N[face], FACE_U[face], FACE_V[face]);
    let joins = |du: i32, dv: i32| {
        let p = [
            x + du * u[0] + dv * v[0],
            y + du * u[1] + dv * v[1],
            z + du * u[2] + dv * v[2],
        ];
        r.get(p[0], p[1], p[2]) == GLASS && r.get(p[0] + n[0], p[1] + n[1], p[2] + n[2]) != GLASS
    };
    let dirs = [
        (-1, 0),
        (1, 0),
        (0, 1),
        (0, -1),
        (-1, 1),
        (1, 1),
        (-1, -1),
        (1, -1),
    ];
    dirs.iter()
        .enumerate()
        .filter(|(_, &(du, dv))| joins(du, dv))
        .fold(0u8, |m, (bit, _)| m | 1 << bit)
}

#[inline]
fn occludes(b: u8) -> bool {
    is_opaque(b) || is_leaves(b)
}

struct Builder {
    verts: Vec<Vertex>,
    opaque: Vec<u32>,
    water: Vec<u32>,
    min_y: f32,
    max_y: f32,
    ox: i32,
    oz: i32,
}

impl Builder {
    #[inline]
    fn push(&mut self, v: Vertex) {
        self.min_y = self.min_y.min(v.pos[1]);
        self.max_y = self.max_y.max(v.pos[1]);
        self.verts.push(v);
    }

    #[allow(clippy::too_many_arguments)]
    fn cube_face(
        &mut self,
        r: &Region,
        x: i32,
        y: i32,
        z: i32,
        face: usize,
        layer: u32,
        tint: [u8; 3],
        fl: u8,
    ) {
        let (n, u, v) = (FACE_N[face], FACE_U[face], FACE_V[face]);
        let (fx, fy, fz) = (x + n[0], y + n[1], z + n[2]);
        let base = self.verts.len() as u32;
        let flame_seed = if layer == tex::FURNACE_FRONT_LIT {
            (x + self.ox)
                .wrapping_mul(73)
                .wrapping_add((z + self.oz).wrapping_mul(151)) as u8
        } else {
            tint[0]
        };
        let mut ao = [3u8; 4];
        for (i, &(su, sv)) in CORNERS.iter().enumerate() {
            let s1 = (fx + su * u[0], fy + su * u[1], fz + su * u[2]);
            let s2 = (fx + sv * v[0], fy + sv * v[1], fz + sv * v[2]);
            let c = (s1.0 + sv * v[0], s1.1 + sv * v[1], s1.2 + sv * v[2]);
            let (b1, b2, bc) = (
                r.get(s1.0, s1.1, s1.2),
                r.get(s2.0, s2.1, s2.2),
                r.get(c.0, c.1, c.2),
            );
            let (o1, o2, oc) = (occludes(b1), occludes(b2), occludes(bc));
            ao[i] = if o1 && o2 {
                0
            } else {
                3 - (o1 as u8 + o2 as u8 + oc as u8)
            };

            let (mut ss, mut sb) = r.light(fx, fy, fz);
            let mut cnt = 1;
            if !is_opaque(b1) {
                let l = r.light(s1.0, s1.1, s1.2);
                ss += l.0;
                sb += l.1;
                cnt += 1;
            }
            if !is_opaque(b2) {
                let l = r.light(s2.0, s2.1, s2.2);
                ss += l.0;
                sb += l.1;
                cnt += 1;
            }
            if !is_opaque(bc) && !(is_opaque(b1) && is_opaque(b2)) {
                let l = r.light(c.0, c.1, c.2);
                ss += l.0;
                sb += l.1;
                cnt += 1;
            }
            let p = corner_pos(face, su, sv);
            self.push(Vertex {
                pos: [
                    (x + self.ox) as f32 + p[0],
                    y as f32 + p[1],
                    (z + self.oz) as f32 + p[2],
                ],
                uv: corner_uv(su, sv),
                layer: layer as f32,
                light: [
                    ao[i] * 85,
                    (ss * 17 / cnt) as u8,
                    (sb * 17 / cnt) as u8,
                    face as u8,
                ],
                tint: [flame_seed, tint[1], tint[2], fl],
            });
        }
        let idx = &mut self.opaque;
        if ao[0] as u32 + ao[2] as u32 > ao[1] as u32 + ao[3] as u32 {
            idx.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
        } else {
            idx.extend_from_slice(&[base + 1, base + 2, base + 3, base + 1, base + 3, base]);
        }
    }

    /// Torch: the same wooden shaft and animated flame used by held torches.
    fn torch(&mut self, r: &Region, x: i32, y: i32, z: i32, kind: u8) {
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
    fn lantern(&mut self, r: &Region, x: i32, y: i32, z: i32, b: u8) {
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

    /// Chest base; the lid is drawn separately every frame so it can open. A double chest
    /// half reaches the other half, with no wall between them.
    fn chest(&mut self, r: &Region, x: i32, y: i32, z: i32, b: u8) {
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

    fn plant(&mut self, r: &Region, x: i32, y: i32, z: i32, layer: u32, tint: [u8; 3]) {
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

    /// Fluid block. Vertices carry the previous surface height and the change time in `uv`
    /// (the shader animates between them and derives texture coordinates from position),
    /// and the flow direction in `tint`.
    fn fluid(&mut self, r: &Region, x: i32, y: i32, z: i32, b: u8) {
        let lava = is_lava(b);
        let same = |q: u8| if lava { is_lava(q) } else { is_water(q) };
        let above_same = same(r.get(x, y + 1, z));
        let now_get = |x: i32, y: i32, z: i32| r.get(x, y, z);
        let h = surface_heights(&now_get, x, y, z, lava);

        // Previous state, if anything around this block changed recently.
        let mut start = -1.0e6f32;
        for dy in 0..=1 {
            for dz in -1..=1 {
                for dx in -1..=1 {
                    if let Some(&(_, t)) = r.old.get(&(x + dx, y + dy, z + dz)) {
                        start = start.max(t);
                    }
                }
            }
        }
        // Only changes from that latest tick are "in progress"; earlier ones already finished animating.
        let recent = |x: i32, y: i32, z: i32| {
            r.old
                .get(&(x, y, z))
                .filter(|o| o.1 > start - 0.01)
                .map(|o| o.0)
        };
        let h_old = if start > -1.0e5 {
            let old_get =
                |x: i32, y: i32, z: i32| recent(x, y, z).unwrap_or_else(|| r.get(x, y, z));
            surface_heights(&old_get, x, y, z, lava)
        } else {
            h
        };

        // A block that just filled with fluid grows out of whatever fed it: downward from
        // above, or sideways out of the lowest-level horizontal neighbour.
        let is_new = recent(x, y, z).is_some_and(|o| !same(o));
        let mut feeder: Option<(i32, i32)> = None;
        if is_new && !above_same {
            let mut best = u8::MAX;
            for (dx, dz) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                let q = r.get(x + dx, y, z + dz);
                if same(q) {
                    let l = fluid_level(q);
                    let eff = if l >= FALLING { 0 } else { l };
                    if eff < best {
                        best = eff;
                        feeder = Some((dx, dz));
                    }
                }
            }
        }

        // Flow direction: away from the feeder for new blocks, otherwise downhill.
        let gx = (h[1][0] + h[1][1]) - (h[0][0] + h[0][1]);
        let gz = (h[0][1] + h[1][1]) - (h[0][0] + h[1][0]);
        let len = (gx * gx + gz * gz).sqrt();
        let (fx, fz) = match feeder {
            Some((dx, dz)) => (-dx as f32, -dz as f32),
            None if len > 0.01 => (-gx / len, -gz / len),
            None => (0.0, 0.0),
        };
        let falling = above_same || fluid_level(b) >= FALLING;
        let tint = [
            ((fx * 0.5 + 0.5) * 255.0) as u8,
            ((fz * 0.5 + 0.5) * 255.0) as u8,
            if falling { 255 } else { 0 },
        ];

        let fl = flags::FLUID | if lava { flags::EMISSIVE } else { flags::WATER };
        let layer = if lava { tex::LAVA } else { tex::WATER } as f32;
        let own = r.light(x, y, z);
        for (face, n) in FACE_N.iter().enumerate() {
            let nb = r.get(x + n[0], y + n[1], z + n[2]);
            let visible = if face == 2 {
                !above_same
            } else {
                !same(nb) && !is_opaque(nb)
            };
            if !visible {
                continue;
            }
            let l = r.light(x + n[0], y + n[1], z + n[2]);
            let light = [
                255,
                (l.0.max(own.0) * 17) as u8,
                (l.1.max(own.1) * 17) as u8,
                face as u8,
            ];
            let base = self.verts.len() as u32;
            for &(su, sv) in &CORNERS {
                let p = corner_pos(face, su, sv);
                let (cx, cz) = (p[0] as usize, p[2] as usize);
                let top = p[1] > 0.5;
                let (py, mut old_y) = if top {
                    (h[cx][cz], h_old[cx][cz])
                } else {
                    (0.0, 0.0)
                };
                let mut vlayer = layer;
                if is_new && above_same && !top {
                    // Falling: the column extends downward from the top.
                    old_y = 1.0;
                } else if let (true, Some((dx, dz))) = (is_new, feeder) {
                    // Corners on the far side start on the feeder's edge (the shader slides them
                    // out along the flow; the +0.25 layer offset marks them) at that edge's height.
                    let far = (dx != 0 && cx as i32 != (dx + 1) / 2)
                        || (dz != 0 && cz as i32 != (dz + 1) / 2);
                    if far {
                        vlayer += 0.25;
                        if top {
                            let (mx, mz) = if dx != 0 { (1 - cx, cz) } else { (cx, 1 - cz) };
                            old_y = h_old[mx][mz];
                        }
                    }
                }
                let (wx, wy, wz) = (
                    (x + self.ox) as f32 + p[0],
                    y as f32 + py,
                    (z + self.oz) as f32 + p[2],
                );
                let uv = [y as f32 + old_y, start];
                self.push(Vertex {
                    pos: [wx, wy, wz],
                    uv,
                    layer: vlayer,
                    light,
                    tint: [tint[0], tint[1], tint[2], fl],
                });
            }
            let list = if lava {
                &mut self.opaque
            } else {
                &mut self.water
            };
            list.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
        }
    }
}

fn fluid_height(level: u8) -> f32 {
    if level == 0 || level >= FALLING {
        0.875
    } else {
        (8 - level) as f32 / 9.0
    }
}

/// Surface height (0..1) at the four top corners, indexed [x][z].
fn surface_heights(
    get: &impl Fn(i32, i32, i32) -> u8,
    x: i32,
    y: i32,
    z: i32,
    lava: bool,
) -> [[f32; 2]; 2] {
    let same = |q: u8| if lava { is_lava(q) } else { is_water(q) };
    if same(get(x, y + 1, z)) {
        return [[1.0; 2]; 2];
    }
    std::array::from_fn(|cx| {
        std::array::from_fn(|cz| corner_height(get, x, y, z, cx as i32, cz as i32, lava))
    })
}

fn corner_height(
    get: &impl Fn(i32, i32, i32) -> u8,
    x: i32,
    y: i32,
    z: i32,
    cx: i32,
    cz: i32,
    lava: bool,
) -> f32 {
    let same = |q: u8| if lava { is_lava(q) } else { is_water(q) };
    let (mut sum, mut w) = (0.0f32, 0.0f32);
    for dx in [cx - 1, cx] {
        for dz in [cz - 1, cz] {
            let (bx, bz) = (x + dx, z + dz);
            if same(get(bx, y + 1, bz)) {
                return 1.0;
            }
            let b = get(bx, y, bz);
            if same(b) {
                let l = fluid_level(b);
                let wt = if l == 0 { 10.0 } else { 1.0 };
                sum += fluid_height(l) * wt;
                w += wt;
            } else if !is_solid(b) {
                w += 1.0;
            }
        }
    }
    if w > 0.0 {
        sum / w
    } else {
        0.0
    }
}

pub fn mesh_chunk(
    pos: ChunkPos,
    nb: &[Arc<ChunkData>; 9],
    anim: &[(glam::IVec3, u8, f32)],
    gen: &Generator,
) -> MeshData {
    let mut r = Region::new(nb);
    r.compute_light();
    let (ox, oz) = (pos.0 * 16 - 16, pos.1 * 16 - 16);
    for &(q, b, t) in anim {
        r.old.insert((q.x - ox, q.y, q.z - oz), (b, t));
    }

    let mut grass = [[0u8; 3]; 256];
    let mut foliage = [[0u8; 3]; 256];
    for z in 0..16 {
        for x in 0..16 {
            let (g, f) = gen.tints(pos.0 * 16 + x as i32, pos.1 * 16 + z as i32);
            grass[z * 16 + x] = g;
            foliage[z * 16 + x] = f;
        }
    }

    let mut m = Builder {
        verts: Vec::with_capacity(16_384),
        opaque: Vec::with_capacity(24_576),
        water: Vec::new(),
        min_y: HEIGHT as f32,
        max_y: 0.0,
        ox: pos.0 * 16 - 16,
        oz: pos.1 * 16 - 16,
    };
    let top = (nb[4].max_y as i32 + 1).min(r.h as i32 - 1);
    for y in 0..=top {
        for z in 16..32 {
            for x in 16..32 {
                let b = r.get(x, y, z);
                if b == AIR {
                    continue;
                }
                let col = ((z - 16) * 16 + (x - 16)) as usize;
                if is_plant(b) {
                    let tint = if b == TALL_GRASS {
                        grass[col]
                    } else {
                        [255; 3]
                    };
                    m.plant(&r, x, y, z, face_texture(b, 0), tint);
                    continue;
                }
                if is_fluid(b) {
                    m.fluid(&r, x, y, z, b);
                    continue;
                }
                if is_lantern(b) {
                    m.lantern(&r, x, y, z, b);
                    continue;
                }
                if is_torch(b) {
                    m.torch(&r, x, y, z, b);
                    continue;
                }
                if is_chest(b) {
                    m.chest(&r, x, y, z, b);
                    continue;
                }
                let fl = if is_leaves(b) {
                    flags::LEAVES
                } else if emission(b) > 0 && !is_furnace(b) {
                    flags::EMISSIVE
                } else {
                    0
                };
                for (face, n) in FACE_N.iter().enumerate() {
                    let nbk = r.get(x + n[0], y + n[1], z + n[2]);
                    // Leaves keep their faces toward other leaves too (Minecraft's "Fancy"
                    // leaves), so a canopy looks full instead of a hollow see-through shell.
                    let visible = !is_opaque(nbk)
                        && match b {
                            GLASS => nbk != GLASS,
                            ICE => nbk != ICE,
                            _ => true,
                        };
                    if !visible {
                        continue;
                    }
                    let tint = match tint_kind(b, face) {
                        // Connected glass: the shader reads the (inverted) mask from red.
                        TintKind::None if b == GLASS => {
                            [255 - glass_mask(&r, x, y, z, face), 255, 255]
                        }
                        TintKind::None => [255; 3],
                        TintKind::Grass => grass[col],
                        TintKind::Foliage => foliage[col],
                        TintKind::Spruce => SPRUCE_TINT,
                        TintKind::Birch => BIRCH_TINT,
                    };
                    m.cube_face(&r, x, y, z, face, face_texture(b, face), tint, fl);
                }
            }
        }
    }

    let opaque_count = m.opaque.len() as u32;
    let mut indices = m.opaque;
    indices.extend_from_slice(&m.water);
    MeshData {
        pos,
        vertices: m.verts,
        indices,
        opaque_count,
        min_y: m.min_y,
        max_y: m.max_y,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Prints where meshing time goes (`cargo test --release mesh_speed -- --nocapture`).
    #[test]
    fn mesh_speed() {
        let gen = Generator::new(1201871768);
        let chunks: Vec<Arc<ChunkData>> = (0..25)
            .map(|i| Arc::new(gen.generate_chunk(i % 5, i / 5)))
            .collect();
        let at = |x: i32, z: i32| chunks[(z * 5 + x) as usize].clone();
        let hoods: Vec<[Arc<ChunkData>; 9]> = (1..4)
            .flat_map(|z| (1..4).map(move |x| (x, z)))
            .map(|(x, z)| std::array::from_fn(|i| at(x + i as i32 % 3 - 1, z + i as i32 / 3 - 1)))
            .collect();
        let t = std::time::Instant::now();
        let mut regions: Vec<Region> = hoods.iter().map(Region::new).collect();
        let region = t.elapsed();
        let t = std::time::Instant::now();
        for r in &mut regions {
            r.compute_light();
        }
        let light = t.elapsed();
        let t = std::time::Instant::now();
        for i in 0..9 {
            for z in 0..16 {
                for x in 0..16 {
                    std::hint::black_box(gen.tints(i * 16 + x, z));
                }
            }
        }
        let tints = t.elapsed();
        let t = std::time::Instant::now();
        let mut verts = 0;
        for (i, nb) in hoods.iter().enumerate() {
            let pos = (1 + i as i32 % 3, 1 + i as i32 / 3);
            verts += mesh_chunk(pos, nb, &[], &gen).vertices.len();
        }
        let total = t.elapsed();
        let per = |d: std::time::Duration| d.as_secs_f64() * 1000.0 / 9.0;
        println!(
            "per chunk: total {:.2} ms (region {:.2}, light {:.2}, tints {:.2}), {} vertices",
            per(total),
            per(region),
            per(light),
            per(tints),
            verts / 9
        );
    }

    #[test]
    fn glass_wall_faces_join_their_neighbours() {
        let mut c = ChunkData::new();
        for y in 1..4 {
            for x in 1..4 {
                c.set(x, y, 8, GLASS);
            }
        }
        let c = Arc::new(c);
        let empty = Arc::new(ChunkData::new());
        let nb: [Arc<ChunkData>; 9] =
            std::array::from_fn(|i| if i == 4 { c.clone() } else { empty.clone() });
        let r = Region::new(&nb);
        // Region coordinates: the center chunk starts at 16. Face 4 is +Z (u = +X, v = +Y).
        assert_eq!(
            glass_mask(&r, 18, 2, 24, 4),
            0xFF,
            "middle pane joins all around"
        );
        assert_eq!(
            glass_mask(&r, 17, 1, 24, 4),
            0b0010_0110,
            "bottom-left pane joins right, up and the up-right corner"
        );
        // A second layer in front hides the neighbour's face, so it does not join.
        let mut c2 = (*c).clone();
        c2.set(2, 1, 7, GLASS);
        let nb: [Arc<ChunkData>; 9] = std::array::from_fn(|i| {
            if i == 4 {
                Arc::new(c2.clone())
            } else {
                empty.clone()
            }
        });
        let r = Region::new(&nb);
        // Face 5 is -Z with u = -X: the pane at x+1 is the -u neighbour, now covered in front.
        assert_eq!(glass_mask(&r, 17, 1, 24, 5) & 0b1, 0);
        assert_eq!(
            glass_mask(&r, 17, 1, 24, 4) & 0b10,
            0b10,
            "the +Z side still joins"
        );
    }
}
