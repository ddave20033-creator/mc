//! The trees' shapes: a round trunk, limbs growing out of it along slanting lines (drawn in
//! blocks as steps, which the round branches join up smoothly), and soft crowns of leaves
//! made of rounded clumps shaped by noise, so no two trees are alike. Oaks spread wide,
//! birches stand tall and slim, spruces are layered cones. Used by the world generator and
//! by saplings growing.

use super::block::*;
use glam::{IVec3, Vec3};
use std::collections::{HashMap, HashSet};
use std::f32::consts::TAU;

/// How far a tree reaches from its trunk (blocks): the generator looks this far around a
/// chunk for trees reaching into it.
pub const REACH: i32 = 7;

/// How far out a limb's end may be (its leaves round it still within `REACH`).
const LIMB_REACH: f32 = REACH as f32 - 2.6;

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
    fn between(&mut self, lo: f32, hi: f32) -> f32 {
        lo + self.next() * (hi - lo)
    }
}

/// Smooth 3D value noise in -1..1 (for the ragged but soft edges of the leaves).
fn noise(seed: u32, p: Vec3) -> f32 {
    let h = |x: i32, y: i32, z: i32| {
        let mut v = seed
            ^ (x as u32).wrapping_mul(0x8DA6_B343)
            ^ (y as u32).wrapping_mul(0xD816_3841)
            ^ (z as u32).wrapping_mul(0xCB1A_B31F);
        v ^= v >> 15;
        v = v.wrapping_mul(0x2C1B_3C6D);
        v ^= v >> 12;
        (v >> 8) as f32 / (1u32 << 24) as f32 * 2.0 - 1.0
    };
    let f = p.floor();
    let (x, y, z) = (f.x as i32, f.y as i32, f.z as i32);
    let t = p - f;
    let s = t * t * (Vec3::splat(3.0) - 2.0 * t);
    let lerp = |a: f32, b: f32, t: f32| a + (b - a) * t;
    let plane = |z: i32| {
        lerp(
            lerp(h(x, y, z), h(x + 1, y, z), s.x),
            lerp(h(x, y + 1, z), h(x + 1, y + 1, z), s.x),
            s.y,
        )
    };
    lerp(plane(z), plane(z + 1), s.z)
}

/// A tree being grown: its wood (the first put at a place stays) and its leaves.
struct Tree {
    log: Block,
    wood: HashMap<IVec3, Block>,
    order: Vec<IVec3>,
    leaves: HashSet<IVec3>,
    seed: u32,
}

impl Tree {
    fn put_wood(&mut self, p: IVec3, b: Block) {
        if p.x.abs() > REACH || p.z.abs() > REACH || p.y < 0 {
            return;
        }
        if let std::collections::hash_map::Entry::Vacant(e) = self.wood.entry(p) {
            e.insert(b);
            self.order.push(p);
        }
    }

    /// The trunk: thick logs from the ground up to `h` (not included).
    fn trunk(&mut self, h: i32) {
        for y in 0..h {
            self.put_wood(IVec3::new(0, y, 0), self.log);
        }
    }

    /// A branch from the block `from` (already wood) toward the point `to`: the blocks
    /// nearest the straight line between them, each one step along one axis from the last
    /// (lying along that axis), so it goes aslant as even steps. Returns its blocks.
    fn limb(&mut self, from: IVec3, to: Vec3) -> Vec<IVec3> {
        let start = from.as_vec3() + Vec3::splat(0.5);
        let dir = (to - start).normalize_or_zero();
        let goal = to.floor().as_ivec3();
        let mut p = from;
        let mut out = Vec::new();
        for _ in 0..48 {
            if p == goal {
                break;
            }
            // The step toward the goal keeping nearest the line.
            let mut best: Option<(f32, IVec3, usize)> = None;
            for k in 0..3 {
                let d = goal[k] - p[k];
                if d == 0 {
                    continue;
                }
                let mut q = p;
                q[k] += d.signum();
                let c = q.as_vec3() + Vec3::splat(0.5) - start;
                let off = (c - dir * c.dot(dir)).length();
                if best.is_none_or(|b| off < b.0) {
                    best = Some((off, q, k));
                }
            }
            let Some((_, q, k)) = best else { break };
            p = q;
            self.put_wood(p, branch_with_axis(self.log, k));
            out.push(p);
        }
        out
    }

