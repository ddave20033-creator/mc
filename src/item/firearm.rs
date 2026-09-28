//! The guns: what kinds there are, how each shoots (damage, rate of fire, magazine, spread,
//! recoil, reach) and what its five parts are at the gun station: the pistol, the revolver
//! (loaded straight from the bullets carried) and the AK-47 (a big-calibre automatic rifle).

use super::*;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum GunKind {
    Pistol,
    Revolver,
    Ak,
}

/// Every gun, in the order of `kind as u8` (LAN messages).
pub const GUN_KINDS: [GunKind; 3] = [GunKind::Pistol, GunKind::Revolver, GunKind::Ak];

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
    /// Seconds to reload the whole magazine.
    pub reload: f32,
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
    sight_zoom: 0.78,
    scope_zoom: 0.45,
    dirt_max: 240,
    builtin_scope: false,
    flash: 1.0,
};

/// Heavier rounds, slower double action, harder kick; six in the cylinder.
const REVOLVER_STATS: Stats = Stats {
    damage: 12.0,
    knockback: 0.9,
    pellets: 1,
    fire_delay: 0.42,
    auto: false,
    magazine: 6,
    extended: 6,
    spread_hip: 2.4,
    spread_laser: 0.7,
    spread_aimed: 0.1,
    kick_hip: 3.0,
    kick_aimed: 1.8,
    speed: 200.0,
    gravity: 10.0,
    range: 90.0,
    reload: 2.3,
    sight_zoom: 0.76,
    scope_zoom: 0.45,
    dirt_max: 300,
    builtin_scope: false,
    flash: 1.3,
};

/// 7.62x39 mm: hits hard and flies far and flat; fully automatic, 600 rounds a minute, a
/// 30-round magazine; it kicks with every shot, so long bursts climb.
const AK_STATS: Stats = Stats {
    damage: 11.0,
    knockback: 0.7,
    pellets: 1,
    fire_delay: 0.1,
    auto: true,
    magazine: 30,
    extended: 40,
    spread_hip: 3.2,
    spread_laser: 1.2,
    spread_aimed: 0.18,
    kick_hip: 1.3,
    kick_aimed: 0.75,
    speed: 300.0,
    gravity: 7.0,
    range: 160.0,
    reload: 2.0,
    sight_zoom: 0.7,
    scope_zoom: 0.4,
    dirt_max: 900,
    builtin_scope: false,
    flash: 1.6,
};

impl GunKind {
    pub fn item(self) -> ItemId {
        match self {
            GunKind::Pistol => PISTOL,
            GunKind::Revolver => REVOLVER,
            GunKind::Ak => AK47,
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
            GunKind::Revolver => MAGNUM_ROUND,
            GunKind::Ak => RIFLE_ROUND,
        }
    }

    pub fn stats(self) -> &'static Stats {
        match self {
            GunKind::Pistol => &PISTOL_STATS,
            GunKind::Revolver => &REVOLVER_STATS,
            GunKind::Ak => &AK_STATS,
        }
    }

    /// Takes magazines (the pistol, the AK); otherwise it is loaded round by round from the
    /// bullets carried (the revolver's cylinder).
    pub fn uses_magazine(self) -> bool {
        self != GunKind::Revolver
    }

    /// Its magazine (the standard one), for a gun that takes magazines.
    pub fn magazine_item(self) -> Option<ItemId> {
        match self {
            GunKind::Pistol => Some(PISTOL_MAGAZINE),
            GunKind::Revolver => None,
            GunKind::Ak => Some(AK_MAGAZINE),
        }
    }

    /// A long gun, held in both hands (its left hand under the handguard).
    pub fn long(self) -> bool {
        self == GunKind::Ak
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

    /// Attachments that can be fitted (a built-in scope cannot be changed; the revolver takes
    /// none).
    pub fn fits(self, bit: u8) -> bool {
        self == GunKind::Pistol && !(bit == gun_mod::SCOPE && self.stats().builtin_scope)
    }

    /// The parts a gun goes together from (at the gun station); none for one crafted whole.
    pub fn parts(self) -> &'static [ItemId] {
        match self {
            GunKind::Pistol => &[PISTOL_FRAME, PISTOL_BARREL, PISTOL_SPRING, PISTOL_SLIDE, PISTOL_MAGAZINE],
            GunKind::Revolver => &REVOLVER_PARTS,
            GunKind::Ak => &AK_PARTS,
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
            assert!(!k.uses_magazine() || k.magazine_size(gun_mod::EXTENDED_MAGAZINE) > k.magazine_size(0));
            // Rounds are kept in 6 bits of the item's data.
            assert!(k.magazine_size(gun_mod::EXTENDED_MAGAZINE) < 64);
            assert!(GunKind::of(k.ammo()).is_none() && max_stack(k.ammo()) == 64);
        }
        assert!(GunKind::Pistol.fits(gun_mod::SCOPE));
        assert!(!GunKind::Revolver.fits(gun_mod::SCOPE));
        assert!(!GunKind::Ak.fits(gun_mod::SCOPE));
        // Each magazine goes into its own gun.
        for k in GUN_KINDS {
            if let Some(m) = k.magazine_item() {
                assert_eq!(magazine_gun(m), Some(k));
                assert_eq!(magazine_capacity(m), Some(k.magazine_size(0)));
                assert_eq!(k.parts()[crate::model::gun::MAGAZINE], m);
            }
        }
    }
}
