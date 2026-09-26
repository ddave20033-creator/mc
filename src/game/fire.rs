//! Fire, by Minecraft Java's rules: every 30-39 ticks a fire ages, may burn out, burns its
//! flammable neighbours (turning them into fire or air, lighting TNT) and spreads to air
//! near flammable blocks, less likely higher up. Lava sets fire to flammable blocks around
//! it on random ticks. Flint and steel lights fire; punching the block it sits on puts it
//! out. The host (or single player) runs it; LAN players just see the blocks.

use super::*;
use crate::entity::TNT_FUSE;

/// A burning fire block's state.
#[derive(Clone, Copy)]
pub(super) struct Fire {
    /// 0..=15: older fire burns out and spreads less.
    pub age: u8,
    /// Seconds until its next tick.
    pub timer: f32,
}

/// Minecraft's difficulty id for Normal (fire spreads a bit more on harder difficulties).
const DIFFICULTY: u32 = 2;

/// Random ticks per 16x16x16 section per game tick (Minecraft's randomTickSpeed), and how
/// many chunks around the player get them.
const RANDOM_TICKS: f32 = 3.0;
const RANDOM_TICK_RADIUS: i32 = 6;

impl Game {
    /// Minecraft's `nextInt(n)`: 0..n.
    fn rand_int(&mut self, n: u32) -> u32 {
        ((self.random() * n as f32) as u32).min(n.saturating_sub(1))
    }

    /// Seconds between fire ticks: 30 to 39 game ticks.
    fn fire_delay(&mut self) -> f32 {
        (30 + self.rand_int(10)) as f32 / 20.0
    }

    /// Fire can stay at `p` on a sturdy block or next to something flammable.
    pub(super) fn fire_survives(w: &World, p: IVec3) -> bool {
        sturdy_top(w.geti(p - IVec3::Y)) || Self::near_flammable(w, p)
    }

    fn near_flammable(w: &World, p: IVec3) -> bool {
        NEIGHBOURS.iter().any(|&d| ignite_odds(w.geti(p + d)) > 0)
    }

    /// Starts tracking a fire block that was just placed (by anyone).
    pub(super) fn fire_placed(&mut self, p: IVec3, age: u8) {
        let timer = self.fire_delay();
        self.fires.insert(p, Fire { age, timer });
    }

    /// Lights a fire at `p` (air) with an age (host and single player).
    pub(super) fn start_fire(&mut self, p: IVec3, age: u8) {
        if self.terrain.world.geti(p) != AIR || !(0..HEIGHT as i32).contains(&p.y) {
            return;
        }
        self.set_block(p, FIRE);
        self.fire_placed(p, age);
    }

    /// Flint and steel on a block face: fire in the air in front of it, if it can burn there.
    /// Returns false when there is no room.
    pub(super) fn light_fire(&mut self, at: IVec3) -> bool {
        let w = &self.terrain.world;
        if w.geti(at) != AIR || !Self::fire_survives(w, at) {
            return false;
        }
        if self.is_client() {
            self.edit_block(at, FIRE);
        } else {
            self.start_fire(at, 0);
        }
        true
    }

    /// Left click on a block face with fire in front of it: puts the fire out.
    pub(super) fn punch_fire(&mut self) -> bool {
        let Some((_, prev)) = self.target else {
            return false;
        };
        if self.terrain.world.geti(prev) != FIRE {
            return false;
        }
        self.edit_block(prev, AIR);
        self.hand.swing();
        self.action_cooldown = 0.25;
        true
    }

    /// Fire ticks and random ticks for this frame (host and single player).
    pub(super) fn update_fire(&mut self, dt: f32) {
        let mut due = Vec::new();
        let w = &self.terrain.world;
        for (p, f) in self.fires.iter_mut() {
            if w.is_loaded(p.x, p.z) {
                f.timer -= dt;
                if f.timer <= 0.0 {
                    due.push(*p);
                }
            }
        }
        // Scan order must not favour a direction.
        due.sort_by_key(|p| (p.x, p.y, p.z));
        for p in due {
            self.fire_tick(p);
        }
        self.random_ticks(dt);
    }

