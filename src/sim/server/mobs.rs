//! Mobs on the server: their ticks (with their loot, bites and grazing), natural spawning,
//! and what players do to them (hits, wolves given a bone or told to sit, shears, dummies
//! taken down, spawn eggs).

use super::Server;
use crate::entity::mob::{standable, Foe, Mob, MobCtx, MobEvent, MobKind, BITE};
use crate::item::*;
use crate::net::{fx, hurt, Msg};
use crate::world::*;
use glam::{Vec2, Vec3};
use std::f32::consts::TAU;

impl Server {
    /// The mobs' tick: they wander, graze, bite, die (dropping their loot), push each other
    /// and are pushed by the players. (Not in chunks not loaded: they wait there.)
    pub(super) fn update_mobs(&mut self, dt: f32) {
        let people: Vec<(u8, String, Vec3)> = self
            .peers
            .iter()
            .filter_map(|p| p.alive_pose().map(|pose| (p.id, p.name.clone(), pose.pos)))
            .collect();
        let ctx = MobCtx {
            players: people.iter().map(|(_, _, p)| *p).collect(),
            people,
            mobs: self.level.mobs.iter().filter(|m| m.alive()).map(|m| (m.id, m.pos)).collect(),
        };
        let mut i = 0;
        while i < self.level.mobs.len() {
            let p = self.level.mobs[i].pos;
            if !self.world.is_loaded(p.x.floor() as i32, p.z.floor() as i32) {
                i += 1;
                continue;
            }
            let event = self.level.mobs[i].update(dt, &self.world, &ctx);
            if self.level.mobs[i].pos.y < -64.0 {
                self.level.mobs.swap_remove(i);
                continue;
            }
            match event {
                MobEvent::Bite(foe) => {
                    let from = self.level.mobs[i].pos;
                    self.bite(foe, from);
                }
                MobEvent::EatGrass(q) => {
                    let b = if self.world.geti(q) == GRASS { DIRT } else { AIR };
                    self.set_block(q, b);
                    self.block_updated(q);
                }
                MobEvent::Remove => {
                    let m = self.level.mobs.swap_remove(i);
                    self.drop_loot(&m);
                    continue;
                }
                _ => {}
            }
            i += 1;
        }
        // Mobs push each other apart (swept along x: only mobs less than 2 blocks apart that
        // way are compared), and the players push them.
        let mobs = &mut self.level.mobs;
        let mut order: Vec<usize> = (0..mobs.len()).collect();
        order.sort_unstable_by(|&a, &b| mobs[a].pos.x.total_cmp(&mobs[b].pos.x));
        for (i, &a) in order.iter().enumerate() {
            for &b in &order[i + 1..] {
                let (pa, pb) = (mobs[a].pos, mobs[b].pos);
                if pb.x - pa.x > 2.0 {
                    break;
                }
                if (pa - pb).length_squared() > 4.0 {
                    continue;
                }
                let (w, tall) = mobs[b].size();
                if let Some(away) = mobs[a].overlaps(pb, w, tall) {
                    let push = away.normalize_or_zero() * (dt * 12.0).min(1.0);
                    mobs[a].push(push);
                    mobs[b].push(-push);
                }
            }
        }
        for me in &ctx.players {
            for m in mobs.iter_mut() {
                if let Some(away) = m.overlaps(*me, 0.3, 1.8) {
                    m.push(away.normalize_or_zero() * (dt * 20.0).min(1.0));
                }
            }
        }
    }

