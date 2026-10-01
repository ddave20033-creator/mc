//! Mobs: their physics and the AI every mob runs. What a kind of mob is and does comes from
//! its line of the table (`content::mobs`): its size, speed and health, its behaviour
//! template's parameters (`Ai`), and its own hooks, which the AI asks first.
//!
//! The AI follows Minecraft's goals for animals: panic after being hurt (run to random spots,
//! away from whoever hit it), wander to random nearby spots (preferring grass, avoiding
//! water, lava and drops of more than 3 blocks), look at a nearby player, look around, and
//! float in water. A neutral mob goes for whoever hurt it (a hostile one for any player near
//! it) and attacks when near. Mobs jump up single blocks, take fall, lava, cactus and
//! suffocation damage, get knocked back, flash red when hurt and tip over when they die.
//!
//! Models use Minecraft's entity model format: cubes with box UVs into a 64 unit wide atlas;
//! the atlases are drawn at 8 texels per unit, their faces spread over several texture layers
//! (`textures::skin_pages`). Each mob's model is in its file; the pieces they share are in
//! `model`.
//!
//! Here: a mob's state, being hurt and pushed, and what goes over the network; its AI in `ai`,
//! its moving through the world in `physics`.

use crate::content::mobs::{MobDef, MobKind, MobState};
use crate::textures::skin_pages::{face_uv, SkinPages};
use crate::model::prim::{quad_at, Paint, Sides};
use crate::util::{ray_box, vertex_light, wrap_angle, Rng};
use crate::world::mesh::{flags, Vertex};
use crate::world::*;
use glam::{Mat4, Vec2, Vec3};
use std::f32::consts::{FRAC_PI_2, PI, TAU};

mod ai;
pub(crate) mod model;
mod physics;
#[cfg(test)]
pub(crate) mod tests;


const GRAVITY: f32 = 32.0;
/// Jump speed: clears one block (Minecraft: 0.42 blocks per tick).
const JUMP: f32 = 8.6;
/// Minecraft's PanicGoal runs at 1.25x the walking speed.
const PANIC_SPEED: f32 = 1.25;
/// Knockback speeds (horizontal, up) in blocks per second, tuned so a normal hit moves a
/// mob about 1.5 blocks like in Minecraft (the wiki measures 1.552 blocks).
const KNOCKBACK: f32 = 5.0;
const KNOCKBACK_UP: f32 = 6.0;
/// Seconds a hit mob stays red and cannot be hurt again.
pub const HURT_TIME: f32 = 0.5;
/// Seconds of the death animation (tipping over) before it disappears.
pub const DEATH_TIME: f32 = 1.0;
/// Head turn limit relative to the body.
pub const HEAD_LIMIT: f32 = 75.0 * PI / 180.0;
/// How far a mob follows someone it goes for before it lets them go.
const CHASE_RANGE: f32 = 24.0;

/// Someone a mob goes for: a mob (its id) or a player (their id).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Foe {
    Mob(u32),
    Player(u8),
}

/// `MobNet::flags` every mob has (the kinds' own are above them, see `wolf::flags`).
pub mod mob_flags {
    /// Going for someone.
    pub const ANGRY: u8 = 1;
}

/// What the world around a mob looks like to its AI.
pub struct MobCtx {
    /// Feet of every living player.
    pub players: Vec<Vec3>,
    /// The same with who they are: their id and name.
    pub people: Vec<(u8, String, Vec3)>,
    /// Where every living mob is (its id and feet), for one going after another.
    pub mobs: Vec<(u32, Vec3)>,
}

/// Something that happened to a mob during an update.
pub enum MobEvent {
    None,
    /// The death animation is over: remove it and drop its loot.
    Remove,
    /// A sheep ate the grass here (a grass block turns to dirt, tall grass goes).
    EatGrass(glam::IVec3),
    /// It attacked someone (its `MobDef::attack`, from where it is).
    Attack(Foe),
}

/// What the AI decided for this step: None to stand, or a direction and a speed.
pub type Steer = Option<(Vec3, f32)>;

/// What a mob's model is drawn from: where it is, how it is turned, its legs' swing and how
/// long it has been dying.
#[derive(Clone, Copy, Default)]
pub struct MobPose {
    pub pos: Vec3,
    pub body_yaw: f32,
    pub head_yaw: f32,
    pub pitch: f32,
    pub limb_swing: f32,
    pub limb_amount: f32,
    pub death: Option<f32>,
}

