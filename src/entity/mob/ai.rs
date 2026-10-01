//! The AI every mob runs (its kind's hooks asked first): where to walk, panicking, chasing
//! and attacking a foe, looking about, and steering its steps.

use super::*;

impl Mob {
    /// Picks a random spot to walk to: up to `radius` blocks away, preferring grass
    /// (Minecraft's animals score grass higher), never into fluids. `away` biases the
    /// direction (panic runs away from the attacker).
    fn pick_target(&mut self, w: &World, radius: f32, away: Option<Vec3>) -> Option<Vec3> {
        let mut best: Option<(Vec3, f32)> = None;
        for _ in 0..10 {
            let mut off = Vec2::new(self.rand() * 2.0 - 1.0, self.rand() * 2.0 - 1.0) * radius;
            if let Some(src) = away {
                let dir = Vec2::new(self.pos.x - src.x, self.pos.z - src.z).normalize_or_zero();
                if off.dot(dir) < 0.0 {
                    off = -off;
                }
            }
            if off.length() < 1.5 {
                continue;
            }
            let (x, z) = (
                (self.pos.x + off.x).floor() as i32,
                (self.pos.z + off.y).floor() as i32,
            );
            let Some((p, ground)) = standable(w, x, z, self.pos.y.round() as i32, 5) else {
                continue;
            };
            if is_lava(ground) || ground == CACTUS {
                continue;
            }
            let score = if ground == GRASS { 10.0 } else { 0.0 } + self.rand();
            if best.is_none_or(|(_, s)| score > s) {
                best = Some((p, score));
            }
        }
        best.map(|(p, _)| p)
    }

    /// Is the ground at `p` (feet level) safe to step onto? No drops over 3 blocks, no lava,
    /// no water unless `wet_ok`.
    fn safe_step(w: &World, p: Vec3, wet_ok: bool) -> bool {
        let (x, z) = (p.x.floor() as i32, p.z.floor() as i32);
        let y = p.y.floor() as i32;
        if !w.is_loaded(x, z) {
            return false;
        }
        for dy in 0..=4 {
            let b = w.get(x, y - dy, z);
            if is_lava(b) || b == CACTUS {
                return false;
            }
            if is_water(b) {
                return wet_ok;
            }
            if is_solid(b) {
                // Ground at most 3 blocks down (dy = 0 is a step up).
                return true;
            }
        }
        false
    }

    /// The closest player within `range`.
    pub(crate) fn nearest_player(&self, ctx: &MobCtx, range: f32) -> Option<Vec3> {
        ctx.players
            .iter()
            .copied()
            .filter(|p| p.distance(self.pos) < range)
            .min_by(|a, b| a.distance(self.pos).total_cmp(&b.distance(self.pos)))
    }

    /// Where someone it goes for is (their feet), if they are still around.
    fn foe_pos(&self, ctx: &MobCtx) -> Option<Vec3> {
        match self.foe? {
            Foe::Mob(id) => ctx.mobs.iter().find(|(i, _)| *i == id).map(|(_, p)| *p),
            Foe::Player(id) => ctx.people.iter().find(|(i, _, _)| *i == id).map(|(_, _, p)| *p),
        }
    }

    /// Going for someone (Minecraft's MeleeAttackGoal): runs to them and attacks when near.
    /// It calms down in a while (a pet does not); anyone gone or far away is let go. Some
    /// while it is after someone.
    pub(crate) fn chase(&mut self, dt: f32, w: &World, ctx: &MobCtx) -> Option<Steer> {
        self.foe?;
        let ai = self.def().ai;
        let pet = self.owner().is_some();
        if !pet {
            self.anger -= dt;
        }
        match self.foe_pos(ctx) {
            Some(p) if p.distance(self.pos) < CHASE_RANGE && (pet || self.anger > 0.0) => {
                let d = Vec2::new(p.x - self.pos.x, p.z - self.pos.z);
                self.look = Look::Ahead;
                if d.length() < ai.reach && (p.y - self.pos.y).abs() < 1.5 {
                    self.target = None;
                    self.body_yaw = turn(self.body_yaw, d.y.atan2(d.x), 12.0 * dt);
                    if self.attack_cooldown <= 0.0 {
                        self.attack_cooldown = ai.attack_every;
                        self.attack = self.foe;
                    }
                    return Some(None);
                }
                self.target = Some(p);
                self.target_time = 1.0;
                self.hurry = Some(self.def().speed * ai.chase);
                Some(self.walk(w))
            }
            _ => {
                self.foe = None;
                self.target = None;
                None
            }
        }
    }

