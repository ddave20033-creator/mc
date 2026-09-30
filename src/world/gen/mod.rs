//! Deterministic terrain generation: every block is a pure function of (seed, x, y, z),
//! so chunks can be generated independently on worker threads.

mod caves;
mod climate;
mod column;
mod features;

pub use climate::Biome;
pub use column::Column;

use super::noise::Perlin;
use super::*;

pub const SEA: i32 = 62;

pub struct Generator {
    pub seed: u32,
    cont: Perlin,
    erosion: Perlin,
    peaks: Perlin,
    hills: Perlin,
    detail: Perlin,
    temp: Perlin,
    humid: Perlin,
    river: Perlin,
    cave_a: Perlin,
    cave_b: Perlin,
    cave_c: Perlin,
    warp: Perlin,
    ranges: Perlin,
    spurs: Perlin,
}

fn smoothstep(a: f64, b: f64, x: f64) -> f64 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn lerp(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}

fn spline(x: f64, pts: &[(f64, f64)]) -> f64 {
    if x <= pts[0].0 {
        return pts[0].1;
    }
    for w in pts.windows(2) {
        let ((x0, y0), (x1, y1)) = (w[0], w[1]);
        if x <= x1 {
            let t = (x - x0) / (x1 - x0);
            let t = t * t * (3.0 - 2.0 * t);
            return y0 + (y1 - y0) * t;
        }
    }
    pts[pts.len() - 1].1
}

pub fn hash(seed: u32, x: i32, y: i32, z: i32) -> f64 {
    let mut h = seed
        ^ (x as u32).wrapping_mul(0x9E37_79B1)
        ^ (y as u32).wrapping_mul(0x7FEB_352D)
        ^ (z as u32).wrapping_mul(0x85EB_CA77);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    h ^= h >> 12;
    h = h.wrapping_mul(0x297A_2D39);
    h ^= h >> 15;
    (h & 0xFF_FFFF) as f64 / 16_777_215.0
}

impl Generator {
    pub fn new(seed: u32) -> Self {
        let p = |k: u32| {
            Perlin::new(
                seed.wrapping_mul(31)
                    .wrapping_add(k.wrapping_mul(0x9E37_79B9)),
            )
        };
        Self {
            seed,
            cont: p(1),
            erosion: p(2),
            peaks: p(3),
            hills: p(4),
            detail: p(5),
            temp: p(6),
            humid: p(7),
            river: p(8),
            cave_a: p(9),
            cave_b: p(10),
            cave_c: p(11),
            warp: p(12),
            ranges: p(13),
            spurs: p(14),
        }
    }

    pub fn generate_chunk(&self, cx: i32, cz: i32) -> ChunkData {
        let mut c = ChunkData::new();
        let (x0, z0) = (cx * 16, cz * 16);
        let mut cols = [[None::<Column>; 16]; 16];
        // Heights with a 1-block border, for the slope of every column.
        let mut heights = [[0i32; 18]; 18];
        for (gz, row) in heights.iter_mut().enumerate() {
            for (gx, h) in row.iter_mut().enumerate() {
                let (wx, wz) = (x0 + gx as i32 - 1, z0 + gz as i32 - 1);
                let col = self.column(wx, wz);
                if (1..17).contains(&gx) && (1..17).contains(&gz) {
                    cols[gz - 1][gx - 1] = Some(col);
                }
                *h = col.height;
            }
        }

        for (lz, row) in cols.iter().enumerate() {
            for (lx, col) in row.iter().enumerate() {
                let (wx, wz) = (x0 + lx as i32, z0 + lz as i32);
                let col = col.unwrap();
                let h = col.height;
                let wet = h < SEA + 2;
                // Ragged ice edges instead of a sharp temperature line.
                let ice_noise = self.detail.noise2(wx as f64 / 24.0, wz as f64 / 24.0) as f32;
                let frozen = col.temp + ice_noise * 0.08 < 0.2;
                let (gx, gz) = (lx + 1, lz + 1);
                let dx = (heights[gz][gx + 1] - heights[gz][gx - 1]).abs();
                let dz = (heights[gz + 1][gx] - heights[gz - 1][gx]).abs();
                let slope = (dx.max(dz) + 1) / 2;
                let (top, filler, depth, deep, deep_depth) =
                    self.surface_blocks(&col, wx, wz, slope);
                let top_y = h.max(SEA);
                for y in 0..=top_y {
                    let mut b = if y == 0
                        || (y <= 3 && hash(self.seed, wx, y, wz) < (4 - y) as f64 / 4.0)
                    {
                        BEDROCK
                    } else if y == h {
                        top
                    } else if y < h && y > h - 1 - depth {
                        filler
                    } else if y < h && y > h - 1 - depth - deep_depth {
                        deep
                    } else if y < h {
                        STONE
                    } else if y == SEA && frozen {
                        ICE
                    } else {
                        WATER
                    };
                    if y <= h && b != BEDROCK && self.carved(wx, y, wz, h, wet) {
                        b = if y <= 10 { LAVA } else { AIR };
                    }
                    c.set_raw(lx, y as usize, lz, b);
                }
            }
        }

        self.place_ores(&mut c, cx, cz);
        self.place_plants(&mut c, x0, z0, &cols);
        self.place_trees(&mut c, x0, z0, &cols);
        c.recompute();
        c
    }

