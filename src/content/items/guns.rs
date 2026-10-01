//! The guns: the pistol, the revolver (loaded straight from the bullets carried) and the AK-47
//! (a big-calibre automatic rifle), the parts they go together from at the gun station, and
//! the attachments fitted there. Each gun's numbers (damage, rate of fire, magazine, spread,
//! recoil, reach), sounds, feed and models are its row of `WEAPONS`; its rounds and magazines
//! are in `ammo`. What a gun holds (its rounds, chamber, attachments) is `item::firearm`'s.
//! Adding a gun is a new `GunKind`, its items here and in `ammo`, a row of `WEAPONS` and its
//! models (a magazine-fed one's `pistol_view::Rig`, its third-person rig in `tp_rig`).

use super::*;
use crate::audio::Sound;
use crate::model::guns::gun::Spec;
use crate::model::guns::pistol_view::{self, Rig};
use crate::model::blockbench::{tp_ak, tp_pistol, tp_revolver};
use crate::model::rig::viewmodel::{Anim, Bone};
use glam::Vec3;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum GunKind {
    Pistol,
    Revolver,
    Ak,
}

/// Every gun, in the order of `kind as u8` (LAN messages) and of `WEAPONS`.
pub const GUN_KINDS: [GunKind; 3] = [GunKind::Pistol, GunKind::Revolver, GunKind::Ak];

/// A gun (its row in `WEAPONS` says which).
const fn gun(kind: GunKind, icon: u32, group: u8) -> ItemDef {
    ItemDef { icon: Icon::Flat(icon), gun: GunRole::Gun(kind), on_use: OnUse::Aim, creative: Creative::Tools(group), ..SINGLE }
}

/// One of a gun's parts (in its row's `parts`): a row of the tools' tab each gun.
const fn part(kind: GunKind, icon: u32) -> ItemDef {
    let group = match kind {
        GunKind::Pistol => 9,
        GunKind::Revolver => 10,
        GunKind::Ak => 11,
    };
    ItemDef { icon: Icon::Flat(icon), gun: GunRole::Part(kind), creative: Creative::Tools(group), ..ITEM }
}

/// An attachment (its `gun_mod` bit).
const fn attachment(bit: u8, icon: u32) -> ItemDef {
    ItemDef { icon: Icon::Flat(icon), gun: GunRole::Attachment(bit), creative: Creative::Tools(8), ..ITEM }
}

