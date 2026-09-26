//! Deterministic terrain generation: every block is a pure function of (seed, x, y, z),
//! so chunks can be generated independently on worker threads.

use super::noise::Perlin;
use super::*;

pub const SEA: i32 = 62;
// Biome sizes. Land is split into biome cells (irregular, about BIOME_CELL blocks: the
// smallest biome patch). Each cell takes one biome from the climate at its center, shifted by
// a random "roll" that varies smoothly over ZONE_SCALE blocks, so neighbouring cells tend to
// roll alike and merge into larger patches. Per climate zone (snowy, cold, temperate, hot):
// a larger scale gives larger biomes. Measured with `rustcraft --sizes` (6 seeds), a spot of
// a biome typically lies in a patch of about (square root of the area, in blocks)
//   plains 970, forest 600, birch forest 430, desert 1000, taiga 560,
//   snowy plains 860, snowy taiga 1000, mountain ranges 490;
// single patches vary widely around that (10%..90%: roughly 0.15x .. 1.1x of it).
const BIOME_CELL: f64 = 420.0;
const ZONE_SCALE: [f64; 4] = [1200.0, 1500.0, 800.0, 1300.0];
/// Strength of the smooth roll, and of an extra per-cell random part (variety).
const ROLL: f64 = 0.55;
const CELL_JITTER: f64 = 0.12;
/// Birch woods inside forests use their own, smaller cells.
const BIRCH_CELL: f64 = 470.0;
/// Highest per-column tree chance of any biome (see `place_trees`).
const MAX_TREE_CHANCE: f64 = 0.075;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Biome {
    Ocean,
    FrozenOcean,
    River,
    Beach,
    /// Rocky coast where hills or mountains meet the sea.
    StonyShore,
    Plains,
    Forest,
    BirchForest,
    Desert,
    Taiga,
    SnowyTaiga,
    SnowyPlains,
    Mountains,
}

impl Biome {
    pub fn name(self) -> &'static str {
        match self {
            Biome::Ocean => "Ocean",
            Biome::FrozenOcean => "Frozen Ocean",
            Biome::River => "River",
            Biome::Beach => "Beach",
            Biome::StonyShore => "Stony Shore",
            Biome::Plains => "Plains",
            Biome::Forest => "Forest",
            Biome::BirchForest => "Birch Forest",
            Biome::Desert => "Desert",
            Biome::Taiga => "Taiga",
            Biome::SnowyTaiga => "Snowy Taiga",
            Biome::SnowyPlains => "Snowy Plains",
            Biome::Mountains => "Mountains",
        }
    }
}

#[derive(Clone, Copy)]
pub struct Column {
    pub height: i32,
    pub biome: Biome,
    pub temp: f32,
    /// How hot the climate is, 0 (temperate or colder) .. 1 (desert heat), ignoring altitude.
    pub heat: f32,
}

#[derive(Clone, Copy)]
enum Tree {
    Oak,
    Birch,
    Spruce,
    Cactus,
}

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

fn lerp3(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    let t = t.clamp(0.0, 1.0);
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
    ]
}

