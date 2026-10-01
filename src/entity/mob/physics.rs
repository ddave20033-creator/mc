//! A mob moving through the world: its steps (a tick each), gravity, jumping, swimming,
//! collisions, and the damage the world does (falls, lava, fire, cactus, suffocation).

use super::*;

impl Mob {
    pub(crate) fn move_axis(&mut self, w: &World, axis: usize, d: f32) -> bool {
        if d == 0.0 {
            return false;
        }
        let size = self.size();
        let mut p = self.pos;
        p[axis] += d;
        if !collides(w, p, size) {
            self.pos = p;
            return false;
        }
        // Stop flush against the block that was hit.
        let (lo, hi) = if axis == 1 {
            (0.0, size.1)
        } else {
            (size.0, size.0)
        };
        let snapped = if d > 0.0 {
            (p[axis] + hi).floor() - hi - 1e-3
        } else {
            (p[axis] - lo).floor() + 1.0 + lo + 1e-3
        };
        let mut q = self.pos;
        q[axis] = snapped;
        let forward = if d > 0.0 {
            snapped > self.pos[axis]
        } else {
            snapped < self.pos[axis]
        };
        if forward && !collides(w, q, size) {
            self.pos = q;
        }
        true
    }

    /// Advances the mob by `dt` seconds: in steps of at most 1/20 s, so a slow frame does not
    /// slow it down (the first thing that happens ends it).
    pub fn update(&mut self, dt: f32, w: &World, ctx: &MobCtx) -> MobEvent {
        let steps = (dt / 0.05).ceil().max(1.0) as usize;
        for _ in 0..steps {
            let event = self.step(dt / steps as f32, w, ctx);
            if !matches!(event, MobEvent::None) {
                return event;
            }
        }
        MobEvent::None
    }

    /// A static one's step: it only falls (onto what it stands on).
    fn step_static(&mut self, dt: f32, w: &World) -> MobEvent {
        self.vel = Vec3::new(0.0, (self.vel.y - GRAVITY * dt).max(-60.0), 0.0);
        if self.move_axis(w, 1, self.vel.y * dt) {
            self.vel.y = 0.0;
        }
        match self.def().hooks.tick {
            Some(tick) => tick(self, dt, w, false),
            None => MobEvent::None,
        }
    }

