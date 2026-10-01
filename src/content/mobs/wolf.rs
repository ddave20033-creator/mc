//! The wolf, Minecraft's: a neutral mob. Wild ones roam the forests and taigas in packs and
//! turn on whoever hurts one of them; given a bone, one may be tamed (a third of the time).
//! A tame wolf wears a collar, has more health, follows its owner (jumping to their side when
//! left far behind), sits and stands up again when its owner right clicks it, and goes for
//! whatever its owner attacks or whoever attacks its owner.

use super::*;
use crate::entity::mob::model::{animal_root, emit_paged, head_turn, part};
use crate::entity::mob::Look;
use crate::item::{BONE, WOLF_SPAWN_EGG};
use crate::textures::tex;
use crate::world::{is_lava, SNOWY_GRASS};
use glam::Mat4;
use std::f32::consts::{FRAC_PI_2, PI};

pub const WOLF: MobKind = MobKind(3);

pub const DEF: MobDef = MobDef {
    kind: WOLF,
    key: "wolf",
    en: "Wolf",
    hu: "Farkas",
    // (a tame one's: `TAME_HEALTH`)
    health: 8.0,
    size: (0.3, 0.85),
    attack: Some(Attack { damage: 4.0, kind: crate::net::hurt::WOLF }),
    spawn: Some(Spawn {
        weight: 12,
        group: (2, 4),
        // (the snowy taiga's ground is snowy grass)
        ground: &[GRASS, SNOWY_GRASS],
        biomes: &[Biome::Forest, Biome::BirchForest, Biome::Taiga, Biome::SnowyTaiga],
        min_light: 9,
    }),
    sounds: Sounds { hurt: Some(Sound::WolfHurt), idle: Some(idle_sound), every: (4.0, 10.0) },
    model,
    egg: WOLF_SPAWN_EGG,
    state: || MobState::Wolf(Pet::default()),
    hooks: Hooks { think: Some(think), use_on: Some(use_on), used: Some(used), ..NO_HOOKS },
    ..NEUTRAL
};

/// A tame wolf's health.
pub const TAME_HEALTH: f32 = 20.0;
/// How fast a tame wolf runs after its owner (times the walking speed), and from how far
/// it jumps to their side.
const FOLLOW_SPEED: f32 = 1.8;
const CATCH_UP: f32 = 14.0;

/// Collar colours (Minecraft's dyes), by `Pet::collar`: red first (a new tame wolf's).
pub const COLLARS: [[u8; 3]; 16] = [
    [176, 46, 38], [249, 128, 29], [254, 216, 61], [128, 199, 31], [94, 124, 22], [22, 156, 156],
    [58, 179, 218], [60, 68, 170], [137, 50, 184], [199, 78, 189], [243, 139, 170], [131, 84, 50],
    [29, 29, 33], [71, 79, 82], [157, 157, 151], [249, 255, 254],
];

/// A wolf's `MobNet::flags` (above `mob_flags`): tame, sitting, and tamed by whoever gets it.
pub mod flags {
    pub const TAME: u8 = 2;
    pub const SITTING: u8 = 4;
    pub const YOURS: u8 = 8;
}

/// A wolf's side as a pet.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Pet {
    /// The name of the player who tamed it.
    pub owner: Option<String>,
    pub sitting: bool,
    /// Its collar's colour (`COLLARS`).
    pub collar: u8,
    /// A player's copy: tame, and tamed by that player (the server's has its `owner`).
    pub shown_tame: bool,
    pub yours: bool,
}

impl Pet {
    pub fn max_health(&self) -> Option<f32> {
        self.owner.as_ref().map(|_| TAME_HEALTH)
    }

    /// Sitting, collar, owner (`escape`d).
    pub fn save(&self) -> String {
        let owner = self.owner.as_deref().map_or(String::new(), escape);
        format!("{},{},{}", self.sitting as u8, self.collar, owner)
    }

    pub fn load(&mut self, s: &str) {
        let mut parts = s.splitn(3, ',');
        self.sitting = parts.next() == Some("1");
        self.collar = parts.next().and_then(|c| c.parse().ok()).unwrap_or(0);
        self.owner = parts.next().filter(|o| !o.is_empty()).map(unescape);
    }

    pub fn to_net(&self, net: &mut crate::net::MobNet, to: Option<&str>) {
        if self.owner.is_some() {
            net.flags |= flags::TAME;
        }
        if self.sitting {
            net.flags |= flags::SITTING;
        }
        if to.is_some() && to == self.owner.as_deref() {
            net.flags |= flags::YOURS;
        }
        net.collar = self.collar;
    }

