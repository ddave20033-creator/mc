//! Mobs on the server: their ticks (with their loot, attacks and grazing), natural spawning
//! (by their `Spawn` rules), and what players do to them (hits, items used on them, static
//! ones taken down, spawn eggs). What a kind does is in its file (`content::mobs`).

use super::Server;
use crate::content::mobs::{Behavior, MobDef, MOBS};
use crate::entity::mob::{standable, Foe, Mob, MobCtx, MobEvent, MobKind};
use crate::item::*;
use crate::net::{fx, Msg};
use crate::world::*;
use glam::{Vec2, Vec3};
use std::f32::consts::TAU;

/// Animals near a player (within 96 blocks) above which no more appear.
const MOB_CAP: usize = 10;

impl Server {
    /// The mobs' tick: they wander, graze, attack, die (dropping their loot), push each other
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
                MobEvent::Attack(foe) => {
                    let m = &self.level.mobs[i];
                    if let Some(attack) = m.def().attack {
                        let from = m.pos;
                        self.mob_attacks(foe, from, attack);
                    }
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
                MobEvent::None => {}
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

    /// What a dead mob drops: its `loot` (burnt, if it died on fire) and what its kind adds.
    fn drop_loot(&mut self, m: &Mob) {
        let c = m.center();
        let def = m.def();
        for l in def.loot {
            if self.random() >= l.chance {
                continue;
            }
            let item = if m.burnt { l.burnt.unwrap_or(l.item) } else { l.item };
            let n = l.min + (self.random() * (l.max - l.min + 1) as f32) as u8;
            self.spawn_drop(c, Stack::new(item, n.min(l.max)));
        }
        if let Some(extra) = def.hooks.loot {
            for s in extra(m) {
                self.spawn_drop(c, s);
            }
        }
    }

    /// A mob (at `from`) attacked `foe`.
    fn mob_attacks(&mut self, foe: Foe, from: Vec3, attack: crate::content::mobs::Attack) {
        match foe {
            Foe::Mob(id) => {
                if let Some(m) = self.level.mobs.iter_mut().find(|m| m.id == id) {
                    m.hurt(attack.damage, Some(from), 1.0);
                }
            }
            Foe::Player(id) => self.send_to(id, &Msg::Hurt { dmg: attack.damage, from, knock: 1.0, kind: attack.kind }),
        }
    }

    /// Player `who` attacked `foe`: see `provoke_all`.
    pub fn attacked(&mut self, foe: Foe, who: u8) {
        let Some(name) = self.player_name(who) else { return };
        let foe_name = match foe {
            Foe::Player(id) => self.player_name(id),
            Foe::Mob(_) => None,
        };
        provoke_all(&mut self.level.mobs, foe, who, &name, foe_name.as_deref());
    }

    /// Player `who` used `item` on mob `i` (what its kind does with it: `Hooks::used`).
    pub(super) fn use_on_mob(&mut self, i: usize, item: ItemId, who: u8) {
        let Some(name) = self.player_name(who) else { return };
        let Some(used) = self.level.mobs[i].def().hooks.used else { return };
        let r = self.random();
        let done = used(&mut self.level.mobs[i], item, &name, r);
        if let Some((kind, pos)) = done.fx {
            self.broadcast(&Msg::Fx { kind, pos }, None);
        }
        if let Some((at, stack)) = done.drop {
            self.spawn_drop(at, stack);
        }
    }

    /// Takes a static mob (a target dummy) down, dropping it as its item (`drop`: not in
    /// creative).
    pub(super) fn take_down(&mut self, i: usize, drop: bool) {
        if self.level.mobs[i].def().behavior != Behavior::Static {
            return;
        }
        let m = self.level.mobs.swap_remove(i);
        let c = m.center();
        self.broadcast(&Msg::Fx { kind: fx::POOF, pos: c }, None);
        if drop {
            self.spawn_drop(c, Stack::one(m.def().egg));
        }
    }

    pub fn spawn_mob(&mut self, kind: MobKind, pos: Vec3) {
        let mut yaw = self.random() * TAU;
        if kind.def().behavior == Behavior::Static {
            // A static one is set up facing whoever is nearest (the one setting it up).
            let near = self.player_positions().into_iter().min_by(|a, b| a.distance_squared(pos).total_cmp(&b.distance_squared(pos)));
            if let Some(p) = near {
                let d = p - pos;
                yaw = d.z.atan2(d.x);
            }
        }
        let id = self.entity_id();
        self.level.mobs.push(Mob::new(kind, pos, yaw, id));
    }

    /// Minecraft-like spawning: now and then a group of one kind appears 24-64 blocks from a
    /// player and out of their sight, while fewer than `MOB_CAP` are around: a kind that may
    /// appear in that biome (by the weights of their `Spawn`), on its ground under the open
    /// sky, where it is light enough.
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
            .filter(|m| m.def().spawn.is_some() && Vec2::new(m.pos.x - me.x, m.pos.z - me.z).length() < 96.0)
            .count();
        if near >= MOB_CAP {
            return;
        }
        let ang = self.random() * TAU;
        let dist = 24.0 + self.random() * 40.0;
        let off = Vec3::new(ang.cos(), 0.0, ang.sin()) * dist;
        // Not right in front of them.
        if off.normalize().dot(crate::entity::player::look_dir(pose.yaw, 0.0)) > 0.3 {
            return;
        }
        let c = me + off;
        let biome = self.gen.column(c.x.floor() as i32, c.z.floor() as i32).biome;
        let r = self.random();
        let Some(def) = pick_spawn(biome, r) else { return };
        let Some(rule) = def.spawn else { return };
        let (least, most) = rule.group;
        let group = least as usize + (self.random() * (most - least + 1) as f32) as usize;
        let mut spots = Vec::new();
        for k in 0..group * 3 {
            if spots.len() >= group {
                break;
            }
            let jitter = if k == 0 { Vec3::ZERO } else { Vec3::new(self.random() - 0.5, 0.0, self.random() - 0.5) * 6.0 };
            let at = c + jitter;
            let (x, z) = (at.x.floor() as i32, at.z.floor() as i32);
            let w = &self.world;
            let Some(top) = w.height_at(x, z) else { continue };
            if !rule.ground.contains(&w.get(x, top, z)) {
                continue;
            }
            if let Some((p, ground)) = standable(w, x, z, top + 1, 0) {
                let (sky, block) = w.light_estimate(p + Vec3::Y * 0.5);
                if rule.ground.contains(&ground) && sky.max(block) >= rule.min_light {
                    spots.push(p);
                }
            }
        }
        for p in spots {
            self.spawn_mob(def.kind, p);
        }
    }
}