    fn fire_tick(&mut self, p: IVec3) {
        let w = &self.terrain.world;
        if w.geti(p) != FIRE {
            self.fires.remove(&p);
            return;
        }
        if !Self::fire_survives(w, p) {
            self.put_out(p);
            return;
        }
        let below = w.geti(p - IVec3::Y);
        let age = self.fires[&p].age;
        // Ages by one a third of the time.
        let aged = (age + self.rand_int(3) as u8 / 2).min(15);
        let timer = self.fire_delay();
        self.fires.insert(p, Fire { age: aged, timer });
        if !Self::near_flammable(&self.terrain.world, p) {
            // Nothing to burn: it goes out soon (at once without a sturdy block below).
            if !sturdy_top(below) || age > 3 {
                self.put_out(p);
            }
            return;
        }
        if age == 15 && self.rand_int(4) == 0 && ignite_odds(below) == 0 {
            self.put_out(p);
            return;
        }
        // Burn the neighbours: harder to reach up and down.
        for (d, chance) in [
            (IVec3::X, 300),
            (IVec3::NEG_X, 300),
            (IVec3::NEG_Y, 250),
            (IVec3::Y, 250),
            (IVec3::NEG_Z, 300),
            (IVec3::Z, 300),
        ] {
            self.try_burn(p + d, chance, age);
        }
        // Spread to air near flammable blocks: 3x3 around, from one below to four above.
        for dx in -1..=1 {
            for dz in -1..=1 {
                for dy in -1..=4 {
                    if dx == 0 && dy == 0 && dz == 0 {
                        continue;
                    }
                    let q = p + IVec3::new(dx, dy, dz);
                    let w = &self.terrain.world;
                    if w.geti(q) != AIR {
                        continue;
                    }
                    let odds = NEIGHBOURS
                        .iter()
                        .map(|&d| ignite_odds(w.geti(q + d)))
                        .max()
                        .unwrap_or(0);
                    if odds == 0 {
                        continue;
                    }
                    let chance = if dy > 1 {
                        100 + (dy - 1) as u32 * 100
                    } else {
                        100
                    };
                    let t = (odds + 40 + DIFFICULTY * 7) / (age as u32 + 30);
                    if t > 0 && self.rand_int(chance) <= t {
                        let new_age = (age + self.rand_int(5) as u8 / 4).min(15);
                        self.start_fire(q, new_age);
                    }
                }
            }
        }
    }

    /// Fire reaching a neighbour: it may burn away (into fire or nothing); TNT is lit.
    fn try_burn(&mut self, q: IVec3, chance: u32, age: u8) {
        let b = self.terrain.world.geti(q);
        let odds = burn_odds(b);
        if odds == 0 || self.rand_int(chance) >= odds {
            return;
        }
        if b == TNT {
            self.ignite_tnt(q, TNT_FUSE);
            return;
        }
        if self.rand_int(age as u32 + 10) < 5 {
            let new_age = (age + self.rand_int(5) as u8 / 4).min(15);
            self.set_block(q, FIRE);
            self.fire_placed(q, new_age);
        } else {
            self.set_block(q, AIR);
        }
        self.saplings.retain(|(s, _)| *s != q);
        self.block_updated(q);
    }

    pub(super) fn put_out(&mut self, p: IVec3) {
        self.fires.remove(&p);
        if self.terrain.world.geti(p) == FIRE {
            self.set_block(p, AIR);
        }
    }

    /// Minecraft's random ticks (3 random blocks per 16x16x16 section per tick) in the chunks
    /// around the player, for lava lighting fires.
    fn random_ticks(&mut self, dt: f32) {
        let r = RANDOM_TICK_RADIUS;
        let chunks = ((2 * r + 1) * (2 * r + 1)) as f32;
        let sections = (HEIGHT / 16) as f32;
        self.random_tick_budget += dt * 20.0 * RANDOM_TICKS * sections * chunks;
        let count = self.random_tick_budget.floor();
        self.random_tick_budget -= count;
        let (cx, cz) = World::chunk_pos(
            self.player.pos.x.floor() as i32,
            self.player.pos.z.floor() as i32,
        );
        for _ in 0..count as u32 {
            let span = (2 * r + 1) as u32;
            let x = (cx - r + self.rand_int(span) as i32) * 16 + self.rand_int(16) as i32;
            let z = (cz - r + self.rand_int(span) as i32) * 16 + self.rand_int(16) as i32;
            let y = self.rand_int(HEIGHT as u32) as i32;
            let p = IVec3::new(x, y, z);
            if is_lava(self.terrain.world.geti(p)) {
                self.lava_tick(p);
            }
        }
    }

    /// Minecraft's lava random tick: looks up to 3 blocks up and out for air beside a block
    /// lava can light, or beside itself for such a block with air on top.
    fn lava_tick(&mut self, p: IVec3) {
        let tries = self.rand_int(3);
        let lights = |w: &World, q: IVec3| NEIGHBOURS.iter().any(|&d| lava_ignites(w.geti(q + d)));
        if tries > 0 {
            let mut q = p;
            for _ in 0..tries {
                q += IVec3::new(self.rand_int(3) as i32 - 1, 1, self.rand_int(3) as i32 - 1);
                let b = self.terrain.world.geti(q);
                if b == AIR {
                    if lights(&self.terrain.world, q) {
                        self.start_fire(q, 0);
                        return;
                    }
                } else if is_solid(b) {
                    return;
                }
            }
        } else {
            for _ in 0..3 {
                let q = p + IVec3::new(self.rand_int(3) as i32 - 1, 0, self.rand_int(3) as i32 - 1);
                let w = &self.terrain.world;
                if w.geti(q + IVec3::Y) == AIR && lava_ignites(w.geti(q)) {
                    self.start_fire(q + IVec3::Y, 0);
                }
            }
        }
    }
}

const NEIGHBOURS: [IVec3; 6] = [
    IVec3::X,
    IVec3::NEG_X,
    IVec3::Y,
    IVec3::NEG_Y,
    IVec3::Z,
    IVec3::NEG_Z,
];