    pub fn apply_net(&mut self, net: &crate::net::MobNet) {
        self.shown_tame = net.flags & flags::TAME != 0;
        self.sitting = net.flags & flags::SITTING != 0;
        self.yours = net.flags & flags::YOURS != 0;
        self.collar = net.collar;
    }
}

impl Mob {
    /// A wolf's side as a pet.
    pub fn pet(&self) -> Option<&Pet> {
        match &self.state {
            MobState::Wolf(p) => Some(p),
            _ => None,
        }
    }

    pub fn pet_mut(&mut self) -> Option<&mut Pet> {
        match &mut self.state {
            MobState::Wolf(p) => Some(p),
            _ => None,
        }
    }

    /// A tame wolf (one with an owner; a player's copy: as the server said).
    pub fn tame(&self) -> bool {
        self.pet().is_some_and(|p| p.owner.is_some() || p.shown_tame)
    }

    /// Given a bone by `name`: a wild wolf is tamed when `took` (it sits down and gets its
    /// collar). Returns whether it took to them.
    pub fn feed_bone(&mut self, name: &str, took: bool) -> bool {
        if self.tame() || !self.alive() || !took {
            return false;
        }
        let Some(p) = self.pet_mut() else {
            return false;
        };
        p.owner = Some(name.to_string());
        p.sitting = true;
        self.foe = None;
        self.target = None;
        self.health = TAME_HEALTH;
        true
    }

    /// Right clicked by its owner: sits down or stands up again (and lets go of whom it was
    /// going for).
    pub fn toggle_sit(&mut self) {
        if let Some(p) = self.pet_mut() {
            p.sitting = !p.sitting;
        }
        self.foe = None;
        self.target = None;
        self.vel.x = 0.0;
        self.vel.z = 0.0;
    }
}

/// Where its owner is, if they are around.
fn owner_pos(m: &Mob, ctx: &MobCtx) -> Option<Vec3> {
    let name = m.owner()?;
    ctx.people.iter().find(|(_, n, _)| n == name).map(|(_, _, p)| *p)
}

/// Its own goals (Minecraft's SitWhenOrderedTo, then the template's MeleeAttack, then
/// FollowOwner), before the template's looking around and strolling.
fn think(m: &mut Mob, dt: f32, w: &World, ctx: &MobCtx) -> Option<Steer> {
    if m.sitting() {
        // It watches its owner while they are around.
        m.target = None;
        if m.look_time <= 0.0 {
            m.look = if owner_pos(m, ctx).is_some() { Look::Player } else { Look::Ahead };
            m.look_time = 2.0;
        }
        return Some(None);
    }
    if let Some(steer) = m.chase(dt, w, ctx) {
        return Some(steer);
    }
    // A tame one keeps up with its owner: runs after them, and jumps to their side when left
    // far behind (if there is ground there; if not, it runs on).
    let o = owner_pos(m, ctx)?;
    let d = o.distance(m.pos);
    if d > CATCH_UP && catch_up(m, w, o) {
        return Some(None);
    }
    if d > 4.0 {
        m.target = Some(o);
        m.target_time = 1.0;
        let speed = m.def().speed * FOLLOW_SPEED;
        m.hurry = Some(if d > 8.0 { speed * 1.3 } else { speed });
        return Some(m.walk(w));
    }
    if m.target.is_some() && d < 2.5 {
        m.target = None;
    }
    None
}

/// Jumps to a spot to stand on about 2 blocks from its owner at `o`. False if there is none.
fn catch_up(m: &mut Mob, w: &World, o: Vec3) -> bool {
    for k in 0..10 {
        let a = k as f32 * 0.7 + m.rand();
        let (x, z) = ((o.x + a.cos() * 2.0).floor() as i32, (o.z + a.sin() * 2.0).floor() as i32);
        if let Some((p, ground)) = crate::entity::mob::standable(w, x, z, o.y.round() as i32, 2) {
            if !is_lava(ground) {
                m.pos = p;
                m.vel = Vec3::ZERO;
                m.fall_peak = p.y;
                m.target = None;
                return true;
            }
        }
    }
    false
}

/// Now and then: an angry one growls, a tame one low on health whines, the rest bark or pant.
fn idle_sound(m: &mut Mob) -> Option<Sound> {
    Some(if m.angry() {
        Sound::WolfGrowl
    } else if m.tame() && m.health < 8.0 {
        Sound::WolfWhine
    } else if m.rand() < 0.35 {
        Sound::WolfBark
    } else {
        Sound::WolfPant
    })
}

