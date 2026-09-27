//! The per-frame simulation: the player (movement, targeting, mining, using) and the world
//! (chest lids, fluids, furnaces, saplings, dropped items, mobs, falling blocks, torch fire,
//! time of day and autosave).

use super::*;
use crate::entity::player::{raycast, MoveInput};
use crate::item::*;
impl Game {
    /// Player simulation. `control` is false while a screen is open (physics still runs).
    pub(super) fn update_player(&mut self, dt: f32, control: bool) {
        if control {
            // Slower turning while zoomed in.
            let zoom = (self.fov_current / self.settings.fov).min(1.0);
            let sens = 0.0022 * self.settings.sensitivity / 100.0 * zoom;
            self.yaw += self.mouse_delta.x * sens;
            self.pitch = (self.pitch - self.mouse_delta.y * sens).clamp(-1.55, 1.55);
            if self.scroll != 0.0 {
                let d = if self.scroll > 0.0 { -1 } else { 1 };
                self.hotbar_slot = (self.hotbar_slot as i32 + d).rem_euclid(9) as usize;
                self.slot_name_timer = 2.0;
            }
        }
        self.hand.equip(self.held());
        if self.sleep.is_some() {
            self.update_sleep(dt, control);
            return;
        }
        if !self.creative() {
            self.player.flying = false;
        }
        // Holding the right mouse button with a sword blocks (Minecraft 1.8).
        self.blocking = control && self.right_down && is_sword(self.held());
        self.hand.blocking = self.blocking;
        self.update_using(dt, control);

        let k = |b: Bind| control && self.bind_down(b);
        let axis = |a: bool, b: bool| (a as i32 - b as i32) as f32;
        let input = MoveInput {
            forward: axis(k(Bind::Forward), k(Bind::Back)),
            strafe: axis(k(Bind::Right), k(Bind::Left)),
            up: k(Bind::Jump),
            down: k(Bind::Sneak),
            sprint: (k(Bind::Sprint) || (self.w_sprint && k(Bind::Forward)))
                && (self.creative() || self.needs.can_sprint()),
            sneak: k(Bind::Sneak),
            using: self.blocking || self.using.is_some(),
        };
        let was_on_ground = self.player.on_ground;
        self.player
            .update(dt, &self.terrain.world, self.yaw, &input);
        self.update_health(dt, was_on_ground);
        if self.screen == Screen::Dead {
            return;
        }
        let speed = self.player.horizontal_speed();
        if !self.creative() {
            // Sprinting, swimming and jumping make you hungry and thirsty.
            use crate::entity::survival::cost;
            let moved = speed * dt;
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

        // Targeting, mining and placing
        let eye = self.player.eye();
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
                .mobs
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
            self.mob_target = mob.filter(|_| player.is_none()).map(|(i, _)| i);
            if self.mob_target.is_some() || self.player_target.is_some() {
                self.target = None;
            }
        }
        self.action_cooldown -= dt;
        self.update_guns(dt, control);
        let mut breaking = None;
        // A sword does not break blocks at all (it only fights); with a pistol the left mouse
        // button shoots instead (one shot per click, no hitting).
        let sword = is_sword(self.held());
        let gun = self.holding_gun();
        // Holding the guide book: the buttons turn its pages (they never hit blocks).
        let reading = control && self.book_in_hand();
        if reading {
            let (left, right) = (self.left_pressed, self.right_down);
            self.book_buttons(dt, left, right);
        }
        if sword || gun || reading {
            self.mining = None;
        }
        // A left click on what is in a furnace takes it out instead of mining (a pistol shoots).
        let furnace_hold = control && !gun && !reading && self.furnace_left_click();
        if control
            && !reading
            && self.left_down
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
        } else if !self.left_down {
            self.mining = None;
            self.dig_timer = 0.0;
        }
        // Hitting: either block with the sword or strike, not both.
        if control
            && !reading
            && self.left_pressed
            && (self.target.is_none() || sword)
            && !self.blocking
            && !gun
        {
            if let Some(i) = self.mob_target {
                self.attack(Some(i), None);
            }
            if let Some(id) = self.player_target {
                self.attack(None, Some(id));
            }
            self.hand.swing();
        }
        // One shot per click; an automatic gun keeps firing while the button is held.
        let auto = GunKind::of(self.held()).is_some_and(|k| k.stats().auto);
        if control && gun && (self.left_pressed || (auto && self.left_down)) {
            self.shoot();
        }
        if let Some(p) = breaking {
            self.break_block(p);
        }
        if control
            && !reading
            && (self.right_pressed || (self.right_down && self.action_cooldown <= 0.0))
        {
            self.use_item();
        }
        // Pick block (creative).
        if control && self.middle_pressed && self.creative() {
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

        // Body follows the head when moving, otherwise lags within 50 degrees (like Minecraft).
        let diff = crate::util::wrap_angle(self.yaw - self.body_yaw);
        if speed > 0.1 {
            self.body_yaw += diff * (1.0 - (-10.0 * dt).exp());
        } else if self.camera.mode < 3 && diff.abs() > 50f32.to_radians() {
            // Shoulder views orbit the body while stationary; in regular views the torso
            // follows the head so a large turn still looks natural.
            let excess = diff - diff.signum() * 50f32.to_radians();
            self.body_yaw += excess * (1.0 - (-12.0 * dt).exp());
        }
        // Limb swing follows horizontal movement (in the air too, like Minecraft).
        let fly = if self.player.flying { 0.3 } else { 1.0 };
        let target = (speed / 4.3).min(1.0) * fly;
        self.limb_amount += (target - self.limb_amount) * (1.0 - (-10.0 * dt).exp());
    }