    /// What a dead mob drops: pigs 1-3 porkchops, sheep 1-2 mutton and their wool (the meat
    /// cooked if they died burning), and now and then a bone.
    fn drop_loot(&mut self, m: &Mob) {
        let c = m.center();
        let burnt = m.fire > 0.0;
        let (meat, most) = match (m.kind, burnt) {
            (MobKind::Pig, false) => (PORKCHOP, 3.0),
            (MobKind::Pig, true) => (COOKED_PORKCHOP, 3.0),
            (MobKind::Sheep, false) => (MUTTON, 2.0),
            (MobKind::Sheep, true) => (COOKED_MUTTON, 2.0),
            // (it never dies; a wolf leaves nothing)
            (MobKind::Dummy | MobKind::Wolf, _) => return,
        };
        let n = 1 + (self.random() * most) as u8;
        self.spawn_drop(c, Stack::new(meat, n.min(most as u8)));
        if self.random() < 1.0 / 3.0 {
            self.spawn_drop(c, Stack::one(BONE));
        }
        if m.kind == MobKind::Sheep && !m.sheared {
            self.spawn_drop(c, Stack::one(WOOL as ItemId));
        }
    }

    /// A wolf (at `from`) bit `foe`.
    fn bite(&mut self, foe: Foe, from: Vec3) {
        match foe {
            Foe::Mob(id) => {
                if let Some(m) = self.level.mobs.iter_mut().find(|m| m.id == id) {
                    m.hurt(BITE, Some(from), 1.0);
                }
            }
            Foe::Player(id) => self.send_to(id, &Msg::Hurt { dmg: BITE, from, knock: 1.0, kind: hurt::WOLF }),
        }
    }

    /// Player `who` attacked `foe`. A wolf hit turns on them, and so does its wild pack;
    /// `who`'s own tame wolves go for `foe`; a player attacked is defended by theirs.
    pub fn attacked(&mut self, foe: Foe, who: u8) {
        let Some(name) = self.player_name(who) else { return };
        let me = Foe::Player(who);
        if let Foe::Mob(id) = foe {
            if let Some((at, wild)) = self.level.mobs.iter().find(|m| m.id == id && m.kind == MobKind::Wolf).map(|m| (m.pos, m.owner.is_none())) {
                for m in self.level.mobs.iter_mut().filter(|m| m.kind == MobKind::Wolf) {
                    let pack = wild && m.owner.is_none() && m.pos.distance(at) < 12.0;
                    if m.id == id || pack {
                        m.provoke(me, Some(&name));
                    }
                }
            }
        }
        let foe_name = match foe {
            Foe::Player(id) => self.player_name(id),
            Foe::Mob(_) => None,
        };
        for m in self.level.mobs.iter_mut().filter(|m| m.kind == MobKind::Wolf && m.owner.is_some()) {
            let own = m.owner.as_deref() == Some(name.as_str());
            if own && Foe::Mob(m.id) != foe {
                m.provoke(foe, foe_name.as_deref());
            }
            if foe_name.is_some() && m.owner == foe_name {
                m.provoke(me, Some(&name));
            }
        }
    }

    /// Player `who` used `item` on wolf `i`: a wild one given a bone may take to them (or not),
    /// a tame one of theirs sits down or stands up.
    pub(super) fn wolf_used(&mut self, i: usize, item: ItemId, who: u8) {
        let Some(name) = self.player_name(who) else { return };
        let m = &mut self.level.mobs[i];
        if m.kind != MobKind::Wolf || !m.alive() {
            return;
        }
        if item == BONE && m.owner.is_none() && m.foe.is_none() {
            let took = m.feed_bone(&name);
            let head = m.pos + Vec3::Y * 0.6;
            let kind = if took { fx::WOLF_TAKES } else { fx::WOLF_REFUSES };
            self.broadcast(&Msg::Fx { kind, pos: head }, None);
            return;
        }
        if m.owner.as_deref() == Some(name.as_str()) {
            m.toggle_sit();
        }
    }

    /// Shears a sheep, dropping its wool.
    pub(super) fn shear_mob(&mut self, i: usize) {
        let m = &mut self.level.mobs[i];
        if !m.can_shear() {
            return;
        }
        m.sheared = true;
        let at = m.pos + Vec3::Y * 1.0;
        let n = 1 + (self.random() * 3.0) as u8;
        self.spawn_drop(at, Stack::new(WOOL as ItemId, n.min(3)));
    }

