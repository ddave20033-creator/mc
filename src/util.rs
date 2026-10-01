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

/// Following values sent 20 times a second (a mob, an item, another player) at an even pace:
/// each new one is reached from where it is drawn in a little over a tick, so the motion is a
/// steady glide. (An exponential chase rushes at each new value and slows before the next:
/// the motion pulses 20 times a second.) Not yet at it when the next comes, it goes on toward
/// that one from where it is; a value that comes late leaves it waiting a moment at the last.
#[derive(Clone, Copy, Debug, Default)]
pub struct Glide {
    /// How far through the way to the latest value (0..1).
    done: f32,
}

impl Glide {
    /// Seconds to reach a value (a tick and a fifth: a value a frame late does not stop it).
    const SPAN: f32 = 1.2 / crate::sim::clock::TICKS_PER_SECOND as f32;

    /// A new value came: the way to it starts from where it is drawn.
    pub fn restart(&mut self) {
        self.done = 0.0;
    }

    /// A frame of `dt` seconds: the share of what is left of the way to go now
    /// (`x += (target - x) * k`, like `damp`).
    pub fn step(&mut self, dt: f32) -> f32 {
        let left = 1.0 - self.done;
        if left <= 1e-6 {
            return 1.0;
        }
        self.done = (self.done + dt / Self::SPAN).min(1.0);
        1.0 - (1.0 - self.done) / left
    }
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

    #[test]
    fn a_glide_goes_at_an_even_pace_at_any_frame_rate() {
        // A value every tick, 1 further each time: drawn, it moves the same each frame (once
        // under way), at 60 and at 144 frames a second.
        for fps in [60.0f32, 144.0] {
            let dt = 1.0 / fps;
            let (mut glide, mut x, mut target, mut clock) = (Glide::default(), 0.0f32, 0.0f32, 0.0f32);
            let mut steps = Vec::new();
            for frame in 0..(fps as usize * 2) {
                let t = frame as f32 * dt;
                if t >= clock {
                    clock += 0.05;
                    target += 1.0;
                    glide.restart();
                }
                let was = x;
                x += (target - x) * glide.step(dt);
                if t > 0.5 {
                    steps.push((x - was) / dt);
                }
            }
            let (lo, hi) = steps.iter().fold((f32::MAX, 0.0f32), |(lo, hi), &v| (lo.min(v), hi.max(v)));
            assert!(lo > 14.0 && hi < 26.0, "at {fps} fps the speed went {lo}..{hi} (20 a second)");
            assert!(target - x < 1.5, "it keeps up");
        }
    }
}