    pub(super) fn update_world(&mut self, dt: f32) {
        // Chest lids: open chests (this player's and the other LAN players') swing up, the
        // others fall shut.
        let mut open_chests = self.remote_open_chests();
        if let Screen::Container(Container::Chest(p)) = self.screen {
            open_chests.push(p);
        }
        for p in &open_chests {
            self.chest_open.entry(*p).or_insert(0.0);
        }
        self.chest_open.retain(|p, k| {
            let target = if open_chests.contains(p) { 1.0 } else { 0.0 };
            *k = if target > *k {
                (*k + dt * 3.5).min(1.0)
            } else {
                (*k - dt * 3.0).max(0.0)
            };
            *k > 0.0 || target > 0.0
        });

        self.furnace_fx(dt);
        if self.is_client() {
            // A LAN player's world is run by the host: only follow what it sends.
            self.client_world(dt);
            return;
        }

        // Fluids
        let mut changed = Vec::new();
        self.fluids
            .update(dt, self.time, &mut self.terrain.world, &mut changed);
        for p in changed {
            let wide = is_lava(self.terrain.world.geti(p));
            self.terrain.block_changed(p, wide);
        }
        for (p, b) in std::mem::take(&mut self.fluids.broken) {
            let r = self.random();
            for s in drops(b, NONE, r) {
                self.spawn_drop(p.as_vec3() + Vec3::splat(0.5), s);
            }
        }

        // Furnaces: cook, smelt, and switch between lit/unlit blocks.
        self.update_furnaces(dt);

        // Saplings grow into trees.
        let mut grow = Vec::new();
        for (p, t) in self.saplings.iter_mut() {
            *t -= dt;
            if *t <= 0.0 {
                grow.push(*p);
            }
        }
        for p in grow {
            self.saplings.retain(|(q, _)| *q != p);
            let b = self.terrain.world.geti(p);
            if is_sapling(b) && !self.grow_tree(p, b) {
                // No room yet: try again later.
                self.saplings.push((p, 30.0));
            }
        }

        // Dropped items: physics and pickup.
        let center = self.player.pos + Vec3::Y * 0.9;
        let pickup_target = self.player.eye() - Vec3::Y * 0.25;
        let alive = self.player.spawned && self.screen != Screen::Dead;
        let mut pickup_visuals = Vec::new();
        let mut i = 0;
        while i < self.items.len() {
            if self.items[i].is_picking_up() {
                if !alive || self.items[i].update_pickup(dt, pickup_target) {
                    self.items.swap_remove(i);
                    continue;
                }
                i += 1;
                continue;
            }
            // Items in unloaded chunks wait there instead of falling through the missing ground.
            let p = self.items[i].pos;
            if !self
                .terrain
                .world
                .is_loaded(p.x.floor() as i32, p.z.floor() as i32)
            {
                i += 1;
                continue;
            }
            self.items[i].update(dt, &self.terrain.world);
            let it = &self.items[i];
            let (pos, age, pickup_delay, stack) = (it.pos, it.age, it.pickup_delay, it.stack);
            if age > 300.0 || pos.y < -64.0 {
                self.items.swap_remove(i);
                continue;
            }
            if alive && pickup_delay <= 0.0 && (pos + Vec3::Y * 0.2).distance(center) < 1.5 {
                match self.inventory.add(stack) {
                    None => {
                        self.items[i].start_pickup(self.time);
                    }
                    Some(left) => {
                        self.items[i].stack = left;
                        if left.count < stack.count {
                            let collected = Stack {
                                count: stack.count - left.count,
                                ..stack
                            };
                            let mut visual = ItemEntity::new(pos, Vec3::ZERO, collected, 0.0);
                            visual.age = age;
                            visual.start_pickup(self.time);
                            pickup_visuals.push(visual);
                        }
                    }
                }
            }
            i += 1;
        }
        self.items.extend(pickup_visuals);
        self.merge_items();

        // Mobs
        self.update_mobs(dt);
        self.spawn_animals(dt);

        // Falling sand / gravel.
        let mut landed = Vec::new();
        let mut i = 0;
        while i < self.falling.len() {
            let p = self.falling[i].pos;
            if !self
                .terrain
                .world
                .is_loaded(p.x.floor() as i32, p.z.floor() as i32)
            {
                i += 1;
                continue;
            }
            if self.falling[i].update(dt, &self.terrain.world) {
                landed.push(self.falling.swap_remove(i));
            } else {
                i += 1;
            }
        }
        for f in landed {
            let at = f.pos.floor().as_ivec3();
            if is_replaceable(self.terrain.world.geti(at)) {
                self.set_block(at, f.block);
                self.block_updated(at);
            } else {
                self.spawn_drop(f.pos + Vec3::Y * 0.5, Stack::one(f.block as ItemId));
            }
        }

        if self.torch_particles {
            self.torch_fire(dt);
        }

        self.time_of_day = (self.time_of_day + dt / DAY_LENGTH).fract();
        self.update_sleepers(dt);
        self.autosave -= dt;
        if self.autosave <= 0.0 {
            self.autosave = AUTOSAVE_SECONDS;
            self.save_world();
        }
    }