    /// A soft clump of leaves round `c`: `rx` across (each way), `ry` up and down, its edge
    /// swelling and dipping with smooth noise (`rough` blocks at most).
    fn clump(&mut self, c: Vec3, rx: f32, ry: f32, rough: f32) {
        let (nx, ny) = ((rx + rough).ceil() as i32 + 1, (ry + rough).ceil() as i32 + 1);
        let base = c.floor().as_ivec3();
        for dy in -ny..=ny {
            for dz in -nx..=nx {
                for dx in -nx..=nx {
                    let p = base + IVec3::new(dx, dy, dz);
                    let d = p.as_vec3() + Vec3::splat(0.5) - c;
                    let e = ((d.x / rx).powi(2) + (d.y / ry).powi(2) + (d.z / rx).powi(2)).sqrt();
                    // How far inside the edge (blocks), the edge moved by the noise.
                    let depth = (1.0 - e) * rx.min(ry).max(1.0);
                    if depth + noise(self.seed, p.as_vec3() * 0.42) * rough > 0.0 {
                        self.leaves.insert(p);
                    }
                }
            }
        }
    }

    /// The blocks, wood first (in the order grown), then the leaves (only into air), without
    /// lone leaves sticking out.
    fn finish(self, leaves: Block) -> Vec<(IVec3, Block, bool)> {
        let solid = |p: IVec3| self.leaves.contains(&p) || self.wood.contains_key(&p);
        let near = |p: IVec3| {
            [IVec3::X, IVec3::NEG_X, IVec3::Y, IVec3::NEG_Y, IVec3::Z, IVec3::NEG_Z]
                .iter()
                .filter(|&&d| solid(p + d))
                .count()
        };
        let mut leaf: Vec<IVec3> = self
            .leaves
            .iter()
            .copied()
            .filter(|p| p.x.abs() <= REACH && p.z.abs() <= REACH && p.y >= 1)
            .filter(|p| !self.wood.contains_key(p) && near(*p) >= 2)
            .collect();
        // (the same order each time for the same seed)
        leaf.sort_by_key(|p| (p.y, p.z, p.x));
        let mut out: Vec<(IVec3, Block, bool)> = self.order.iter().map(|p| (*p, self.wood[p], false)).collect();
        out.extend(leaf.into_iter().map(|p| (p, leaves, true)));
        out
    }
}

/// A point `len` out from `from` toward the heading `angle` (round the trunk), rising at
/// `rise` (radians up from level), kept within `LIMB_REACH` of the trunk.
fn toward(from: Vec3, angle: f32, rise: f32, len: f32) -> Vec3 {
    let d = Vec3::new(angle.cos() * rise.cos(), rise.sin(), angle.sin() * rise.cos());
    let mut p = from + d * len;
    let flat = Vec3::new(p.x - 0.5, 0.0, p.z - 0.5);
    if flat.length() > LIMB_REACH {
        let f = flat.normalize() * LIMB_REACH;
        p.x = f.x + 0.5;
        p.z = f.z + 0.5;
    }
    p
}

fn center(p: IVec3) -> Vec3 {
    p.as_vec3() + Vec3::splat(0.5)
}

/// The blocks of a tree of `log` (oak, birch or spruce) standing at the origin: (offset,
/// block, only into air). Wood first, then the leaves, which do not replace it.
pub fn tree_shape(log: Block, seed: u32) -> Vec<(IVec3, Block, bool)> {
    let mut rng = Rng(seed.wrapping_mul(0x9E37_79B9) | 1);
    for _ in 0..3 {
        rng.next();
    }
    let leaves = leaves_of(log);
    let mut t = Tree {
        log,
        wood: HashMap::new(),
        order: Vec::new(),
        leaves: HashSet::new(),
        seed: seed.wrapping_mul(0x27D4_EB2F) ^ 0x1656_67B1,
    };
    match log {
        SPRUCE_LOG => spruce(&mut t, &mut rng),
        BIRCH_LOG => birch(&mut t, &mut rng),
        _ => oak(&mut t, &mut rng),
    }
    t.finish(leaves)
}

