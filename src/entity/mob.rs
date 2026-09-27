//! Mobs (the pig and the sheep): physics, a Minecraft-style passive animal AI and the models.
//!
//! The AI follows Minecraft's goals for animals: panic after being hurt (run to random spots,
//! away from whoever hit it), wander to random nearby spots (preferring grass, avoiding
//! water, lava and drops of more than 3 blocks), look at a nearby player, look around, and
//! float in water. Mobs jump up single blocks, take fall, lava, cactus and suffocation
//! damage, get knocked back, flash red when hurt and tip over when they die.
//!
//! Models use Minecraft's entity model format: cubes with box UVs into a 64x64 texel atlas
//! (one texture layer), set up exactly like Minecraft's `PigModel`.

use crate::util::{ray_box, vertex_light, wrap_angle, Rng};
use crate::world::mesh::{flags, Vertex};
use crate::world::textures::tex;
use crate::world::*;
use glam::{Mat4, Vec2, Vec3};
use std::f32::consts::{FRAC_PI_2, PI, TAU};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MobKind {
    Pig,
    Sheep,
}

impl MobKind {
    pub fn key(self) -> &'static str {
        match self {
            MobKind::Pig => "pig",
            MobKind::Sheep => "sheep",
        }
    }

    pub fn from_key(k: &str) -> Option<MobKind> {
        match k.strip_prefix("minecraft:").unwrap_or(k) {
            "pig" => Some(MobKind::Pig),
            "sheep" => Some(MobKind::Sheep),
            _ => None,
        }
    }

    /// The kind with this `kind as u8` (LAN messages).
    pub fn from_u8(v: u8) -> Option<MobKind> {
        match v {
            0 => Some(MobKind::Pig),
            1 => Some(MobKind::Sheep),
            _ => None,
        }
    }

    pub fn max_health(self) -> f32 {
        match self {
            MobKind::Pig => 10.0,
            MobKind::Sheep => 8.0,
        }
    }

    /// Half width and height of the bounding box (Minecraft: pig 0.9 x 0.9, sheep 0.9 x 1.3).
    pub fn size(self) -> (f32, f32) {
        match self {
            MobKind::Pig => (0.45, 0.9),
            MobKind::Sheep => (0.45, 1.3),
        }
    }
}
const GRAVITY: f32 = 32.0;
/// Jump speed: clears one block (Minecraft: 0.42 blocks per tick).
const JUMP: f32 = 8.6;
const WALK_SPEED: f32 = 1.7;
/// Minecraft's PanicGoal runs at 1.25x the walking speed.
const PANIC_SPEED: f32 = WALK_SPEED * 1.25;
/// Knockback speeds (horizontal, up) in blocks per second, tuned so a normal hit moves a
/// mob about 1.5 blocks like in Minecraft (the wiki measures 1.552 blocks).
const KNOCKBACK: f32 = 5.0;
const KNOCKBACK_UP: f32 = 6.0;
/// Seconds a hit mob stays red and cannot be hurt again.
const HURT_TIME: f32 = 0.5;
/// Seconds of the death animation (tipping over) before it disappears.
pub const DEATH_TIME: f32 = 1.0;
/// Head turn limit relative to the body.
const HEAD_LIMIT: f32 = 75.0 * PI / 180.0;

/// What the world around a mob looks like to its AI.
pub struct MobCtx {
    /// Feet of every living player (the host and LAN players).
    pub players: Vec<Vec3>,
}

/// Something that happened to a mob during an update.
pub enum MobEvent {
    None,
    /// The death animation is over: remove it and drop its loot.
    Remove,
    /// A sheep ate the grass here (a grass block turns to dirt, tall grass goes).
    EatGrass(glam::IVec3),
}

