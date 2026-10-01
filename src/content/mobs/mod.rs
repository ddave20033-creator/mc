//! Every mob: a file each in this folder (the pig, the sheep, the target dummy, the wolf).
//! A file gives its mob's line of the table (`MobDef`: its key, names, health, size, speed,
//! attack, loot, where it spawns, its sounds and its model) and whatever it does that its
//! behaviour template does not (its `Hooks`, its own state in `MobState`).
//!
//! The templates (`PASSIVE`, `NEUTRAL`, `HOSTILE`, `STATIC`) are whole lines to start a mob
//! from (`..PASSIVE`), each with its AI's parameters (`Ai`):
//! - passive: never hurts anyone; runs away when hurt (the pig, the sheep);
//! - neutral: minds its own business until it is hurt, then goes for whoever did it, and
//!   so do others of its kind nearby (the wolf);
//! - hostile: goes for any player it sees;
//! - static: does not move at all, only falls (the target dummy).
//!
//! The AI itself (`entity::mob`) runs the template and asks the hooks first. Ids follow the
//! order of `MOBS`; save files keep the keys, so the order can change.

pub mod pig;
pub mod sheep;
pub mod target_dummy;
pub mod wolf;

pub use target_dummy::TARGET_DUMMY;

use crate::audio::Sound;
use crate::entity::mob::{Mob, MobCtx, MobEvent, Steer};
use crate::item::{ItemId, Stack};
use crate::world::gen::Biome;
use crate::world::mesh::Vertex;
use crate::world::{Block, World, GRASS};
use glam::Vec3;

/// A kind of mob: its line in `MOBS`.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub struct MobKind(pub u8);

impl MobKind {
    /// Its line of the table.
    pub fn def(self) -> &'static MobDef {
        &MOBS[self.0 as usize]
    }

    pub fn key(self) -> &'static str {
        self.def().key
    }

    /// The kind with this key (`minecraft:` in front is fine).
    pub fn from_key(k: &str) -> Option<MobKind> {
        let k = k.strip_prefix("minecraft:").unwrap_or(k);
        MOBS.iter().find(|d| d.key == k).map(|d| d.kind)
    }

    /// The kind with this number (network messages).
    pub fn from_u8(v: u8) -> Option<MobKind> {
        ((v as usize) < MOBS.len()).then_some(MobKind(v))
    }

    /// The kind this item puts into the world (a spawn egg, a target dummy).
    pub fn by_egg(item: ItemId) -> Option<MobKind> {
        MOBS.iter().find(|d| d.egg == item).map(|d| d.kind)
    }

    /// Its name in the current language.
    pub fn name(self) -> &'static str {
        let d = self.def();
        if crate::app::lang::is_hungarian() {
            d.hu
        } else {
            d.en
        }
    }
}

/// How a mob behaves toward players: its template.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Behavior {
    Passive,
    Neutral,
    #[allow(dead_code)]
    Hostile,
    Static,
}

/// The AI's parameters (distances in blocks, times in seconds, speeds times the walking
/// speed). The templates' are in `PASSIVE`, `NEUTRAL`, `HOSTILE` and `STATIC`.
#[derive(Clone, Copy, Debug)]
pub struct Ai {
    /// Moves on its own at all (a static one only falls and is never pushed).
    pub moves: bool,
    /// Strolls to random spots now and then (Minecraft's RandomStrollGoal).
    pub wander: bool,
    /// Runs away for a few seconds when hurt (Minecraft's PanicGoal).
    pub panics: bool,
    /// Goes for whoever hurts it; others of its kind within `alert` join in.
    pub retaliates: bool,
    pub alert: f32,
    /// Goes for any player within this range on its own (0: never).
    pub hunts: f32,
    /// How long it stays after someone (a pet: until they are gone).
    pub anger: f32,
    /// How fast it runs after them, from how near it attacks, and how often.
    pub chase: f32,
    pub reach: f32,
    pub attack_every: f32,
    /// Looks at players this near now and then.
    pub looks: f32,
}

/// A mob's attack: its damage and what kind of hurt it is (`net::hurt`).
#[derive(Clone, Copy, Debug)]
pub struct Attack {
    pub damage: f32,
    pub kind: u8,
}

