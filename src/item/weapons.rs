//! Every gun in one table (`WEAPONS`, in the order of `GunKind`): its items, how it shoots,
//! what it sounds like, what it leaves behind, how it is fed and which Blockbench models it is.
//! Adding a gun is a new `GunKind`, a row here and its models (a magazine-fed one's
//! `pistol_view::Rig`, its third-person rig in `tp_rig`).

use super::*;
use crate::audio::Sound;
use crate::model::gun::Spec;
use crate::model::pistol_view::{self, Rig};
use crate::model::viewmodel::{Anim, Bone};
use glam::Vec3;

/// How rounds get into the gun.
pub enum Feed {
    /// From a magazine (the pistol, the AK): its items, its sounds and the model's `Rig`.
    Magazine(MagazineFeed),
    /// Round by round (or from a speedloader) into a swing-out cylinder (the revolver; its
    /// model is `revolver_view`).
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

use crate::model::tp_rig::{ak as tp_ak, pistol as tp_pistol, revolver as tp_revolver};

pub static WEAPONS: [WeaponDef; 3] = [
    // The 9 mm pistol: light and quick; takes attachments and an extended magazine.
    WeaponDef {
        kind: GunKind::Pistol,
        item: PISTOL,
        ammo: BULLET,
        parts: &[PISTOL_FRAME, PISTOL_BARREL, PISTOL_SPRING, PISTOL_SLIDE, PISTOL_MAGAZINE],
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
            // Its parts are items, its own, one each; they get dirty with it.
            assert_eq!(d.parts.len(), crate::model::gun::PARTS, "{k:?}");
            for (p, &item) in d.parts.iter().enumerate() {
                assert!(!name(item).is_empty(), "{k:?} part {p}");
                assert_eq!(d.parts.iter().filter(|&&i| i == item).count(), 1);
                assert!(WEAPONS.iter().filter(|w| w.parts.contains(&item)).count() == 1);
                assert_eq!(max_damage(item), d.stats.dirt_max);
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
                    assert_eq!(d.parts[crate::model::gun::MAGAZINE], m.item);
                    if let Some((ext, n)) = m.extended {
                        assert_eq!(magazine_gun(ext), Some(k));
                        assert_eq!(magazine_capacity(ext), Some(n));
                        assert_eq!(max_damage(ext), d.stats.dirt_max);
                        assert!(n > d.stats.magazine);
                    }
                }
                Feed::Cylinder => assert!(!d.parts.iter().any(|&p| magazine_gun(p).is_some())),
            }
            // Its round is its own, and its third-person rig has a gun in it.
            assert!(WEAPONS.iter().filter(|w| w.ammo == d.ammo).count() == 1);
            assert!(crate::model::viewmodel::find_bone(d.tp_bones, "gun").is_some());
        }
    }
}
