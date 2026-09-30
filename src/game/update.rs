//! The player and the world moving on: in ticks (`tick_player`, `tick_world`: movement,
//! health, fluids, furnaces, saplings, dropped items, mobs, falling blocks, time of day and
//! autosave), and in frames (`update_player`, `update_world`: looking, aiming, what the hands
//! do, chest lids, shots, particles and sounds).

use super::*;
use crate::entity::player::{raycast, MoveInput};
use crate::item::*;
impl Game {
    /// The player's tick: moving (with the keys held, `control`: no screen open), health,
    /// hunger and thirst.
    pub(super) fn tick_player(&mut self, control: bool) {
        let dt = TICK_SECS;
        self.player.start_tick();
        if self.sleep.is_some() {
            self.update_sleep(dt, control);
            return;
        }
        if self.spectator() {
            self.tick_spectator(control);
            return;
        }
        self.player.noclip = false;
        if !self.creative() {
            self.player.flying = false;
        }
        // Aiming a gun (right button held): no sprinting, and the double-tap sprint ends.
        let aiming = control && self.input.right_down && self.holding_gun();
        let firing = self.guns.no_sprint > 0.0;
        if aiming {
            self.w_sprint = false;
        }
        let k = |b: Bind| control && self.bind_down(b);
        let axis = |a: bool, b: bool| (a as i32 - b as i32) as f32;
        // With a fishing rod in hand the sneak key shifts the reel's gear instead.
        let rod = self.held() == FISHING_ROD;
        let input = MoveInput {
            forward: axis(k(Bind::Forward), k(Bind::Back)),
            strafe: axis(k(Bind::Right), k(Bind::Left)),
            up: k(Bind::Jump),
            down: k(Bind::Sneak) && (!rod || self.player.flying),
            sprint: (k(Bind::Sprint) || (self.w_sprint && k(Bind::Forward)))
                && (self.creative() || self.needs.can_sprint())
                && !firing,
            sneak: k(Bind::Sneak) && !rod,
            using: self.blocking || self.using.is_some(),
            aiming,
        };
        let was_on_ground = self.player.on_ground;
        self.player.update(dt, &self.terrain.world, self.yaw, &input);
        self.update_health(dt, was_on_ground);
        if self.screen == Screen::Dead {
            return;
        }
        if !self.creative() {
            // Sprinting, swimming and jumping make you hungry and thirsty.
            use crate::entity::survival::cost;
            let moved = self.player.horizontal_speed() * dt;
            if self.player.sprinting {
                self.needs.exhaust(cost::SPRINT * moved);
            } else if self.player.fluid(&self.terrain.world) != AIR {
                self.needs.exhaust(cost::SWIM * moved);
            }
            if was_on_ground
                && !self.player.on_ground
                && self.player.vel.y > 0.0
                && !self.player.flying
            {
                let c = if self.player.sprinting {
                    cost::SPRINT_JUMP
                } else {
                    cost::JUMP
                };
                self.needs.exhaust(c);
            }
        }
    }

