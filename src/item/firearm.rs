//! The guns: what kinds there are, how each shoots (damage, rate of fire, magazine, spread,
//! recoil, reach) and what its five parts are made of at the gun station.

use super::*;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum GunKind {
    Pistol,
    DesertEagle,
    M16,
    Sniper,
    Shotgun,
}

pub const GUN_KINDS: [GunKind; 5] = [
    GunKind::Pistol,
    GunKind::DesertEagle,
    GunKind::M16,
    GunKind::Sniper,
    GunKind::Shotgun,
];

/// How a gun shoots.
pub struct Stats {
    /// Damage of one bullet (or of each pellet), and how hard it knocks back.
    pub damage: f32,
    pub knockback: f32,
    /// Bullets per shot (pellets of a shotgun shell).
    pub pellets: u8,
    /// Seconds between shots; `auto` fires while the button is held.
    pub fire_delay: f32,
    pub auto: bool,
    /// Rounds in the magazine, and with the extended magazine.
    pub magazine: u8,
    pub extended: u8,
    /// Cone of the shots (degrees): from the hip, from the hip with the laser, aimed.
    pub spread_hip: f32,
    pub spread_laser: f32,
    pub spread_aimed: f32,
    /// How far the view kicks up per shot (degrees), from the hip and aimed.
    pub kick_hip: f32,
    pub kick_aimed: f32,
    /// Bullet speed, the pull bending it down (blocks, seconds) and how far it flies.
    pub speed: f32,
    pub gravity: f32,
    pub range: f32,
    /// Seconds to reload: the whole magazine, or with `shells` each round on its own.
    pub reload: f32,
    pub shells: bool,
    /// Seconds to work the bolt or the pump after every shot (0: it cycles by itself).
    pub cycle: f32,
    /// How much the view narrows when aimed: with the sights and through a scope.
    pub sight_zoom: f32,
    pub scope_zoom: f32,
    /// Shots until it is too dirty to fire.
    pub dirt_max: u16,
    /// Always has a scope (it cannot be fitted or taken off).
    pub builtin_scope: bool,
    /// How big its muzzle flash is.
    pub flash: f32,
}

const PISTOL_STATS: Stats = Stats {
    damage: 7.0,
    knockback: 0.5,
    pellets: 1,
    fire_delay: 0.18,
    auto: false,
    magazine: 12,
    extended: 20,
    spread_hip: 2.4,
    spread_laser: 0.7,
    spread_aimed: 0.12,
    kick_hip: 1.4,
    kick_aimed: 0.8,
    speed: 180.0,
    gravity: 12.0,
    range: 80.0,
    reload: 2.0,
    shells: false,
    cycle: 0.0,
    sight_zoom: 0.78,
    scope_zoom: 0.25,
    dirt_max: 40,
    builtin_scope: false,
    flash: 1.0,
};

const DEAGLE_STATS: Stats = Stats {
    damage: 14.0,
    knockback: 1.0,
    fire_delay: 0.34,
    magazine: 7,
    extended: 10,
    spread_hip: 3.0,
    spread_laser: 1.0,
    spread_aimed: 0.2,
    kick_hip: 4.5,
    kick_aimed: 3.0,
    speed: 200.0,
    range: 90.0,
    reload: 2.2,
    sight_zoom: 0.75,
    dirt_max: 30,
    flash: 1.6,
    ..PISTOL_STATS
};

const M16_STATS: Stats = Stats {
    damage: 6.0,
    knockback: 0.3,
    fire_delay: 0.085,
    auto: true,
    magazine: 30,
    extended: 45,
    spread_hip: 3.2,
    spread_laser: 1.2,
    spread_aimed: 0.25,
    kick_hip: 0.9,
    kick_aimed: 0.5,
    speed: 250.0,
    gravity: 9.0,
    range: 130.0,
    reload: 2.4,
    sight_zoom: 0.7,
    scope_zoom: 0.3,
    dirt_max: 150,
    flash: 1.2,
    ..PISTOL_STATS
};

const SNIPER_STATS: Stats = Stats {
    damage: 40.0,
    knockback: 1.6,
    fire_delay: 0.25,
    magazine: 5,
    extended: 8,
    spread_hip: 6.0,
    spread_laser: 3.0,
    spread_aimed: 0.0,
    kick_hip: 8.0,
    kick_aimed: 5.0,
    speed: 330.0,
    gravity: 6.0,
    range: 250.0,
    reload: 3.0,
    cycle: 1.1,
    sight_zoom: 0.7,
    scope_zoom: 0.12,
    dirt_max: 25,
    builtin_scope: true,
    flash: 2.2,
    ..PISTOL_STATS
};

const SHOTGUN_STATS: Stats = Stats {
    damage: 3.5,
    knockback: 0.6,
    pellets: 8,
    fire_delay: 0.2,
    magazine: 6,
    extended: 9,
    spread_hip: 6.5,
    spread_laser: 5.0,
    spread_aimed: 3.5,
    kick_hip: 5.0,
    kick_aimed: 3.5,
    speed: 120.0,
    gravity: 14.0,
    range: 40.0,
    reload: 0.55,
    shells: true,
    cycle: 0.6,
    sight_zoom: 0.85,
    scope_zoom: 0.4,
    dirt_max: 60,
    flash: 1.9,
    ..PISTOL_STATS
};

/// A part of a gun: its name (translation key) and what makes it at the gun station: an
/// item of its own (the pistol's), or materials.
pub struct Part {
    pub name: &'static str,
    pub item: Option<ItemId>,
    pub cost: &'static [(ItemId, u8)],
}

const fn made(name: &'static str, cost: &'static [(ItemId, u8)]) -> Part {
    Part {
        name,
        item: None,
        cost,
    }
}

