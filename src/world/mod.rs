pub mod block;
pub mod chunk;
pub mod fluid;
pub mod gen;
pub mod jobs;
pub mod mesh;
pub mod noise;
pub mod terrain;
pub mod textures;

pub use block::*;
pub use chunk::*;

use glam::IVec3;
use std::sync::Arc;

/// All loaded chunks. Chunk data is shared copy-on-write with the meshing threads.
pub struct World {
    pub chunks: FastMap<ChunkPos, Arc<ChunkData>>,
    /// Edited chunks that were unloaded; restored when they come back into range.
    pub saved: FastMap<ChunkPos, Arc<ChunkData>>,
    pub modified: FastSet<ChunkPos>,
    /// Recent fluid changes: position -> (previous block, game time). Lets the mesher
    /// animate fluid surfaces smoothly between simulation ticks.
    pub fluid_changes: FastMap<IVec3, (u8, f32)>,
    /// LAN host: every block change, to send to the players.
    pub log: Option<Vec<(IVec3, u8)>>,
    /// LAN player: changes from the host for chunks that are not here yet.
    pub pending: FastMap<ChunkPos, Vec<(IVec3, u8)>>,
    /// The light of each meshed chunk, as the mesher flood-filled it.
    pub light: FastMap<ChunkPos, ChunkLight>,
}

/// A chunk's sky and block light per block (sky in the high nibble), up to height `h`
/// (above it: full sky, no block light).
#[derive(Clone, Default)]
pub struct ChunkLight {
    pub h: usize,
    pub data: Arc<[u8]>,
}

/// How long a fluid surface takes to move to its new height (one flow step).
pub const WATER_ANIM: f32 = 0.25;
pub const LAVA_ANIM: f32 = 1.5;

impl World {
    pub fn new() -> Self {
        Self {
            chunks: FastMap::default(),
            saved: FastMap::default(),
            modified: FastSet::default(),
            fluid_changes: FastMap::default(),
            log: None,
            pending: FastMap::default(),
            light: FastMap::default(),
        }
    }

    pub fn record_fluid_change(&mut self, p: IVec3, old: u8, new: u8, now: f32) {
        if is_fluid(old) || is_fluid(new) {
            // Keep the oldest still-animating state so rapid changes blend continuously.
            let keep = self
                .fluid_changes
                .get(&p)
                .filter(|(_, t)| now - *t < WATER_ANIM * 0.5)
                .map(|&(b, _)| b);
            self.fluid_changes.insert(p, (keep.unwrap_or(old), now));
        }
    }

    pub fn prune_fluid_changes(&mut self, now: f32) {
        self.fluid_changes
            .retain(|_, (_, t)| now - *t < LAVA_ANIM + 0.5);
    }

    /// Fluid changes inside the 3x3 chunk neighbourhood of `p` (for a mesh job).
    pub fn fluid_changes_near(&self, p: ChunkPos) -> Vec<(IVec3, u8, f32)> {
        let (x0, z0) = (p.0 * 16 - 16, p.1 * 16 - 16);
        self.fluid_changes
            .iter()
            .filter(|(q, _)| q.x >= x0 && q.x < x0 + 48 && q.z >= z0 && q.z < z0 + 48)
            .map(|(q, &(b, t))| (*q, b, t))
            .collect()
    }

    #[inline]
    pub fn chunk_pos(x: i32, z: i32) -> ChunkPos {
        (x.div_euclid(CHUNK as i32), z.div_euclid(CHUNK as i32))
    }

    #[inline]
    pub fn get(&self, x: i32, y: i32, z: i32) -> u8 {
        if y < 0 {
            return BEDROCK;
        }
        if y >= HEIGHT as i32 {
            return AIR;
        }
        match self.chunks.get(&Self::chunk_pos(x, z)) {
            Some(c) => c.get(
                x.rem_euclid(16) as usize,
                y as usize,
                z.rem_euclid(16) as usize,
            ),
            None => AIR,
        }
    }

    #[inline]
    pub fn geti(&self, p: IVec3) -> u8 {
        self.get(p.x, p.y, p.z)
    }

