//! Mobs (the pig, the sheep and the wolf): physics, a Minecraft-style animal AI and the
//! models. The target dummy is one too: it stands where it was set up, rocks when hit and
//! counts the damage it takes (it never dies).
//!
//! The wolf is Minecraft's: wild ones roam the forests in packs and turn on whoever hurts
//! one of them; given a bone, one may be tamed (a third of the time). A tame wolf wears a
//! collar, follows its owner (catching up by jumping to them when far behind), sits and
//! stands up again when its owner right clicks it, and goes for whatever its owner attacks
//! or whoever attacks it.
//!
//! The AI follows Minecraft's goals for animals: panic after being hurt (run to random spots,
//! away from whoever hit it), wander to random nearby spots (preferring grass, avoiding
//! water, lava and drops of more than 3 blocks), look at a nearby player, look around, and
//! float in water. Mobs jump up single blocks, take fall, lava, cactus and suffocation
//! damage, get knocked back, flash red when hurt and tip over when they die.
//!
//! Models use Minecraft's entity model format: cubes with box UVs into a 64 unit wide atlas,
//! set up exactly like Minecraft's `PigModel`, `SheepModel` and `WolfModel`; the atlases are
//! drawn at 8 texels per unit, their faces spread over several texture layers
//! (`super::skin_pages`).

use crate::entity::skin_pages::{face_uv, SkinPages};
use crate::model::prim::{quad_at, Paint, Sides};
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
    Dummy,
    Wolf,
}

impl MobKind {
    pub fn key(self) -> &'static str {
        match self {
            MobKind::Pig => "pig",
            MobKind::Sheep => "sheep",
            MobKind::Dummy => "target_dummy",
            MobKind::Wolf => "wolf",
        }
    }

    pub fn from_key(k: &str) -> Option<MobKind> {
        match k.strip_prefix("minecraft:").unwrap_or(k) {
            "pig" => Some(MobKind::Pig),
            "sheep" => Some(MobKind::Sheep),
            "target_dummy" => Some(MobKind::Dummy),
            "wolf" => Some(MobKind::Wolf),
            _ => None,
        }
    }

    /// The kind with this `kind as u8` (LAN messages).
    pub fn from_u8(v: u8) -> Option<MobKind> {
        match v {
            0 => Some(MobKind::Pig),
            1 => Some(MobKind::Sheep),
            2 => Some(MobKind::Dummy),
            3 => Some(MobKind::Wolf),
            _ => None,
        }
    }

    pub fn max_health(self) -> f32 {
        match self {
            MobKind::Pig => 10.0,
            MobKind::Sheep => 8.0,
            MobKind::Dummy => 20.0,
            // (a tame one: `TAME_HEALTH`)
            MobKind::Wolf => 8.0,
        }
    }

    /// Half width and height of the bounding box (Minecraft: pig 0.9 x 0.9, sheep 0.9 x 1.3).
    pub fn size(self) -> (f32, f32) {
        match self {
            MobKind::Pig => (0.45, 0.9),
            MobKind::Sheep => (0.45, 1.3),
            MobKind::Dummy => (0.35, 1.95),
            MobKind::Wolf => (0.3, 0.85),
        }
    }
}

/// Seconds without a hit after which a dummy starts counting from zero again.
pub const DUMMY_RESET: f32 = 6.0;
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
/// A tame wolf's health; how hard a wolf bites, how often, and from how near; how fast it
/// runs after something and after its owner; how long a wild one stays angry.
pub const TAME_HEALTH: f32 = 20.0;
pub const BITE: f32 = 4.0;
const BITE_EVERY: f32 = 1.0;
const BITE_REACH: f32 = 1.5;
const CHASE_SPEED: f32 = WALK_SPEED * 2.1;
const FOLLOW_SPEED: f32 = WALK_SPEED * 1.8;
const WILD_ANGER: f32 = 25.0;

/// Collar colours (Minecraft's dyes), by `Mob::collar`: red first (a new tame wolf's).
pub const COLLARS: [[u8; 3]; 16] = [
    [176, 46, 38], [249, 128, 29], [254, 216, 61], [128, 199, 31], [94, 124, 22], [22, 156, 156],
    [58, 179, 218], [60, 68, 170], [137, 50, 184], [199, 78, 189], [243, 139, 170], [131, 84, 50],
    [29, 29, 33], [71, 79, 82], [157, 157, 151], [249, 255, 254],
];