    /// The template's goals, after the mob's own (`Hooks::think`): going for someone,
    /// looking at players and around, panicking and strolling.
    pub(super) fn think(&mut self, dt: f32, w: &World, ctx: &MobCtx) -> Steer {
        let def = self.def();
        let ai = def.ai;
        self.look_time -= dt;
        self.attack_cooldown -= dt;
        self.hurry = None;
        if let Some(think) = def.hooks.think {
            if let Some(steer) = think(self, dt, w, ctx) {
                return steer;
            }
        }
        // A hostile one goes for the nearest player it sees.
        if ai.hunts > 0.0 && self.foe.is_none() {
            let near = ctx
                .people
                .iter()
                .filter(|(_, _, p)| p.distance(self.pos) < ai.hunts)
                .min_by(|a, b| a.2.distance(self.pos).total_cmp(&b.2.distance(self.pos)));
            if let Some((id, _, _)) = near {
                self.foe = Some(Foe::Player(*id));
                self.anger = ai.anger;
            }
        }
        if let Some(steer) = self.chase(dt, w, ctx) {
            return steer;
        }
        // Looking: at a nearby player now and then, or around (Minecraft's LookAtPlayerGoal
        // and RandomLookAroundGoal, each started with a 2% chance per tick).
        let player_near = self.nearest_player(ctx, ai.looks).filter(|_| self.panic <= 0.0);
        if let (Look::Player, None) = (self.look, player_near) {
            self.look_time = 0.0;
        }
        if self.look_time <= 0.0 {
            self.look = Look::Ahead;
            if player_near.is_some() && self.rand() < per_tick(0.02, dt) {
                self.look = Look::Player;
                self.look_time = 2.0 + self.rand() * 2.0;
            } else if self.target.is_none() && self.rand() < per_tick(0.02, dt) {
                let yaw = self.body_yaw + (self.rand() - 0.5) * TAU * 0.8;
                self.look = Look::Yaw(yaw);
                self.look_time = 1.0 + self.rand();
            }
        }

        // Moving.
        if self.panic > 0.0 {
            self.panic -= dt;
            if self.target.is_none() || self.target_time <= 0.0 {
                self.target = self.pick_target(w, 6.0, self.flee_from);
                self.target_time = 3.0;
                self.stuck = 0.0;
            }
        } else if self.target.is_none() && ai.wander {
            self.idle -= dt;
            // Minecraft's RandomStrollGoal: a 1 in 120 chance per tick once idle.
            if self.idle <= 0.0 && self.rand() < per_tick(1.0 / 120.0, dt) {
                self.target = self.pick_target(w, 10.0, None);
                self.target_time = 10.0;
                self.stuck = 0.0;
                if self.target.is_none() {
                    self.idle = 1.0;
                }
            }
        }
        self.target_time -= dt;
        self.walk(w)
    }

    /// Walks toward `target` (at its `hurry`, or its pace), looking out for drops, lava,
    /// water and walls, jumping up single blocks.
    pub(crate) fn walk(&mut self, w: &World) -> Steer {
        let target = self.target?;
        let to = Vec2::new(target.x - self.pos.x, target.z - self.pos.z);
        if to.length() < 0.4 || self.target_time <= 0.0 || self.stuck > 1.5 {
            self.target = None;
            self.idle = 1.0 + self.rand() * 4.0;
            return None;
        }
        let pace = self.def().speed;
        let speed = if let Some(s) = self.hurry {
            s
        } else if self.panic > 0.0 {
            pace * PANIC_SPEED
        } else {
            pace
        };
        let dir = to.normalize();
        let dir3 = Vec3::new(dir.x, 0.0, dir.y);
        let (half_w, tall) = self.size();
        // Look before stepping: stop at cliffs, lava and (unless already swimming) water.
        let ahead = self.pos + dir3 * (half_w + 0.35) + Vec3::Y * 0.5;
        let feet = w.get(
            ahead.x.floor() as i32,
            self.pos.y.floor() as i32,
            ahead.z.floor() as i32,
        );
        let step_up = is_solid(feet);
        let probe = if step_up { ahead + Vec3::Y } else { ahead };
        if !Self::safe_step(w, probe, self.in_water) {
            self.target = None;
            self.idle = 0.5 + self.rand() * 2.0;
            self.vel.x *= 0.2;
            self.vel.z *= 0.2;
            return None;
        }
        if step_up && !self.step_room(w, ahead, tall) {
            // A wall: give up on this spot.
            self.target = None;
            self.idle = 0.5 + self.rand() * 2.0;
            return None;
        }
        if step_up && self.on_ground && self.jump_cooldown <= 0.0 {
            self.vel.y = JUMP;
            self.jump_cooldown = 0.4;
        }
        Some((dir3, speed))
    }

    /// Room to step up onto the block at `ahead`: its whole height free on top of it, and
    /// above its head where it stands (for the jump).
    pub(super) fn step_room(&self, w: &World, ahead: Vec3, tall: f32) -> bool {
        let y = self.pos.y.floor();
        // The highest block its head is in, standing on the step.
        let top = (y + 1.0 + tall - 1e-3).floor() as i32;
        let (ax, az) = (ahead.x.floor() as i32, ahead.z.floor() as i32);
        let (x, z) = (self.pos.x.floor() as i32, self.pos.z.floor() as i32);
        let head_now = (self.pos.y + tall - 1e-3).floor() as i32;
        (y as i32 + 1..=top).all(|yy| !is_solid(w.get(ax, yy, az)))
            && (head_now..=top).all(|yy| !is_solid(w.get(x, yy, z)))
    }
}
