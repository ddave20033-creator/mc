//! The player's health: damage (falls, lava, fire, cactus, suffocation, drowning, the void),
//! death, and hunger, thirst and effects.

use crate::client::{Game, MAX_AIR, MAX_HEALTH, Screen};
use crate::client::player::sleep;
use crate::entity::survival::Needs;
use crate::item::*;
use crate::lang::t;
use crate::ui::chat;
use crate::world::{AIR, Block, CACTUS, is_lava, is_opaque, is_water};
use glam::{IVec3, Vec3};

/// Health and what goes on with it: hurt, burning, out of breath, falling, hungry and
/// thirsty; death, and where the player comes back to life; sleeping.
pub(in crate::client) struct Vitals {
    pub(in crate::client) health: f32,
    /// The moment of invulnerability after a hit (seconds left).
    invuln: f32,
    /// The view's shake after a hit (seconds left).
    pub(in crate::client) hurt_time: f32,
    /// The hit that started the moment of invulnerability (`invuln`): a harder one in it
    /// still does the difference.
    last_hit: f32,
    /// Hunger, thirst and effects.
    pub(in crate::client) needs: Needs,
    /// The highest point since leaving the ground (falls hurt from there).
    pub(in crate::client) fall_peak: f32,
    /// Seconds left burning, and till its next hurt.
    pub(in crate::client) fire: f32,
    fire_tick: f32,
    /// Breath left underwater, in seconds, and till drowning hurts again.
    pub(in crate::client) air: f32,
    drown_tick: f32,
    /// What the death screen says.
    pub(in crate::client) death_message: String,
    /// The head of the bed this player last used: where they come back to life.
    pub(in crate::client) bed_spawn: Option<IVec3>,
    /// Lying in a bed.
    pub(in crate::client) sleep: Option<sleep::Sleep>,
}

impl Vitals {
    pub(in crate::client) fn new() -> Self {
        Self {
            health: MAX_HEALTH,
            invuln: 0.0,
            hurt_time: 0.0,
            last_hit: 0.0,
            needs: Needs::new(),
            fall_peak: 0.0,
            fire: 0.0,
            fire_tick: 0.0,
            air: MAX_AIR,
            drown_tick: 0.0,
            death_message: String::new(),
            bed_spawn: None,
            sleep: None,
        }
    }

    /// Back to life: full health, fed and watered, not burning nor hurt.
    pub(in crate::client) fn revive(&mut self) {
        self.health = MAX_HEALTH;
        self.needs = Needs::new();
        self.fire = 0.0;
        self.invuln = 0.0;
        self.hurt_time = 0.0;
    }

    /// A hit of `amount`. Just hurt (Minecraft's rule): only as much as it is harder than the
    /// hit before does anything, and the moment of invulnerability does not start again.
    /// Whether it hurt.
    fn hurt(&mut self, amount: f32) -> bool {
        if self.invuln > 0.0 {
            if amount <= self.last_hit {
                return false;
            }
            self.health -= amount - self.last_hit;
            self.last_hit = amount;
        } else {
            self.health -= amount;
            self.last_hit = amount;
            self.invuln = 0.5;
            self.hurt_time = 0.4;
        }
        self.needs.exhaust(crate::entity::survival::cost::HURT);
        true
    }

    /// The tick's part of the moment of invulnerability and of the view's shake.
    fn tick_hurt(&mut self, dt: f32) {
        self.invuln -= dt;
        self.hurt_time = (self.hurt_time - dt).max(0.0);
    }

    /// Falling: the highest point since leaving the ground is kept (`steady`: flying,
    /// swimming or in creative, nothing is counted); on landing (`landed`), the damage of the
    /// fall, if it was long enough.
    fn fall(&mut self, y: f32, steady: bool, on_ground: bool, landed: bool) -> Option<f32> {
        if steady {
            self.fall_peak = y;
            None
        } else if !on_ground {
            self.fall_peak = self.fall_peak.max(y);
            None
        } else {
            let dist = self.fall_peak - y;
            self.fall_peak = y;
            (landed && dist > 3.0).then(|| (dist - 3.0).ceil())
        }
    }

    /// The tick's part of burning: whether it hurts now (once a second).
    fn burn(&mut self, dt: f32) -> bool {
        if self.fire <= 0.0 {
            return false;
        }
        self.fire -= dt;
        self.fire_tick -= dt;
        if self.fire_tick <= 0.0 {
            self.fire_tick = 1.0;
            return true;
        }
        false
    }

    /// The tick's breath: it runs out with the head underwater (`underwater`), and comes back
    /// out of it. Whether drowning hurts now (every second once it is out).
    fn breathe(&mut self, dt: f32, underwater: bool) -> bool {
        if !underwater {
            self.air = (self.air + dt * 5.0).min(MAX_AIR);
            self.drown_tick = 0.0;
            return false;
        }
        self.air = (self.air - dt).max(0.0);
        if self.air > 0.0 {
            return false;
        }
        self.drown_tick -= dt;
        if self.drown_tick <= 0.0 {
            self.drown_tick = 1.0;
            return true;
        }
        false
    }
}

