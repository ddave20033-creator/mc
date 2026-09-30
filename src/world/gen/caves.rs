//! Caves: spaghetti tunnels and cheese caverns carved out underground.

use super::*;

impl Generator {
    /// Is this underground cell carved out as a cave?
    pub(super) fn carved(&self, x: i32, y: i32, z: i32, surface: i32, wet: bool) -> bool {
        if y <= 1 || (wet && y > surface - 9) {
            return false;
        }
        let (fx, fy, fz) = (x as f64, y as f64, z as f64);
        // Spaghetti tunnels: intersection of two noise "zero sheets".
        let a = self.cave_a.noise3(fx / 72.0, fy / 44.0, fz / 72.0);
        let b = self.cave_b.noise3(fx / 72.0, fy / 44.0, fz / 72.0);
        // Thin near the surface, a little wider deep down where the rarer ores are.
        let width = if y > surface - 5 {
            0.0025
        } else {
            0.0045 + 0.0015 * ((40 - y) as f64 / 30.0).clamp(0.0, 1.0)
        };
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
}
