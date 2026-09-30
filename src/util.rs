//! Small helpers shared across the game: a fast random number generator, angles, easing,
//! vertex light values and ray/box intersection.

use glam::Vec3;
use std::f32::consts::{PI, TAU};

/// Xorshift random numbers: fast and good enough for gameplay and effects (not for secrets).
#[derive(Clone, Copy, Debug)]
pub struct Rng(u32);

impl Rng {
    pub fn new(seed: u32) -> Self {
        Self(seed | 1)
    }

    /// Uniform in 0..1.
    pub fn next(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 17;
        self.0 ^= self.0 << 5;
        (self.0 >> 8) as f32 / (1u32 << 24) as f32
    }
}

/// A file or folder name Windows accepts: its device names (CON, NUL, COM1...) get a `_`
/// in front (a world or a player called so would not be saved at all).
pub fn windows_safe(name: String) -> String {
    let stem = name.split('.').next().unwrap_or("").trim_end().to_ascii_uppercase();
    let device = matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || (stem.len() == 4
            && (stem.starts_with("COM") || stem.starts_with("LPT"))
            && stem.as_bytes()[3].is_ascii_digit());
    if device {
        format!("_{name}")
    } else {
        name
    }
}

/// How far something easing toward a target at `rate` (per second) gets in `dt` seconds (0..1),
/// the same whatever the frame rate: `x += (target - x) * damp(rate, dt)`.
#[inline]
pub fn damp(rate: f32, dt: f32) -> f32 {
    1.0 - (-rate * dt).exp()
}

/// An angle wrapped into -PI..PI.
pub fn wrap_angle(a: f32) -> f32 {
    (a + PI).rem_euclid(TAU) - PI
}

/// Blends angle `a` toward `b` by `k` the short way round.
pub fn lerp_angle(a: f32, b: f32, k: f32) -> f32 {
    a + wrap_angle(b - a) * k
}

pub fn smoothstep(a: f32, b: f32, x: f32) -> f32 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// `Vertex::light` for things drawn outside chunk meshes: sky and block light (0..15),
/// no ambient occlusion; the normal index (last byte) is filled in per face.
pub fn vertex_light(sky: u8, blk: u8) -> [u8; 4] {
    [255, sky.min(15) * 17, blk.min(15) * 17, 0]
}

/// Distance along a ray to where it enters the box `min`..`max`, if within `max_dist`.
pub fn ray_box(origin: Vec3, dir: Vec3, min: Vec3, max: Vec3, max_dist: f32) -> Option<f32> {
    let (mut t0, mut t1) = (0.0f32, max_dist);
    for a in 0..3 {
        if dir[a].abs() < 1e-8 {
            if origin[a] < min[a] || origin[a] > max[a] {
                return None;
            }
            continue;
        }
        let inv = 1.0 / dir[a];
        let (mut ta, mut tb) = ((min[a] - origin[a]) * inv, (max[a] - origin[a]) * inv);
        if ta > tb {
            std::mem::swap(&mut ta, &mut tb);
        }
        t0 = t0.max(ta);
        t1 = t1.min(tb);
        if t0 > t1 {
            return None;
        }
    }
    Some(t0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rng_stays_in_range_and_angles_wrap() {
        let mut r = Rng::new(0);
        for _ in 0..10_000 {
            let v = r.next();
            assert!((0.0..1.0).contains(&v));
        }
        assert!(
            (wrap_angle(3.0 * PI) - PI).abs() < 1e-4 || (wrap_angle(3.0 * PI) + PI).abs() < 1e-4
        );
        assert!((lerp_angle(PI - 0.1, -PI + 0.1, 0.5).abs() - PI).abs() < 1e-4);
        assert_eq!(
            ray_box(
                Vec3::ZERO,
                Vec3::X,
                Vec3::new(2.0, -1.0, -1.0),
                Vec3::new(3.0, 1.0, 1.0),
                5.0
            ),
            Some(2.0)
        );
    }

    #[test]
    fn windows_device_names_are_made_safe() {
        assert_eq!(windows_safe("CON".into()), "_CON");
        assert_eq!(windows_safe("nul".into()), "_nul");
        assert_eq!(windows_safe("com1".into()), "_com1");
        assert_eq!(windows_safe("Console".into()), "Console");
        assert_eq!(windows_safe("COMx".into()), "COMx");
    }
}