/// A player's right click: a wild, calm one is given a bone (it may take to them), and a tame
/// one of theirs sits down or stands up.
fn use_on(m: &mut Mob, held: ItemId, fresh: bool) -> Option<Use> {
    if !fresh || !m.alive() {
        return None;
    }
    if held == BONE && !m.tame() && !m.angry() {
        return Some(Use { consume: true, wear: 0 });
    }
    if m.pet().is_some_and(|p| p.yours) {
        m.toggle_sit();
        return Some(Use::default());
    }
    None
}

/// The server's side: the bone is taken a third of the time (crumbs either way, and a bark
/// or a puff of smoke); its owner makes it sit or stand.
fn used(m: &mut Mob, held: ItemId, who: &str, r: f32) -> Used {
    use crate::net::fx;
    if !m.alive() {
        return Used::default();
    }
    if held == BONE && !m.tame() && !m.angry() {
        let took = m.feed_bone(who, r < 1.0 / 3.0);
        let kind = if took { fx::WOLF_TAKES } else { fx::WOLF_REFUSES };
        return Used { fx: Some((kind, m.pos + Vec3::Y * 0.6)), drop: None };
    }
    if m.owner() == Some(who) {
        m.toggle_sit();
    }
    Used::default()
}

/// The wolf's skin, as detailed as the blocks: Minecraft's 64x32 unit wolf atlas at 8
/// texels per unit (512x256), its faces on `PAGES` texture layers (`skin_pages`).
pub mod skin {
    use crate::textures::skin_pages::{BoxUv, SkinPages};

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

/// Minecraft's `WolfModel`: head (with its ears and snout), body, mane, four legs and the
/// tail; sitting, the body tips back onto its haunches. The wild, tame or angry wolf's
/// texture, and a tame one's collar over it in its colour.
fn model(m: &Mob, out: &mut Vec<Vertex>, light: [u8; 4]) {
    let p = &m.pose();
    let (root, tint) = (animal_root(p), m.tint());
    let part = |px: f32, py: f32, pz: f32, rot: Mat4| part(root, px, py, pz, rot);
    let rx = |a: f32| Mat4::from_rotation_x(-a);
    let (tame, angry, sit) = (m.tame(), m.angry(), m.sitting());
    let ls = p.limb_swing * 0.6662;
    let la = p.limb_amount;

    let head = part(-1.0, 13.5, -7.0, head_turn(p));
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
        (0.55 - (TAME_HEALTH - m.health).max(0.0) * 0.02) * PI
    } else {
        PI / 5.0
    };
    let wag = if angry { 0.0 } else { ls.cos() * 1.4 * la };
    let tail = part(tail_at.0, tail_at.1, tail_at.2, Mat4::from_rotation_y(-wag) * rx(tail_up));