pub struct Mob {
    /// Unique id (the server's).
    pub id: u32,
    /// A player's copy: the latest state from the server, which the mob glides toward.
    net_target: Option<crate::net::MobNet>,
    /// The glide to it (an even pace from one state sent to the next).
    glide: crate::util::Glide,
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
    /// Seconds on fire left (from lava), and whether it died burning (its meat drops cooked).
    pub fire: f32,
    pub burnt: bool,
    pub(crate) on_ground: bool,
    pub(crate) in_water: bool,
    /// Highest point since leaving the ground, for fall damage.
    pub(crate) fall_peak: f32,
    pub(crate) limb_swing: f32,
    pub(crate) limb_amount: f32,
    /// Where it is walking to, and for how long it keeps trying.
    pub(crate) target: Option<Vec3>,
    pub(crate) target_time: f32,
    /// Seconds to stand around before the next stroll.
    pub(crate) idle: f32,
    /// Seconds of panic left (after being hurt).
    pub(crate) panic: f32,
    /// Where the last hit came from (runs away from it).
    flee_from: Option<Vec3>,
    /// Seconds without making progress toward the target.
    stuck: f32,
    pub(crate) look: Look,
    pub(crate) look_time: f32,
    /// Standing still: seconds, and the body slowly turns to where the head looks.
    still: f32,
    jump_cooldown: f32,
    damage_tick: f32,
    rng: Rng,
    /// Whom it goes for, for how much longer (not a pet: it stays on them), and when it may
    /// attack again.
    pub foe: Option<Foe>,
    pub(crate) anger: f32,
    attack_cooldown: f32,
    /// A player's copy: going for someone (the server's has its `foe`).
    shown_angry: bool,
    /// Walking somewhere at a speed of its own (running after something).
    pub(crate) hurry: Option<f32>,
    /// An attack to report from this update.
    attack: Option<Foe>,
    /// When it may make its next sound, and whether this hurt was already sounded.
    sound_wait: f32,
    yelped: bool,
    /// Its kind's own state.
    pub state: MobState,
}

