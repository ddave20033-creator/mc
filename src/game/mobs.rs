//! Mobs from the game's side: attacking mobs and other players, spawn eggs, natural
//! spawning (animals on grass, creepers in the dark), despawning, and updating them (with
//! their loot and creeper explosions).

use super::*;
use crate::item::inventory::{self, take};
use crate::item::*;

impl Game {
    /// Left click on a mob (index into `mobs`) or another LAN player: damage by the held item
    /// (x1.5 for a critical hit while falling), knockback (more while sprinting), and tool
    /// wear like Minecraft (swords 1, tools 2).
    pub(super) fn attack(&mut self, mob: Option<usize>, player: Option<u8>) {
        let held = self.held();
        let mut dmg = attack_damage(held);
        let falling = !self.player.on_ground && !self.player.flying && self.player.vel.y < 0.0;
        if falling && self.player.fluid(&self.terrain.world) == AIR {
            dmg *= 1.5;
        }
        let knock = if self.player.sprinting { 2.0 } else { 1.0 };
        let from = self.player.pos;
        if !self.creative() {
            self.needs.exhaust(crate::entity::survival::cost::ATTACK);
        }
        use crate::net::Msg;
        let hit = match (mob, player) {
            (Some(i), _) if self.is_client() => {
                // The host has the real mob; it can be hit when it is not still flashing red.
                let m = &self.mobs[i];
                let can = m.alive() && m.hurt_time <= 0.0;
                let id = m.id;
                self.send(Msg::AttackMob { id, dmg, knock });
                can
            }
            (Some(i), _) => self.mobs[i].hurt(dmg, Some(from), knock),
            (None, Some(id)) => {
                if self.is_client() {
                    self.send(Msg::AttackPlayer { id, dmg, knock });
                } else {
                    self.send_to(id, &Msg::Hurt { dmg, from, knock });
                }
                true
            }
            _ => false,
        };
        let hit_at = match (mob, player) {
            (Some(i), _) => self.mobs[i].center(),
            (None, Some(id)) => self.remote_pos(id).unwrap_or(from) + Vec3::Y,
            _ => from,
        };
        if hit && !self.creative() {
            let wear = match tool_of(held) {
                Some((ToolKind::Sword, _)) => 1,
                Some(_) => 2,
                None => 0,
            };
            if wear > 0 {
                let slot = self.hotbar_slot;
                if inventory::damage(&mut self.inventory.slots[slot], wear) {
                    let p = hit_at.floor().as_ivec3();
                    self.particles
                        .burst(&self.terrain.world, p, STONE, 12, [255; 3]);
                }
            }
        }
        if self.player.sprinting {
            // Hitting while sprinting stops the sprint (Minecraft).
            self.player.sprinting = false;
            self.w_sprint = false;
        }
    }

    /// Spawn egg on a block: the mob appears on the clicked face.
    pub(super) fn use_spawn_egg(&mut self, kind: MobKind) {
        let Some((hit, prev)) = self.target else {
            return;
        };
        let at = if is_solid(self.terrain.world.geti(prev)) {
            hit
        } else {
            prev
        };
        let pos = Vec3::new(at.x as f32 + 0.5, at.y as f32, at.z as f32 + 0.5);
        self.spawn_mob(kind, pos);
        if !self.creative() {
            take(&mut self.inventory.slots[self.hotbar_slot], 1);
        }
        self.hand.swing();
        self.action_cooldown = 0.25;
    }

    pub(super) fn spawn_mob(&mut self, kind: MobKind, pos: Vec3) {
        if self.is_client() {
            self.send(crate::net::Msg::SpawnMob {
                kind: kind as u8,
                pos,
            });
            return;
        }
        let yaw = self.random() * TAU;
        let seed = (self.random() * 16_777_216.0) as u32;
        let mut m = Mob::new(kind, pos, yaw, seed);
        m.id = self.entity_id();
        self.mobs.push(m);
    }