pub struct Mob {
    /// Unique id for LAN play.
    pub id: u32,
    /// LAN player: the latest state from the host, which the mob glides toward.
    net_target: Option<crate::net::MobNet>,
    pub kind: MobKind,
    /// Feet position (center of the bottom face).
    pub pos: Vec3,
    pub vel: Vec3,
    /// Yaw like the player's: forward is (cos yaw, 0, sin yaw).
    pub body_yaw: f32,
    pub head_yaw: f32,
    /// Positive looks up.
    pub pitch: f32,
    pub health: f32,
    /// Red flash left after a hit.
    pub hurt_time: f32,
    /// Seconds since dying (None while alive).
    pub death: Option<f32>,
    /// Seconds on fire left (from lava); a pig that dies burning drops cooked meat.
    pub fire: f32,
    /// A sheep without its wool (it grows back when the sheep eats grass).
    pub sheared: bool,
    on_ground: bool,
    in_water: bool,
    /// Highest point since leaving the ground, for fall damage.
    fall_peak: f32,
    limb_swing: f32,
    limb_amount: f32,
    /// Where it is walking to, and for how long it keeps trying.
    target: Option<Vec3>,
    target_time: f32,
    /// Seconds to stand around before the next stroll.
    idle: f32,
    /// Seconds of panic left (after being hurt).
    panic: f32,
    /// Where the last hit came from (runs away from it).
    flee_from: Option<Vec3>,
    /// Seconds without making progress toward the target.
    stuck: f32,
    look: Look,
    look_time: f32,
    /// Standing still: seconds, and the body slowly turns to where the head looks.
    still: f32,
    jump_cooldown: f32,
    damage_tick: f32,
    rng: Rng,
}

#[derive(Clone, Copy)]
enum Look {
    Ahead,
    Player,
    Yaw(f32),
}

fn collides(w: &World, p: Vec3, (half_w, tall): (f32, f32)) -> bool {
    let min = p - Vec3::new(half_w, 0.0, half_w);
    let max = p + Vec3::new(half_w, tall, half_w);
    for x in min.x.floor() as i32..=(max.x - 1e-4).floor() as i32 {
        for z in min.z.floor() as i32..=(max.z - 1e-4).floor() as i32 {
            if !w.is_loaded(x, z) {
                return true;
            }
            for y in min.y.floor() as i32..=(max.y - 1e-4).floor() as i32 {
                if is_solid(w.get(x, y, z)) {
                    return true;
                }
            }
        }
    }
    false
}

/// Blocks overlapping the box grown by `grow`.
fn touching(
    w: &World,
    p: Vec3,
    (half_w, tall): (f32, f32),
    grow: f32,
    pred: impl Fn(u8) -> bool,
) -> bool {
    let min = p - Vec3::new(half_w + grow, grow, half_w + grow);
    let max = p + Vec3::new(half_w + grow, tall + grow, half_w + grow);
    for x in min.x.floor() as i32..=(max.x - 1e-4).floor() as i32 {
        for y in min.y.floor() as i32..=(max.y - 1e-4).floor() as i32 {
            for z in min.z.floor() as i32..=(max.z - 1e-4).floor() as i32 {
                if pred(w.get(x, y, z)) {
                    return true;
                }
            }
        }
    }
    false
}

/// Turns `cur` toward `want` by at most `step` radians.
fn turn(cur: f32, want: f32, step: f32) -> f32 {
    cur + wrap_angle(want - cur).clamp(-step, step)
}

/// Chance that something with probability `p` per game tick (1/20 s) happens within `dt`.
fn per_tick(p: f32, dt: f32) -> f32 {
    1.0 - (1.0 - p).powf(dt * 20.0)
}

