//! Mobs from the game's side: attacking mobs and other players, spawn eggs, natural
//! spawning, and updating them (with their loot).

use super::*;
use crate::item::inventory::{self, take};
use crate::item::*;

impl Game {
    /// Left click on a mob (index into `mobs`) or another LAN player: damage by the held item
    /// (x1.5 for a critical hit while falling), knockback (more while sprinting), and tool
    /// wear like Minecraft (swords 1, tools 2).
    pub(super) fn attack(&mut self, mob: Option<usize>, player: Option<u8>) {
        // Sneaking, a hit takes a target dummy down (it drops as an item).
        if let Some(i) = mob.filter(|&i| self.mobs[i].kind == MobKind::Dummy && self.player.sneaking) {
            if self.is_client() {
                let id = self.mobs[i].id;
                self.send(crate::net::Msg::BreakDummy { id });
            } else {
                self.break_dummy(i, !self.creative());
            }
            return;
        }
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
                    let kind = crate::net::hurt::MELEE;
                    self.send(Msg::AttackPlayer { id, dmg, knock, kind });
                } else {
                    let kind = crate::net::hurt::MELEE;
                    self.send_to(id, &Msg::Hurt { dmg, from, knock, kind });
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

    /// Right click on a sheep with shears: 1-3 wool pops off (the host drops it).
    pub(super) fn shear(&mut self, i: usize) {
        if !self.mobs[i].can_shear() {
            return;
        }
        if self.is_client() {
            let id = self.mobs[i].id;
            self.mobs[i].sheared = true;
            self.send(crate::net::Msg::Shear { id });
        } else {
            self.shear_mob(i);
        }
        if !self.creative() {
            let slot = self.hotbar_slot;
            inventory::damage(&mut self.inventory.slots[slot], 1);
        }
        self.hand.swing();
        self.action_cooldown = 0.25;
    }

    /// Host: shears a sheep and drops its wool.
    pub(super) fn shear_mob(&mut self, i: usize) {
        let m = &mut self.mobs[i];
        if !m.can_shear() {
            return;
        }
        m.sheared = true;
        let at = m.pos + Vec3::Y * 1.0;
        let n = 1 + (self.random() * 3.0) as u8;
        self.spawn_drop(at, Stack::new(WOOL as ItemId, n.min(3)));
    }

    /// Host: takes a target dummy down, dropping it as an item (`drop`).
    pub(super) fn break_dummy(&mut self, i: usize, drop: bool) {
        if self.mobs[i].kind != MobKind::Dummy {
            return;
        }
        let m = self.mobs.swap_remove(i);
        let c = m.center();
        let (sky, blk) = (self.terrain.world.sky_estimate(c), self.terrain.world.block_light_estimate(c));
        self.particles.poof(c, sky, blk);
        if drop {
            self.spawn_drop(c, Stack::one(TARGET_DUMMY));
        }
    }

    pub(super) fn spawn_mob(&mut self, kind: MobKind, pos: Vec3) {
        if self.is_client() {
            self.send(crate::net::Msg::SpawnMob {
                kind: kind as u8,
                pos,
            });
            return;
        }
        let mut yaw = self.random() * TAU;
        if kind == MobKind::Dummy {
            // A dummy is set up facing whoever is nearest (the one setting it up).
            let near = self.player_positions().into_iter().min_by(|a, b| a.distance_squared(pos).total_cmp(&b.distance_squared(pos)));
            if let Some(p) = near {
                let d = p - pos;
                yaw = d.z.atan2(d.x);
            }
        }
        let seed = (self.random() * 16_777_216.0) as u32;
        let mut m = Mob::new(kind, pos, yaw, seed);
        m.id = self.entity_id();
        self.mobs.push(m);
    }

    /// Minecraft-like animal spawning: now and then a group of sheep or pigs (Minecraft's
    /// weights: 12 to 10) appears on grass under the open sky, 24-64 blocks from the player
    /// and out of sight, while fewer than 10 animals are around.
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
            .filter(|m| m.kind != MobKind::Dummy && Vec2::new(m.pos.x - me.x, m.pos.z - me.z).length() < 96.0)
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
        let kind = if self.random() < 12.0 / 22.0 {
            MobKind::Sheep
        } else {
            MobKind::Pig
        };
        for p in spots {
            self.spawn_mob(kind, p);
        }
    }

    pub(super) fn update_mobs(&mut self, dt: f32) {
        let ctx = MobCtx {
            players: self.player_positions(),
        };
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
            if let MobEvent::EatGrass(q) = event {
                let b = if self.terrain.world.geti(q) == GRASS {
                    DIRT
                } else {
                    AIR
                };
                self.set_block(q, b);
                self.block_updated(q);
            }
            if let MobEvent::Remove = event {
                let m = self.mobs.swap_remove(i);
                let c = m.center();
                let (sky, blk) = (
                    self.terrain.world.sky_estimate(c),
                    self.terrain.world.block_light_estimate(c),
                );
                self.particles.poof(c, sky, blk);
                // Pigs drop 1-3 porkchops, sheep 1-2 mutton and their wool; the meat is
                // cooked if they died burning.
                let burnt = m.fire > 0.0;
                let (meat, most) = match (m.kind, burnt) {
                    (MobKind::Pig, false) => (PORKCHOP, 3.0),
                    (MobKind::Pig, true) => (COOKED_PORKCHOP, 3.0),
                    (MobKind::Sheep, false) => (MUTTON, 2.0),
                    (MobKind::Sheep, true) => (COOKED_MUTTON, 2.0),
                    // (it never dies)
                    (MobKind::Dummy, _) => continue,
                };
                let n = 1 + (self.random() * most) as u8;
                self.spawn_drop(c, Stack::new(meat, n.min(most as u8)));
                if m.kind == MobKind::Sheep && !m.sheared {
                    self.spawn_drop(c, Stack::one(WOOL as ItemId));
                }
                continue;
            }
            i += 1;
        }
        // Mobs push each other apart, and the player pushes them.
        let n = self.mobs.len();
        for a in 0..n {
            for b in a + 1..n {
                let (pa, pb) = (self.mobs[a].pos, self.mobs[b].pos);
                if (pa - pb).length_squared() > 4.0 {
                    continue;
                }
                let (w, tall) = self.mobs[b].size();
                if let Some(away) = self.mobs[a].overlaps(pb, w, tall) {
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