    /// Minecraft-like animal spawning: now and then a group of pigs appears on grass under the
    /// open sky, 24-64 blocks from the player and out of sight, while fewer than 10 animals
    /// are around.
    pub(super) fn spawn_animals(&mut self, dt: f32) {
        self.mob_spawn_timer -= dt;
        if self.mob_spawn_timer > 0.0 {
            return;
        }
        self.mob_spawn_timer = 3.0 + self.random() * 4.0;
        // Around a random player (the host or a LAN player).
        let players = self.player_positions();
        if players.is_empty() {
            return;
        }
        let pick = (self.random() * players.len() as f32) as usize;
        let me = players[pick.min(players.len() - 1)];
        let own = me == self.player.pos;
        let near = self
            .mobs
            .iter()
            .filter(|m| !m.kind.hostile())
            .filter(|m| Vec2::new(m.pos.x - me.x, m.pos.z - me.z).length() < 96.0)
            .count();
        if near >= 10 {
            return;
        }
        let ang = self.random() * TAU;
        let dist = 24.0 + self.random() * 40.0;
        let off = Vec3::new(ang.cos(), 0.0, ang.sin()) * dist;
        // Not right in front of the camera.
        if own && off.normalize().dot(look_dir(self.yaw, 0.0)) > 0.3 {
            return;
        }
        let group = 2 + (self.random() * 3.0) as i32;
        let mut spots = Vec::new();
        for k in 0..group * 3 {
            if spots.len() as i32 >= group {
                break;
            }
            let jitter = if k == 0 {
                Vec3::ZERO
            } else {
                Vec3::new(self.random() - 0.5, 0.0, self.random() - 0.5) * 6.0
            };
            let c = me + off + jitter;
            let (x, z) = (c.x.floor() as i32, c.z.floor() as i32);
            let w = &self.terrain.world;
            let Some(top) = w.height_at(x, z) else {
                continue;
            };
            if w.get(x, top, z) != GRASS {
                continue;
            }
            if let Some((p, GRASS)) = crate::entity::mob::standable(w, x, z, top + 1, 0) {
                spots.push(p);
            }
        }
        for p in spots {
            self.spawn_mob(MobKind::Pig, p);
        }
    }

    /// Minecraft's monster spawning (1.18+): now and then a creeper appears on a solid block
    /// 24-64 blocks from a player where it is dark enough: no block light at all, and sky
    /// light (dimmed at night) at most a random 0..7, so on the surface only at night but
    /// in caves any time. At most 20 monsters around.
    pub(super) fn spawn_monsters(&mut self, dt: f32) {
        self.monster_spawn_timer -= dt;
        if self.monster_spawn_timer > 0.0 {
            return;
        }
        self.monster_spawn_timer = 0.5;
        let players = self.player_positions();
        if players.is_empty() {
            return;
        }
        let near = |p: Vec3, r: f32| players.iter().any(|q| q.distance(p) < r);
        let monsters = self
            .mobs
            .iter()
            .filter(|m| m.kind.hostile() && near(m.pos, 96.0))
            .count();
        if monsters >= 20 {
            return;
        }
        // Minecraft's sky darkening: 0 by day, 11 at night.
        let sun = (self.time_of_day * TAU).sin();
        let darken = ((1.0 - (sun * 2.0 + 0.5).clamp(0.0, 1.0)) * 11.0).round() as i32;
        // Most tries miss (a spot inside rock, in the light); a few land.
        for _ in 0..16 {
            let pick = (self.random() * players.len() as f32) as usize;
            let me = players[pick.min(players.len() - 1)];
            let ang = self.random() * TAU;
            let dist = 24.0 + self.random() * 40.0;
            let c = me + Vec3::new(ang.cos(), 0.0, ang.sin()) * dist;
            let (x, z) = (c.x.floor() as i32, c.z.floor() as i32);
            let (ry, limit) = (self.random(), (self.random() * 8.0) as i32);
            let w = &self.terrain.world;
            let Some(top) = w.height_at(x, z) else {
                continue;
            };
            let y = 1 + (ry * (top + 1) as f32) as i32;
            let Some((p, ground)) = crate::entity::mob::standable(w, x, z, y, 0) else {
                continue;
            };
            let head = w.get(x, p.y as i32 + 1, z);
            if !sturdy_top(ground) || is_fluid(ground) || is_fluid(head) || near(p, 24.0) {
                continue;
            }
            // Sky light: full under the open sky, none a few blocks under cover.
            let depth = top - p.y as i32;
            let sky = if depth < 0 {
                15
            } else if depth < 2 {
                8
            } else {
                0
            };
            if sky - darken > limit || Self::block_lit(w, p.floor().as_ivec3()) {
                continue;
            }
            self.spawn_mob(MobKind::Creeper, p);
        }
    }

