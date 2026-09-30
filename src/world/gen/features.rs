//! What is placed on and in the terrain: ores, plants and trees.

use super::*;
use crate::world::trees;

/// Highest per-column tree chance of any biome (see `place_trees`).
const MAX_TREE_CHANCE: f64 = 0.075;

#[derive(Clone, Copy)]
enum Tree {
    Oak,
    Birch,
    Spruce,
    Cactus,
}

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 32) as u32
    }
    fn range(&mut self, lo: i32, hi: i32) -> i32 {
        lo + (self.next() % (hi - lo).max(1) as u32) as i32
    }
}

/// A value in lo..hi, most often near `peak` and ever rarer toward the ends (a triangular
/// distribution); `u` is uniform in 0..1.
fn triangular(u: f32, lo: f32, peak: f32, hi: f32) -> f32 {
    let (w, c) = (hi - lo, peak - lo);
    if u * w < c {
        lo + (u * w * c).sqrt()
    } else {
        hi - ((1.0 - u) * w * (hi - peak)).sqrt()
    }
}

/// Writes a block into the chunk if the world position falls inside it.
#[allow(clippy::too_many_arguments)]
fn put(
    c: &mut ChunkData,
    x0: i32,
    z0: i32,
    wx: i32,
    y: i32,
    wz: i32,
    b: u8,
    only_replaceable: bool,
) {
    let (lx, lz) = (wx - x0, wz - z0);
    if !(0..16).contains(&lx) || !(0..16).contains(&lz) || !(1..HEIGHT as i32).contains(&y) {
        return;
    }
    let cur = c.get(lx as usize, y as usize, lz as usize);
    if cur == BEDROCK {
        return;
    }
    if only_replaceable && !(cur == AIR || is_plant(cur)) {
        return;
    }
    c.set_raw(lx as usize, y as usize, lz as usize, b);
}

impl Generator {
    pub(super) fn place_ores(&self, c: &mut ChunkData, cx: i32, cz: i32) {
        let mut rng = Rng(((hash(self.seed ^ 0x0E5, cx, 0, cz) * 1e9) as u64) | 1);
        // (ore, veins per chunk (on average), lowest y, highest y, most common y, blocks per
        // vein). Tuned to about coal 100 : copper 55 : iron 22 : gold 8 : diamond 3. The
        // deeper the tier, the deeper and rarer the ore: coal all over, copper around sea
        // level, iron below it (and up in the mountains), gold deep, diamond at the bottom
        // among the lava lakes (y <= 10).
        let ores: [(u8, f32, i32, i32, i32, (u32, u32)); 6] = [
            (COAL_ORE, 30.0, 12, 130, 55, (5, 14)),
            (COPPER_ORE, 17.0, 22, 100, 52, (4, 9)),
            (IRON_ORE, 4.5, 8, 70, 36, (3, 7)),
            (IRON_ORE, 2.0, 80, 200, 130, (3, 7)),
            (GOLD_ORE, 2.0, 4, 40, 18, (2, 5)),
            (DIAMOND_ORE, 1.2, 3, 22, 9, (1, 3)),
        ];
        for (ore, veins, ymin, ymax, peak, (smin, smax)) in ores {
            // The fraction of `veins` is the chance of one more.
            let extra = ((rng.next() % 1000) as f32) < veins.fract() * 1000.0;
            for _ in 0..veins as u32 + extra as u32 {
                let u = (rng.next() % 1_000_000) as f32 / 1_000_000.0;
                let y = triangular(u, ymin as f32, peak as f32, ymax as f32) as i32;
                let (mut x, mut y, mut z) = (rng.range(0, 16), y, rng.range(0, 16));
                let size = smin + rng.next() % (smax - smin + 1);
                for _ in 0..size {
                    if (0..16).contains(&x)
                        && (0..16).contains(&z)
                        && (1..HEIGHT as i32).contains(&y)
                        && c.get(x as usize, y as usize, z as usize) == STONE
                    {
                        c.set_raw(x as usize, y as usize, z as usize, ore);
                    }
                    match rng.next() % 6 {
                        0 => x += 1,
                        1 => x -= 1,
                        2 => y += 1,
                        3 => y -= 1,
                        4 => z += 1,
                        _ => z -= 1,
                    }
                }
            }
        }
    }

    pub(super) fn place_plants(&self, c: &mut ChunkData, x0: i32, z0: i32, cols: &[[Option<Column>; 16]; 16]) {
        for (lz, row) in cols.iter().enumerate() {
            for (lx, col) in row.iter().enumerate() {
                let col = col.unwrap();
                let h = col.height;
                if h < SEA || h + 1 >= HEIGHT as i32 {
                    continue;
                }
                let (wx, wz) = (x0 + lx as i32, z0 + lz as i32);
                let ground = c.get(lx, h as usize, lz);
                if c.get(lx, h as usize + 1, lz) != AIR {
                    continue;
                }
                let r = hash(self.seed ^ 0x9A55, wx, 0, wz);
                let plant = match (col.biome, ground) {
                    (Biome::Desert, SAND) if r < 0.012 => DEAD_BUSH,
                    (_, GRASS) => {
                        let (grass, flowers) = match col.biome {
                            Biome::Plains => (0.26, 0.03),
                            Biome::Forest | Biome::BirchForest => (0.12, 0.012),
                            Biome::Taiga => (0.07, 0.0),
                            Biome::Mountains => (0.05, 0.004),
                            _ => (0.05, 0.0),
                        };
                        if r < flowers {
                            if hash(self.seed, wx, 3, wz) < 0.5 {
                                POPPY
                            } else {
                                DANDELION
                            }
                        } else if r < flowers + grass {
                            TALL_GRASS
                        } else {
                            continue;
                        }
                    }
                    _ => continue,
                };
                c.set_raw(lx, h as usize + 1, lz, plant);
            }
        }
    }