/// A spot a mob can stand on in column (x, z) near height `y`: solid ground below, room for
/// its body. Returns the feet position and the ground block.
pub fn standable(w: &World, x: i32, z: i32, y: i32, range: i32) -> Option<(Vec3, u8)> {
    if !w.is_loaded(x, z) {
        return None;
    }
    for dy in 0..=range * 2 {
        // Search outward from `y`: y, y+1, y-1, y+2, ...
        let yy = if dy % 2 == 0 {
            y - dy / 2
        } else {
            y + (dy + 1) / 2
        };
        let (below, feet, head) = (w.get(x, yy - 1, z), w.get(x, yy, z), w.get(x, yy + 1, z));
        if is_solid(below) && !is_solid(feet) && !is_fluid(feet) && !is_solid(head) {
            return Some((Vec3::new(x as f32 + 0.5, yy as f32, z as f32 + 0.5), below));
        }
    }
    None
}

impl Mob {
    pub fn new(kind: MobKind, pos: Vec3, yaw: f32, seed: u32) -> Self {
        Self {
            id: 0,
            net_target: None,
            kind,
            pos,
            vel: Vec3::ZERO,
            body_yaw: yaw,
            head_yaw: yaw,
            pitch: 0.0,
            health: kind.max_health(),
            hurt_time: 0.0,
            death: None,
            fire: 0.0,
            sheared: false,
            on_ground: false,
            in_water: false,
            fall_peak: pos.y,
            limb_swing: 0.0,
            limb_amount: 0.0,
            target: None,
            target_time: 0.0,
            idle: 2.0,
            panic: 0.0,
            flee_from: None,
            stuck: 0.0,
            look: Look::Ahead,
            look_time: 0.0,
            still: 0.0,
            jump_cooldown: 0.0,
            damage_tick: 0.0,
            rng: Rng::new(seed),
        }
    }

    /// What LAN players need to draw this mob.
    pub fn to_net(&self) -> crate::net::MobNet {
        crate::net::MobNet {
            id: self.id,
            kind: self.kind as u8,
            pos: self.pos,
            body_yaw: self.body_yaw,
            head_yaw: self.head_yaw,
            pitch: self.pitch,
            limb_swing: self.limb_swing,
            limb_amount: self.limb_amount,
            hurt: self.hurt_time > 0.0,
            death: self.death.unwrap_or(-1.0),
            sheared: self.sheared,
        }
    }

    /// A LAN player's copy of a host mob.
    pub fn from_net(s: &crate::net::MobNet) -> Option<Mob> {
        let kind = MobKind::from_u8(s.kind)?;
        let mut m = Mob::new(kind, s.pos, s.body_yaw, 1);
        m.id = s.id;
        m.apply_net(s);
        m.follow(1.0);
        Some(m)
    }

    pub fn apply_net(&mut self, s: &crate::net::MobNet) {
        self.net_target = Some(*s);
        self.hurt_time = if s.hurt { HURT_TIME } else { 0.0 };
        self.death = (s.death >= 0.0).then_some(s.death);
        self.sheared = s.sheared;
    }

    /// Half width and height of its bounding box.
    pub fn size(&self) -> (f32, f32) {
        self.kind.size()
    }

    /// A sheep that still has its wool.
    pub fn can_shear(&self) -> bool {
        self.kind == MobKind::Sheep && !self.sheared && self.alive()
    }

    /// LAN player: glides toward the host's latest state (sent 20 times a second).
    pub fn follow(&mut self, dt: f32) {
        let Some(s) = self.net_target else {
            return;
        };
        let k = 1.0 - (-15.0 * dt).exp();
        self.pos = if self.pos.distance_squared(s.pos) > 16.0 {
            s.pos
        } else {
            self.pos.lerp(s.pos, k)
        };
        self.body_yaw += wrap_angle(s.body_yaw - self.body_yaw) * k;
        self.head_yaw += wrap_angle(s.head_yaw - self.head_yaw) * k;
        self.pitch += (s.pitch - self.pitch) * k;
        self.limb_swing += (s.limb_swing - self.limb_swing) * k;
        self.limb_amount += (s.limb_amount - self.limb_amount) * k;
        if let Some(d) = &mut self.death {
            *d += dt;
        }
    }

