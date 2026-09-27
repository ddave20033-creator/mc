//! Minecraft-style fluid simulation: source blocks, flowing levels 1..7, falling fluid,
//! infinite water sources, flow towards nearby drops, and lava/water interactions.

use super::*;
use std::collections::BTreeMap;

const TICK: f32 = 0.05;
const WATER_DELAY: u64 = 5;
const LAVA_DELAY: u64 = 20;
const BUDGET: usize = 3000;
const HORIZONTAL: [IVec3; 4] = [IVec3::X, IVec3::NEG_X, IVec3::Z, IVec3::NEG_Z];

pub struct Fluids {
    tick: u64,
    acc: f32,
    queue: BTreeMap<u64, Vec<IVec3>>,
    pending: FastSet<IVec3>,
    /// Game time of the current update, used to timestamp changes for smooth rendering.
    now: f32,
    /// Plants and torches washed away by flowing fluid (the game turns them into drops).
    pub broken: Vec<(IVec3, u8)>,
}

fn delay(b: u8) -> u64 {
    if is_lava(b) {
        LAVA_DELAY
    } else {
        WATER_DELAY
    }
}

impl Fluids {
    pub fn new() -> Self {
        Self {
            tick: 0,
            acc: 0.0,
            queue: BTreeMap::new(),
            pending: FastSet::default(),
            now: 0.0,
            broken: Vec::new(),
        }
    }

    fn schedule(&mut self, p: IVec3, d: u64) {
        if self.pending.insert(p) {
            self.queue.entry(self.tick + d).or_default().push(p);
        }
    }

    /// Call after any block change at `p`: wakes up fluids at and around it.
    pub fn notify(&mut self, w: &World, p: IVec3) {
        for q in [
            p,
            p + IVec3::Y,
            p - IVec3::Y,
            p + IVec3::X,
            p - IVec3::X,
            p + IVec3::Z,
            p - IVec3::Z,
        ] {
            let b = w.geti(q);
            if is_fluid(b) {
                self.schedule(q, delay(b));
            }
        }
    }

    /// Wakes the fluids of a chunk that was loaded from a save: flowing fluid and any fluid
    /// that could still spread (the simulation queue itself is not saved).
    pub fn wake_chunk(&mut self, w: &World, pos: ChunkPos) {
        let Some(c) = w.chunks.get(&pos) else { return };
        let (x0, z0) = (pos.0 * CHUNK as i32, pos.1 * CHUNK as i32);
        for y in 0..=c.max_y as usize {
            for z in 0..CHUNK {
                for x in 0..CHUNK {
                    let b = c.get(x, y, z);
                    if !is_fluid(b) {
                        continue;
                    }
                    let p = IVec3::new(x0 + x as i32, y as i32, z0 + z as i32);
                    let lava = is_lava(b);
                    let can_spread = [IVec3::X, IVec3::NEG_X, IVec3::Z, IVec3::NEG_Z, IVec3::NEG_Y]
                        .iter()
                        .any(|&d| {
                            let n = w.geti(p + d);
                            let same = if lava { is_lava(n) } else { is_water(n) };
                            !same && (is_replaceable(n) || fluid_breaks(n))
                        });
                    if fluid_level(b) != 0 || can_spread {
                        self.schedule(p, delay(b));
                    }
                }
            }
        }
    }

    fn set(&mut self, w: &mut World, p: IVec3, b: u8, changed: &mut Vec<IVec3>) {
        let old = w.geti(p);
        if fluid_breaks(old) {
            self.broken.push((p, old));
        }
        if w.seti(p, b) {
            w.record_fluid_change(p, old, b, self.now);
            changed.push(p);
            self.notify(w, p);
        }
    }

    pub fn update(&mut self, dt: f32, now: f32, w: &mut World, changed: &mut Vec<IVec3>) {
        self.now = now;
        w.prune_fluid_changes(now);
        self.acc = (self.acc + dt).min(0.5);
        while self.acc >= TICK {
            self.acc -= TICK;
            self.tick += 1;
            let mut budget = BUDGET;
            while budget > 0 {
                let Some((&key, _)) = self.queue.first_key_value() else {
                    break;
                };
                if key > self.tick {
                    break;
                }
                let mut list = self.queue.remove(&key).unwrap_or_default();
                while budget > 0 {
                    let Some(p) = list.pop() else { break };
                    budget -= 1;
                    self.pending.remove(&p);
                    self.update_block(w, p, changed);
                }
                if !list.is_empty() {
                    self.queue.entry(key).or_default().extend(list);
                }
            }
        }
    }

    fn can_flow(w: &World, q: IVec3) -> bool {
        (0..HEIGHT as i32).contains(&q.y)
            && w.is_loaded(q.x, q.z)
            && (is_replaceable(w.geti(q)) || fluid_breaks(w.geti(q)))
    }