    /// Any block light at `p`: a light source close enough for its light to reach
    /// (Minecraft's light drops by one per block).
    fn block_lit(w: &World, p: IVec3) -> bool {
        const R: i32 = 14;
        for dy in -R..=R {
            for dz in -R..=R {
                let left = R - dy.abs() - dz.abs();
                if left < 0 {
                    continue;
                }
                for dx in -left..=left {
                    let e = emission(w.geti(p + IVec3::new(dx, dy, dz)));
                    if e as i32 > dx.abs() + dy.abs() + dz.abs() {
                        return true;
                    }
                }
            }
        }
        false
    }

    pub(super) fn update_mobs(&mut self, dt: f32) {
        let ctx = MobCtx {
            players: self.player_positions(),
            prey: self.prey_positions(),
        };
        let mut explosions = Vec::new();
        let mut i = 0;
        while i < self.mobs.len() {
            let p = self.mobs[i].pos;
            // Mobs in unloaded chunks wait there.
            if !self
                .terrain
                .world
                .is_loaded(p.x.floor() as i32, p.z.floor() as i32)
            {
                i += 1;
                continue;
            }
            let event = self.mobs[i].update(dt, &self.terrain.world, &ctx);
            if p.y < -64.0 {
                self.mobs.swap_remove(i);
                continue;
            }
            // Monsters despawn: at once far from every player, now and then when not close
            // (Minecraft: past 128 blocks, and 1 in 800 per tick past 32).
            if self.mobs[i].kind.hostile() {
                let d = ctx
                    .players
                    .iter()
                    .map(|q| q.distance(p))
                    .fold(f32::INFINITY, f32::min);
                let chance = 1.0 - (1.0 - 1.0 / 800.0f32).powf(dt * 20.0);
                if d > 128.0 || (d > 32.0 && self.random() < chance) {
                    self.mobs.swap_remove(i);
                    continue;
                }
            }
            if let MobEvent::Explode = event {
                let m = self.mobs.swap_remove(i);
                explosions.push(m.center());
                continue;
            }
            if let MobEvent::Remove = event {
                let m = self.mobs.swap_remove(i);
                let c = m.center();
                let (sky, blk) = (
                    self.terrain.world.sky_estimate(c),
                    self.terrain.world.block_light_estimate(c),
                );
                self.particles.poof(c, sky, blk);
                match m.kind {
                    MobKind::Pig => {
                        // 1-3 porkchops, cooked if it died burning.
                        let meat = if m.fire > 0.0 {
                            COOKED_PORKCHOP
                        } else {
                            PORKCHOP
                        };
                        let n = 1 + (self.random() * 3.0) as u8;
                        self.spawn_drop(c, Stack::new(meat, n.min(3)));
                    }
                    MobKind::Creeper => {
                        // 0-2 gunpowder.
                        let n = (self.random() * 3.0) as u8;
                        if n > 0 {
                            self.spawn_drop(c, Stack::new(GUNPOWDER, n.min(2)));
                        }
                    }
                }
                continue;
            }
            i += 1;
        }
        for c in explosions {
            self.explode(c, crate::entity::mob::CREEPER_POWER);
        }
        // Mobs push each other apart, and the player pushes them.
        let n = self.mobs.len();
        for a in 0..n {
            for b in a + 1..n {
                let (pa, pb) = (self.mobs[a].pos, self.mobs[b].pos);
                if (pa - pb).length_squared() > 4.0 {
                    continue;
                }
                let (hw, tall) = self.mobs[b].kind.size();
                if let Some(away) = self.mobs[a].overlaps(pb, hw, tall) {
                    let push = away.normalize_or_zero() * (dt * 12.0).min(1.0);
                    self.mobs[a].push(push);
                    self.mobs[b].push(-push);
                }
            }
        }
        // Players push them too.
        for me in &ctx.players {
            for m in &mut self.mobs {
                if let Some(away) = m.overlaps(*me, 0.3, 1.8) {
                    m.push(away.normalize_or_zero() * (dt * 20.0).min(1.0));
                }
            }
        }
    }
}
