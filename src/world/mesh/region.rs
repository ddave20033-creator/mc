//! The 3x3 chunk neighbourhood a chunk is meshed in, and its sky and block light.

use super::*;
use std::collections::VecDeque;

const RW: usize = 48;

pub(super) struct Region {
    blocks: Vec<Block>,
    pub(super) sky: Vec<u8>,
    pub(super) blk: Vec<u8>,
    pub(super) h: usize,
    hm: Vec<i32>,
    /// Region coords -> (previous block, change time) for recently changed fluids.
    pub(super) old: FastMap<(i32, i32, i32), (Block, f32)>,
}

fn propagate(levels: &mut [u8], blocks: &[Block], h: usize, q: &mut VecDeque<u32>) {
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
    pub(super) fn new(nb: &[Arc<ChunkData>; 9]) -> Self {
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
    pub(super) fn idx(&self, x: usize, y: usize, z: usize) -> usize {
        (y * RW + z) * RW + x
    }

    #[inline]
    pub(super) fn get(&self, x: i32, y: i32, z: i32) -> Block {
        if y < 0 {
            return BEDROCK;
        }
        if y >= self.h as i32 || x < 0 || z < 0 || x >= RW as i32 || z >= RW as i32 {
            return AIR;
        }
        self.blocks[self.idx(x as usize, y as usize, z as usize)]
    }

    #[inline]
    pub(super) fn light(&self, x: i32, y: i32, z: i32) -> (u32, u32) {
        if y >= self.h as i32 {
            return (15, 0);
        }
        if y < 0 || x < 0 || z < 0 || x >= RW as i32 || z >= RW as i32 {
            return (0, 0);
        }
        let i = self.idx(x as usize, y as usize, z as usize);
        (self.sky[i] as u32, self.blk[i] as u32)
    }

    pub(super) fn compute_light(&mut self) {
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