    fn update_block(&mut self, w: &mut World, p: IVec3, changed: &mut Vec<IVec3>) {
        let b = w.geti(p);
        if !is_fluid(b) {
            return;
        }
        let lava = is_lava(b);
        let base = if lava { LAVA } else { WATER };
        let same = |x: u8| if lava { is_lava(x) } else { is_water(x) };
        let drop = if lava { 2 } else { 1 };
        let mut level = b - base;

        // Lava touching water hardens.
        if lava {
            for d in [IVec3::X, IVec3::NEG_X, IVec3::Z, IVec3::NEG_Z, IVec3::Y] {
                if is_water(w.geti(p + d)) {
                    let nb = if level == 0 { OBSIDIAN } else { COBBLE };
                    self.set(w, p, nb, changed);
                    return;
                }
            }
        }

        // Recompute level of flowing fluid from its neighbours.
        if level != 0 {
            let new = if same(w.geti(p + IVec3::Y)) {
                FALLING
            } else {
                let mut best = u8::MAX;
                let mut sources = 0;
                for d in HORIZONTAL {
                    let n = w.geti(p + d);
                    if same(n) {
                        let l = n - base;
                        if l == 0 {
                            sources += 1;
                        }
                        best = best.min(if l >= FALLING { 0 } else { l });
                    }
                }
                let below = w.geti(p - IVec3::Y);
                if !lava && sources >= 2 && (is_solid(below) || below == WATER) {
                    0
                } else {
                    best.saturating_add(drop)
                }
            };
            if new > 7 && new != FALLING {
                self.set(w, p, AIR, changed);
                return;
            }
            if new != level {
                level = new;
                self.set(w, p, base + new, changed);
            }
        }

        // Flow down if possible.
        let below = p - IVec3::Y;
        if Self::can_flow(w, below) {
            let bb = w.geti(below);
            if !(same(bb) && (fluid_level(bb) == 0 || fluid_level(bb) == FALLING)) {
                self.flow_into(w, below, bb, base + FALLING, lava, changed);
            }
            return;
        }

        // Otherwise spread sideways, preferring directions that lead to a drop.
        let eff = if level >= FALLING { 0 } else { level };
        let next = eff + drop;
        if next > 7 {
            return;
        }
        let range = if lava { 2 } else { 4 };
        let mut best = i32::MAX;
        let mut dirs = [false; 4];
        let mut dist = [i32::MAX; 4];
        for (i, d) in HORIZONTAL.iter().enumerate() {
            let q = p + *d;
            if !Self::can_flow(w, q) || (same(w.geti(q)) && fluid_level(w.geti(q)) == 0) {
                continue;
            }
            dist[i] = if Self::can_flow(w, q - IVec3::Y) {
                0
            } else {
                Self::slope(w, q, *d, 1, range, same)
            };
            best = best.min(dist[i]);
        }
        for i in 0..4 {
            dirs[i] = dist[i] != i32::MAX && dist[i] == best;
        }
        for (i, d) in HORIZONTAL.iter().enumerate() {
            if !dirs[i] {
                continue;
            }
            let q = p + *d;
            let qb = w.geti(q);
            if same(qb) {
                let ql = fluid_level(qb);
                if ql == 0 || ql == FALLING || ql <= next {
                    continue;
                }
            }
            self.flow_into(w, q, qb, base + next, lava, changed);
        }
    }

    /// Shortest distance (in blocks) from `from` to a place where the fluid can fall.
    fn slope(
        w: &World,
        from: IVec3,
        came: IVec3,
        depth: i32,
        range: i32,
        same: impl Fn(u8) -> bool + Copy,
    ) -> i32 {
        let mut best = 1000;
        for d in HORIZONTAL {
            if d == -came {
                continue;
            }
            let q = from + d;
            if !Self::can_flow(w, q) || (same(w.geti(q)) && fluid_level(w.geti(q)) == 0) {
                continue;
            }
            if Self::can_flow(w, q - IVec3::Y) {
                return depth;
            }
            if depth < range {
                best = best.min(Self::slope(w, q, d, depth + 1, range, same));
            }
        }
        best
    }

    #[allow(clippy::too_many_arguments)]
    fn flow_into(
        &mut self,
        w: &mut World,
        q: IVec3,
        qb: u8,
        nb: u8,
        lava: bool,
        changed: &mut Vec<IVec3>,
    ) {
        if lava && is_water(qb) {
            self.set(w, q, STONE, changed);
            return;
        }
        if !lava && is_lava(qb) {
            self.set(w, q, if qb == LAVA { OBSIDIAN } else { COBBLE }, changed);
            return;
        }
        self.set(w, q, nb, changed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn restored_chunk_fluids_keep_flowing() {
        let mut world = World::new();
        let mut chunk = ChunkData::new();
        for z in 0..CHUNK {
            for x in 0..CHUNK {
                chunk.set(x, 0, z, STONE);
            }
        }
        // A water source that had not spread yet when the world was saved.
        chunk.set(8, 1, 8, WATER);
        world.chunks.insert((0, 0), Arc::new(chunk));

        let mut fluids = Fluids::new();
        fluids.wake_chunk(&world, (0, 0));
        let mut changed = Vec::new();
        for i in 0..40 {
            fluids.update(TICK, i as f32 * TICK, &mut world, &mut changed);
        }
        assert!(is_water(world.get(9, 1, 8)), "water did not resume flowing");
        assert!(
            is_water(world.get(8, 1, 11)),
            "water did not spread far enough"
        );
    }
}
