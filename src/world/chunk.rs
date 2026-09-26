use super::block::{attenuates_sky, AIR};
use std::collections::{HashMap, HashSet};
use std::hash::{BuildHasherDefault, Hasher};

pub const CHUNK: usize = 16;
pub const HEIGHT: usize = 256;
const VOL: usize = CHUNK * CHUNK * HEIGHT;

pub type ChunkPos = (i32, i32);

/// Small, fast non-cryptographic hasher for integer keys.
#[derive(Default)]
pub struct FxHasher(u64);

impl FxHasher {
    #[inline]
    fn add(&mut self, i: u64) {
        self.0 = (self.0.rotate_left(5) ^ i).wrapping_mul(0x517c_c1b7_2722_0a95);
    }
}

impl Hasher for FxHasher {
    fn finish(&self) -> u64 {
        self.0
    }
    fn write(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.add(b as u64);
        }
    }
    fn write_u8(&mut self, i: u8) {
        self.add(i as u64)
    }
    fn write_u32(&mut self, i: u32) {
        self.add(i as u64)
    }
    fn write_i32(&mut self, i: i32) {
        self.add(i as u32 as u64)
    }
    fn write_u64(&mut self, i: u64) {
        self.add(i)
    }
    fn write_usize(&mut self, i: usize) {
        self.add(i as u64)
    }
}

pub type FastMap<K, V> = HashMap<K, V, BuildHasherDefault<FxHasher>>;
pub type FastSet<K> = HashSet<K, BuildHasherDefault<FxHasher>>;

/// A 16x256x16 column of blocks, stored y-major so horizontal slices are contiguous.
#[derive(Clone)]
pub struct ChunkData {
    blocks: Box<[u8]>,
    /// Per column: y of the highest block that stops full sunlight.
    pub heightmap: [u8; 256],
    /// Highest non-air block (conservative upper bound).
    pub max_y: u8,
}

impl ChunkData {
    pub fn new() -> Self {
        Self {
            blocks: vec![AIR; VOL].into_boxed_slice(),
            heightmap: [0; 256],
            max_y: 0,
        }
    }

    /// Raw block array (for saving).
    pub fn raw(&self) -> &[u8] {
        &self.blocks
    }

    /// Rebuilds a chunk from a saved block array.
    pub fn from_raw(blocks: Vec<u8>) -> Option<Self> {
        if blocks.len() != VOL {
            return None;
        }
        let mut c = Self {
            blocks: blocks.into_boxed_slice(),
            heightmap: [0; 256],
            max_y: 0,
        };
        c.recompute();
        Some(c)
    }

    #[inline]
    fn idx(x: usize, y: usize, z: usize) -> usize {
        (y * CHUNK + z) * CHUNK + x
    }

    #[inline]
    pub fn get(&self, x: usize, y: usize, z: usize) -> u8 {
        self.blocks[Self::idx(x, y, z)]
    }

    /// The 16 blocks of row (y, z).
    #[inline]
    pub fn row(&self, y: usize, z: usize) -> &[u8] {
        let i = Self::idx(0, y, z);
        &self.blocks[i..i + CHUNK]
    }

    /// Raw write used during generation; call `recompute` afterwards.
    #[inline]
    pub fn set_raw(&mut self, x: usize, y: usize, z: usize, b: u8) {
        self.blocks[Self::idx(x, y, z)] = b;
    }

    pub fn set(&mut self, x: usize, y: usize, z: usize, b: u8) {
        self.blocks[Self::idx(x, y, z)] = b;
        let hi = z * CHUNK + x;
        if attenuates_sky(b) {
            if y as u8 > self.heightmap[hi] {
                self.heightmap[hi] = y as u8;
            }
        } else if y == self.heightmap[hi] as usize {
            self.heightmap[hi] = (0..y)
                .rev()
                .find(|&yy| attenuates_sky(self.get(x, yy, z)))
                .unwrap_or(0) as u8;
        }
        if b != AIR && y as u8 > self.max_y {
            self.max_y = y as u8;
        }
    }

    pub fn recompute(&mut self) {
        let mut max_y = 0usize;
        for z in 0..CHUNK {
            for x in 0..CHUNK {
                let mut hm = 0usize;
                for y in (0..HEIGHT).rev() {
                    let b = self.get(x, y, z);
                    if b != AIR && y > max_y {
                        max_y = y;
                    }
                    if attenuates_sky(b) {
                        hm = y;
                        break;
                    }
                }
                self.heightmap[z * CHUNK + x] = hm as u8;
            }
        }
        // Account for non-attenuating blocks above the heightmap (plants, glass).
        for y in (max_y..HEIGHT).rev() {
            if (0..CHUNK).any(|z| self.row(y, z).iter().any(|&b| b != AIR)) {
                max_y = max_y.max(y);
                break;
            }
        }
        self.max_y = max_y as u8;
    }
}
