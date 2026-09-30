//! The height and biome of a column (continents, mountain ranges, hills and rivers), and
//! the blocks its surface is made of.

use super::*;

#[derive(Clone, Copy)]
pub struct Column {
    pub height: i32,
    pub biome: Biome,
    pub temp: f32,
    /// How hot the climate is, 0 (temperate or colder) .. 1 (desert heat), ignoring altitude.
    pub heat: f32,
}

impl Generator {
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

    /// (top, filler, filler depth, deep filler, deep depth) of a column. `slope` is the height
    /// difference across the column in blocks per block: steep slopes show bare rock (stone
    /// cliffs, sandstone in deserts) and hold no snow, like in Minecraft.
    pub(super) fn surface_blocks(&self, col: &Column, x: i32, z: i32, slope: i32) -> (u8, u8, i32, u8, i32) {
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
}
