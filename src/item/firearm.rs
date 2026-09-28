//! The guns: what kinds there are, how each shoots (damage, rate of fire, magazine, spread,
//! recoil, reach) and what its five parts are at the gun station. The pistol is the only gun.

use super::*;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum GunKind {
    Pistol,
}

pub const GUN_KINDS: [GunKind; 1] = [GunKind::Pistol];

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
    dirt_max: 40,
    builtin_scope: false,
    flash: 1.0,
};

impl GunKind {
    pub fn item(self) -> ItemId {
        match self {
            GunKind::Pistol => PISTOL,
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
        }
    }

    pub fn stats(self) -> &'static Stats {
        match self {
            GunKind::Pistol => &PISTOL_STATS,
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

    /// The five parts a gun goes together from (at the gun station).
    pub fn parts(self) -> &'static [ItemId; 5] {
        match self {
            GunKind::Pistol => &[PISTOL_FRAME, PISTOL_BARREL, PISTOL_SPRING, PISTOL_SLIDE, PISTOL_MAGAZINE],
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
        assert!(GunKind::Pistol.fits(gun_mod::SCOPE));
    }
}
