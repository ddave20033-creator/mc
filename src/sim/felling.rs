//! Trees felled with an axe, the same for the server and the players' games: which of a
//! tree comes down when its trunk is cut through, how it falls over, and its trunk lying on
//! the ground after, cut up piece by piece. (The chopping itself, the axe's swing and where
//! its edge bites in, is the player's game's: `game::sim::felling`.)

use crate::item::{tool_of, ItemId, Tier, ToolKind};
use crate::world::mesh::{stump_heights, Notch};
use crate::world::*;
use glam::{IVec3, Mat4, Vec2, Vec3};
use std::collections::HashSet;

/// How deep (of the trunk's width) the cut goes before the trunk breaks.
pub const FELL_DEPTH: f32 = 0.75;
/// The most blocks a falling tree takes with it.
pub const MAX_BLOCKS: usize = 900;


/// A tree falling over: its blocks turning about the hinge left by the cut.
#[derive(Clone, Debug, PartialEq)]
pub struct FallingTree {
    /// Which, for the players (`Msg::TreeLands`).
    pub id: u32,
    /// Where it turns about (on the trunk's far side from the cut, at the cut's height), and
    /// the axis it turns about (level, across the way it falls).
    pub pivot: Vec3,
    pub axis: Vec3,
    /// How far over it is (radians from upright) and how fast it is going over, and how far
    /// before the last tick (drawn between).
    pub angle: f32,
    pub prev_angle: f32,
    pub speed: f32,
    /// How high it reaches over the hinge (its fall is slower, the taller it is).
    pub height: f32,
    /// Its blocks: their lower corner from the pivot as it stood, and the block; the first
    /// `trunk` of them its trunk from the cut up (which stays lying where it falls).
    pub blocks: Vec<(Vec3, Block)>,
    pub trunk: usize,
    /// The middle of the stump's top (where the trunk lies from).
    pub stump: Vec3,
    /// The part of the cut block above the cut (its lower corner from the pivot, the block
    /// and the cut's height in it), which goes with the tree.
    pub stub: (Vec3, Block, f32),
    pub leaf_tint: [u8; 3],
    /// What it was felled with (for the drops), and whether in creative (none).
    pub tool: ItemId,
    pub creative: bool,
}

impl FallingTree {
    pub fn turn(&self) -> Mat4 {
        self.turned(self.angle)
    }

    pub fn turned(&self, angle: f32) -> Mat4 {
        Mat4::from_translation(self.pivot) * Mat4::from_axis_angle(self.axis, angle)
    }

    /// Where a block's middle is now.
    pub fn at(&self, turn: &Mat4, o: Vec3) -> Vec3 {
        turn.transform_point3(o + Vec3::splat(0.5))
    }

    /// A tick of its fall (like a pole tipping about its foot: slowly at first, faster as it
    /// goes over). True once its wood hits something solid (leaves do not stop it) or it lies
    /// flat.
    pub fn step(&mut self, dt: f32, w: &World) -> bool {
        const GRAVITY: f32 = 28.0;
        self.prev_angle = self.angle;
        let pull = 1.5 * GRAVITY / self.height.max(1.5) * self.angle.sin().max(0.04);
        self.speed += pull * dt;
        self.angle += self.speed * dt;
        let turn = self.turn();
        self.angle > 1.65
            || self.blocks.iter().any(|&(o, b)| {
                if is_leaves(b) || o.y < 0.6 {
                    return false;
                }
                let g = w.geti(self.at(&turn, o).floor().as_ivec3());
                is_solid(g) && !is_leaves(g)
            })
    }

    /// The way it fell (level), once it is down.
    pub fn fell_toward(&self) -> Vec3 {
        let fell = self.turn().transform_vector3(Vec3::Y);
        Vec3::new(fell.x, 0.0, fell.z).normalize_or_zero()
    }
}

