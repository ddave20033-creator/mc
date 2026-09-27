//! The player's health: damage (falls, lava, fire, cactus, suffocation, drowning, the void),
//! death, and hunger, thirst and effects.

use super::*;
use crate::item::*;
impl Game {
    pub(super) fn damage(&mut self, amount: f32, cause: &'static str) {
        if self.creative() || self.invuln > 0.0 || self.screen == Screen::Dead || self.health <= 0.0
        {
            return;
        }
        // Blocking with a sword takes about half of blockable damage, like Minecraft 1.8:
        // (1 + amount) / 2. Falling, burning, drowning, suffocating and the void go through.
        let blockable = matches!(cause, "death.lava" | "death.cactus");
        let amount = if self.blocking && blockable {
            (1.0 + amount) * 0.5
        } else {
            amount
        };
        self.health -= amount;
        self.invuln = 0.5;
        self.hurt_time = 0.4;
        self.last_damage = self.time;
        self.needs.exhaust(crate::entity::survival::cost::HURT);
        if self.health <= 0.0 {
            self.health = 0.0;
            self.die(cause);
        }
    }

    /// Armor takes its share of a hit (`kind` as in `net::hurt`) and wears; returns what
    /// gets through.
    pub(super) fn armor_hit(&mut self, dmg: f32, kind: u8) -> f32 {
        use crate::net::hurt;
        if self.creative() || self.inventory.armor.iter().all(|s| s.is_none()) {
            return dmg;
        }
        let (bullet, blast) = (kind == hurt::BULLET, kind == hurt::BLAST);
        let k = armor_factor(&self.inventory.armor, bullet, blast);
        let wear = (dmg / 4.0).max(1.0) as u16;
        for (i, slot) in self.inventory.armor.iter_mut().enumerate() {
            let Some(s) = slot else { continue };
            let w = match i {
                VEST_SLOT if bullet || blast => (dmg / 2.0).max(1.0) as u16,
                VEST_SLOT => 0,
                _ => wear,
            };
            s.damage = s.damage.saturating_add(w);
            if s.damage >= armor_durability(s.item) {
                *slot = None;
            }
        }
        if !blast {
            self.audio.play(crate::audio::Sound::ArmorHit, None, 0.8);
        }
        dmg * k
    }

    pub(super) fn die(&mut self, cause: &'static str) {
        self.death_message = t(cause).to_string();
        let msg = self.death_message.clone();
        self.say(msg, chat::WHITE);
        // Everything you carry drops where you died (a crafting table keeps its own grid).
        self.drag = None;
        self.stash_table(true);
        let mut loose: Vec<Stack> = self.craft.iter_mut().filter_map(|s| s.take()).collect();
        loose.extend(self.cursor.take());
        if !self.creative() {
            loose.extend(self.inventory.armor.iter_mut().filter_map(|s| s.take()));
        }
        loose.extend(self.craft_out.take());
        self.craft_fx = None;
        loose.extend(self.guns.bench.items());
        self.guns.bench.clear();
        if !self.creative() {
            let center = self.player.pos + Vec3::Y * 0.8;
            let mut stacks: Vec<Stack> = self
                .inventory
                .slots
                .iter_mut()
                .filter_map(|s| s.take())
                .collect();
            stacks.extend(loose);
            for s in stacks {
                self.spawn_drop(center, s);
            }
        } else {
            // Creative keeps the inventory; the 2x2 grid and the cursor go back into it.
            for s in loose {
                self.give(s);
            }
        }
        self.screen = Screen::Dead;
        self.sleep = None;
        self.set_grab(false);
        self.fire = 0.0;
        self.player.vel = Vec3::ZERO;
    }

    /// Blocks overlapping the player's bounding box (optionally grown a bit).
    pub(super) fn touching(&self, grow: f32, pred: impl Fn(u8) -> bool) -> bool {
        let p = self.player.pos;
        let min = p - Vec3::new(0.3 + grow, 0.0, 0.3 + grow);
        let max = p + Vec3::new(0.3 + grow, 1.8, 0.3 + grow);
        for x in min.x.floor() as i32..=max.x.floor() as i32 {
            for y in min.y.floor() as i32..=max.y.floor() as i32 {
                for z in min.z.floor() as i32..=max.z.floor() as i32 {
                    if pred(self.terrain.world.get(x, y, z)) {
                        return true;
                    }
                }
            }
        }
        false
    }

    pub(super) fn update_health(&mut self, dt: f32, was_on_ground: bool) {
        self.invuln -= dt;
        self.hurt_time = (self.hurt_time - dt).max(0.0);
        let creative = self.creative();
        let fluid = self.player.fluid(&self.terrain.world);

        // Fall damage: track the highest point since leaving the ground.
        let y = self.player.pos.y;
        if self.player.flying || fluid != AIR || creative {
            self.fall_peak = y;
        } else if !self.player.on_ground {
            self.fall_peak = self.fall_peak.max(y);
        } else {
            if !was_on_ground {
                let dist = self.fall_peak - y;
                if dist > 3.0 {
                    self.damage((dist - 3.0).ceil(), "death.fall");
                }
            }
            self.fall_peak = y;
        }

        if self.touching(0.0, is_lava) {
            self.damage(4.0, "death.lava");
            if !creative {
                self.fire = 8.0;
            }
        }
        if self.touching(0.0, is_water) {
            self.fire = 0.0;
        }
        if self.fire > 0.0 {
            self.fire -= dt;
            self.fire_tick -= dt;
            if self.fire_tick <= 0.0 {
                self.fire_tick = 1.0;
                self.damage(1.0, "death.fire");
            }
        }
        if self.touching(0.05, |b| b == CACTUS) {
            self.damage(1.0, "death.cactus");
        }
        let eye = self.sleep_eye().unwrap_or(self.player.eye());
        let eye_block = self.terrain.world.get(
            eye.x.floor() as i32,
            eye.y.floor() as i32,
            eye.z.floor() as i32,
        );
        if is_opaque(eye_block) {
            self.damage(1.0, "death.suffocate");
        }
        // Breath runs out with the head underwater; after that drowning hurts every second.
        if !creative && is_water(eye_block) {
            self.air = (self.air - dt).max(0.0);
            if self.air <= 0.0 {
                self.drown_tick -= dt;
                if self.drown_tick <= 0.0 {
                    self.drown_tick = 1.0;
                    self.damage(2.0, "death.drown");
                }
            }
        } else {
            self.air = (self.air + dt * 5.0).min(MAX_AIR);
            self.drown_tick = 0.0;
        }
        if y < -64.0 {
            self.damage(4.0, "death.void");
            if creative && y < -200.0 {
                self.spawn_player();
            }
        }

        // Hunger, thirst and effects: natural regeneration comes from being well fed.
        if creative {
            self.needs.poison = 0.0;
            self.needs.nausea = 0.0;
            return;
        }
        let fx = self.needs.update(dt, self.health, MAX_HEALTH);
        self.health = (self.health + fx.heal).min(MAX_HEALTH);
        for (amount, cause, can_kill) in fx.damage {
            if can_kill || self.health > amount {
                // Starving and poison hurt through the damage cooldown, like Minecraft.
                self.invuln = 0.0;
                self.damage(amount, cause);
            }
        }
    }
}
