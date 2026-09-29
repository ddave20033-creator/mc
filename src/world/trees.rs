//! The trees' shapes: a round trunk, branches growing out of it and turning up, and round
//! crowns of leaves at their ends, different each time within limits. Used by the world
//! generator and by saplings growing.

use super::block::*;
use glam::IVec3;

/// How far a tree reaches from its trunk (blocks): the generator looks this far around a
/// chunk for trees reaching into it.
pub const REACH: i32 = 5;

/// A small random number generator (xorshift), so a tree is the same for the same seed.
struct Rng(u32);

impl Rng {
    fn next(&mut self) -> f32 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.0 = x;
        (x >> 8) as f32 / (1u32 << 24) as f32
    }
    fn range(&mut self, lo: i32, hi: i32) -> i32 {
        lo + (self.next() * (hi - lo + 1) as f32) as i32
    }
}

const DIRS: [IVec3; 4] = [IVec3::X, IVec3::NEG_X, IVec3::Z, IVec3::NEG_Z];

/// The blocks of a tree of `log` (oak, birch or spruce) standing at the origin: (offset,
/// block, only into air). Wood first, then the leaves, which do not replace it.
pub fn tree_shape(log: u8, seed: u32) -> Vec<(IVec3, u8, bool)> {
    let mut rng = Rng(seed.wrapping_mul(0x9E37_79B9) | 1);
    for _ in 0..3 {
        rng.next();
    }
    let leaves = match log {
        BIRCH_LOG => BIRCH_LEAVES,
        SPRUCE_LOG => SPRUCE_LEAVES,
        _ => OAK_LEAVES,
    };
    let mut wood: Vec<(IVec3, u8)> = Vec::new();
    let mut leaf: Vec<IVec3> = Vec::new();
    // A round clump of leaves around `c`: `rad` across, `tall` times as tall, its edge ragged.
    let blob = |rng: &mut Rng, leaf: &mut Vec<IVec3>, c: IVec3, rad: f32, tall: f32| {
        let n = rad.ceil() as i32 + 1;
        let ny = (rad * tall).ceil() as i32 + 1;
        for dy in -ny..=ny {
            for dz in -n..=n {
                for dx in -n..=n {
                    let d = ((dx * dx + dz * dz) as f32 + (dy as f32 / tall).powi(2)).sqrt();
                    if d + (rng.next() - 0.5) * 0.9 <= rad {
                        leaf.push(c + IVec3::new(dx, dy, dz));
                    }
                }
            }
        }
    };
    // A branch out of the trunk at height `y` toward `dir`: `len` blocks out, then `up`
    // blocks up; where it ends.
    let branch = |wood: &mut Vec<(IVec3, u8)>, y: i32, dir: IVec3, len: i32, up: i32| {
        let axis = if dir.x != 0 { 0 } else { 2 };
        let mut p = IVec3::new(0, y, 0);
        for _ in 0..len {
            p += dir;
            wood.push((p, branch_with_axis(log, axis)));
        }
        for _ in 0..up {
            p += IVec3::Y;
            wood.push((p, branch_with_axis(log, 1)));
        }
        p
    };
    let mut dirs = DIRS;
    for i in (1..4).rev() {
        dirs.swap(i, rng.range(0, i as i32) as usize);
    }
    match log {
        SPRUCE_LOG => {
            // A cone: the trunk up the middle, rings of level branches with leaves on
            // them, smaller toward the top.
            let h = rng.range(8, 11);
            for y in 0..h {
                wood.push((IVec3::new(0, y, 0), log));
            }
            let top = h + 1;
            for y in 2..=top {
                let k = (top - y) as f32 / (top - 2) as f32;
                let wide = (y - top) % 2 == 0;
                let rad = 0.6 + k * if wide { 3.2 } else { 2.3 };
                if wide && rad >= 1.6 && y < h - 1 {
                    let len = ((rad - 0.6) * 0.6).round().clamp(1.0, 2.0) as i32;
                    for d in dirs {
                        if rng.next() < 0.85 {
                            branch(&mut wood, y, d, len, 0);
                        }
                    }
                }
                blob(&mut rng, &mut leaf, IVec3::new(0, y, 0), rad, 0.35);
            }
            leaf.push(IVec3::new(0, top, 0));
            leaf.push(IVec3::new(0, top + 1, 0));
        }
        BIRCH_LOG => {
            // Tall and slim: a narrow crown up high, a short branch or two.
            let h = rng.range(6, 8);
            for y in 0..h {
                wood.push((IVec3::new(0, y, 0), log));
            }
            for y in h..h + 2 {
                wood.push((IVec3::new(0, y, 0), branch_with_axis(log, 1)));
            }
            let crown = rng.next() * 0.5;
            blob(&mut rng, &mut leaf, IVec3::new(0, h + 1, 0), 1.9 + crown, 1.5);
            blob(&mut rng, &mut leaf, IVec3::new(0, h - 1, 0), 2.1 + crown, 0.9);
            for &d in dirs.iter().take(rng.range(0, 2) as usize) {
                let tip = branch(&mut wood, rng.range(3, h - 2), d, 1, 1);
                blob(&mut rng, &mut leaf, tip + IVec3::Y, 1.4, 0.9);
            }
        }
        _ => {
            // Oak: a thick trunk going on thinner at the top, two to four branches out of
            // it turning up, a round crown on each and a big one on top.
            let h = rng.range(4, 6);
            for y in 0..h {
                wood.push((IVec3::new(0, y, 0), log));
            }
            let lead = rng.range(1, 2);
            for y in h..h + lead {
                wood.push((IVec3::new(0, y, 0), branch_with_axis(log, 1)));
            }
            let crown = rng.next() * 0.6;
            blob(&mut rng, &mut leaf, IVec3::new(0, h + lead, 0), 2.5 + crown, 0.8);
            let n = rng.range(2, 4) as usize;
            for &d in dirs.iter().take(n) {
                let len = rng.range(1, 3);
                let up = rng.range(1, 2);
                let y = rng.range(2, h - 1);
                let tip = branch(&mut wood, y, d, len, up);
                let rad = (1.7 + rng.next() * 0.7).min((REACH - len) as f32);
                blob(&mut rng, &mut leaf, tip, rad, 0.75);
            }
        }
    }
    let mut out: Vec<(IVec3, u8, bool)> = wood.into_iter().map(|(p, b)| (p, b, false)).collect();
    out.extend(
        leaf.into_iter()
            .filter(|p| p.x.abs() <= REACH && p.z.abs() <= REACH && p.y >= 1)
            .map(|p| (p, leaves, true)),
    );
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trees_differ_but_stay_in_bounds() {
        for log in [OAK_LOG, BIRCH_LOG, SPRUCE_LOG] {
            let mut shapes = std::collections::HashSet::new();
            for seed in 0..200u32 {
                let t = tree_shape(log, seed);
                assert!(t.iter().all(|(p, _, _)| p.x.abs() <= REACH && p.z.abs() <= REACH && p.y < 20));
                // A trunk from the ground, branches and leaves.
                assert!(t.iter().any(|(p, b, _)| *p == IVec3::ZERO && *b == log));
                assert!(t.iter().any(|(_, b, _)| is_branch(*b)), "{log} {seed}");
                assert!(t.iter().filter(|(_, b, _)| is_leaves(*b)).count() > 20);
                let mut key: Vec<_> = t.iter().map(|(p, b, _)| (p.x, p.y, p.z, *b)).collect();
                key.sort();
                shapes.insert(key);
            }
            assert!(shapes.len() > 150, "{log}: only {} different trees", shapes.len());
        }
    }
}