/// Oak: a short thick trunk parting into limbs that spread out and up (forking now and
/// then), a rounded clump of leaves on each, together a wide soft dome. Small young ones,
/// ordinary ones and now and then a big old one.
fn oak(t: &mut Tree, rng: &mut Rng) {
    let size = rng.next();
    let (h, limbs, reach, leaf) = if size < 0.25 {
        (rng.range(3, 4), rng.range(2, 3), (2.4, 3.3), (1.9, 2.4))
    } else if size < 0.88 {
        (rng.range(4, 5), rng.range(3, 4), (3.0, 4.3), (2.2, 2.8))
    } else {
        (rng.range(5, 6), rng.range(4, 6), (4.0, 5.2), (2.5, 3.1))
    };
    t.trunk(h);
    // The trunk goes on thinner up the middle.
    let lead = rng.range(1, 2) + (size >= 0.88) as i32;
    let top = t.limb(IVec3::new(0, h - 1, 0), Vec3::new(0.5 + rng.between(-0.6, 0.6), (h + lead) as f32 + 0.5, 0.5 + rng.between(-0.6, 0.6)));
    let top = top.last().copied().unwrap_or(IVec3::new(0, h - 1, 0));
    let r = rng.between(leaf.0, leaf.1) + 0.3;
    t.clump(center(top) + Vec3::Y * 0.4, r, r * 0.8, 1.1);
    let turn = rng.next() * TAU;
    for i in 0..limbs {
        let angle = turn + i as f32 / limbs as f32 * TAU + rng.between(-0.45, 0.45);
        // Lower limbs reach further and lie flatter.
        let low = rng.next();
        let y = (h - 1 - (low * (h as f32 * 0.5)) as i32).max(2);
        let from = IVec3::new(0, y, 0);
        let len = rng.between(reach.0, reach.1) * (0.85 + low * 0.25);
        let rise = rng.between(0.3, 0.62) + (1.0 - low) * 0.15;
        let path = t.limb(from, toward(center(from), angle, rise, len));
        let Some(&end) = path.last() else { continue };
        let r = rng.between(leaf.0, leaf.1);
        t.clump(center(end) + Vec3::Y * rng.between(0.1, 0.9), r, r * 0.78, 1.1);
        // A fork partway out, its own clump of leaves.
        if path.len() >= 3 && rng.next() < 0.55 + size * 0.3 {
            let at = path[path.len() * 3 / 5];
            let side = if rng.next() < 0.5 { -1.0 } else { 1.0 };
            let fork = t.limb(at, toward(center(at), angle + side * rng.between(0.6, 1.0), rise + 0.3, len * 0.5));
            if let Some(&tip) = fork.last() {
                let r = rng.between(leaf.0, leaf.1) * 0.8;
                t.clump(center(tip) + Vec3::Y * 0.4, r, r * 0.8, 1.0);
            }
        }
        // Leaves along a long limb, so the crown is whole underneath.
        if path.len() >= 5 {
            let mid = center(path[path.len() / 2]);
            let r = leaf.0 * 0.75;
            t.clump(mid + Vec3::Y * 0.8, r, r * 0.75, 0.8);
        }
    }
}

/// Birch: a tall slim trunk going on as a thin leader, short limbs growing steeply up off
/// its upper part, and a slim oval crown of loose lumpy clumps round it.
fn birch(t: &mut Tree, rng: &mut Rng) {
    let h = rng.range(6, 9);
    t.trunk(h);
    let lead = rng.range(2, 3);
    let top = t.limb(IVec3::new(0, h - 1, 0), Vec3::new(0.5, (h + lead) as f32 + 0.5, 0.5));
    let top = top.last().copied().unwrap_or(IVec3::new(0, h - 1, 0));
    let slim = rng.between(0.85, 1.1);
    // The crown from a little under half the trunk to over the leader: oval, widest a
    // third of the way up, clumps round the middle at different sides.
    let low = (h / 2 + rng.range(0, 1)) as f32;
    let high = (h + lead) as f32 + 1.2;
    let mut y = low;
    let mut turn = rng.next() * TAU;
    while y <= high {
        let k = (y - low) / (high - low);
        let r = (0.9 + 1.5 * (std::f32::consts::PI * (0.15 + k * 0.85)).sin()) * slim;
        let off = r * 0.45;
        for i in 0..3 {
            let a = turn + i as f32 * TAU / 3.0;
            let c = Vec3::new(0.5 + a.cos() * off, y + 0.5, 0.5 + a.sin() * off);
            t.clump(c, r * 0.7, 1.2, 0.9);
        }
        turn += 1.3;
        y += 1.5;
    }
    t.clump(center(top) + Vec3::Y * 0.3, 1.1, 1.5, 0.5);
    let turn = rng.next() * TAU;
    let n = rng.range(2, 3);
    for i in 0..n {
        let angle = turn + i as f32 / n as f32 * TAU + rng.between(-0.5, 0.5);
        let from = IVec3::new(0, rng.range(h / 2 + 1, h - 1), 0);
        let path = t.limb(from, toward(center(from), angle, rng.between(0.85, 1.15), rng.between(1.8, 2.8)));
        if let Some(&end) = path.last() {
            let r = rng.between(1.2, 1.5);
            t.clump(center(end) + Vec3::Y * 0.5, r, r * 1.1, 0.6);
        }
    }
}