items! {
    after armor::END;

    /// The gun, put together at the gun station. Its `damage` is how dirty it is (one per shot;
    /// cleaned at the gun station), its `data` holds the rounds in its magazine and its
    /// attachments (`item::firearm`).
    PISTOL = ItemDef { key: "pistol", en: "Pistol", hu: "Pisztoly", ..gun(GunKind::Pistol, tex::PISTOL, 5) };
    /// A six-shot revolver, loaded straight from the bullets carried (one at a time, or six at
    /// once from a speedloader). Its `data` is its cylinder (`item::revolver_chamber`), its
    /// `damage` how dirty it is.
    REVOLVER = ItemDef { key: "revolver", en: "Revolver", hu: "Revolver", ..gun(GunKind::Revolver, tex::REVOLVER, 5) };
    /// The AK-47. Its data is laid out like the pistol's (rounds, the magazine in it, the
    /// chamber), its `damage` is how dirty it is.
    AK47 = ItemDef { key: "ak47", en: "AK-47", hu: "AK-47", ..gun(GunKind::Ak, tex::AK47, 6) };

    // The pistol's attachments.
    /// Zooms in far when aiming.
    SCOPE = ItemDef { key: "scope", en: "Scope", hu: "Távcső", ..attachment(gun_mod::SCOPE, tex::GUN_ATTACHMENTS) };
    /// No muzzle flash.
    SILENCER = ItemDef {
        key: "silencer", en: "Silencer", hu: "Hangtompító", ..attachment(gun_mod::SILENCER, tex::GUN_ATTACHMENTS + 1)
    };
    /// Steadier from the hip.
    LASER_SIGHT = ItemDef {
        key: "laser_sight", en: "Laser Sight", hu: "Lézeres célzó", ..attachment(gun_mod::LASER, tex::GUN_ATTACHMENTS + 3)
    };
    /// A weapon light for the accessory rail (instead of a laser sight): switched on and off in
    /// the hand, it lights up what the gun points at. Only in creative (it is not made).
    FLASHLIGHT = ItemDef {
        key: "weapon_light", en: "Weapon Light", hu: "Fegyverlámpa", ..attachment(gun_mod::LIGHT, tex::FLASHLIGHT)
    };

    /// The pistol's parts, in the order they go together at the gun station: the frame (with
    /// the grip and trigger), the barrel, the recoil spring and the slide (and its magazine).
    PISTOL_FRAME = ItemDef {
        key: "pistol_frame", en: "Pistol Frame", hu: "Pisztolyváz", ..part(GunKind::Pistol, tex::PISTOL_PARTS)
    };
    PISTOL_BARREL = ItemDef {
        key: "pistol_barrel", en: "Pistol Barrel", hu: "Pisztolycső", ..part(GunKind::Pistol, tex::PISTOL_PARTS + 1)
    };
    PISTOL_SPRING = ItemDef {
        key: "pistol_spring", en: "Recoil Spring", hu: "Visszatérítő rugó", ..part(GunKind::Pistol, tex::PISTOL_PARTS + 2)
    };
    PISTOL_SLIDE = ItemDef {
        key: "pistol_slide", en: "Pistol Slide", hu: "Pisztolyszán", ..part(GunKind::Pistol, tex::PISTOL_PARTS + 3)
    };
    /// The revolver's five parts, in the order of `model::gun::FRAME` ..: the frame (with the
    /// grip, trigger and sights), the barrel, the mainspring, the cylinder (on its crane, with
    /// the ejector) and the hammer.
    REVOLVER_FRAME = ItemDef {
        key: "revolver_frame", en: "Revolver Frame", hu: "Revolverváz", ..part(GunKind::Revolver, tex::REVOLVER_PARTS)
    };
    REVOLVER_BARREL = ItemDef {
        key: "revolver_barrel", en: "Revolver Barrel", hu: "Revolvercső", ..part(GunKind::Revolver, tex::REVOLVER_PARTS + 1)
    };
    REVOLVER_SPRING = ItemDef {
        key: "revolver_mainspring", en: "Mainspring", hu: "Kakasrugó", ..part(GunKind::Revolver, tex::REVOLVER_PARTS + 2)
    };
    REVOLVER_CYLINDER = ItemDef {
        key: "revolver_cylinder", en: "Revolver Cylinder", hu: "Forgótár",
        ..part(GunKind::Revolver, tex::REVOLVER_PARTS + 3)
    };
    REVOLVER_HAMMER = ItemDef {
        key: "revolver_hammer", en: "Hammer", hu: "Kakas", ..part(GunKind::Revolver, tex::REVOLVER_PARTS + 4)
    };
    /// The AK's parts: the receiver (with the barrel, sights, handguard, grip and stock), the
    /// gas tube, the bolt carrier and the dust cover with the recoil spring (and its curved
    /// 30-round magazine).
    AK_RECEIVER = ItemDef { key: "ak_receiver", en: "AK Receiver", hu: "AK-tok", ..part(GunKind::Ak, tex::AK_PARTS) };
    AK_GAS_TUBE = ItemDef { key: "ak_gas_tube", en: "AK Gas Tube", hu: "AK-gázcső", ..part(GunKind::Ak, tex::AK_PARTS + 1) };
    AK_BOLT = ItemDef { key: "ak_bolt_carrier", en: "AK Bolt Carrier", hu: "AK-zárkeret", ..part(GunKind::Ak, tex::AK_PARTS + 2) };
    AK_COVER = ItemDef { key: "ak_dust_cover", en: "AK Dust Cover", hu: "AK-tokfedél", ..part(GunKind::Ak, tex::AK_PARTS + 3) };
}

