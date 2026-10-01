//! Perlin noise (2D and 3D) and fractal sums of it, for the terrain.

/// Classic Perlin gradient noise (2D and 3D) with a seeded permutation table.
pub struct Perlin {
    perm: [u8; 512],
}

impl Perlin {
    pub fn new(seed: u32) -> Self {
        let mut p: [u8; 256] = std::array::from_fn(|i| i as u8);
        let mut s = (seed as u64).wrapping_mul(0x2545_F491_4F6C_DD1D) ^ 0x9E37_79B9_7F4A_7C15;
        for i in (1..256).rev() {
            s ^= s << 13;
            s ^= s >> 7;
            s ^= s << 17;
            let j = (s % (i as u64 + 1)) as usize;
            p.swap(i, j);
        }
        let mut perm = [0u8; 512];
        for (i, v) in perm.iter_mut().enumerate() {
            *v = p[i & 255];
        }
        Self { perm }
    }

    pub fn noise2(&self, x: f64, y: f64) -> f64 {
        let (xf, yf) = (x.floor(), y.floor());
        let xi = (xf as i64 & 255) as usize;
        let yi = (yf as i64 & 255) as usize;
        let (x, y) = (x - xf, y - yf);
        let (u, v) = (fade(x), fade(y));
        let p = &self.perm;
        let aa = p[p[xi] as usize + yi];
        let ab = p[p[xi] as usize + yi + 1];
        let ba = p[p[xi + 1] as usize + yi];
        let bb = p[p[xi + 1] as usize + yi + 1];
        lerp(
            v,
            lerp(u, grad2(aa, x, y), grad2(ba, x - 1.0, y)),
            lerp(u, grad2(ab, x, y - 1.0), grad2(bb, x - 1.0, y - 1.0)),
        )
    }

    pub fn noise3(&self, x: f64, y: f64, z: f64) -> f64 {
        let (xf, yf, zf) = (x.floor(), y.floor(), z.floor());
        let xi = (xf as i64 & 255) as usize;
        let yi = (yf as i64 & 255) as usize;
        let zi = (zf as i64 & 255) as usize;
        let (x, y, z) = (x - xf, y - yf, z - zf);
        let (u, v, w) = (fade(x), fade(y), fade(z));
        let p = &self.perm;
        let a = p[xi] as usize + yi;
        let aa = p[a] as usize + zi;
        let ab = p[a + 1] as usize + zi;
        let b = p[xi + 1] as usize + yi;
        let ba = p[b] as usize + zi;
        let bb = p[b + 1] as usize + zi;
        lerp(
            w,
            lerp(
                v,
                lerp(u, grad3(p[aa], x, y, z), grad3(p[ba], x - 1.0, y, z)),
                lerp(
                    u,
                    grad3(p[ab], x, y - 1.0, z),
                    grad3(p[bb], x - 1.0, y - 1.0, z),
                ),
            ),
            lerp(
                v,
                lerp(
                    u,
                    grad3(p[aa + 1], x, y, z - 1.0),
                    grad3(p[ba + 1], x - 1.0, y, z - 1.0),
                ),
                lerp(
                    u,
                    grad3(p[ab + 1], x, y - 1.0, z - 1.0),
                    grad3(p[bb + 1], x - 1.0, y - 1.0, z - 1.0),
                ),
            ),
        )
    }

    /// Fractal Brownian motion, normalized by the amplitude sum.
    pub fn fbm2(&self, x: f64, y: f64, octaves: u32) -> f64 {
        let (mut sum, mut amp, mut freq, mut norm) = (0.0, 1.0, 1.0, 0.0);
        for _ in 0..octaves {
            sum += amp * self.noise2(x * freq, y * freq);
            norm += amp;
            amp *= 0.5;
            freq *= 2.0;
        }
        sum / norm
    }

    pub fn fbm3(&self, x: f64, y: f64, z: f64, octaves: u32) -> f64 {
        let (mut sum, mut amp, mut freq, mut norm) = (0.0, 1.0, 1.0, 0.0);
        for _ in 0..octaves {
            sum += amp * self.noise3(x * freq, y * freq, z * freq);
            norm += amp;
            amp *= 0.5;
            freq *= 2.0;
        }
        sum / norm
    }
}

fn fade(t: f64) -> f64 {
    t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
}

fn lerp(t: f64, a: f64, b: f64) -> f64 {
    a + t * (b - a)
}

fn grad2(h: u8, x: f64, y: f64) -> f64 {
    match h & 7 {
        0 => x + y,
        1 => -x + y,
        2 => x - y,
        3 => -x - y,
        4 => x,
        5 => -x,
        6 => y,
        _ => -y,
    }
}

fn grad3(h: u8, x: f64, y: f64, z: f64) -> f64 {
    let h = h & 15;
    let u = if h < 8 { x } else { y };
    let v = if h < 4 {
        y
    } else if h == 12 || h == 14 {
        x
    } else {
        z
    };
    (if h & 1 == 0 { u } else { -u }) + (if h & 2 == 0 { v } else { -v })
}