/// The wolf's skin, as detailed as the blocks: Minecraft's 64x32 unit wolf atlas at 8
/// texels per unit (512x256), its faces on `PAGES` texture layers (`skin_pages`).
pub mod wolf_skin {
    use crate::entity::skin_pages::{BoxUv, SkinPages};

    pub const PAGES: u32 = 8;
    /// The model's boxes: texture offset and size (units), as in Minecraft's `WolfModel`.
    pub const BOXES: [BoxUv; 7] = [
        ([0.0, 0.0], [6.0, 6.0, 4.0]),
        ([16.0, 14.0], [2.0, 2.0, 1.0]),
        ([0.0, 10.0], [3.0, 3.0, 4.0]),
        ([18.0, 14.0], [6.0, 9.0, 6.0]),
        ([21.0, 0.0], [8.0, 6.0, 7.0]),
        ([0.0, 18.0], [2.0, 8.0, 2.0]),
        ([9.0, 18.0], [2.0, 8.0, 2.0]),
    ];
    pub const HEAD: usize = 0;
    pub const EAR: usize = 1;
    pub const SNOUT: usize = 2;
    pub const BODY: usize = 3;
    pub const MANE: usize = 4;
    pub const LEG: usize = 5;
    pub const TAIL: usize = 6;
    pub static SKIN: SkinPages = SkinPages::new(&BOXES, 8, PAGES, 1);
}

/// The pig's skin (Minecraft's 64x64 unit `pig_temperate` atlas at 8 texels per unit, 512x512)
/// on `PAGES` texture layers, like the wolf's.
pub mod pig_skin {
    use crate::entity::skin_pages::{BoxUv, SkinPages};

    pub const PAGES: u32 = 6;
    /// `PigModel`'s boxes: head, snout, body, leg.
    pub const BOXES: [BoxUv; 4] = [
        ([0.0, 0.0], [8.0, 8.0, 8.0]),
        ([16.0, 16.0], [4.0, 3.0, 1.0]),
        ([28.0, 8.0], [10.0, 16.0, 8.0]),
        ([0.0, 16.0], [4.0, 6.0, 4.0]),
    ];
    pub const HEAD: usize = 0;
    pub const SNOUT: usize = 1;
    pub const BODY: usize = 2;
    pub const LEG: usize = 3;
    pub static SKIN: SkinPages = SkinPages::new(&BOXES, 8, PAGES, 0);
}

/// The sheep's skin and its wool coat (Minecraft's 64x32 unit atlases at 8 texels per unit,
/// 512x256) on `PAGES` and `WOOL_PAGES` texture layers, like the wolf's.
pub mod sheep_skin {
    use crate::entity::skin_pages::{BoxUv, SkinPages};

    pub const PAGES: u32 = 5;
    pub const WOOL_PAGES: u32 = 4;
    /// `SheepModel`'s boxes (head, body, leg), and `SheepFurModel`'s.
    pub const BOXES: [BoxUv; 3] = [
        ([0.0, 0.0], [6.0, 6.0, 8.0]),
        ([28.0, 8.0], [8.0, 16.0, 6.0]),
        ([0.0, 16.0], [4.0, 12.0, 4.0]),
    ];
    pub const WOOL_BOXES: [BoxUv; 3] = [
        ([0.0, 0.0], [6.0, 6.0, 6.0]),
        ([28.0, 8.0], [8.0, 16.0, 6.0]),
        ([0.0, 16.0], [4.0, 6.0, 4.0]),
    ];
    pub const HEAD: usize = 0;
    pub const BODY: usize = 1;
    pub const LEG: usize = 2;
    pub static SKIN: SkinPages = SkinPages::new(&BOXES, 8, PAGES, 0);
    pub static WOOL: SkinPages = SkinPages::new(&WOOL_BOXES, 8, WOOL_PAGES, 0);
}

/// Someone a wolf goes for: a mob (its id) or a player (their LAN id; the host is 0).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Foe {
    Mob(u32),
    Player(u8),
}

/// `MobNet::flags` of a wolf: tame, sitting, angry, and tamed by whoever gets it.
pub mod wolf_flags {
    pub const TAME: u8 = 1;
    pub const SITTING: u8 = 2;
    pub const ANGRY: u8 = 4;
    pub const YOURS: u8 = 8;
}

