//! A chunk: a 16x256x16 column of blocks stored in 16 sections (an all-air one takes no
//! memory), with its height map; and the fast hash maps keyed by positions.

use super::block::{attenuates_sky, valid, Block, AIR};
use std::collections::{HashMap, HashSet};
use std::hash::{BuildHasherDefault, Hasher};

pub const CHUNK: usize = 16;
pub const HEIGHT: usize = 256;
pub const VOL: usize = CHUNK * CHUNK * HEIGHT;

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

/// Blocks in a section: 16 x 16 x 16.
pub const SECTION_VOL: usize = CHUNK * CHUNK * CHUNK;
const SECTIONS: usize = HEIGHT / CHUNK;

/// A 16x256x16 column of blocks, in 16 sections of 16x16x16 (none: all air), each y-major so
/// horizontal rows are contiguous.
#[derive(Clone)]
pub struct ChunkData {
    sections: [Option<Box<[Block; SECTION_VOL]>>; SECTIONS],
    /// Per column: y of the highest block that stops full sunlight.
    pub heightmap: [u8; 256],
    /// Highest non-air block (conservative upper bound).
    pub max_y: u8,
}

/// A row of air (`row` in an empty section).
static AIR_ROW: [Block; CHUNK] = [AIR; CHUNK];

impl ChunkData {
    pub fn new() -> Self {
        Self {
            sections: Default::default(),
            heightmap: [0; 256],
            max_y: 0,
        }
    }

    /// Bytes its blocks take.
    pub fn memory(&self) -> usize {
        self.sections.iter().flatten().count() * SECTION_VOL * size_of::<Block>()
    }

    /// All the blocks, y-major (`(y * 16 + z) * 16 + x`), for saving and sending.
    pub fn to_vec(&self) -> Vec<Block> {
        let mut v = Vec::with_capacity(VOL);
        for s in &self.sections {
            match s {
                Some(s) => v.extend_from_slice(&s[..]),
                None => v.resize(v.len() + SECTION_VOL, AIR),
            }
        }
        v
    }

    /// Rebuilds a chunk from all its blocks (`to_vec`); unknown ids become air.
    pub fn from_vec(blocks: &[Block]) -> Option<Self> {
        if blocks.len() != VOL {
            return None;
        }
        let mut c = Self::new();
        for (i, part) in blocks.chunks_exact(SECTION_VOL).enumerate() {
            if part.iter().any(|&b| b != AIR) {
                let mut s = Box::new([AIR; SECTION_VOL]);
                for (d, &b) in s.iter_mut().zip(part) {
                    *d = valid(b);
                }
                c.sections[i] = Some(s);
            }
        }
        c.recompute();
        Some(c)
    }

    #[inline]
    fn idx(x: usize, y: usize, z: usize) -> usize {
        ((y & 15) * CHUNK + z) * CHUNK + x
    }

    #[inline]
    pub fn get(&self, x: usize, y: usize, z: usize) -> Block {
        match &self.sections[y >> 4] {
            Some(s) => s[Self::idx(x, y, z)],
            None => AIR,
        }
    }

    /// The 16 blocks of row (y, z).
    #[inline]
    pub fn row(&self, y: usize, z: usize) -> &[Block] {
        match &self.sections[y >> 4] {
            Some(s) => {
                let i = Self::idx(0, y, z);
                &s[i..i + CHUNK]
            }
            None => &AIR_ROW,
        }
    }

    /// Raw write used during generation; call `recompute` afterwards.
    #[inline]
    pub fn set_raw(&mut self, x: usize, y: usize, z: usize, b: Block) {
        let s = &mut self.sections[y >> 4];
        if s.is_none() {
            if b == AIR {
                return;
            }
            *s = Some(Box::new([AIR; SECTION_VOL]));
        }
        s.as_mut().unwrap()[Self::idx(x, y, z)] = b;
    }

    pub fn set(&mut self, x: usize, y: usize, z: usize, b: Block) {
        self.set_raw(x, y, z, b);
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