    fn rand(&mut self) -> f32 {
        self.rng.next()
    }

    pub fn alive(&self) -> bool {
        self.death.is_none()
    }

    pub fn center(&self) -> Vec3 {
        self.pos + Vec3::Y * (self.size().1 * 0.5)
    }

    /// Distance along the ray to the mob's bounding box, if it is hit.
    pub fn ray_hit(&self, origin: Vec3, dir: Vec3, max: f32) -> Option<f32> {
        if !self.alive() {
            return None;
        }
        // Minecraft picks entities with their box grown by 0.1.
        let (half_w, tall) = self.size();
        let grow = Vec3::new(half_w + 0.1, 0.1, half_w + 0.1);
        let top = Vec3::new(half_w + 0.1, tall + 0.1, half_w + 0.1);
        ray_box(origin, dir, self.pos - grow, self.pos + top, max)
    }

    /// Hit by something at `from`. Returns false if it could not be hurt right now.
    pub fn hurt(&mut self, amount: f32, from: Option<Vec3>, knockback: f32) -> bool {
        if !self.alive() || self.hurt_time > 0.0 {
            return false;
        }
        self.health -= amount;
        self.hurt_time = HURT_TIME;
        self.panic = 4.0 + self.rand() * 2.0;
        self.target = None;
        if let Some(src) = from {
            self.flee_from = Some(src);
            // Minecraft's knockback: pushed away, and up a bit (when on the ground).
            let away = (self.pos - src) * Vec3::new(1.0, 0.0, 1.0);
            let away = away.try_normalize().unwrap_or(Vec3::X);
            self.vel.x = self.vel.x * 0.5 + away.x * KNOCKBACK * knockback;
            self.vel.z = self.vel.z * 0.5 + away.z * KNOCKBACK * knockback;
            if self.on_ground || self.in_water {
                self.vel.y = self.vel.y.max(0.0) * 0.5 + KNOCKBACK_UP;
            }
        }
        if self.health <= 0.0 {
            self.health = 0.0;
            self.death = Some(0.0);
        }
        true
    }

    /// Environmental damage (no knockback).
    fn hurt_env(&mut self, amount: f32) {
        self.hurt(amount, None, 0.0);
    }

    /// Nudged by something overlapping it (other mobs, the player).
    pub fn push(&mut self, v: Vec3) {
        self.vel.x += v.x;
        self.vel.z += v.z;
    }

    pub fn overlaps(&self, p: Vec3, half_w: f32, tall: f32) -> Option<Vec3> {
        let d = self.pos - p;
        let (own_w, own_tall) = self.size();
        let reach = own_w + half_w;
        if d.x.abs() < reach
            && d.z.abs() < reach
            && self.pos.y < p.y + tall
            && p.y < self.pos.y + own_tall
        {
            let away = Vec3::new(d.x, 0.0, d.z);
            Some(away.try_normalize().unwrap_or(Vec3::X) * (reach - away.length()).max(0.0))
        } else {
            None
        }
    }

    // ------------------------------------------------------------------------ AI

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
    fn nearest_player(&self, ctx: &MobCtx, range: f32) -> Option<Vec3> {
        ctx.players
            .iter()
            .copied()
            .filter(|p| p.distance(self.pos) < range)
            .min_by(|a, b| a.distance(self.pos).total_cmp(&b.distance(self.pos)))
    }

