//! The guns: what kinds there are (the pistol, the revolver, loaded straight from the bullets
//! carried, and the AK-47, a big-calibre automatic rifle), what says how one shoots (damage,
//! rate of fire, magazine, spread, recoil, reach) and what is asked of a gun. Each gun's
//! numbers, items and models are its row of `weapons::WEAPONS`.

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
    /// Rounds in the magazine (the standard one; the cylinder).
    pub magazine: u8,
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
    /// How much the view narrows when aimed with the sights.
    pub sight_zoom: f32,
    /// Shots until it is too dirty to fire.
    pub dirt_max: u16,
    /// How big its muzzle flash is.
    pub flash: f32,
}

impl GunKind {
    /// Its row of `WEAPONS`.
    pub fn def(self) -> &'static WeaponDef {
        &WEAPONS[self as usize]
    }

    pub fn item(self) -> ItemId {
        self.def().item
    }

    /// The gun an item is.
    pub fn of(item: ItemId) -> Option<GunKind> {
        GUN_KINDS.into_iter().find(|k| k.item() == item)
    }

    /// What it fires.
    pub fn ammo(self) -> ItemId {
        self.def().ammo
    }

    pub fn stats(self) -> &'static Stats {
        &self.def().stats
    }

    /// Takes magazines (the pistol, the AK); otherwise it is loaded round by round from the
    /// bullets carried (the revolver's cylinder).
    pub fn uses_magazine(self) -> bool {
        self.magazine().is_some()
    }

    /// How it takes magazines (None: a cylinder).
    pub fn magazine(self) -> Option<&'static MagazineFeed> {
        match &self.def().feed {
            Feed::Magazine(m) => Some(m),
            Feed::Cylinder => None,
        }
    }

    /// Its magazine (the standard one), for a gun that takes magazines.
    pub fn magazine_item(self) -> Option<ItemId> {
        self.magazine().map(|m| m.item)
    }

    /// A long gun, held in both hands (its left hand under the handguard).
    pub fn long(self) -> bool {
        self.def().long
    }

    /// Rounds the magazine holds with these attachments.
    pub fn magazine_size(self, mods: u8) -> u8 {
        match self.magazine().and_then(|m| m.extended) {
            Some((_, n)) if mods & gun_mod::EXTENDED_MAGAZINE != 0 => n,
            _ => self.stats().magazine,
        }
    }

    /// Whether an attachment (a `gun_mod` bit) can be fitted (the revolver and the AK take
    /// none).
    pub fn fits(self, bit: u8) -> bool {
        bit != 0 && self.def().attachments & bit == bit
    }

    /// The parts a gun goes together from (at the gun station).
    pub fn parts(self) -> &'static [ItemId] {
        self.def().parts
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
            let ext = k.magazine().and_then(|m| m.extended).is_some();
            assert_eq!(k.magazine_size(gun_mod::EXTENDED_MAGAZINE) > k.magazine_size(0), ext);
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
