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
    /// Opaque indices first, then translucent (water) indices. The opaque ones end with what
    /// far chunks leave out: faces between leaves, then grass and flowers.
    pub indices: Vec<u32>,
    pub opaque_count: u32,
    /// Opaque indices without the faces between leaves and the plants.
    pub solid_count: u32,
    /// Where the solid indices are faces of whole blocks grouped by direction (see `FACE_N`):
    /// the other solid ones (stairs, chests, torches...) come first, then these groups.
    pub dir_counts: [u32; 6],
    /// Indices of the faces between leaves (after the solid ones).
    pub leaf_inner_count: u32,
    pub min_y: f32,
    pub max_y: f32,
    /// Door halves in the chunk (world positions): drawn every frame so they can swing.
    pub doors: Vec<glam::IVec3>,
    /// Chests in the chunk (world positions): their lids are drawn every frame so they can
    /// open, whether or not their contents are known here (a LAN player only gets those
    /// once someone opens them).
    pub chests: Vec<glam::IVec3>,
    /// Gun stations in the chunk (world positions): their Blockbench model is drawn every
    /// frame (its drawer slides out while one is used).
    pub gun_stations: Vec<glam::IVec3>,
    /// Torches and the marks of cut-down trunks in the chunk (world positions): the torches'
    /// fire and the grass growing back over the marks look for them here, not through every
    /// block around.
    pub torches: Vec<glam::IVec3>,
    pub stump_marks: Vec<glam::IVec3>,
    /// The chunk's light (see `ChunkLight`), for things drawn outside chunk meshes.
    pub light: ChunkLight,
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