    pub fn is_loaded(&self, x: i32, z: i32) -> bool {
        self.chunks.contains_key(&Self::chunk_pos(x, z))
    }

    pub fn set(&mut self, x: i32, y: i32, z: i32, b: u8) -> bool {
        if y < 0 || y >= HEIGHT as i32 {
            return false;
        }
        let pos = Self::chunk_pos(x, z);
        match self.chunks.get_mut(&pos) {
            Some(c) => {
                Arc::make_mut(c).set(
                    x.rem_euclid(16) as usize,
                    y as usize,
                    z.rem_euclid(16) as usize,
                    b,
                );
                self.modified.insert(pos);
                if let Some(log) = &mut self.log {
                    log.push((IVec3::new(x, y, z), b));
                }
                true
            }
            None => false,
        }
    }

    pub fn seti(&mut self, p: IVec3, b: u8) -> bool {
        self.set(p.x, p.y, p.z, b)
    }

    /// y of the highest sunlight-blocking block, or None if not loaded.
    pub fn height_at(&self, x: i32, z: i32) -> Option<i32> {
        self.chunks
            .get(&Self::chunk_pos(x, z))
            .map(|c| c.heightmap[(z.rem_euclid(16) * 16 + x.rem_euclid(16)) as usize] as i32)
    }

    /// Sky light (0..15) at `p` for things rendered outside chunk meshes.
    pub fn sky_estimate(&self, p: glam::Vec3) -> u8 {
        self.light_estimate(p).0
    }

    /// (sky, block) light at `p` for things drawn outside chunk meshes: the same light the
    /// blocks around it are drawn with. Inside a solid block, the brightest open side.
    pub fn light_estimate(&self, p: glam::Vec3) -> (u8, u8) {
        let c = p.floor().as_ivec3();
        if let Some(l) = self.cell_light(c) {
            if !is_opaque(self.geti(c)) {
                return l;
            }
            let mut best = (0, 0);
            for d in [IVec3::Y, IVec3::X, IVec3::NEG_X, IVec3::Z, IVec3::NEG_Z, IVec3::NEG_Y] {
                if let Some(l) = self.cell_light(c + d).filter(|_| !is_opaque(self.geti(c + d))) {
                    best = (best.0.max(l.0), best.1.max(l.1));
                }
            }
            return best;
        }
        // Not meshed yet: guess from the height map and the light sources nearby.
        let sky = match self.height_at(c.x, c.z) {
            Some(h) if c.y <= h => (15 - ((h - c.y) * 2).min(12)).max(3) as u8,
            _ => 15,
        };
        (sky, self.block_light_guess(p))
    }

    /// Block light (0..15) at `p`.
    pub fn block_light_estimate(&self, p: glam::Vec3) -> u8 {
        self.light_estimate(p).1
    }

    fn cell_light(&self, c: IVec3) -> Option<(u8, u8)> {
        let l = self.light.get(&Self::chunk_pos(c.x, c.z))?;
        if c.y < 0 {
            return Some((0, 0));
        }
        if c.y as usize >= l.h {
            return Some((15, 0));
        }
        let v = l.data[(c.y as usize * 16 + c.z.rem_euclid(16) as usize) * 16 + c.x.rem_euclid(16) as usize];
        Some((v >> 4, v & 15))
    }

    /// Nearby emissive blocks -> block light guess 0..15.
    fn block_light_guess(&self, p: glam::Vec3) -> u8 {
        let c = p.floor().as_ivec3();
        let mut best = 0i32;
        for dy in -3..=3 {
            for dz in -3..=3 {
                for dx in -3..=3 {
                    if emission(self.get(c.x + dx, c.y + dy, c.z + dz)) > 0 {
                        best = best.max(15 - (dx.abs() + dy.abs() + dz.abs()));
                    }
                }
            }
        }
        best.clamp(0, 15) as u8
    }
}

impl World {
    /// LAN player: applies the host's changes that were waiting for chunk `p`.
    pub fn apply_pending(&mut self, p: ChunkPos) {
        if let Some(list) = self.pending.remove(&p) {
            for (q, b) in list {
                self.seti(q, b);
            }
        }
    }
}