    use skin::*;
    let look = if angry {
        tex::WOLF_ANGRY
    } else if tame {
        tex::WOLF_TAME
    } else {
        tex::WOLF
    };
    let c = |out: &mut Vec<Vertex>, m: Mat4, o: [f32; 3], b: usize| {
        emit_paged(out, m, o, &SKIN, b, 0.0, look, tint, light);
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
        let collar = m.pet().map_or(0, |p| p.collar);
        let col = COLLARS[collar as usize % COLLARS.len()];
        let tinted = std::array::from_fn(|i| (col[i] as u16 * tint[i] as u16 / 255) as u8);
        emit_paged(out, mane, [-3.0, -3.0, -3.0], &SKIN, MANE, 0.25, tex::WOLF_COLLAR, tinted, light);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity::mob::{mob_flags, Foe};

    fn tame(wolf: &mut Mob, name: &str) {
        assert!(wolf.feed_bone(name, true));
    }

    #[test]
    fn a_wolf_is_built_and_a_tame_one_wears_its_collar() {
        let mut wolf = Mob::new(WOLF, Vec3::ZERO, 0.0, 7);
        let mut out = Vec::new();
        wolf.build(&mut out, 15, 0);
        // Head, two ears, snout, body, mane, four legs and the tail.
        assert_eq!(out.len(), 11 * 6 * 6);
        let max_y = out.iter().map(|v| v.pos[1]).fold(f32::MIN, f32::max);
        assert!(max_y > 0.8 && max_y < 1.1, "top at {max_y}");
        tame(&mut wolf, "Alby");
        out.clear();
        wolf.build(&mut out, 15, 0);
        assert_eq!(out.len(), 12 * 6 * 6);
        assert!(out.iter().any(|v| v.layer == tex::WOLF_COLLAR as f32));
        let net = wolf.to_net(None);
        assert!(net.flags & flags::TAME != 0 && net.flags & flags::YOURS == 0);
        assert!(wolf.to_net(Some("Alby")).flags & flags::YOURS != 0);
        let copy = Mob::from_net(&net).expect("a wolf");
        assert!(copy.tame() && copy.kind == WOLF && copy.health == TAME_HEALTH);
    }

    #[test]
    fn a_wolf_bites_what_it_goes_for_but_never_its_owner() {
        let w = World::new();
        let mut wolf = Mob::new(WOLF, Vec3::new(0.5, 70.0, 0.5), 0.0, 7);
        tame(&mut wolf, "Alby");
        wolf.toggle_sit();
        wolf.provoke(Foe::Player(0), Some("Alby"));
        assert!(wolf.foe.is_none(), "turned on its owner");
        wolf.provoke(Foe::Mob(9), None);
        let ctx = MobCtx {
            players: vec![],
            people: vec![(0, "Alby".into(), Vec3::new(3.0, 70.0, 0.5))],
            mobs: vec![(9, Vec3::new(1.2, 70.0, 0.5))],
        };
        // (in an empty world: what matters is the goal)
        assert!(matches!(wolf.update(0.05, &w, &ctx), MobEvent::Attack(Foe::Mob(9))));
        // Sitting, it goes for nothing.
        wolf.toggle_sit();
        assert!(wolf.sitting() && wolf.foe.is_none());
        wolf.provoke(Foe::Mob(9), None);
        assert!(wolf.foe.is_none());
    }

    #[test]
    fn a_sitting_wolf_keeps_watching_its_owner_only_while_they_are_around() {
        let w = World::new();
        let mut wolf = Mob::new(WOLF, Vec3::new(0.5, 70.0, 0.5), 0.0, 7);
        tame(&mut wolf, "Alby");
        let alby = MobCtx { players: vec![Vec3::new(3.0, 70.0, 0.5)], people: vec![(0, "Alby".into(), Vec3::new(3.0, 70.0, 0.5))], mobs: vec![] };
        let nobody = MobCtx { players: vec![], people: vec![], mobs: vec![] };
        for _ in 0..10 {
            wolf.update(0.5, &w, &alby);
            assert!(matches!(wolf.look, Look::Player));
        }
        // Gone: it looks ahead again once its look runs out.
        for _ in 0..10 {
            wolf.update(0.5, &w, &nobody);
        }
        assert!(matches!(wolf.look, Look::Ahead));
    }

    #[test]
    fn a_tame_wolf_catches_up_or_runs_after_its_owner() {
        let w = crate::entity::mob::tests::flat_world();
        let mut wolf = Mob::new(WOLF, Vec3::new(0.5, 64.0, 0.5), 0.0, 7);
        tame(&mut wolf, "Alby");
        wolf.toggle_sit();
        let far = Vec3::new(14.5, 64.0, 14.5);
        let ctx = MobCtx { players: vec![far], people: vec![(0, "Alby".into(), far)], mobs: vec![] };
        assert!(matches!(think(&mut wolf, 0.05, &w, &ctx), Some(None)));
        assert!(wolf.pos.distance(far) < 3.5, "left behind at {}", wolf.pos);
        // Flying high over it: nowhere to jump to, so it runs after them.
        wolf.pos = Vec3::new(0.5, 64.0, 0.5);
        let up = Vec3::new(14.5, 90.0, 14.5);
        let ctx = MobCtx { players: vec![up], people: vec![(0, "Alby".into(), up)], mobs: vec![] };
        think(&mut wolf, 0.05, &w, &ctx);
        assert_eq!(wolf.target, Some(up));
        assert!(wolf.pos.y < 65.0);
    }

    #[test]
    fn a_bone_tames_only_a_wild_calm_wolf() {
        let mut wolf = Mob::new(WOLF, Vec3::ZERO, 0.0, 7);
        wolf.foe = Some(Foe::Player(1));
        assert!(use_on(&mut wolf, BONE, true).is_none(), "an angry one");
        // A player's copy knows it is angry from the server.
        let mut copy = Mob::from_net(&wolf.to_net(None)).unwrap();
        assert!(copy.to_net(None).flags & mob_flags::ANGRY == 0 && copy.angry());
        assert!(use_on(&mut copy, BONE, true).is_none());
        wolf.foe = None;
        assert_eq!(use_on(&mut wolf, BONE, true), Some(Use { consume: true, wear: 0 }));
        assert!(used(&mut wolf, BONE, "Alby", 0.9).fx.is_some() && !wolf.tame());
        assert!(used(&mut wolf, BONE, "Alby", 0.1).fx.is_some() && wolf.owner() == Some("Alby"));
        assert!(wolf.sitting());
        used(&mut wolf, BONE, "Alby", 0.1);
        assert!(!wolf.sitting());
        used(&mut wolf, BONE, "Bob", 0.1);
        assert!(!wolf.sitting() && wolf.owner() == Some("Alby"));
    }
}