/// Something a dead mob drops: `min..=max` of `item` (`burnt` instead if it died on fire),
/// with a chance.
#[derive(Clone, Copy, Debug)]
pub struct Loot {
    pub item: ItemId,
    pub burnt: Option<ItemId>,
    pub min: u8,
    pub max: u8,
    pub chance: f32,
}

/// Where and how a mob appears on its own: its weight among the mobs that may appear there,
/// how many come together, the ground under them (the top block of its column), the
/// biomes (none listed: any), and the least light there.
#[derive(Clone, Copy, Debug)]
pub struct Spawn {
    pub weight: u32,
    pub group: (u8, u8),
    pub ground: &'static [Block],
    pub biomes: &'static [Biome],
    pub min_light: u8,
}

/// Its sounds: when hurt, and now and then (`idle` picks one, or none) every `every` seconds
/// (the least and the most).
#[derive(Clone, Copy)]
pub struct Sounds {
    pub hurt: Option<Sound>,
    pub idle: Option<fn(&mut Mob) -> Option<Sound>>,
    pub every: (f32, f32),
}

/// What a player's right click with an item did on their side (the server decides: `used`):
/// the item is used up (one), or worn by `wear`.
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub struct Use {
    pub consume: bool,
    pub wear: u16,
}

/// What the server does after a player used an item on a mob: an effect (`net::fx`) and
/// something dropped, where.
#[derive(Clone, Copy, Default)]
pub struct Used {
    pub fx: Option<(u8, Vec3)>,
    pub drop: Option<(Vec3, Stack)>,
}

/// A mob's own behaviour, on top of its template's. Every one may be left out.
#[derive(Clone, Copy)]
pub struct Hooks {
    /// Its own goals, asked before the template's every AI step: Some decides what it does
    /// now (`Steer`), None leaves it to the template.
    pub think: Option<fn(&mut Mob, f32, &World, &MobCtx) -> Option<Steer>>,
    /// After each step of its physics (whether it was steering): grazing, a dummy rocking...
    pub tick: Option<fn(&mut Mob, f32, &World, bool) -> MobEvent>,
    /// Hit (amount, from where, knockback) instead of the usual: whether it took the hit.
    pub hurt: Option<fn(&mut Mob, f32, Option<Vec3>, f32) -> bool>,
    /// A player's right click with the item they hold (`fresh`: not the button held down),
    /// on their side: None if nothing happens. It may already show what will happen.
    pub use_on: Option<fn(&mut Mob, ItemId, bool) -> Option<Use>>,
    /// The same on the server (by the player with this name; a random number in 0..1).
    pub used: Option<fn(&mut Mob, ItemId, &str, f32) -> Used>,
    /// What it drops dead besides its `loot`.
    pub loot: Option<fn(&Mob) -> Vec<Stack>>,
}

pub const NO_HOOKS: Hooks = Hooks { think: None, tick: None, hurt: None, use_on: None, used: None, loot: None };

/// A mob's own state, besides what every mob has (`Mob`).
#[derive(Clone, Debug, PartialEq)]
pub enum MobState {
    None,
    Sheep(sheep::Wool),
    Dummy(target_dummy::Tally),
    Wolf(wolf::Pet),
}

/// A mob: one line of the table.
#[derive(Clone, Copy)]
pub struct MobDef {
    pub kind: MobKind,
    pub key: &'static str,
    pub en: &'static str,
    pub hu: &'static str,
    pub behavior: Behavior,
    pub ai: Ai,
    pub health: f32,
    /// Half width and height of its box.
    pub size: (f32, f32),
    /// Walking speed (blocks per second).
    pub speed: f32,
    pub attack: Option<Attack>,
    pub loot: &'static [Loot],
    /// None: it only appears from its egg.
    pub spawn: Option<Spawn>,
    pub sounds: Sounds,
    /// Its model: builds the mob into the vertices, with this light (`util::vertex_light`).
    pub model: fn(&Mob, &mut Vec<Vertex>, [u8; 4]),
    /// The item that puts it into the world (its spawn egg); a static one taken down drops it.
    pub egg: ItemId,
    /// Its own state when it appears.
    pub state: fn() -> MobState,
    pub hooks: Hooks,
}