/// Each gun's parts, in the order of `model::gun::FRAME` .. (a magazine-fed gun's last one is
/// its magazine).
pub const PISTOL_PARTS: [ItemId; 5] = [PISTOL_FRAME, PISTOL_BARREL, PISTOL_SPRING, PISTOL_SLIDE, PISTOL_MAGAZINE];
pub const REVOLVER_PARTS: [ItemId; 5] = [REVOLVER_FRAME, REVOLVER_BARREL, REVOLVER_SPRING, REVOLVER_CYLINDER, REVOLVER_HAMMER];
pub const AK_PARTS: [ItemId; 5] = [AK_RECEIVER, AK_GAS_TUBE, AK_BOLT, AK_COVER, AK_MAGAZINE];

/// A gun's attachments as bits of `gun_mods`, with their items. `EXTENDED_MAGAZINE` is not
/// fitted: it says the magazine in the gun is an extended one.
pub mod gun_mod {
    pub const SCOPE: u8 = 1;
    pub const SILENCER: u8 = 2;
    pub const EXTENDED_MAGAZINE: u8 = 4;
    pub const LASER: u8 = 8;
    /// A weapon light on the accessory rail (where the laser sight goes: one or the other),
    /// and whether it is switched on (not an attachment: its state).
    pub const LIGHT: u8 = 16;
    pub const LIGHT_ON: u8 = 32;
    /// What the accessory rail holds.
    pub const RAIL: u8 = LASER | LIGHT;
}

/// The attachments (their `gun_mod` bits and items), in the order of the table.
pub static ATTACHMENTS: [(u8, ItemId); 4] = {
    let mut t = [(0, NONE); 4];
    let (mut i, mut n) = (0, 0);
    while i < ALL.len() {
        if let GunRole::Attachment(bit) = ALL[i].gun {
            t[n] = (bit, ALL[i].id);
            n += 1;
        }
        i += 1;
    }
    assert!(n == t.len());
    t
};

/// Whether an attachment (`gun_mod` bit) can go on a gun with `mods`: not one it has, and the
/// accessory rail holds one thing (a laser sight or a weapon light).
pub fn attachment_fits(mods: u8, bit: u8) -> bool {
    mods & bit == 0 && !(bit & gun_mod::RAIL != 0 && mods & gun_mod::RAIL != 0)
}

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
    /// Shots until it is too dirty to fire (its parts and magazines get as dirty).
    pub dirt_max: u16,
    /// How big its muzzle flash is.
    pub flash: f32,
}

/// How rounds get into the gun.
pub enum Feed {
    /// From a magazine (the pistol, the AK): its items, its sounds and the model's `Rig`.
    Magazine(MagazineFeed),
    /// Round by round, or all at once from its loader (`GunRole::Loader`), into a swing-out
    /// cylinder that holds `Stats::magazine` (the revolver; its model is `revolver_view`).
    Cylinder,
}

pub struct MagazineFeed {
    /// The standard magazine (it holds `Stats::magazine`), and the extended one with how many
    /// it holds, for a gun that takes one.
    pub item: ItemId,
    pub extended: Option<(ItemId, u8)>,
    /// The magazine coming out, going in, and the slide (or the bolt carrier) let go.
    pub out_sound: Sound,
    pub in_sound: Sound,
    pub rack_sound: Sound,
    /// The first-person Blockbench model, its moving parts and points.
    pub rig: &'static Rig,
}

