//! TNT: lighting it with flint and steel, the fuse, and explosions (Minecraft's: rays that
//! blast resistance soaks up, damage and knockback by distance and exposure). On a LAN, the
//! host runs the explosion and every player feels it and sees the smoke.

use super::*;
use crate::entity::{PrimedTnt, TNT_FUSE};
use crate::item::inventory;
use crate::item::*;

/// Explosion strength of TNT.
pub(super) const TNT_POWER: f32 = 4.0;

/// How strongly an explosion at `center` hits a box (`min`..`max`, feet at `feet`): 0..1 from
/// distance and how much of the box it can see, and the direction it pushes.
fn impact(
    w: &World,
    center: Vec3,
    power: f32,
    feet: Vec3,
    min: Vec3,
    max: Vec3,
) -> Option<(f32, Vec3)> {
    let d = feet.distance(center) / (power * 2.0);
    if d > 1.0 {
        return None;
    }
    let target = (min + max) * 0.5 + Vec3::Y * (max.y - min.y) * 0.3;
    let dir = (target - center).try_normalize().unwrap_or(Vec3::Y);
    // Exposure: the share of points in the box with a clear line to the explosion.
    let mut seen = 0;
    let n = 3;
    for i in 0..n {
        for j in 0..n {
            for k in 0..n {
                let t = Vec3::new(i as f32, j as f32, k as f32) / (n - 1) as f32;
                let p = min + (max - min) * t;
                let (dist, step) = (p.distance(center), 0.25);
                let ray = (center - p) / dist.max(1e-4);
                let blocked = (1..(dist / step) as i32).any(|s| {
                    let q = (p + ray * s as f32 * step).floor().as_ivec3();
                    is_solid(w.geti(q))
                });
                seen += !blocked as i32;
            }
        }
    }
    let exposure = seen as f32 / (n * n * n) as f32;
    let impact = (1.0 - d) * exposure;
    (impact > 0.0).then_some((impact, dir))
}

/// Minecraft's explosion damage for an impact.
fn blast_damage(impact: f32, power: f32) -> f32 {
    ((impact * impact + impact) * 0.5 * 7.0 * power * 2.0 + 1.0).floor()
}

/// Speed (blocks per second) an explosion gives at full impact.
const BLAST_PUSH: f32 = 18.0;

impl Game {
    /// Right click with flint and steel: lights the TNT it is aimed at.
    pub(super) fn use_flint_and_steel(&mut self) {
        let Some((hit, prev)) = self.target else {
            return;
        };
        if self.terrain.world.geti(hit) == TNT {
            if self.is_client() {
                self.set_block(hit, AIR);
                self.send(crate::net::Msg::Ignite { p: hit });
            } else {
                self.ignite_tnt(hit, TNT_FUSE);
            }
        } else if !self.light_fire(prev) {
            // No room for fire: just sparks where it strikes.
            let face = (prev - hit).as_vec3();
            let at = hit.as_vec3() + Vec3::splat(0.5) + face * 0.52;
            for _ in 0..4 {
                let jitter = Vec3::new(self.random(), self.random(), self.random()) - 0.5;
                self.particles
                    .flame(at + jitter * (Vec3::ONE - face.abs()) * 0.6);
            }
        }
        if !self.creative() {
            let slot = self.hotbar_slot;
            inventory::damage(&mut self.inventory.slots[slot], 1);
        }
        self.hand.swing();
        self.action_cooldown = 0.25;
    }

    /// A TNT block becomes primed TNT that explodes after `fuse` seconds (host and single
    /// player).
    pub(super) fn ignite_tnt(&mut self, p: IVec3, fuse: f32) {
        if self.terrain.world.geti(p) != TNT {
            return;
        }
        self.set_block(p, AIR);
        self.block_updated(p);
        // Minecraft's little random hop.
        let a = self.random() * TAU;
        self.tnt.push(PrimedTnt {
            pos: p.as_vec3() + Vec3::new(0.5, 0.0, 0.5),
            vel: Vec3::new(-a.sin() * 0.4, 4.0, -a.cos() * 0.4),
            fuse,
        });
    }

    /// Primed TNT: falls, and explodes when its fuse runs out.
    pub(super) fn update_tnt(&mut self, dt: f32) {
        let mut boom = Vec::new();
        let mut i = 0;
        while i < self.tnt.len() {
            let p = self.tnt[i].pos;
            if !self
                .terrain
                .world
                .is_loaded(p.x.floor() as i32, p.z.floor() as i32)
            {
                i += 1;
                continue;
            }
            if self.tnt[i].update(dt, &self.terrain.world) || p.y < -64.0 {
                boom.push(self.tnt.swap_remove(i).center());
            } else {
                i += 1;
            }
        }
        for c in boom {
            self.explode(c, TNT_POWER);
        }
    }