/// The trunk at `p` breaks at the cut `notch`: everything of its tree above it (the trunk, the
/// branches, and the leaves round them that are no other tree's) comes away and falls over,
/// away from the cut's side (`tint`: the leaves' color at a place). The tree falling (felled
/// with `tool`, in creative or not), and the blocks it takes: its leaves and its wood.
pub fn fell(
    w: &World,
    p: IVec3,
    notch: Notch,
    tool: ItemId,
    creative: bool,
    tint: impl Fn(IVec3) -> [u8; 3],
) -> (FallingTree, Vec<IVec3>, Vec<IVec3>) {
    let b0 = w.geti(p);
    let kind = log_base(b0);
    let leaves = leaves_of(kind);
    const SIDES: [IVec3; 6] = [IVec3::X, IVec3::NEG_X, IVec3::Y, IVec3::NEG_Y, IVec3::Z, IVec3::NEG_Z];
    let mut wood = vec![p];
    let mut seen: HashSet<IVec3> = HashSet::from([p]);
    let mut i = 0;
    while i < wood.len() && wood.len() < MAX_BLOCKS {
        let q = wood[i];
        i += 1;
        for d in SIDES {
            let r = q + d;
            if r.y < p.y || seen.contains(&r) {
                continue;
            }
            // Only what a tree is made of: its trunk straight up from the cut, and its branches
            // within a tree's reach. Logs laid by a player (a wall, a house) or the trunk of
            // another tree the branches touch stay.
            let b = w.geti(r);
            let own = if is_branch(b) {
                (r.x - p.x).abs().max((r.z - p.z).abs()) <= crate::world::trees::REACH
            } else {
                is_trunk(b) && r.x == p.x && r.z == p.z
            };
            if own && log_base(b) == kind {
                seen.insert(r);
                wood.push(r);
            }
        }
    }
    // The leaves within a few steps of its wood, but none next to another tree's wood.
    let mut leaf = Vec::new();
    let mut front = wood.clone();
    for _ in 0..5 {
        let mut next = Vec::new();
        for q in front {
            for d in SIDES {
                let r = q + d;
                if seen.contains(&r) || w.geti(r) != leaves {
                    continue;
                }
                seen.insert(r);
                let foreign = (-1..=1).any(|dy| {
                    (-1..=1).any(|dz| {
                        (-1..=1).any(|dx| {
                            let s = r + IVec3::new(dx, dy, dz);
                            is_log(w.geti(s)) && !seen.contains(&s)
                        })
                    })
                });
                if !foreign {
                    leaf.push(r);
                    next.push(r);
                }
            }
        }
        front = next;
    }
    // It falls away from the side the cut was made on, about a hinge on the far side.
    let toward = Vec3::new(-notch.angle.cos(), 0.0, -notch.angle.sin());
    let pivot = p.as_vec3() + Vec3::new(0.5, notch.height, 0.5) + toward * log_radius(b0) * 0.7;
    // The stump stays; the rest of its block goes with the tree. Its trunk from the cut up
    // first (it will lie where it falls), then the rest of its wood and its leaves.
    wood.retain(|&q| q != p);
    let trunk: Vec<IVec3> = (1..)
        .map(|dy| p + IVec3::Y * dy)
        .take_while(|&q| seen.contains(&q) && is_trunk(w.geti(q)) && log_base(w.geti(q)) == kind)
        .collect();
    let mut blocks: Vec<(Vec3, Block)> = Vec::with_capacity(wood.len() + leaf.len());
    for &q in trunk.iter().chain(wood.iter().filter(|q| !trunk.contains(q))).chain(&leaf) {
        blocks.push((q.as_vec3() - pivot, w.geti(q)));
    }
    let height = blocks.iter().map(|(o, _)| o.y + 1.0).fold(1.0f32, f32::max);
    let leaf_tint = leaf.first().map_or([255; 3], |&q| tint(q));
    let tree = FallingTree {
        id: 0,
        pivot,
        axis: Vec3::Y.cross(toward).normalize(),
        angle: 0.02,
        prev_angle: 0.02,
        speed: 0.15,
        height,
        blocks,
        trunk: trunk.len(),
        stump: p.as_vec3() + Vec3::new(0.5, notch.height, 0.5),
        stub: (p.as_vec3() - pivot, b0, notch.height),
        leaf_tint,
        tool,
        creative,
    };
    (tree, leaf, wood)
}