/// One gun.
pub struct WeaponDef {
    pub kind: GunKind,
    /// The gun's item and the round it fires.
    pub item: ItemId,
    pub ammo: ItemId,
    /// The parts it goes together from at the gun station, in the order of `model::gun::FRAME`
    /// .. (a magazine-fed gun's last one is its magazine).
    pub parts: &'static [ItemId],
    /// The attachments (`gun_mod` bits) that can be fitted on it.
    pub attachments: u8,
    /// A long gun, held in both hands (its left hand under the handguard); worked on only at
    /// the rifle station.
    pub long: bool,
    pub stats: Stats,
    /// How much the view narrows aimed through a scope (a gun a scope fits).
    pub scope_zoom: Option<f32>,
    /// How big a hole its bullet leaves (blocks across).
    pub hole: f32,
    /// Its shot, and its spent case landing.
    pub shot_sound: Sound,
    pub case_sound: Sound,
    pub feed: Feed,
    /// How it is held and how big it is (gun space).
    pub spec: Spec,
    /// How a player holds it as the others see them (`tp_rig`).
    pub tp_bones: &'static [Bone],
    pub tp_anims: &'static [Anim],
}

/// Every gun, in the order of `GunKind`.
pub static WEAPONS: [WeaponDef; 3] = [
    // The 9 mm pistol: light and quick; takes attachments and an extended magazine.
    WeaponDef {
        kind: GunKind::Pistol,
        item: PISTOL,
        ammo: BULLET,
        parts: &PISTOL_PARTS,
        attachments: gun_mod::SCOPE | gun_mod::SILENCER | gun_mod::LASER | gun_mod::LIGHT,
        long: false,
        stats: Stats {
            damage: 7.0,
            knockback: 0.5,
            pellets: 1,
            fire_delay: 0.18,
            auto: false,
            magazine: 12,
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
            dirt_max: 240,
            flash: 1.0,
        },
        scope_zoom: Some(0.45),
        // The 9 mm's the smallest.
        hole: 0.085,
        shot_sound: Sound::ShotPistol,
        case_sound: Sound::CaseBrass,
        feed: Feed::Magazine(MagazineFeed {
            item: PISTOL_MAGAZINE,
            extended: Some((EXTENDED_MAGAZINE, 20)),
            out_sound: Sound::MagOut,
            in_sound: Sound::MagIn,
            rack_sound: Sound::SlideRelease,
            rig: &pistol_view::PISTOL,
        }),
        spec: Spec {
            hand: Vec3::new(-5.8, -4.4, 0.0),
            arm_scale: 0.45,
            thick: 1.7,
            bounds: (Vec3::new(-10.8, -9.8, -1.5), Vec3::new(7.5, 3.8, 1.5)),
        },
        tp_bones: tp_pistol::BONES,
        tp_anims: tp_pistol::ANIMS,
    },
    // The .357 Magnum revolver: heavier rounds, slower double action, harder kick; six in the
    // cylinder. Held like the pistol (`revolver_view::to_gun_space` puts its grip in the same
    // fist); longer (its 4.2" barrel), the cylinder bulging out either side.
    WeaponDef {
        kind: GunKind::Revolver,
        item: REVOLVER,
        ammo: MAGNUM_ROUND,
        parts: &REVOLVER_PARTS,
        attachments: 0,
        long: false,
        stats: Stats {
            damage: 12.0,
            knockback: 0.9,
            pellets: 1,
            fire_delay: 0.42,
            auto: false,
            magazine: 6,
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
            dirt_max: 300,
            flash: 1.3,
        },
        scope_zoom: None,
        hole: 0.105,
        shot_sound: Sound::ShotRevolver,
        case_sound: Sound::CaseMagnum,
        feed: Feed::Cylinder,
        spec: Spec {
            hand: Vec3::new(-5.8, -4.4, 0.0),
            arm_scale: 0.45,
            thick: 1.6,
            bounds: (Vec3::new(-9.9, -9.5, -1.8), Vec3::new(12.2, 4.0, 1.8)),
        },
        tp_bones: tp_revolver::BONES,
        tp_anims: tp_revolver::ANIMS,
    },
    // The AK-47, 7.62x39 mm: hits hard and flies far and flat (the biggest hole: the fastest
    // bullet); fully automatic, 600 rounds a minute, a 30-round magazine; it kicks with every
    // shot, so long bursts climb. The grip in the same fist as the pistol's, the left hand
    // under the handguard; on the player model smaller against the hands than the handguns
    // are (a rifle as big against the fists as they are would be longer than the player is
    // tall).
    WeaponDef {
        kind: GunKind::Ak,
        item: AK47,
        ammo: RIFLE_ROUND,
        parts: &AK_PARTS,
        attachments: 0,
        long: true,
        stats: Stats {
            damage: 11.0,
            knockback: 0.7,
            pellets: 1,
            fire_delay: 0.1,
            auto: true,
            magazine: 30,
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
            dirt_max: 900,
            flash: 1.6,
        },
        scope_zoom: None,
        hole: 0.125,
        shot_sound: Sound::ShotRifle,
        case_sound: Sound::CaseRifle,
        feed: Feed::Magazine(MagazineFeed {
            item: AK_MAGAZINE,
            extended: None,
            out_sound: Sound::MagOutRifle,
            in_sound: Sound::MagInRifle,
            rack_sound: Sound::BoltRifle,
            rig: &pistol_view::AK,
        }),
        spec: Spec {
            hand: Vec3::new(-5.8, -4.4, 0.0),
            arm_scale: 0.2,
            thick: 2.5,
            bounds: (Vec3::new(-29.0, -14.5, -1.7), Vec3::new(49.4, 4.6, 2.8)),
        },
        tp_bones: tp_ak::BONES,
        tp_anims: tp_ak::ANIMS,
    },
];

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
        match gun_role(item) {
            GunRole::Gun(k) => Some(k),
            _ => None,
        }
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