    /// An explosion (host and single player): hurts and pushes everything around it, then
    /// blows up the blocks its rays reach. Other TNT it hits is lit with a short fuse.
    pub(super) fn explode(&mut self, center: Vec3, power: f32) {
        // Everyone feels it (before the blocks go, like Minecraft, so walls give cover).
        if self.is_host() {
            self.broadcast(&crate::net::Msg::Explosion { pos: center, power }, None);
        }
        self.explosion_effects(center, power);
        let w = &self.terrain.world;
        let hits: Vec<(usize, f32, Vec3)> = self
            .mobs
            .iter()
            .enumerate()
            .filter(|(_, m)| m.alive())
            .filter_map(|(i, m)| {
                let (min, max) = (
                    m.pos - Vec3::new(0.45, 0.0, 0.45),
                    m.pos + Vec3::new(0.45, 0.9, 0.45),
                );
                impact(w, center, power, m.pos, min, max).map(|(k, dir)| (i, k, dir))
            })
            .collect();
        for (i, k, dir) in hits {
            let m = &mut self.mobs[i];
            m.hurt(blast_damage(k, power), Some(center), 0.0);
            m.vel += dir * k * BLAST_PUSH;
        }
        // Dropped items nearby are destroyed; other primed TNT is thrown around.
        let reach = power * 2.0;
        self.items
            .retain(|it| it.is_picking_up() || it.pos.distance(center) > reach * 0.5);
        for t in &mut self.tnt {
            let (min, max) = (
                t.pos - Vec3::new(0.49, 0.0, 0.49),
                t.pos + Vec3::new(0.49, 0.98, 0.49),
            );
            if let Some((k, dir)) = impact(&self.terrain.world, center, power, t.pos, min, max) {
                t.vel += dir * k * BLAST_PUSH;
            }
        }

        // Blocks: rays in every direction from a 16x16x16 grid's surface, each losing strength
        // as it goes and more through tougher blocks.
        let mut blown = Vec::new();
        let mut seen = FastSet::default();
        for i in 0..16 {
            for j in 0..16 {
                for k in 0..16 {
                    if ![i, j, k].iter().any(|&c| c == 0 || c == 15) {
                        continue;
                    }
                    let dir =
                        (Vec3::new(i as f32, j as f32, k as f32) / 15.0 * 2.0 - 1.0).normalize();
                    let mut strength = power * (0.7 + self.random() * 0.6);
                    let mut p = center;
                    while strength > 0.0 {
                        let q = p.floor().as_ivec3();
                        let b = self.terrain.world.geti(q);
                        if b != AIR {
                            strength -= (blast_resistance(b) + 0.3) * 0.3;
                            if strength > 0.0 && (0..HEIGHT as i32).contains(&q.y) && seen.insert(q)
                            {
                                blown.push(q);
                            }
                        }
                        p += dir * 0.3;
                        strength -= 0.225;
                    }
                }
            }
        }
        for &q in &blown {
            let b = self.terrain.world.geti(q);
            if b == TNT {
                let fuse = 0.5 + self.random();
                self.ignite_tnt(q, fuse);
                continue;
            }
            let contents = self.block_entities.remove(q);
            self.split_chest(q, b);
            self.set_block(q, AIR);
            self.saplings.retain(|(s, _)| *s != q);
            let center = q.as_vec3() + Vec3::splat(0.5);
            // Some of the blocks drop (one in `power`), whatever tool they would need.
            if self.random() < 1.0 / power {
                let r = self.random();
                for s in harvest_drops(b, r) {
                    self.spawn_drop(center, s);
                }
            }
            for s in contents {
                self.spawn_drop(center, s);
            }
        }
        for q in blown {
            self.block_updated(q);
        }
    }

    /// What every player sees and feels of an explosion: smoke and flames, and their own
    /// damage and knockback.
    pub(super) fn explosion_effects(&mut self, center: Vec3, power: f32) {
        let w = &self.terrain.world;
        let (sky, blk) = (w.sky_estimate(center), w.block_light_estimate(center));
        self.particles.explosion(center, sky, blk);
        if !self.player.spawned || self.screen == Screen::Dead {
            return;
        }
        let p = self.player.pos;
        let (min, max) = (p - Vec3::new(0.3, 0.0, 0.3), p + Vec3::new(0.3, 1.8, 0.3));
        let Some((k, dir)) = impact(&self.terrain.world, center, power, p, min, max) else {
            return;
        };
        self.damage(blast_damage(k, power), "death.explosion");
        if !(self.creative() && self.player.flying) {
            self.player.vel += dir * k * BLAST_PUSH;
            self.player.on_ground = false;
        }
    }
}