    /// The player's frame: looking about, aiming, and what is done with the hands (mining,
    /// hitting, using, shooting), and the body's animation. `control` is false while a screen
    /// is open.
    pub(super) fn update_player(&mut self, dt: f32, control: bool) {
        if control {
            // Slower turning while zoomed in.
            let zoom = (self.fov_current / self.settings.fov).min(1.0);
            let sens = 0.0022 * self.settings.sensitivity / 100.0 * zoom;
            self.yaw += self.input.mouse_delta.x * sens;
            self.pitch = (self.pitch - self.input.mouse_delta.y * sens).clamp(-1.55, 1.55);
            if self.input.scroll != 0.0 && !self.spectator() && !self.fishing_scroll() {
                let d = if self.input.scroll > 0.0 { -1 } else { 1 };
                self.hotbar_slot = (self.hotbar_slot as i32 + d).rem_euclid(9) as usize;
                self.slot_name_timer = 2.0;
            }
        }
        self.hand.equip(self.held());
        let st = self.inventory.slots[self.hotbar_slot];
        self.hand.held_data = st.map_or(0, |s| s.data);
        self.hand.held_damage = st.map_or(0, |s| s.damage);
        if self.sleep.is_some() {
            return;
        }
        if self.spectator() {
            self.update_spectator(dt, control);
            return;
        }
        // Holding the right mouse button with a sword blocks (Minecraft 1.8).
        self.blocking = control && self.input.right_down && is_sword(self.held());
        self.hand.blocking = self.blocking;
        self.update_using(dt, control);
        if self.screen == Screen::Dead {
            return;
        }
        let speed = self.player.horizontal_speed();

        // Targeting, mining and placing (from the eye where it is drawn: what is aimed at is
        // what is seen)
        let eye = self.eye();
        self.target = if control {
            let dir = look_dir(self.yaw, self.pitch);
            match self.camera.shoulder_camera(&self.terrain.world, eye, dir) {
                Some(cam) => {
                    super::camera::shoulder_target(&self.terrain.world, eye, dir, cam, 5.0)
                }
                None => raycast(&self.terrain.world, eye, dir, 5.0),
            }
        } else {
            None
        };
        if let Some((hit, _)) = self.target {
            let dir = look_dir(self.yaw, self.pitch);
            self.target_point = crate::entity::player::ray_boxes(&self.terrain.world, eye, dir, hit, 6.0)
                .map(|(t, _)| eye + dir * t)
                .unwrap_or(hit.as_vec3() + Vec3::splat(0.5));
        }
        self.aim_furnace();
        // A mob in front of the targeted block takes the crosshair (entity reach: 3 blocks).
        self.mob_target = None;
        self.player_target = None;
        if control {
            let dir = look_dir(self.yaw, self.pitch);
            let block_dist = self
                .target
                .and_then(|(hit, _)| {
                    let min = hit.as_vec3();
                    crate::util::ray_box(eye, dir, min, min + Vec3::ONE, 5.0)
                })
                .unwrap_or(f32::INFINITY);
            let reach = if self.creative() { 5.0 } else { 3.0 };
            let mob = self
                .level.mobs
                .iter()
                .enumerate()
                .filter_map(|(i, m)| m.ray_hit(eye, dir, reach).map(|d| (i, d)))
                .filter(|&(_, d)| d < block_dist)
                .min_by(|a, b| a.1.total_cmp(&b.1));
            // Another LAN player in front of the mob and the block takes it instead.
            let player = self
                .pick_player(eye, dir, reach)
                .filter(|&(_, d)| d < block_dist && mob.is_none_or(|(_, md)| d < md));
            self.player_target = player.map(|(id, _)| id);
            self.mob_target = mob.filter(|_| player.is_none()).map(|(i, _)| self.level.mobs[i].id);
            if self.mob_target.is_some() || self.player_target.is_some() {
                self.target = None;
            }
        }
        // A felled trunk lying there, aimed at with an axe.
        self.aim_lying_logs(control && self.mob_target.is_none() && self.player_target.is_none());
        self.action_cooldown -= dt;
        self.update_guns(dt, control);
        let book = self.book_in_hand();
        self.update_grenade_hold(dt, control && !book);
        self.update_fishing(dt, control);
        let mut breaking = None;
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
            self.mining = None;
        }
        // A left click on what is in a furnace takes it out instead of mining (a pistol shoots).
        let furnace_hold = control && !gun && !reading && self.furnace_left_click();
        // An axe on a tree's trunk chops it instead.
        let chopping = self.update_chopping(control && !reading && !sword && !gun && !furnace_hold, dt);
        if control
            && !chopping
            && !reading
            && self.input.left_down
            && self.action_cooldown <= 0.0
            && !sword
            && !gun
            && !furnace_hold
        {
            if let Some((hit, _)) = self.target {
                let b = self.terrain.world.geti(hit);
                let time =
                    break_time(b, self.held()).map(|t| if self.creative() { 0.0 } else { t });
                if let Some(time) = time {
                    let progress = match self.mining {
                        Some((p, prog)) if p == hit => prog,
                        _ => {
                            self.hand.keep_swinging();
                            0.0
                        }
                    } + dt / time.max(1e-4);
                    self.mining = Some((hit, progress));
                    self.dig_timer -= dt;
                    if self.dig_timer <= 0.0 && time > 0.0 {
                        self.dig_timer = 0.24;
                        let tint = self.block_tint(hit, b);
                        self.particles.burst(&self.terrain.world, hit, b, 2, tint);
                    }
                    if progress >= 1.0 || time == 0.0 {
                        breaking = Some(hit);
                    }
                } else {
                    self.mining = None;
                }
            } else {
                self.mining = None;
            }
        } else if !self.input.left_down {
            self.mining = None;
            self.dig_timer = 0.0;
        }
        // Hitting: either block with the sword or strike, not both.
        if control
            && !reading
            && self.input.left_pressed
            && (self.target.is_none() || sword)
            && !self.blocking
            && !gun
        {
            if let Some(i) = self.target_mob() {
                self.attack(Some(i), None);
            }
            if let Some(id) = self.player_target {
                self.attack(None, Some(id));
            }
            self.hand.swing();
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
            && (self.input.right_pressed || (self.input.right_down && self.action_cooldown <= 0.0))
        {
            self.use_item();
        }
        // Pick block (creative).
        if control && self.input.middle_pressed && self.creative() {
            if let Some((hit, _)) = self.target {
                if let Some(item) = item_of_block(self.terrain.world.geti(hit)) {
                    if let Some(i) = self.inventory.slots[..9]
                        .iter()
                        .position(|s| s.is_some_and(|s| s.item == item))
                    {
                        self.hotbar_slot = i;
                    } else {
                        self.inventory.slots[self.hotbar_slot] =
                            Some(Stack::new(item, max_stack(item)));
                    }
                    self.slot_name_timer = 2.0;
                }
            }
        }

        // Body follows the head when moving, otherwise lags within 50 degrees (like Minecraft);
        // with a gun in hand it turns with the head, all of it at once.
        let diff = crate::util::wrap_angle(self.yaw - self.body_yaw);
        let lag = 50f32.to_radians();
        // (The shoulder views, 3 and 4, orbit the body while stationary.)
        let orbiting = matches!(self.camera.mode, 3 | 4);
        if self.holding_gun() {
            // Turned with the look at once, all of it (the torso never twisted off the legs).
            self.body_yaw = self.yaw;
        } else if speed > 0.1 {
            self.body_yaw += diff * (crate::util::damp(10.0, dt));
        } else if !orbiting && diff.abs() > lag {
            // In regular views the torso follows the head so a large turn still looks natural.
            let excess = diff - diff.signum() * lag;
            self.body_yaw += excess * (crate::util::damp(12.0, dt));
        }
        // Limb swing follows horizontal movement (in the air too, like Minecraft).
        let fly = if self.player.flying { 0.3 } else { 1.0 };
        let target = (speed / 4.3).min(1.0) * fly;
        self.limb_amount += (target - self.limb_amount) * (crate::util::damp(10.0, dt));
    }