/// Where a felled trunk comes to lie: from `start` (the cut's middle, where the tree came
/// down) along `dir`, on whatever is under it. Its base end's middle, the way it lies
/// (level), and how many of its `pieces` fit (the ones past them would go into something
/// solid: they break off there).
pub fn log_rest(w: &World, start: Vec3, dir: Vec3, pieces: &[Block]) -> Option<(Vec3, Vec3, usize)> {
    let dir = Vec3::new(dir.x, 0.0, dir.z).normalize_or_zero();
    if dir == Vec3::ZERO || pieces.is_empty() {
        return None;
    }
    let r = log_radius(pieces[0]);
    let solid = |q: Vec3| {
        let b = w.geti(q.floor().as_ivec3());
        is_solid(b) && !is_leaves(b)
    };
    // The ground under a point: the top of the first solid block below it.
    let ground = |at: Vec3| {
        let mut y = at.y.floor();
        for _ in 0..12 {
            if solid(Vec3::new(at.x, y - 0.5, at.z)) {
                return y;
            }
            y -= 1.0;
        }
        y
    };
    // It rests on the highest ground under its first pieces.
    let reach = pieces.len().min(4);
    let mut floor = f32::MIN;
    for i in 0..reach {
        let at = start + dir * (i as f32 + 0.5) + Vec3::Y * 0.5;
        floor = floor.max(ground(at));
    }
    let base = Vec3::new(start.x, floor + r, start.z);
    // It goes as far as it is free of the world.
    let free = pieces
        .iter()
        .enumerate()
        .take_while(|&(i, _)| {
            let m = base + dir * (i as f32 + 0.5);
            !solid(m) && !solid(m + Vec3::Y * (r * 0.5))
        })
        .count();
    // (a single block left lying comes apart at once)
    let free = if free == 1 { 0 } else { free };
    Some((base, dir, free))
}


/// The trunk of a standing tree (an upright log, not a branch), which an axe fells.
pub fn is_trunk(b: Block) -> bool {
    is_log(b) && !is_branch(b) && log_axis(b) == 1
}

/// A chop into a trunk: the cut `prev` (none yet: a new one) made `step` deeper where the axe
/// bit in (`angle` round the trunk from its middle, `height` up the block): a chop a little
/// off the cut moves it that way, weighed by how much is cut already.
pub fn deepen(prev: Option<Notch>, angle: f32, height: f32, step: f32) -> Notch {
    use std::f32::consts::{PI, TAU};
    let height = height.clamp(0.25, 0.75);
    let n = match prev {
        Some(n) => {
            let k = step / (n.depth + step);
            let turn = (angle - n.angle + PI).rem_euclid(TAU) - PI;
            Notch { angle: n.angle + turn * k, height: n.height + (height - n.height) * k, ..n }
        }
        None => Notch { angle, height, depth: 0.0, felled: false },
    };
    Notch { depth: (n.depth + step).min(FELL_DEPTH), ..n }
}

/// How much deeper a chop with `held` cuts (in creative a few chops fell any tree; not an
/// axe: none).
pub fn chop_step(held: ItemId, creative: bool) -> f32 {
    let chops = if creative { 4.0 } else { chops_needed(held).unwrap_or(f32::INFINITY) };
    FELL_DEPTH / chops
}

/// Chops it takes an axe to fell a tree (None: not an axe): each chop cuts this much of the
/// way through, a weak axe little, a strong one more.
pub fn chops_needed(held: ItemId) -> Option<f32> {
    match tool_of(held)? {
        (ToolKind::Axe, tier) => Some(match tier {
            Tier::Wood => 10.0,
            Tier::Stone => 8.0,
            Tier::Copper => 7.0,
            Tier::Iron => 6.0,
            Tier::Diamond => 5.0,
            Tier::Gold => 4.0,
        }),
        _ => None,
    }
}

/// Whether the point `q` is in the wood of an upright trunk (not in the air round it, nor in
/// a cut already taken out of it), and which trunk.
pub fn in_trunk(w: &World, q: Vec3) -> Option<IVec3> {
    let p = q.floor().as_ivec3();
    let b = w.geti(p);
    if !is_trunk(b) {
        return None;
    }
    let rel = Vec2::new(q.x - (p.x as f32 + 0.5), q.z - (p.z as f32 + 0.5));
    let r = log_radius(b);
    if rel.length() > r {
        return None;
    }
    if let Some(n) = w.notch(p) {
        let y = q.y - p.y as f32;
        let h = n.height.clamp(0.12, 0.88);
        let deep = n.depth.clamp(0.0, 1.0) * 2.0 * r;
        if n.felled {
            // A stump: flat, the hinge standing on its far side.
            let (flat, hinge) = stump_heights(n, r);
            let far = rel.dot(Vec2::new(n.angle.cos(), n.angle.sin())) <= r - deep;
            return (y <= flat || (y <= hinge && far)).then_some(p);
        }
        let half = (deep * 0.8).max(0.08);
        let cut = r - deep * (1.0 - (y - h).abs() / half).max(0.0);
        if rel.dot(Vec2::new(n.angle.cos(), n.angle.sin())) > cut {
            return None;
        }
    }
    Some(p)
}

