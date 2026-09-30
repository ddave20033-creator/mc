//! Climate and biomes: temperature and humidity, and the biome cells land is split into.

use super::*;

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

impl Generator {
    /// Domain warp: large features are sampled at bent coordinates so coastlines, mountain
    /// ranges, rivers and biome borders meander instead of forming round blobs.
    pub(super) fn warped(&self, x: f64, z: f64) -> (f64, f64) {
        let (u, v) = (x / 1000.0, z / 1000.0);
        let dx = self.warp.fbm2(u, v, 3) * 300.0 + self.warp.fbm2(x / 190.0, z / 190.0, 2) * 40.0;
        let dz = self.warp.fbm2(u + 31.7, v - 11.3, 3) * 300.0
            + self.warp.fbm2(x / 190.0 - 7.1, z / 190.0 + 3.9, 2) * 40.0;
        (x + dx, z + dz)
    }

    /// Temperature and humidity (0..1) at warped coordinates, with a little fine noise so
    /// biome borders are ragged rather than smooth curves.
    pub(super) fn climate_at(&self, wx: f64, wz: f64, x: f64, z: f64) -> (f32, f32) {
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
    pub(super) fn land_biome(&self, wx: f64, wz: f64) -> Biome {
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
}