fn no_state() -> MobState {
    MobState::None
}

fn no_model(_: &Mob, _: &mut Vec<Vertex>, _: [u8; 4]) {}

/// Minecraft's walking speed for animals (blocks per second).
pub const WALK_SPEED: f32 = 1.7;

/// Can't hurt anyone: strolls around, looks at players and runs away when hurt.
pub const PASSIVE: MobDef = MobDef {
    kind: MobKind(0),
    key: "",
    en: "",
    hu: "",
    behavior: Behavior::Passive,
    ai: Ai {
        moves: true,
        wander: true,
        panics: true,
        retaliates: false,
        alert: 0.0,
        hunts: 0.0,
        anger: 0.0,
        chase: 1.0,
        reach: 0.0,
        attack_every: 1.0,
        looks: 6.0,
    },
    health: 10.0,
    size: (0.45, 0.9),
    speed: WALK_SPEED,
    attack: None,
    loot: &[],
    spawn: None,
    sounds: Sounds { hurt: None, idle: None, every: (4.0, 10.0) },
    model: no_model,
    egg: 0,
    state: no_state,
    hooks: NO_HOOKS,
};

/// Only fights back: whoever hurts it (or one of its kind near it) is gone for, and bitten.
pub const NEUTRAL: MobDef = MobDef {
    behavior: Behavior::Neutral,
    ai: Ai {
        panics: false,
        retaliates: true,
        alert: 12.0,
        anger: 25.0,
        chase: 2.1,
        reach: 1.5,
        attack_every: 1.0,
        ..PASSIVE.ai
    },
    attack: Some(Attack { damage: 2.0, kind: crate::net::hurt::MELEE }),
    ..PASSIVE
};

/// Hostile: goes for any player near it, and fights back.
#[allow(dead_code)] // (no mob is hostile yet)
pub const HOSTILE: MobDef = MobDef {
    behavior: Behavior::Hostile,
    ai: Ai { hunts: 16.0, alert: 0.0, looks: 8.0, chase: 1.3, ..NEUTRAL.ai },
    ..NEUTRAL
};

/// Stands where it was put: never moves or is pushed, only falls.
pub const STATIC: MobDef = MobDef {
    behavior: Behavior::Static,
    ai: Ai { moves: false, wander: false, panics: false, looks: 0.0, ..PASSIVE.ai },
    ..PASSIVE
};

/// Every mob, in the order of their kinds.
pub static MOBS: [MobDef; 4] = [pig::DEF, sheep::DEF, target_dummy::DEF, wolf::DEF];

/// Grass: where most animals appear.
pub(super) const GRASSY: &[Block] = &[GRASS];

impl MobState {
    /// Its greatest health, if not the table's (a tame wolf's is more).
    pub fn max_health(&self) -> Option<f32> {
        match self {
            MobState::Wolf(p) => p.max_health(),
            _ => None,
        }
    }

    /// Its state in a save file (without `:` or line breaks).
    pub fn save(&self) -> String {
        match self {
            MobState::None => String::new(),
            MobState::Sheep(s) => s.save(),
            MobState::Dummy(_) => String::new(),
            MobState::Wolf(p) => p.save(),
        }
    }

    /// Reads back `save`'s text (what does not make sense is left as it is).
    pub fn load(&mut self, s: &str) {
        match self {
            MobState::Sheep(w) => w.load(s),
            MobState::Wolf(p) => p.load(s),
            MobState::None | MobState::Dummy(_) => {}
        }
    }

    /// Into what a player gets of the mob (`to`: that player's name).
    pub fn to_net(&self, net: &mut crate::net::MobNet, to: Option<&str>) {
        match self {
            MobState::None => {}
            MobState::Sheep(s) => net.sheared = s.sheared,
            MobState::Dummy(t) => t.to_net(net),
            MobState::Wolf(p) => p.to_net(net, to),
        }
    }