    pub(super) fn place_trees(&self, c: &mut ChunkData, x0: i32, z0: i32, cols: &[[Option<Column>; 16]; 16]) {
        // (trees standing next to the chunk reach into it)
        let reach = trees::REACH;
        // The columns round the chunk, each worked out once when first needed (a tree's spot
        // and the slope there look at the same ones again): those in the chunk are `cols`.
        let side = (16 + 2 * reach + 2) as usize;
        let (ox, oz) = (x0 - reach - 1, z0 - reach - 1);
        let mut around: Vec<Option<Column>> = vec![None; side * side];
        let mut column = |x: i32, z: i32| -> Column {
            let (lx, lz) = (x - x0, z - z0);
            if (0..16).contains(&lx) && (0..16).contains(&lz) {
                if let Some(c) = cols[lz as usize][lx as usize] {
                    return c;
                }
            }
            *around[(z - oz) as usize * side + (x - ox) as usize].get_or_insert_with(|| self.column(x, z))
        };
        for wz in z0 - reach..z0 + 16 + reach {
            for wx in x0 - reach..x0 + 16 + reach {
                // Cheap rejection first: no biome has a tree chance above MAX_TREE_CHANCE, so
                // most columns (especially the ones outside this chunk) skip `column()`.
                let tree_roll = hash(self.seed ^ 0x7EE5, wx, 0, wz);
                if tree_roll >= MAX_TREE_CHANCE {
                    continue;
                }
                let col = column(wx, wz);
                let h = col.height;
                if h <= SEA || h > 200 {
                    continue;
                }
                // Groves and clearings: tree density varies across a forest.
                let grove = smoothstep(
                    -0.25,
                    0.35,
                    self.detail
                        .fbm2(wx as f64 / 110.0 + 50.0, wz as f64 / 110.0, 2),
                );
                let pick = hash(self.seed, wx, 1, wz);
                let (chance, kind) = match col.biome {
                    Biome::Forest => (
                        0.02 + 0.055 * grove,
                        if pick < 0.2 { Tree::Birch } else { Tree::Oak },
                    ),
                    Biome::BirchForest => (
                        0.018 + 0.05 * grove,
                        if pick < 0.8 { Tree::Birch } else { Tree::Oak },
                    ),
                    Biome::Taiga => (0.015 + 0.05 * grove, Tree::Spruce),
                    Biome::SnowyTaiga => (0.012 + 0.04 * grove, Tree::Spruce),
                    Biome::SnowyPlains => (0.003, Tree::Spruce),
                    // Lone trees and the odd small grove on the plains.
                    Biome::Plains => (if grove > 0.92 { 0.02 } else { 0.0008 }, Tree::Oak),
                    Biome::Mountains if h < 118 => (0.012 * grove, Tree::Spruce),
                    Biome::Desert => (0.005, Tree::Cactus),
                    _ => continue,
                };
                if tree_roll >= chance {
                    continue;
                }
                // One tree to a spot: of trees that would stand closer than three blocks
                // (their trunks side by side under one crown, like one tree with several
                // trunks), only the one with the lowest roll grows.
                let crowded = (-2..=2).any(|dz: i32| {
                    (-2..=2).any(|dx: i32| {
                        (dx, dz) != (0, 0) && dx * dx + dz * dz <= 8 && hash(self.seed ^ 0x7EE5, wx + dx, 0, wz + dz) < tree_roll
                    })
                });
                if crowded {
                    continue;
                }
                if self.carved(wx, h, wz, h, false) || self.carved(wx, h - 1, wz, h, false) {
                    continue;
                }
                // Only on soil (or sand for cacti): not on cliffs, rock or snow.
                // The steepest height difference round it (blocks per block), from its
                // neighbours.
                let slope = {
                    let mut h = |x, z| column(x, z).height;
                    let dx = (h(wx + 1, wz) - h(wx - 1, wz)).abs();
                    let dz = (h(wx, wz + 1) - h(wx, wz - 1)).abs();
                    (dx.max(dz) + 1) / 2
                };
                let top = self.surface_blocks(&col, wx, wz, slope).0;
                let soil = match kind {
                    Tree::Cactus => top == SAND,
                    _ => matches!(top, GRASS | SNOWY_GRASS | DIRT),
                };
                if !soil {
                    continue;
                }
                let r = hash(self.seed ^ 0xBEEF, wx, 2, wz);
                self.tree(c, x0, z0, wx, h + 1, wz, kind, r);
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn tree(
        &self,
        c: &mut ChunkData,
        x0: i32,
        z0: i32,
        x: i32,
        y: i32,
        z: i32,
        kind: Tree,
        r: f64,
    ) {
        let log = match kind {
            Tree::Oak => OAK_LOG,
            Tree::Birch => BIRCH_LOG,
            Tree::Spruce => SPRUCE_LOG,
            Tree::Cactus => CACTUS,
        };
        match kind {
            Tree::Oak | Tree::Birch | Tree::Spruce => {
                let seed = (r * u32::MAX as f64) as u32 ^ (x as u32).wrapping_mul(0x85EB_CA77) ^ (z as u32).wrapping_mul(0xC2B2_AE3D);
                for (d, b, soft) in trees::tree_shape(log, seed) {
                    put(c, x0, z0, x + d.x, y + d.y, z + d.z, b, soft);
                }
            }
            Tree::Cactus => {
                let height = 1 + (r * 3.0) as i32;
                for dy in 0..height {
                    put(c, x0, z0, x, y + dy, z, CACTUS, true);
                }
            }
        }
    }
}