#[derive(Clone, Copy)]
pub(crate) enum Look {
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
    pred: impl Fn(Block) -> bool,
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
pub(crate) fn turn(cur: f32, want: f32, step: f32) -> f32 {
    cur + wrap_angle(want - cur).clamp(-step, step)
}

/// Chance that something with probability `p` per game tick (1/20 s) happens within `dt`.
pub(crate) fn per_tick(p: f32, dt: f32) -> f32 {
    1.0 - (1.0 - p).powf(dt * 20.0)
}

/// A spot a mob can stand on in column (x, z) near height `y`: solid ground below, room for
/// its body. Returns the feet position and the ground block.
pub fn standable(w: &World, x: i32, z: i32, y: i32, range: i32) -> Option<(Vec3, Block)> {
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

/// A mob's random numbers start from its id (every mob different, the same one the same).
fn seed_of(id: u32) -> u32 {
    id.wrapping_mul(0x9E37_79B9) ^ 0x5bd1_e995
}

impl Mob {
    /// A new mob of `kind` with the id `id` (its random numbers follow from it).
    pub fn new(kind: MobKind, pos: Vec3, yaw: f32, id: u32) -> Self {
        let def = kind.def();
        let seed = seed_of(id);
        Self {
            id,
            net_target: None,
            glide: Default::default(),
            kind,
            pos,
            vel: Vec3::ZERO,
            body_yaw: yaw,
            head_yaw: yaw,
            pitch: 0.0,
            health: def.health,
            hurt_time: 0.0,
            death: None,
            fire: 0.0,
            burnt: false,
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
            foe: None,
            anger: 0.0,
            attack_cooldown: 0.0,
            shown_angry: false,
            hurry: None,
            attack: None,
            sound_wait: 3.0 + (seed % 97) as f32 * 0.05,
            yelped: false,
            state: (def.state)(),
        }
    }

    /// Gives it another id (a loaded mob), and the random numbers that go with it.
    pub fn set_id(&mut self, id: u32) {
        self.id = id;
        self.rng = Rng::new(seed_of(id));
    }

    /// Its line of the table.
    pub fn def(&self) -> &'static MobDef {
        self.kind.def()
    }

    /// Its greatest health (a tame wolf's is more).
    pub fn max_health(&self) -> f32 {
        self.state.max_health().unwrap_or(self.def().health)
    }

    /// The name of the player whose pet it is.
    pub fn owner(&self) -> Option<&str> {
        match &self.state {
            MobState::Wolf(p) => p.owner.as_deref(),
            _ => None,
        }
    }

    /// A pet told to sit: it stays where it is and goes for no one.
    pub fn sitting(&self) -> bool {
        matches!(&self.state, MobState::Wolf(p) if p.sitting)
    }

    /// Going for someone (a player's copy: as the server said).
    pub fn angry(&self) -> bool {
        self.foe.is_some() || self.shown_angry
    }

    /// Attacked by `foe` (named `foe_name`, a player): one that fights back goes for them,
    /// unless it sits or they are its owner.
    pub fn provoke(&mut self, foe: Foe, foe_name: Option<&str>) {
        let ai = self.def().ai;
        if !ai.retaliates || !self.alive() || self.sitting() {
            return;
        }
        if foe_name.is_some() && foe_name == self.owner() {
            return;
        }
        self.foe = Some(foe);
        self.anger = ai.anger;
    }

    /// The sound it makes now, if any: its hurt sound once when hit, and now and then
    /// whatever its `Sounds::idle` picks.
    pub fn sound(&mut self, dt: f32) -> Option<crate::audio::Sound> {
        let sounds = self.def().sounds;
        if self.hurt_time > 0.0 && !self.yelped && self.alive() {
            self.yelped = true;
            if sounds.hurt.is_some() {
                return sounds.hurt;
            }
        }
        if self.hurt_time <= 0.0 {
            self.yelped = false;
        }
        let idle = sounds.idle?;
        self.sound_wait -= dt;
        if self.sound_wait > 0.0 || !self.alive() {
            return None;
        }
        let (least, most) = sounds.every;
        self.sound_wait = least + self.rand() * (most - least);
        idle(self)
    }

    /// What a player (named `to`) needs to draw this mob.
    pub fn to_net(&self, to: Option<&str>) -> crate::net::MobNet {
        let mut n = crate::net::MobNet {
            id: self.id,
            kind: self.kind.0,
            pos: self.pos,
            body_yaw: self.body_yaw,
            head_yaw: self.head_yaw,
            pitch: self.pitch,
            limb_swing: self.limb_swing,
            limb_amount: self.limb_amount,
            hurt: self.hurt_time > 0.0,
            death: self.death.unwrap_or(-1.0),
            health: self.health,
            sheared: false,
            taken: 0.0,
            last_hit: 0.0,
            flags: if self.foe.is_some() { mob_flags::ANGRY } else { 0 },
            collar: 0,
        };
        self.state.to_net(&mut n, to);
        n
    }

    /// A player's copy of a server's mob.
    pub fn from_net(s: &crate::net::MobNet) -> Option<Mob> {
        let kind = MobKind::from_u8(s.kind)?;
        let mut m = Mob::new(kind, s.pos, s.body_yaw, s.id);
        m.apply_net(s);
        m.follow(1.0);
        Some(m)
    }

    pub fn apply_net(&mut self, s: &crate::net::MobNet) {
        self.net_target = Some(*s);
        self.glide.restart();
        self.hurt_time = if s.hurt { HURT_TIME } else { 0.0 };
        self.death = (s.death >= 0.0).then_some(s.death);
        self.health = s.health;
        self.shown_angry = s.flags & mob_flags::ANGRY != 0;
        self.state.apply_net(s);
    }

    /// Half width and height of its bounding box.
    pub fn size(&self) -> (f32, f32) {
        self.def().size
    }

    /// A player's copy: glides toward the server's latest state (sent 20 times a second) at
    /// an even pace (`Glide`).
    pub fn follow(&mut self, dt: f32) {
        let Some(s) = self.net_target else {
            return;
        };
        let k = self.glide.step(dt);
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
        if let MobState::Dummy(t) = &mut self.state {
            t.follow(self.limb_swing, self.limb_amount, dt);
        }
        if let Some(d) = &mut self.death {
            *d += dt;
        }
    }

    pub(crate) fn rand(&mut self) -> f32 {
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
        if let Some(hurt) = self.def().hooks.hurt {
            return hurt(self, amount, from, knockback);
        }
        if !self.alive() || self.hurt_time > 0.0 {
            return false;
        }
        self.health -= amount;
        self.hurt_time = HURT_TIME;
        // (one that fights back does not run away: it turns on whoever hurt it, `provoke`)
        if self.def().ai.panics {
            self.panic = 4.0 + self.rand() * 2.0;
            self.target = None;
            if from.is_some() {
                self.flee_from = from;
            }
        }
        if let Some(src) = from {
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
            self.burnt = self.fire > 0.0;
        }
        true
    }

    /// Environmental damage (no knockback).
    fn hurt_env(&mut self, amount: f32) {
        self.hurt(amount, None, 0.0);
    }

    /// Nudged by something overlapping it (other mobs, the player).
    pub fn push(&mut self, v: Vec3) {
        if !self.def().ai.moves {
            return;
        }
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
}
