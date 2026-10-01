//! The player and what is seen of the world moving on: the player in ticks (`tick_player`:
//! movement, health, hunger and thirst) and in frames (`update_player`: looking, aiming, what
//! the hands do), and the world in frames (`update_world`: chest lids, shots, grenades,
//! furnaces, particles and sounds, and the things the server sent gliding on between its
//! updates). The world itself runs on the server (`sim::server`).

use crate::client::{Container, Game, Screen};
use crate::entity::player::MoveInput;
use crate::item::*;
use crate::keys::Bind;
use crate::sim::clock::TICK_SECS;
use crate::world::{AIR, TORCH, is_torch};
use glam::Vec3;

impl Game {
    /// The player's tick: moving (with the keys held, `control`: no screen open), health,
    /// hunger and thirst.
    pub(super) fn tick_player(&mut self, control: bool) {
        let dt = TICK_SECS;
        self.me.body.start_tick();
        if self.me.vitals.sleep.is_some() {
            self.update_sleep(dt, control);
            return;
        }
        if self.spectator() {
            self.tick_spectator(control);
            return;
        }
        self.me.body.noclip = false;
        if !self.creative() {
            self.me.body.flying = false;
        }
        // Aiming a gun (right button held): no sprinting, and the double-tap sprint ends.
        let aiming = control && self.input.right_down && self.holding_gun();
        let firing = self.tools.guns.no_sprint > 0.0;
        if aiming {
            self.input.w_sprint = false;
        }
        let k = |b: Bind| control && self.bind_down(b);
        let axis = |a: bool, b: bool| (a as i32 - b as i32) as f32;
        // With a fishing rod in hand the sneak key shifts the reel's gear instead.
        let rod = self.held() == FISHING_ROD;
        let input = MoveInput {
            forward: axis(k(Bind::Forward), k(Bind::Back)),
            strafe: axis(k(Bind::Right), k(Bind::Left)),
            up: k(Bind::Jump),
            down: k(Bind::Sneak) && (!rod || self.me.body.flying),
            sprint: (k(Bind::Sprint) || (self.input.w_sprint && k(Bind::Forward)))
                && (self.creative() || self.me.vitals.needs.can_sprint())
                && !firing,
            sneak: k(Bind::Sneak) && !rod,
            using: self.me.aim.blocking || self.me.aim.using.is_some(),
            aiming,
        };
        let was_on_ground = self.me.body.on_ground;
        self.me.body.update(dt, &self.terrain.world, self.me.look.yaw, &input);
        self.update_health(dt, was_on_ground);
        if self.screen == Screen::Dead {
            return;
        }
        if !self.creative() {
            // Sprinting, swimming and jumping make you hungry and thirsty.
            use crate::entity::survival::cost;
            let moved = self.me.body.horizontal_speed() * dt;
            if self.me.body.sprinting {
                self.me.vitals.needs.exhaust(cost::SPRINT * moved);
            } else if self.me.body.fluid(&self.terrain.world) != AIR {
                self.me.vitals.needs.exhaust(cost::SWIM * moved);
            }
            if was_on_ground
                && !self.me.body.on_ground
                && self.me.body.vel.y > 0.0
                && !self.me.body.flying
            {
                let c = if self.me.body.sprinting {
                    cost::SPRINT_JUMP
                } else {
                    cost::JUMP
                };
                self.me.vitals.needs.exhaust(c);
            }
        }
    }

