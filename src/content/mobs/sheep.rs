//! The sheep: a passive animal that wears wool. Shears take it off (1-3 wool), and it grows
//! back when the sheep grazes (Minecraft's EatBlockGoal: now and then, standing, it eats the
//! tall grass it stands in or the grass block under it). Dead, it drops mutton and, unless
//! sheared, a wool.

use super::*;
use crate::entity::mob::{animal_root, emit_paged, head_turn, part, per_tick, quadruped_legs};
use crate::item::{BONE, COOKED_MUTTON, MUTTON, SHEARS, SHEEP_SPAWN_EGG};
use crate::textures::tex;
use crate::world::{TALL_GRASS, WOOL};
use glam::{IVec3, Mat4};
use std::f32::consts::FRAC_PI_2;

pub const SHEEP: MobKind = MobKind(1);

pub const DEF: MobDef = MobDef {
    kind: SHEEP,
    key: "sheep",
    en: "Sheep",
    hu: "Birka",
    health: 8.0,
    // (Minecraft: 0.9 x 1.3)
    size: (0.45, 1.3),
    loot: &[
        Loot { item: MUTTON, burnt: Some(COOKED_MUTTON), min: 1, max: 2, chance: 1.0 },
        Loot { item: BONE, burnt: None, min: 1, max: 1, chance: 1.0 / 3.0 },
    ],
    spawn: Some(Spawn { weight: 12, group: (2, 4), ground: GRASSY, biomes: &[], min_light: 9 }),
    model,
    egg: SHEEP_SPAWN_EGG,
    state: || MobState::Sheep(Wool::default()),
    hooks: Hooks { tick: Some(graze), use_on: Some(shear), used: Some(sheared), loot: Some(wool_loot), ..NO_HOOKS },
    ..PASSIVE
};

/// A sheep's coat.
#[derive(Clone, Copy, Default, Debug, PartialEq)]
pub struct Wool {
    /// Without its wool.
    pub sheared: bool,
}

impl Wool {
    pub fn save(&self) -> String {
        (self.sheared as u8).to_string()
    }

    pub fn load(&mut self, s: &str) {
        self.sheared = s == "1";
    }
}

impl Mob {
    /// A sheep that still has its wool.
    pub fn can_shear(&self) -> bool {
        matches!(self.state, MobState::Sheep(Wool { sheared: false })) && self.alive()
    }

    fn set_sheared(&mut self, sheared: bool) {
        if let MobState::Sheep(w) = &mut self.state {
            w.sheared = sheared;
        }
    }
}

/// Grazes now and then (1 in 1000 per tick) when it stands about, not panicking; its wool
/// grows back.
fn graze(m: &mut Mob, dt: f32, w: &World, steering: bool) -> MobEvent {
    if !m.alive() || !m.on_ground || steering || m.panic > 0.0 || m.rand() >= per_tick(1.0 / 1000.0, dt) {
        return MobEvent::None;
    }
    let feet = m.pos.floor().as_ivec3();
    let eat = if w.geti(feet) == TALL_GRASS {
        Some(feet)
    } else {
        (w.geti(feet - IVec3::Y) == GRASS).then(|| feet - IVec3::Y)
    };
    match eat {
        Some(p) => {
            m.set_sheared(false);
            MobEvent::EatGrass(p)
        }
        None => MobEvent::None,
    }
}

/// Shears (held down is fine): its wool comes off, and the shears wear.
fn shear(m: &mut Mob, held: ItemId, _fresh: bool) -> Option<Use> {
    if held != SHEARS || !m.can_shear() {
        return None;
    }
    m.set_sheared(true);
    Some(Use { consume: false, wear: 1 })
}

/// The server's side: 1-3 wool pops off.
fn sheared(m: &mut Mob, held: ItemId, _who: &str, r: f32) -> Used {
    if held != SHEARS || !m.can_shear() {
        return Used::default();
    }
    m.set_sheared(true);
    let n = 1 + (r * 3.0) as u8;
    Used { fx: None, drop: Some((m.pos + Vec3::Y, Stack::new(WOOL as ItemId, n.min(3)))) }
}

/// Its wool, unless sheared.
fn wool_loot(m: &Mob) -> Vec<Stack> {
    match m.state {
        MobState::Sheep(Wool { sheared: false }) => vec![Stack::one(WOOL as ItemId)],
        _ => Vec::new(),
    }
}

/// The sheep's skin and its wool coat (Minecraft's 64x32 unit atlases at 8 texels per unit,
/// 512x256) on `PAGES` and `WOOL_PAGES` texture layers.
pub mod skin {
    use crate::textures::skin_pages::{BoxUv, SkinPages};

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

/// Minecraft's `SheepModel` (a `QuadrupedModel` with leg height 12) and, unless sheared,
/// `SheepFurModel` over it: the same parts grown a little, with the wool texture.
fn model(m: &Mob, out: &mut Vec<Vertex>, light: [u8; 4]) {
    use skin::*;
    let p = &m.pose();
    let (root, tint) = (animal_root(p), m.tint());
    let head = part(root, 0.0, 6.0, -8.0, head_turn(p));
    let body = part(root, 0.0, 5.0, 2.0, Mat4::from_rotation_x(-FRAC_PI_2));
    let legs = quadruped_legs(root, p, 12.0);
    let skin = |out: &mut Vec<Vertex>, m: Mat4, o: [f32; 3], b: usize| {
        emit_paged(out, m, o, &SKIN, b, 0.0, tex::SHEEP, tint, light);
    };
    skin(out, head, [-3.0, -4.0, -6.0], HEAD);
    skin(out, body, [-4.0, -10.0, -7.0], BODY);
    for leg in legs {
        skin(out, leg, [-2.0, 0.0, -2.0], LEG);
    }
    if !matches!(m.state, MobState::Sheep(Wool { sheared: false })) {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sheep_wears_its_wool_until_sheared() {
        let mut sheep = Mob::new(SHEEP, Vec3::ZERO, 0.0, 7);
        let mut out = Vec::new();
        sheep.build(&mut out, 15, 0);
        // Head, body and four legs, each with a wool cube over it.
        assert_eq!(out.len(), 12 * 6 * 6);
        let max_y = out.iter().map(|v| v.pos[1]).fold(f32::MIN, f32::max);
        assert!(max_y > 1.3 && max_y < 1.5, "top at {max_y}");
        assert!(sheep.can_shear());
        assert_eq!(wool_loot(&sheep).len(), 1);
        assert_eq!(shear(&mut sheep, SHEARS, false), Some(Use { consume: false, wear: 1 }));
        out.clear();
        sheep.build(&mut out, 15, 0);
        assert_eq!(out.len(), 6 * 6 * 6);
        assert!(!sheep.can_shear() && wool_loot(&sheep).is_empty());
        assert!(shear(&mut sheep, SHEARS, true).is_none());
        let net = sheep.to_net(None);
        assert!(Mob::from_net(&net).is_some_and(|m| !m.can_shear() && m.kind == SHEEP));
    }
}