    /// A player's copy: from what the server sent.
    pub fn apply_net(&mut self, net: &crate::net::MobNet) {
        match self {
            MobState::None => {}
            MobState::Sheep(s) => s.sheared = net.sheared,
            MobState::Dummy(t) => t.apply_net(net),
            MobState::Wolf(p) => p.apply_net(net),
        }
    }
}

/// Text for a save file's line (a player's name): no `:`, `,` or line breaks in it (they are
/// written as `%3A`...; `%` itself as `%25`).
pub fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '%' | ':' | ',' | '\n' | '\r' => out += &format!("%{:02X}", c as u32),
            c => out.push(c),
        }
    }
    out
}

/// `escape`'s text back as it was.
pub fn unescape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find('%') {
        out += &rest[..i];
        let code = rest.get(i + 1..i + 3).and_then(|h| u8::from_str_radix(h, 16).ok());
        match code {
            Some(b) => {
                out.push(b as char);
                rest = &rest[i + 3..];
            }
            None => {
                out.push('%');
                rest = &rest[i + 1..];
            }
        }
    }
    out + rest
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An item the game knows (by its key).
    fn exists(id: ItemId) -> bool {
        let k = crate::item::key(id);
        !k.is_empty() && crate::item::from_key(&k) == Some(id)
    }

    #[test]
    fn the_table_is_consistent() {
        let mut keys = std::collections::HashSet::new();
        let mut eggs = std::collections::HashSet::new();
        for (i, d) in MOBS.iter().enumerate() {
            assert_eq!(d.kind, MobKind(i as u8), "{} is out of order", d.key);
            assert!(!d.key.is_empty() && keys.insert(d.key), "key {:?}", d.key);
            assert!(!d.en.is_empty() && !d.hu.is_empty(), "{} has no name", d.key);
            assert_eq!(MobKind::from_key(d.key), Some(d.kind));
            assert_eq!(MobKind::from_u8(i as u8), Some(d.kind));
            assert!(d.health > 0.0 && d.size.0 > 0.0 && d.size.1 > 0.0, "{}", d.key);
            // Its egg is a real item, and puts only it into the world.
            assert!(eggs.insert(d.egg) && exists(d.egg), "{}'s egg", d.key);
            assert_eq!(MobKind::by_egg(d.egg), Some(d.kind));
            for l in d.loot {
                assert!(exists(l.item), "{} drops an unknown item", d.key);
                assert!(l.burnt.is_none_or(exists), "{} drops an unknown item", d.key);
                assert!(l.min >= 1 && l.min <= l.max && l.chance > 0.0, "{}", d.key);
            }
            if let Some(s) = d.spawn {
                assert!(s.weight > 0 && s.group.0 >= 1 && s.group.0 <= s.group.1 && !s.ground.is_empty(), "{}", d.key);
            }
            if d.behavior != Behavior::Passive && d.behavior != Behavior::Static {
                assert!(d.attack.is_some(), "{} fights without an attack", d.key);
            }
        }
        assert_eq!(MobKind::from_key("minecraft:pig"), Some(pig::PIG));
        assert_eq!(MobKind::from_u8(MOBS.len() as u8), None);
    }

    #[test]
    fn every_mob_has_a_model() {
        for d in &MOBS {
            let m = Mob::new(d.kind, Vec3::ZERO, 0.0, 1);
            let mut out = Vec::new();
            (d.model)(&m, &mut out, [15, 15, 15, 15]);
            assert!(!out.is_empty() && out.len() % 3 == 0, "{} has no model", d.key);
        }
    }

    #[test]
    fn states_survive_a_save() {
        for d in &MOBS {
            let mut m = Mob::new(d.kind, Vec3::ZERO, 0.0, 3);
            match &mut m.state {
                MobState::Sheep(s) => s.sheared = true,
                MobState::Wolf(p) => {
                    // (a name with the save file's separators in it)
                    p.owner = Some("Al:by, 100%\nx".into());
                    p.sitting = true;
                    p.collar = 5;
                }
                _ => {}
            }
            let mut back = (d.state)();
            back.load(&m.state.save());
            if let MobState::Dummy(_) = back {
                continue;
            }
            assert_eq!(back, m.state, "{}", d.key);
        }
    }
}