    /// The player's frame: looking about, aiming, and what is done with the hands (mining,
    /// hitting, using, shooting), and the body's animation. `control` is false while a screen
    /// is open.
    pub(super) fn update_player(&mut self, dt: f32, control: bool) {
        if control {
            // Slower turning while zoomed in.
            let zoom = (self.me.look.fov / self.settings.fov).min(1.0);
            let sens = 0.0022 * self.settings.sensitivity / 100.0 * zoom;
            self.me.look.turn(self.input.mouse_delta, sens);
            if self.input.scroll != 0.0 && !self.spectator() && !self.fishing_scroll() {
                let d = if self.input.scroll > 0.0 { -1 } else { 1 };
                self.me.items.hotbar_slot = (self.me.items.hotbar_slot as i32 + d).rem_euclid(9) as usize;
                self.hud.slot_name_timer = 2.0;
            }
        }
        self.me.hand.equip(self.held());
        let st = self.me.items.held_stack();
        self.me.hand.held_data = st.map_or(0, |s| s.data);
        self.me.hand.held_damage = st.map_or(0, |s| s.damage);
        if self.me.vitals.sleep.is_some() {
            return;
        }
        if self.spectator() {
            self.update_spectator(dt, control);
            return;
        }
        // Holding the right mouse button with a sword blocks (Minecraft 1.8).
        self.me.aim.blocking = control && self.input.right_down && is_sword(self.held());
        self.me.hand.blocking = self.me.aim.blocking;
        self.update_using(dt, control);
        if self.screen == Screen::Dead {
            // (a swing does not go on after death)
            self.me.aim.chop = None;
            self.me.aim.struck = None;
            self.me.hand.hidden = false;
            return;
        }
        let speed = self.me.body.horizontal_speed();

        self.update_aim(control);
        self.me.aim.action_cooldown -= dt;
        self.update_guns(dt, control);
        let book = self.book_in_hand();
        self.update_grenade_hold(dt, control && !book);
        self.update_fishing(dt, control);
        // A sword does not break blocks at all (it only fights); with a pistol the left mouse
        // button shoots instead (one shot per click, no hitting).
        let sword = is_sword(self.held());
        let gun = self.holding_gun();
        // Holding the guide book: the buttons turn its pages (they never hit blocks).
        let reading = control && self.book_in_hand();
        if reading {
            let (left, right) = (self.input.left_pressed, self.input.right_down);
            self.book_buttons(dt, left, right);
        }
        if sword || gun || reading {
            self.me.aim.mining = None;
        }
        // A left click on what is in a furnace takes it out instead of mining (a pistol shoots).
        let furnace_hold = control && !gun && !reading && self.furnace_left_click();
        // An axe on a tree's trunk chops it instead.
        let chopping = self.update_chopping(control && !reading && !sword && !gun && !furnace_hold, dt);
        let can_mine = control && !chopping && !reading && !sword && !gun && !furnace_hold;
        let breaking = self.update_mining(dt, can_mine);
        // Hitting: either block with the sword or strike, not both.
        if control
            && !reading
            && self.input.left_pressed
            && (self.me.aim.target.is_none() || sword)
            && !self.me.aim.blocking
            && !gun
        {
            if let Some(i) = self.target_mob() {
                self.attack(Some(i), None);
            }
            if let Some(id) = self.me.aim.player_target {
                self.attack(None, Some(id));
            }
            self.me.hand.swing();
        }
        // One shot per click; an automatic gun keeps firing while the button is held.
        let auto = GunKind::of(self.held()).is_some_and(|k| k.stats().auto);
        if control && gun && (self.input.left_pressed || (auto && self.input.left_down)) {
            self.shoot();
        }
        if let Some(p) = breaking {
            self.break_block(p);
        }
        if control
            && !reading
            && (self.input.right_pressed || (self.input.right_down && self.me.aim.action_cooldown <= 0.0))
        {
            self.use_item();
        }
        // Pick block (creative).
        if control && self.input.middle_pressed && self.creative() {
            if let Some((hit, _)) = self.me.aim.target {
                if let Some(item) = item_of_block(self.terrain.world.geti(hit)) {
                    if let Some(i) = self.me.items.inventory.slots[..9]
                        .iter()
                        .position(|s| s.is_some_and(|s| s.item == item))
                    {
                        self.me.items.hotbar_slot = i;
                    } else {
                        *self.me.items.held_slot_mut() =
                            Some(Stack::new(item, max_stack(item)));
                    }
                    self.hud.slot_name_timer = 2.0;
                }
            }
        }

        let gun = self.holding_gun();
        let chopping = self.me.aim.chop.is_some();
        self.me.look.follow_body(dt, speed, gun, chopping);
        self.me.look.swing_limbs(dt, speed, self.me.body.flying);
    }

    /// The world's frame: chest lids, shots and grenades flying, furnace glow and sounds, torch
    /// fire, and this game's copy of the server's world following it.
    pub(super) fn update_world(&mut self, dt: f32) {
        // Chest lids: open chests (this player's and the other LAN players') swing up, the
        // others fall shut.
        let mut open_chests = self.remote_open_chests();
        if let Screen::Container(Container::Chest(p)) = self.screen {
            open_chests.push(p);
        }
        self.level.swing_chest_lids(&open_chests, dt);

        // Shots and thrown grenades go on whatever the player is doing (dead, asleep).
        self.update_shots(dt);
        self.update_grenades(dt);

        self.furnace_fx(dt);
        let mut loops = self.furnace_sounds();
        loops.extend(self.grenade_sounds());
        loops.extend(self.fishing_sounds());
        self.audio.set_loops(&loops);
        // The world is run by the server: only follow what it sends.
        self.client_world(dt);
        if self.gfx.torch_particles {
            self.torch_fire(dt);
        }
    }

    /// Resource-pack torch fire: flame and smoke particles rising from the tips of the torches
    /// around the player, like Minecraft's torches.
    fn torch_fire(&mut self, dt: f32) {
        self.level.torch_scan -= dt;
        if self.level.torch_scan <= 0.0 {
            self.level.torch_scan = 1.0;
            let c = self.me.body.pos.floor().as_ivec3();
            let w = &self.terrain.world;
            self.level.torches.clear();
            self.level.torches.extend(crate::world::terrain::listed_near(&self.terrain.torches, c, 20, 12).filter(|&p| is_torch(w.geti(p))));
        }
        // Torches in hands burn too: this player's (where it was drawn) and the others'
        // (about where they hold it up).
        let mut tips: Vec<Vec3> = self.me.look.held_torch_tip.into_iter().collect();
        tips.extend(
            self.remote_held_lights()
                .into_iter()
                .filter(|&(_, _, item)| item == TORCH as ItemId)
                .map(|(_, light, _)| light + Vec3::Y * 0.3),
        );
        for tip in tips {
            self.burn_torch(tip, dt);
        }
        for i in 0..self.level.torches.len() {
            let p = self.level.torches[i];
            let b = self.terrain.world.geti(p);
            if !is_torch(b) {
                continue; // broken since the last scan
            }
            let base = p.as_vec3() + Vec3::new(0.5, 0.0, 0.5);
            let tip = crate::world::mesh::torch_transform(base, b)
                .transform_point3(Vec3::new(0.0, 0.21, 0.0));
            self.burn_torch(tip, dt);
        }
    }

    /// A torch's flame flickering up at its tip, and its smoke now and then.
    fn burn_torch(&mut self, tip: Vec3, dt: f32) {
        if self.random() < dt * 3.0 {
            self.level.particles.flame(tip);
        }
        if self.random() < dt * 1.0 {
            let w = &self.terrain.world;
            let (sky, blk) = (w.sky_estimate(tip), w.block_light_estimate(tip));
            self.level.particles.smoke(tip + Vec3::Y * 0.08, sky, blk);
        }
    }
}