fn to_u8(c: [f32; 3]) -> [u8; 3] {
    c.map(|v| (v.clamp(0.0, 1.0) * 255.0) as u8)
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

    /// Domain warp: large features are sampled at bent coordinates so coastlines, mountain
    /// ranges, rivers and biome borders meander instead of forming round blobs.
    fn warped(&self, x: f64, z: f64) -> (f64, f64) {
        let (u, v) = (x / 1000.0, z / 1000.0);
        let dx = self.warp.fbm2(u, v, 3) * 300.0 + self.warp.fbm2(x / 190.0, z / 190.0, 2) * 40.0;
        let dz = self.warp.fbm2(u + 31.7, v - 11.3, 3) * 300.0
            + self.warp.fbm2(x / 190.0 - 7.1, z / 190.0 + 3.9, 2) * 40.0;
        (x + dx, z + dz)
    }

    /// Temperature and humidity (0..1) at warped coordinates, with a little fine noise so
    /// biome borders are ragged rather than smooth curves.
    fn climate_at(&self, wx: f64, wz: f64, x: f64, z: f64) -> (f32, f32) {
        let fine = |n: &Perlin, o: f64| n.noise2(x / 70.0 + o, z / 70.0 - o) * 0.035;
        let t = (self.temp.fbm2(wx / 2700.0, wz / 2700.0, 3) * 1.9 + 0.5 + fine(&self.temp, 5.3))
            .clamp(0.0, 1.0);
        let h = (self.humid.fbm2(wx / 1600.0 + 71.0, wz / 1600.0 - 17.0, 3) * 1.8
            + 0.5
            + fine(&self.humid, 9.1))
        .clamp(0.0, 1.0);
        (t as f32, h as f32)
    }

    /// Center of grid cell (cx, cz) of a jittered grid with cells of `size` blocks.
    fn cell_point(&self, cx: i32, cz: i32, size: f64, salt: u32) -> (f64, f64) {
        let jitter = |k| 0.1 + 0.8 * hash(self.seed ^ salt, cx, k, cz);
        (
            (cx as f64 + jitter(0)) * size,
            (cz as f64 + jitter(1)) * size,
        )
    }

    /// Nearest point of a jittered grid with cells of about `size` blocks (a Voronoi cell):
    /// returns the cell's grid index, its center and a random value 0..1 belonging to it.
    fn cell_at(&self, x: f64, z: f64, size: f64, salt: u32) -> ((i32, i32), f64, f64, f64) {
        let (gx, gz) = ((x / size).floor() as i32, (z / size).floor() as i32);
        let mut best = (f64::MAX, (0, 0), 0.0, 0.0);
        for dz in -1..=1 {
            for dx in -1..=1 {
                let (cx, cz) = (gx + dx, gz + dz);
                let (px, pz) = self.cell_point(cx, cz, size, salt);
                let d = (px - x).powi(2) + (pz - z).powi(2);
                if d < best.0 {
                    best = (d, (cx, cz), px, pz);
                }
            }
        }
        let (cx, cz) = best.1;
        (best.1, best.2, best.3, hash(self.seed ^ salt, cx, 2, cz))
    }

    /// Like `cell_at`, without the grid index.
    fn cell(&self, x: f64, z: f64, size: f64, salt: u32) -> (f64, f64, f64) {
        let (_, px, pz, r) = self.cell_at(x, z, size, salt);
        (px, pz, r)
    }

    /// Climate zone of a temperature: 0 snowy, 1 cold, 2 temperate, 3 hot.
    fn zone(t: f64) -> usize {
        match t {
            t if t < 0.2 => 0,
            t if t < 0.39 => 1,
            t if t <= 0.64 => 2,
            _ => 3,
        }
    }

    /// Raw climate zone of a biome cell (from the temperature at its center).
    fn cell_zone(&self, cx: i32, cz: i32) -> usize {
        let (px, pz) = self.cell_point(cx, cz, BIOME_CELL, 0xB10E);
        Self::zone(self.climate_at(px, pz, px, pz).0 as f64)
    }

    /// Land biome from biome cells: every cell (about BIOME_CELL blocks) takes one biome from
    /// the climate at its center plus a small random roll, so biomes come in patches of a
    /// predictable size (neighbouring cells with the same biome merge into bigger ones),
    /// while the smooth climate keeps sensible neighbours (no desert next to snow).
    fn land_biome(&self, wx: f64, wz: f64) -> Biome {
        // Extra small-scale warp for ragged, natural borders between cells.
        // A medium-scale bend hides the straight cell edges, a small one roughens them
        // (gentle enough that it never folds and cuts off little islands).
        let bend = |ox: f64, oz: f64| {
            self.warp.fbm2(wx / 260.0 + ox, wz / 260.0 + oz, 2) * 90.0
                + self.warp.noise2(wx / 90.0 + ox, wz / 90.0 + oz) * 20.0
        };
        let (bx, bz) = (wx + bend(0.0, 0.0), wz + bend(19.1, -7.7));
        let ((gx, gz), cx, cz, jitter) = self.cell_at(bx, bz, BIOME_CELL, 0xB10E);
        let (t, hu) = self.climate_at(cx, cz, cx, cz);
        let t = t as f64;
        let mut zone = Self::zone(t);
        // Where the climate changes quickly, a hot cell could touch a cold or snowy one
        // (desert right next to taiga or snow). Such extreme cells step one zone toward the
        // middle instead (hot becomes temperate, snowy becomes cold), so neighbouring cells
        // always differ by at most one zone.
        if zone == 0 || zone == 3 {
            let far = (-1..=1)
                .flat_map(|dz| (-1..=1).map(move |dx| (dx, dz)))
                .filter(|&d| d != (0, 0))
                .any(|(dx, dz)| self.cell_zone(gx + dx, gz + dz).abs_diff(zone) >= 2);
            if far {
                zone = if zone == 0 { 1 } else { 2 };
            }
        }
        let scale = ZONE_SCALE[zone];
        let roll = self.humid.noise2(cx / scale + 300.0, cz / scale - 120.0);
        let wet = hu as f64 + roll * ROLL + (jitter - 0.5) * CELL_JITTER;
        if zone == 3 && wet < 0.5 {
            Biome::Desert
        } else if zone == 0 {
            if wet > 0.62 {
                Biome::SnowyTaiga
            } else {
                Biome::SnowyPlains
            }
        } else if zone == 1 {
            // Cold country is taiga where it is wet enough, open plains elsewhere.
            if wet > 0.45 {
                Biome::Taiga
            } else {
                Biome::Plains
            }
        } else if wet > 0.58 {
            // Birch woods: smaller cells inside the forests, more of them where it is cooler.
            let (_, _, birch) = self.cell(bx, bz, BIRCH_CELL, 0xB1C4);
            if birch < if t < 0.5 { 0.55 } else { 0.2 } {
                Biome::BirchForest
            } else {
                Biome::Forest
            }
        } else {
            Biome::Plains
        }
    }

    /// The land biome region a column belongs to, ignoring rivers and shores cutting through
    /// it (for measuring biome sizes).
    pub fn biome_region(&self, x: i32, z: i32) -> Biome {
        let c = self.column(x, z);
        match c.biome {
            Biome::River | Biome::Beach | Biome::StonyShore => {
                let (wx, wz) = self.warped(x as f64, z as f64);
                self.land_biome(wx, wz)
            }
            b => b,
        }
    }

    pub fn climate(&self, x: i32, z: i32) -> (f32, f32) {
        let (fx, fz) = (x as f64, z as f64);
        let (wx, wz) = self.warped(fx, fz);
        self.climate_at(wx, wz, fx, fz)
    }

    /// Distance in blocks from (wx, wz) to the nearest river center line. Rivers are the zero
    /// lines of a smooth noise field; dividing by its gradient turns the value into a distance,
    /// so rivers keep an even width instead of swelling into blobs where the noise is flat.
    fn river_distance(&self, wx: f64, wz: f64) -> f64 {
        let s = 1000.0;
        let n = |x: f64, z: f64| self.river.fbm2(x / s, z / s, 2);
        let e = 4.0;
        let v = n(wx, wz);
        let gx = (n(wx + e, wz) - n(wx - e, wz)) / (2.0 * e);
        let gz = (n(wx, wz + e) - n(wx, wz - e)) / (2.0 * e);
        v.abs() / (gx * gx + gz * gz).sqrt().max(1e-7)
    }

    pub fn column(&self, x: i32, z: i32) -> Column {
        let (fx, fz) = (x as f64, z as f64);
        let (wx, wz) = self.warped(fx, fz);

        // Continents and erosion (flat vs rugged land), both on warped coordinates.
        let c = (self.cont.fbm2(wx / 1600.0, wz / 1600.0, 5) * 1.9 + 0.08).clamp(-1.0, 1.0);
        let e = (self.erosion.fbm2(wx / 800.0 + 91.0, wz / 800.0 - 33.0, 4) * 1.9).clamp(-1.0, 1.0);
        let base = spline(
            c,
            &[
                (-1.0, 30.0),
                (-0.5, 40.0),
                (-0.25, 50.0),
                (-0.1, 58.0),
                (-0.02, 63.0),
                (0.12, 66.0),
                (0.4, 70.0),
                (1.0, 80.0),
            ],
        );
        let rough = smoothstep(0.25, -0.55, e);
        // Wide, almost level lowlands where erosion is high (Minecraft's flat plains).
        let flat = smoothstep(-0.1, 0.35, e);
        let land = smoothstep(-0.12, 0.02, c);
        let inland = smoothstep(0.02, 0.3, c);
        let (t, hu) = self.climate_at(wx, wz, fx, fz);
        // Hot and dry: deserts get gentle dunes and (almost) no rivers.
        let dry = smoothstep(0.58, 0.72, t as f64) * smoothstep(0.5, 0.35, hu as f64);

        // Mountain ranges: long winding belts (along the zero lines of a large-scale noise),
        // with ridges (zero lines of a smaller noise) and side spurs inside them.
        // Only some belts actually rise into mountains; the others stay hilly country.
        let belt = 1.0 - self.ranges.fbm2(wx / 3200.0, wz / 3200.0, 2).abs() * 3.8;
        let raised = smoothstep(
            -0.2,
            0.12,
            self.ranges.fbm2(wx / 5000.0 + 40.0, wz / 5000.0, 2),
        );
        let zone = smoothstep(0.2, 0.8, belt) * raised * inland;
        // Foothills: hilly country along the belts, mostly where they rise into mountains.
        let foothills = smoothstep(0.0, 0.7, belt) * inland * (0.35 + 0.65 * raised);
        let ridge = (1.0 - self.peaks.fbm2(wx / 700.0, wz / 700.0, 3).abs() * 2.0).max(0.0);
        let spur = (1.0 - self.spurs.fbm2(fx / 190.0, fz / 190.0, 3).abs() * 2.2).max(0.0);
        // Toward hot country the ranges sink into low, bare ridges, so deserts never sit under
        // tall snowy peaks.
        let heat = smoothstep(0.4, 0.6, t as f64);
        let mountain = zone
            * (ridge.powi(3) * 115.0 + ridge * spur.powi(2) * 32.0)
            * (0.6 + 0.4 * rough)
            * (1.0 - 0.7 * heat);

        // Hills: gentle on the plains, rolling in rugged areas, strongest near the ranges.
        let hills = self.hills.fbm2(fx / 180.0, fz / 180.0, 5)
            * (2.5 + 5.0 * (1.0 - flat) + 10.0 * rough * rough + 14.0 * foothills + 10.0 * zone)
            * (1.0 - 0.4 * dry)
            + self.detail.fbm2(fx / 45.0, fz / 45.0, 3) * (2.5 - 1.2 * flat);
        // Hill country sits a little higher than the plains around it.
        let mut h = base + hills * (0.3 + 0.7 * land) + foothills * rough * 10.0 + mountain;

        // Rivers: an even-width channel (a bit wider in the lowlands) in a valley with gently
        // sloping banks. They start in the mountains (fading out on the high ridges).
        let dist = self.river_distance(wx, wz);
        let width =
            3.0 + 3.5 * (1.0 - inland * 0.5) + self.detail.noise2(fx / 260.0, fz / 260.0) * 1.5;
        let fade = smoothstep(70.0, 20.0, mountain) * land * (1.0 - dry);
        let valley = smoothstep(width + 34.0, width, dist) * fade;
        // The channel itself only exists where the valley is deep enough to hold water;
        // higher up only a broad valley (a pass) remains instead of a narrow trench.
        let channel = smoothstep(width + 1.0, width - 1.0, dist) * smoothstep(0.6, 0.95, fade);
        if h > SEA as f64 - 1.0 {
            // The valley pulls the land down toward the water level; the channel cuts below it.
            let floor = SEA as f64 + 1.0;
            h = lerp(h, floor.min(h), valley.powf(1.5));
            h = lerp(h, SEA as f64 - 2.0 - 2.0 * channel, channel);
        }
        let height = (h.round() as i32).clamp(4, HEIGHT as i32 - 8);
        let temp = t - (height - SEA - 20).max(0) as f32 * 0.004;

        let biome = if channel > 0.5 && height < SEA {
            Biome::River
        } else if height < SEA - 1 {
            if temp + self.detail.noise2(fx / 24.0, fz / 24.0) as f32 * 0.08 < 0.2 {
                Biome::FrozenOcean
            } else {
                Biome::Ocean
            }
        } else if dist < width + 2.5
            && height <= SEA + 1
            && fade > 0.3
            && self.detail.noise2(fx / 40.0 + 13.0, fz / 40.0) > 0.15
        {
            // Sandy stretches of river bank; elsewhere the grass reaches the water.
            Biome::River
        } else if height <= SEA + 1 && c < self.detail.noise2(fx / 90.0, fz / 90.0) * 0.03 - 0.01 {
            // Only the strip of land right at the sea: sandy, or rocky where hills meet it.
            if mountain > 8.0 || rough > 0.8 {
                Biome::StonyShore
            } else {
                Biome::Beach
            }
        } else if height > 120 || mountain > 38.0 {
            Biome::Mountains
        } else {
            self.land_biome(wx, wz)
        };
        Column {
            height,
            biome,
            temp,
            heat: heat as f32,
        }
    }

    /// Is this underground cell carved out as a cave?
    fn carved(&self, x: i32, y: i32, z: i32, surface: i32, wet: bool) -> bool {
        if y <= 1 || (wet && y > surface - 9) {
            return false;
        }
        let (fx, fy, fz) = (x as f64, y as f64, z as f64);
        // Spaghetti tunnels: intersection of two noise "zero sheets".
        let a = self.cave_a.noise3(fx / 72.0, fy / 44.0, fz / 72.0);
        let b = self.cave_b.noise3(fx / 72.0, fy / 44.0, fz / 72.0);
        let width = if y > surface - 5 { 0.0025 } else { 0.0045 };
        if a * a + b * b < width {
            return true;
        }
        // Cheese caverns deep underground.
        if y < 48 && y < surface - 16 {
            let c = self.cave_c.fbm3(fx / 110.0, fy / 60.0, fz / 110.0, 2);
            if c > 0.3 {
                return true;
            }
        }
        false
    }

    /// (top, filler, filler depth, deep filler, deep depth) of a column. `slope` is the height
    /// difference across the column in blocks per block: steep slopes show bare rock (stone
    /// cliffs, sandstone in deserts) and hold no snow, like in Minecraft.
    fn surface_blocks(&self, col: &Column, x: i32, z: i32, slope: i32) -> (u8, u8, i32, u8, i32) {
        let r = hash(self.seed, x, 7, z);
        let rock = if r < 0.15 {
            (GRAVEL, STONE, 1, STONE, 0)
        } else {
            (STONE, STONE, 1, STONE, 0)
        };
        match col.biome {
            // River beds: sand with patches of gravel and clay.
            Biome::River if col.height < SEA => match r {
                r if r < 0.25 => (GRAVEL, GRAVEL, 2, STONE, 0),
                r if r < 0.35 => (CLAY, CLAY, 2, STONE, 0),
                _ => (SAND, SAND, 3, STONE, 0),
            },
            Biome::Ocean | Biome::FrozenOcean | Biome::River => {
                if col.height >= SEA - 5 {
                    (SAND, SAND, 3, STONE, 0)
                } else if r < 0.2 {
                    (CLAY, CLAY, 2, STONE, 0)
                } else {
                    (GRAVEL, GRAVEL, 3, STONE, 0)
                }
            }
            Biome::Beach => (SAND, SAND, 4, SANDSTONE, 3),
            Biome::StonyShore => rock,
            Biome::Desert if slope >= 3 => (SANDSTONE, SANDSTONE, 3, STONE, 0),
            Biome::Desert => (SAND, SAND, 4, SANDSTONE, 5),
            Biome::Mountains => {
                let snow_line = 126 + (r * 10.0) as i32 + slope * 3 + (col.heat * 60.0) as i32;
                if col.height > snow_line && slope <= 3 {
                    (SNOW, STONE, 1, STONE, 0)
                } else if slope >= 3 || col.height > 112 {
                    rock
                } else {
                    (GRASS, DIRT, 3, STONE, 0)
                }
            }
            _ if slope >= 4 => rock,
            Biome::SnowyPlains | Biome::SnowyTaiga => (SNOWY_GRASS, DIRT, 3, STONE, 0),
            _ => (GRASS, DIRT, 3 + (r * 2.0) as i32, STONE, 0),
        }
    }

    /// Steepest height difference around a column (blocks per block), from its neighbours.
    fn slope_at(&self, x: i32, z: i32) -> i32 {
        let h = |x, z| self.column(x, z).height;
        let dx = (h(x + 1, z) - h(x - 1, z)).abs();
        let dz = (h(x, z + 1) - h(x, z - 1)).abs();
        (dx.max(dz) + 1) / 2
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

    fn place_ores(&self, c: &mut ChunkData, cx: i32, cz: i32) {
        let mut rng = Rng(((hash(self.seed ^ 0x0E5, cx, 0, cz) * 1e9) as u64) | 1);
        let ores: [(u8, u32, i32, i32, u32); 4] = [
            (COAL_ORE, 20, 5, 130, 9),
            (IRON_ORE, 14, 5, 64, 7),
            (GOLD_ORE, 3, 5, 32, 6),
            (DIAMOND_ORE, 2, 5, 16, 5),
        ];
        for (ore, tries, ymin, ymax, size) in ores {
            for _ in 0..tries {
                let (mut x, mut y, mut z) =
                    (rng.range(0, 16), rng.range(ymin, ymax), rng.range(0, 16));
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

    fn place_plants(&self, c: &mut ChunkData, x0: i32, z0: i32, cols: &[[Option<Column>; 16]; 16]) {
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

    fn place_trees(&self, c: &mut ChunkData, x0: i32, z0: i32, cols: &[[Option<Column>; 16]; 16]) {
        for wz in z0 - 3..z0 + 19 {
            for wx in x0 - 3..x0 + 19 {
                // Cheap rejection first: no biome has a tree chance above MAX_TREE_CHANCE, so
                // most columns (especially the ones outside this chunk) skip `column()`.
                let tree_roll = hash(self.seed ^ 0x7EE5, wx, 0, wz);
                if tree_roll >= MAX_TREE_CHANCE {
                    continue;
                }
                let (lx, lz) = (wx - x0, wz - z0);
                let inside = (0..16).contains(&lx) && (0..16).contains(&lz);
                let col = if inside {
                    cols[lz as usize][lx as usize].unwrap()
                } else {
                    self.column(wx, wz)
                };
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
                if self.carved(wx, h, wz, h, false) || self.carved(wx, h - 1, wz, h, false) {
                    continue;
                }
                // Only on soil (or sand for cacti): not on cliffs, rock or snow.
                let top = self.surface_blocks(&col, wx, wz, self.slope_at(wx, wz)).0;
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
        let leaf_skip = |dx: i32, dy: i32, dz: i32| {
            hash(self.seed ^ 0xF00D, x + dx * 7, y + dy, z + dz * 13) < 0.5
        };
        match kind {
            Tree::Oak | Tree::Birch => {
                let (log, leaves) = if matches!(kind, Tree::Oak) {
                    (OAK_LOG, OAK_LEAVES)
                } else {
                    (BIRCH_LOG, BIRCH_LEAVES)
                };
                let trunk = 4 + (r * 3.0) as i32 + if matches!(kind, Tree::Birch) { 1 } else { 0 };
                put(c, x0, z0, x, y - 1, z, DIRT, false);
                let top = y + trunk - 1;
                for dy in -2..=1 {
                    let rad: i32 = if dy <= -1 { 2 } else { 1 };
                    for dx in -rad..=rad {
                        for dz in -rad..=rad {
                            let corner = dx.abs() == rad && dz.abs() == rad;
                            if corner && (dy == 1 || leaf_skip(dx, dy, dz)) {
                                continue;
                            }
                            put(c, x0, z0, x + dx, top + dy, z + dz, leaves, true);
                        }
                    }
                }
                for dy in 0..trunk {
                    put(c, x0, z0, x, y + dy, z, log, false);
                }
            }
            Tree::Spruce => {
                let trunk = 6 + (r * 4.0) as i32;
                put(c, x0, z0, x, y - 1, z, DIRT, false);
                let top = y + trunk;
                put(c, x0, z0, x, top, z, SPRUCE_LEAVES, true);
                for i in 1..trunk - 1 {
                    let ly = top - i;
                    let rad = if i % 2 == 1 { 1 } else { (1 + i / 3).min(3) };
                    for dx in -rad..=rad {
                        for dz in -rad..=rad {
                            if dx.abs() + dz.abs() > rad + 1
                                || (dx.abs() == rad && dz.abs() == rad && rad > 1)
                            {
                                continue;
                            }
                            put(c, x0, z0, x + dx, ly, z + dz, SPRUCE_LEAVES, true);
                        }
                    }
                }
                for dy in 0..trunk {
                    put(c, x0, z0, x, y + dy, z, SPRUCE_LOG, false);
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

    /// Grass and foliage tint colors (sRGB) for a column.
    pub fn tints(&self, x: i32, z: i32) -> ([u8; 3], [u8; 3]) {
        let (t, h) = self.climate(x, z);
        let cold = [0.50, 0.68, 0.56];
        let temperate = [0.47, 0.71, 0.31];
        let lush = [0.35, 0.68, 0.25];
        let dry = [0.74, 0.70, 0.38];
        let mid = lerp3(temperate, lush, h * 0.4);
        let g = if t < 0.45 {
            lerp3(cold, mid, t / 0.45)
        } else {
            lerp3(mid, lerp3(dry, lush, h), (t - 0.45) / 0.55)
        };
        let f = [g[0] * 0.8, g[1] * 0.86, g[2] * 0.76];
        (to_u8(g), to_u8(f))
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

    /// Prints the chunk generation speed (`cargo test --release gen_speed -- --nocapture`).
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