    fn think(&mut self, dt: f32, w: &World, ctx: &MobCtx) -> Option<(Vec3, f32)> {
        // Looking: at a nearby player now and then, or around (Minecraft's LookAtPlayerGoal
        // and RandomLookAroundGoal, each started with a 2% chance per tick).
        self.look_time -= dt;
        let player_near = self.nearest_player(ctx, 6.0).filter(|_| self.panic <= 0.0);
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
        } else if self.target.is_none() {
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
        let target = self.target?;
        self.target_time -= dt;
        let to = Vec2::new(target.x - self.pos.x, target.z - self.pos.z);
        if to.length() < 0.4 || self.target_time <= 0.0 || self.stuck > 1.5 {
            self.target = None;
            self.idle = 1.0 + self.rand() * 4.0;
            return None;
        }
        let speed = if self.panic > 0.0 {
            PANIC_SPEED
        } else {
            WALK_SPEED
        };
        let dir = to.normalize();
        let dir3 = Vec3::new(dir.x, 0.0, dir.y);
        // Look before stepping: stop at cliffs, lava and (unless already swimming) water.
        let ahead = self.pos + dir3 * (self.size().0 + 0.35) + Vec3::Y * 0.5;
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
        if step_up {
            let (x, y, z) = (
                ahead.x.floor() as i32,
                self.pos.y.floor() as i32,
                ahead.z.floor() as i32,
            );
            let room = !is_solid(w.get(x, y + 1, z))
                && !is_solid(w.get(self.pos.x.floor() as i32, y + 2, self.pos.z.floor() as i32));
            if !room {
                // A wall: give up on this spot.
                self.target = None;
                self.idle = 0.5 + self.rand() * 2.0;
                return None;
            }
            if self.on_ground && self.jump_cooldown <= 0.0 {
                self.vel.y = JUMP;
                self.jump_cooldown = 0.4;
            }
        }
        Some((dir3, speed))
    }

    // ------------------------------------------------------------------------ physics