impl Game {
    pub(in crate::client) fn damage(&mut self, amount: f32, cause: &'static str) {
        if self.creative()
            || self.spectator()
            || self.screen == Screen::Dead || self.me.vitals.health <= 0.0
        {
            return;
        }
        // Blocking with a sword takes about half of blockable damage, like Minecraft 1.8:
        // (1 + amount) / 2. Falling, burning, drowning, suffocating and the void go through.
        let blockable = matches!(cause, "death.lava" | "death.cactus");
        let amount = if self.me.aim.blocking && blockable {
            (1.0 + amount) * 0.5
        } else {
            amount
        };
        if !self.me.vitals.hurt(amount) {
            return;
        }
        if self.me.vitals.health <= 0.0 {
            self.me.vitals.health = 0.0;
            self.die(cause);
        }
    }

    /// Armor takes its share of a hit (`kind` as in `net::hurt`) and wears; returns what
    /// gets through.
    pub(in crate::client) fn armor_hit(&mut self, dmg: f32, kind: u8) -> f32 {
        use crate::net::hurt;
        if self.creative() || self.spectator() || self.me.items.inventory.armor.iter().all(|s| s.is_none())
        {
            return dmg;
        }
        let (bullet, blast) = (kind == hurt::BULLET, kind == hurt::BLAST);
        let k = armor_factor(&self.me.items.inventory.armor, bullet, blast);
        let wear = (dmg / 4.0).max(1.0) as u16;
        for (i, slot) in self.me.items.inventory.armor.iter_mut().enumerate() {
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

    pub(in crate::client) fn die(&mut self, cause: &'static str) {
        self.me.vitals.death_message = t(cause).to_string();
        let msg = self.me.vitals.death_message.clone();
        self.say(msg, chat::WHITE);
        // Everything you carry drops where you died (a crafting table keeps its own grid); a
        // magazine being put in goes back among it first.
        self.cancel_reload();
        self.inv_ui.drag = None;
        self.stash_table(true);
        let mut loose: Vec<Stack> = self.me.items.craft.iter_mut().filter_map(|s| s.take()).collect();
        loose.extend(self.me.items.cursor.take());
        if !self.creative() {
            loose.extend(self.me.items.inventory.armor.iter_mut().filter_map(|s| s.take()));
        }
        loose.extend(self.me.items.craft_out.take());
        self.me.items.craft_fx = None;
        if !self.creative() {
            let center = self.me.body.pos + Vec3::Y * 0.8;
            let mut stacks: Vec<Stack> = self
                .me.items.inventory
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
        self.me.vitals.sleep = None;
        self.set_grab(false);
        self.me.vitals.fire = 0.0;
        self.me.body.vel = Vec3::ZERO;
    }

    /// Blocks overlapping the player's bounding box (optionally grown a bit).
    fn touching(&self, grow: f32, pred: impl Fn(Block) -> bool) -> bool {
        let p = self.me.body.pos;
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

    pub(in crate::client) fn update_health(&mut self, dt: f32, was_on_ground: bool) {
        self.me.vitals.tick_hurt(dt);
        let creative = self.creative();
        let fluid = self.me.body.fluid(&self.terrain.world);

        // Fall damage: from the highest point since leaving the ground.
        let y = self.me.body.pos.y;
        let steady = self.me.body.flying || fluid != AIR || creative;
        let landed = !was_on_ground;
        if let Some(hurt) = self.me.vitals.fall(y, steady, self.me.body.on_ground, landed) {
            self.damage(hurt, "death.fall");
        }

        if self.touching(0.0, is_lava) {
            self.damage(4.0, "death.lava");
            if !creative {
                self.me.vitals.fire = 8.0;
            }
        }
        if self.touching(0.0, is_water) {
            self.me.vitals.fire = 0.0;
        }
        if self.me.vitals.burn(dt) {
            self.damage(1.0, "death.fire");
        }
        if self.touching(0.05, |b| b == CACTUS) {
            self.damage(1.0, "death.cactus");
        }
        let eye = self.sleep_eye().unwrap_or(self.me.body.eye());
        let eye_block = self.terrain.world.get(
            eye.x.floor() as i32,
            eye.y.floor() as i32,
            eye.z.floor() as i32,
        );
        if is_opaque(eye_block) {
            self.damage(1.0, "death.suffocate");
        }
        // Breath runs out with the head underwater; after that drowning hurts every second.
        if self.me.vitals.breathe(dt, !creative && is_water(eye_block)) {
            self.damage(2.0, "death.drown");
        }
        if y < -64.0 {
            self.damage(4.0, "death.void");
            if creative && y < -200.0 {
                self.spawn_player();
            }
        }

        // Hunger, thirst and effects: natural regeneration comes from being well fed.
        if creative {
            self.me.vitals.needs.poison = 0.0;
            self.me.vitals.needs.nausea = 0.0;
            return;
        }
        let fx = self.me.vitals.needs.update(dt, self.me.vitals.health, MAX_HEALTH);
        self.me.vitals.health = (self.me.vitals.health + fx.heal).min(MAX_HEALTH);
        for (amount, cause, can_kill) in fx.damage {
            if can_kill || self.me.vitals.health > amount {
                // Starving and poison hurt through the damage cooldown, like Minecraft.
                self.me.vitals.invuln = 0.0;
                self.damage(amount, cause);
            }
        }
    }
}