/// The stump that `p` is part of, if it is one: its cut block (left by a felled tree) on top
/// and the trunk under it down to what it stands on, top first.
pub fn stump_of(w: &World, p: IVec3) -> Option<Vec<IVec3>> {
    let b = w.geti(p);
    if !is_trunk(b) {
        return None;
    }
    let same = |q: IVec3| is_trunk(w.geti(q)) && log_base(w.geti(q)) == log_base(b);
    // Up to the cut (a trunk still standing above is no stump).
    let mut top = p;
    while !w.notch(top).is_some_and(|n| n.felled) {
        top += IVec3::Y;
        if !same(top) || top.y - p.y > 64 {
            return None;
        }
    }
    let mut column = vec![top];
    let mut q = top - IVec3::Y;
    while same(q) && w.notch(q).is_none() && top.y - q.y < 64 {
        column.push(q);
        q -= IVec3::Y;
    }
    Some(column)
}


/// A trunk lying on the ground.
#[derive(Clone, Debug, PartialEq)]
pub struct LyingLog {
    pub id: u32,
    /// Its base end's middle, and the way it lies (level) from there.
    pub base: Vec3,
    pub dir: Vec3,
    /// Its pieces from the base: the trunk's blocks they were.
    pub pieces: Vec<Block>,
    /// How many blocks the next stroke takes off (1 to 3, rolled after each).
    pub next: usize,
}

impl LyingLog {
    pub fn len(&self) -> f32 {
        self.pieces.len() as f32
    }

    pub fn radius(&self) -> f32 {
        self.pieces.first().map_or(0.44, |&b| log_radius(b))
    }

    /// How far along it (from its base) the point `q` is, if it is in its wood.
    pub fn contains(&self, q: Vec3) -> Option<f32> {
        let s = (q - self.base).dot(self.dir);
        if !(0.0..=self.len()).contains(&s) {
            return None;
        }
        let off = q - (self.base + self.dir * s);
        (off.length() <= self.radius()).then_some(s)
    }

    /// Where the ray from `o` along `d` (a unit vector) first meets its wood within `max`:
    /// how far along the ray, and how far along the trunk (from its base). Worked out
    /// exactly (a ray against a round trunk with flat ends), not stepped along.
    pub fn ray_hit(&self, o: Vec3, d: Vec3, max: f32) -> Option<(f32, f32)> {
        let (a, r, len) = (self.dir, self.radius(), self.len());
        let w = o - self.base;
        let (wa, da) = (w.dot(a), d.dot(a));
        // Round side: the part of the ray within `r` of the axis.
        let (wp, dp) = (w - a * wa, d - a * da);
        let (qa, qb, qc) = (dp.dot(dp), 2.0 * wp.dot(dp), wp.dot(wp) - r * r);
        let (mut t0, mut t1) = if qa > 1e-9 {
            let disc = qb * qb - 4.0 * qa * qc;
            if disc < 0.0 {
                return None;
            }
            let root = disc.sqrt();
            ((-qb - root) / (2.0 * qa), (-qb + root) / (2.0 * qa))
        } else if qc <= 0.0 {
            (f32::NEG_INFINITY, f32::INFINITY)
        } else {
            return None;
        };
        // Its ends: the part of the ray between them.
        if da.abs() > 1e-9 {
            let (e0, e1) = ((0.0 - wa) / da, (len - wa) / da);
            t0 = t0.max(e0.min(e1));
            t1 = t1.min(e0.max(e1));
        } else if !(0.0..=len).contains(&wa) {
            return None;
        }
        let t = t0.max(0.0);
        (t <= t1 && t < max).then(|| (t, (wa + da * t).clamp(0.0, len)))
    }

    /// The middle of the piece `i`.
    pub fn piece_middle(&self, i: usize) -> Vec3 {
        self.base + self.dir * (i as f32 + 0.5)
    }

    /// Which end a stroke at `s` along it takes the next piece off: the nearer one.
    pub fn from_base_at(&self, s: f32) -> bool {
        s < self.len() * 0.5
    }