/// Height of the top of a chest's base, drawn as its inside (what is in it lies there).
pub const CHEST_FLOOR: f32 = 10.0 / 16.0;
/// The hollows behind a furnace's front openings: (bottom, top, depth) of the mouth above
/// and the firebox below, and their half width. A little bigger than the openings in the
/// texture, so their edges stay hidden behind the front.
pub const FURNACE_HOLLOWS: &[(f32, f32, f32)] = &[(0.53, 0.84, 0.5), (0.02, 0.31, 0.45)];
pub const FURNACE_HOLLOW_HALF: f32 = 0.39;

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
    /// Faces between leaves, and grass and flowers: after the opaque ones, so far chunks can
    /// leave them out.
    leaf_inner: Vec<u32>,
    plants: Vec<u32>,
    /// Faces of whole blocks by direction: a chunk's faces turned away from the camera can be
    /// left out as a group.
    dirs: [Vec<u32>; 6],
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
        rotated: bool,
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
                uv: if rotated {
                    let uv = corner_uv(su, sv);
                    [uv[1], 1.0 - uv[0]]
                } else {
                    corner_uv(su, sv)
                },
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

    /// Furnace: a cube whose front has two openings (the mouth above, for things to smelt,
    /// and the firebox below), each with a hollow behind it.
    fn furnace(&mut self, r: &Region, x: i32, y: i32, z: i32, b: u8) {
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
            let quad = self.opaque.drain(from..);
            self.dirs[face].extend(quad);
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
    fn chimney(&mut self, r: &Region, x: i32, y: i32, z: i32, b: u8) {
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

    /// A log or a branch, round: a many-sided cylinder along its axis (logs thick, branches
    /// thin), its bark around it. An end joining the same kind of log goes on into it; one
    /// meeting a log across (a branch out of a trunk, a branch turning up) reaches on into
    /// its middle, so the joint is closed; a free end is capped with the rings.
    fn round_log(&mut self, r: &Region, x: i32, y: i32, z: i32, b: u8) {
        let axis = log_axis(b);
        let radius = log_radius(b);
        let sides = if is_branch(b) { 8 } else { 12 };
        // Block-local position from (along the axis, u, v across it).
        let (u_axis, v_axis) = match axis {
            0 => (2, 1),
            1 => (0, 2),
            _ => (0, 1),
        };
        let at = |t: f32, u: f32, v: f32| {
            let mut p = [0.5f32; 3];
            p[axis] = t;
            p[u_axis] = 0.5 + u;
            p[v_axis] = 0.5 + v;
            p
        };
        let mut step = [0i32; 3];
        step[axis] = 1;
        // Another log comes into this one across (a branch goes on from here another way).
        let elbow = (0..3).filter(|&k| k != axis).any(|k| {
            [-1i32, 1].into_iter().any(|dir| {
                let mut d = [0i32; 3];
                d[k] = dir;
                let nb = r.get(x + d[0], y + d[1], z + d[2]);
                is_log(nb) && log_axis(nb) == k && log_radius(nb) <= radius + 0.01
            })
        });
        let mut ends = [(0.0f32, true), (1.0f32, true)];
        for (k, dir) in [-1i32, 1].into_iter().enumerate() {
            let nb = r.get(x + step[0] * dir, y + step[1] * dir, z + step[2] * dir);
            let t = if dir < 0 { 0.0 } else { 1.0 };
            ends[k] = if is_log(nb) && log_axis(nb) == axis {
                // Goes on into the next one (a thicker one here shows its end ring).
                (t, radius > log_radius(nb) + 0.01)
            } else if is_log(nb) && radius <= log_radius(nb) + 0.01 {
                (t + 0.5 * dir as f32, false)
            } else if is_branch(b) && elbow {
                // A branch turning (up, or aside): it stops just past the middle, where the
                // one going on from here closes round it, not out into the air.
                (0.5 + radius * dir as f32, true)
            } else {
                (t, true)
            };
        }
        let (s, bl) = r.light(x, y, z);
        let side_layer = face_texture(b, if axis == 1 { 0 } else { 2 });
        let end_layer = face_texture(b, if axis == 1 { 2 } else { 0 });
        let (t0, t1) = (ends[0].0, ends[1].0);
        // (no side faces straight along an axis: two logs crossing never have sides in the
        // same plane, which would flicker)
        let angle = |i: usize| i as f32 / sides as f32 * std::f32::consts::TAU;
        let around = if is_branch(b) { 1.0 } else { 3.0 };
        let (wx, wz) = ((x + self.ox) as f32, (z + self.oz) as f32);
        let world = move |p: [f32; 3]| [wx + p[0], y as f32 + p[1], wz + p[2]];
        let face_of = |n: [f32; 3]| {
            let a = n.map(f32::abs);
            let k = if a[0] >= a[1] && a[0] >= a[2] { 0 } else if a[1] >= a[2] { 1 } else { 2 };
            (k * 2 + (n[k] < 0.0) as usize) as u8
        };
        // A corner of the bark at `a` round: its smooth normal (see world.vert).
        let round_n = |a: f32| (16 + axis * 64 + ((a / std::f32::consts::TAU * 64.0).round() as usize % 64)) as u8;
        let emit = |b: &mut Self, ps: [[f32; 3]; 4], uvs: [[f32; 2]; 4], n: [f32; 3], layer: u32, ns: Option<[u8; 4]>| {
            // Wound to face `n`.
            let d = |a: [f32; 3], c: [f32; 3]| [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
            let (e1, e2) = (d(ps[0], ps[1]), d(ps[0], ps[2]));
            let cross = [e1[1] * e2[2] - e1[2] * e2[1], e1[2] * e2[0] - e1[0] * e2[2], e1[0] * e2[1] - e1[1] * e2[0]];
            let flip = cross[0] * n[0] + cross[1] * n[1] + cross[2] * n[2] < 0.0;
            let base = b.verts.len() as u32;
            let face = face_of(n);
            for i in 0..4 {
                b.push(Vertex {
                    pos: world(ps[i]),
                    uv: uvs[i],
                    layer: layer as f32,
                    light: [255, (s * 17) as u8, (bl * 17) as u8, ns.map_or(face, |ns| ns[i])],
                    tint: [255, 255, 255, 0],
                });
            }
            let idx = if flip { [base, base + 2, base + 1, base, base + 3, base + 2] } else { [base, base + 1, base + 2, base, base + 2, base + 3] };
            b.opaque.extend_from_slice(&idx);
        };
        if axis == 1 && !is_branch(b) {
            if let Some(notch) = notch_at(IVec3::new(x + self.ox, y, z + self.oz)) {
                self.notched_log(notch, radius, sides, around, (t0, ends[0].1), (t1, ends[1].1), side_layer, end_layer, &emit, &round_n, &at);
                return;
            }
        }
        for i in 0..sides {
            let (a0, a1) = (angle(i), angle(i + 1));
            let (c0, s0, c1, s1) = (a0.cos() * radius, a0.sin() * radius, a1.cos() * radius, a1.sin() * radius);
            let mid = (a0 + a1) * 0.5;
            let mut n = [0.0f32; 3];
            n[u_axis] = mid.cos();
            n[v_axis] = mid.sin();
            let (u0, u1) = (i as f32 / sides as f32 * around, (i + 1) as f32 / sides as f32 * around);
            emit(
                self,
                [at(t0, c0, s0), at(t0, c1, s1), at(t1, c1, s1), at(t1, c0, s0)],
                [[u0, 1.0 - t0], [u1, 1.0 - t0], [u1, 1.0 - t1], [u0, 1.0 - t1]],
                n,
                side_layer,
                Some([round_n(a0), round_n(a1), round_n(a1), round_n(a0)]),
            );
            // The end caps: a slice of the rings each.
            for (k, &(t, capped)) in ends.iter().enumerate() {
                if !capped {
                    continue;
                }
                let mut n = [0.0f32; 3];
                n[axis] = if k == 0 { -1.0 } else { 1.0 };
                // The whole end (rings and the bark round them) at any thickness.
                let k = LOG_END_RIM / radius;
                let uv = |u: f32, v: f32| [0.5 + u * k, 0.5 + v * k];
                emit(
                    self,
                    [at(t, 0.0, 0.0), at(t, c0, s0), at(t, c1, s1), at(t, 0.0, 0.0)],
                    [uv(0.0, 0.0), uv(c0, s0), uv(c1, s1), uv(0.0, 0.0)],
                    n,
                    end_layer,
                    None,
                );
            }
        }
    }

    /// An upright trunk with an axe's cut in it (`Notch`): a wedge taken out of its side,
    /// deepest at the cut's middle, in steps like chips hewn out one after another. The
    /// trunk is drawn in slices: whole below and above the cut, and each slice of the cut
    /// the round cross-section with the part past the cut's face gone (its face and the
    /// steps between the slices bare wood).
    #[allow(clippy::too_many_arguments)]
    fn notched_log(
        &mut self,
        notch: Notch,
        radius: f32,
        sides: usize,
        around: f32,
        (t0, cap0): (f32, bool),
        (t1, cap1): (f32, bool),
        side_layer: u32,
        end_layer: u32,
        emit: &dyn Fn(&mut Self, [[f32; 3]; 4], [[f32; 2]; 4], [f32; 3], u32, Option<[u8; 4]>),
        round_n: &dyn Fn(f32) -> u8,
        at: &dyn Fn(f32, f32, f32) -> [f32; 3],
    ) {
        use std::f32::consts::TAU;
        const SLICES: usize = 6;
        let dir = [notch.angle.cos(), notch.angle.sin()];
        let across = [-dir[1], dir[0]];
        let deep = notch.depth.clamp(0.0, 1.0) * 2.0 * radius;
        let half = (deep * 0.8).max(0.08);
        let h = notch.height.clamp(0.12, 0.88);
        let (z0, z1) = ((h - half).max(t0 + 0.02), (h + half).min(t1 - 0.02));
        let ring: Vec<[f32; 2]> = (0..sides)
            .map(|i| {
                let a = i as f32 / sides as f32 * TAU;
                [a.cos() * radius, a.sin() * radius]
            })
            .collect();
        // The cross-section with everything past `c` along the cut's direction gone.
        let clip = |c: f32| -> Vec<[f32; 2]> {
            let d = |p: [f32; 2]| p[0] * dir[0] + p[1] * dir[1] - c;
            let mut out = Vec::new();
            for i in 0..ring.len() {
                let (a, b) = (ring[i], ring[(i + 1) % ring.len()]);
                let (da, db) = (d(a), d(b));
                if da <= 0.0 {
                    out.push(a);
                }
                if (da <= 0.0) != (db <= 0.0) {
                    let k = da / (da - db);
                    out.push([a[0] + (b[0] - a[0]) * k, a[1] + (b[1] - a[1]) * k]);
                }
            }
            out
        };
        let rim = LOG_END_RIM / radius;
        // One slice from `ya` up to `yb` of the cross-section `poly` (cut at `c`), with its
        // bottom and top ends if asked.
        let slice = |m: &mut Self, poly: &[[f32; 2]], c: f32, ya: f32, yb: f32, bottom: bool, top: bool| {
            if poly.len() < 3 || yb - ya < 1e-4 {
                return;
            }
            let on_cut = |p: [f32; 2]| (p[0] * dir[0] + p[1] * dir[1] - c).abs() < 1e-4;
            for i in 0..poly.len() {
                let (p, q) = (poly[i], poly[(i + 1) % poly.len()]);
                if on_cut(p) && on_cut(q) {
                    // The cut's face: bare wood, its grain along the trunk.
                    let u = |p: [f32; 2]| 0.5 + p[0] * across[0] + p[1] * across[1];
                    let n = [dir[0], 0.0, dir[1]];
                    emit(
                        m,
                        [at(ya, p[0], p[1]), at(ya, q[0], q[1]), at(yb, q[0], q[1]), at(yb, p[0], p[1])],
                        [[u(p), 1.0 - ya], [u(q), 1.0 - ya], [u(q), 1.0 - yb], [u(p), 1.0 - yb]],
                        n,
                        end_layer,
                        None,
                    );
                    continue;
                }
                let (ap, mut aq) = (p[1].atan2(p[0]).rem_euclid(TAU), q[1].atan2(q[0]).rem_euclid(TAU));
                if aq < ap {
                    aq += TAU;
                }
                let mid = (ap + aq) * 0.5;
                let (up, uq) = (ap / TAU * around, aq / TAU * around);
                emit(
                    m,
                    [at(ya, p[0], p[1]), at(ya, q[0], q[1]), at(yb, q[0], q[1]), at(yb, p[0], p[1])],
                    [[up, 1.0 - ya], [uq, 1.0 - ya], [uq, 1.0 - yb], [up, 1.0 - yb]],
                    [mid.cos(), 0.0, mid.sin()],
                    side_layer,
                    Some([round_n(ap), round_n(aq), round_n(aq), round_n(ap)]),
                );
            }
            let n = poly.len() as f32;
            let mid = poly.iter().fold([0.0, 0.0], |s, p| [s[0] + p[0] / n, s[1] + p[1] / n]);
            let uv = |p: [f32; 2]| [0.5 + p[0] * rim, 0.5 + p[1] * rim];
            for (y, up, on) in [(ya, -1.0, bottom), (yb, 1.0, top)] {
                if !on {
                    continue;
                }
                for i in 0..poly.len() {
                    let (p, q) = (poly[i], poly[(i + 1) % poly.len()]);
                    emit(
                        m,
                        [at(y, mid[0], mid[1]), at(y, p[0], p[1]), at(y, q[0], q[1]), at(y, mid[0], mid[1])],
                        [uv(mid), uv(p), uv(q), uv(mid)],
                        [0.0, up, 0.0],
                        end_layer,
                        None,
                    );
                }
            }
        };
        let whole = clip(f32::INFINITY);
        if notch.felled {
            // A stump: cut flat, and on the far side the hinge the tree broke off at, a ridge
            // of the wood the cut had not reached, up to the cut's middle.
            let (flat, hinge) = stump_heights(notch, radius);
            let c = radius - deep;
            slice(self, &whole, f32::INFINITY, t0, flat, cap0, true);
            slice(self, &clip(c), c, flat, hinge, false, true);
            return;
        }
        slice(self, &whole, f32::INFINITY, t0, z0, cap0, true);
        let n = SLICES;
        for k in 0..n {
            let ya = z0 + (z1 - z0) * k as f32 / n as f32;
            let yb = z0 + (z1 - z0) * (k + 1) as f32 / n as f32;
            let dy = ((ya + yb) * 0.5 - h).abs();
            let c = radius - deep * (1.0 - dy / half).max(0.0);
            slice(self, &clip(c), c, ya, yb, true, true);
        }
        slice(self, &whole, f32::INFINITY, z1, t1, true, cap1);
    }

    /// Stairs: the filled eighths of the block, without the faces between them or against
    /// solid neighbours.
    fn stairs(&mut self, r: &Region, x: i32, y: i32, z: i32, b: u8) {
        let octants = |x: i32, y: i32, z: i32, b: u8| {
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
    fn bed(&mut self, r: &Region, x: i32, y: i32, z: i32, b: u8) {
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

/// How high a fluid of this level stands in its block (0..1).
pub fn fluid_height(level: u8) -> f32 {
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
        leaf_inner: Vec::new(),
        plants: Vec::new(),
        dirs: Default::default(),
        water: Vec::new(),
        min_y: HEIGHT as f32,
        max_y: 0.0,
        ox: pos.0 * 16 - 16,
        oz: pos.1 * 16 - 16,
    };
    let mut doors = Vec::new();
    let mut chests = Vec::new();
    let mut gun_stations = Vec::new();
    let mut torches = Vec::new();
    let mut stump_marks = Vec::new();
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
                    let from = m.opaque.len();
                    m.plant(&r, x, y, z, face_texture(b, 0), tint);
                    m.plants.extend(m.opaque.drain(from..));
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
                    torches.push(glam::IVec3::new(x + m.ox, y, z + m.oz));
                    continue;
                }
                if is_chest(b) {
                    m.chest(&r, x, y, z, b);
                    chests.push(glam::IVec3::new(x + m.ox, y, z + m.oz));
                    continue;
                }
                if is_furnace(b) {
                    m.furnace(&r, x, y, z, b);
                    continue;
                }
                if is_chimney(b) {
                    m.chimney(&r, x, y, z, b);
                    continue;
                }
                if is_stump_mark(b) {
                    stump_marks.push(glam::IVec3::new(x + m.ox, y, z + m.oz));
                }
                if is_door(b) {
                    doors.push(glam::IVec3::new(x + m.ox, y, z + m.oz));
                    continue;
                }
                if is_gun_bench(b) {
                    // Drawn every frame from its left half (see `gun_stations`).
                    if is_bench_main(b) {
                        gun_stations.push(glam::IVec3::new(x + m.ox, y, z + m.oz));
                    }
                    continue;
                }
                if is_stairs(b) {
                    m.stairs(&r, x, y, z, b);
                    continue;
                }
                if is_log(b) {
                    m.round_log(&r, x, y, z, b);
                    continue;
                }
                if is_bed(b) {
                    m.bed(&r, x, y, z, b);
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
                    let rotated = face_rotated(b, face);
                    let from = m.opaque.len();
                    m.cube_face(&r, x, y, z, face, face_texture(b, face), tint, fl, rotated);
                    if face == 2 && is_stump_mark(b) {
                        // The mark of the cut-down trunk, a hair over the grass.
                        let v0 = m.verts.len();
                        let layer = tex::STUMP_MARK + stump_stage(b) as u32;
                        m.cube_face(&r, x, y, z, face, layer, [255; 3], fl, false);
                        for v in &mut m.verts[v0..] {
                            v.pos[1] += 0.002;
                        }
                    }
                    // (moved over without a list of its own for every face)
                    let quad = m.opaque.drain(from..);
                    if is_leaves(b) && is_leaves(nbk) {
                        m.leaf_inner.extend(quad);
                    } else {
                        m.dirs[face].extend(quad);
                    }
                }
            }
        }
    }

    // The center chunk's light, sky in the high nibble: (y * 16 + z) * 16 + x.
    let mut light = vec![0u8; 256 * r.h];
    for y in 0..r.h {
        for z in 0..16 {
            for x in 0..16 {
                let i = r.idx(x + 16, y, z + 16);
                light[(y * 16 + z) * 16 + x] = (r.sky[i] << 4) | r.blk[i];
            }
        }
    }

    let dir_counts = std::array::from_fn(|d| m.dirs[d].len() as u32);
    let mut indices = m.opaque;
    for d in &m.dirs {
        indices.extend_from_slice(d);
    }
    let solid_count = indices.len() as u32;
    let leaf_inner_count = m.leaf_inner.len() as u32;
    indices.extend_from_slice(&m.leaf_inner);
    indices.extend_from_slice(&m.plants);
    let opaque_count = indices.len() as u32;
    indices.extend_from_slice(&m.water);
    MeshData {
        pos,
        vertices: m.verts,
        indices,
        opaque_count,
        solid_count,
        dir_counts,
        leaf_inner_count,
        min_y: m.min_y,
        max_y: m.max_y,
        doors,
        chests,
        gun_stations,
        torches,
        stump_marks,
        light: ChunkLight {
            h: r.h,
            data: light.into(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A 3x3 neighbourhood of chunks with a stone floor at y 0, the center chunk from `edit`.
    fn hood(edit: impl Fn(&mut ChunkData)) -> [Arc<ChunkData>; 9] {
        let floor = || {
            let mut c = ChunkData::new();
            for z in 0..16 {
                for x in 0..16 {
                    c.set(x, 0, z, STONE);
                }
            }
            c
        };
        let mut center = floor();
        edit(&mut center);
        std::array::from_fn(|i| Arc::new(if i == 4 { center.clone() } else { floor() }))
    }

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
        let mut vertices = 0;
        for (i, nb) in hoods.iter().enumerate() {
            let m = mesh_chunk(((i % 3) as i32 + 1, (i / 3) as i32 + 1), nb, &[], &gen);
            vertices += m.vertices.len();
        }
        println!("9 chunks meshed in {:?}, {vertices} vertices", t.elapsed());
    }

    #[test]
    fn beds_turn_their_head_north_for_the_textures() {
        for f in 0..4 {
            let head = facing_dir(f);
            assert_eq!(bed_local(head, f), glam::IVec3::NEG_Z);
            // The bed's right (its east when facing north) stays on its right.
            assert_eq!(bed_local(facing_dir(f + 1), f), glam::IVec3::X);
        }
    }

    #[test]
    fn glass_wall_faces_join_their_neighbours() {
        // A glass wall three blocks wide and one high along x, at z 8.
        let nb = hood(|c| {
            for x in 7..10 {
                c.set(x, 1, 8, GLASS);
            }
        });
        let r = Region::new(&nb);
        // Face -Z of the middle pane: joined left and right (u runs along -x), not up or down.
        let mask = glass_mask(&r, 16 + 8, 1, 16 + 8, 5);
        assert_eq!(mask & 0b11, 0b11, "{mask:08b}");
        assert_eq!(mask & 0b1100, 0, "{mask:08b}");
        // The end pane joins only toward the middle.
        let end = glass_mask(&r, 16 + 7, 1, 16 + 8, 5);
        assert_eq!((end & 0b11).count_ones(), 1, "{end:08b}");
    }

    #[test]
    fn a_just_broken_block_takes_the_light_around_it() {
        // A stone block on the floor, under the open sky.
        let nb = hood(|c| c.set(8, 1, 8, STONE));
        let m = mesh_chunk((0, 0), &nb, &[], &Generator::new(1));
        let mut world = World::new();
        world.chunks.insert((0, 0), nb[4].clone());
        world.light.insert((0, 0), m.light.clone());
        // Broken: its cell still has the solid block's light until the chunk is lit again.
        world.set(8, 1, 8, AIR);
        let at = glam::IVec3::new(8, 1, 8);
        assert!(world.light_estimate(at.as_vec3() + glam::Vec3::splat(0.5)).0 < 15);
        assert_eq!(world.light_around(at).0, 15);
    }

    #[test]
    fn chests_are_listed_for_their_lids() {
        let nb = hood(|c| {
            c.set(3, 1, 4, crate::world::CHEST);
            c.set(10, 5, 12, crate::world::CHEST + 2);
        });
        let m = mesh_chunk((0, 0), &nb, &[], &Generator::new(1));
        let mut chests = m.chests.clone();
        chests.sort_by_key(|p| p.x);
        assert_eq!(chests, [glam::IVec3::new(3, 1, 4), glam::IVec3::new(10, 5, 12)]);
    }

    #[test]
    fn things_under_a_lintel_get_the_light_around_them() {
        // A door in a wall with a beam over it: the door's cell is lit from the open sides,
        // not dark as if it were under a roof.
        let nb = hood(|c| {
            for x in 6..11 {
                for y in 1..4 {
                    c.set(x, y, 8, PLANKS);
                }
            }
            c.set(8, 1, 8, door_id(0, false, false, false));
            c.set(8, 2, 8, door_id(0, false, true, false));
        });
        let gen = Generator::new(1);
        let m = mesh_chunk((0, 0), &nb, &[], &gen);
        let mut world = World::new();
        world.chunks.insert((0, 0), nb[4].clone());
        world.light.insert((0, 0), m.light.clone());
        assert_eq!(m.doors.len(), 2);
        for y in [1.5, 2.5] {
            let (sky, _) = world.light_estimate(glam::Vec3::new(8.5, y, 8.5));
            assert!(sky >= 14, "door at y {y}: sky {sky}");
        }
        // Inside the wall: the light of its open sides.
        assert_eq!(world.light_estimate(glam::Vec3::new(6.5, 1.5, 8.5)).0, 15);
    }

    #[test]
    fn torches_and_stump_marks_are_listed() {
        let nb = hood(|c| {
            c.set(3, 1, 4, TORCH);
            c.set(5, 0, 5, stump_mark(GRASS, 0));
        });
        let m = mesh_chunk((0, 0), &nb, &[], &Generator::new(1));
        assert_eq!(m.torches, vec![glam::IVec3::new(3, 1, 4)]);
        assert_eq!(m.stump_marks, vec![glam::IVec3::new(5, 0, 5)]);
    }
}

/// How far out a round log's end reaches in its end texture (0.5 is the edge): just into
/// the bark ring round the wood.
pub const LOG_END_RIM: f32 = 0.47;

/// An axe's cut in an upright trunk: the way its face looks (radians round the trunk, 0 =
/// +X, toward +Z), the height of its middle in the block (0..1) and how deep it goes (0..1
/// of the trunk's width); after the tree fell, the stump left.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Notch {
    pub angle: f32,
    pub height: f32,
    pub depth: f32,
    /// The tree above has been felled: only the stump is left, up to the cut's middle.
    pub felled: bool,
}

/// The cuts in the trunks (few at a time), read by the mesher.
static NOTCHES: std::sync::RwLock<Vec<(IVec3, Notch)>> = std::sync::RwLock::new(Vec::new());

pub fn notch_at(p: IVec3) -> Option<Notch> {
    let list = NOTCHES.read().ok()?;
    list.iter().find(|(q, _)| *q == p).map(|(_, n)| *n)
}

/// A stump's heights in its block (a trunk `radius` thick): its flat top, and the top of the
/// hinge left standing on its far side (the cut's middle).
pub fn stump_heights(notch: Notch, radius: f32) -> (f32, f32) {
    let h = notch.height.clamp(0.12, 0.88);
    let deep = notch.depth.clamp(0.0, 1.0) * 2.0 * radius;
    let low = (h - (deep * 0.8).max(0.08)).max(0.02);
    ((low + h) * 0.5, h)
}

/// Every cut there is (for saving).
pub fn all_notches() -> Vec<(IVec3, Notch)> {
    NOTCHES.read().map(|l| l.clone()).unwrap_or_default()
}

/// No cuts any more (another world is loaded).
pub fn clear_notches() {
    if let Ok(mut list) = NOTCHES.write() {
        list.clear();
    }
}

/// Puts (or with None, takes away) the cut at `p`; the chunk has to be meshed again.
pub fn set_notch(p: IVec3, notch: Option<Notch>) {
    let Ok(mut list) = NOTCHES.write() else { return };
    list.retain(|(q, _)| *q != p);
    if let Some(n) = notch {
        list.push((p, n));
    }
}