    /// The world's frame: chest lids, shots and grenades flying, furnace glow and sounds, torch
    /// fire (and a LAN player's copy of the host's world following it).
    pub(super) fn update_world(&mut self, dt: f32) {
        // Chest lids: open chests (this player's and the other LAN players') swing up, the
        // others fall shut.
        let mut open_chests = self.remote_open_chests();
        if let Screen::Container(Container::Chest(p)) = self.screen {
            open_chests.push(p);
        }
        for p in &open_chests {
            self.level.chest_open.entry(*p).or_insert(0.0);
        }
        self.level.chest_open.retain(|p, k| {
            let target = if open_chests.contains(p) { 1.0 } else { 0.0 };
            *k = if target > *k {
                (*k + dt * 3.5).min(1.0)
            } else {
                (*k - dt * 3.0).max(0.0)
            };
            *k > 0.0 || target > 0.0
        });

        // Shots and thrown grenades go on whatever the player is doing (dead, asleep).
        self.update_shots(dt);
        self.update_grenades(dt);

        self.furnace_fx(dt);
        let mut loops = self.furnace_sounds();
        loops.extend(self.grenade_sounds());
        loops.extend(self.fishing_sounds());
        self.audio.set_loops(&loops);
        // A LAN player's world is run by the host: only follow what it sends.
        self.client_world(dt);
        if self.torch_particles {
            self.torch_fire(dt);
        }
    }

    /// Resource-pack torch fire: flame and smoke particles rising from the tips of the torches
    /// around the player, like Minecraft's torches.
    pub(super) fn torch_fire(&mut self, dt: f32) {
        self.level.torch_scan -= dt;
        if self.level.torch_scan <= 0.0 {
            self.level.torch_scan = 1.0;
            let c = self.player.pos.floor().as_ivec3();
            let w = &self.terrain.world;
            self.level.torches.clear();
            self.level.torches.extend(crate::world::terrain::listed_near(&self.terrain.torches, c, 20, 12).filter(|&p| is_torch(w.geti(p))));
        }
        // Torches in hands burn too: this player's (where it was drawn) and the others'
        // (about where they hold it up).
        let mut tips: Vec<Vec3> = self.held_torch_tip.into_iter().collect();
        tips.extend(
            self.remote_held_lights()
                .into_iter()
                .filter(|&(_, _, item)| item == TORCH as ItemId)
                .map(|(_, light, _)| light + Vec3::Y * 0.3),
        );
        for tip in tips {
            if self.random() < dt * 3.0 {
                self.particles.flame(tip);
            }
            if self.random() < dt * 1.0 {
                let w = &self.terrain.world;
                let (sky, blk) = (w.sky_estimate(tip), w.block_light_estimate(tip));
                self.particles.smoke(tip + Vec3::Y * 0.08, sky, blk);
            }
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
            if self.random() < dt * 3.0 {
                self.particles.flame(tip);
            }
            if self.random() < dt * 1.0 {
                let w = &self.terrain.world;
                let (sky, blk) = (w.sky_estimate(tip), w.block_light_estimate(tip));
                self.particles.smoke(tip + Vec3::Y * 0.08, sky, blk);
            }
        }
    }

}