    /// The pieces the next stroke takes off from that end (the rest too, if only one would be
    /// left).
    pub fn taken(&self, from_base: bool) -> std::ops::Range<usize> {
        let n = self.pieces.len();
        let k = self.next.clamp(1, 3).min(n);
        let k = if n - k <= 1 { n } else { k };
        if from_base {
            0..k
        } else {
            n - k..n
        }
    }

    /// A stroke from that end: the pieces it takes off go (the trunk now starts past them,
    /// cut from its base); they and where they lay are returned.
    pub fn cut(&mut self, from_base: bool) -> Vec<(Block, Vec3)> {
        let taken = self.taken(from_base);
        let at: Vec<Vec3> = taken.clone().map(|j| self.piece_middle(j)).collect();
        let off: Vec<Block> = self.pieces.drain(taken.clone()).collect();
        if taken.start == 0 {
            self.base += self.dir * taken.end as f32;
        }
        off.into_iter().zip(at).collect()
    }
}

/// Lying trunks saved with a world: `x,y,z,dx,dz,piece;piece;...` a line (the pieces by
/// their blocks' keys).
pub fn logs_text(logs: &[LyingLog]) -> String {
    logs.iter()
        .map(|l| {
            let pieces: Vec<&str> = l.pieces.iter().map(|&b| def(b).key).collect();
            format!("{},{},{},{},{},{}\n", l.base.x, l.base.y, l.base.z, l.dir.x, l.dir.z, pieces.join(";"))
        })
        .collect()
}

pub fn parse_logs(text: &str) -> Vec<LyingLog> {
    let mut out = Vec::new();
    for line in text.lines() {
        let v: Vec<&str> = line.trim().split(',').collect();
        if v.len() != 6 {
            continue;
        }
        let f: Vec<f32> = v[..5].iter().filter_map(|x| x.parse().ok()).collect();
        let pieces: Vec<Block> = v[5].split(';').filter_map(by_key).filter(|&b| is_log(b)).collect();
        if f.len() != 5 || pieces.is_empty() {
            continue;
        }
        let dir = Vec3::new(f[3], 0.0, f[4]).normalize_or_zero();
        if dir == Vec3::ZERO {
            continue;
        }
        let id = out.len() as u32 + 1;
        out.push(LyingLog { id, base: Vec3::new(f[0], f[1], f[2]), dir, pieces, next: 1 + id as usize % 3 });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::Vec3;

    /// The first point stepped along the ray that is in the wood (as the aim used to find it).
    fn stepped(l: &LyingLog, o: Vec3, d: Vec3, max: f32) -> Option<(f32, f32)> {
        let mut t = 0.0;
        while t < max {
            if let Some(s) = l.contains(o + d * t) {
                return Some((t, s));
            }
            t += 0.002;
        }
        None
    }

    #[test]
    fn a_ray_meets_a_lying_trunk_where_stepping_along_it_does() {
        let mut rng = crate::util::Rng::new(7);
        let mut hits = 0;
        for k in 0..400 {
            let dir = Vec3::new(rng.next() - 0.5, (rng.next() - 0.5) * 0.3, rng.next() - 0.5).normalize();
            let l = LyingLog { id: k, base: Vec3::new(0.3, 64.4, -0.7), dir, pieces: vec![crate::world::OAK_LOG; 1 + (k % 5) as usize], next: 1 };
            let o = l.base + Vec3::new(rng.next() - 0.5, rng.next() * 0.6 + 0.8, rng.next() - 0.5) * 6.0;
            let target = l.base + dir * (rng.next() * l.len());
            let d = (target - o).normalize();
            let exact = l.ray_hit(o, d, 8.0);
            let step = stepped(&l, o, d, 8.0);
            match (exact, step) {
                (Some((t, s)), Some((ts, ss))) => {
                    hits += 1;
                    assert!(t <= ts + 1e-4 && ts - t < 0.003, "{t} {ts}");
                    assert!((s - ss).abs() < 0.01, "{s} {ss}");
                }
                (None, None) => {}
                // (only a ray grazing the wood between two steps)
                (Some((t, _)), None) => assert!(l.contains(o + d * (t + 1e-3)).is_none() || t > 7.99),
                (None, Some(_)) => panic!("stepping found a hit the exact test missed"),
            }
        }
        assert!(hits > 100, "{hits}");
    }
}