    fn step(&mut self, dt: f32, w: &World, ctx: &MobCtx) -> MobEvent {
        let def = self.def();
        if !def.ai.moves {
            return self.step_static(dt, w);
        }
        self.hurt_time = (self.hurt_time - dt).max(0.0);
        self.jump_cooldown -= dt;
        self.damage_tick -= dt;
        if let Some(t) = &mut self.death {
            *t += dt;
            if *t >= DEATH_TIME {
                return MobEvent::Remove;
            }
        }

        self.in_water = is_water(w.get(
            self.pos.x.floor() as i32,
            (self.pos.y + 0.4).floor() as i32,
            self.pos.z.floor() as i32,
        ));
        let size = self.size();
        let in_lava = touching(w, self.pos, size, -0.05, is_lava);

        // Knocked back: no steering for a moment, so the hit carries it.
        let steer = if self.alive() && self.hurt_time < HURT_TIME - 0.2 {
            self.think(dt, w, ctx)
        } else {
            None
        };
        let want = steer.map(|(d, s)| d * s).unwrap_or(Vec3::ZERO);
        let grip = if self.on_ground {
            12.0
        } else if self.in_water {
            4.0
        } else {
            // Minecraft's air drag: 0.91 per tick.
            1.9
        };
        let k = crate::util::damp(grip, dt);
        self.vel.x += (want.x - self.vel.x) * k;
        self.vel.z += (want.z - self.vel.z) * k;

        if self.in_water {
            // Floats up to the surface and bobs there (Minecraft's FloatGoal).
            let target_up = if self.alive() { 2.0 } else { 0.5 };
            self.vel.y += (target_up - self.vel.y) * (crate::util::damp(3.0, dt));
            self.fall_peak = self.pos.y;
        } else if in_lava {
            self.vel.y = (self.vel.y - 8.0 * dt).max(-1.5);
            self.fall_peak = self.pos.y;
        } else {
            self.vel.y = (self.vel.y - GRAVITY * dt).max(-60.0);
        }

        // Move in small steps so fast falls cannot tunnel through blocks.
        let delta = self.vel * dt;
        let steps = (delta.abs().max_element() / 0.4).ceil().max(1.0) as i32;
        let was_on_ground = self.on_ground;
        self.on_ground = false;

        for _ in 0..steps {
            let d = delta / steps as f32;
            if self.move_axis(w, 1, d.y) {
                if d.y < 0.0 {
                    self.on_ground = true;
                }
                self.vel.y = 0.0;
            }
            self.move_axis(w, 0, d.x);
            self.move_axis(w, 2, d.z);
        }
        if !self.on_ground && self.vel.y <= 0.0 {
            // Resting on the ground (no vertical movement this frame).
            self.on_ground = collides(w, self.pos - Vec3::Y * 0.02, size);
        }

        // Fall damage: 1 per block after the first 3.
        if self.on_ground {
            if !was_on_ground {
                let fall = self.fall_peak - self.pos.y;
                if fall > 3.0 {
                    self.hurt_env((fall - 3.0).ceil());
                }
            }
            self.fall_peak = self.pos.y;
        } else {
            self.fall_peak = self.fall_peak.max(self.pos.y);
        }

        // Burning, cactus, being stuck inside blocks.
        if in_lava {
            self.fire = 8.0;
            if self.damage_tick <= 0.0 {
                self.damage_tick = 0.5;
                self.hurt_env(4.0);
            }
            if self.alive() {
                self.panic = self.panic.max(1.0);
            }
        } else if self.in_water {
            self.fire = 0.0;
        }
        if self.fire > 0.0 {
            self.fire -= dt;
            if self.damage_tick <= 0.0 {
                self.damage_tick = 1.0;
                self.hurt_env(1.0);
            }
        }
        if touching(w, self.pos, size, 0.01, |b| b == CACTUS) && self.damage_tick <= 0.0 {
            self.damage_tick = 0.5;
            self.hurt_env(1.0);
        }
        let head = self.pos + Vec3::Y * (size.1 - 0.1);
        if is_opaque(w.get(
            head.x.floor() as i32,
            head.y.floor() as i32,
            head.z.floor() as i32,
        )) && self.damage_tick <= 0.0
        {
            self.damage_tick = 0.5;
            self.hurt_env(1.0);
        }

        let hvel = Vec2::new(self.vel.x, self.vel.z);
        // Turning: while walking the body faces where it is going (toward the path, not the
        // drift from a push) and the head may only look up to 75 degrees aside from it;
        // standing still, the body slowly follows the head instead.
        let moving = steer.is_some() && hvel.length() > 0.3;
        if let (true, Some((dir, _))) = (moving, steer) {
            self.still = 0.0;
            self.body_yaw = turn(self.body_yaw, dir.z.atan2(dir.x), 10.0 * dt);
        } else {
            self.still += dt;
        }
        let (want_yaw, want_pitch) = match self.look {
            Look::Player => match self.nearest_player(ctx, 8.0) {
                Some(p) => {
                    let eye = self.pos + Vec3::Y * 0.75;
                    let d = p + Vec3::Y * 1.62 - eye;
                    (d.z.atan2(d.x), d.y.atan2(Vec2::new(d.x, d.z).length()))
                }
                None => (self.body_yaw, 0.0),
            },
            Look::Yaw(y) if !moving => (y, 0.0),
            _ => (self.body_yaw, 0.0),
        };
        if self.alive() {
            self.head_yaw = turn(self.head_yaw, want_yaw, 8.0 * dt);
            self.pitch += (want_pitch.clamp(-0.8, 0.8) - self.pitch) * (crate::util::damp(8.0, dt));
        }
        let rel = wrap_angle(self.head_yaw - self.body_yaw);
        if moving {
            self.head_yaw = self.body_yaw + rel.clamp(-HEAD_LIMIT, HEAD_LIMIT);
        } else if rel.abs() > HEAD_LIMIT {
            self.body_yaw = self.head_yaw - rel.signum() * HEAD_LIMIT;
        } else if self.still > 0.5 {
            self.body_yaw = turn(self.body_yaw, self.head_yaw, 1.5 * dt);
        }

        // Stuck detection: trying to walk but barely moving.
        if steer.is_some() && hvel.length() < 0.25 {
            self.stuck += dt;
        } else {
            self.stuck = 0.0;
        }

        // Leg swing follows horizontal movement.
        let speed = if self.alive() { hvel.length() } else { 0.0 };
        let target = (speed * 0.25).min(1.0);
        self.limb_amount += (target - self.limb_amount) * (crate::util::damp(10.0, dt));
        self.limb_swing += speed * dt * 4.0;

        if let Some(tick) = def.hooks.tick {
            let event = tick(self, dt, w, steer.is_some());
            if !matches!(event, MobEvent::None) {
                return event;
            }
        }
        if let (Some(f), true) = (self.attack.take(), self.alive()) {
            return MobEvent::Attack(f);
        }
        MobEvent::None
    }
}