    /// Takes a target dummy down, dropping it as an item (`drop`: not in creative).
    pub(super) fn break_dummy(&mut self, i: usize, drop: bool) {
        if self.level.mobs[i].kind != MobKind::Dummy {
            return;
        }
        let m = self.level.mobs.swap_remove(i);
        let c = m.center();
        self.broadcast(&Msg::Fx { kind: fx::POOF, pos: c }, None);
        if drop {
            self.spawn_drop(c, Stack::one(TARGET_DUMMY));
        }
    }

    pub fn spawn_mob(&mut self, kind: MobKind, pos: Vec3) {
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
        self.level.mobs.push(m);
    }

    /// Minecraft-like animal spawning: now and then a group of sheep or pigs (Minecraft's
    /// weights: 12 to 10) appears on grass under the open sky, 24-64 blocks from a player and
    /// out of their sight, while fewer than 10 animals are around; in forests and taigas now
    /// and then a pack of wolves instead.
    pub(super) fn spawn_animals(&mut self, dt: f32) {
        self.level.mob_spawn_timer -= dt;
        if self.level.mob_spawn_timer > 0.0 {
            return;
        }
        self.level.mob_spawn_timer = 3.0 + self.random() * 4.0;
        let players: Vec<crate::net::Pose> = self.peers.iter().filter_map(|p| p.alive_pose()).collect();
        if players.is_empty() {
            return;
        }
        let pick = (self.random() * players.len() as f32) as usize;
        let pose = players[pick.min(players.len() - 1)];
        let me = pose.pos;
        let near = self
            .level
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
        // Not right in front of them.
        if off.normalize().dot(crate::entity::player::look_dir(pose.yaw, 0.0)) > 0.3 {
            return;
        }
        let group = 2 + (self.random() * 3.0) as i32;
        let mut spots = Vec::new();
        for k in 0..group * 3 {
            if spots.len() as i32 >= group {
                break;
            }
            let jitter = if k == 0 { Vec3::ZERO } else { Vec3::new(self.random() - 0.5, 0.0, self.random() - 0.5) * 6.0 };
            let c = me + off + jitter;
            let (x, z) = (c.x.floor() as i32, c.z.floor() as i32);
            let w = &self.world;
            let Some(top) = w.height_at(x, z) else { continue };
            if w.get(x, top, z) != GRASS {
                continue;
            }
            if let Some((p, GRASS)) = standable(w, x, z, top + 1, 0) {
                spots.push(p);
            }
        }
        let c = me + off;
        let biome = self.gen.column(c.x.floor() as i32, c.z.floor() as i32).biome;
        use crate::world::gen::Biome;
        let woods = matches!(biome, Biome::Forest | Biome::BirchForest | Biome::Taiga | Biome::SnowyTaiga);
        let kind = if woods && self.random() < 0.35 {
            MobKind::Wolf
        } else if self.random() < 12.0 / 22.0 {
            MobKind::Sheep
        } else {
            MobKind::Pig
        };
        if kind == MobKind::Wolf && spots.is_empty() {
            // (the snowy taiga's ground is snowy grass)
            for k in 0..group * 3 {
                let jitter = Vec3::new(k as f32 * 0.37 % 1.0 - 0.5, 0.0, k as f32 * 0.61 % 1.0 - 0.5) * 6.0;
                let c = me + off + jitter;
                let (x, z) = (c.x.floor() as i32, c.z.floor() as i32);
                let w = &self.world;
                let Some(top) = w.height_at(x, z) else { continue };
                if w.get(x, top, z) == SNOWY_GRASS {
                    if let Some((p, _)) = standable(w, x, z, top + 1, 0) {
                        spots.push(p);
                    }
                }
                if spots.len() as i32 >= group {
                    break;
                }
            }
        }
        for p in spots {
            self.spawn_mob(kind, p);
        }
    }
}
