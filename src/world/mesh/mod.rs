//! Chunk meshing. Each job copies the 3x3 chunk neighbourhood into a flat region,
//! flood-fills sky and block light through it, then emits faces for the center chunk
//! with smooth lighting + ambient occlusion.

mod face;
mod fluid;
mod log;
mod notches;
mod region;
mod shapes;
#[cfg(test)]
mod tests;

pub use face::{box_uv, corner_pos, corner_uv, CORNERS, FACE_N, FACE_U, FACE_V};
pub use fluid::fluid_height;
pub use log::{stump_heights, LOG_END_RIM};
pub use notches::Notch;
pub use shapes::{chest_open_layer, torch_transform, CHEST_FLOOR, FURNACE_HOLLOWS};

use super::gen::Generator;
use super::textures::tex;
use super::*;
use face::glass_mask;
use region::Region;
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
    /// Opaque indices without the faces between leaves and the plants. They are, in order:
    /// faces of whole blocks with no see-through texels grouped by direction (see `FACE_N`;
    /// drawn without alpha testing, so the depth test runs before their fragment shader),
    /// the other solid ones (stairs, chests, torches...), then the faces of whole blocks
    /// with cut-out texels (glass, the outside of leaves, stump marks, furnace fronts) grouped
    /// by direction.
    pub solid_count: u32,
    /// Counts of the plain whole-block faces by direction (from index 0 on).
    pub dir_counts: [u32; 6],
    /// Counts of the cut-out whole-block faces by direction (ending the solid indices).
    pub cut_dir_counts: [u32; 6],
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

struct Builder {
    verts: Vec<Vertex>,
    opaque: Vec<u32>,
    /// Faces between leaves, and grass and flowers: after the opaque ones, so far chunks can
    /// leave them out.
    leaf_inner: Vec<u32>,
    plants: Vec<u32>,
    /// Faces of whole blocks by direction: a chunk's faces turned away from the camera can be
    /// left out as a group. Plain ones, and ones with cut-out texels (see `MeshData`).
    dirs: [Vec<u32>; 6],
    cut_dirs: [Vec<u32>; 6],
    water: Vec<u32>,
    min_y: f32,
    max_y: f32,
    ox: i32,
    oz: i32,
    /// The cuts in trunks round the chunk (see `World::notches`).
    notches: Vec<(glam::IVec3, Notch)>,
}

impl Builder {
    fn notch_at(&self, p: glam::IVec3) -> Option<Notch> {
        self.notches.iter().find(|(q, _)| *q == p).map(|(_, n)| *n)
    }

    /// Moves the whole-block face whose indices start at `from` in `opaque` to its
    /// direction's group: the cut-out one if its texture `layer` has see-through texels.
    fn to_dir(&mut self, face: usize, from: usize, layer: u32) {
        let quad = self.opaque.drain(from..);
        if has_cutout(layer) {
            self.cut_dirs[face].extend(quad);
        } else {
            self.dirs[face].extend(quad);
        }
    }

    #[inline]
    fn push(&mut self, v: Vertex) {
        self.min_y = self.min_y.min(v.pos[1]);
        self.max_y = self.max_y.max(v.pos[1]);
        self.verts.push(v);
    }
}

/// Texture layers of whole-block faces with see-through texels (alpha tested). Every other
/// layer of a whole-block face is opaque throughout (a resource pack's too: `textures::pack`
/// fills in the alpha of layers that are not cut out), so it needs no alpha test; the test
/// `plain_faces_have_no_see_through_texels` checks the procedural ones.
fn has_cutout(layer: u32) -> bool {
    matches!(
        layer,
        tex::GLASS
            | tex::OAK_LEAVES
            | tex::SPRUCE_LEAVES
            | tex::BIRCH_LEAVES
            | tex::FURNACE_FRONT_CUT
            | tex::BLAST_FRONT_CUT
            | tex::ADV_FRONT_CUT
    ) || (tex::STUMP_MARK..tex::STUMP_MARK + STUMP_STAGES as u32).contains(&layer)
}

pub fn mesh_chunk(
    pos: ChunkPos,
    nb: &[Arc<ChunkData>; 9],
    anim: &[(glam::IVec3, u8, f32)],
    notches: &[(glam::IVec3, Notch)],
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
        notches: notches.to_vec(),
        verts: Vec::with_capacity(16_384),
        opaque: Vec::with_capacity(24_576),
        leaf_inner: Vec::new(),
        plants: Vec::new(),
        dirs: Default::default(),
        cut_dirs: Default::default(),
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
                    let layer = face_texture(b, face);
                    // (moved over without a list of its own for every face)
                    let from = m.opaque.len();
                    m.cube_face(&r, x, y, z, face, layer, tint, fl, rotated);
                    if is_leaves(b) && is_leaves(nbk) {
                        let quad = m.opaque.drain(from..);
                        m.leaf_inner.extend(quad);
                    } else {
                        m.to_dir(face, from, layer);
                    }
                    if face == 2 && is_stump_mark(b) {
                        // The mark of the cut-down trunk, a hair over the grass.
                        let v0 = m.verts.len();
                        let from = m.opaque.len();
                        let layer = tex::STUMP_MARK + stump_stage(b) as u32;
                        m.cube_face(&r, x, y, z, face, layer, [255; 3], fl, false);
                        for v in &mut m.verts[v0..] {
                            v.pos[1] += 0.002;
                        }
                        m.to_dir(face, from, layer);
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
    let cut_dir_counts = std::array::from_fn(|d| m.cut_dirs[d].len() as u32);
    let mut indices: Vec<u32> = m.dirs.concat();
    indices.extend_from_slice(&m.opaque);
    for d in &m.cut_dirs {
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
        cut_dir_counts,
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