/// The kind that appears in `biome`, picked by the weights with `r` (0..1): of those whose
/// `Spawn` lets them appear there.
fn pick_spawn(biome: crate::world::gen::Biome, r: f32) -> Option<&'static MobDef> {
    let here = || MOBS.iter().filter(move |d| d.spawn.is_some_and(|s| s.biomes.is_empty() || s.biomes.contains(&biome)));
    let total: u32 = here().map(|d| d.spawn.map_or(0, |s| s.weight)).sum();
    let mut left = r * total as f32;
    for d in here() {
        let w = d.spawn.map_or(0, |s| s.weight) as f32;
        if left < w {
            return Some(d);
        }
        left -= w;
    }
    here().last()
}

/// Player `who` (named `name`) attacked `foe` (named `foe_name`, a player): a mob hit that
/// fights back turns on them, and so do the wild ones of its kind near it; `who`'s pets go
/// for `foe` (not for a static mob, nor for another of their pets); and a player attacked is
/// defended by theirs.
fn provoke_all(mobs: &mut [Mob], foe: Foe, who: u8, name: &str, foe_name: Option<&str>) {
    let me = Foe::Player(who);
    let mut pets_may = true;
    if let Foe::Mob(id) = foe {
        if let Some(hit) = mobs.iter().find(|m| m.id == id) {
            let (kind, at, wild) = (hit.kind, hit.pos, hit.owner().is_none());
            let alert = kind.def().ai.alert;
            pets_may = hit.def().behavior != Behavior::Static && hit.owner() != Some(name);
            for m in mobs.iter_mut().filter(|m| m.kind == kind) {
                let pack = wild && m.owner().is_none() && m.pos.distance(at) < alert;
                if m.id == id || pack {
                    m.provoke(me, Some(name));
                }
            }
        }
    }
    for m in mobs.iter_mut().filter(|m| m.owner().is_some()) {
        let own = m.owner() == Some(name);
        if own && pets_may && Foe::Mob(m.id) != foe {
            m.provoke(foe, foe_name);
        }
        if foe_name.is_some() && m.owner() == foe_name {
            m.provoke(me, Some(name));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content::mobs::{pig::PIG, sheep::SHEEP, wolf::WOLF, TARGET_DUMMY};
    use crate::world::gen::Biome;

    fn wolf(id: u32, x: f32, owner: Option<&str>) -> Mob {
        let mut m = Mob::new(WOLF, Vec3::new(x, 64.0, 0.0), 0.0, id);
        if let Some(o) = owner {
            m.feed_bone(o, true);
            m.toggle_sit();
        }
        m
    }

    #[test]
    fn pets_leave_dummies_and_each_other_alone() {
        let mut mobs = vec![wolf(1, 0.0, Some("Alby")), wolf(2, 1.0, Some("Alby")), Mob::new(TARGET_DUMMY, Vec3::new(3.0, 64.0, 0.0), 0.0, 3)];
        provoke_all(&mut mobs, Foe::Mob(3), 0, "Alby", None);
        assert!(mobs.iter().all(|m| m.foe.is_none()), "went for the dummy");
        provoke_all(&mut mobs, Foe::Mob(2), 0, "Alby", None);
        assert!(mobs.iter().all(|m| m.foe.is_none()), "went for one of their own");
        // Someone else's pig: they go for it.
        mobs.push(Mob::new(PIG, Vec3::new(5.0, 64.0, 0.0), 0.0, 4));
        provoke_all(&mut mobs, Foe::Mob(4), 0, "Alby", None);
        assert_eq!(mobs[0].foe, Some(Foe::Mob(4)));
        assert_eq!(mobs[1].foe, Some(Foe::Mob(4)));
    }

    #[test]
    fn a_wild_pack_turns_on_whoever_hurts_one_of_them() {
        let mut mobs = vec![wolf(1, 0.0, None), wolf(2, 5.0, None), wolf(3, 40.0, None), wolf(4, 1.0, Some("Bob"))];
        provoke_all(&mut mobs, Foe::Mob(1), 7, "Alby", None);
        assert_eq!(mobs[0].foe, Some(Foe::Player(7)));
        assert_eq!(mobs[1].foe, Some(Foe::Player(7)));
        assert!(mobs[2].foe.is_none() && mobs[3].foe.is_none());
        // Attacking Bob: his wolf defends him.
        provoke_all(&mut mobs, Foe::Player(9), 7, "Alby", Some("Bob"));
        assert_eq!(mobs[3].foe, Some(Foe::Player(7)));
    }

    #[test]
    fn spawns_follow_the_biomes_and_weights() {
        let pick = |b, r| pick_spawn(b, r).map(|d| d.kind);
        // In a forest: wolves too (12 of 34).
        let forest: Vec<_> = (0..34).map(|i| pick(Biome::Forest, (i as f32 + 0.5) / 34.0).unwrap()).collect();
        assert_eq!(forest.iter().filter(|k| **k == WOLF).count(), 12);
        assert_eq!(forest.iter().filter(|k| **k == SHEEP).count(), 12);
        assert_eq!(forest.iter().filter(|k| **k == PIG).count(), 10);
        // On the plains: never.
        assert!((0..100).all(|i| pick(Biome::Plains, i as f32 / 100.0) != Some(WOLF)));
        assert!((0..100).all(|i| pick(Biome::Plains, i as f32 / 100.0) != Some(TARGET_DUMMY)));
    }
}
