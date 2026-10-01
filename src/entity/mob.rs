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
//! (`textures::skin_pages`). Each mob's model is in its file; the pieces they share are here.

use crate::content::mobs::{MobDef, MobState};
use crate::textures::skin_pages::{face_uv, SkinPages};
use crate::model::prim::{quad_at, Paint, Sides};
use crate::util::{ray_box, vertex_light, wrap_angle, Rng};
use crate::world::mesh::{flags, Vertex};
use crate::world::*;
use glam::{Mat4, Vec2, Vec3};
use std::f32::consts::{FRAC_PI_2, PI, TAU};

pub use crate::content::mobs::MobKind;
// (the skins' texture layers are laid out by `textures`)
pub use crate::content::mobs::pig::skin as pig_skin;
pub use crate::content::mobs::sheep::skin as sheep_skin;
pub use crate::content::mobs::wolf::skin as wolf_skin;

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

    /// A player's copy: glides toward the server's latest state (sent 20 times a second).
    pub fn follow(&mut self, dt: f32) {
        let Some(s) = self.net_target else {
            return;
        };
        let k = crate::util::damp(15.0, dt);
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
    fn think(&mut self, dt: f32, w: &World, ctx: &MobCtx) -> Steer {
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
    fn step_room(&self, w: &World, ahead: Vec3, tall: f32) -> bool {
        let y = self.pos.y.floor();
        // The highest block its head is in, standing on the step.
        let top = (y + 1.0 + tall - 1e-3).floor() as i32;
        let (ax, az) = (ahead.x.floor() as i32, ahead.z.floor() as i32);
        let (x, z) = (self.pos.x.floor() as i32, self.pos.z.floor() as i32);
        let head_now = (self.pos.y + tall - 1e-3).floor() as i32;
        (y as i32 + 1..=top).all(|yy| !is_solid(w.get(ax, yy, az)))
            && (head_now..=top).all(|yy| !is_solid(w.get(x, yy, z)))
    }

    // ------------------------------------------------------------------------ physics

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

    // ------------------------------------------------------------------------ model

    pub(crate) fn pose(&self) -> MobPose {
        MobPose {
            pos: self.pos,
            body_yaw: self.body_yaw,
            head_yaw: self.head_yaw,
            pitch: self.pitch,
            limb_swing: self.limb_swing,
            limb_amount: self.limb_amount,
            death: self.death,
        }
    }

    /// Its model into `out`, lit by this sky and block light.
    pub fn build(&self, out: &mut Vec<Vertex>, sky: u8, blk: u8) {
        (self.def().model)(self, out, vertex_light(sky, blk));
    }

    /// What its model is tinted with: red while hurt or dying.
    pub(crate) fn tint(&self) -> [u8; 3] {
        if self.hurt_time > 0.0 || self.death.is_some() {
            [255, 110, 110]
        } else {
            [255, 255, 255]
        }
    }
}

/// An animal's model space: at its feet, turned its way, tipped over onto its side while it
/// dies (Minecraft's LivingEntityRenderer flip), in model pixels.
pub(crate) fn animal_root(p: &MobPose) -> Mat4 {
    let flip = p
        .death
        .map(|t| (t * 1.6).sqrt().min(1.0) * FRAC_PI_2)
        .unwrap_or(0.0);
    Mat4::from_translation(p.pos)
        * Mat4::from_rotation_y(-p.body_yaw - FRAC_PI_2)
        * Mat4::from_rotation_z(flip)
        * Mat4::from_scale(Vec3::splat(1.0 / 16.0))
}

/// A part's pivot, given in Minecraft's model coordinates (Y down from 24 = the ground,
/// X mirrored), then its rotation in ours.
pub(crate) fn part(root: Mat4, px: f32, py: f32, pz: f32, rot: Mat4) -> Mat4 {
    root * Mat4::from_translation(Vec3::new(-px, 24.0 - py, pz)) * rot * Mat4::from_scale(Vec3::new(-1.0, -1.0, 1.0))
}

/// The head's turn: where it looks relative to the body (within `HEAD_LIMIT`), and up or down.
pub(crate) fn head_turn(p: &MobPose) -> Mat4 {
    let head_yaw = wrap_angle(p.head_yaw - p.body_yaw).clamp(-HEAD_LIMIT, HEAD_LIMIT);
    Mat4::from_rotation_y(-head_yaw) * Mat4::from_rotation_x(p.pitch)
}

/// A `QuadrupedModel`'s four legs (Minecraft's pig and sheep) with their pivots at height
/// `py`, swinging as it walks.
pub(crate) fn quadruped_legs(root: Mat4, p: &MobPose, py: f32) -> [Mat4; 4] {
    let ls = p.limb_swing * 0.6662;
    let la = p.limb_amount;
    [
        (-3.0, 7.0, ls.cos()),
        (3.0, 7.0, (ls + PI).cos()),
        (-3.0, -5.0, (ls + PI).cos()),
        (3.0, -5.0, ls.cos()),
    ]
    .map(|(x, z, swing)| part(root, x, py, z, Mat4::from_rotation_x(-swing * 1.4 * la)))
}

/// One cube of a Minecraft entity model (Minecraft's `ModelPart.Cube`): box `b` of a skin
/// (its size and box UV), at `o` in model pixels, textured from the skin's pages from layer
/// `base`. `grow` makes it bigger on every side without changing its texture (Minecraft's
/// `CubeDeformation`).
#[allow(clippy::too_many_arguments)]
pub(crate) fn emit_paged(
    out: &mut Vec<Vertex>,
    m: Mat4,
    o: [f32; 3],
    skin: &SkinPages,
    b: usize,
    grow: f32,
    base: u32,
    tint: [u8; 3],
    light: [u8; 4],
) {
    let (uv, s) = skin.boxes[b];
    let (x0, y0, z0) = (o[0] - grow, o[1] - grow, o[2] - grow);
    let (x1, y1, z1) = (o[0] + s[0] + grow, o[1] + s[1] + grow, o[2] + s[2] + grow);
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
    // Corners of each face, as in ModelPart.Cube.
    let corners: [[usize; 4]; 6] = [[5, 4, 0, 1], [2, 3, 7, 6], [0, 4, 7, 3], [1, 0, 3, 2], [5, 1, 2, 6], [4, 5, 6, 7]];
    let center = m.transform_point3((v[0] + v[6]) * 0.5);
    for (f, idx) in corners.into_iter().enumerate() {
        // The corners run (ub, va), (ua, va), (ua, vb), (ub, vb) over the texture.
        let (ua, va, ub, vb) = face_uv(uv, s, f);
        let p: [Vec3; 4] = std::array::from_fn(|i| m.transform_point3(v[idx[i]]));
        // A point of the face by its texture point (the face is a rectangle).
        let at = |u: f32, w: f32| {
            let su = if ub != ua { (u - ub) / (ua - ub) } else { 0.0 };
            let sv = if vb != va { (w - va) / (vb - va) } else { 0.0 };
            p[0] + (p[1] - p[0]) * su + (p[3] - p[0]) * sv
        };
        // Counter-clockwise seen from outside, like the rest of the game's geometry.
        let n = (p[1] - p[0]).cross(p[2] - p[0]);
        let outward = n.dot((p[0] + p[2]) * 0.5 - center) >= 0.0;
        for piece in skin.pieces(b, f) {
            let (l, t, r, bt) = piece.rect;
            let (pa, pb) = if ua <= ub { (l, r) } else { (r, l) };
            let (qa, qb) = if va <= vb { (t, bt) } else { (bt, t) };
            let uvs = [[pb, qa], [pa, qa], [pa, qb], [pb, qb]];
            let paint = Paint { layer: base + piece.page, light, face: light[3], tint, fl: flags::ENTITY };
            let sides = if outward { Sides::Front } else { Sides::Back };
            quad_at(out, uvs.map(|[u, v]| at(u, v)), uvs.map(|[u, v]| skin.page_uv(piece, u, v)), &paint, sides);
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::content::mobs::{pig::PIG, sheep::SHEEP};

    /// A chunk with a stone floor at y 63 (the ground at 64), air above.
    pub(crate) fn flat_world() -> World {
        let mut world = World::new();
        let mut chunk = ChunkData::new();
        for x in 0..16 {
            for z in 0..16 {
                chunk.set(x, 63, z, STONE);
            }
        }
        world.chunks.insert((0, 0), std::sync::Arc::new(chunk));
        world
    }

    #[test]
    fn knockback_and_death() {
        let mut pig = Mob::new(PIG, Vec3::ZERO, 0.0, 7);
        pig.on_ground = true;
        assert!(pig.hurt(4.0, Some(Vec3::new(-1.0, 0.0, 0.0)), 1.0));
        assert!(pig.vel.x > 0.0 && pig.vel.y > 0.0);
        // Invulnerable right after a hit.
        assert!(!pig.hurt(4.0, None, 0.0));
        pig.hurt_time = 0.0;
        assert!(pig.hurt(8.0, None, 0.0));
        assert!(!pig.alive() && !pig.burnt);
    }

    #[test]
    fn dying_on_fire_is_remembered() {
        // (the fire goes out during the death animation: what counts is how it died)
        let mut pig = Mob::new(PIG, Vec3::ZERO, 0.0, 7);
        pig.fire = 0.2;
        assert!(pig.hurt(20.0, None, 0.0));
        pig.fire = 0.0;
        assert!(pig.burnt);
    }

    #[test]
    fn every_mob_gets_its_own_random_numbers() {
        let mut a = Mob::new(PIG, Vec3::ZERO, 0.0, 1);
        let mut b = Mob::new(PIG, Vec3::ZERO, 0.0, 2);
        assert_ne!(a.rand(), b.rand());
        let net = Mob::new(SHEEP, Vec3::ZERO, 0.0, 41).to_net(None);
        let (mut c, mut d) = (Mob::from_net(&net).unwrap(), Mob::new(SHEEP, Vec3::ZERO, 0.0, 41));
        assert_eq!(c.rand(), d.rand());
    }

    #[test]
    fn health_reaches_the_players() {
        let mut pig = Mob::new(PIG, Vec3::ZERO, 0.0, 7);
        pig.hurt(3.0, None, 0.0);
        let copy = Mob::from_net(&pig.to_net(None)).unwrap();
        assert_eq!(copy.health, 7.0);
    }

    #[test]
    fn steps_up_only_with_room_for_its_height() {
        let mut w = flat_world();
        // A step at x = 2, and a sheep (1.3 tall) walking at it.
        w.set(2, 64, 0, STONE);
        let sheep = Mob::new(SHEEP, Vec3::new(1.5, 64.0, 0.5), 0.0, 3);
        let ahead = sheep.pos + Vec3::X * (0.45 + 0.35) + Vec3::Y * 0.5;
        assert!(sheep.step_room(&w, ahead, 1.3));
        // A block two up over the step: a pig fits under it, the sheep does not.
        w.set(2, 66, 0, STONE);
        assert!(!sheep.step_room(&w, ahead, 1.3));
        assert!(sheep.step_room(&w, ahead, 0.9));
        // A block right over its own head: no jumping.
        w.set(2, 66, 0, AIR);
        w.set(1, 65, 0, STONE);
        assert!(!sheep.step_room(&w, ahead, 1.3));
        assert!(!sheep.step_room(&w, ahead, 0.9));
    }
}