const fn crafted(name: &'static str, item: ItemId) -> Part {
    Part {
        name,
        item: Some(item),
        cost: &[],
    }
}

const PLANK: ItemId = PLANKS as ItemId;

impl GunKind {
    pub fn item(self) -> ItemId {
        match self {
            GunKind::Pistol => PISTOL,
            GunKind::DesertEagle => DESERT_EAGLE,
            GunKind::M16 => M16,
            GunKind::Sniper => SNIPER_RIFLE,
            GunKind::Shotgun => SHOTGUN,
        }
    }

    /// The gun an item is.
    pub fn of(item: ItemId) -> Option<GunKind> {
        GUN_KINDS.into_iter().find(|k| k.item() == item)
    }

    /// What it fires.
    pub fn ammo(self) -> ItemId {
        match self {
            GunKind::Pistol => BULLET,
            GunKind::DesertEagle => MAGNUM_ROUND,
            GunKind::M16 => RIFLE_ROUND,
            GunKind::Sniper => BMG_ROUND,
            GunKind::Shotgun => SHOTGUN_SHELL,
        }
    }

    pub fn stats(self) -> &'static Stats {
        match self {
            GunKind::Pistol => &PISTOL_STATS,
            GunKind::DesertEagle => &DEAGLE_STATS,
            GunKind::M16 => &M16_STATS,
            GunKind::Sniper => &SNIPER_STATS,
            GunKind::Shotgun => &SHOTGUN_STATS,
        }
    }

    /// Rounds the magazine holds with these attachments.
    pub fn magazine_size(self, mods: u8) -> u8 {
        let s = self.stats();
        if mods & gun_mod::EXTENDED_MAGAZINE != 0 {
            s.extended
        } else {
            s.magazine
        }
    }

    /// Attachments that can be fitted (a built-in scope cannot be changed).
    pub fn fits(self, bit: u8) -> bool {
        !(bit == gun_mod::SCOPE && self.stats().builtin_scope)
    }

    /// The attachments as the model shows them (with a built-in scope).
    pub fn shown_mods(self, mods: u8) -> u8 {
        if self.stats().builtin_scope {
            mods | gun_mod::SCOPE
        } else {
            mods
        }
    }

    /// The five parts, in the order they go together.
    pub fn parts(self) -> &'static [Part; 5] {
        static PISTOL_PARTS_LIST: [Part; 5] = [
            crafted("gun.part.frame", PISTOL_FRAME),
            crafted("gun.part.barrel", PISTOL_BARREL),
            crafted("gun.part.spring", PISTOL_SPRING),
            crafted("gun.part.slide", PISTOL_SLIDE),
            crafted("gun.part.magazine", PISTOL_MAGAZINE),
        ];
        static DEAGLE_PARTS: [Part; 5] = [
            made("gun.part.frame", &[(IRON_INGOT, 3)]),
            made("gun.part.barrel", &[(IRON_INGOT, 2), (GOLD_INGOT, 1)]),
            made("gun.part.spring", &[(IRON_NUGGET, 4)]),
            made("gun.part.slide", &[(IRON_INGOT, 4)]),
            made("gun.part.magazine", &[(IRON_INGOT, 2)]),
        ];
        static M16_PARTS: [Part; 5] = [
            made("gun.part.lower", &[(IRON_INGOT, 4)]),
            made("gun.part.barrel", &[(IRON_INGOT, 3)]),
            made("gun.part.bolt", &[(IRON_INGOT, 1), (IRON_NUGGET, 4)]),
            made("gun.part.upper", &[(IRON_INGOT, 4)]),
            made("gun.part.magazine", &[(IRON_INGOT, 2)]),
        ];
        static SNIPER_PARTS: [Part; 5] = [
            made("gun.part.stock", &[(IRON_INGOT, 5)]),
            made("gun.part.barrel", &[(IRON_INGOT, 4), (DIAMOND, 1)]),
            made("gun.part.bolt", &[(IRON_INGOT, 2)]),
            made("gun.part.scoped_upper", &[(IRON_INGOT, 4), (GLASS as ItemId, 2)]),
            made("gun.part.magazine", &[(IRON_INGOT, 2)]),
        ];
        static SHOTGUN_PARTS: [Part; 5] = [
            made("gun.part.receiver", &[(IRON_INGOT, 3)]),
            made("gun.part.barrel", &[(IRON_INGOT, 3)]),
            made("gun.part.tube", &[(IRON_INGOT, 2)]),
            made("gun.part.pump", &[(PLANK, 2)]),
            made("gun.part.stock", &[(PLANK, 4)]),
        ];
        match self {
            GunKind::Pistol => &PISTOL_PARTS_LIST,
            GunKind::DesertEagle => &DEAGLE_PARTS,
            GunKind::M16 => &M16_PARTS,
            GunKind::Sniper => &SNIPER_PARTS,
            GunKind::Shotgun => &SHOTGUN_PARTS,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_gun_has_its_own_item_and_ammo() {
        for k in GUN_KINDS {
            assert_eq!(GunKind::of(k.item()), Some(k));
            assert_eq!(max_stack(k.item()), 1);
            assert_eq!(max_damage(k.item()), k.stats().dirt_max);
            assert!(k.magazine_size(gun_mod::EXTENDED_MAGAZINE) > k.magazine_size(0));
            // Rounds are kept in 6 bits of the item's data.
            assert!(k.magazine_size(gun_mod::EXTENDED_MAGAZINE) < 64);
            assert!(GunKind::of(k.ammo()).is_none() && max_stack(k.ammo()) == 64);
        }
        assert!(!GunKind::Sniper.fits(gun_mod::SCOPE));
        assert!(GunKind::M16.fits(gun_mod::SCOPE));
    }
}