    /// Resource-pack torch fire: flame and smoke particles rising from the tips of the torches
    /// around the player, like Minecraft's torches.
    pub(super) fn torch_fire(&mut self, dt: f32) {
        self.torch_scan -= dt;
        if self.torch_scan <= 0.0 {
            self.torch_scan = 1.0;
            let c = self.player.pos.floor().as_ivec3();
            let w = &self.terrain.world;
            self.torches.clear();
            for dy in -12..=12 {
                for dz in -20..=20 {
                    for dx in -20..=20 {
                        let p = c + IVec3::new(dx, dy, dz);
                        if is_torch(w.geti(p)) {
                            self.torches.push(p);
                        }
                    }
                }
            }
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
        for i in 0..self.torches.len() {
            let p = self.torches[i];
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

    /// Nearby dropped stacks of the same item combine into one entity, like in Minecraft.
    pub(super) fn merge_items(&mut self) {
        let mut a = 0;
        while a < self.items.len() {
            let mut b = a + 1;
            while b < self.items.len() {
                let (x, y) = (&self.items[a], &self.items[b]);
                let fits =
                    x.stack.count as u16 + y.stack.count as u16 <= max_stack(x.stack.item) as u16;
                if !x.is_picking_up()
                    && !y.is_picking_up()
                    && x.stack.stacks_with(&y.stack)
                    && fits
                    && x.pos.distance_squared(y.pos) < 0.5 * 0.5
                {
                    let y = self.items.swap_remove(b);
                    let x = &mut self.items[a];
                    x.stack.count += y.stack.count;
                    x.age = x.age.min(y.age);
                    x.pickup_delay = x.pickup_delay.max(y.pickup_delay);
                } else {
                    b += 1;
                }
            }
            a += 1;
        }
    }
}