/// What the world around a mob looks like to its AI.
pub struct MobCtx {
    /// Feet of every living player (the host and LAN players).
    pub players: Vec<Vec3>,
    /// The same with who they are: their LAN id (the host 0) and name.
    pub people: Vec<(u8, String, Vec3)>,
    /// Where every living mob is (its id and feet), for a wolf going after one.
    pub mobs: Vec<(u32, Vec3)>,
}

/// Something that happened to a mob during an update.
pub enum MobEvent {
    None,
    /// The death animation is over: remove it and drop its loot.
    Remove,
    /// A sheep ate the grass here (a grass block turns to dirt, tall grass goes).
    EatGrass(glam::IVec3),
    /// A wolf bit someone (`BITE` damage, from where it is).
    Bite(Foe),
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
    /// A dummy: the damage it has taken since it was last left alone for `DUMMY_RESET`
    /// seconds, the last hit, and seconds since that hit.
    pub taken: f32,
    pub last_hit: f32,
    pub since_hit: f32,
    /// A dummy: how far its body is tipped (radians toward model +X and +Z), and how fast.
    pub tilt: Vec2,
    tilt_vel: Vec2,
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
    /// A wolf: the name of the player who tamed it, whether it sits, its collar's colour
    /// (`COLLARS`), whom it is going for (and, a wild one, for how much longer), and when it
    /// may bite again. `yours`: on a LAN player's copy, tamed by that player.
    pub owner: Option<String>,
    pub sitting: bool,
    pub collar: u8,
    pub foe: Option<Foe>,
    anger: f32,
    bite_cooldown: f32,
    pub yours: bool,
    /// Walking somewhere at a speed of its own (a wolf running after something).
    hurry: Option<f32>,
    /// A bite to report from this update.
    bite: Option<Foe>,
    /// When it may make its next sound, and whether this hurt was already yelped.
    sound_wait: f32,
    yelped: bool,
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
            taken: 0.0,
            last_hit: 0.0,
            since_hit: f32::MAX,
            tilt: Vec2::ZERO,
            tilt_vel: Vec2::ZERO,
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
            owner: None,
            sitting: false,
            collar: 0,
            foe: None,
            anger: 0.0,
            bite_cooldown: 0.0,
            yours: false,
            hurry: None,
            bite: None,
            sound_wait: 3.0 + (seed % 97) as f32 * 0.05,
            yelped: false,
        }
    }

    /// A tame wolf (one with an owner).
    pub fn tame(&self) -> bool {
        self.owner.is_some() || (self.kind == MobKind::Wolf && self.yours_or_tame_net())
    }

    fn yours_or_tame_net(&self) -> bool {
        self.net_target.is_some_and(|s| s.flags & wolf_flags::TAME != 0)
    }

    /// Its greatest health (a tame wolf's is more).
    pub fn max_health(&self) -> f32 {
        if self.kind == MobKind::Wolf && self.owner.is_some() {
            TAME_HEALTH
        } else {
            self.kind.max_health()
        }
    }

    /// Given a bone by `name`: a wild wolf is tamed a third of the time (it sits down and
    /// gets its collar). Returns whether it took to them.
    pub fn feed_bone(&mut self, name: &str) -> bool {
        if self.kind != MobKind::Wolf || self.owner.is_some() || !self.alive() {
            return false;
        }
        if self.rand() >= 1.0 / 3.0 {
            return false;
        }
        self.owner = Some(name.to_string());
        self.sitting = true;
        self.foe = None;
        self.target = None;
        self.health = TAME_HEALTH;
        true
    }

    /// Right clicked by its owner: sits down or stands up again (and lets go of whom it was
    /// going for).
    pub fn toggle_sit(&mut self) {
        self.sitting = !self.sitting;
        self.foe = None;
        self.target = None;
        self.vel.x = 0.0;
        self.vel.z = 0.0;
    }

    /// Attacked by `foe` (a player's id for players): a wolf goes for them, unless it is its
    /// owner's or it sits. `owner_of_foe`: that player's name.
    pub fn provoke(&mut self, foe: Foe, foe_name: Option<&str>) {
        if self.kind != MobKind::Wolf || !self.alive() || self.sitting {
            return;
        }
        if foe_name.is_some() && foe_name == self.owner.as_deref() {
            return;
        }
        self.foe = Some(foe);
        self.anger = WILD_ANGER;
    }

    /// The sound it makes now, if any: a wolf barks and pants now and then (a tame one
    /// low on health whines, an angry one growls), and yelps when hurt.
    pub fn sound(&mut self, dt: f32) -> Option<crate::audio::Sound> {
        use crate::audio::Sound;
        if self.kind != MobKind::Wolf {
            return None;
        }
        if self.hurt_time > 0.0 && !self.yelped && self.alive() {
            self.yelped = true;
            return Some(Sound::WolfHurt);
        }
        if self.hurt_time <= 0.0 {
            self.yelped = false;
        }
        self.sound_wait -= dt;
        if self.sound_wait > 0.0 || !self.alive() {
            return None;
        }
        self.sound_wait = 4.0 + self.rand() * 6.0;
        let angry = self.foe.is_some() || self.net_target.is_some_and(|s| s.flags & wolf_flags::ANGRY != 0);
        Some(if angry {
            Sound::WolfGrowl
        } else if self.tame() && self.health < 8.0 {
            Sound::WolfWhine
        } else if self.rand() < 0.35 {
            Sound::WolfBark
        } else {
            Sound::WolfPant
        })
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
            // A dummy has no legs: its limb values carry how it is tipped.
            limb_swing: if self.kind == MobKind::Dummy { self.tilt.x } else { self.limb_swing },
            limb_amount: if self.kind == MobKind::Dummy { self.tilt.y } else { self.limb_amount },
            hurt: self.hurt_time > 0.0,
            death: self.death.unwrap_or(-1.0),
            sheared: self.sheared,
            taken: self.taken,
            last_hit: self.last_hit,
            flags: self.wolf_flags(None),
            collar: self.collar,
        }
    }

    /// A wolf's `MobNet::flags`, for the player named `to` (`YOURS` if it is theirs).
    pub fn wolf_flags(&self, to: Option<&str>) -> u8 {
        use wolf_flags::*;
        let mut f = 0;
        if self.owner.is_some() {
            f |= TAME;
        }
        if self.sitting {
            f |= SITTING;
        }
        if self.foe.is_some() {
            f |= ANGRY;
        }
        if to.is_some() && to == self.owner.as_deref() {
            f |= YOURS;
        }
        f
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
        if s.taken != self.taken {
            self.since_hit = if s.taken > 0.0 { 0.0 } else { f32::MAX };
        }
        self.taken = s.taken;
        self.last_hit = s.last_hit;
        self.sitting = s.flags & wolf_flags::SITTING != 0;
        self.yours = s.flags & wolf_flags::YOURS != 0;
        self.collar = s.collar;
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
        if self.kind == MobKind::Dummy {
            self.tilt = Vec2::new(self.limb_swing, self.limb_amount);
            self.since_hit += dt;
        }
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
        if self.kind == MobKind::Dummy {
            self.hit_dummy(amount, from, knockback);
            return true;
        }
        if !self.alive() || self.hurt_time > 0.0 {
            return false;
        }
        self.health -= amount;
        self.hurt_time = HURT_TIME;
        // (a wolf does not run away: it turns on whoever hurt it, `provoke`)
        if self.kind != MobKind::Wolf {
            self.panic = 4.0 + self.rand() * 2.0;
            self.target = None;
        }
        if let Some(src) = from {
            if self.kind != MobKind::Wolf {
                self.flee_from = Some(src);
            }
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

    /// A dummy hit: every hit counts (it is never out of reach for a moment like a hurt
    /// animal), and it rocks away from where the hit came from.
    fn hit_dummy(&mut self, amount: f32, from: Option<Vec3>, knockback: f32) {
        if self.since_hit > DUMMY_RESET {
            self.taken = 0.0;
        }
        self.taken += amount;
        self.last_hit = amount;
        self.since_hit = 0.0;
        let away = from
            .and_then(|src| ((self.center() - src) * Vec3::new(1.0, 0.0, 1.0)).try_normalize())
            .unwrap_or_else(|| {
                let a = self.rand() * TAU;
                Vec3::new(a.cos(), 0.0, a.sin())
            });
        // Into model space (the model is turned by `FRAC_PI_2 - body_yaw`, see `build`).
        let local = Mat4::from_rotation_y(self.body_yaw - FRAC_PI_2).transform_vector3(away);
        let kick = (1.6 + amount * 0.2) * knockback.clamp(0.3, 2.0);
        self.tilt_vel += Vec2::new(local.x, local.z) * kick.min(5.5);
    }

    /// A dummy's update: it only falls (onto what it stands on), rocks back upright, and
    /// forgets the damage after a while left alone.
    fn update_dummy(&mut self, dt: f32, w: &World) -> MobEvent {
        self.since_hit += dt;
        if self.since_hit > DUMMY_RESET {
            self.taken = 0.0;
            self.last_hit = 0.0;
        }
        // A springy wooden foot: it rocks back and forth a few times before it settles.
        let acc = -self.tilt * 55.0 - self.tilt_vel * 3.0;
        self.tilt_vel += acc * dt;
        self.tilt += self.tilt_vel * dt;
        if self.tilt.length() > 0.6 {
            self.tilt = self.tilt.normalize() * 0.6;
            self.tilt_vel *= 0.5;
        }
        self.vel = Vec3::new(0.0, (self.vel.y - GRAVITY * dt).max(-60.0), 0.0);
        if self.move_axis(w, 1, self.vel.y * dt) {
            self.vel.y = 0.0;
        }
        MobEvent::None
    }

    /// Environmental damage (no knockback).
    fn hurt_env(&mut self, amount: f32) {
        self.hurt(amount, None, 0.0);
    }

    /// Nudged by something overlapping it (other mobs, the player).
    pub fn push(&mut self, v: Vec3) {
        if self.kind == MobKind::Dummy {
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
    fn nearest_player(&self, ctx: &MobCtx, range: f32) -> Option<Vec3> {
        ctx.players
            .iter()
            .copied()
            .filter(|p| p.distance(self.pos) < range)
            .min_by(|a, b| a.distance(self.pos).total_cmp(&b.distance(self.pos)))
    }

    /// Where someone a wolf goes for is (their feet), if they are still around.
    fn foe_pos(&self, ctx: &MobCtx) -> Option<Vec3> {
        match self.foe? {
            Foe::Mob(id) => ctx.mobs.iter().find(|(i, _)| *i == id).map(|(_, p)| *p),
            Foe::Player(id) => ctx.people.iter().find(|(i, _, _)| *i == id).map(|(_, _, p)| *p),
        }
    }

    /// Where a tame wolf's owner is, if they are around.
    fn owner_pos(&self, ctx: &MobCtx) -> Option<Vec3> {
        let name = self.owner.as_deref()?;
        ctx.people.iter().find(|(_, n, _)| n == name).map(|(_, _, p)| *p)
    }

    /// A wolf's own goals (Minecraft's SitWhenOrderedTo, MeleeAttack, FollowOwner): Some
    /// when they decide what it does now.
    fn wolf_think(&mut self, dt: f32, w: &World, ctx: &MobCtx) -> Option<Option<(Vec3, f32)>> {
        self.bite_cooldown -= dt;
        self.hurry = None;
        if self.sitting {
            self.target = None;
            if self.owner_pos(ctx).is_some() && self.look_time <= 0.0 {
                self.look = Look::Player;
                self.look_time = 2.0;
            }
            return Some(None);
        }
        // Going for someone: runs to them and bites when near (a wild one calms down in
        // a while; anyone gone or far away is let go).
        if self.foe.is_some() {
            if self.owner.is_none() {
                self.anger -= dt;
            }
            match self.foe_pos(ctx) {
                Some(p) if p.distance(self.pos) < 24.0 && (self.owner.is_some() || self.anger > 0.0) => {
                    let d = Vec2::new(p.x - self.pos.x, p.z - self.pos.z);
                    self.look = Look::Ahead;
                    if d.length() < BITE_REACH && (p.y - self.pos.y).abs() < 1.5 {
                        self.target = None;
                        self.body_yaw = turn(self.body_yaw, d.y.atan2(d.x), 12.0 * dt);
                        if self.bite_cooldown <= 0.0 {
                            self.bite_cooldown = BITE_EVERY;
                            self.bite = self.foe;
                        }
                        return Some(None);
                    }
                    self.target = Some(p);
                    self.target_time = 1.0;
                    self.hurry = Some(CHASE_SPEED);
                    return Some(self.walk(w));
                }
                _ => {
                    self.foe = None;
                    self.target = None;
                }
            }
        }
        // A tame one keeps up with its owner: runs after them, and jumps to their side when
        // left far behind.
        if let Some(o) = self.owner_pos(ctx) {
            let d = o.distance(self.pos);
            if d > 14.0 {
                for k in 0..10 {
                    let a = k as f32 * 0.7 + self.rand();
                    let (x, z) = ((o.x + a.cos() * 2.0).floor() as i32, (o.z + a.sin() * 2.0).floor() as i32);
                    if let Some((p, ground)) = standable(w, x, z, o.y.round() as i32, 2) {
                        if !is_lava(ground) {
                            self.pos = p;
                            self.vel = Vec3::ZERO;
                            self.fall_peak = p.y;
                            self.target = None;
                            break;
                        }
                    }
                }
                return Some(None);
            }
            if d > 4.0 {
                self.target = Some(o);
                self.target_time = 1.0;
                self.hurry = Some(if d > 8.0 { FOLLOW_SPEED * 1.3 } else { FOLLOW_SPEED });
                return Some(self.walk(w));
            }
            if self.target.is_some() && d < 2.5 {
                self.target = None;
            }
        }
        None
    }

    fn think(&mut self, dt: f32, w: &World, ctx: &MobCtx) -> Option<(Vec3, f32)> {
        if self.kind == MobKind::Wolf {
            if let Some(r) = self.wolf_think(dt, w, ctx) {
                return r;
            }
        }
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
        self.target_time -= dt;
        self.walk(w)
    }

    /// Walks toward `target` (at its `hurry`, or its pace), looking out for drops, lava,
    /// water and walls, jumping up single blocks.
    fn walk(&mut self, w: &World) -> Option<(Vec3, f32)> {
        let target = self.target?;
        let to = Vec2::new(target.x - self.pos.x, target.z - self.pos.z);
        if to.length() < 0.4 || self.target_time <= 0.0 || self.stuck > 1.5 {
            self.target = None;
            self.idle = 1.0 + self.rand() * 4.0;
            return None;
        }
        let speed = if let Some(s) = self.hurry {
            s
        } else if self.panic > 0.0 {
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

    fn step(&mut self, dt: f32, w: &World, ctx: &MobCtx) -> MobEvent {
        if self.kind == MobKind::Dummy {
            return self.update_dummy(dt, w);
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
        if let (Some(f), true) = (self.bite.take(), self.alive()) {
            return MobEvent::Bite(f);
        }
        MobEvent::None
    }

    // ------------------------------------------------------------------------ model

    pub fn build(&self, out: &mut Vec<Vertex>, sky: u8, blk: u8) {
        let light = vertex_light(sky, blk);
        if self.kind == MobKind::Dummy {
            // Its model's front (+Z) toward where it faces.
            let root = Mat4::from_translation(self.pos)
                * Mat4::from_rotation_y(FRAC_PI_2 - self.body_yaw)
                * Mat4::from_scale(Vec3::splat(1.0 / 16.0));
            crate::model::dummy::emit(out, root, self.tilt, light, 0);
            return;
        }
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
            MobKind::Wolf => self.build_wolf(out, root, tint, light),
            MobKind::Dummy => {}
        }
    }

    /// Minecraft's `WolfModel`: head (with its ears and snout), body, mane, four legs and
    /// the tail; sitting, the body tips back onto its haunches. The wild, tame or angry
    /// wolf's texture, and a tame one's collar over it in its colour.
    fn build_wolf(&self, out: &mut Vec<Vertex>, root: Mat4, tint: [u8; 3], light: [u8; 4]) {
        let part = |px: f32, py: f32, pz: f32, rot: Mat4| {
            root * Mat4::from_translation(Vec3::new(-px, 24.0 - py, pz))
                * rot
                * Mat4::from_scale(Vec3::new(-1.0, -1.0, 1.0))
        };
        let rx = |a: f32| Mat4::from_rotation_x(-a);
        let flags = self.net_target.map(|s| s.flags);
        let tame = self.owner.is_some() || flags.is_some_and(|f| f & wolf_flags::TAME != 0);
        let angry = self.foe.is_some() || flags.is_some_and(|f| f & wolf_flags::ANGRY != 0);
        let sit = self.sitting;
        let ls = self.limb_swing * 0.6662;
        let la = self.limb_amount;

        let head_yaw = wrap_angle(self.head_yaw - self.body_yaw).clamp(-HEAD_LIMIT, HEAD_LIMIT);
        let head = part(-1.0, 13.5, -7.0, Mat4::from_rotation_y(-head_yaw) * Mat4::from_rotation_x(self.pitch));
        let (body, mane, tail_at, legs) = if sit {
            (
                part(0.0, 18.0, 0.0, rx(PI / 4.0)),
                part(-1.0, 16.0, -3.0, rx(1.256_637)),
                (-1.0, 21.0, 6.0),
                [
                    part(-2.5, 22.7, 2.0, rx(PI * 1.5)),
                    part(0.5, 22.7, 2.0, rx(PI * 1.5)),
                    part(-2.49, 17.0, -4.0, rx(5.811_947)),
                    part(0.51, 17.0, -4.0, rx(5.811_947)),
                ],
            )
        } else {
            (
                part(0.0, 14.0, 2.0, rx(FRAC_PI_2)),
                part(-1.0, 14.0, -3.0, rx(FRAC_PI_2)),
                (-1.0, 12.0, 8.0),
                [
                    part(-2.5, 16.0, 7.0, rx(ls.cos() * 1.4 * la)),
                    part(0.5, 16.0, 7.0, rx((ls + PI).cos() * 1.4 * la)),
                    part(-2.5, 16.0, -4.0, rx((ls + PI).cos() * 1.4 * la)),
                    part(0.5, 16.0, -4.0, rx(ls.cos() * 1.4 * la)),
                ],
            )
        };
        // The tail: high and wagging on a tame one (lower the more it is hurt), straight up
        // on an angry one, hanging on a wild one.
        let tail_up = if angry {
            1.539_380_4
        } else if tame {
            (0.55 - (TAME_HEALTH - self.health).max(0.0) * 0.02) * PI
        } else {
            PI / 5.0
        };
        let wag = if angry { 0.0 } else { ls.cos() * 1.4 * la };
        let tail = part(tail_at.0, tail_at.1, tail_at.2, Mat4::from_rotation_y(-wag) * rx(tail_up));

        use wolf_skin::*;
        let skin = if angry {
            tex::WOLF_ANGRY
        } else if tame {
            tex::WOLF_TAME
        } else {
            tex::WOLF
        };
        let c = |out: &mut Vec<Vertex>, m: Mat4, o: [f32; 3], b: usize| {
            emit_paged(out, m, o, &SKIN, b, 0.0, skin, tint, light);
        };
        c(out, head, [-2.0, -3.0, -2.0], HEAD);
        c(out, head, [-2.0, -5.0, 0.0], EAR);
        c(out, head, [2.0, -5.0, 0.0], EAR);
        c(out, head, [-0.5, 0.0, -5.0], SNOUT);
        c(out, body, [-3.0, -2.0, -3.0], BODY);
        c(out, mane, [-3.0, -3.0, -3.0], MANE);
        for leg in legs {
            c(out, leg, [0.0, 0.0, -1.0], LEG);
        }
        c(out, tail, [0.0, 0.0, -1.0], TAIL);
        if tame {
            // The collar round the mane (where its texture has it), in its colour, clearly
            // over the fur: a layer just on it would flicker through it.
            let col = COLLARS[self.collar as usize % COLLARS.len()];
            let tinted = std::array::from_fn(|i| (col[i] as u16 * tint[i] as u16 / 255) as u8);
            emit_paged(out, mane, [-3.0, -3.0, -3.0], &SKIN, MANE, 0.25, tex::WOLF_COLLAR, tinted, light);
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

        use sheep_skin::*;
        let skin = |out: &mut Vec<Vertex>, m: Mat4, o: [f32; 3], b: usize| {
            emit_paged(out, m, o, &SKIN, b, 0.0, tex::SHEEP, tint, light);
        };
        skin(out, head, [-3.0, -4.0, -6.0], HEAD);
        skin(out, body, [-4.0, -10.0, -7.0], BODY);
        for leg in legs {
            skin(out, leg, [-2.0, 0.0, -2.0], LEG);
        }
        if self.sheared {
            return;
        }
        let wool = |out: &mut Vec<Vertex>, m: Mat4, o: [f32; 3], b: usize, grow: f32| {
            emit_paged(out, m, o, &WOOL, b, grow, tex::SHEEP_WOOL, tint, light);
        };
        wool(out, head, [-3.0, -4.0, -4.0], HEAD, 0.6);
        wool(out, body, [-4.0, -10.0, -7.0], BODY, 1.75);
        for leg in legs {
            wool(out, leg, [-2.0, 0.0, -2.0], LEG, 0.5);
        }
    }

    /// Minecraft's `PigModel` (a `QuadrupedModel` with leg height 6), in model pixels.
    fn build_pig(&self, out: &mut Vec<Vertex>, root: Mat4, tint: [u8; 3], light: [u8; 4]) {
        use pig_skin::*;
        // A part's pivot, given in Minecraft's model coordinates (Y down from 24 = the ground,
        // X mirrored), then its rotation in ours.
        let part = |px: f32, py: f32, pz: f32, rot: Mat4| {
            root * Mat4::from_translation(Vec3::new(-px, 24.0 - py, pz))
                * rot
                * Mat4::from_scale(Vec3::new(-1.0, -1.0, 1.0))
        };
        let cube = |out: &mut Vec<Vertex>, m: Mat4, o: [f32; 3], b: usize| {
            emit_paged(out, m, o, &SKIN, b, 0.0, tex::PIG, tint, light);
        };

        let head_yaw = wrap_angle(self.head_yaw - self.body_yaw).clamp(-HEAD_LIMIT, HEAD_LIMIT);
        let head = part(
            0.0,
            12.0,
            -6.0,
            Mat4::from_rotation_y(-head_yaw) * Mat4::from_rotation_x(self.pitch),
        );
        cube(out, head, [-4.0, -4.0, -8.0], HEAD);
        cube(out, head, [-2.0, 0.0, -9.0], SNOUT);

        let body = part(0.0, 11.0, 2.0, Mat4::from_rotation_x(-FRAC_PI_2));
        cube(out, body, [-5.0, -10.0, -7.0], BODY);

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
            cube(out, leg, [-2.0, 0.0, -2.0], LEG);
        }
    }
}

/// One cube of a Minecraft entity model (Minecraft's `ModelPart.Cube`): box `b` of a skin
/// (its size and box UV), at `o` in model pixels, textured from the skin's pages from layer
/// `base`. `grow` makes it bigger on every side without changing its texture (Minecraft's
/// `CubeDeformation`).
#[allow(clippy::too_many_arguments)]
fn emit_paged(
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
    fn a_wolf_is_built_and_a_tame_one_wears_its_collar() {
        let mut wolf = Mob::new(MobKind::Wolf, Vec3::ZERO, 0.0, 7);
        let mut out = Vec::new();
        wolf.build(&mut out, 15, 0);
        // Head, two ears, snout, body, mane, four legs and the tail.
        assert_eq!(out.len(), 11 * 6 * 6);
        let max_y = out.iter().map(|v| v.pos[1]).fold(f32::MIN, f32::max);
        assert!(max_y > 0.8 && max_y < 1.1, "top at {max_y}");
        wolf.owner = Some("Alby".into());
        out.clear();
        wolf.build(&mut out, 15, 0);
        assert_eq!(out.len(), 12 * 6 * 6);
        assert!(out.iter().any(|v| v.layer == tex::WOLF_COLLAR as f32));
        let net = wolf.to_net();
        assert!(net.flags & wolf_flags::TAME != 0 && net.flags & wolf_flags::YOURS == 0);
        assert!(wolf.wolf_flags(Some("Alby")) & wolf_flags::YOURS != 0);
        let copy = Mob::from_net(&net).expect("a wolf");
        assert!(copy.tame() && copy.kind == MobKind::Wolf);
    }

    #[test]
    fn a_wolf_bites_what_it_goes_for_but_never_its_owner() {
        let w = World::new();
        let mut wolf = Mob::new(MobKind::Wolf, Vec3::new(0.5, 70.0, 0.5), 0.0, 7);
        wolf.owner = Some("Alby".into());
        wolf.provoke(Foe::Player(0), Some("Alby"));
        assert!(wolf.foe.is_none(), "turned on its owner");
        wolf.provoke(Foe::Mob(9), None);
        let ctx = MobCtx {
            players: vec![],
            people: vec![(0, "Alby".into(), Vec3::new(3.0, 70.0, 0.5))],
            mobs: vec![(9, Vec3::new(1.2, 70.0, 0.5))],
        };
        // (in an empty world: what matters is the goal)
        let r = wolf.wolf_think(0.05, &w, &ctx);
        assert!(matches!(r, Some(None)));
        assert_eq!(wolf.bite, Some(Foe::Mob(9)));
        // Sitting, it goes for nothing.
        wolf.toggle_sit();
        assert!(wolf.sitting && wolf.foe.is_none());
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
