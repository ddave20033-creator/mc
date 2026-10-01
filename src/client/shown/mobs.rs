//! Mobs from the game's side: attacking mobs and other players, items used on mobs, spawn
//! eggs. (The server runs them; what a kind does is in its file, `content::mobs`.)

use crate::client::*;
use crate::item::inventory::{self, take};
use crate::item::*;

impl Game {
    /// Left click on a mob (index into `mobs`) or another LAN player: damage by the held item
    /// (x1.5 for a critical hit while falling), knockback (more while sprinting), and tool
    /// wear like Minecraft (swords 1, tools 2).
    pub(in crate::client) fn attack(&mut self, mob: Option<usize>, player: Option<u8>) {
        // Sneaking, a hit takes a static mob (a target dummy) down (it drops as its item).
        let fixed = |m: &Mob| m.def().behavior == crate::content::mobs::Behavior::Static;
        if let Some(i) = mob.filter(|&i| fixed(&self.level.mobs[i]) && self.me.body.sneaking) {
            let id = self.level.mobs[i].id;
            self.send(crate::net::Msg::TakeDown { id });
            return;
        }
        let held = self.held();
        let mut dmg = attack_damage(held);
        let falling = !self.me.body.on_ground && !self.me.body.flying && self.me.body.vel.y < 0.0;
        if falling && self.me.body.fluid(&self.terrain.world) == AIR {
            dmg *= 1.5;
        }
        let knock = if self.me.body.sprinting { 2.0 } else { 1.0 };
        let from = self.me.body.pos;
        if !self.creative() {
            self.me.vitals.needs.exhaust(crate::entity::survival::cost::ATTACK);
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
                let slot = self.me.items.hotbar_slot;
                if inventory::damage(&mut self.me.items.inventory.slots[slot], wear) {
                    let p = hit_at.floor().as_ivec3();
                    self.level.particles
                        .burst(&self.terrain.world, p, STONE, 12, [255; 3]);
                }
            }
        }
        if self.me.body.sprinting {
            // Hitting while sprinting stops the sprint (Minecraft).
            self.me.body.sprinting = false;
            self.input.w_sprint = false;
        }
    }

    /// Right click on a mob holding an item: what its kind does with it (`Hooks::use_on`:
    /// shears on a sheep, a bone for a wild wolf, sitting down one's own...); the server
    /// decides. True if something happened.
    pub(in crate::client) fn use_on_mob(&mut self, i: usize) -> bool {
        let held = self.held();
        let fresh = self.input.right_pressed;
        let m = &mut self.level.mobs[i];
        let Some(use_on) = m.def().hooks.use_on else {
            return false;
        };
        let Some(used) = use_on(m, held, fresh) else {
            return false;
        };
        let id = m.id;
        self.send(crate::net::Msg::UseOnMob { id, item: held });
        if !self.creative() {
            let slot = self.me.items.held_slot_mut();
            if used.consume {
                take(slot, 1);
            }
            if used.wear > 0 {
                inventory::damage(slot, used.wear);
            }
        }
        self.me.hand.swing();
        self.me.aim.action_cooldown = 0.25;
        true
    }

    /// Spawn egg on a block: the mob appears on the clicked face.
    pub(in crate::client) fn use_spawn_egg(&mut self, kind: MobKind) {
        let Some((hit, prev)) = self.me.aim.target else {
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
            take(self.me.items.held_slot_mut(), 1);
        }
        self.me.hand.swing();
        self.me.aim.action_cooldown = 0.25;
    }

    pub(in crate::client) fn spawn_mob(&mut self, kind: MobKind, pos: Vec3) {
        self.send(crate::net::Msg::SpawnMob {
            kind: kind.0,
            pos,
        });
    }

}
