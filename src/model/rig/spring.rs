//! Damped springs for procedural animation: a value pulled toward a target that overshoots a
//! little and settles, the way a gun's weight moves in the hands (sway, recoil, the lean into
//! a strafe, the bounce of a landing, going in and out of poses).

use glam::Vec3;

/// Longest step the springs are integrated with (they stay stable at any frame rate).
const STEP: f32 = 1.0 / 240.0;

/// A damped spring on a 3D value (a position, or three angles).
#[derive(Clone, Copy, Default, Debug)]
pub struct Spring3 {
    pub x: Vec3,
    pub v: Vec3,
}

impl Spring3 {
    /// Moves toward `target` for `dt` seconds, oscillating `freq` times a second with damping
    /// ratio `zeta` (1: no overshoot; lower overshoots more). Returns the new value.
    pub fn step(&mut self, target: Vec3, freq: f32, zeta: f32, dt: f32) -> Vec3 {
        let w = std::f32::consts::TAU * freq;
        let n = (dt / STEP).ceil().clamp(1.0, 64.0) as usize;
        let h = dt / n as f32;
        for _ in 0..n {
            let a = (target - self.x) * (w * w) - self.v * (2.0 * zeta * w);
            self.v += a * h;
            self.x += self.v * h;
        }
        self.x
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_underdamped_spring_overshoots_and_settles() {
        let mut s = Spring3::default();
        let mut peak: f32 = 0.0;
        for _ in 0..600 {
            peak = peak.max(s.step(Vec3::X, 4.0, 0.4, 1.0 / 60.0).x);
        }
        assert!(peak > 1.1, "no overshoot: {peak}");
        assert!((s.x.x - 1.0).abs() < 1e-3 && s.v.length() < 1e-2, "{s:?}");
    }

    #[test]
    fn a_critically_damped_spring_does_not_overshoot() {
        let mut s = Spring3::default();
        for _ in 0..600 {
            assert!(s.step(Vec3::X, 4.0, 1.0, 1.0 / 60.0).x <= 1.0 + 1e-4);
        }
    }

    #[test]
    fn a_push_comes_back_to_rest_at_any_frame_rate() {
        for dt in [1.0 / 30.0, 1.0 / 144.0, 0.1] {
            let mut s = Spring3 { x: Vec3::ZERO, v: Vec3::new(0.0, 5.0, 0.0) };
            for _ in 0..(4.0 / dt) as usize {
                s.step(Vec3::ZERO, 6.0, 0.5, dt);
            }
            assert!(s.x.length() < 1e-3, "dt {dt}: {:?}", s.x);
        }
    }
}