/// Spruce: a straight trunk with rings of level limbs under layers of needles (a wide one
/// on each ring, a narrower one between), smaller toward the top, the rims drooping, and a
/// thin spire.
fn spruce(t: &mut Tree, rng: &mut Rng) {
    let h = rng.range(7, 12);
    t.trunk(h);
    let lead = rng.range(2, 3);
    let top = t.limb(IVec3::new(0, h - 1, 0), Vec3::new(0.5, (h + lead) as f32 + 0.5, 0.5));
    let top = top.last().copied().unwrap_or(IVec3::new(0, h - 1, 0));
    let wide = rng.between(2.4, 3.0) + (h - 7) as f32 * 0.12;
    let base = rng.range(2, 3);
    let crown = (h + lead + 1 - base) as f32;
    let mut turn = rng.next() * TAU;
    // Tiers every 2 or 3 blocks: a ring of level limbs under a layer of needles whose rim
    // droops, and a narrower layer between.
    let step = rng.range(2, 3);
    for y in base..=h + lead {
        let k = (y - base) as f32 / crown;
        let ring = (y - base) % step == 0;
        let r = 0.5 + wide * (1.0 - k) * if ring { 1.0 } else { 0.62 };
        let c = Vec3::new(0.5, y as f32 + 0.5, 0.5);
        if ring {
            if y < h {
                let from = IVec3::new(0, y, 0);
                let len = r - 1.4;
                if len >= 1.0 {
                    let n = rng.range(3, 5);
                    for i in 0..n {
                        let angle = turn + i as f32 / n as f32 * TAU + rng.between(-0.3, 0.3);
                        t.limb(from, toward(center(from), angle, 0.0, len));
                    }
                }
            }
            turn += 2.4;
            t.clump(c, r, 0.75, 0.35);
            t.clump(c - Vec3::Y * 0.9, r * 0.97 + 0.15, 0.4, 0.3);
        } else {
            t.clump(c, r, 0.6, 0.3);
        }
    }
    // The spire: needles all round the leader, a tip over it.
    for dy in -2..=0 {
        for d in [IVec3::X, IVec3::NEG_X, IVec3::Z, IVec3::NEG_Z] {
            t.leaves.insert(top + d + IVec3::Y * dy);
        }
    }
    t.leaves.insert(top + IVec3::Y);
    t.leaves.insert(top + IVec3::Y * 2);
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
                assert!(t.iter().all(|(p, _, _)| p.x.abs() <= REACH && p.z.abs() <= REACH && p.y < 24));
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

    #[test]
    fn branches_are_joined_to_the_tree() {
        // Every piece of wood reaches the ground through wood.
        for log in [OAK_LOG, BIRCH_LOG, SPRUCE_LOG] {
            for seed in 0..100u32 {
                let t = tree_shape(log, seed);
                let wood: HashSet<IVec3> = t.iter().filter(|(_, b, _)| is_log(*b)).map(|(p, _, _)| *p).collect();
                let mut seen = HashSet::from([IVec3::ZERO]);
                let mut todo = vec![IVec3::ZERO];
                while let Some(p) = todo.pop() {
                    for d in [IVec3::X, IVec3::NEG_X, IVec3::Y, IVec3::NEG_Y, IVec3::Z, IVec3::NEG_Z] {
                        if wood.contains(&(p + d)) && seen.insert(p + d) {
                            todo.push(p + d);
                        }
                    }
                }
                assert_eq!(seen.len(), wood.len(), "{log} {seed}");
            }
        }
    }
}