    /// Finds a dry land column near the origin.
    pub fn find_spawn(&self) -> (i32, i32) {
        for r in 0..400 {
            let n = (r * 6).max(1);
            for i in 0..n {
                let a = i as f64 / n as f64 * std::f64::consts::TAU;
                let (x, z) = (
                    (a.cos() * r as f64 * 6.0) as i32,
                    (a.sin() * r as f64 * 6.0) as i32,
                );
                let c = self.column(x, z);
                if c.height > SEA + 2
                    && matches!(
                        c.biome,
                        Biome::Plains
                            | Biome::Forest
                            | Biome::BirchForest
                            | Biome::Taiga
                            | Biome::SnowyTaiga
                            | Biome::SnowyPlains
                            | Biome::Desert
                    )
                {
                    return (x, z);
                }
            }
        }
        (0, 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The ores by rarity: coal most, then copper, iron, gold and diamond fewest; and by
    /// depth: from copper down, each tier deeper on average than the one before.
    #[test]
    fn ores_by_rarity_and_depth() {
        let gen = Generator::new(12345);
        let ores = [COAL_ORE, COPPER_ORE, IRON_ORE, GOLD_ORE, DIAMOND_ORE];
        let mut count = [0usize; 5];
        let mut ysum = [0usize; 5];
        for i in 0..36 {
            let c = gen.generate_chunk(i % 6, i / 6);
            for y in 0..HEIGHT {
                for z in 0..16 {
                    for x in 0..16 {
                        if let Some(k) = ores.iter().position(|&o| o == c.get(x, y, z)) {
                            count[k] += 1;
                            ysum[k] += y;
                        }
                    }
                }
            }
        }
        let depth: Vec<usize> = (0..5).map(|k| ysum[k] / count[k].max(1)).collect();
        println!("coal, copper, iron, gold, diamond: {count:?}, mean y {depth:?}");
        assert!(count.windows(2).all(|w| w[0] > w[1]), "{count:?}");
        // Coal is everywhere; from copper down each tier lies deeper.
        assert!(depth[1] > depth[2] && depth[2] > depth[3] && depth[3] > depth[4], "{depth:?}");
        assert!(depth[4] < 15, "diamonds too high: {depth:?}");
    }

    /// Prints the chunk generation speed (`cargo test --release gen_speed -- --nocapture`).
    /// No two trees' trunks side by side (which look like one tree with several trunks).
    #[test]
    fn trees_stand_apart() {
        let gen = Generator::new(12345);
        let mut bases = Vec::new();
        for i in 0..400 {
            let (cx, cz) = (i % 20, i / 20);
            let c = gen.generate_chunk(cx, cz);
            for y in 1..HEIGHT {
                for z in 0..16 {
                    for x in 0..16 {
                        let b = c.get(x, y, z);
                        if is_log(b) && !is_branch(b) && log_axis(b) == 1 && !is_log(c.get(x, y - 1, z)) {
                            bases.push((cx * 16 + x as i32, cz * 16 + z as i32));
                        }
                    }
                }
            }
        }
        assert!(bases.len() > 50, "{} trees", bases.len());
        for (i, a) in bases.iter().enumerate() {
            for b in &bases[i + 1..] {
                let (dx, dz) = (a.0 - b.0, a.1 - b.1);
                assert!(dx * dx + dz * dz > 8, "trunks at {a:?} and {b:?}");
            }
        }
    }

    #[test]
    fn gen_speed() {
        let gen = Generator::new(12345);
        let start = std::time::Instant::now();
        for i in 0..64 {
            gen.generate_chunk(i % 8, i / 8);
        }
        let ms = start.elapsed().as_secs_f64() * 1000.0 / 64.0;
        println!("{ms:.2} ms per chunk");
    }

    /// No snow-capped mountains near deserts: within 96 blocks of desert, no column reaches
    /// the lowest snow line (126) unless it is warm enough to lift that line above it.
    #[test]
    fn no_snowy_peaks_by_desert() {
        for seed in [1, 7, 42, 12345, 99991] {
            let gen = Generator::new(seed);
            for gz in -80..80 {
                for gx in -80..80 {
                    let (x, z) = (gx * 48, gz * 48);
                    if gen.column(x, z).biome != Biome::Desert {
                        continue;
                    }
                    for dz in (-96..=96).step_by(16) {
                        for dx in (-96..=96).step_by(16) {
                            let c = gen.column(x + dx, z + dz);
                            let snow_line = 126 + (c.heat * 60.0) as i32;
                            assert!(
                                c.biome != Biome::Mountains || c.height <= snow_line,
                                "seed {seed}: snowy peak at {},{} (height {}, heat {}, temp {}) by desert at {x},{z}",
                                x + dx,
                                z + dz,
                                c.height,
                                c.heat,
                                c.temp
                            );
                        }
                    }
                }
            }
        }
    }

    /// Deserts never touch snowy biomes: on a coarse grid over several seeds, no desert
    /// column has a snowy one within 32 blocks.
    #[test]
    fn no_desert_next_to_snow() {
        let snowy = |b| matches!(b, Biome::SnowyPlains | Biome::SnowyTaiga);
        for seed in [1, 7, 42, 12345, 99991] {
            let gen = Generator::new(seed);
            let land = |x, z| {
                let (wx, wz) = gen.warped(x as f64, z as f64);
                gen.land_biome(wx, wz)
            };
            for gz in -150..150 {
                for gx in -150..150 {
                    let (x, z) = (gx * 32, gz * 32);
                    if land(x, z) != Biome::Desert {
                        continue;
                    }
                    for (dx, dz) in [(32, 0), (-32, 0), (0, 32), (0, -32)] {
                        assert!(
                            !snowy(land(x + dx, z + dz)),
                            "seed {seed}: desert at {x},{z} next to snow"
                        );
                    }
                }
            }
        }
    }
}