    fn move_axis(&mut self, w: &World, axis: usize, d: f32) -> bool {
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

    /// Advances the mob by `dt` seconds.
    pub fn update(&mut self, dt: f32, w: &World, ctx: &MobCtx) -> MobEvent {
        let dt = dt.min(0.05);
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
        let k = 1.0 - (-grip * dt).exp();
        self.vel.x += (want.x - self.vel.x) * k;
        self.vel.z += (want.z - self.vel.z) * k;

        if self.in_water {
            // Floats up to the surface and bobs there (Minecraft's FloatGoal).
            let target_up = if self.alive() { 2.0 } else { 0.5 };
            self.vel.y += (target_up - self.vel.y) * (1.0 - (-3.0 * dt).exp());
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
            self.pitch += (want_pitch.clamp(-0.8, 0.8) - self.pitch) * (1.0 - (-8.0 * dt).exp());
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
        self.limb_amount += (target - self.limb_amount) * (1.0 - (-10.0 * dt).exp());
        self.limb_swing += speed * dt * 4.0;

        // Sheep graze now and then (Minecraft's EatBlockGoal: 1 in 1000 per tick) on tall
        // grass they stand in or the grass block under them, and their wool grows back.
        if self.kind == MobKind::Sheep
            && self.alive()
            && self.on_ground
            && steer.is_none()
            && self.panic <= 0.0
            && self.rand() < per_tick(1.0 / 1000.0, dt)
        {
            let feet = self.pos.floor().as_ivec3();
            let eat = if w.geti(feet) == TALL_GRASS {
                Some(feet)
            } else {
                (w.geti(feet - glam::IVec3::Y) == GRASS).then(|| feet - glam::IVec3::Y)
            };
            if let Some(p) = eat {
                self.sheared = false;
                return MobEvent::EatGrass(p);
            }
        }
        MobEvent::None
    }

    // ------------------------------------------------------------------------ model

    pub fn build(&self, out: &mut Vec<Vertex>, sky: u8, blk: u8) {
        let light = vertex_light(sky, blk);
        let tint = if self.hurt_time > 0.0 || self.death.is_some() {
            [255, 110, 110]
        } else {
            [255, 255, 255]
        };
        // Dying: tips over onto its side (Minecraft's LivingEntityRenderer flip).
        let flip = self
            .death
            .map(|t| (t * 1.6).sqrt().min(1.0) * FRAC_PI_2)
            .unwrap_or(0.0);
        let root = Mat4::from_translation(self.pos)
            * Mat4::from_rotation_y(-self.body_yaw - FRAC_PI_2)
            * Mat4::from_rotation_z(flip)
            * Mat4::from_scale(Vec3::splat(1.0 / 16.0));
        match self.kind {
            MobKind::Pig => self.build_pig(out, root, tint, light),
            MobKind::Sheep => self.build_sheep(out, root, tint, light),
        }
    }

    /// Minecraft's `SheepModel` (a `QuadrupedModel` with leg height 12) and, unless sheared,
    /// `SheepFurModel` over it: the same parts grown a little, with the wool texture.
    fn build_sheep(&self, out: &mut Vec<Vertex>, root: Mat4, tint: [u8; 3], light: [u8; 4]) {
        let part = |px: f32, py: f32, pz: f32, rot: Mat4| {
            root * Mat4::from_translation(Vec3::new(-px, 24.0 - py, pz))
                * rot
                * Mat4::from_scale(Vec3::new(-1.0, -1.0, 1.0))
        };
        let head_yaw = wrap_angle(self.head_yaw - self.body_yaw).clamp(-HEAD_LIMIT, HEAD_LIMIT);
        let head = part(
            0.0,
            6.0,
            -8.0,
            Mat4::from_rotation_y(-head_yaw) * Mat4::from_rotation_x(self.pitch),
        );
        let body = part(0.0, 5.0, 2.0, Mat4::from_rotation_x(-FRAC_PI_2));
        let ls = self.limb_swing * 0.6662;
        let la = self.limb_amount;
        let legs = [
            (-3.0, 7.0, ls.cos()),
            (3.0, 7.0, (ls + PI).cos()),
            (-3.0, -5.0, (ls + PI).cos()),
            (3.0, -5.0, ls.cos()),
        ]
        .map(|(x, z, swing)| part(x, 12.0, z, Mat4::from_rotation_x(-swing * 1.4 * la)));

        let skin = tex::SHEEP;
        emit_cube(out, head, [-3.0, -4.0, -6.0], [6.0, 6.0, 8.0], [0.0, 0.0], 0.0, skin, tint, light);
        emit_cube(out, body, [-4.0, -10.0, -7.0], [8.0, 16.0, 6.0], [28.0, 8.0], 0.0, skin, tint, light);
        for leg in legs {
            emit_cube(out, leg, [-2.0, 0.0, -2.0], [4.0, 12.0, 4.0], [0.0, 16.0], 0.0, skin, tint, light);
        }
        if self.sheared {
            return;
        }
        let wool = tex::SHEEP_WOOL;
        emit_cube(out, head, [-3.0, -4.0, -4.0], [6.0, 6.0, 6.0], [0.0, 0.0], 0.6, wool, tint, light);
        emit_cube(out, body, [-4.0, -10.0, -7.0], [8.0, 16.0, 6.0], [28.0, 8.0], 1.75, wool, tint, light);
        for leg in legs {
            emit_cube(out, leg, [-2.0, 0.0, -2.0], [4.0, 6.0, 4.0], [0.0, 16.0], 0.5, wool, tint, light);
        }
    }

    /// Minecraft's `PigModel` (a `QuadrupedModel` with leg height 6), in model pixels.
    fn build_pig(&self, out: &mut Vec<Vertex>, root: Mat4, tint: [u8; 3], light: [u8; 4]) {
        let layer = tex::PIG;
        // A part's pivot, given in Minecraft's model coordinates (Y down from 24 = the ground,
        // X mirrored), then its rotation in ours.
        let part = |px: f32, py: f32, pz: f32, rot: Mat4| {
            root * Mat4::from_translation(Vec3::new(-px, 24.0 - py, pz))
                * rot
                * Mat4::from_scale(Vec3::new(-1.0, -1.0, 1.0))
        };
        let cube = |out: &mut Vec<Vertex>, m: Mat4, o: [f32; 3], s: [f32; 3], uv: [f32; 2]| {
            emit_cube(out, m, o, s, uv, 0.0, layer, tint, light);
        };

        let head_yaw = wrap_angle(self.head_yaw - self.body_yaw).clamp(-HEAD_LIMIT, HEAD_LIMIT);
        let head = part(
            0.0,
            12.0,
            -6.0,
            Mat4::from_rotation_y(-head_yaw) * Mat4::from_rotation_x(self.pitch),
        );
        cube(out, head, [-4.0, -4.0, -8.0], [8.0, 8.0, 8.0], [0.0, 0.0]);
        cube(out, head, [-2.0, 0.0, -9.0], [4.0, 3.0, 1.0], [16.0, 16.0]);

        let body = part(0.0, 11.0, 2.0, Mat4::from_rotation_x(-FRAC_PI_2));
        cube(
            out,
            body,
            [-5.0, -10.0, -7.0],
            [10.0, 16.0, 8.0],
            [28.0, 8.0],
        );

        let ls = self.limb_swing * 0.6662;
        let la = self.limb_amount;
        let legs = [
            (-3.0, 7.0, ls.cos()),
            (3.0, 7.0, (ls + PI).cos()),
            (-3.0, -5.0, (ls + PI).cos()),
            (3.0, -5.0, ls.cos()),
        ];
        for (x, z, swing) in legs {
            let leg = part(x, 18.0, z, Mat4::from_rotation_x(-swing * 1.4 * la));
            cube(out, leg, [-2.0, 0.0, -2.0], [4.0, 6.0, 4.0], [0.0, 16.0]);
        }
    }
}

/// One cube of a Minecraft entity model: origin and size in model pixels, `uv` the texture
/// offset of its box UV layout in a 64x64 atlas (Minecraft's `ModelPart.Cube`). `grow` makes
/// it bigger on every side without changing its texture (Minecraft's `CubeDeformation`).
#[allow(clippy::too_many_arguments)]
fn emit_cube(
    out: &mut Vec<Vertex>,
    m: Mat4,
    o: [f32; 3],
    s: [f32; 3],
    uv: [f32; 2],
    grow: f32,
    layer: u32,
    tint: [u8; 3],
    light: [u8; 4],
) {
    let (x0, y0, z0) = (o[0] - grow, o[1] - grow, o[2] - grow);
    let (x1, y1, z1) = (
        o[0] + s[0] + grow,
        o[1] + s[1] + grow,
        o[2] + s[2] + grow,
    );
    let (dx, dy, dz) = (s[0], s[1], s[2]);
    let v = [
        Vec3::new(x0, y0, z0),
        Vec3::new(x1, y0, z0),
        Vec3::new(x1, y1, z0),
        Vec3::new(x0, y1, z0),
        Vec3::new(x0, y0, z1),
        Vec3::new(x1, y0, z1),
        Vec3::new(x1, y1, z1),
        Vec3::new(x0, y1, z1),
    ];
    let (u, w) = (uv[0], uv[1]);
    let (u0, u1, u2, u3, u4, u5) = (
        u,
        u + dz,
        u + dz + dx,
        u + dz + dx + dx,
        u + dz + dx + dz,
        u + dz + dx + dz + dx,
    );
    let (w0, w1, w2) = (w, w + dz, w + dz + dy);
    // (corners, u1, v1, u2, v2) per face, as in ModelPart.Cube.
    let faces: [([usize; 4], f32, f32, f32, f32); 6] = [
        ([5, 4, 0, 1], u1, w0, u2, w1),
        ([2, 3, 7, 6], u2, w1, u3, w0),
        ([0, 4, 7, 3], u0, w1, u1, w2),
        ([1, 0, 3, 2], u1, w1, u2, w2),
        ([5, 1, 2, 6], u2, w1, u4, w2),
        ([4, 5, 6, 7], u4, w1, u5, w2),
    ];
    let center = m.transform_point3((v[0] + v[6]) * 0.5);
    let k = 1.0 / 64.0;
    for (idx, ua, va, ub, vb) in faces {
        let uvs = [[ub, va], [ua, va], [ua, vb], [ub, vb]];
        let p: [Vec3; 4] = std::array::from_fn(|i| m.transform_point3(v[idx[i]]));
        let quad: [Vertex; 4] = std::array::from_fn(|i| Vertex {
            pos: p[i].to_array(),
            uv: [uvs[i][0] * k, uvs[i][1] * k],
            layer: layer as f32,
            light,
            tint: [tint[0], tint[1], tint[2], flags::ENTITY],
        });
        // Counter-clockwise seen from outside, like the rest of the game's geometry.
        let n = (p[1] - p[0]).cross(p[2] - p[0]);
        let face_center = (p[0] + p[2]) * 0.5;
        if n.dot(face_center - center) >= 0.0 {
            out.extend_from_slice(&[quad[0], quad[1], quad[2], quad[0], quad[2], quad[3]]);
        } else {
            out.extend_from_slice(&[quad[0], quad[2], quad[1], quad[0], quad[3], quad[2]]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pig_model_has_all_cubes() {
        let pig = Mob::new(MobKind::Pig, Vec3::ZERO, 0.0, 7);
        let mut out = Vec::new();
        pig.build(&mut out, 15, 0);
        // Head, snout, body and four legs, 6 faces of 2 triangles each.
        assert_eq!(out.len(), 7 * 6 * 6);
        // It stands on the ground and is about a block tall and long.
        let min_y = out.iter().map(|v| v.pos[1]).fold(f32::MAX, f32::min);
        let max_y = out.iter().map(|v| v.pos[1]).fold(f32::MIN, f32::max);
        assert!(min_y.abs() < 1e-4, "feet at {min_y}");
        assert!((max_y - 1.0).abs() < 1e-4, "top at {max_y}");
        // Facing +X at yaw 0: the snout is the furthest point forward.
        let max_x = out.iter().map(|v| v.pos[0]).fold(f32::MIN, f32::max);
        assert!((max_x - 15.0 / 16.0).abs() < 1e-4, "snout at {max_x}");
    }

    #[test]
    fn sheep_wears_its_wool_until_sheared() {
        let mut sheep = Mob::new(MobKind::Sheep, Vec3::ZERO, 0.0, 7);
        let mut out = Vec::new();
        sheep.build(&mut out, 15, 0);
        // Head, body and four legs, each with a wool cube over it.
        assert_eq!(out.len(), 12 * 6 * 6);
        let max_y = out.iter().map(|v| v.pos[1]).fold(f32::MIN, f32::max);
        assert!(max_y > 1.3 && max_y < 1.5, "top at {max_y}");
        assert!(sheep.can_shear());
        sheep.sheared = true;
        out.clear();
        sheep.build(&mut out, 15, 0);
        assert_eq!(out.len(), 6 * 6 * 6);
        assert!(!sheep.can_shear());
        let net = sheep.to_net();
        assert!(Mob::from_net(&net).is_some_and(|m| m.sheared && m.kind == MobKind::Sheep));
    }

    #[test]
    fn knockback_and_death() {
        let mut pig = Mob::new(MobKind::Pig, Vec3::ZERO, 0.0, 7);
        pig.on_ground = true;
        assert!(pig.hurt(4.0, Some(Vec3::new(-1.0, 0.0, 0.0)), 1.0));
        assert!(pig.vel.x > 0.0 && pig.vel.y > 0.0);
        // Invulnerable right after a hit.
        assert!(!pig.hurt(4.0, None, 0.0));
        pig.hurt_time = 0.0;
        assert!(pig.hurt(8.0, None, 0.0));
        assert!(!pig.alive());
    }
}