/// A gun loaded round by round into a cylinder (the revolver): no magazine, no chamber of its
/// own, no attachments (see `item::firearm`).
pub fn cylinder_gun(item: ItemId) -> bool {
    GunKind::of(item).is_some_and(|k| !k.uses_magazine())
}

/// A magazine (or a speedloader): how many rounds it holds. Its data is the rounds in it.
pub fn magazine_capacity(item: ItemId) -> Option<u8> {
    match gun_role(item) {
        GunRole::Magazine(k) => {
            let extended = k.magazine().and_then(|m| m.extended).is_some_and(|(e, _)| e == item);
            Some(k.magazine_size(if extended { gun_mod::EXTENDED_MAGAZINE } else { 0 }))
        }
        GunRole::Loader(k) => Some(k.stats().magazine),
        _ => None,
    }
}

/// A magazine that goes into a gun (not a speedloader): which gun.
pub fn magazine_gun(item: ItemId) -> Option<GunKind> {
    match gun_role(item) {
        GunRole::Magazine(k) => Some(k),
        _ => None,
    }
}

/// A magazine that goes into a gun (not a speedloader).
pub fn is_gun_magazine(item: ItemId) -> bool {
    magazine_gun(item).is_some()
}

/// Gets as dirty as its gun: the gun, its parts and its magazines (the dirt shows on them,
/// not as a bar).
pub fn gets_dirty(item: ItemId) -> bool {
    matches!(gun_role(item), GunRole::Gun(_) | GunRole::Part(_) | GunRole::Magazine(_))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_gun_is_its_own_row_and_its_row_holds_together() {
        for (i, k) in GUN_KINDS.into_iter().enumerate() {
            let d = k.def();
            assert_eq!(d.kind, k);
            assert_eq!(k as usize, i);
            assert_eq!(GunKind::of(d.item), Some(k));
            assert_eq!(gun_role(d.item), GunRole::Gun(k));
            assert_eq!(max_stack(k.item()), 1);
            assert_eq!(max_damage(k.item()), k.stats().dirt_max);
            let ext = k.magazine().and_then(|m| m.extended).is_some();
            assert_eq!(k.magazine_size(gun_mod::EXTENDED_MAGAZINE) > k.magazine_size(0), ext);
            // Rounds are kept in 6 bits of the item's data.
            assert!(k.magazine_size(gun_mod::EXTENDED_MAGAZINE) < 64);
            assert!(GunKind::of(k.ammo()).is_none() && max_stack(k.ammo()) == 64);
            // Its parts are items, its own, one each; they get dirty with it.
            assert_eq!(d.parts.len(), crate::model::guns::gun::PARTS, "{k:?}");
            for (p, &item) in d.parts.iter().enumerate() {
                assert!(!name(item).is_empty(), "{k:?} part {p}");
                assert_eq!(d.parts.iter().filter(|&&i| i == item).count(), 1);
                assert!(WEAPONS.iter().filter(|w| w.parts.contains(&item)).count() == 1);
                assert_eq!(max_damage(item), d.stats.dirt_max);
                assert!(matches!(gun_role(item), GunRole::Part(g) | GunRole::Magazine(g) if g == k));
            }
            // A scope's zoom only where a scope fits.
            assert_eq!(d.scope_zoom.is_some(), k.fits(gun_mod::SCOPE), "{k:?}");
            match &d.feed {
                Feed::Magazine(m) => {
                    // Its magazines go into it, hold what it says, and the standard one is
                    // its last part; its model is its own.
                    assert_eq!(m.rig.kind, k);
                    assert_eq!(magazine_gun(m.item), Some(k));
                    assert_eq!(magazine_capacity(m.item), Some(d.stats.magazine));
                    assert_eq!(max_stack(m.item), 1);
                    assert_eq!(d.parts[crate::model::guns::gun::MAGAZINE], m.item);
                    if let Some((ext, n)) = m.extended {
                        assert_eq!(magazine_gun(ext), Some(k));
                        assert_eq!(magazine_capacity(ext), Some(n));
                        assert_eq!(max_damage(ext), d.stats.dirt_max);
                        assert!(n > d.stats.magazine);
                    }
                    assert!(!cylinder_gun(d.item));
                }
                Feed::Cylinder => {
                    assert!(!d.parts.iter().any(|&p| magazine_gun(p).is_some()));
                    assert!(cylinder_gun(d.item));
                }
            }
            // Its round is its own, and its third-person rig has a gun in it.
            assert!(WEAPONS.iter().filter(|w| w.ammo == d.ammo).count() == 1);
            assert!(crate::model::rig::viewmodel::find_bone(d.tp_bones, "gun").is_some());
        }
        assert!(GunKind::Pistol.fits(gun_mod::SCOPE));
        assert!(!GunKind::Revolver.fits(gun_mod::SCOPE));
        assert!(!GunKind::Ak.fits(gun_mod::SCOPE));
        // Every gun's thing is in its row: its parts, its magazines, its loader.
        for d in &ITEMS {
            match d.gun {
                GunRole::Gun(k) => assert_eq!(k.item(), d.id),
                GunRole::Part(k) => assert!(k.parts().contains(&d.id) && !is_gun_magazine(d.id)),
                GunRole::Magazine(k) => {
                    let m = k.magazine().unwrap();
                    assert!(m.item == d.id || m.extended.is_some_and(|e| e.0 == d.id), "{}", d.key);
                    assert_eq!(d.stack, 1);
                }
                GunRole::Loader(k) => {
                    assert!(matches!(k.def().feed, Feed::Cylinder));
                    assert_eq!(magazine_capacity(d.id), Some(k.stats().magazine));
                    assert!(max_damage(d.id) == 0 && d.stack == 1);
                }
                GunRole::Attachment(bit) => assert!(ATTACHMENTS.contains(&(bit, d.id))),
                GunRole::None => {}
            }
        }
    }
}
