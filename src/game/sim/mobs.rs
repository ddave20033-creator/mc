//! Mobs from the game's side: attacking mobs and other players, spawn eggs, natural
//! spawning, wolves (taming, their bites, going for whoever their owner attacks), and
//! updating them (with their loot and sounds).

use crate::game::*;
use crate::item::inventory::{self, take};
use crate::item::*;

impl Game {
    /// Left click on a mob (index into `mobs`) or another LAN player: damage by the held item
    /// (x1.5 for a critical hit while falling), knockback (more while sprinting), and tool
    /// wear like Minecraft (swords 1, tools 2).
    pub(in crate::game) fn attack(&mut self, mob: Option<usize>, player: Option<u8>) {
        // Sneaking, a hit takes a target dummy down (it drops as an item).
        if let Some(i) = mob.filter(|&i| self.level.mobs[i].kind == MobKind::Dummy && self.player.sneaking) {
            let id = self.level.mobs[i].id;
            self.send(crate::net::Msg::BreakDummy { id });
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
            (Some(i), _) => {
                // The server has the real mob; it can be hit when it is not still flashing red.
                let m = &self.level.mobs[i];
                let can = m.alive() && m.hurt_time <= 0.0;
                let id = m.id;
                self.send(Msg::AttackMob { id, dmg, knock });
                can
            }
            (None, Some(id)) => {
                let kind = crate::net::hurt::MELEE;
                self.send(Msg::AttackPlayer { id, dmg, knock, kind });
                true
            }
            _ => false,
        };
        let hit_at = match (mob, player) {
            (Some(i), _) => self.level.mobs[i].center(),
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

    /// Right click on a wolf: a wild one is given a bone (it may take to this player), and a
    /// tame one of theirs sits down or stands up. True if something happened.
    pub(in crate::game) fn use_on_wolf(&mut self, i: usize) -> bool {
        let held = self.held();
        let m = &self.level.mobs[i];
        if m.kind != MobKind::Wolf || !m.alive() {
            return false;
        }
        let wild = !m.tame();
        let bone = held == BONE && wild && m.foe.is_none();
        if !bone && !m.yours {
            return false;
        }
        let id = m.id;
        self.send(crate::net::Msg::UseOnMob { id, item: held });
        if !bone {
            self.level.mobs[i].toggle_sit();
        }
        if bone && !self.creative() {
            take(&mut self.inventory.slots[self.hotbar_slot], 1);
        }
        self.hand.swing();
        self.action_cooldown = 0.25;
        true
    }

    /// Spawn egg on a block: the mob appears on the clicked face.
    pub(in crate::game) fn use_spawn_egg(&mut self, kind: MobKind) {
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
    pub(in crate::game) fn shear(&mut self, i: usize) {
        if !self.level.mobs[i].can_shear() {
            return;
        }
        let id = self.level.mobs[i].id;
        self.level.mobs[i].sheared = true;
        self.send(crate::net::Msg::Shear { id });
        if !self.creative() {
            let slot = self.hotbar_slot;
            inventory::damage(&mut self.inventory.slots[slot], 1);
        }
        self.hand.swing();
        self.action_cooldown = 0.25;
    }

    pub(in crate::game) fn spawn_mob(&mut self, kind: MobKind, pos: Vec3) {
        self.send(crate::net::Msg::SpawnMob {
            kind: kind as u8,
            pos,
        });
    }

}
